//! Generator 14 valley. One downhill spine, a channel, a timber hamlet, and a tower.
//! Heights are metres. The plan is a pure function of the world config and is cached
//! so chunk order cannot move it. Probes sample uncarved relief and must not re-enter.

use super::{BlockId as B, WorldSampleConfig, WorldType, hash2};
use glam::Vec3;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

const STEPS: usize = 16;
const STEP_M: f32 = 70.0;
const STOREY: f32 = 2.9;

thread_local! {
    static PROBING: Cell<bool> = const { Cell::new(false) };
    static LOCAL_PLAN: std::cell::RefCell<Option<((u32, u32, u8, u8), Arc<Plan>)>> = const { std::cell::RefCell::new(None) };
}

static PLANS: std::sync::OnceLock<std::sync::Mutex<HashMap<(u32, u32, u8, u8), Arc<Plan>>>> =
    std::sync::OnceLock::new();

#[derive(Clone)]
struct Plan {
    x: Vec<f32>,
    z: Vec<f32>,
    bed: Vec<f32>,
    lake: Vec<bool>,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    hamlet: Vec3,
    hamlet_fwd: Vec3,
    tower: Vec3,
    tower_fwd: Vec3,
}

pub(super) fn carve(cfg: &WorldSampleConfig, wx: f32, wz: f32, h: f32) -> f32 {
    if cfg.generator_version < 14 || cfg.world_type == WorldType::Superflat || PROBING.get() {
        return h;
    }
    let Some(plan) = plan(cfg) else {
        return h;
    };
    if wx < plan.min_x - 48.0
        || wx > plan.max_x + 48.0
        || wz < plan.min_z - 48.0
        || wz > plan.max_z + 48.0
    {
        return h;
    }
    let Some((bed, lateral, lake)) = profile_in(&plan, wx, wz) else {
        return h;
    };
    let half = if lake { 14.0 } else { 4.5 };
    let flood = if lake { 40.0 } else { 30.0 };
    if lateral > flood {
        return h;
    }
    let bed = if lake { bed - 2.0 } else { bed };
    if lateral <= half {
        return h.min(bed);
    }
    let t = super::smoothstep(half, flood, lateral);
    let bank = bed + 1.4 + (h - bed - 1.4).max(0.0) * t;
    h.min(bank.max(bed))
}

pub(super) fn in_channel(cfg: &WorldSampleConfig, wx: f32, wz: f32) -> bool {
    let Some(plan) = plan(cfg) else {
        return false;
    };
    if wx < plan.min_x - 20.0
        || wx > plan.max_x + 20.0
        || wz < plan.min_z - 20.0
        || wz > plan.max_z + 20.0
    {
        return false;
    }
    profile_in(&plan, wx, wz)
        .is_some_and(|(_, lateral, lake)| lateral <= if lake { 14.0 } else { 4.5 })
}

pub(super) fn river_top(cfg: &WorldSampleConfig, wx: f32, wz: f32) -> Option<f32> {
    let plan = plan(cfg)?;
    if wx < plan.min_x - 20.0
        || wx > plan.max_x + 20.0
        || wz < plan.min_z - 20.0
        || wz > plan.max_z + 20.0
    {
        return None;
    }
    let (bed, lateral, lake) = profile_in(&plan, wx, wz)?;
    let half = if lake { 14.0 } else { 4.5 };
    if lateral > half {
        return None;
    }
    let bed = if lake { bed - 2.0 } else { bed };
    let depth = if lake { 4.0 } else { 1.5 };
    Some(bed + depth)
}

pub(super) fn reserves(cfg: &WorldSampleConfig, wx: f32, wz: f32) -> bool {
    if cfg.generator_version < 14 || cfg.world_type == WorldType::Superflat {
        return false;
    }
    let Some(plan) = plan(cfg) else {
        return false;
    };
    if wx < plan.min_x - 56.0
        || wx > plan.max_x + 56.0
        || wz < plan.min_z - 56.0
        || wz > plan.max_z + 56.0
    {
        return false;
    }
    built_up_in(&plan, wx, wz)
        || profile_in(&plan, wx, wz).is_some_and(|(_, lateral, _)| lateral < 48.0)
}

pub(super) fn built_up(cfg: &WorldSampleConfig, wx: f32, wz: f32) -> bool {
    let Some(plan) = plan(cfg) else {
        return false;
    };
    built_up_in(&plan, wx, wz)
}

fn built_up_in(plan: &Plan, wx: f32, wz: f32) -> bool {
    let hamlet = (wx - plan.hamlet.x).hypot(wz - plan.hamlet.z) < 48.0;
    let tower = (wx - plan.tower.x).hypot(wz - plan.tower.z) < 28.0;
    hamlet || tower
}

pub(super) fn architecture(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    if cfg.generator_version < 14 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let plan = plan(cfg)?;
    if (m.x - plan.tower.x).abs() < 28.0
        && (m.z - plan.tower.z).abs() < 28.0
        && (m.x - plan.tower.x).hypot(m.z - plan.tower.z) < 28.0
    {
        if let Some(b) = tower(&plan, cfg, m) {
            return Some(b);
        }
    }
    if (m.x - plan.hamlet.x).abs() < 52.0
        && (m.z - plan.hamlet.z).abs() < 52.0
        && (m.x - plan.hamlet.x).hypot(m.z - plan.hamlet.z) < 52.0
    {
        if let Some(b) = hamlet(&plan, cfg, m) {
            return Some(b);
        }
    }
    None
}

fn profile_in(plan: &Plan, wx: f32, wz: f32) -> Option<(f32, f32, bool)> {
    let mut best: Option<(f32, f32, bool)> = None;
    for i in 0..STEPS - 1 {
        let ax = plan.x[i];
        let az = plan.z[i];
        let bx = plan.x[i + 1];
        let bz = plan.z[i + 1];
        let dx = bx - ax;
        let dz = bz - az;
        let len2 = dx * dx + dz * dz;
        if len2 < 1.0 {
            continue;
        }
        let t = (((wx - ax) * dx + (wz - az) * dz) / len2).clamp(0.0, 1.0);
        let px = ax + dx * t;
        let pz = az + dz * t;
        let lateral = (wx - px).hypot(wz - pz);
        if best.is_some_and(|(best_lat, _, _)| lateral >= best_lat) {
            continue;
        }
        let bed = plan.bed[i] + (plan.bed[i + 1] - plan.bed[i]) * t;
        best = Some((lateral, bed, plan.lake[i]));
    }
    best.map(|(lateral, bed, lake)| (bed, lateral, lake))
}

/// Shared, not cloned: this runs for every generated voxel, and cloning the
/// plan's vectors per call was most of the valley's generation cost.
fn plan(cfg: &WorldSampleConfig) -> Option<Arc<Plan>> {
    if cfg.generator_version < 14 || cfg.world_type == WorldType::Superflat || PROBING.get() {
        return None;
    }
    let key = (
        cfg.seed,
        cfg.generator_version,
        cfg.world_type as u8,
        cfg.voxel_scale as u8,
    );
    if let Some(found) = LOCAL_PLAN.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, p)| p.clone())
    }) {
        return Some(found);
    }
    let cache = PLANS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Some(found) = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .cloned()
    {
        LOCAL_PLAN.with(|c| *c.borrow_mut() = Some((key, found.clone())));
        return Some(found);
    }
    PROBING.set(true);
    let built = Arc::new(build(cfg));
    PROBING.set(false);
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if guard.len() >= 8 {
        guard.clear();
    }
    guard.insert(key, built.clone());
    drop(guard);
    LOCAL_PLAN.with(|c| *c.borrow_mut() = Some((key, built.clone())));
    Some(built)
}

fn relief(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    // These are deliberately UNCARVED plan-construction probes. Reading or
    // writing the final-column cache here leaked provisional heights into
    // ordinary queries (and let an already-carved cached column steer a plan).
    let column = super::sample_column_uncached(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    cfg.world_y(column.height)
}

fn build(cfg: &WorldSampleConfig) -> Plan {
    let spin = hash2(1, 7, cfg.seed.wrapping_add(1400)) * std::f32::consts::TAU;
    let mut x = spin.cos() * 520.0;
    let mut z = spin.sin() * 520.0;
    let mut dir_x = spin.cos();
    let mut dir_z = spin.sin();
    let mut xs = Vec::with_capacity(STEPS);
    let mut zs = Vec::with_capacity(STEPS);
    let mut raw = Vec::with_capacity(STEPS);
    let mut lake = vec![false; STEPS - 1];
    for step in 0..STEPS {
        let h = relief(cfg, x, z);
        xs.push(x);
        zs.push(z);
        raw.push(h);
        if step + 1 == STEPS {
            break;
        }
        let mut best_h = f32::MAX;
        let mut best = (dir_x, dir_z);
        for turn in [-0.55_f32, 0.0, 0.55] {
            let (tx, tz) = rot(dir_x, dir_z, turn);
            let nh = relief(cfg, x + tx * STEP_M, z + tz * STEP_M);
            if nh < best_h {
                best_h = nh;
                best = (tx, tz);
            }
        }
        lake[step] = best_h > h + 1.5;
        dir_x = best.0;
        dir_z = best.1;
        x += dir_x * STEP_M;
        z += dir_z * STEP_M;
    }
    let mut bed = Vec::with_capacity(STEPS);
    let mut level = raw[0] - 3.0;
    for (i, h) in raw.iter().copied().enumerate() {
        level = level.min(h - 2.2) - 0.35;
        if i > 0 && lake[i - 1] {
            level -= 2.0;
        }
        bed.push(level);
    }
    let hamlet_i = 9;
    let tower_i = 2;
    let (hx, hz, hfwd) = frame_at(&xs, &zs, hamlet_i, 22.0);
    let (tx, tz, tfwd) = frame_at(&xs, &zs, tower_i, -36.0);
    let min_x = xs.iter().copied().fold(hx.min(tx), f32::min);
    let max_x = xs.iter().copied().fold(hx.max(tx), f32::max);
    let min_z = zs.iter().copied().fold(hz.min(tz), f32::min);
    let max_z = zs.iter().copied().fold(hz.max(tz), f32::max);
    Plan {
        x: xs,
        z: zs,
        bed,
        lake,
        min_x,
        max_x,
        min_z,
        max_z,
        hamlet: Vec3::new(hx, relief(cfg, hx, hz), hz),
        hamlet_fwd: hfwd,
        tower: Vec3::new(tx, relief(cfg, tx, tz), tz),
        tower_fwd: tfwd,
    }
}

fn frame_at(xs: &[f32], zs: &[f32], i: usize, side: f32) -> (f32, f32, Vec3) {
    let i = i.min(xs.len() - 2);
    let dx = xs[i + 1] - xs[i];
    let dz = zs[i + 1] - zs[i];
    let len = dx.hypot(dz).max(0.001);
    let fx = dx / len;
    let fz = dz / len;
    let sx = -fz;
    let sz = fx;
    (xs[i] + sx * side, zs[i] + sz * side, Vec3::new(fx, 0.0, fz))
}

fn rot(x: f32, z: f32, turn: f32) -> (f32, f32) {
    let (s, c) = turn.sin_cos();
    (x * c - z * s, x * s + z * c)
}

fn hamlet(plan: &Plan, cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    let right = Vec3::new(-plan.hamlet_fwd.z, 0.0, plan.hamlet_fwd.x);
    let local = m - plan.hamlet;
    let along = local.x * plan.hamlet_fwd.x + local.z * plan.hamlet_fwd.z;
    let across = local.x * right.x + local.z * right.z;
    const ROLES: [(f32, f32, u8); 6] = [
        (6.0, 5.0, 0),
        (8.0, 7.0, 1),
        (7.0, 6.0, 2),
        (6.0, 6.0, 3),
        (5.5, 6.0, 0),
        (7.0, 5.0, 0),
    ];
    for (i, (w, d, role)) in ROLES.iter().copied().enumerate() {
        let cx = -30.0 + i as f32 * 12.0;
        let lx = along - cx;
        let lz = across;
        if lx.abs() > w * 0.5 + 1.2
            || lz.abs() > d * 0.5 + 1.2
            || m.y < plan.hamlet.y - 8.0
            || m.y > plan.hamlet.y + 12.0
        {
            continue;
        }
        let floor = parcel_floor(cfg, plan.hamlet, plan.hamlet_fwd, right, cx, w, d);
        let ly = m.y - floor;
        let vox = if cfg.generator_version >= 16 {
            cfg.voxel_meters()
        } else {
            0.0
        };
        if let Some(b) = timber(role, lx, ly, lz, w, d, vox) {
            return Some(b);
        }
    }
    None
}

fn parcel_floor(
    cfg: &WorldSampleConfig,
    origin: Vec3,
    fwd: Vec3,
    right: Vec3,
    cx: f32,
    w: f32,
    d: f32,
) -> f32 {
    let mut floor = origin.y;
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let p = origin + fwd * (cx + sx * w * 0.5) + right * (sz * d * 0.5);
        floor = floor.max(relief(cfg, p.x, p.z));
    }
    floor
}

/// `v` is the voxel size for generator 16+, 0 for older worlds. Every member is at least one
/// voxel thick, otherwise coarse voxel scales sample straight through walls and roofs.
fn timber(role: u8, lx: f32, ly: f32, lz: f32, w: f32, d: f32, v: f32) -> Option<B> {
    let hw = w * 0.5;
    let hd = d * 0.5;
    if lx.abs() > hw + 0.15 || lz.abs() > hd + 0.15 {
        return None;
    }
    if (-6.0..0.0).contains(&ly) {
        return Some(B::StoneBricks);
    }
    let two = role == 1;
    let wall_top = if two { 5.8 } else { 3.2 };
    let roof = wall_top + (hw - lx.abs()).max(0.0) * 0.62;
    let shingle = 0.35f32.max(v * 0.81);
    if v > 0.0 {
        // Generator 16 chimney: climbs from the floor through the roof on the gable end.
        let cw = 1.2f32.max(v);
        let inner = hw - 0.4f32.max(v);
        if lx > inner - cw && lx < inner && lz.abs() < cw * 0.5 && ly >= 0.0 && ly < roof + 1.6 {
            return Some(if ly > roof + 0.6 {
                B::Cobblestone
            } else {
                B::StoneBricks
            });
        }
    }
    if ly > roof + shingle {
        return None;
    }
    if ly >= roof - shingle {
        return Some(B::Planks);
    }
    let et = 0.4f32.max(v);
    let inside = lx.abs() < hw - et && lz.abs() < hd - et;
    if (0.0..0.35f32.max(v)).contains(&ly) {
        let stair_hole = two && lx > hw - 2.4 && lz > hd - 2.2;
        if !stair_hole {
            return Some(if role == 2 { B::StoneBricks } else { B::Planks });
        }
    }
    if two && (2.9..2.9 + 0.35f32.max(v)).contains(&ly) && inside {
        let hole = lx > hw - 2.4 && lz > hd - 2.2;
        if !hole {
            return Some(B::Planks);
        }
    }
    if two && stair_plank(lx, ly, lz, hw, hd) {
        return Some(B::Planks);
    }
    let face = et.max(0.55);
    let door =
        lz < -hd + face && lx.abs() < 0.7f32.max(v * 0.75) && ly < 2.2f32.max(v * 2.0) && role != 2;
    let smith_front = role == 2 && lz < -hd + face && ly < 2.4;
    let window = !door
        && ly > 1.3
        && ly < 2.2f32.max(1.3 + v)
        && (lz.abs() > hd - face || lx.abs() > hw - face)
        && (lx * 0.5).fract().abs() < 0.35f32.max(v * 0.5)
        && lx.abs() > 1.2;
    let edge = lx.abs() > hw - et || lz.abs() > hd - et;
    let post = lx.abs() > hw - et.max(0.45) && lz.abs() > hd - et.max(0.45);
    if edge && ly < wall_top && !door && !smith_front && !window {
        return Some(if post || ly < 1.0 { B::Log } else { B::Planks });
    }
    if role == 2 && inside && lx > 0.8 && lx < 2.2 && lz > -0.4 && lz < 1.0 && ly < 1.2 {
        return Some(if ly < 0.8 { B::CoalOre } else { B::Smelter });
    }
    if role == 3 && inside && lz > hd - 1.3 && ly > 1.1 && ly < 1.45 && lx.abs() < hw - 1.0 {
        return Some(B::Planks);
    }
    if role == 0 && inside && lx > hw - 2.0 && lz < -hd + 2.0 && ly > 0.35 && ly < 1.3 {
        return Some(B::Workbench);
    }
    if role == 1 && inside && lz < -hd + 1.6 && lx.abs() < hw - 1.5 && ly > 0.35 && ly < 1.15 {
        return Some(B::Planks);
    }
    if v > 0.0 && !smith_front {
        // Generator 16: a partition wall with a doorway splits the larger houses.
        if inside && (role == 1 || role == 3) && lx.abs() < v.max(0.5) * 0.5 + 0.05 && ly < wall_top
        {
            let doorway = (lz + hd * 0.3).abs() < 0.9f32.max(v * 0.75) && ly < 2.3f32.max(v * 2.0);
            if !doorway && !(two && stair_plank(lx, ly, lz, hw, hd)) {
                return Some(B::Planks);
            }
        }
    }
    if inside && ly < wall_top {
        return Some(B::Air);
    }
    None
}

fn stair_plank(lx: f32, ly: f32, lz: f32, hw: f32, hd: f32) -> bool {
    if lz < hd - 2.2 || lx < -hw + 0.8 {
        return false;
    }
    let along = (lx - (-hw + 0.8)).clamp(0.0, 6.4);
    let tread = along * 0.45;
    ly < tread && ly > tread - 0.9
}

fn tower(plan: &Plan, cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    let right = Vec3::new(-plan.tower_fwd.z, 0.0, plan.tower_fwd.x);
    let rel = m - Vec3::new(plan.tower.x, 0.0, plan.tower.z);
    let lx = rel.x * right.x + rel.z * right.z;
    let lz = rel.x * plan.tower_fwd.x + rel.z * plan.tower_fwd.z;
    let floor = tower_floor(cfg, plan);
    let ly = m.y - floor;
    let gallery = gallery(lx, ly, lz);
    if gallery.is_some() {
        return gallery;
    }
    let r = lx.hypot(lz);
    if r > 8.5 || ly < -8.0 || ly > 15.0 {
        return None;
    }
    if r > 6.4 {
        return buttress(lx, ly, lz);
    }
    let vox = if cfg.generator_version >= 16 {
        cfg.voxel_meters()
    } else {
        0.0
    };
    let wall = r > 4.35 - 0.8f32.max(vox * 1.2) && r < 4.35;
    let door = lz < -3.3 && lx.abs() < 0.8 && (0.0..2.3).contains(&ly);
    let window = wall
        && ly > 1.2
        && ((ly / STOREY).fract() > 0.35)
        && ((ly / STOREY).fract() < 0.62)
        && lx.abs() > 1.4
        && !door;
    if (-6.5..0.0).contains(&ly) && r < 4.35 && !cellar_stair_clear(lx, ly, lz) {
        if r > 3.5 {
            return Some(B::StoneBricks);
        }
        if ly > -1.2 && cellar_stair(lx, ly, lz) {
            return Some(B::StoneBricks);
        }
        if ly < -5.6 {
            return Some(B::StoneBricks);
        }
        return Some(B::Air);
    }
    if ly < 0.0 {
        return None;
    }
    let storey = (ly / STOREY).floor();
    let on_floor = (ly % STOREY) < 0.34f32.max(vox) && storey >= 1.0 && storey <= 3.0 && r < 3.55;
    if on_floor && !stair_hole(storey as i32, lx, lz) {
        return Some(if storey == 2.0 {
            B::StoneBricks
        } else {
            B::Planks
        });
    }
    if let Some(tread) = flight(ly, lx, lz) {
        if ly < tread && ly > tread - 0.85f32.max(vox) {
            return Some(B::Planks);
        }
    }
    if wall && ly < 11.6 && !door && !window {
        return Some(B::StoneBricks);
    }
    if r < 3.55 && (11.6..11.6 + 0.4f32.max(vox)).contains(&ly) {
        return Some(B::Planks);
    }
    let spire = 12.0 + (4.3 - r).max(0.0) * 0.7;
    if ly >= 11.7 && ly <= spire && r < 4.5 {
        return Some(if ly > spire - 0.4 {
            B::Crystal
        } else {
            B::Planks
        });
    }
    if r < 3.5 && ly < 11.6 {
        if storey == 1.0 && lx < -2.3 && (0.6..2.4).contains(&(ly % STOREY)) {
            return Some(B::Planks);
        }
        if storey == 1.0 && lx.abs() < 0.28 && lz.abs() < 1.6 && (0.4..2.5).contains(&(ly % STOREY))
        {
            return Some(B::Planks);
        }
        if storey == 2.0 && lx > 1.4 && lx < 2.5 && lz > -0.5 && lz < 0.7 && (ly % STOREY) < 1.2 {
            return Some(B::Smelter);
        }
        if storey == 3.0 && r < 0.7 && (ly % STOREY) < 1.4 {
            return Some(B::Crystal);
        }
        return Some(B::Air);
    }
    None
}

fn tower_floor(cfg: &WorldSampleConfig, plan: &Plan) -> f32 {
    let mut floor = plan.tower.y;
    for (sx, sz) in [(-3.5, 0.0), (3.5, 0.0), (0.0, -3.5), (0.0, 3.5)] {
        floor = floor.max(relief(cfg, plan.tower.x + sx, plan.tower.z + sz));
    }
    floor
}

fn buttress(lx: f32, ly: f32, lz: f32) -> Option<B> {
    let lean = lx < -4.2 && lx > -7.2 && lz.abs() < 2.4;
    if lean && (-0.2..3.2).contains(&ly) {
        let shell = lx < -6.7 || lz.abs() > 1.9 || ly < 0.3 || ly > 2.8;
        return Some(if shell { B::Planks } else { B::Air });
    }
    let rib = lx > 4.2 && lx < 6.6 && lz.abs() < 0.7 && ly < 7.0 - (lx - 4.2) * 1.4 && ly > -0.2;
    if rib {
        return Some(B::StoneBricks);
    }
    None
}

fn flight(ly: f32, lx: f32, lz: f32) -> Option<f32> {
    let storey = (ly / STOREY).floor();
    if !(0.0..4.0).contains(&storey) {
        return None;
    }
    let going_pos = storey as i32 % 2 == 0;
    let z_ok = if going_pos {
        (1.65..3.15).contains(&lz)
    } else {
        (-3.15..-1.65).contains(&lz)
    };
    if !z_ok {
        return None;
    }
    let along = if going_pos { lx + 3.15 } else { 3.15 - lx };
    if !(0.0..=6.3).contains(&along) {
        return None;
    }
    Some(storey * STOREY + along * (STOREY / 6.3))
}

fn stair_hole(storey: i32, lx: f32, lz: f32) -> bool {
    let from_pos = (storey - 1).rem_euclid(2) == 0;
    if from_pos {
        lx > 1.4 && (1.65..3.15).contains(&lz)
    } else {
        lx < -1.4 && (-3.15..-1.65).contains(&lz)
    }
}

fn cellar_stair(lx: f32, ly: f32, lz: f32) -> bool {
    if !(-2.8..-1.3).contains(&lx) || !(-0.4..2.6).contains(&lz) {
        return false;
    }
    let tread = -0.5 - lz * 2.2;
    ly < tread && ly > tread - 0.8
}

fn cellar_stair_clear(lx: f32, ly: f32, lz: f32) -> bool {
    (-2.8..-1.3).contains(&lx)
        && (-0.2..2.8).contains(&lz)
        && ly > -6.2
        && !cellar_stair(lx, ly, lz)
}

fn gallery(lx: f32, ly: f32, lz: f32) -> Option<B> {
    let hall = (4.0..18.0).contains(&lx) && lz.abs() < 1.6 && (-6.4..-2.8).contains(&ly);
    let pocket =
        (14.5..17.5).contains(&lx) && (1.2..5.0).contains(&lz) && (-6.4..-3.2).contains(&ly);
    if !hall && !pocket {
        return None;
    }
    let shell = if pocket {
        lz > 4.5 || lx < 14.8 || lx > 17.2 || ly < -6.05
    } else {
        lz.abs() > 1.25 || ly < -6.05 || lx > 17.6
    };
    if shell {
        let ore = lx > 17.2 && lz.abs() < 0.8 && (-5.6..-4.2).contains(&ly);
        return Some(if ore { B::CopperOre } else { B::StoneBricks });
    }
    if pocket && lz > 3.6 && (-5.8..-4.6).contains(&ly) {
        return Some(B::TinOre);
    }
    Some(B::Air)
}
