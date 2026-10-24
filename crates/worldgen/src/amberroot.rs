//! Generator 35 Amberroot grove: one authored reference slice of the luminous
//! old-growth forest. A sheltered hollow with an open approach under a giant
//! root arch, a handful of colossal fluted trees, still pools, a warm
//! amber-gilled mushroom grove, and a hollow tree whose stair leads to a
//! glowcap chamber. Heights are metres. The plan is a pure function of the
//! world config, cached so chunk order cannot move it; probes read uncarved
//! terrain and must not re-enter (same contract as `valley`).

mod dress;
mod giant;
mod plan;
mod site;
#[cfg(test)]
mod tests;

use super::{BlockId as B, WorldSampleConfig, WorldType};
use glam::{Vec2, Vec3};
use plan::Plan;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub const VERSION: u32 = 35;
/// Fully shaped floor radius; the hollow blends back to natural ground by `OUTER_R`.
const FLOOR_R: f32 = 70.0;
const OUTER_R: f32 = 102.0;
/// Reach of grove structures, which may overhang `OUTER_R`.
const STRUCTURE_R: f32 = 126.0;

type Key = (u32, u32, u8);

thread_local! {
    static PROBING: Cell<bool> = const { Cell::new(false) };
    static LOCAL_PLAN: RefCell<Option<(Key, Option<Arc<Plan>>)>> = const { RefCell::new(None) };
    static CENTRE: Cell<Option<(Key, Option<Vec2>)>> = const { Cell::new(None) };
}
static PLANS: OnceLock<Mutex<HashMap<Key, Option<Arc<Plan>>>>> = OnceLock::new();

fn plan(cfg: &WorldSampleConfig) -> Option<Arc<Plan>> {
    if cfg.generator_version < VERSION
        || cfg.world_type == WorldType::Superflat
        || PROBING.get()
        || super::valley::probing()
    {
        return None;
    }
    // Every voxel scale shares one plan, surveyed at one metre.
    let key = (cfg.seed, cfg.generator_version, cfg.world_type as u8);
    if let Some(found) = LOCAL_PLAN.with(|c| {
        c.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, p)| p.clone())
    }) {
        return found;
    }
    let cache = PLANS.get_or_init(|| Mutex::new(HashMap::new()));
    let cached = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key).cloned();
    let found = match cached {
        Some(found) => found,
        None => {
            PROBING.set(true);
            let canon = WorldSampleConfig { voxel_scale: engine_core::VoxelScalePreset::OneMeter, ..*cfg };
            let built = site::build(&canon).map(Arc::new);
            PROBING.set(false);
            let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
            if guard.len() >= 8 {
                guard.clear();
            }
            guard.insert(key, built.clone());
            built
        }
    };
    LOCAL_PLAN.with(|c| *c.borrow_mut() = Some((key, found.clone())));
    found
}

/// Cheap reject before any plan work: is (x, z) within `reach` metres of the
/// grove centre? Every generated voxel asks, so this avoids the shared plan.
fn near(cfg: &WorldSampleConfig, x: f32, z: f32, reach: f32) -> bool {
    centre(cfg).is_some_and(|o| (Vec2::new(x, z) - o).length_squared() < reach * reach)
}

fn centre(cfg: &WorldSampleConfig) -> Option<Vec2> {
    if cfg.generator_version < VERSION
        || cfg.world_type == WorldType::Superflat
        || PROBING.get()
        || super::valley::probing()
    {
        return None;
    }
    let key = (cfg.seed, cfg.generator_version, cfg.world_type as u8);
    match CENTRE.get() {
        Some((k, centre)) if k == key => centre,
        _ => {
            let centre = plan(cfg).map(|p| p.o);
            CENTRE.set(Some((key, centre)));
            centre
        }
    }
}

/// Does a metre-space box reach into the grove? Dungeon complexes yield to it.
pub(super) fn overlaps(cfg: &WorldSampleConfig, lo: Vec3, hi: Vec3) -> bool {
    centre(cfg).is_some_and(|o| {
        let nearest = o.clamp(Vec2::new(lo.x, lo.z), Vec2::new(hi.x, hi.z));
        (nearest - o).length() < OUTER_R + 10.0
    })
}

/// Terrain height after the grove's hollow, rim berm and pools.
pub(super) fn carve(cfg: &WorldSampleConfig, wx: f32, wz: f32, h: f32) -> f32 {
    if !near(cfg, wx, wz, OUTER_R) {
        return h;
    }
    let Some(plan) = plan(cfg) else {
        return h;
    };
    let (u, v) = plan.local(wx, wz);
    let d = u.hypot(v);
    if d >= OUTER_R {
        return h;
    }
    let s = super::smoothstep(FLOOR_R, OUTER_R, d);
    let target = plan.floor_at(u, v);
    // The berm closes the hollow except along the approach.
    let berm = 3.4 * super::smooth_band(d, 85.0, 15.0) * (1.0 - plan.approach_weight(u, v));
    target + (h - target) * s + berm
}

/// Surface, filler and vegetation density for a grove column, if it is one.
pub(super) fn column(cfg: &WorldSampleConfig, wx: f32, wz: f32, ground: (B, B), veg: f32) -> Option<(B, B, f32)> {
    if !near(cfg, wx, wz, OUTER_R + 24.0) {
        return None;
    }
    let plan = plan(cfg)?;
    let (u, v) = plan.local(wx, wz);
    let d = u.hypot(v);
    if d >= OUTER_R + 24.0 {
        return None;
    }
    if d >= FLOOR_R + 10.0 {
        // A dense natural forest wall frames the hollow; the approach stays open.
        let open = plan.approach_weight(u, v);
        let veg = if plan.path_distance(u, v) < 4.0 { 0.0 } else { veg.max(1.0) * (1.0 - open) + veg * 0.15 * open };
        return Some((ground.0, ground.1, veg));
    }
    let surface = if plan.on_path(u, v) {
        B::Dirt
    } else if plan.pool_at(u, v).is_some() {
        B::Clay
    } else if plan.mossy(wx, wz) {
        B::FungalSoil
    } else {
        B::Grass
    };
    Some((surface, B::Dirt, 0.0))
}

/// Grove voxels: structures above ground, the stair and chamber below it.
pub(super) fn block(cfg: &WorldSampleConfig, p: glam::IVec3, height: i32) -> Option<B> {
    let vm = cfg.voxel_meters();
    let m = (p.as_vec3() + Vec3::splat(0.5)) * vm;
    // The far giants' crowns overhang the hollow's rim.
    if !near(cfg, m.x, m.z, STRUCTURE_R) {
        return None;
    }
    let plan = plan(cfg)?;
    let (u, v) = plan.local(m.x, m.z);
    let ground = cfg.world_y(height) + vm;
    if let Some(b) = dress::underground(&plan, m, vm) {
        return Some(b);
    }
    if p.y <= height {
        return None;
    }
    // Columns sample at the voxel corner; the pool test must agree with them.
    let (cu, cv) = plan.local(cfg.world_x(p.x), cfg.world_z(p.z));
    if let Some(level) = plan.pool_at(cu, cv)
        && cfg.world_y(p.y) < level
    {
        return Some(B::Water);
    }
    let clear = plan.keep_clear(u, v) && m.y < ground + 5.0;
    for g in &plan.giants {
        if let Some(b) = g.block(m, ground, vm, clear) {
            return (b != B::Air).then_some(b);
        }
    }
    dress::above(&plan, cfg, p, m, ground, height, clear)
}

/// Ground the grove owns; homesteads and wayside shrines stay out of it.
pub(super) fn reserves(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    near(cfg, x, z, OUTER_R + 10.0)
}

/// Place names for the HUD.
pub(super) fn location(cfg: &WorldSampleConfig, m: Vec3) -> Option<&'static str> {
    let plan = plan(cfg)?;
    let (u, v) = plan.local(m.x, m.z);
    if dress::in_chamber(&plan, m) {
        return Some("Amberroot Undercroft");
    }
    (u.hypot(v) < FLOOR_R + 6.0).then_some("Amberroot Hollow")
}

/// Review points in metres: the hollow tree's door and the undercroft floor centre.
pub fn interior_points(cfg: &WorldSampleConfig) -> Option<(Vec3, Vec3)> {
    let plan = plan(cfg)?;
    let door = plan.world(plan.door.x, plan.door.y);
    let (cc, floor) = dress::chamber(&plan)?;
    Some((Vec3::new(door.x, plan.floor_at(plan.door.x, plan.door.y), door.y), Vec3::new(cc.x, floor, cc.y)))
}

/// Grove centre (metres, floor height) and the approach direction, for review
/// cameras and surveys. `None` when this seed has no suitable site.
pub fn anchor(cfg: &WorldSampleConfig) -> Option<(Vec3, Vec2)> {
    let plan = plan(cfg)?;
    Some((Vec3::new(plan.o.x, plan.floor, plan.o.y), plan.f))
}
