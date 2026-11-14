//! Growth rules for generator 137 broadleaf woodland.
use super::{Buttress, Cluster, Hit, Segment, Structure, Vine, MAX_CLUSTERS, MAX_GROUPS, MAX_SEGMENTS};
use crate::trees::foliage::Relief;
use crate::trees::rainforest::{self, Emergent};
use crate::trees::TreeForm;
use crate::{BlockId, TreeProfile, WorldSampleConfig, hash2, hash3};
use glam::{IVec3, Vec2, Vec3, Vec3Swizzles};
use std::f32::consts::TAU;

mod anatomy;
pub(super) use anatomy::uses_anatomy;

/// Angular width of the sector no primary limb enters, radians.
pub(super) const OPEN_SECTOR: f32 = 1.05;
/// Child branch radius as a fraction of its parent's radius at the station.
const PIPE: f32 = 0.58;

/// Hash stream for one tree.
#[derive(Clone, Copy)]
pub(super) struct Draw {
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

/// Proportions in metres and radians.
struct Design {
    height: f32,
    crown: f32,
    reach: f32,
    trunk_r: f32,
    flare: f32,
    fork: f32,
    limbs: u32,
    rise: (f32, f32),
    droop: f32,
    cluster: f32,
    slender: bool,
    old: bool,
}

fn design(cfg: &WorldSampleConfig, t: &TreeProfile, form: Option<Emergent>) -> Design {
    let scale = cfg.voxel_meters();
    let height = t.trunk_h as f32 * scale;
    let radius = t.canopy_radius as f32 * scale;
    let crown = t.canopy_height as f32 * scale;
    let age = crate::trees::stand_age(cfg, t);
    if t.form == TreeForm::OldOak {
        return Design {
            height, crown, reach: radius * 1.39,
            trunk_r: 1.10 + age * 0.66, flare: 0.73,
            fork: (t.branch_start_y - t.ground_y) as f32 * scale,
            limbs: 5, rise: (0.48, 0.76), droop: 0.19,
            cluster: radius * 0.34, slender: false, old: true,
        };
    }
    if t.form == TreeForm::RainforestFig {
        return Design {
            height, crown, reach: radius * 1.35,
            trunk_r: 0.72 + age * 0.58, flare: 0.66,
            fork: (t.branch_start_y - t.ground_y) as f32 * scale,
            limbs: 5, rise: (0.20, 0.46), droop: 0.13,
            cluster: radius * 0.32, slender: false, old: false,
        };
    }
    if t.form == TreeForm::Rainforest {
        let tall = rainforest::tall_canopy(cfg);
        let amazonian = rainforest::amazonian(cfg);
        let emergent = if amazonian { form.is_some() } else { height >= if tall { 34.0 } else { 26.0 } };
        let understory = height <= 11.0;
        let trunk_r = match (form, tall, emergent, understory) {
            (Some(Emergent::Ceiba), ..) => 1.80 + age * 0.85,
            (Some(Emergent::Angelim), ..) => 1.60 + age * 1.05,
            (_, true, true, _) => 1.65 + age * 1.05,
            (_, _, true, _) => 1.2 + age * 0.85,
            (_, true, false, false) => 0.44 + radius * 0.09,
            _ => 0.32 + radius * 0.08,
        };
        return Design {
            height,
            crown,
            // Even a 17 m profile plus relief and voxel padding stays within
            // the 24 m search around a tree's placement cell.
            reach: radius * if amazonian { 1.28 } else { 1.32 },
            trunk_r,
            flare: if form.is_some() { 0.55 } else if emergent { 0.85 } else { 0.42 },
            fork: (t.branch_start_y - t.ground_y) as f32 * scale,
            limbs: if emergent { 5 } else if understory { 3 } else { 4 },
            rise: match form {
                Some(Emergent::Ceiba) => (0.12, 0.32),
                Some(Emergent::Angelim) => (0.28, 0.57),
                None if tall && !understory => (0.16, 0.48),
                None => (0.25, 0.65),
            },
            droop: 0.10,
            cluster: radius * if form.is_some() { 0.37 } else { 0.28 },
            slender: false,
            old: emergent,
        };
    }
    let slender = height > radius * 3.2;
    let old = age > 0.72;
    Design {
        height,
        crown,
        reach: radius * 1.35,
        trunk_r: (0.20 + age * 0.50) * (radius / 5.0).clamp(0.60, 1.25),
        flare: 0.45 + age * 0.35,
        fork: height * if slender { 0.55 } else { 0.36 + age * 0.08 },
        limbs: if slender { 2 + (age * 1.99) as u32 } else { 3 + (age * 2.99) as u32 },
        rise: if slender { (0.90, 1.25) } else { (0.50, 0.95) },
        droop: if old { 0.22 } else { 0.14 },
        // Several masses sit on each branch tip. Size each one below the
        // branch spacing so neighbouring twig fans retain visible air gaps.
        cluster: radius * if slender { 0.30 } else { 0.24 },
        slender,
        old,
    }
}

#[derive(Clone, Copy)]
enum LeafHabit {
    Rounded,
    Fan,
    Spray,
    Draped,
}

struct Builder {
    s: Structure,
    rng: Draw,
    mass_depth: f32,
    fill_canopy: bool,
    trailing_leaves: bool,
    leaf_habit: LeafHabit,
}

impl Builder {
    fn begin(&mut self) {
        let g = &mut self.s.groups[self.s.group_count];
        g.segments = (self.s.segment_count as u8, self.s.segment_count as u8);
        g.clusters = (self.s.cluster_count as u8, self.s.cluster_count as u8);
    }

    fn end(&mut self) {
        if self.s.group_count + 1 >= MAX_GROUPS {
            return;
        }
        let g = &mut self.s.groups[self.s.group_count];
        g.segments.1 = self.s.segment_count as u8;
        g.clusters.1 = self.s.cluster_count as u8;
        self.s.group_count += 1;
    }

    fn segment(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32) {
        if self.s.segment_count < MAX_SEGMENTS {
            self.s.segments[self.s.segment_count] = Segment { a, b, ra, rb, ..Segment::default() };
            self.s.segment_count += 1;
        }
    }

    fn cluster(&mut self, c: Vec3, axis: Vec3, r: Vec3, density: f32) {
        if self.s.cluster_count < MAX_CLUSTERS {
            let side = axis.cross(Vec3::Y).normalize_or_zero();
            let up = side.cross(axis).normalize_or_zero();
            self.s.clusters[self.s.cluster_count] = Cluster { c, axis, side, up, r, density };
            self.s.cluster_count += 1;
        }
    }

    /// A leaf mass at a twig tip: the main body along the twig, a lobe
    /// pushed on and up past the tip, and a lobe hanging under it.
    fn mass(&mut self, tip: Vec3, dir: Vec3, r: f32) {
        let axis = Vec3::new(dir.x, dir.y * 0.35, dir.z).normalize_or_zero();
        let axis = if axis == Vec3::ZERO { Vec3::X } else { axis };
        let density = self.rng.range(0.60, 1.0);
        let body = Vec3::new(r * 1.20, r * 0.76 * self.mass_depth, r * 0.96);
        self.cluster(tip + axis * r * 0.25 + Vec3::Y * r * 0.18, axis, body, density);
        let side = axis.cross(Vec3::Y).normalize_or_zero();
        let spin = self.rng.range(-1.0, 1.0);
        let out = tip + axis * r * 0.75 + Vec3::Y * r * 0.45 * self.mass_depth + side * spin * r * 0.35;
        let out_r = r * self.rng.range(0.52, 0.66);
        let under = tip - Vec3::Y * r * 0.42 * self.mass_depth - side * spin * r * 0.40 + axis * r * 0.10;
        let under_r = r * self.rng.range(0.46, 0.60);
        let (out, outer_shape) = match self.leaf_habit {
            LeafHabit::Rounded => (out, Vec3::ONE),
            LeafHabit::Fan => (out + side * spin * out_r * 0.35, Vec3::new(1.17, 0.62, 1.35)),
            LeafHabit::Spray => (out + Vec3::Y * out_r * 0.38, Vec3::new(0.81, 1.52, 0.78)),
            LeafHabit::Draped => (out - Vec3::Y * out_r * 0.35, Vec3::new(0.93, 0.78, 1.21)),
        };
        self.cluster(out, axis, Vec3::new(out_r, out_r * self.mass_depth, out_r) * outer_shape, density);
        let (under, lower_shape) = match self.leaf_habit {
            LeafHabit::Fan => (under + Vec3::Y * under_r * 0.4 - side * spin * under_r * 0.6,
                Vec3::new(1.03, 0.66, 1.35)),
            LeafHabit::Spray => (under + side * spin * under_r * 0.25, Vec3::new(0.85, 1.3, 0.82)),
            LeafHabit::Draped => (under + axis * under_r * 0.56 - Vec3::Y * under_r * 0.43,
                Vec3::new(0.63, 1.92, 0.67)),
            LeafHabit::Rounded => (under, Vec3::ONE),
        };
        if self.trailing_leaves && spin.abs() > 0.20 {
            let trail = under - Vec3::Y * under_r * (0.4 + spin.abs() * 0.55);
            let radii = Vec3::new(under_r * 0.60, under_r * (1.35 + spin.abs() * 1.20), under_r * 0.62);
            self.cluster(trail, axis, radii, density * 0.84);
        } else {
            self.cluster(under, axis, Vec3::new(under_r, under_r * self.mass_depth, under_r) * lower_shape, density * 0.85);
        }
    }

    /// A tapered branch of `parts` bending down by `droop` per part. Returns
    /// each part's end point and direction.
    fn branch(&mut self, start: Vec3, azimuth: f32, elevation: f32, length: f32, r0: f32, r1: f32, parts: u32, droop: f32) -> ([(Vec3, Vec3); 4], usize) {
        let mut out = [(Vec3::ZERO, Vec3::Y); 4];
        let mut p = start;
        let mut e = elevation;
        let mut a = azimuth;
        let n = parts.clamp(1, 4) as usize;
        for k in 0..n {
            let t0 = k as f32 / n as f32;
            let t1 = (k + 1) as f32 / n as f32;
            let d = direction(a, e);
            let q = p + d * (length / n as f32);
            self.segment(p, q, r0 + (r1 - r0) * t0, r0 + (r1 - r0) * t1);
            out[k] = (q, d);
            p = q;
            e -= droop * (0.6 + 0.8 * self.rng.next());
            a += (self.rng.next() - 0.5) * 0.30;
        }
        (out, n)
    }

    /// Side branch off `parent` at `station`, with its own twigs and masses.
    fn secondary(&mut self, origin: Vec3, azimuth: f32, elevation: f32, length: f32, r: f32, d: &Design, station: f32) {
        let r1 = (r * 0.45).max(0.08);
        let (parts, n) = self.branch(origin, azimuth, elevation, length, r, r1, 2, d.droop * 1.2);
        let (end, dir) = parts[n - 1];
        let size = d.cluster * (0.80 + 0.35 * (1.0 - station));
        self.mass(end, dir, size);
        let twigs = 2;
        for j in 0..twigs {
            let side = if j == 0 { 1.0 } else { -1.0 };
            let a = azimuth + side * self.rng.range(0.45, 0.80);
            let e = elevation - self.rng.range(0.05, 0.30);
            let twig_len = length * self.rng.range(0.30, 0.42);
            let tip = end + direction(a, e) * twig_len;
            self.segment(end, tip, r1, (r1 * 0.55).max(0.07));
            let k = self.rng.range(0.70, 0.90);
            self.mass(tip, direction(a, e), size * k);
        }
    }

    /// Primary limb: three bending parts, side branches at two stations,
    /// twigs fanning from the end.
    fn limb(&mut self, start: Vec3, azimuth: f32, elevation: f32, length: f32, r: f32, d: &Design) {
        self.begin();
        let r1 = (r * 0.38).max(0.10);
        let (parts, n) = self.branch(start, azimuth, elevation, length, r, r1, 3, d.droop);
        if self.fill_canopy {
            // Foliage along the main bough connects the outer fans to the crown.
            let (at, dir) = parts[1];
            self.mass(at, dir, d.cluster * 1.05);
        }
        for (k, station) in [(0usize, 0.40f32), (1, 0.72)] {
            let (at, dir) = parts[k.min(n - 1)];
            let flip = if self.rng.next() > 0.5 { 1.0 } else { -1.0 };
            let side = if k == 0 { flip } else { -flip };
            let a = dir.z.atan2(dir.x) + side * self.rng.range(0.55, 0.95);
            let e = elevation - 0.10 - self.rng.range(0.0, 0.30);
            let len = length * self.rng.range(0.42, 0.58) * (1.0 - station * 0.25);
            let rr = (r + (r1 - r) * station) * PIPE;
            self.secondary(at, a, e, len, rr, d, station);
        }
        let (end, dir) = parts[n - 1];
        let size = d.cluster * 1.10;
        self.mass(end, dir, size);
        for j in 0..3 {
            let a = azimuth + (j as f32 - 1.0) * self.rng.range(0.40, 0.70);
            let e = elevation * 0.7 - self.rng.range(0.0, 0.35) + if j == 1 { 0.25 } else { 0.0 };
            let twig_len = length * self.rng.range(0.22, 0.34);
            let tip = end + direction(a, e) * twig_len;
            self.segment(end, tip, r1, (r1 * 0.5).max(0.07));
            let k = self.rng.range(0.55, 0.75);
            self.mass(tip, direction(a, e), size * k);
        }
        self.end();
    }
}

pub(super) fn build(cfg: &WorldSampleConfig, t: &TreeProfile) -> Structure {
    let form = rainforest::emergent_form(cfg, t);
    let ceiba = form == Some(Emergent::Ceiba);
    let fig = t.form == TreeForm::RainforestFig;
    let oak = t.form == TreeForm::OldOak;
    let rainforest = t.form.is_rainforest();
    let distinct_foliage = cfg.generator_version >= engine_voxel::DISTINCT_FOLIAGE_GENERATOR;
    let leaf_habit = if distinct_foliage {
        match t.form {
            TreeForm::RainforestFig | TreeForm::CloudCrown => LeafHabit::Draped,
            TreeForm::Aspen | TreeForm::Cypress => LeafHabit::Spray,
            TreeForm::Acacia | TreeForm::PlateauCrown => LeafHabit::Fan,
            _ if ceiba => LeafHabit::Fan,
            _ if rainforest && form.is_some() => LeafHabit::Spray,
            _ => LeafHabit::Rounded,
        }
    } else {
        LeafHabit::Rounded
    };
    let mut b = Builder {
        s: Structure::empty(),
        rng: Draw { cell: (t.cell_x, t.cell_z), seed: cfg.seed.wrapping_add(137_101), n: 0 },
        mass_depth: if ceiba { 0.62 } else if fig { 0.77 } else { 1.0 },
        fill_canopy: form.is_some() || fig || oak,
        trailing_leaves: false,
        leaf_habit,
    };
    b.s.relief = Relief::of(cfg, t);
    b.s.sag_seed = cfg.seed.wrapping_add(137_301) ^ (hash2(t.cell_x, t.cell_z, cfg.seed.wrapping_add(137_303)) * 65535.0) as u32;
    if uses_anatomy(cfg, t) {
        b.s.detailed_lobes = true;
        anatomy::grow(&mut b, cfg, t);
        finish(&mut b.s);
        return b.s;
    }
    if t.form == TreeForm::RainforestPalm {
        grow_palm(&mut b, cfg, t);
        finish(&mut b.s);
        return b.s;
    }
    if distinct_foliage && !matches!(leaf_habit, LeafHabit::Rounded) {
        b.s.detailed_lobes = true;
    }
    let d = design(cfg, t, form);
    b.s.flare = d.flare;
    b.s.flare_h = d.trunk_r * if form.is_some() { 2.55 } else { 1.8 } + 0.6;
    let rotation = b.rng.next() * TAU;
    let lean_dir = b.rng.next() * TAU;
    let sweep = if form.is_some() { 0.55 } else { 1.0 };
    let lean = Vec3::new(lean_dir.cos(), 0.0, lean_dir.sin()) * d.fork * b.rng.range(0.03, 0.14) * sweep;
    let bow = Vec3::new(-lean_dir.sin(), 0.0, lean_dir.cos()) * (b.rng.next() - 0.5) * d.fork * 0.5 * sweep;
    let base = Vec3::new(0.0, -1.0, 0.0);
    let fork = lean + Vec3::Y * d.fork;
    let mid = lean * 0.25 + bow + Vec3::Y * d.fork * 0.5;
    let fork_r = d.trunk_r * 0.78;

    b.begin();
    for k in 0..3 {
        let (t0, t1) = (k as f32 / 3.0, (k + 1) as f32 / 3.0);
        b.segment(bezier(base, mid, fork, t0), bezier(base, mid, fork, t1), d.trunk_r + (fork_r - d.trunk_r) * t0, d.trunk_r + (fork_r - d.trunk_r) * t1);
    }
    if form.is_some() {
        let roots = 4 + (b.rng.next() > 0.55) as usize;
        for (k, root) in b.s.buttresses.iter_mut().take(roots).enumerate() {
            let angle = rotation + k as f32 * TAU / roots as f32 + b.rng.range(-0.26, 0.26);
            *root = Some(Buttress {
                axis: Vec2::new(angle.cos(), angle.sin()),
                length: d.trunk_r * b.rng.range(3.0, 4.4),
                height: b.s.flare_h * b.rng.range(0.67, 1.0),
                width: d.trunk_r * b.rng.range(0.32, 0.48),
            });
        }
    } else if d.old {
        let roots = 3 + (b.rng.next() * 2.99) as u32;
        for k in 0..roots {
            let a = rotation + (k as f32 + 0.5) * TAU / roots as f32 + (b.rng.next() - 0.5) * 0.6;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            let length = d.trunk_r * b.rng.range(1.4, 2.4);
            let knee = out * d.trunk_r * 0.9 + Vec3::Y * d.trunk_r * 0.5;
            let tip = out * (d.trunk_r + length) - Vec3::Y * 0.7;
            b.segment(Vec3::Y * d.trunk_r * 1.1, knee, d.trunk_r * 0.55, d.trunk_r * 0.40);
            b.segment(knee, tip, d.trunk_r * 0.40, (d.trunk_r * 0.10).max(0.12));
        }
    }
    // The leader is one more limb, steep and short, so the crown top is
    // branch-supported too instead of one cap lobe.
    let leader_gain = if ceiba || fig || oak { 0.66 } else if rainforest { 1.0 } else if d.slender { 0.80 } else { 0.62 };
    let leader_len = (d.height - d.fork + d.crown * 0.35) * leader_gain;
    let leader_az = rotation + b.rng.range(0.0, TAU);
    let (parts, n) = b.branch(fork, leader_az, 1.30, leader_len, fork_r * 0.60, (fork_r * 0.22).max(0.10), 2, 0.12);
    for (k, station) in [(0usize, 0.45f32), (1, 0.85)] {
        let (at, _) = parts[k.min(n - 1)];
        let a = leader_az + (k as f32 * 2.1) + b.rng.range(-0.5, 0.5);
        let len = d.reach * if ceiba || oak { b.rng.range(0.30, 0.43) } else { b.rng.range(0.22, 0.34) };
        let rr = fork_r * 0.60 * PIPE;
        let e = if ceiba || oak { b.rng.range(0.30, 0.55) } else { b.rng.range(0.55, 0.95) };
        b.secondary(at, a, e, len, rr, &d, station);
    }
    let (end, dir) = parts[n - 1];
    b.mass(end, dir, d.cluster * 0.95);
    if b.fill_canopy {
        b.mass(parts[0].0, dir, d.cluster * 1.40);
    }
    b.end();

    // Primary limbs share the circle minus one open sector.
    let gap_at = rotation + b.rng.next() * TAU;
    let open_sector = if ceiba { 0.55 } else { OPEN_SECTOR };
    let span = TAU - open_sector;
    for i in 0..d.limbs {
        let azimuth = gap_at + open_sector * 0.5 + (i as f32 + 0.5) * span / d.limbs as f32 + (b.rng.next() - 0.5) * 0.35;
        let low = !d.slender && i == 0 && (d.old || rainforest);
        let start_t = if low && form.is_some() { b.rng.range(0.84, 0.92) }
            else if low { b.rng.range(0.70, 0.85) }
            else { b.rng.range(0.94, 1.0) };
        let start = bezier(base, mid, fork, start_t);
        let elevation = b.rng.range(d.rise.0, d.rise.1) - if low { 0.20 } else { 0.0 };
        let length = d.reach * if ceiba || fig || oak { b.rng.range(0.72, 1.02) }
            else if rainforest { b.rng.range(0.43, 0.78) }
            else { b.rng.range(0.52, 0.74) };
        let r = fork_r * if low { 0.48 } else { 0.56 };
        b.limb(start, azimuth, elevation, length, r, &d);
    }
    fit(&mut b.s, d.reach, d.height + d.crown, d.fork);
    if cfg.generator_version >= engine_voxel::CANOPY_UNDERSTORY_GENERATOR && rainforest && (fig || d.height >= 13.0) {
        widen(&mut b.s, 1.34);
    }
    finish(&mut b.s);
    if fig || (rainforest && d.height >= 13.0) {
        if rainforest::forest_floor(cfg) {
            // Distant crowns have no vines. Only survey the ground below a
            // liana when the cached structure is used for editable voxels.
            b.s.vine_draw = Some(b.rng);
        } else {
            hang_vines(&mut b.s, cfg, t, &mut b.rng);
        }
    }
    b.s
}

/// Spread the branch skeleton and its leaves together. Keep a margin for
/// voxel padding within the existing two-cell tree search (24 metres).
fn widen(s: &mut Structure, factor: f32) {
    let gain = (21.8 / s.reach()).clamp(1.0, factor);
    if gain <= 1.0 {
        return;
    }
    // The stretched boughs cross the lattice at shallower angles. Cover the
    // corner of a voxel so their diagonal steps share faces at coarse scales.
    s.wood_floor = 0.90;
    let horizontal = Vec3::new(gain, 1.0, gain);
    for segment in &mut s.segments[..s.segment_count] {
        segment.a *= horizontal;
        segment.b *= horizontal;
    }
    for cluster in &mut s.clusters[..s.cluster_count] {
        cluster.c *= horizontal;
        cluster.r *= horizontal;
    }
}

fn grow_palm(b: &mut Builder, cfg: &WorldSampleConfig, tree: &TreeProfile) {
    let height = cfg.world_y(tree.trunk_h);
    let radius = cfg.world_x(tree.canopy_radius);
    let crown = cfg.world_y(tree.canopy_height);
    let turn = b.rng.next() * TAU;
    let lean = direction(turn, 0.0) * height * b.rng.range(0.07, 0.16);
    let base = Vec3::NEG_Y;
    let tip = lean + Vec3::Y * height;
    let mid = Vec3::Y * (height * 0.47) - lean * 0.18;
    b.begin();
    for k in 0..4 {
        let t = k as f32 / 4.0;
        let next = t + 0.25;
        b.segment(bezier(base, mid, tip, t), bezier(base, mid, tip, next), 0.44 - 0.14 * t, 0.44 - 0.14 * next);
    }
    b.end();
    // Narrow overlapping leaf masses trace each feathered frond. The tips
    // fall below the crown shaft, leaving clear gaps between the six fronds.
    for k in 0..6 {
        let angle = turn + k as f32 * TAU / 6.0 + b.rng.range(-0.18, 0.18);
        let outward = direction(angle, 0.0);
        let length = radius * b.rng.range(0.78, 1.08);
        let droop = crown * b.rng.range(0.60, 0.86);
        b.begin();
        for t in [0.09_f32, 0.28, 0.47, 0.66, 0.85, 0.97] {
            let y = droop * t * (1.8 - 2.8 * t);
            let tangent = (outward * length + Vec3::Y * (droop * (1.8 - 5.6 * t))).normalize();
            let width = 0.21 + 0.83 * (std::f32::consts::PI * t).sin();
            b.cluster(tip + outward * (length * t) + Vec3::Y * y, tangent, Vec3::new(length * 0.16, 0.37, width), 0.95);
        }
        b.end();
    }
}

pub(super) fn hang_vines(s: &mut Structure, cfg: &WorldSampleConfig, tree: &TreeProfile, rng: &mut Draw) {
    let scale = cfg.voxel_meters();
    let clearance = cfg.blocks_for_span(3.0);
    let layered = rainforest::forest_floor(cfg);
    let (min_length, max_length) = if layered {
        (4.0, 13.0)
    } else if rainforest::tall_canopy(cfg) {
        (3.0, 9.0)
    } else {
        (1.8, 5.0)
    };
    let mut count = 0;
    let mut seek_ground = layered && rng.next() < 0.72;
    let limit = if layered { s.vines.len() } else { 4 };
    // One possible curtain per outer limb. Start on a real leaf and end in
    // air beneath it, so the strand meets the crown at every voxel scale.
    for g in &s.groups[1..s.group_count] {
        if count == limit {
            break;
        }
        if rng.next() < if layered { 0.16 } else { 0.28 } {
            continue;
        }
        let clusters = &s.clusters[g.clusters.0 as usize..g.clusters.1 as usize];
        let Some(outer) = clusters.iter().max_by(|a, b| {
            a.c.xz().length_squared().total_cmp(&b.c.xz().length_squared())
        }) else { continue };
        let leaf = (outer.c / scale).round().as_ivec3();
        if !matches!(s.sample(leaf.as_vec3() * scale, scale, 0.5), Hit::Leaf) {
            continue;
        }
        let mut top = leaf - IVec3::Y;
        while top.y > clearance && !matches!(s.sample(top.as_vec3() * scale, scale, 0.5), Hit::Air) {
            top.y -= 1;
        }
        let mut bottom_y = (top.y - cfg.blocks_for_span(rng.range(min_length, max_length)) + 1).max(clearance);
        if seek_ground {
            let floor = crate::sample_column(cfg, tree.anchor_x + top.x, tree.anchor_z + top.z).height;
            let start = (floor - tree.ground_y + 1).max(1);
            if floor > cfg.water_level()
                && floor >= tree.ground_y - cfg.blocks_for_span(1.0)
                && top.y - start >= cfg.blocks_for_span(4.0)
                && (start..top.y).all(|y| matches!(s.sample(IVec3::new(top.x, y, top.z).as_vec3() * scale, scale, 0.5), Hit::Air))
            {
                bottom_y = start;
                seek_ground = false;
            }
        }
        if bottom_y >= top.y {
            continue;
        }
        let normal = if top.x.abs() > top.z.abs() {
            IVec3::X * top.x.signum()
        } else {
            IVec3::Z * top.z.signum()
        };
        if let Some(block) = engine_content::thin::attach(BlockId::Vines, normal) {
            s.vines[count] = Some(Vine { top, bottom_y, block });
            count += 1;
        }
    }
}

/// Squeezes the crown horizontally and vertically into the given bounds.
fn fit(s: &mut Structure, reach: f32, top: f32, fork: f32) {
    let outer = s.reach() - if s.relief.is_some() { crate::trees::foliage::OUTSET } else { 0.0 };
    if outer > reach {
        let k = reach / outer;
        for seg in &mut s.segments[..s.segment_count] {
            seg.a = Vec3::new(seg.a.x * k, seg.a.y, seg.a.z * k);
            seg.b = Vec3::new(seg.b.x * k, seg.b.y, seg.b.z * k);
        }
        for l in &mut s.clusters[..s.cluster_count] {
            l.c = Vec3::new(l.c.x * k, l.c.y, l.c.z * k);
            l.r = Vec3::new(l.r.x * k, l.r.y, l.r.z * k);
        }
    }
    let high = s.top();
    if high > top && high > fork + 1.0 {
        let k = (top - fork) / (high - fork);
        let squash = |y: f32| if y > fork { fork + (y - fork) * k } else { y };
        for seg in &mut s.segments[..s.segment_count] {
            seg.a.y = squash(seg.a.y);
            seg.b.y = squash(seg.b.y);
        }
        for l in &mut s.clusters[..s.cluster_count] {
            l.c.y = squash(l.c.y);
            l.r.y *= k;
        }
    }
}

fn finish(s: &mut Structure) {
    let out = if s.relief.is_some() { crate::trees::foliage::OUTSET } else { 0.0 };
    for g in &mut s.groups[..s.group_count] {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for seg in &mut s.segments[g.segments.0 as usize..g.segments.1 as usize] {
            let r = seg.ra.max(seg.rb) + s.flare * seg.ra + 1.0;
            let min = seg.a.min(seg.b) - r;
            let max = seg.a.max(seg.b) + r;
            seg.y_min = min.y;
            seg.y_max = max.y;
            lo = lo.min(min);
            hi = hi.max(max);
        }
        // Crown bounds include overhanging leaf clusters. Wood-only bounds let
        // those voxels skip all branch distance evaluations.
        g.wood_lo = lo;
        g.wood_hi = hi;
        for l in &s.clusters[g.clusters.0 as usize..g.clusters.1 as usize] {
            let r = Vec3::splat(l.r.max_element() * super::LEAF_REACH + out + 0.5);
            lo = lo.min(l.c - r);
            hi = hi.max(l.c + r);
        }
        g.lo = lo;
        g.hi = hi;
    }
}
