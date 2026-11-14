//! Villagers, boats and ridden herd members as passive actors.
use super::*;
use super::super::status;
use crate::world::travel::Ride;

#[path = "resident_model.rs"]
mod resident_model;
mod reload_check;
pub(crate) use reload_check::check as check_worker_reload;

pub(crate) const VILLAGER: u32 = 40;
pub(crate) const BOAT: u32 = 41;
const SPRITE: u32 = 43;
const DRYAD: u32 = 44;
const SATYR_RESIDENT: u32 = 45;
pub(super) fn is_villager(kind: u32) -> bool { matches!(kind, VILLAGER | SPRITE | DRYAD | SATYR_RESIDENT) }
const VILLAGER_Z: i32 = 8;
const BOAT_Z: i32 = 9;
const DRIVER_Z: i32 = 10;
const CAMPAIGN_Z: i32 = 11;
pub(crate) const VILLAGERS: u32 = 3;
const SPAWN_METRES: f32 = 75.0;
const RIDE_SPEED: f32 = 5.0;
const BOAT_SPEED: f32 = 4.0;

pub(crate) fn scuffle_reaction(actor: &mut Enemy) {
    if is_villager(actor.kind) { actor.stagger = actor.stagger.max(0.25); }
}

/// Persistent physical state for a worker whose visible body may be unloaded.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ResidentBody {
    kind: u32,
    health: f32,
    velocity: Vec3,
    yaw: f32,
    slow: f32,
    stagger: f32,
    alert: f32,
    flash: f32,
    #[serde(default)] menace: u8,
    #[serde(default)] affixes: status::Affixes,
    #[serde(default)] status: status::Status,
    #[serde(default)] shield: f32,
}

impl ResidentBody {
    pub(crate) fn capture(actor: &Enemy) -> Self {
        Self { kind: actor.kind, health: actor.health, velocity: actor.velocity, yaw: actor.yaw,
            slow: actor.slow, stagger: actor.stagger, alert: actor.alert, flash: actor.flash,
            menace: actor.menace, affixes: actor.affixes, status: actor.status, shield: actor.shield }
    }

    pub(crate) fn valid(self) -> bool {
        is_villager(self.kind) && self.health.is_finite() && (0.0..=1.0e6).contains(&self.health)
            && self.velocity.is_finite() && self.velocity.abs().max_element() <= 1000.0
            && self.yaw.is_finite()
            && self.status.stacks <= 16
            && [self.slow, self.stagger, self.alert, self.flash, self.shield,
                self.status.burn, self.status.poison, self.status.shock, self.status.wet].iter()
                .all(|v| v.is_finite() && (0.0..=86400.0).contains(v))
    }

    pub(crate) fn alive(self) -> bool { self.health > 0.0 }
    pub(crate) fn kind(self) -> u32 { self.kind }
    pub(crate) fn can_schedule(self) -> bool {
        self.alive() && !self.status.any() && self.affixes.is_empty()
            && !self.seeking_safety() && self.slow <= 0.0 && self.stagger <= 0.0
            && self.velocity.y.abs() < 0.2
    }
    pub(crate) fn parked(mut self) -> Self { self.velocity = Vec3::ZERO; self }
    pub(crate) fn health(self) -> f32 { self.health }
    pub(crate) fn with_health(mut self, health: f32) -> Self { self.health = health.max(0.0); self }
    pub(crate) fn needs_tick(self) -> bool {
        self.velocity.length_squared() > 0.000001 || self.status.any() || !self.affixes.is_empty()
            || self.seeking_safety() || self.slow > 0.0 || self.stagger > 0.0
    }
    pub(crate) fn seeking_safety(self) -> bool { self.flash > 0.0 || self.alert > 0.0 }

    fn apply(self, actor: &mut Enemy) {
        actor.kind = self.kind;
        actor.menace = self.menace;
        actor.affixes = self.affixes;
        actor.status = self.status;
        actor.shield = self.shield;
        actor.health = self.health.min(actor.max_health());
        actor.velocity = self.velocity;
        actor.yaw = self.yaw;
        actor.slow = self.slow;
        actor.stagger = self.stagger;
        actor.alert = self.alert;
        actor.flash = self.flash;
    }
}
/// Hull depth below the water line, in metres.
const DRAFT: f32 = 0.12;
const HOP_SPEED: f32 = 6.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Villager(u32),
    Driver(u32),
    Mount(u32),
    Boat(u32),
}

pub(crate) fn driver_id(e: &Enemy) -> Option<u32> {
    let id = e.site.x as u32;
    (e.kind == VILLAGER && e.site.z == DRIVER_Z && crate::world::farming::planet_freight::driver_name(id).is_some()).then_some(id)
}

pub(crate) fn remember_driver(world: &mut World, e: &Enemy) {
    if let Some(id) = driver_id(e) {
        crate::world::farming::planet_freight::remember_driver(world, id, e.health);
    }
    if let Some(id) = campaign_id(e) {
        crate::world::travel::civilization::politics::wound(world, id, e.health);
    }
}

fn campaign_id(actor: &Enemy) -> Option<u32> {
    (actor.kind == VILLAGER && actor.site.z == CAMPAIGN_Z).then_some(actor.site.x as u32)
}

pub(crate) fn campaign_present(world: &World, id: u32) -> bool {
    world.adventure.enemies.iter().any(|actor| campaign_id(actor) == Some(id) && actor.health > 0.0)
}

pub(crate) fn campaign_space(world: &World, at: Vec3) -> bool {
    cave::loaded(world, at) && free(world, at, VILLAGER) && supported(world, at, 0.25)
}

pub(crate) fn campaign_crossing(world:&World,from:Vec3,to:Vec3)->bool {
    let at=walk(world,VILLAGER,from,to-from);
    at.distance_squared(to)<0.55*0.55 && campaign_space(world,to)
}

pub(crate) fn campaign_at(world: &World, id: u32, at: Vec3) -> bool {
    world.adventure.enemies.iter().any(|actor| campaign_id(actor) == Some(id)
        && actor.health > 0.0 && actor.pos.distance_squared(at) < 0.55 * 0.55)
}

pub(in crate::world::adventure) fn hostile_campaign(actor: &Enemy) -> bool {
    (campaign_id(actor).is_some() || (villager_id(actor).is_some() && actor.site.y==1)) && actor.menace != 0
}

fn sync_campaigns(world: &mut World, viewer: Vec3) {
    use crate::world::travel::civilization::politics;
    let mut poses: std::collections::BTreeMap<_, _> = politics::actors(world, viewer).into_iter()
        .filter(|&(_, at)| campaign_space(world, at))
        .map(|(force, at)| (force.resident, (force, at))).collect();
    let politics = &world.travel.politics;
    world.adventure.enemies.retain_mut(|actor| {
        if villager_id(actor).is_some_and(|id| politics.mobilized(id) && !politics.physical_march(id)) { return false; }
        let Some(id) = campaign_id(actor) else { return true; };
        let Some((force, at)) = poses.remove(&id) else { return false; };
        actor.home = at;
        actor.health = actor.health.min(force.health);
        true
    });
    for (id, (force, at)) in poses {
        let mut actor = Enemy::new(IVec3::new(id as i32, 0, CAMPAIGN_Z), 0, VILLAGER, at, u8::from(force.hostile_to_player()));
        actor.health = force.health;
        world.adventure.enemies.push(actor);
    }
}

pub(crate) fn capture_driver_damage(world: &mut World) {
    if world.world_type != crate::world::WorldType::Planet { return; }
    if let Some((id, health)) = world.adventure.enemies.iter().find_map(|e| driver_id(e).map(|id| (id, e.health))) {
        crate::world::farming::planet_freight::remember_driver(world, id, health);
    }
}

/// Materialize one actor for the live shipment. The route continues to own movement and cargo.
pub(crate) fn sync_driver(world: &mut World, viewer: Vec3) {
    if world.world_type != crate::world::WorldType::Planet { return; }
    let pose = crate::world::farming::planet_freight::driver(world, viewer)
        .filter(|&(_, feet, ..)| feet.distance_squared(viewer) < 75.0 * 75.0
            && cave::loaded(world, feet) && free(world, feet, VILLAGER) && supported(world, feet, 0.2));
    let mut existing = None;
    world.adventure.enemies.retain_mut(|actor| {
        let Some(id) = driver_id(actor) else { return true };
        if let Some((wanted, feet, yaw, phase, gait, _)) = pose && id == wanted && existing.is_none() {
            actor.pos = feet;
            actor.yaw = yaw;
            actor.phase = phase;
            actor.gait = gait;
            existing = Some(id);
            return true;
        }
        false
    });
    if let Some((id, feet, yaw, phase, gait, health)) = pose && existing.is_none() {
        let mut actor = Enemy::new(IVec3::new(id as i32, 0, DRIVER_Z), 0, VILLAGER, feet, 0);
        actor.health = health;
        actor.yaw = yaw;
        actor.phase = phase;
        actor.gait = gait;
        world.adventure.enemies.push(actor);
    }
}

pub(crate) fn villager_id(e: &Enemy) -> Option<u32> {
    (is_villager(e.kind) && e.site.z == VILLAGER_Z).then_some(e.site.x as u32)
}
fn boat_id(e: &Enemy) -> Option<u32> {
    (e.kind == BOAT && e.site.z == BOAT_Z).then_some(e.site.x as u32)
}
fn grown_member(world: &World, e: &Enemy) -> Option<u32> {
    husbandry::member_id(e).filter(|id| world.adventure.wildlife.herd.members.get(id).is_some_and(|m| m.born.is_none()))
}

/// Villager, boat or grown herd member under the crosshair within `reach` metres.
pub(crate) fn target(world: &World, eye: Vec3, forward: Vec3, reach: f32) -> Option<Target> {
    target_distance(world, eye, forward, reach).map(|(_, target)| target)
}

pub(crate) fn target_distance(world: &World, eye: Vec3, forward: Vec3, reach: f32) -> Option<(f32, Target)> {
    world.adventure.enemies.iter()
        .filter(|e| e.health > 0.0 && passive(e.kind))
        .filter_map(|e| {
            let found = driver_id(e).map(Target::Driver).or_else(|| villager_id(e).map(Target::Villager))
                .or_else(|| boat_id(e).map(Target::Boat))
                .or_else(|| grown_member(world, e).map(Target::Mount))?;
            let c = e.center();
            let t = (c - eye).dot(forward);
            let radius = bestiary::species(e.kind).map_or(0.6, |s| s.body_width * 0.5 + 0.25);
            ((0.0..=reach).contains(&t) && (eye + forward * t).distance(c) <= radius
                && super::super::line(world, eye, c)).then_some((t, found))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

pub(crate) fn villager_centre(world: &World, id: u32) -> Option<Vec3> {
    world.adventure.enemies.iter().find(|e| villager_id(e) == Some(id) && e.health > 0.0).map(Enemy::center)
}

pub(crate) fn villager_feet(world: &World, id: u32) -> Option<Vec3> {
    villager_actor(world, id).map(|e| e.pos)
}

pub(crate) fn villager_actor(world: &World, id: u32) -> Option<&Enemy> {
    world.adventure.enemies.iter().find(|e| villager_id(e) == Some(id) && e.health > 0.0)
}

pub(crate) fn resident_dimensions(kind:u32)->Vec3 {
    let (lo,hi)=super::super::body_bounds(Vec3::ZERO,kind,1.0);
    hi-lo
}

pub(crate) fn resident_walk_speed(kind: u32) -> f32 {
    super::bestiary::species(kind).map_or(1.0, |s|s.speed)
}

pub(crate) fn spawn_resident(world: &mut World, id: u32, feet: Vec3) {
    if !world.sample_config().civilization_enabled() { return; }
    world.adventure.enemies.push(Enemy::new(IVec3::new(id as i32, 0, VILLAGER_Z), 0, VILLAGER, feet, 0));
}

pub(crate) fn seeking_safety(e: &Enemy) -> bool {
    e.flash > 0.0 || e.alert > 0.0
}

/// A resident without a presentation actor still follows the same collision mover.
/// Substeps are fixed; the owner retains any unprocessed time for the next wake.
pub(crate) fn advance_worker(world: &World, id: u32, feet: Vec3, goal: Vec3, ticks: u64,
    body: Option<ResidentBody>) -> (Vec3, ResidentBody) {
    let kind = if crate::world::farming::jobs::colony_resident(world, id) { VILLAGER }
        else { match engine_worldgen::households::role(id) {
            Some(0) => SATYR_RESIDENT, Some(2) => DRYAD, Some(3) => SPRITE, _ => VILLAGER,
        }};
    let mut actor = Enemy::new(IVec3::new(id as i32, 0, VILLAGER_Z), 0, kind, feet, 0);
    if let Some(body) = body { body.apply(&mut actor); }
    actor.home = feet;
    let mut navigation_budget = 0;
    for _ in 0..ticks.min(30) {
        let dt = engine_core::time::PLAYER_STEP_SECONDS;
        actor.flash = (actor.flash - dt).max(0.0);
        super::super::water_contact::update(&mut actor, world);
        let damage = actor.status.tick(dt, actor.affixes);
        actor.health = (actor.health - damage).min(actor.max_health());
        if damage > 0.0 { actor.flash = actor.flash.max(0.06); }
        if actor.health <= 0.0 { break; }
        navigation::gravity(&mut actor, world, dt);
        super::tick(&mut actor, world, feet + Vec3::splat(1000.0), 0.0,
            engine_core::time::PLAYER_STEP_SECONDS, Some(goal), &mut navigation_budget);
        if actor.pos.distance_squared(goal) < 0.0064 && actor.velocity.length_squared() < 0.000001
            && actor.flash == 0.0 && actor.alert == 0.0 && actor.slow == 0.0 && actor.stagger == 0.0
            && !actor.status.any() && actor.affixes.is_empty() { break; }
    }
    (actor.pos, ResidentBody::capture(&actor))
}

pub(crate) fn set_guard_rank(world: &mut World, id: u32, rank: u8) {
    if let Some(e) = world.adventure.enemies.iter_mut()
        .find(|e| villager_id(e) == Some(id) && e.health > 0.0)
    {
        let health_fraction = e.health / e.max_health();
        e.menace = rank;
        e.health = health_fraction * e.max_health();
        e.cooldown = 0.0;
    }
}

/// Height in metres a hull floats at over the water column under `p`, if that column has an open surface.
pub(crate) fn float_y(world: &World, p: Vec3) -> Option<f32> {
    let s = world.voxel_scale.meters();
    let cell = (p / s).floor().as_ivec3();
    let column = glam::IVec2::new(cell.x, cell.z);
    let top = world.water_surface_top(column, cell.y)
        .or_else(|| world.water_surface_top(column, cell.y - 2))?;
    let fill = f32::from(world.water_volume(IVec3::new(cell.x, top, cell.z))) / 8.0;
    Some((top as f32 + fill) * s - DRAFT)
}

/// Home of villager `i`: a hashed spot around the homestead, on the generated surface.
fn villager_home(world: &World, i: u32) -> Vec3 {
    if let Some(home)=world.travel.homes.get(&i){return *home;}
    if let Some(home) = crate::world::travel::civilization::home(world, i) { return home; }
    if let Some(home) = engine_worldgen::households::home(&world.sample_config(),i) {return home;}
    if let Some(home) = engine_worldgen::fernwake::resident_home(&world.sample_config(), i) {
        return home;
    }
    let anchor = crate::world::workshop::home(&world.sample_config()).unwrap_or(Vec3::ZERO);
    let h = engine_core::hash::hash_u32(world.seed ^ 0x7111_a6e5 ^ i.wrapping_mul(0x9e37_79b9));
    let angle = (h & 1023) as f32 / 1023.0 * std::f32::consts::TAU;
    let radius = 6.0 + ((h >> 10) & 255) as f32 / 255.0 * 8.0;
    let s = world.voxel_scale.meters();
    let mut p = Vec3::new(anchor.x + angle.cos() * radius, 0.0, anchor.z + angle.sin() * radius);
    let cell = (p / s).floor().as_ivec3();
    p.y = (world.terrain_height_at(cell.x, cell.z) + 1) as f32 * s + 0.02;
    p
}

/// District residents alternate between their actual work bay, shared crossing,
/// and home. The ordinary collision mover handles the journey and interruptions.
pub(super) fn daily_goal(world: &World, e: &Enemy, feet: Vec3) -> Option<Vec3> {
    let id = villager_id(e)?;
    if let Some(&post) = world.travel.guard_posts.get(&id) {
        return Some(post);
    }
    if world.travel.following == Some(id) {
        #[cfg(feature = "multiplayer")]
        let feet = world.travel.follow_owner.and_then(|owner| {
            world.multiplayer.as_ref()?.players.iter().find(|p| p.id == owner).map(|p| p.feet)
        }).unwrap_or(feet);
        return Some(feet + (e.pos - feet).normalize_or_zero() * 2.5);
    }
    if let Some(goal) = crate::world::farming::jobs::goal(world, id, e.pos) {
        return Some(goal);
    }
    if let Some(home)=world.travel.homes.get(&id){return Some(*home);}
    if let Some(goal) = crate::world::travel::civilization::goal(world, id, e.pos) { return Some(goal); }
    if let Some(goal) = crate::world::farming::freight::work_goal(world, id) {
        return Some(goal);
    }
    if let Some(goal) = crate::world::farming::pantry::work_goal(world, id) {
        return Some(goal);
    }
    let day = crate::world::travel::DAY_TICKS;
    let shift = u64::from(engine_core::hash::hash_u32(id)) % (day / 8);
    let phase = (world.farming.clock % day + day - day / 16 + shift) % day;
    let at_home = !(day / 4..day * 3 / 4).contains(&phase);
    if let Some(goal) = engine_worldgen::civilization::daily_destination(&world.sample_config(), id, e.pos, at_home) {
        return Some(goal);
    }
    if !(3..8).contains(&id) || !engine_worldgen::fernwake::active(&world.sample_config()) { return None; }
    if at_home { return Some(e.home); }
    let goal = if phase < day/2 {
        match id {
            3 => Vec3::new(405.5,0.02,38.5),
            4 => Vec3::new(426.5,0.02,59.5),
            5 => Vec3::new(463.5,0.02,54.5),
            7 => Vec3::new(589.5,0.02,40.5),
            _ => Vec3::new(443.5,0.02,62.5),
        }
    } else {
        // Remain on the same side of the tree rows and house partitions.
        match id {
            3 => Vec3::new(413.5,0.02,44.5),
            4 => Vec3::new(429.5,0.02,51.5),
            5 => Vec3::new(461.5,0.02,51.5),
            7 => Vec3::new(590.5,0.02,52.5),
            _ => Vec3::new(443.5,0.02,56.5),
        }
    };
    Some(goal + Vec3::Y * engine_worldgen::habitats_region::floor(&world.sample_config()))
}

/// Mirrors boat bodies into their records, then once a second loads nearby villagers and boats.
pub(crate) fn service(world: &mut World, feet: Vec3) {
    let today = crate::world::travel::day(world);
    for e in &world.adventure.enemies {
        if let Some(id)=villager_id(e) && world.travel.following==Some(id){world.travel.homes.insert(id,e.pos);}
        if let Some(b) = boat_id(e).and_then(|id| world.travel.boats.get_mut(&id)) {
            b.pos = e.pos;
            b.yaw = e.yaw;
        }
    }
    if !world.farming.clock.is_multiple_of(120) {
        return;
    }
    let mut trace = crate::gameplay_trace::Trace::new();
    use crate::world::travel::civilization;
    civilization::service(world, feet);
    trace.mark("residents-history");
    let mut actors = std::mem::take(&mut world.adventure.enemies);
    actors.retain(|actor| {
        let Some(id) = villager_id(actor) else { return true; };
        civilization::present(world, id)
            && (world.travel.homes.contains_key(&id) || world.travel.guard_posts.contains_key(&id)
                || civilization::home(world, id).is_none_or(|home| actor.home == home))
    });
    world.adventure.enemies = actors;
    let w: &mut World = world;
    let mut spawns = Vec::new();
    let living = w.sample_config().civilization_enabled();
    for actor in &mut w.adventure.enemies {
        let Some(id)=villager_id(actor) else {continue;};
        if w.travel.politics.physical_march(id) {
            let hostile=w.travel.politics.hostile_march(id);
            actor.site.y=if hostile {1} else {2};
            actor.menace=u8::from(hostile);
        } else if matches!(actor.site.y,1|2) {
            actor.site.y=0;actor.menace=0;
        }
    }
    let district = if living && engine_worldgen::fernwake::active(&w.sample_config()) {
        engine_worldgen::fernwake::RESIDENTS + u32::from(engine_worldgen::fernwake::freight::active(&w.sample_config()))
    } else { 0 };
    let households = if living { engine_worldgen::households::nearby(&w.sample_config(), feet) } else { Vec::new() };
    let workers:std::collections::BTreeMap<_,_> = if living {
        crate::world::farming::jobs::crew::saved_workers_near(w,feet,SPAWN_METRES).into_iter().collect()
    } else {std::collections::BTreeMap::new()};
    let mut identities:Vec<_>=(0..if living {VILLAGERS+district} else {0}).chain(households.iter().map(|s|s.id))
        .chain(civilization::residents(w, feet)).chain(w.travel.homes.keys().copied())
        .chain(w.travel.guard_posts.keys().copied()).chain(workers.keys().copied())
        .filter(|_| living).collect();
    identities.sort_unstable();identities.dedup();
    trace.mark("residents-discovery");
    for i in identities {
        if !civilization::present(w, i) || w.travel.away.get(&i) == Some(&today)
            || w.adventure.enemies.iter().any(|e| villager_id(e) == Some(i)) {
            continue;
        }
        let colony = crate::world::farming::jobs::colony_resident(w,i);
        let kind = if colony {VILLAGER} else {match engine_worldgen::households::role(i) {
            Some(0)=>SATYR_RESIDENT,Some(2)=>DRYAD,Some(3)=>SPRITE,_=>VILLAGER}};
        let worker = workers.get(&i).copied();
        let footing = |p| if worker.is_some() {resident_footing(w,p,kind,0.2)} else {supported(w,p,0.2)};
        let post = w.travel.guard_posts.get(&i).copied();
        // These three legacy villagers live within 14 m of chart origin on
        // Planet. Reject distant homes before a cold terrain-height query can
        // generate an unrelated region on the gameplay thread. Saved moves,
        // posts and workers still use their actual positions below.
        if i < VILLAGERS && w.world_type == crate::world::WorldType::Planet
            && post.is_none() && worker.is_none() && !w.travel.homes.contains_key(&i)
            && feet.x * feet.x + feet.z * feet.z > (SPAWN_METRES + 14.0).powi(2)
        {
            continue;
        }
        let restoring = post.is_none() && worker.is_some();
        // Saved workers can be between steps. Defer unsafe poses instead of relocating them.
        let home = post.or(worker).unwrap_or_else(||villager_home(w,i));
        if home.distance_squared(feet) <= SPAWN_METRES * SPAWN_METRES && cave::loaded(w, home)
            && free(w, home, kind) && (restoring || footing(home)) {
            let mut e = Enemy::new(IVec3::new(i as i32, 0, VILLAGER_Z), 0, kind, home, if post.is_some() { 2 } else { 0 });
            if restoring && let Some(body) = crate::world::farming::jobs::crew::body(w, i) {
                body.apply(&mut e);
            }
            if w.travel.politics.physical_march(i) {
                let hostile=w.travel.politics.hostile_march(i);
                e.site.y=if hostile {1} else {2};e.menace=u8::from(hostile);
            }
            e.home = home;
            spawns.push(e);
        }
    }
    for (&id, b) in &w.travel.boats {
        if b.pos.distance_squared(feet) <= SPAWN_METRES * SPAWN_METRES
            && !w.adventure.enemies.iter().any(|e| boat_id(e) == Some(id))
        {
            let mut e = Enemy::new(IVec3::new(id as i32, 0, BOAT_Z), 0, BOAT, b.pos, 0);
            e.home = b.pos;
            e.yaw = b.yaw;
            spawns.push(e);
        }
    }
    world.adventure.enemies.extend(spawns);
    trace.mark("residents-spawn");
    sync_campaigns(world, feet);
}

/// A destroyed boat forgets its record; a villager stays away for the rest of the day.
pub(super) fn fell(world: &mut World, e: &Enemy) {
    if let Some(id) = boat_id(e) {
        world.travel.boats.remove(&id);
        if world.travel.riding == Some(Ride::Boat(id)) {
            world.travel.riding = None;
        }
    } else if let Some(id) = villager_id(e) {
        crate::world::farming::jobs::worker_fell(world, id, e.pos);
        if world.travel.following==Some(id){
            world.travel.following=None;
            world.travel.follow_owner=None;
        }
        let today = crate::world::travel::day(world);
        world.travel.away.insert(id, today);
    } else if let Some(id) = husbandry::member_id(e)
        && world.travel.riding == Some(Ride::Mount(id))
    {
        world.travel.riding = None;
    }
}

fn ridden(world: &World, e: &Enemy) -> bool {
    boat_id(e).is_some_and(|id| world.travel.occupied(Ride::Boat(id)))
        || husbandry::member_id(e).is_some_and(|id| world.travel.occupied(Ride::Mount(id)))
}

/// Boats float in place and ridden bodies wait for the rider. True when the actor is handled.
pub(super) fn tick(e: &mut Enemy, world: &World, dt: f32) -> bool {
    if driver_id(e).is_some() { return true; }
    if let Some(id) = villager_id(e)
        && ResidentBody::capture(e).can_schedule()
        && let Some(at) = crate::world::farming::jobs::activity::pose(world,id)
    {
        let moved = at-e.pos;
        e.pos = at;
        e.velocity = Vec3::ZERO;
        super::super::enemy_presentation::advance_walk(e,moved,moved.length()/dt.max(0.0001),dt);
        if moved.length_squared() > 0.000001 { e.yaw = super::super::local_yaw(e.up,moved); }
        return true;
    }
    if campaign_id(e).is_some() {
        e.cooldown = (e.cooldown - dt).max(0.0);
        let delta = (e.home - e.pos) * Vec3::new(1.0, 0.0, 1.0);
        let before = e.pos;
        if e.tell <= 0.0 && e.stagger <= 0.0 {
            e.pos = walk(world, e.kind, e.pos, delta.clamp_length_max(1.65 * dt));
        }
        let moved = e.pos - before;
        super::super::enemy_presentation::advance_walk(e, moved, 1.65, dt);
        if moved.length_squared() > 0.000001 { e.yaw = super::super::local_yaw(e.up, moved); }
        e.stagger = (e.stagger - dt).max(0.0);
        return true;
    }
    if e.kind == BOAT {
        e.visible = false;
        e.gait = 0.0;
        match float_y(world, e.pos) {
            Some(y) => e.pos.y += (y - e.pos.y).clamp(-dt * 2.0, dt * 2.0),
            None if free(world, e.pos - Vec3::Y * dt * 3.0, BOAT) => e.pos.y -= dt * 3.0,
            None => {}
        }
        return true;
    }
    if ridden(world, e) {
        e.visible = false;
        e.alert = 0.0;
        return true;
    }
    false
}

/// Moves `pos` by `delta` over walkable ground, climbing one-metre steps; airborne bodies only need room.
fn walk(world: &World, kind: u32, pos: Vec3, delta: Vec3) -> Vec3 {
    let steps = (delta.length() / 0.12).ceil().max(1.0) as usize;
    let grounded = supported(world, pos, 0.2);
    let mut at = pos;
    for _ in 0..steps {
        let q = at + delta / steps as f32;
        if free(world, q, kind) && (!grounded || supported(world, q, 1.1)) {
            at = q;
            continue;
        }
        let Some(raised) = [0.25, 0.5, 0.75, 1.0].into_iter().map(|lift| q + Vec3::Y * lift)
            .find(|&r| grounded && supported(world, r, 0.2) && free(world, r, kind)) else { break };
        at = raised;
    }
    at
}

/// Steers the ridden actor along `heading` and returns the rider's feet in metres, or `None` once it is gone.
pub(crate) fn drive(world: &mut World, ride: Ride, heading: Vec3, hop: bool, dt: f32) -> Option<Vec3> {
    let i = world.adventure.enemies.iter().position(|e| e.health > 0.0 && match ride {
        Ride::Boat(id) => boat_id(e) == Some(id),
        Ride::Mount(id) => husbandry::member_id(e) == Some(id),
    })?;
    let e = &world.adventure.enemies[i];
    let (kind, pos) = (e.kind, e.pos);
    let w: &World = world;
    let (next, seat, jump) = if kind == BOAT {
        let q = pos + heading * BOAT_SPEED * dt;
        let next = float_y(w, q).map(|y| Vec3::new(q.x, y, q.z)).filter(|&n| free(w, n, BOAT)).unwrap_or(pos);
        (next, 0.15, false)
    } else {
        let next = walk(w, kind, pos, heading * RIDE_SPEED * dt);
        let height = bestiary::species(kind).map_or(1.0, |s| s.body_height);
        (next, height * 0.6, hop && supported(w, pos, 0.2))
    };
    let e = &mut world.adventure.enemies[i];
    let moved = (next - e.pos) * Vec3::new(1.0, 0.0, 1.0);
    e.pos = next;
    e.home = next;
    if jump {
        e.velocity.y = HOP_SPEED;
    }
    let travel = moved.length();
    super::super::enemy_presentation::advance_walk(e, moved, RIDE_SPEED, dt);
    if travel > 0.0001 {
        e.yaw = super::super::local_yaw(e.up, moved);
    }
    Some(e.pos + Vec3::Y * seat)
}

/// Villager and boat bodies. True when `e` was drawn here.
pub(super) fn draw(e: &Enemy, m: &mut crate::mesh::models::Model, time: f32, world: &World) -> bool {
    let mark = m.mark();
    let at = |x: f32, y: f32, z: f32| e.pos + Vec3::new(x, y, z);
    let r = Quat::IDENTITY;
    match e.kind {
        SATYR_RESIDENT => {
            super::super::folklore::satyr(e, m, 0.0);
        }
        SPRITE | DRYAD => {
            let small = e.kind == SPRITE;
            let scale = if small { 0.34 } else { 1.0 };
            let bark = if small { [0.60,0.41,0.24] } else { [0.34,0.26,0.17] };
            let leaf = if small { [0.63,0.38,0.20] } else { [0.31,0.49,0.24] };
            let sway = (time*1.5+e.site.x as f32).sin()*0.025;
            let mut part = |p: Vec3, size: Vec3, color| m.material_box(e.pos+p*scale,size*scale,r,color,70.0,0.0);
            for side in [-1.0,1.0] {
                let offset = if side < 0.0 { 0.0 } else { std::f32::consts::PI };
                let (stride, lift) = super::super::enemy_presentation::walk_scaled(e, 0.13, 0.06, offset, 1.0, scale);
                part(Vec3::new(side*0.15,0.4+lift,stride),Vec3::new(0.17,0.8,0.20),bark);
                part(Vec3::new(side*0.34,1.1+sway,0.0),Vec3::new(0.15,0.65,0.18),bark);
            }
            part(Vec3::new(0.0,1.05,0.0),Vec3::new(0.48,0.65,0.30),leaf);
            part(Vec3::new(0.0,1.60+sway,0.0),Vec3::new(0.40,0.42,0.35),bark);
            for side in [-1.0,1.0] {
                part(Vec3::new(side*0.1,1.65+sway,-0.185),Vec3::new(0.055,0.055,0.025),[0.90,0.80,0.44]);
                part(Vec3::new(side*0.26,1.86+sway,0.0),Vec3::new(0.4,0.2,0.4),leaf);
            }
            part(Vec3::new(0.0,1.98+sway,0.0),Vec3::new(0.45,0.18,0.42),leaf);
        }
        VILLAGER => {
            let id = e.site.x as u32;
            let (work, cargo) = crate::world::farming::jobs::presentation(world, id);
            let work = work.or_else(|| crate::world::travel::civilization::work_pose(world, id, e.pos));
            let stationed = campaign_id(e).is_some() || world.travel.guard_posts.contains_key(&(e.site.x as u32));
            resident_model::draw(e, m, time, work, cargo, stationed);
        }
        BOAT => {
            let wood = [0.55, 0.38, 0.22];
            let dark = [0.40, 0.27, 0.16];
            m.cuboid(at(0.0, 0.05, 0.0), Vec3::new(1.20, 0.10, 2.0), r, dark, 0.0);
            for side in [-0.56, 0.56] {
                m.cuboid(at(side, 0.24, 0.0), Vec3::new(0.08, 0.36, 2.0), r, wood, 0.0);
            }
            for end in [-0.96, 0.96] {
                m.cuboid(at(0.0, 0.24, end), Vec3::new(1.20, 0.36, 0.08), r, wood, 0.0);
            }
            m.cuboid(at(0.0, 0.28, 0.25), Vec3::new(1.04, 0.06, 0.30), r, wood, 0.0);
        }
        _ => return false,
    }
    let flash = (e.flash / 0.18).clamp(0.0, 1.0) * 0.5;
    if flash > 0.0 {
        for v in &mut m.vertices[mark..] {
            for c in v.color.iter_mut() { *c = *c * (1.0 - flash) + flash; }
        }
    }
    let up = if e.up.length_squared() > 0.5 { e.up.normalize() } else { Vec3::Y };
    m.rotate_since(mark, e.pos, Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(e.yaw));
    true
}

/// Review hook: villager 0 held at `villager` and one boat record at `boat`, in metres.
pub(crate) fn review_scene(world: &mut World, villager: Vec3, boat: Vec3) {
    if let Some(e) = world.adventure.enemies.iter_mut().find(|e| villager_id(e) == Some(0)) {
        e.pos = villager;
        e.home = villager;
        e.yaw = 2.7;
        e.gait = 0.0;
        return;
    }
    let mut e = Enemy::new(IVec3::new(0, 0, VILLAGER_Z), 0, VILLAGER, villager, 0);
    e.home = villager;
    e.yaw = 2.7;
    world.adventure.enemies.push(e);
    if world.travel.boats.is_empty() {
        world.travel.add_boat(crate::world::travel::Boat { pos: boat, yaw: 0.6 });
    }
}
