//! Generator 137 broadleaf woodland: a branching skeleton whose foliage is a
//! set of clusters hung from twig tips, not a crown envelope. Primary limbs
//! leave one open sector, secondaries fork off along each limb, and every
//! cluster is an oriented mass with a protruding and a hanging lobe whose
//! underside sags by its own density. Geometry is metres from the anchor;
//! the grid only decides how much of it a voxel can show.
use super::foliage::{self, Relief};
use super::{BlockId, IVec3, TreeProfile, WorldSampleConfig, distance_to_segment, woodland};
use crate::value_noise3;
use glam::{Vec2, Vec3, Vec3Swizzles};
use std::cell::RefCell;

mod grow;
#[cfg(test)]
mod tests;

/// Original authoring gate; also released in tree-only generator 127.
pub const VERSION: u32 = 137;

pub(super) const MAX_SEGMENTS: usize = 128;
pub(super) const MAX_CLUSTERS: usize = 192;
pub(super) const MAX_GROUPS: usize = 8;
/// Wood radius floor in voxels; a capsule this thick is 6-connected.
const MIN_WOOD_VOXELS: f32 = 0.84;
/// Smallest leaf mass radius in voxels on any grid.
const MIN_LEAF_VOXELS: f32 = 0.9;
/// Leaves fill `field < 0.84 + edge * 0.22`; lobes end at sqrt(1.06).
const LEAF_REACH: f32 = 1.029_563;
/// Wavelength of the underside sag noise, metres.
const SAG_WAVELENGTH: f32 = 1.3;

#[derive(Clone, Copy, Default)]
pub(super) struct Segment {
    pub a: Vec3,
    pub b: Vec3,
    pub ra: f32,
    pub rb: f32,
    pub y_min: f32,
    pub y_max: f32,
}

/// One foliage mass: an ellipsoid stretched along `axis` (the twig that
/// carries it). `density` in 0.6..1 sets how far the underside sags.
#[derive(Clone, Copy, Default)]
pub(super) struct Cluster {
    pub c: Vec3,
    pub axis: Vec3,
    pub side: Vec3,
    pub up: Vec3,
    /// Radii along the axis, up and sideways.
    pub r: Vec3,
    pub density: f32,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Group {
    pub lo: Vec3,
    pub hi: Vec3,
    pub wood_lo: Vec3,
    pub wood_hi: Vec3,
    pub segments: (u8, u8),
    pub clusters: (u8, u8),
}

#[derive(Clone, Copy)]
struct Vine {
    top: IVec3,
    bottom_y: i32,
    block: BlockId,
}

#[derive(Clone, Copy)]
pub(super) struct Buttress {
    pub axis: Vec2,
    pub length: f32,
    pub height: f32,
    pub width: f32,
}

impl Buttress {
    fn contains(&self, q: Vec3, scale: f32) -> bool {
        let p = q.xz();
        let t = p.dot(self.axis) / self.length;
        if !(0.0..1.0).contains(&t) {
            return false;
        }
        let remaining = 1.0 - t;
        let top = self.height * remaining * (0.55 + 0.45 * remaining);
        let width = (self.width * remaining).max(scale * 0.72);
        q.y <= top && p.perp_dot(self.axis).abs() <= width
    }
}

#[derive(Clone, Copy)]
pub(super) struct Structure {
    pub segments: [Segment; MAX_SEGMENTS],
    pub segment_count: usize,
    pub clusters: [Cluster; MAX_CLUSTERS],
    pub cluster_count: usize,
    pub groups: [Group; MAX_GROUPS],
    pub group_count: usize,
    pub flare: f32,
    pub flare_h: f32,
    pub wood_floor: f32,
    pub relief: Option<Relief>,
    pub sag_seed: u32,
    pub detailed_lobes: bool,
    pub buttresses: [Option<Buttress>; 5],
    vines: [Option<Vine>; 6],
    vine_draw: Option<grow::Draw>,
}

pub(super) enum Hit {
    Wood,
    Leaf,
    Air,
}

/// One crown mass for the far proxy: an axis-aligned ellipsoid in metres
/// relative to the tree's ground voxel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lobe {
    pub centre: Vec3,
    pub radii: Vec3,
}

pub(crate) fn applies(cfg: &WorldSampleConfig, t: &TreeProfile) -> bool {
    t.form.is_rainforest() || t.form == super::TreeForm::OldOak || grow::uses_anatomy(cfg, t)
        || ((cfg.generator_version == engine_voxel::TREE_DETAIL_GENERATOR || cfg.generator_version >= VERSION)
            && woodland::applies(cfg, t))
}

impl Structure {
    pub(super) fn empty() -> Self {
        Self {
            segments: [Segment::default(); MAX_SEGMENTS],
            segment_count: 0,
            clusters: [Cluster::default(); MAX_CLUSTERS],
            cluster_count: 0,
            groups: [Group::default(); MAX_GROUPS],
            group_count: 0,
            flare: 0.0,
            flare_h: 1.0,
            wood_floor: MIN_WOOD_VOXELS,
            relief: None,
            sag_seed: 0,
            detailed_lobes: false,
            buttresses: [None; 5],
            vines: [None; 6],
            vine_draw: None,
        }
    }

    fn outset(&self) -> f32 {
        if self.relief.is_some() { foliage::OUTSET } else { 0.0 }
    }

    fn clusters(&self) -> &[Cluster] {
        &self.clusters[..self.cluster_count]
    }

    /// Outermost leaf, metres from the anchor axis.
    pub fn reach(&self) -> f32 {
        let out = self.outset();
        let leaves = self.clusters().iter().map(|l| l.c.xz().length() + l.r.max_element() * LEAF_REACH + out);
        let wood = self.segments[..self.segment_count]
            .iter()
            .map(|s| (s.a.xz().length() + s.ra).max(s.b.xz().length() + s.rb));
        let roots = self.buttresses.iter().flatten().map(|root| root.length.hypot(root.width));
        leaves.chain(wood).chain(roots).fold(0.0, f32::max)
    }

    /// Highest leaf, metres above the anchor's ground voxel.
    pub fn top(&self) -> f32 {
        let out = self.outset();
        self.clusters().iter().map(|l| l.c.y + l.r.max_element() * LEAF_REACH + out).fold(0.0, f32::max)
    }

    /// Squared normalised distance from `q` to this cluster and its smallest
    /// radius. The lower half sags by density and a slow noise, so no two
    /// undersides share one curve.
    fn field(&self, l: &Cluster, q: Vec3, scale: f32, best: f32, sag: &mut Option<f32>) -> (f32, f32) {
        let d = q - l.c;
        // Coarse grids widen small masses to a voxel, as the wood floor does,
        // so a twig's leaves never vanish into one empty cell.
        let r = l.r.max(Vec3::splat(scale * MIN_LEAF_VOXELS));
        let (a, u, s) = (d.dot(l.axis), d.dot(l.up), d.dot(l.side));
        let along = (a / r.x).powi(2);
        let across = (s / r.z).powi(2);
        let mut ry = r.y;
        if u < 0.0 {
            // Even the widest possible underside cannot beat the current
            // nearest mass. Skip its three-dimensional noise lookup.
            if along + (u / (ry * 1.16)).powi(2) + across >= best {
                return (f32::MAX, 1.0);
            }
            let n = *sag.get_or_insert_with(|| {
                value_noise3(q.x / SAG_WAVELENGTH, q.y / SAG_WAVELENGTH, q.z / SAG_WAVELENGTH, self.sag_seed)
            });
            ry *= (0.50 + 0.50 * l.density) * (0.70 + 0.45 * n);
        }
        let f = along + (u / ry).powi(2) + across;
        (f, r.x.min(ry).min(r.z))
    }

    /// Classifies one voxel centre `q` (metres from the anchor ground voxel).
    pub fn sample(&self, q: Vec3, scale: f32, edge: f32) -> Hit {
        if q.y <= self.flare_h && self.buttresses.iter().flatten().any(|root| root.contains(q, scale)) {
            return Hit::Wood;
        }
        let floor = scale * self.wood_floor;
        let (mut field, mut size) = (f32::MAX, 1.0);
        let mut sag = None;
        for (index, g) in self.groups[..self.group_count].iter().enumerate() {
            if q.cmplt(g.lo).any() || q.cmpgt(g.hi).any() {
                continue;
            }
            if q.cmpge(g.wood_lo).all() && q.cmple(g.wood_hi).all() {
                for (i, s) in self.segments[g.segments.0 as usize..g.segments.1 as usize].iter().enumerate() {
                    if q.y < s.y_min || q.y > s.y_max {
                        continue;
                    }
                    let (d, t) = distance_to_segment(q, s.a, s.b);
                    let mut r = s.ra + (s.rb - s.ra) * t;
                    if index == 0 && i == 0 {
                        let k = (1.0 - q.y / self.flare_h).max(0.0);
                        r += s.ra * self.flare * k * k;
                    }
                    if d <= r.max(floor) {
                        return Hit::Wood;
                    }
                }
            }
            for l in &self.clusters[g.clusters.0 as usize..g.clusters.1 as usize] {
                let (f, s) = self.field(l, q, scale, field, &mut sag);
                if f < field {
                    field = f;
                    size = s;
                }
            }
        }
        if field == f32::MAX {
            return Hit::Air;
        }
        let leaf = match self.relief {
            Some(relief) => relief.leaf(q, scale, field, size, foliage::CLUSTER),
            None => field < 0.84 + edge * 0.22,
        };
        if leaf { Hit::Leaf } else { Hit::Air }
    }

    /// One bounding ellipsoid per limb group, for the far proxy.
    pub fn lobes(&self) -> ([Lobe; MAX_GROUPS], usize) {
        let mut out = [Lobe::default(); MAX_GROUPS];
        let mut n = 0;
        for g in &self.groups[..self.group_count] {
            let clusters = &self.clusters[g.clusters.0 as usize..g.clusters.1 as usize];
            if clusters.is_empty() {
                continue;
            }
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for l in clusters {
                let r = if self.detailed_lobes {
                    let along = l.axis * l.r.x;
                    let up = l.side.cross(l.axis) * l.r.y;
                    let across = l.side * l.r.z;
                    let extent = along * along + up * up + across * across;
                    Vec3::new(extent.x.sqrt(), extent.y.sqrt(), extent.z.sqrt())
                } else {
                    Vec3::splat(l.r.max_element())
                };
                lo = lo.min(l.c - r);
                hi = hi.max(l.c + r);
            }
            out[n] = Lobe { centre: (lo + hi) * 0.5, radii: (hi - lo) * 0.5 };
            n += 1;
        }
        (out, n)
    }

    fn vine(&self, local: IVec3) -> BlockId {
        self.vines.iter().flatten().find(|vine| {
            local.x == vine.top.x && local.z == vine.top.z
                && (vine.bottom_y..=vine.top.y).contains(&local.y)
        }).map_or(BlockId::Air, |vine| vine.block)
    }
}

type CacheKey = (WorldSampleConfig, i32, i32, i32, i32, i32, i32, i32, super::TreeForm);
type Slot = Option<(CacheKey, Box<Structure>, (f32, f32))>;

thread_local! {
    static CACHE: RefCell<Vec<Slot>> = RefCell::new((0..64).map(|_| None).collect());
}

fn with<R>(cfg: &WorldSampleConfig, t: &TreeProfile, f: impl FnOnce(&mut Structure, (f32, f32)) -> R) -> R {
    // Long lianas are cut to the terrain below, which may differ from the
    // approximate root altitude used by a distant foliage profile.
    let ground = if super::rainforest::forest_floor(cfg) && t.form.is_rainforest() { t.ground_y } else { 0 };
    let key = (*cfg, t.cell_x, t.cell_z, ground, t.trunk_h, t.canopy_radius, t.canopy_height,
        t.branch_start_y - t.ground_y, t.form);
    let slot = (t.cell_x.rem_euclid(8) + t.cell_z.rem_euclid(8) * 8) as usize;
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = &mut cache[slot];
        if !matches!(entry, Some((k, _, _)) if *k == key) {
            let structure = Box::new(grow::build(cfg, t));
            let scale = cfg.voxel_meters();
            let bounds = (structure.reach() + scale, structure.top() + 0.5 * scale);
            *entry = Some((key, structure, bounds));
        }
        let (_, structure, bounds) = entry.as_mut().unwrap();
        f(structure, *bounds)
    })
}

/// Leaf reach and top in metres, from the same structure `block` samples.
pub(super) fn bounds(cfg: &WorldSampleConfig, t: &TreeProfile) -> (f32, f32) {
    with(cfg, t, |_, bounds| bounds)
}

/// Crown masses for the far proxy, metres relative to the ground voxel.
pub(crate) fn visual(cfg: &WorldSampleConfig, t: &TreeProfile) -> ((f32, f32), [Lobe; MAX_GROUPS], usize) {
    with(cfg, t, |s, bounds| {
        let (lobes, count) = s.lobes();
        (bounds, lobes, count)
    })
}

pub(super) fn block(cfg: &WorldSampleConfig, t: &TreeProfile, p: IVec3) -> BlockId {
    let scale = cfg.voxel_meters();
    let local = p - IVec3::new(t.anchor_x, t.ground_y, t.anchor_z);
    let q = local.as_vec3() * scale;
    if q.y <= 0.0 {
        return BlockId::Air;
    }
    with(cfg, t, |s, (reach, top)| {
        if q.y > top || q.xz().length_squared() > reach * reach {
            return BlockId::Air;
        }
        let edge = if s.relief.is_none() { super::skeleton::edge(cfg, p, 137_121) } else { 0.5 };
        match s.sample(q, scale, edge) {
            Hit::Wood => BlockId::Log,
            Hit::Leaf if t.conifer => BlockId::PineLeaves,
            Hit::Leaf => BlockId::Leaves,
            Hit::Air => {
                if let Some(mut draw) = s.vine_draw.take() {
                    grow::hang_vines(s, cfg, t, &mut draw);
                }
                s.vine(local)
            }
        }
    })
}
