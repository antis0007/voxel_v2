//! Generator 52 branching skeletons: tapered wood capsules and clumped leaf
//! lobes in metres from the tree anchor. Geometry never depends on voxel size;
//! the sampler only widens wood to stay 6-connected at coarse scales.
use super::foliage::{self, Relief};
use super::{IVec3, TreeProfile, WorldSampleConfig, distance_to_segment, hash3};
use glam::{Vec3, Vec3Swizzles};
use std::cell::RefCell;
use std::f32::consts::TAU;

const MAX_SEGMENTS: usize = 64;
/// Generator 52 crowns stop at 28 lobes; clumped crowns split each lobe.
const PLAIN_LOBES: usize = 28;
const MAX_LOBES: usize = 64;
const MAX_GROUPS: usize = 8;
/// Leaves fill `field < LEAF_CORE + edge * LEAF_EDGE`, so lobes end at sqrt(max).
const LEAF_CORE: f32 = 0.84;
const LEAF_EDGE: f32 = 0.22;
const LEAF_REACH: f32 = 1.029_563; // sqrt(1.06)
/// Wood radius floor in voxels; a capsule this thick is 6-connected in any direction.
const MIN_WOOD_VOXELS: f32 = 0.84;

#[derive(Clone, Copy, Default)]
struct Segment {
    a: Vec3,
    b: Vec3,
    ra: f32,
    rb: f32,
}

/// Ellipsoid of leaves; `under` squashes the half below the centre.
#[derive(Clone, Copy, Default)]
struct Lobe {
    c: Vec3,
    r: Vec3,
    under: f32,
}

#[derive(Clone, Copy, Default)]
struct Group {
    lo: Vec3,
    hi: Vec3,
    segments: (u8, u8),
    lobes: (u8, u8),
}

/// Silhouette inputs, all in metres or radians.
pub(super) struct Design {
    pub top: f32,
    pub reach: f32,
    pub fork: f32,
    pub trunk_r: f32,
    /// Extra trunk radius at the ground as a fraction of `trunk_r`.
    pub flare: f32,
    pub roots: u32,
    pub limbs: u32,
    pub splits: u32,
    pub lean: f32,
    /// Primary limb elevation range above horizontal.
    pub rise: (f32, f32),
    /// Secondary branch droop subtracted from the limb elevation.
    pub droop: f32,
    pub lobe: f32,
    /// Fraction of limbs that leave the trunk below the fork.
    pub low_limbs: f32,
}

#[derive(Clone, Copy)]
pub(super) struct Skeleton {
    segments: [Segment; MAX_SEGMENTS],
    segment_count: usize,
    lobes: [Lobe; MAX_LOBES],
    lobe_count: usize,
    lobe_limit: usize,
    groups: [Group; MAX_GROUPS],
    group_count: usize,
    flare: f32,
    flare_h: f32,
    relief: Option<Relief>,
    clumped: bool,
}

pub(super) enum Hit {
    Wood,
    Leaf,
    Air,
}

impl Skeleton {
    fn empty() -> Self {
        Self {
            segments: [Segment::default(); MAX_SEGMENTS],
            segment_count: 0,
            lobes: [Lobe::default(); MAX_LOBES],
            lobe_count: 0,
            lobe_limit: PLAIN_LOBES,
            groups: [Group::default(); MAX_GROUPS],
            group_count: 0,
            flare: 0.0,
            flare_h: 1.0,
            relief: None,
            clumped: false,
        }
    }

    /// Metres leaf relief may push a lobe surface past `LEAF_REACH` radii.
    fn outset(&self) -> f32 {
        if self.relief.is_some() { foliage::OUTSET } else { 0.0 }
    }

    fn begin(&mut self) {
        let g = &mut self.groups[self.group_count];
        g.segments = (self.segment_count as u8, self.segment_count as u8);
        g.lobes = (self.lobe_count as u8, self.lobe_count as u8);
    }

    fn end(&mut self) {
        let g = &mut self.groups[self.group_count];
        g.segments.1 = self.segment_count as u8;
        g.lobes.1 = self.lobe_count as u8;
        self.group_count += 1;
    }

    fn segment(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32) {
        if self.segment_count < MAX_SEGMENTS {
            self.segments[self.segment_count] = Segment { a, b, ra, rb };
            self.segment_count += 1;
        }
    }

    fn lobe(&mut self, c: Vec3, r: Vec3, under: f32) {
        if self.lobe_count < self.lobe_limit {
            self.lobes[self.lobe_count] = Lobe { c, r, under };
            self.lobe_count += 1;
        }
    }

    /// One leaf mass at `c`. Clumped crowns split it into a core clump pushed
    /// toward the branch tip and `satellites` smaller clumps with sky between
    /// them, shrunk so the cluster spans about the plain lobe's reach.
    fn leaves(&mut self, rng: &mut Draw, c: Vec3, r: Vec3, under: f32, out: Vec3, satellites: u32) {
        if !self.clumped {
            return self.lobe(c, r, under);
        }
        let r = r * 0.92;
        let out = Vec3::new(out.x, 0.0, out.z).normalize_or_zero();
        // Smaller clumps hang lower so their undersides still sheath the wood
        // that carries them; the leader lobe, with no outward push, hangs most.
        let (drop, lift) = if out == Vec3::ZERO { (0.30, (-0.70, 0.10)) } else { (0.22, (-0.45, 0.30)) };
        self.lobe(c + out * r.x * 0.22 - Vec3::Y * r.y * drop, r * 0.86, under);
        let spin = rng.next() * TAU;
        for k in 0..satellites {
            let a = spin + k as f32 * TAU / satellites as f32 + (rng.next() - 0.5) * 0.8;
            let lift = rng.range(lift.0, lift.1);
            let offset = Vec3::new(a.cos() * r.x, lift * r.y, a.sin() * r.z) * 0.50;
            self.lobe(c + offset, r * rng.range(0.58, 0.74), under);
        }
    }

    fn lobes(&self) -> &[Lobe] {
        &self.lobes[..self.lobe_count]
    }

    /// Outermost leaf, metres from the anchor axis.
    pub fn reach(&self) -> f32 {
        let out = self.outset();
        self.lobes()
            .iter()
            .map(|l| l.c.xz().length() + l.r.x.max(l.r.z) * LEAF_REACH + out)
            .fold(0.0, f32::max)
    }

    /// Highest leaf, metres above the anchor's ground voxel.
    pub fn top(&self) -> f32 {
        let out = self.outset();
        self.lobes().iter().map(|l| l.c.y + l.r.y * LEAF_REACH + out).fold(0.0, f32::max)
    }

    /// Outermost wood, so the caller's bound covers roots as well as leaves.
    fn wood_reach(&self) -> f32 {
        self.segments[..self.segment_count]
            .iter()
            .map(|s| (s.a.xz().length() + s.ra).max(s.b.xz().length() + s.rb))
            .fold(0.0, f32::max)
    }

    /// Squeezes the crown horizontally and vertically into the given bounds.
    /// Leaf relief may still reach `outset()` past the horizontal bound, so
    /// clumped crowns keep their generator 52 spread.
    fn fit(&mut self, reach: f32, top: f32, fork: f32) {
        let outer = (self.reach() - self.outset()).max(self.wood_reach());
        if outer > reach {
            let k = reach / outer;
            for s in &mut self.segments[..self.segment_count] {
                s.a = Vec3::new(s.a.x * k, s.a.y, s.a.z * k);
                s.b = Vec3::new(s.b.x * k, s.b.y, s.b.z * k);
            }
            for l in &mut self.lobes[..self.lobe_count] {
                l.c = Vec3::new(l.c.x * k, l.c.y, l.c.z * k);
                l.r = Vec3::new(l.r.x * k, l.r.y, l.r.z * k);
            }
        }
        // Vertically the outset counts: an unsquashed crown lifts the leader
        // lobe off a bare leader.
        let high = self.top();
        if high > top && high > fork + 1.0 {
            let k = (top - fork) / (high - fork);
            let squash = |y: f32| if y > fork { fork + (y - fork) * k } else { y };
            for s in &mut self.segments[..self.segment_count] {
                s.a.y = squash(s.a.y);
                s.b.y = squash(s.b.y);
            }
            for l in &mut self.lobes[..self.lobe_count] {
                l.c.y = squash(l.c.y);
                l.r.y *= k;
            }
        }
    }

    fn finish(&mut self) {
        let out = self.outset();
        for g in &mut self.groups[..self.group_count] {
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for s in &self.segments[g.segments.0 as usize..g.segments.1 as usize] {
                let r = s.ra.max(s.rb) + self.flare * s.ra + 1.0;
                lo = lo.min(s.a.min(s.b) - r);
                hi = hi.max(s.a.max(s.b) + r);
            }
            for l in &self.lobes[g.lobes.0 as usize..g.lobes.1 as usize] {
                let r = l.r * LEAF_REACH + out + 0.5;
                lo = lo.min(l.c - r);
                hi = hi.max(l.c + r);
            }
            g.lo = lo;
            g.hi = hi;
        }
    }

    /// Classifies one voxel centre `q` (metres from the anchor ground voxel).
    pub fn sample(&self, q: Vec3, scale: f32, edge: f32) -> Hit {
        let floor = scale * MIN_WOOD_VOXELS;
        let (mut field, mut size) = (f32::MAX, 1.0);
        for (index, g) in self.groups[..self.group_count].iter().enumerate() {
            if q.cmplt(g.lo).any() || q.cmpgt(g.hi).any() {
                continue;
            }
            let segments = &self.segments[g.segments.0 as usize..g.segments.1 as usize];
            for (i, s) in segments.iter().enumerate() {
                let (d, t) = distance_to_segment(q, s.a, s.b);
                let mut r = s.ra + (s.rb - s.ra) * t;
                if index == 0 && i == 0 {
                    // Root flare: buttressed base that blends into the bole.
                    let k = (1.0 - q.y / self.flare_h).max(0.0);
                    r += s.ra * self.flare * k * k;
                }
                if d <= r.max(floor) {
                    return Hit::Wood;
                }
            }
            for l in &self.lobes[g.lobes.0 as usize..g.lobes.1 as usize] {
                let d = q - l.c;
                let ry = if d.y < 0.0 { l.r.y * l.under } else { l.r.y };
                let f = (d / Vec3::new(l.r.x, ry, l.r.z)).length_squared();
                if f < field {
                    field = f;
                    size = l.r.x.min(ry).min(l.r.z);
                }
            }
        }
        let leaf = match self.relief {
            Some(relief) => relief.leaf(q, scale, field, size, foliage::CLUSTER),
            None => field < LEAF_CORE + edge * LEAF_EDGE,
        };
        if leaf { Hit::Leaf } else { Hit::Air }
    }
}

/// Hash stream for one tree: each call draws the next value in [0, 1].
struct Draw {
    cell: (i32, i32),
    seed: u32,
    n: i32,
}

impl Draw {
    fn next(&mut self) -> f32 {
        self.n += 1;
        hash3(self.cell.0, self.n, self.cell.1, self.seed)
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

fn direction(azimuth: f32, elevation: f32) -> Vec3 {
    Vec3::new(azimuth.cos() * elevation.cos(), elevation.sin(), azimuth.sin() * elevation.cos())
}

fn bezier(p0: Vec3, p1: Vec3, p2: Vec3, t: f32) -> Vec3 {
    p0.lerp(p1, t).lerp(p1.lerp(p2, t), t)
}

/// Builds the skeleton, then fits it inside `reach_limit` and `top_limit`.
/// From generator 130 its leaf surface carries resolution-gated relief.
pub(super) fn build(cfg: &WorldSampleConfig, tree: &TreeProfile, d: &Design, reach_limit: f32, top_limit: f32) -> Skeleton {
    grow(cfg, tree, d, reach_limit, top_limit, false)
}

/// `build` with each leaf mass split into clumps from generator 130.
pub(super) fn build_clumped(cfg: &WorldSampleConfig, tree: &TreeProfile, d: &Design, reach_limit: f32, top_limit: f32) -> Skeleton {
    grow(cfg, tree, d, reach_limit, top_limit, true)
}

fn grow(cfg: &WorldSampleConfig, tree: &TreeProfile, d: &Design, reach_limit: f32, top_limit: f32, clumped: bool) -> Skeleton {
    let mut rng = Draw { cell: (tree.cell_x, tree.cell_z), seed: cfg.seed.wrapping_add(52_101), n: 0 };
    let mut s = Skeleton::empty();
    s.relief = Relief::of(cfg, tree);
    if clumped && s.relief.is_some() {
        s.clumped = true;
        s.lobe_limit = MAX_LOBES;
    }
    s.flare = d.flare;
    s.flare_h = d.trunk_r * 1.8 + 0.6;
    let rotation = rng.next() * TAU;
    let lean_dir = rng.next() * TAU;
    let lean = Vec3::new(lean_dir.cos(), 0.0, lean_dir.sin()) * d.fork * d.lean.tan();
    // A sideways bow in the lower bole turns the lean into a twist.
    let bow = Vec3::new(-lean_dir.sin(), 0.0, lean_dir.cos()) * (rng.next() - 0.5) * d.fork * 0.6;
    let base = Vec3::new(0.0, -1.0, 0.0);
    let fork = lean + Vec3::Y * d.fork;
    let mid = lean * 0.25 + bow + Vec3::Y * d.fork * 0.5;
    let fork_r = d.trunk_r * 0.80;

    s.begin();
    let radius_at = |t: f32| d.trunk_r + (fork_r - d.trunk_r) * t;
    for k in 0..3 {
        let (t0, t1) = (k as f32 / 3.0, (k + 1) as f32 / 3.0);
        s.segment(bezier(base, mid, fork, t0), bezier(base, mid, fork, t1), radius_at(t0), radius_at(t1));
    }
    for k in 0..d.roots {
        let a = rotation + (k as f32 + 0.5) * TAU / d.roots as f32 + (rng.next() - 0.5) * 0.6;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let length = d.trunk_r * rng.range(1.5, 2.6);
        let knee = out * d.trunk_r * 0.9 + Vec3::Y * d.trunk_r * 0.55;
        let tip = out * (d.trunk_r + length) - Vec3::Y * 0.7;
        s.segment(Vec3::Y * d.trunk_r * 1.2, knee, d.trunk_r * 0.55, d.trunk_r * 0.42);
        s.segment(knee, tip, d.trunk_r * 0.42, (d.trunk_r * 0.10).max(0.12));
    }
    // The leader carries a crown-top lobe so the centre of the crown is not hollow.
    let leader_top = fork + Vec3::Y * ((d.top - d.fork) * 0.62);
    s.segment(fork, leader_top, fork_r * 0.55, fork_r * 0.28);
    let top_r = d.reach * d.lobe;
    s.leaves(&mut rng, leader_top + Vec3::Y * top_r * 0.15, Vec3::new(top_r, top_r * 0.80, top_r * 0.95), 0.70, Vec3::ZERO, 4);
    s.end();

    for i in 0..d.limbs {
        s.begin();
        let azimuth = rotation + i as f32 * TAU / d.limbs as f32 + (rng.next() - 0.5) * 0.7;
        let low = (i as f32 + 0.5) / d.limbs as f32 <= d.low_limbs;
        let start_t = if low { rng.range(0.62, 0.86) } else { 1.0 };
        let start = bezier(base, mid, fork, start_t);
        let elevation = rng.range(d.rise.0, d.rise.1);
        let length = d.reach * rng.range(0.44, 0.56);
        let limb_r = fork_r * if low { 0.50 } else { 0.60 };
        // An elbow sags or lifts each limb so no two read as straight spokes.
        let elbow = start + direction(azimuth, elevation) * length * 0.55
            + Vec3::Y * (rng.next() - 0.45) * length * 0.18;
        let end = start + direction(azimuth, elevation * 0.8) * length;
        s.segment(start, elbow, limb_r, limb_r * 0.82);
        s.segment(elbow, end, limb_r * 0.82, limb_r * 0.66);
        let lobe_r = d.reach * d.lobe * 0.80;
        let out = direction(azimuth, elevation);
        s.leaves(&mut rng, end + Vec3::Y * lobe_r * 0.35, Vec3::new(lobe_r, lobe_r * 0.74, lobe_r * 0.90), 0.66, out, 2);
        let spread = 0.62;
        for j in 0..d.splits {
            let side = j as f32 - (d.splits as f32 - 1.0) * 0.5;
            let a = azimuth + side * spread + (rng.next() - 0.5) * 0.35;
            let e = elevation * 0.8 - d.droop * rng.next() + side.abs() * 0.12;
            let twig_len = d.reach * rng.range(0.30, 0.42);
            let tip = end + direction(a, e) * twig_len;
            let twig_r = limb_r * 0.66 * 0.62;
            s.segment(end, tip, twig_r, (twig_r * 0.55).max(0.10));
            let r = d.reach * d.lobe * rng.range(0.85, 1.12);
            let flat = rng.range(0.66, 0.84);
            let radii = Vec3::new(r, r * flat, r * rng.range(0.82, 1.05));
            s.leaves(&mut rng, tip + Vec3::Y * r * flat * 0.25, radii, 0.62, direction(a, e), 3);
        }
        s.end();
    }
    s.fit(reach_limit, top_limit, d.fork);
    s.finish();
    s
}

type Slot = Option<((WorldSampleConfig, i32, i32, i32, i32, i32), Skeleton)>;

thread_local! {
    static CACHE: RefCell<Vec<Slot>> = RefCell::new(vec![None; 64]);
}

/// Runs `f` on the tree's skeleton, building it at most once per worker slot.
pub(super) fn with<R>(cfg: &WorldSampleConfig, tree: &TreeProfile, build: impl FnOnce() -> Skeleton, f: impl FnOnce(&Skeleton) -> R) -> R {
    let key = (*cfg, tree.cell_x, tree.cell_z, tree.trunk_h, tree.canopy_radius, tree.canopy_height);
    // A 5x5 tree neighbourhood never maps two cells onto one 8x8 slot.
    let slot = (tree.cell_x.rem_euclid(8) + tree.cell_z.rem_euclid(8) * 8) as usize;
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let hit = matches!(&cache[slot], Some((k, _)) if *k == key);
        if !hit {
            cache[slot] = Some((key, build()));
        }
        match &cache[slot] {
            Some((_, skeleton)) => f(skeleton),
            None => f(&Skeleton::empty()),
        }
    })
}

/// Edge breakup in metre-sized groups, as in generator 47.
pub(super) fn edge(cfg: &WorldSampleConfig, p: IVec3, salt: u32) -> f32 {
    let grain = cfg.blocks_for_span(1.6);
    hash3(p.x.div_euclid(grain), p.y.div_euclid(grain), p.z.div_euclid(grain), cfg.seed.wrapping_add(salt))
}
