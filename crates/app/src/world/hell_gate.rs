//! Local realm travel: checkpoint both sides before replacing the live world.
use super::{World, WorldType, storage::{self, SaveService}};
use crate::player::Player;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

const FILE: &str = "hell-gate.ron";
const BUILT_FILE: &str = "hell-portals.ron";
pub mod appearance;
pub mod built;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    parent: String, nether: String, seed: u32, generator: u32,
    #[serde(default)]
    nether_generator: Option<u32>,
}

impl Link {
    fn validate(&self, w: &World) -> Result<()> {
        let valid = |s: &str| !s.is_empty() && s.len() <= 96 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        ensure!(valid(&self.parent) && valid(&self.nether) && self.parent != self.nether, "Invalid gate link");
        ensure!(self.seed == w.seed && self.version(w.world_type) == w.generator_version, "Gate world mismatch");
        let here = w.regions.view.session_name();
        ensure!(here == if w.world_type == WorldType::Nether { &self.nether } else { &self.parent }, "Gate realm mismatch");
        Ok(())
    }

    fn version(&self, kind: WorldType) -> u32 {
        if kind == WorldType::Nether { self.nether_generator.unwrap_or(self.generator) } else { self.generator }
    }

    fn publish(&self, root: &Path, file: &str) -> Result<()> {
        let path = root.join(file);
        let text = ron::to_string(self)?;
        if path.exists() {
            let stored: Self = ron::from_str(&fs::read_to_string(&path)?)?;
            ensure!(stored == *self, "Gate link changed");
            return Ok(());
        }
        fs::create_dir_all(root)?;
        let tmp = root.join(format!("hell-gate-{}-{}.tmp", std::process::id(), super::regions::checkpoint_id()));
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(tmp, path)?;
        Ok(())
    }
}

struct Destination {
    world: World, link: Link, saves: SaveService, file: &'static str,
    previous: glam::Vec3, previous_precise: glam::DVec3,
}

#[derive(Default)]
pub struct Travel {
    requested: bool, destination: Option<Destination>, near_gate: bool,
    previous_eye: Option<(String, glam::Vec3)>,
}

pub fn focused(w: &World, player: &Player) -> bool {
    if player.anchor.is_some() { return false; }
    if w.adventure.health > 0.0 && built::near(w, player).is_some() { return true; }
    let cfg = w.sample_config();
    let feet = player.pos * w.voxel_scale.meters();
    if !engine_worldgen::hell_gate::focus(&cfg, feet) || w.adventure.health <= 0.0 { return false; }
    let c = engine_worldgen::hell_gate::center(&cfg, feet);
    let base = engine_worldgen::hell_gate::floor(&cfg, c);
    let target = c + glam::Vec3::Y * (base + 2.0);
    let to = target - player.eye(w.voxel_scale) * w.voxel_scale.meters();
    to.normalize_or_zero().dot(player.forward_dir()) > 0.35 && intact(w, c, base)
        && super::raycast_block(w, player.eye(w.voxel_scale), to.normalize_or_zero(),
            to.length() / w.voxel_scale.meters()).is_none()
}

pub fn intact(w: &World, center: glam::Vec3, base: f32) -> bool {
    [(-2.8, 2.0), (2.8, 2.0), (0.0, 5.4)].into_iter().all(|(x, y)| {
        let p = center + glam::Vec3::new(x, base + y, 0.0);
        w.get((p / w.voxel_scale.meters()).floor().as_ivec3()) == super::BlockId::Basalt
    })
}

pub fn prompt(w: &World, player: &Player, network: bool) -> Option<super::interaction::Prompt> {
    if player.anchor.is_some() { return None; }
    if let Some(prompt) = built::prompt(w, player, network) { return Some(prompt); }
    focused(w, player).then_some(super::interaction::Prompt {
        title: engine_worldgen::hell_gate::INSCRIPTION,
        action: if network { "Realm crossing requires local play" }
            else if w.world_type == WorldType::Nether { "F  Return" } else { "F  Enter" },
    })
}

/// Exchange the actor, retaining each realm's enemies, mechanisms and drops.
fn exchange_actor(a: &mut World, b: &mut World) {
    a.adventure.swap_realm_shared(&mut b.adventure);
    std::mem::swap(&mut a.adventure, &mut b.adventure);
    std::mem::swap(&mut a.inventory.dropped, &mut b.inventory.dropped);
    std::mem::swap(&mut a.inventory, &mut b.inventory);
    std::mem::swap(&mut a.expedition, &mut b.expedition);
    std::mem::swap(&mut a.progression, &mut b.progression);
    a.inventory.container = None;
    b.inventory.container = None;
    a.factory.focus = None;
    b.factory.focus = None;
    a.factory.bench = None;
    b.factory.bench = None;
}

impl Travel {
    pub fn crossing(&mut self, world: &World, player: &mut Player) -> bool {
        if player.anchor.is_some() {
            self.previous_eye = None;
            return false;
        }
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        let id = world.regions.view.session_name().to_owned();
        let old = self.previous_eye.replace((id.clone(), eye));
        let Some((old_id, previous)) = old else { return false; };
        // A teleport, including a cube-face handoff, is not swept movement.
        if old_id != id || !eye.is_finite() || previous.distance_squared(eye) > 64.0 * 64.0 {
            return false;
        }
        let Some(stop) = built::crossed(world, previous, eye) else { return false; };
        let scale = world.voxel_scale.meters();
        let eye_offset = eye - player.pos * scale;
        let feet = (stop - eye_offset) / scale;
        let (lo, hi) = super::player_aabb(feet, world.voxel_scale, player.scale, player.crouch_t);
        if super::collides_world(world, lo, hi) { return false; }
        player.pos = feet;
        player.precise_pos = feet.as_dvec3();
        self.previous_eye = Some((id, stop));
        true
    }
    pub fn ambience(&mut self, world: &mut World, player: &Player) {
        let cfg = world.sample_config();
        if !engine_worldgen::hell_gate::enabled(&cfg) { self.near_gate = false; return; }
        let feet = player.pos * world.voxel_scale.meters();
        let c = engine_worldgen::hell_gate::center(&cfg, feet);
        let near = (feet.x-c.x).abs() < 20.0 && (feet.z-c.z).abs() < 20.0;
        let pos = if near { c + glam::Vec3::Y * (engine_worldgen::hell_gate::floor(&cfg, c) + 2.0) } else { c };
        let near = near && (feet.y-pos.y).abs() < 12.0;
        if near && !self.near_gate && world.adventure.sounds.len() < 64 {
            world.adventure.sounds.push(crate::audio::Event { cue: crate::audio::Cue::Echo, pos: Some(pos), gain: 0.22, pitch: 0.55 });
        }
        self.near_gate = near;
    }
    pub fn busy(&self) -> bool { self.requested || self.destination.is_some() }
    pub fn saving_destination(&self) -> bool { self.destination.is_some() }

    pub fn request(&mut self, saves: &mut SaveService) {
        if !self.busy() { self.requested = true; saves.request(true); }
    }

    pub fn poll(&mut self, source: &mut World, player: &mut Player, day: &super::DayNightCycle, saves: &SaveService,
        prepare: impl FnOnce(&mut World, &Player, glam::Vec3) -> Result<()>) -> Result<Option<World>> {
        if let Some(mut dest) = self.destination.take() {
            dest.saves.tick(Some(&mut dest.world), player, day);
            if dest.saves.failed {
                exchange_actor(source, &mut dest.world);
                player.pos = dest.previous;
                player.precise_pos = dest.previous_precise;
                player.vel = glam::Vec3::ZERO;
                anyhow::bail!("Destination checkpoint failed; gate crossing cancelled");
            }
            if dest.saves.busy() { self.destination = Some(dest); return Ok(None); }
            if let Err(e) = dest.link.publish(&dest.world.regions.view.root, dest.file)
                .and_then(|_| dest.link.publish(&source.regions.view.root, dest.file)) {
                exchange_actor(source, &mut dest.world);
                player.pos = dest.previous;
                player.precise_pos = dest.previous_precise;
                player.vel = glam::Vec3::ZERO;
                return Err(e);
            }
            return Ok(Some(dest.world));
        }
        if !self.requested || saves.busy() { return Ok(None); }
        self.requested = false;
        ensure!(!saves.failed, "Source checkpoint failed; gate crossing cancelled");
        ensure!(source.adventure.health > 0.0, "Cannot cross while dead");
        ensure!(player.anchor.is_none(), "Return to the surface before crossing");
        let threshold = built::near(source, player);
        ensure!(threshold.is_some() || focused(source, player), "Gate no longer open or in reach");
        let file = if threshold.is_some() { BUILT_FILE } else { FILE };
        let path = source.regions.view.root.join(file);
        let (mut target, link) = if path.exists() {
            ensure!(fs::metadata(&path)?.len() <= 4096, "Gate link is oversized");
            let link: Link = ron::from_str(&fs::read_to_string(&path)?)?;
            link.validate(source)?;
            let target_id = if source.world_type == WorldType::Nether { &link.parent } else { &link.nether };
            let saved = storage::load_named(target_id)?.context("Linked realm checkpoint missing")?;
            ensure!(saved.world.regions.view.session_name() == target_id, "Linked realm identity mismatch");
            ensure!(saved.world.seed == source.seed && saved.world.generator_version == link.version(saved.world.world_type)
                && saved.world.voxel_scale == source.voxel_scale, "Linked realm configuration mismatch");
            ensure!((saved.world.world_type == WorldType::Nether) != (source.world_type == WorldType::Nether), "Invalid linked dimension");
            (saved.world, link)
        } else {
            ensure!(source.world_type != WorldType::Nether, "No parent realm linked");
            let mut target = World::with_seed(WorldType::Nether, source.voxel_scale, source.seed);
            target.generator_version = threshold.map_or(source.generator_version, |gate| gate.generator);
            if target.sample_config().hell_mirror() {
                target.planet_settings = Some(source.sample_config().planet_settings().unwrap_or_default());
            }
            target.set_rules(source.rules);
            target.adventure.respawn_point = Some(player.pos * source.voxel_scale.meters());
            let link = Link { parent: source.regions.view.session_name().into(), nether: target.regions.view.session_name().into(), seed: source.seed,
                generator: source.generator_version, nether_generator: threshold.map(|gate| gate.generator) };
            (target, link)
        };
        let previous = player.pos;
        let previous_precise = player.precise_pos;
        let scale = target.voxel_scale.meters();
        let feet = arrival(&mut target, player, threshold)? * scale;
        prepare(&mut target, player, feet / scale)?;
        player.pos = feet / scale;
        player.precise_pos = player.pos.as_dvec3();
        player.vel = glam::Vec3::ZERO;
        player.gravity_frame = glam::Quat::IDENTITY;
        player.jump_tap_timer = 0.0;
        player.step_camera_offset = 0.0;
        super::spawn::record_arrival(&mut target, feet, threshold.is_some() && !path.exists());
        exchange_actor(source, &mut target);
        let mut target_saves = SaveService::new();
        target_saves.request(true);
        self.destination = Some(Destination { world: target, link, saves: target_saves, file, previous, previous_precise });
        Ok(None)
    }
}

/// A previously published portal can be resolved without opening another copy
/// of a world the server already has resident.
pub(crate) struct Linked {
    link: Link,
    pub(crate) destination: String,
    scale: super::VoxelScalePreset,
    returning: bool,
    threshold: Option<built::Threshold>,
}

pub(crate) fn linked(source: &World, player: &Player) -> Result<Linked> {
    ensure!(player.anchor.is_none() && focused(source, player), "Gate closed or out of reach");
    let threshold = built::near(source, player);
    let path = source.regions.view.root.join(if threshold.is_some() { BUILT_FILE } else { FILE });
    let meta = fs::metadata(&path).context("Have the host open this realm first")?;
    ensure!(meta.len() <= 4096, "Gate link is oversized");
    let link: Link = ron::from_str(&fs::read_to_string(path)?)?;
    link.validate(source)?;
    let returning = source.world_type == WorldType::Nether;
    let destination = if returning { &link.parent } else { &link.nether }.clone();
    Ok(Linked { link, destination, scale: source.voxel_scale, returning, threshold })
}

impl Linked {
    #[cfg(feature = "multiplayer")]
    pub(crate) fn arrival_hint(&self, player: &Player) -> glam::Vec3 {
        arrival_feet(self.threshold, player, self.scale.meters()) / self.scale.meters()
    }

    pub(crate) fn arrival(&self, target: &mut World, player: &Player) -> Result<glam::Vec3> {
        ensure!(target.regions.view.session_name() == self.destination
            && target.seed == self.link.seed && target.generator_version == self.link.version(target.world_type)
            && target.voxel_scale == self.scale
            && (target.world_type == WorldType::Nether) != self.returning, "Linked realm configuration mismatch");
        arrival(target, player, self.threshold)
    }
}

fn arrival_feet(threshold: Option<built::Threshold>, player: &Player, scale: f32) -> glam::Vec3 {
    let feet = player.pos * scale;
    threshold.map_or(feet, |gate| gate.arrival(feet, scale))
}

fn arrival(target: &mut World, player: &Player, threshold: Option<built::Threshold>) -> Result<glam::Vec3> {
    // Check the actual edited destination, never excavate it during travel.
    // Conservative normal-body clearance; enlarged forms require more room.
    let scale = target.voxel_scale.meters();
    if let Some(gate) = threshold {
        built::receive(target, gate)?;
    }
    let feet = arrival_feet(threshold, player, scale);
    let radius = (0.45 * player.scale).max(0.45);
    let height = (2.0 * player.scale).max(2.0);
    let lo = ((feet + glam::Vec3::new(-radius, 0.05, -radius)) / scale).floor().as_ivec3();
    let hi = ((feet + glam::Vec3::new(radius, height, radius)) / scale).floor().as_ivec3();
    ensure!((hi - lo).max_element() <= 64, "Body exceeds gate clearance");
    let support = ((feet - glam::Vec3::Y * 0.1) / scale).floor().as_ivec3();
    let chunk_lo = super::chunk_coord_from_world_block(lo.min(support));
    let chunk_hi = super::chunk_coord_from_world_block(hi);
    for z in chunk_lo.z..=chunk_hi.z { for y in chunk_lo.y..=chunk_hi.y { for x in chunk_lo.x..=chunk_hi.x {
        ensure!(target.admit_edit_chunk(glam::IVec3::new(x, y, z)), "Gate destination terrain unavailable");
    } } }
    for z in lo.z..=hi.z { for y in lo.y..=hi.y { for x in lo.x..=hi.x {
        let b = target.get(glam::IVec3::new(x, y, z));
        ensure!(!b.is_solid() && !b.is_liquid(), "Gate destination obstructed");
    } } }
    let cfg = target.sample_config();
    let c = engine_worldgen::hell_gate::center(&cfg, feet);
    if threshold.is_none() {
        ensure!(intact(target, c, engine_worldgen::hell_gate::floor(&cfg, c)), "The far gate has been broken");
    }
    ensure!(target.get(support).is_solid(), "Gate destination has no footing");
    Ok(feet / scale)
}
