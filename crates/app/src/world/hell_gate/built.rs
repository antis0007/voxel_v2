//! Player-built Hell thresholds. Apertures and return links are checkpointed
//! with the world's existing portal sidecar; broken frames retain their address.
use crate::{player::Player, world::{BlockId as B, World, WorldType, raycast_block}};
use crate::world::portals::{Frame, gates::{self, Aperture}};
use anyhow::{Result, ensure};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = engine_voxel::HELL_BIOME_GENERATOR;
fn original_version() -> u32 { engine_voxel::HELL_PORTAL_GENERATOR }
pub const MAX_GATES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Threshold {
    pub local: Aperture,
    pub remote: Aperture,
    #[serde(default = "original_version")]
    pub generator: u32,
}

impl Threshold {
    pub fn validate(self, meters: f64) -> Result<()> {
        ensure!(matches!(self.generator, engine_voxel::HELL_PORTAL_GENERATOR | engine_voxel::HELL_ECOLOGY_GENERATOR | engine_voxel::HELL_RUINS_GENERATOR | engine_voxel::HELL_SURVEYED_RUINS_GENERATOR | engine_voxel::HELL_MIRROR_GENERATOR | VERSION), "Invalid Hell generator");
        for a in [self.local, self.remote] {
            ensure!(matches!(a.axis, 0 | 2) && a.max.cmpge(a.min).all(), "Invalid Hell aperture");
            ensure!(a.min[a.axis] == a.max[a.axis] && (a.max.as_i64vec3() - a.min.as_i64vec3()).max_element() <= 64,
                "Hell aperture exceeds cell limit");
            a.frame(meters, true).validate()?;
        }
        ensure!(self.local.axis == self.remote.axis && self.local.max - self.local.min == self.remote.max - self.remote.min,
            "Hell aperture sizes differ");
        Ok(())
    }
    pub fn intact(self, world: &World) -> bool { self.local.intact_with(world, B::Basalt) }
    pub fn reverse(self) -> Self { Self { local: self.remote, remote: self.local, generator: self.generator } }
    pub fn frames(self, scale: f64) -> [Frame; 2] {
        [true, false].map(|side| {
            let mut f = self.local.frame(scale, side);
            f.center += f.normal() * 0.01;
            f
        })
    }
    pub fn arrival(self, feet: Vec3, scale: f32) -> Vec3 {
        // Preserve which side of the frame the player approached. Arrive just
        // outside the film, so no movement can immediately cross back.
        let frame = self.local.frame(f64::from(scale), true);
        let side = if frame.local(feet.as_dvec3()).z >= 0.0 { 1.0 } else { -1.0 };
        let far = self.remote.frame(f64::from(scale), true);
        let mut result = far.center + far.normal() * side * 1.25;
        result.y = f64::from(self.remote.min.y) * f64::from(scale);
        result.as_vec3()
    }
}

pub fn aimed(world: &World, eye: Vec3, dir: Vec3) -> Option<Aperture> {
    if !matches!(world.world_type, WorldType::Planet | WorldType::Hills | WorldType::Nether) { return None; }
    let scale = world.voxel_scale.meters();
    let (cell, _) = raycast_block(world, eye / scale, dir, 5.0 / scale)?;
    if world.get(cell) != B::Basalt { return None; }
    let a = gates::find_with(world, cell, dir, B::Basalt)?;
    let size = (a.max - a.min + IVec3::ONE).as_vec3() * scale;
    // Bounds in metres, independent of the world's cell resolution.
    (size[a.across()] >= 2.0 && size.y >= 3.0 && size[a.across()] <= 4.0 && size.y <= 5.0).then_some(a)
}

/// Called only by a paid Spark cast. Normal wand payment, delay and animation
/// still apply. A reverse modifier cannot ignite a frame behind the caster.
pub fn ignite(world: &mut World, eye: Vec3, dir: Vec3) -> bool {
    if !matches!(world.world_type, WorldType::Planet | WorldType::Hills) { return false; }
    let Some(a) = aimed(world, eye, dir) else { return false; };
    let home = world.sample_config();
    if a.min.y <= home.min_world_y() || a.max.y >= home.max_world_y() - 1 { return false; }
    if world.portals.hell.iter().any(|g| g.local == a) { return false; }
    if world.portals.hell.len() >= MAX_GATES { return false; }
    let mut cfg = world.sample_config();
    cfg.world_type = WorldType::Nether;
    cfg.generator_version = world.portals.hell.first().map_or(VERSION, |gate| gate.generator);
    cfg.planet = cfg.hell_mirror().then(|| world.sample_config().planet_settings().unwrap_or_default());
    let scale = world.voxel_scale.meters();
    let u = a.across();
    // A broad landing above the highest generated column in the whole pad,
    // never a hole cut through an existing player structure.
    let pad = (2.0 / scale).ceil() as i32;
    let mut top = cfg.min_world_y();
    for du in -pad..=(a.max[u] - a.min[u] + pad) {
        for dn in -pad..=pad {
            let mut p = a.min;
            p[u] += du;
            p[a.axis] += dn;
            let column = engine_worldgen::sample_column(&cfg, p.x, p.z);
            top = top.max(column.height);
            if cfg.hell_ecology() {
                let clearance = if cfg.hell_ruins() { 10.0 } else { 5.0 };
                for y in (column.height + 1)..=(column.height + cfg.blocks_for_span(clearance)) {
                    p.y = y;
                    if engine_worldgen::sample_block_in_column(&cfg, p, column) != B::Air { top = top.max(y); }
                }
            }
        }
    }
    let dy = top + 2 - a.min.y;
    let remote = Aperture { min: a.min + IVec3::Y * dy, max: a.max + IVec3::Y * dy, axis: a.axis };
    if remote.min.y <= cfg.min_world_y() || remote.max.y >= cfg.max_world_y() - 1 { return false; }
    let gate = Threshold { local: a, remote, generator: cfg.generator_version };
    if gate.validate(f64::from(scale)).is_err() { return false; }
    world.portals.hell.push(gate);
    crate::world::progression::thresholds::Milestone::LitHell.record(&mut world.progression);
    world.expedition.say("Hell portal · Active");
    true
}

pub fn near(world: &World, player: &Player) -> Option<Threshold> {
    world.portals.hell.iter().copied().find(|g| {
        let f = g.local.frame(f64::from(world.voxel_scale.meters()), true);
        let eye = f.local((player.eye(world.voxel_scale) * world.voxel_scale.meters()).as_dvec3());
        eye.x.abs() < f.half_size.x && eye.y.abs() < f.half_size.y && eye.z.abs() < 2.0 && g.intact(world)
    })
}

pub fn prompt(world: &World, player: &Player, network: bool) -> Option<crate::world::interaction::Prompt> {
    if near(world, player).is_some() {
        return Some(crate::world::interaction::Prompt {
            title: if world.world_type == WorldType::Nether { "Surface portal" } else { "Hell portal" },
            action: if network { "Local play required" } else { "F  Cross" },
        });
    }
    let a = aimed(world, player.eye(world.voxel_scale) * world.voxel_scale.meters(), player.forward_dir())?;
    let active = world.portals.hell.iter().any(|g| g.local == a);
    Some(crate::world::interaction::Prompt { title: "Hell portal", action: if active { "Active" }
        else if world.world_type == WorldType::Nether { "Unlinked frame" } else { "Ignition: Spark" } })
}

pub fn append_lights(world: &World, eye_m: Vec3, lights: &mut Vec<crate::render::TorchLight>) {
    let scale = f64::from(world.voxel_scale.meters());
    let mut nearby: Vec<_> = world.portals.hell.iter().filter_map(|gate| {
        let frame = gate.local.frame(scale, true);
        let distance = frame.center.distance_squared(eye_m.as_dvec3());
        (distance < 24.0 * 24.0).then_some((distance, gate, frame))
    }).collect();
    nearby.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, gate, frame) in nearby.into_iter().take(2) {
        if !gate.intact(world) { continue; }
        if lights.len() >= crate::render::MAX_TORCH_LIGHTS { lights.pop(); }
        lights.insert(0, crate::render::TorchLight { position: frame.center.as_vec3(),
            radius: 5.0, color: [1.0, 0.18, 0.035], strength: 0.9, moving: false });
    }
}

/// Only first arrival builds a counterpart. Later damage is never repaired by
/// crossing. No pre-existing solid, fluid or edit is excavated or overwritten.
pub fn receive(world: &mut World, source: Threshold) -> Result<()> {
    let gate = source.reverse();
    if world.portals.hell.iter().any(|g| *g == gate) {
        ensure!(gate.intact(world), "Return portal frame broken or blocked");
        return Ok(());
    }
    ensure!(world.world_type == WorldType::Nether, "Return portal link missing");
    ensure!(world.portals.hell.len() < MAX_GATES, "Hell portal limit reached");
    let a = gate.local;
    let pad = (2.0 / world.voxel_scale.meters()).ceil() as i32;
    let u = a.across();
    let mut edits = Vec::new();
    let mut put = |p: IVec3| {
        edits.push((p, B::Basalt));
    };
    for du in -pad..=(a.max[u] - a.min[u] + pad) {
        for dn in -pad..=pad {
            let mut p = a.min - IVec3::Y;
            p[u] += du;
            p[a.axis] += dn;
            put(p);
        }
    }
    for p in a.ring().into_iter().filter(|p| p.y >= a.min.y) { put(p); }
    // The entire opening and both exit approaches must already be clear.
    let mut approaches = Vec::new();
    for y in a.min.y..=a.max.y { for w in a.min[u]..=a.max[u] { for d in -pad..=pad {
        let mut p = a.min; p.y = y; p[u] = w; p[a.axis] += d;
        approaches.push(p);
    } } }
    let cfg = world.sample_config();
    let mut chunks = std::collections::HashSet::new();
    for p in edits.iter().map(|&(p, _)| p).chain(approaches.iter().copied()) {
        ensure!((cfg.min_world_y()..cfg.max_world_y()).contains(&p.y), "Arrival outside realm height");
        let chunk = crate::world::chunk_coord_from_world_block(p);
        if chunks.insert(chunk) {
            ensure!(world.admit_edit_chunk(chunk), "Arrival terrain unavailable");
        }
        ensure!(world.get(p) == B::Air, "Arrival portal obstructed");
    }
    for &(p, _) in &edits {
        ensure!(engine_worldgen::sample_block(&cfg, p) == B::Air, "Arrival platform obstructed");
    }
    crate::world::effects::dry_material_batch(world, &edits,
        &mut std::collections::HashSet::new(), &mut std::collections::HashSet::new())
        .map_err(|failure| anyhow::anyhow!("Arrival platform unavailable: {failure:?}"))?;
    ensure!(gate.intact(world) && edits.iter().all(|&(p, b)| world.get(p) == b),
        "Return portal failed to form");
    world.portals.hell.push(gate);
    Ok(())
}

/// Swept eye/plane crossing; no per-frame particle entities or world reads
/// beyond the bounded nearby frames. Explicit Interact remains available.
pub fn crossed(world: &World, previous: Vec3, current: Vec3) -> Option<Vec3> {
    if !previous.is_finite() || !current.is_finite() {
        return None;
    }
    let scale = f64::from(world.voxel_scale.meters());
    let mut first = None;
    for g in &world.portals.hell {
        let f = g.local.frame(scale, true);
        let a = f.local(previous.as_dvec3());
        let b = f.local(current.as_dvec3());
        if a.z * b.z > 0.0 || (a.z - b.z).abs() < 1e-6 { continue; }
        let t = a.z / (a.z - b.z);
        if first.is_some_and(|(nearest, _)| t >= nearest) { continue; }
        let p = a.lerp(b, t);
        if p.x.abs() >= f.half_size.x - 0.25 || p.y.abs() >= f.half_size.y || !g.intact(world) { continue; }
        let local = glam::DVec3::new(p.x, p.y, if a.z >= 0.0 { 0.65 } else { -0.65 });
        first = Some((t, (f.center + f.rotation * local).as_vec3()));
    }
    first.map(|(_, stop)| stop)
}
