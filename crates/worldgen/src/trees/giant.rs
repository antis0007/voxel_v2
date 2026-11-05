//! Generator 52 landmark trees: a few 30-44 m broadleaf giants per square
//! kilometre. A giant replaces the ordinary tree of one 12 m cell and clears
//! the ordinary trees under its crown; bushes stay as undergrowth. Like
//! ordinary trees, giants ignore dungeons, which lie below the surface.
use super::foliage;
use super::skeleton::{self, Design, Hit, Skeleton};
use super::{BlockId, IVec3, TreeForm, TreeProfile, WorldSampleConfig, hash2, landscape};
use crate::WorldType;
use engine_core::fast_hash::FastMap;
use engine_core::floor_div;
use std::cell::RefCell;
use glam::{Vec3, Vec3Swizzles};

#[cfg(test)]
mod tests;

pub(crate) const VERSION: u32 = 52;
/// Tree cells per side of one giant lottery block (120 m).
const BLOCK: i32 = 10;
const CHANCE: f32 = 0.16;
/// Leaves and wood stay inside the +-2 cell (24 m) tree search.
const REACH_LIMIT: f32 = 22.5;
const SPAWN_CLEARANCE: f32 = 150.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Habit {
    /// Low, wide limbs under a broad dome.
    Spreading,
    /// Steep limbs and a high, narrower crown.
    Tall,
    /// Strong lean, drooping limbs and heavy buttress roots.
    Ancient,
}

fn habit(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Habit {
    match hash2(cell_x, cell_z, cfg.seed.wrapping_add(52_031)) {
        v if v < 0.42 => Habit::Spreading,
        v if v < 0.74 => Habit::Tall,
        _ => Habit::Ancient,
    }
}

/// The single lottery cell of a block, kept two cells from its edges so
/// neighbouring giants never share a voxel.
fn designated(cfg: &WorldSampleConfig, bx: i32, bz: i32) -> Option<(i32, i32)> {
    if hash2(bx, bz, cfg.seed.wrapping_add(52_011)) >= CHANCE {
        return None;
    }
    let ox = 2 + ((hash2(bx, bz, cfg.seed.wrapping_add(52_013)) * 6.0) as i32).min(5);
    let oz = 2 + ((hash2(bx, bz, cfg.seed.wrapping_add(52_017)) * 6.0) as i32).min(5);
    Some((bx * BLOCK + ox, bz * BLOCK + oz))
}

pub(crate) fn candidate(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> bool {
    cfg.generator_version >= VERSION
        && cfg.world_type != WorldType::Superflat
        && designated(cfg, cell_x.div_euclid(BLOCK), cell_z.div_euclid(BLOCK)) == Some((cell_x, cell_z))
}

/// Lottery cells within `cells` tree cells of (cell_x, cell_z).
fn nearby(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32, cells: i32) -> impl Iterator<Item = (i32, i32)> + '_ {
    let bx = (cell_x - cells).div_euclid(BLOCK)..=(cell_x + cells).div_euclid(BLOCK);
    let bz = (cell_z - cells).div_euclid(BLOCK)..=(cell_z + cells).div_euclid(BLOCK);
    bz.flat_map(move |z| bx.clone().map(move |x| (x, z)))
        .filter_map(|(x, z)| designated(cfg, x, z))
        .filter(move |&(x, z)| (x - cell_x).abs() <= cells && (z - cell_z).abs() <= cells)
}

type Decisions = (Option<WorldSampleConfig>, FastMap<(i32, i32), Option<TreeProfile>>);

thread_local! {
    /// One site query costs milliseconds and the shared tree cache is cleared
    /// by horizon sweeps, so each worker keeps its own giant decisions.
    static DECISIONS: RefCell<Decisions> = RefCell::new((None, FastMap::default()));
}

/// The giant for a lottery cell, or `None` where the site cannot hold one.
/// LOD callers get the exact decision, so proxies and voxels always agree.
pub(crate) fn profile(cfg: &WorldSampleConfig, cell: (i32, i32), anchor: (i32, i32)) -> Option<TreeProfile> {
    let found = DECISIONS.with(|d| {
        let d = d.borrow();
        if d.0 == Some(*cfg) { d.1.get(&cell).copied() } else { None }
    });
    if let Some(found) = found {
        return found;
    }
    let value = evaluate(cfg, cell, anchor);
    DECISIONS.with(|d| {
        let mut d = d.borrow_mut();
        if d.0 != Some(*cfg) || d.1.len() >= 16_384 {
            *d = (Some(*cfg), FastMap::default());
        }
        d.1.insert(cell, value);
    });
    value
}

fn evaluate(cfg: &WorldSampleConfig, (cell_x, cell_z): (i32, i32), (anchor_x, anchor_z): (i32, i32)) -> Option<TreeProfile> {
    // Cheapest rejections first: an exact column costs about a millisecond.
    let scale = cfg.voxel_meters();
    let m = Vec3::new(anchor_x as f32, 0.0, anchor_z as f32) * scale;
    if m.xz().length() < SPAWN_CLEARANCE {
        return None;
    }
    let climate = landscape::sample(cfg, anchor_x, anchor_z);
    use landscape::Biome;
    if climate.temperature < 0.26
        || matches!(climate.biome, Biome::Desert | Biome::Badlands | Biome::Tundra | Biome::Alpine | Biome::Taiga
            | Biome::Ocean | Biome::Coast | Biome::Marsh | Biome::Fen)
    {
        return None;
    }
    let column_at = |x, z| crate::sample_column(cfg, x, z);
    let column = column_at(anchor_x, anchor_z);
    let ground_y = column.height;
    let soil = column.surface == BlockId::Grass
        || (matches!(column.surface, BlockId::Dirt | BlockId::Mud) && column.vegetation_density > 0.2);
    if !soil || ground_y <= cfg.water_level() + cfg.blocks_for_span(3.0) {
        return None;
    }
    // The buttress covers a little relief, not a hillside.
    let span = cfg.blocks_for_span(4.0);
    let steep = [(span, 0), (-span, 0), (0, span), (0, -span), (span, span), (-span, -span)]
        .into_iter()
        .any(|(dx, dz)| (column_at(anchor_x + dx, anchor_z + dz).height - ground_y).abs() > cfg.blocks_for_span(1.6));
    if steep {
        return None;
    }
    let age = hash2(cell_x, cell_z, cfg.seed.wrapping_add(52_023));
    let habit = habit(cfg, cell_x, cell_z);
    let (top, reach, fork) = match habit {
        Habit::Spreading => (29.0 + age * 10.0, 0.62, 0.30),
        Habit::Tall => (34.0 + age * 10.0, 0.44, 0.38),
        Habit::Ancient => (30.0 + age * 9.0, 0.56, 0.28),
    };
    let reach = (top * reach).min(REACH_LIMIT);
    if reserved(cfg, m, ground_y as f32 * scale, reach) {
        return None;
    }
    let fork_y = cfg.blocks_for_span(top * fork);
    Some(TreeProfile {
        conifer: false,
        cell_x,
        cell_z,
        anchor_x,
        anchor_z,
        ground_y,
        trunk_h: fork_y,
        canopy_radius: cfg.blocks_for_span(reach),
        canopy_height: cfg.blocks_for_span(top) - fork_y,
        branch_count: 0,
        branch_len: 0,
        branch_start_y: ground_y + fork_y,
        lean_x: 0,
        lean_z: 0,
        form: TreeForm::Giant,
    })
}

/// Settlements, the Undercrown and trails keep their ground. Checks run
/// cheapest first; a cold site query costs about 20 ms.
fn reserved(cfg: &WorldSampleConfig, m: Vec3, ground: f32, reach: f32) -> bool {
    let ring = [(0.0, 0.0), (reach, 0.0), (-reach, 0.0), (0.0, reach), (0.0, -reach)];
    if ring.iter().any(|&(dx, dz)| crate::undercrown::reserved(cfg, Vec3::new(m.x + dx, ground, m.z + dz))) {
        return true;
    }
    // Trails clear tree voxels; a trunk on one would be cut through.
    let trail = (0..9).any(|k| {
        let a = k as f32 * std::f32::consts::TAU / 8.0;
        let r = if k == 8 { 0.0 } else { 6.0 };
        crate::surface::trail_clears(cfg, Vec3::new(m.x + a.cos() * r, ground + 1.5, m.z + a.sin() * r))
    });
    trail || crate::sites::building_reserved(cfg, m.x, m.z)
}

fn design(cfg: &WorldSampleConfig, tree: &TreeProfile) -> Design {
    let scale = cfg.voxel_meters();
    let fork = tree.trunk_h as f32 * scale;
    let reach = tree.canopy_radius as f32 * scale;
    let top = fork + tree.canopy_height as f32 * scale;
    let h = |salt: u32| hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(salt));
    let trunk_r = (reach * 0.12).clamp(1.3, 2.5);
    let base = Design {
        top,
        reach,
        fork,
        trunk_r,
        flare: 0.65 + h(52_041) * 0.15,
        roots: 5 + (h(52_043) * 3.0) as u32,
        limbs: 5 + (h(52_047) * 2.0) as u32,
        splits: 3,
        lean: 0.16 + h(52_051) * 0.18,
        rise: (0.32, 0.66),
        droop: 0.30,
        lobe: 0.22,
        low_limbs: 0.35,
    };
    match habit(cfg, tree.cell_x, tree.cell_z) {
        Habit::Spreading => base,
        Habit::Tall => Design { rise: (0.62, 0.98), droop: 0.12, lobe: 0.25, low_limbs: 0.2, ..base },
        Habit::Ancient => Design {
            lean: 0.30 + h(52_053) * 0.14,
            flare: 0.80,
            roots: 7,
            rise: (0.22, 0.80),
            droop: 0.62,
            low_limbs: 0.5,
            ..base
        },
    }
}

fn build(cfg: &WorldSampleConfig, tree: &TreeProfile) -> Skeleton {
    let scale = cfg.voxel_meters();
    let d = design(cfg, tree);
    // Leaf relief may pass the fitted reach, and giants already fill the
    // 24 m tree search, so the fit gives it room inside the old bound.
    let room = if cfg.generator_version >= foliage::VERSION { foliage::OUTSET } else { 0.0 };
    skeleton::build(cfg, tree, &d, tree.canopy_radius as f32 * scale - room, d.top)
}

pub(super) fn block(cfg: &WorldSampleConfig, tree: &TreeProfile, p: IVec3) -> BlockId {
    let scale = cfg.voxel_meters();
    let q = (p - IVec3::new(tree.anchor_x, tree.ground_y, tree.anchor_z)).as_vec3() * scale;
    let reach = tree.canopy_radius as f32 * scale + scale;
    let top = (tree.trunk_h + tree.canopy_height) as f32 * scale + scale;
    if q.y <= 0.0 || q.y > top || q.xz().length_squared() > reach * reach {
        return BlockId::Air;
    }
    let edge = skeleton::edge(cfg, p, 52_119);
    skeleton::with(cfg, tree, || build(cfg, tree), |s| match s.sample(q, scale, edge) {
        Hit::Wood => BlockId::Log,
        Hit::Leaf => BlockId::Leaves,
        Hit::Air => BlockId::Air,
    })
}

/// Leaf reach and top in metres, from the same skeleton `block` samples.
pub(super) fn bounds(cfg: &WorldSampleConfig, tree: &TreeProfile) -> (f32, f32) {
    let s = build(cfg, tree);
    let scale = cfg.voxel_meters();
    (s.reach() + scale, s.top() + 0.5 * scale)
}

/// True when an ordinary tree would stand under a giant's crown.
/// Anchors are compared first, so the giant's profile is only evaluated for
/// the few trees that stand within its largest possible crown.
pub(crate) fn crowds(
    cfg: &WorldSampleConfig,
    tree: &TreeProfile,
    anchor: impl Fn(i32, i32) -> (i32, i32),
) -> bool {
    let scale = cfg.voxel_meters();
    let limit = (REACH_LIMIT + 3.5) / scale;
    nearby(cfg, tree.cell_x, tree.cell_z, 3)
        .filter(|&cell| cell != (tree.cell_x, tree.cell_z))
        .map(|(x, z)| ((x, z), anchor(x, z)))
        .filter(|&(_, (ax, az))| ((tree.anchor_x - ax) as f32).hypot((tree.anchor_z - az) as f32) < limit)
        .filter_map(|(cell, at)| profile(cfg, cell, at))
        .any(|g| {
            let d = ((tree.anchor_x - g.anchor_x) as f32).hypot((tree.anchor_z - g.anchor_z) as f32) * scale;
            d < g.canopy_radius as f32 * scale + 2.0
        })
}

/// Highest giant voxel whose crown reaches the voxel box, for residency.
pub fn top_near(cfg: &WorldSampleConfig, min: (i32, i32), max: (i32, i32)) -> Option<i32> {
    if cfg.generator_version < VERSION || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let cell = cfg.blocks_for_span(12.0);
    // Residency is conservative: a crown counts within 1.6x its leaf reach.
    let margin = cfg.blocks_for_span(REACH_LIMIT * 1.6 + 2.0);
    let (x0, z0) = (floor_div(min.0 - margin, cell), floor_div(min.1 - margin, cell));
    let (x1, z1) = (floor_div(max.0 + margin, cell), floor_div(max.1 + margin, cell));
    let (cx, cz) = ((x0 + x1).div_euclid(2), (z0 + z1).div_euclid(2));
    let cells = (x1 - x0).max(z1 - z0).div_euclid(2) + 1;
    nearby(cfg, cx, cz, cells)
        .filter_map(|(x, z)| profile(cfg, (x, z), crate::tree_anchor(cfg, x, z)))
        .filter(|g| {
            let r = (g.canopy_radius as f32 * 1.6) as i32 + cfg.blocks_for_span(1.0);
            g.anchor_x + r >= min.0 && g.anchor_x - r <= max.0 && g.anchor_z + r >= min.1 && g.anchor_z - r <= max.1
        })
        .map(|g| g.ground_y + g.trunk_h + g.canopy_height + 2)
        .max()
}
