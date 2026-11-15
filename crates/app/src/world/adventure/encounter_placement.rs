use super::*;
use crate::world::{WorldSnapshot, chunk_coord_from_world_block as chunk_of, collision::collides_with};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

#[derive(Default)]
pub(super) struct Placement {
    reply: Option<Mutex<mpsc::Receiver<Plan>>>,
    plan: Option<Plan>,
}

struct Job {
    snapshot: WorldSnapshot,
    player: Vec3,
    records: HashMap<IVec3, u32>,
    active: HashSet<(IVec3, u32)>,
    reply: mpsc::SyncSender<Plan>,
}

struct Plan {
    snapshot: WorldSnapshot,
    player: Vec3,
    nearby: Vec<IVec3>,
    sites: VecDeque<Site>,
}

struct Site {
    id: IVec3,
    center: Vec3,
    actors: Vec<Actor>,
}

struct Actor {
    slot: u32,
    pos: Vec3,
    kind: u32,
    menace: u8,
    affixes: status::Affixes,
    ambush: Option<encounters::Mechanic>,
    min: Vec3,
    max: Vec3,
    chunks: HashSet<IVec3>,
}

fn lane() -> Option<&'static mpsc::SyncSender<Job>> {
    static LANE: OnceLock<Option<mpsc::SyncSender<Job>>> = OnceLock::new();
    LANE.get_or_init(|| {
        let (send, receive) = mpsc::sync_channel::<Job>(2);
        std::thread::Builder::new().name("encounter-placement".into()).spawn(move || {
            while let Ok(job) = receive.recv() {
                let _ = job.reply.send(build(&job));
            }
        }).ok()?;
        Some(send)
    }).as_ref()
}

impl Adventure {
    pub(super) fn request_encounters(&mut self, world: &mut World, player: Vec3) {
        if self.encounter_placement.reply.is_some() || self.encounter_placement.plan.is_some() {
            return;
        }
        let Some(lane) = lane() else { return };
        let (reply, receive) = mpsc::sync_channel(1);
        let job = Job {
            snapshot: world.snapshot(),
            player,
            records: self.records.clone(),
            active: self.enemies.iter().map(|e| (e.site, e.slot)).collect(),
            reply,
        };
        if lane.try_send(job).is_ok() {
            self.encounter_placement.reply = Some(Mutex::new(receive));
        }
    }

    pub(super) fn publish_encounters(&mut self, world: &mut World, player: Vec3) {
        if let Some(reply) = self.encounter_placement.reply.as_mut() {
            match reply.get_mut().unwrap().try_recv() {
                Ok(plan) => self.encounter_placement.plan = Some(plan),
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
            self.encounter_placement.reply = None;
        }
        let Some(mut plan) = self.encounter_placement.plan.take() else { return };
        if plan.snapshot.cfg != world.sample_config()
            || !Arc::ptr_eq(&plan.snapshot.regions.root, &world.regions.view.root)
            || player.distance_squared(plan.player) > 24.0 * 24.0
        {
            return;
        }
        // Publish at most one site per tick. Terrain probes and affix sampling
        // already ran on the worker.
        let Some(site) = plan.sites.pop_front() else { return };
        if self.admit_record(site.id, &plan.nearby) {
            let record = self.records.entry(site.id).or_default();
            if *record & 256 == 0 { *record |= reliquaries::PHYSICAL; }
            let record = *record;
            let count = encounters::slots(site.id);
            if record & ((1 << count) - 1) == (1 << count) - 1 && record & 256 == 0 {
                self.complete_site(world, site.id, site.center);
            }
            let mut active = self.enemies.iter().filter(|e| e.habitat.is_none() && !wildlife::passive(e.kind)).count();
            for actor in site.actors {
                if active >= MAX_ENEMIES { break; }
                if record & (1 << actor.slot) != 0
                    || self.enemies.iter().any(|e| e.site == site.id && e.slot == actor.slot)
                    || player.distance_squared(actor.pos) < 64.0
                    || actor.chunks.iter().any(|&c| !unchanged(world, &plan.snapshot, c))
                    || world.ca_reservations.iter().any(|&p| {
                        p.cmpge(actor.min.floor().as_ivec3()).all()
                            && p.cmple((actor.max - Vec3::splat(0.001)).floor().as_ivec3()).all()
                    })
                {
                    continue;
                }
                active += usize::from(!wildlife::passive(actor.kind));
                let mut enemy = Enemy::new(site.id, actor.slot, actor.kind, actor.pos, actor.menace)
                    .with_affixes(actor.affixes);
                if let Some(mechanic) = actor.ambush {
                    enemy.encounter.specialize(mechanic);
                }
                self.enemies.push(enemy);
            }
        }
        if !plan.sites.is_empty() { self.encounter_placement.plan = Some(plan); }
    }
}

fn unchanged(world: &World, snapshot: &WorldSnapshot, c: IVec3) -> bool {
    match (world.chunks.get(&c), snapshot.chunks.get(&c)) {
        (Some(now), Some(then)) => Arc::ptr_eq(now, then),
        (None, None) => {
            let identity = |record: &crate::world::regions::Record| (record.version, record.checksum);
            world.regions.view.records.get(&c).map(identity) == snapshot.regions.records.get(&c).map(identity)
        }
        _ => false,
    }
}

fn build(job: &Job) -> Plan {
    let snapshot = &job.snapshot;
    let cfg = snapshot.cfg;
    let encounters = sites::encounters(&cfg, job.player);
    let nearby = encounters.iter().map(|e| e.0).collect();
    let mut read = snapshot.voxel_reader();
    let sites = encounters.into_iter().map(|(id, center, kind)| {
        let record = job.records.get(&id).copied().unwrap_or_default();
        let actors = (0..encounters::slots(id)).filter(|&slot| {
            record & (1 << slot) == 0 && !job.active.contains(&(id, slot))
        }).filter_map(|slot| {
            let (kind, menace) = actor_kind(cfg, id, center, kind, slot);
            probe(cfg, &mut read, job.player, id, center, slot, kind, menace)
        }).collect();
        Site { id, center, actors }
    }).collect();
    Plan { snapshot: snapshot.clone(), player: job.player, nearby, sites }
}

fn probe(
    cfg: engine_worldgen::WorldSampleConfig, read: &mut impl FnMut(IVec3) -> crate::world::BlockId,
    player: Vec3, id: IVec3, center: Vec3, slot: u32, kind: u32, menace: u8,
) -> Option<Actor> {
    let scale = cfg.voxel_meters();
    let floor = cfg.min_world_y();
    for attempt in 0..12 {
        let angle = slot as f32 * 2.1 + attempt as f32 * 0.83;
        let xz = Vec3::new(angle.cos(), 0.0, angle.sin()) * (3.0 + attempt as f32 * 0.25);
        for dy in [0.0, 1.0, -1.0, 2.0, -2.0] {
            let pos = center + xz + Vec3::Y * dy;
            if pos.distance_squared(player) < 64.0 || engine_worldgen::fernwake::refuge(&cfg, pos) {
                continue;
            }
            let mut chunks = HashSet::new();
            let mut get = |p| {
                chunks.insert(chunk_of(p));
                read(p)
            };
            let support = ((pos - Vec3::Y * 0.2) / scale).floor().as_ivec3();
            if !flying(kind) && support.y > floor && !get(support).is_solid() { continue; }
            let (min, max) = body_bounds(pos, kind, scale);
            if collides_with(cfg, min, max, IVec3::ZERO, &mut get, None) { continue; }
            let kind = aberrations::orient(pos, kind, scale, |p| p.y <= floor || get(p).is_solid());
            let (pos, min, max, kind, ambush) = if engine_worldgen::features::legacy_preview(cfg.generator_version, engine_voxel::DUNGEON_MECHANISMS_GENERATOR)
                && matches!(id.z, 1 | 5) && !sites::district_post(id)
                && slot == 0 && id.x.wrapping_add(id.y).rem_euclid(3) == 0
            {
                hidden_spawn(cfg, &mut get, pos, kind, id.x.wrapping_sub(id.y) & 1 == 0)
            } else {
                (pos, min, max, kind, None)
            };
            let affixes = if id == IVec3::ZERO && crate::world::workshop::has_lanterncap(&cfg) {
                status::Affixes::default()
            } else {
                let rolled = if is_boss(kind) { status::Affixes::default() }
                    else { status::roll(cfg.seed, id.x, id.y, slot, menace, zone_archetype(cfg, pos)) };
                modifiers::affixes(modifiers::for_site(&cfg, id), rolled)
            };
            return Some(Actor { slot, pos, kind, menace, affixes, ambush, min, max, chunks });
        }
    }
    None
}

fn hidden_spawn(
    cfg: engine_worldgen::WorldSampleConfig,
    get: &mut impl FnMut(IVec3) -> crate::world::BlockId,
    pos: Vec3,
    kind: u32,
    prefer_ceiling: bool,
) -> (Vec3, Vec3, Vec3, u32, Option<encounters::Mechanic>) {
    let scale = cfg.voxel_meters();
    // Use the same collision bounds as normal placement. A perch has solid
    // overhead support, a clear shaft below it and a normal ground spawn.
    for i in 1..=(5.0 / scale).ceil() as i32 {
        let ceiling = ((pos + Vec3::Y * (i as f32 * scale)) / scale).floor().as_ivec3();
        if !get(ceiling).is_solid() {
            continue;
        }
        let perch = Vec3::new(pos.x, ceiling.y as f32 * scale - 1.05, pos.z);
        if prefer_ceiling && perch.y - pos.y >= 1.8 {
            let (min, max) = body_bounds(perch, 3, scale);
            let (_, bottom_max) = body_bounds(pos, 3, scale);
            let shaft_min = Vec3::new(min.x, pos.y / scale, min.z);
            if max.y >= bottom_max.y && !collides_with(cfg, shaft_min, max, IVec3::ZERO, &mut *get, None) {
                return (perch, min, max, 3, Some(encounters::Mechanic::CeilingDrop));
            }
        }
        let (min, max) = body_bounds(pos, 11, scale);
        if !collides_with(cfg, min, max, IVec3::ZERO, get, None) {
            return (pos, min, max, 11, Some(encounters::Mechanic::LightReveal));
        }
        break;
    }
    let (min, max) = body_bounds(pos, kind, scale);
    (pos, min, max, kind, None)
}

fn actor_kind(cfg: engine_worldgen::WorldSampleConfig, id: IVec3, center: Vec3, kind: u32, slot: u32) -> (u32, u8) {
    if id == IVec3::ZERO && crate::world::workshop::has_lanterncap(&cfg) {
        return (if slot == 0 { 0 } else { 4 }, 0);
    }
    let variation = (id.x.wrapping_mul(31) ^ id.y.wrapping_mul(17) ^ cfg.seed as i32)
        .unsigned_abs();
    let menace = (kind >> 8) as u8;
    let kind = kind & 0xFF;
    let mut kind = if slot == 0 {
        kind
    } else {
        // Escorts contrast the lead by role: ranged pressure or a
        // flyer beside a brawler, a defender beside a rooted body.
        const BY_LEAD: [[u32; 2]; 6] = [[4, 3], [5, 0], [0, 4], [0, 1], [2, 5], [1, 4]];
        let pair = BY_LEAD[(kind as usize) % 6];
        let pick = pair[((variation as usize) + slot as usize) % 2];
        if pick == kind { pair[0] } else { pick }
    };
    // Deep rooms and some escorts use the broader bestiary. These
    // selections are deterministic from the site id and seed, so a
    // cleared record can never resurrect a different family on load.
    if id.z == 2 || id.z == 3 {
        if variation.is_multiple_of(2) {
            kind = 10; // Threshold Mimic in a cache room.
        }
    } else if id.z >= 4 || (slot > 0 && variation.is_multiple_of(3)) {
        const DEEP: [u32; 4] = [10, 11, 12, 13];
        kind = DEEP[((variation / 7 + slot + id.z.unsigned_abs()) % 4) as usize];
    }
    // High-menace sites occasionally field a size-varied brute.
    // Keeping it rare preserves the existing encounter rhythm while
    // making a dangerous site read differently at a glance.
    if menace >= 2 && slot == 0 && variation.is_multiple_of(3) && !sites::district_post(id) {
        kind = 9;
    }
    // District posts field machines only; the lead picks the mix.
    if sites::district_post(id) {
        kind = if slot == 0 { kind } else if (variation + slot) % 3 == 0 { robots::HOUND } else { robots::SENTRY };
    }
    (aberrations::at_depth(cfg, center, kind, variation, slot), menace)
}
