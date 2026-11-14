//! Spherical crust and plate motion shared by every face.
use crate::{WorldSampleConfig, planet::noise_at};
use glam::{DQuat, DVec3};

use super::drift::{Crust, Drift, PLATE_REFERENCE_CM_PER_YEAR, PLATE_MAX_CM_PER_YEAR,
    PLATE_ACCELERATION_CM_PER_YEAR_PER_MYR};

mod assembly;
#[cfg(test)]
mod province_noise_tests;

pub(super) const RIFT_WIDTH: f64 = 0.055;

pub struct Plates {
    cfg: WorldSampleConfig,
    plates: Vec<Plate>,
    seed: u32,
    scale: f64,
    ocean: f64,
    relief: f64,
    coastal: bool,
    pub(super) shape_shelf: bool,
    faults: bool,
    traverse: bool,
    rifted_margins: bool,
    prior_spins: Vec<DVec3>,
    motion_history: Vec<(f64, Vec<DVec3>)>,
    parents: Vec<usize>,
    drift: Option<Drift>,
    assembly: Option<assembly::Assembly>,
}
#[derive(Clone, Copy)]
struct Plate { centre: DVec3, spin: DVec3, continental: f64 }

pub(super) struct Edge {
    pub owner: usize,
    pub neighbour: usize,
    pub normal: DVec3,
    pub boundary: f64,
    pub closing: f64,
    pub shear: f64,
}

impl Edge {
    fn legacy_stresses(&self) -> [f64; 3] {
        let belt = (-((self.boundary / 0.035).powi(2))).exp();
        [belt * self.closing.max(0.0), belt * (-self.closing).max(0.0), 0.0]
    }

    pub fn stresses(&self) -> [f64; 3] {
        // Half of a peak-normalized beta(3, 3) profile on either side of
        // the margin. Separate widths resolve broad folds and narrow faults.
        let belt = |width: f64| (1.0 - (self.boundary / width).powi(2)).max(0.0).powi(2);
        [
            belt(0.085) * self.closing.max(0.0),
            belt(RIFT_WIDTH) * (-self.closing).max(0.0),
            belt(0.041) * (self.shear - self.closing.abs() * 0.6).max(0.0),
        ]
    }
}

/// Seed/direction-only terms. Never cache composed terrain or moving-margin noise.
/// A value must be used only at the exact direction and seed that created it.
pub(super) struct ProvinceNoise {
    ridge: f64,
    country: f64,
    rounded: f64,
    spine: f64,
    carbonate: f64,
}

pub struct Province {
    pub height: f64,
    pub collision: f64,
    pub rift: f64,
    pub carbonate: f64,
    pub boundary_m: f64,
    pub convergence: f64,
    pub fault_damage: f64,
    pub fault_displacement_m: f64,
    pub(super) trench: f64,
}

pub fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Plates {
    pub fn new(cfg: &WorldSampleConfig) -> Self {
        let unit = |i: u32| f64::from(crate::hash_u32(cfg.seed ^ i.wrapping_mul(0x9e3779b9))) / f64::from(u32::MAX);
        let vector = |i| DVec3::new(unit(i) * 2.0 - 1.0, unit(i + 1) * 2.0 - 1.0, unit(i + 2) * 2.0 - 1.0).normalize_or_zero();
        let mut plates = Vec::new();
        // Fibonacci sites avoid pole clustering; a seed-derived rotation changes the layout.
        let rotation = glam::DQuat::from_axis_angle(vector(301), unit(305) * std::f64::consts::TAU);
        for i in 0..18u32 {
            let y = 1.0 - 2.0 * (f64::from(i) + 0.5) / 18.0;
            let angle = f64::from(i) * 2.399963229728653;
            let r = (1.0 - y * y).sqrt();
            plates.push(Plate { centre: rotation * DVec3::new(r * angle.cos(), y, r * angle.sin()),
                spin: vector(401 + i * 7) * (0.3 + unit(405 + i * 7)),
                continental: f64::from(u8::from(unit(601 + i) < 0.58)) });
        }
        let s = cfg.planet_settings();
        let traverse = cfg.generator_version == super::TECTONIC_TRAVERSE_GENERATOR
            || super::watershed_erosion(cfg.generator_version);
        Self { cfg: *cfg, plates, seed: cfg.seed,
            scale: s.map_or(1.0, |s| 2.0_f64.powf((0.5 - s.unit(1)) * 2.0)),
            ocean: s.map_or(0.0, |s| (s.unit(0) - 0.5) * 0.85),
            relief: s.map_or(420.0, |s| 80.0 + s.unit(2) * 400.0),
            coastal: cfg.generator_version >= engine_voxel::COASTAL_PLANET_GENERATOR,
            shape_shelf: true,
            faults: traverse || super::uses_fault_history(cfg.generator_version),
            traverse, rifted_margins: super::rifted_margins(cfg.generator_version),
            prior_spins: Vec::new(), motion_history: Vec::new(), parents: (0..18).collect(), drift: None,
            assembly: super::deep_time(cfg.generator_version).then(|| assembly::Assembly::new(cfg.seed)) }
    }

    pub(super) fn faults(&self) -> bool { self.faults }
    pub(super) fn rifted_margins(&self) -> bool { self.rifted_margins }
    pub(super) fn config(&self) -> WorldSampleConfig { self.cfg }
    /// Apply offshore relief after terrain and drainage have settled.
    pub(super) fn offshore_bed(&self, dir: DVec3, top: f64, sea: f64, trench: f64) -> f64 {
        let depth = sea - top;
        if !self.shape_shelf || depth <= 8.0 { return top; }
        let weight = smooth(8.0, 32.0, depth);
        let shaped = self.shelf_depth(dir, depth) + 12.0 * trench * smooth(6.0, 25.0, depth);
        (sea - (depth + (shaped - depth) * weight).max(8.0)).max(-58.0)
    }

    fn shelf_depth(&self, dir: DVec3, depth: f64) -> f64 {
        let shelf = 12.0 + 14.0 * noise_at(dir * 8.0, self.seed.wrapping_add(907));
        depth * (0.48 + 0.52 * smooth(shelf, shelf + 33.0, depth))
    }
    pub(super) fn count(&self) -> usize { self.plates.len() }

    pub(super) fn source_lineage(&self, mut owner: usize) -> u16 {
        while self.parents[owner] != owner { owner = self.parents[owner]; }
        owner as u16 + 1
    }

    pub(super) fn evolve(&mut self, progress: &crate::geo::Progress) {
        self.drift = Some(Drift::solve(self, progress));
    }

    pub(super) fn advance(&mut self, radians: f64) -> Vec<DQuat> {
        if super::resolved_history(self.cfg.generator_version) {
            self.motion_history.push((radians, self.plates.iter().map(|p| p.spin).collect()));
        }
        self.plates.iter_mut().map(|plate| {
            let rotation = DQuat::from_scaled_axis(plate.spin * radians);
            plate.centre = (rotation * plate.centre).normalize();
            rotation.conjugate()
        }).collect()
    }

    pub(super) fn reorganize(&mut self) {
        if !self.traverse { return; }
        self.prior_spins = self.plates.iter().map(|plate| plate.spin).collect();
        for (i, plate) in self.plates.iter_mut().enumerate() {
            let turn = noise_at(plate.centre * 3.0, self.seed.wrapping_add(i as u32 + 903)) - 0.5;
            plate.spin = DQuat::from_axis_angle(plate.centre, 1.3 * turn) * plate.spin;
        }
    }

    pub(super) fn previous_spin(&self, plate: usize) -> Option<DVec3> {
        self.prior_spins.get(plate).copied()
    }

    /// Forward transport through recorded Euler stages, following the parent
    /// before a daughter plate existed. Hotspots share the crust's real clock.
    pub(super) fn carried_rotation(&self, mut plate: usize, age_myr: f64) -> DQuat {
        let rate = super::drift::motion_radians_per_myr(self.cfg.generator_version);
        let mut remaining = age_myr * rate;
        let mut rotation = DQuat::IDENTITY;
        for (radians, spins) in self.motion_history.iter().rev() {
            if remaining <= 0.0 { break; }
            while plate >= spins.len() { plate = self.parents[plate]; }
            let step = remaining.min(*radians);
            rotation *= DQuat::from_scaled_axis(spins[plate] * step);
            remaining -= step;
        }
        rotation.normalize()
    }

    pub(super) fn relax_motion(&mut self, torque: &[(DVec3, f64)]) {
        self.prior_spins = self.plates.iter().map(|p| p.spin).collect();
        let resolved = super::resolved_history(self.cfg.generator_version);
        for (plate, &(force, area)) in self.plates.iter_mut().zip(torque) {
            if area <= 0.0 { continue; }
            if resolved {
                let dt = if super::fine_history(self.cfg.generator_version) { 1.0 } else { f64::from(super::drift::MOTION_CHANGE_AGE_MYR) };
                let max_turn = PLATE_ACCELERATION_CM_PER_YEAR_PER_MYR * dt
                    / PLATE_REFERENCE_CM_PER_YEAR;
                let turn = (force / area * 0.008 * (dt / f64::from(super::drift::MOTION_CHANGE_AGE_MYR))).clamp_length_max(max_turn);
                plate.spin = (plate.spin + turn).clamp_length_max(PLATE_MAX_CM_PER_YEAR / PLATE_REFERENCE_CM_PER_YEAR);
            } else {
                let turn = (force / area * 0.025).clamp_length_max(0.045);
                plate.spin = (plate.spin + turn).clamp_length(0.12, 1.6);
            }
        }
    }

    pub(super) fn history_column(&self, dir: DVec3) -> Option<(super::drift::history::Column, f64)> {
        self.drift.as_ref().and_then(|d| d.history_column(dir))
    }

    pub(super) fn former_ice(&self, dir: DVec3) -> f64 {
        self.drift.as_ref().map_or(0.0, |d| d.former_ice(dir))
    }

    pub(super) fn climate_anomaly(&self, dir: DVec3) -> f64 {
        self.drift.as_ref().map_or(0.0, |d| d.climate_anomaly(dir))
    }

    pub(super) fn history_checksum(&self, hash: u64) -> u64 {
        self.drift.as_ref().map_or(hash, |d| d.history_checksum(hash))
    }

    pub(super) fn split(&mut self, salt: u32) {
        let continental: Vec<_> = self.plates.iter().enumerate()
            .filter_map(|(i, p)| (p.continental > 0.0).then_some(i)).collect();
        let key = crate::hash_u32(self.seed ^ salt.wrapping_mul(0x9e37_79b9)) as usize;
        let i = if continental.is_empty() { key % self.plates.len() } else { continental[key % continental.len()] };
        let parent = self.plates[i];
        let axis = (parent.spin - parent.centre * parent.spin.dot(parent.centre))
            .normalize_or(parent.centre.any_orthonormal_vector());
        let daughter = |side| Plate {
            centre: DQuat::from_axis_angle(axis, side * 0.038) * parent.centre,
            spin: parent.spin + axis * (side * 0.54),
            ..parent
        };
        self.plates[i] = daughter(-1.0);
        self.plates.push(daughter(1.0));
    }

    pub(super) fn split_strained(&mut self, owner: usize, across: DVec3) {
        let parent = self.plates[owner];
        let axis = parent.centre.cross(across).normalize_or(parent.centre.any_orthonormal_vector());
        let resolved = super::resolved_history(self.cfg.generator_version);
        let daughter = |side: f64| Plate {
            centre: DQuat::from_axis_angle(axis, side * 0.038) * parent.centre,
            spin: if resolved { (parent.spin + axis * (side * 0.10)).clamp_length_max(PLATE_MAX_CM_PER_YEAR / PLATE_REFERENCE_CM_PER_YEAR) }
                else { parent.spin + axis * (side * 0.54) },
            ..parent
        };
        self.plates[owner] = daughter(-1.0);
        self.parents.push(owner);
        self.plates.push(daughter(1.0));
    }

    pub(super) fn warp(&self, dir: DVec3) -> DVec3 {
        let n = |f, salt| noise_at(dir * f, self.seed.wrapping_add(salt));
        let frequency = if self.rifted_margins { 4.0 * self.scale } else { 4.0 };
        let warp = DVec3::new(n(frequency, 801), n(frequency, 803), n(frequency, 809)) - 0.5;
        (dir + warp * 0.22).normalize()
    }

    fn nearest_warped(&self, q: DVec3) -> ((f64, usize), (f64, usize)) {
        let (mut a, mut b) = ((f64::MIN, 0), (f64::MIN, 0));
        for (i, p) in self.plates.iter().enumerate() {
            let d = q.dot(p.centre);
            if d > a.0 { b = a; a = (d, i); } else if d > b.0 { b = (d, i); }
        }
        (a, b)
    }

    fn nearest(&self, dir: DVec3) -> (DVec3, (f64, usize), (f64, usize)) {
        let q = self.warp(dir);
        let (a, b) = self.nearest_warped(q);
        (q, a, b)
    }

    pub(super) fn oceanic_motion(&self, dir: DVec3) -> Option<(usize, DVec3)> {
        let (_, (_, i), _) = self.nearest(dir);
        let plate = &self.plates[i];
        let oceanic = self.drift.as_ref().map_or(plate.continental == 0.0, |d| d.at(dir).continental < 0.48);
        oceanic.then_some((i, plate.spin))
    }

    fn blend_crust(&self, q: DVec3, nearest: f64) -> f64 {
        let mut crust = 0.0;
        let mut total = 0.0;
        for p in &self.plates {
            let w = ((q.dot(p.centre) - nearest) * 32.0).exp();
            crust += p.continental * w;
            total += w;
        }
        crust / total
    }

    pub(super) fn initial_crust(&self, dir: DVec3) -> f64 {
        let (q, a, _) = self.nearest(dir);
        let crust = self.blend_crust(q, a.0);
        if !super::deep_time(self.cfg.generator_version) { return crust; }
        // Inherited failed rifts and shelf blocks are material thickness, not
        // a coastline overlay; transport carries and collisions thicken them.
        let shelf = smooth(0.12, 0.38, crust) * (1.0 - smooth(0.72, 0.95, crust));
        let blocks = noise_at(dir * 38.0, self.seed.wrapping_add(28701));
        let fractures = (noise_at(dir * 17.0, self.seed.wrapping_add(28703)) - 0.5).abs();
        (crust + shelf * (0.27 * (blocks - 0.5) - 0.18 * (1.0 - smooth(0.02, 0.09, fractures)))).clamp(0.0, 1.0)
    }

    pub(super) fn initial_lineage(&self, dir: DVec3) -> u16 {
        let (q, a, _) = self.nearest(dir);
        self.plates.iter().enumerate().max_by(|(_, left), (_, right)| {
            let weight = |p: &Plate| p.continental * ((q.dot(p.centre) - a.0) * 32.0).exp();
            weight(left).total_cmp(&weight(right))
        }).map_or(0, |(i, _)| i as u16 + 1)
    }

    pub(super) fn crust_at(&self, dir: DVec3) -> Option<Crust> {
        self.drift.as_ref().map(|drift| drift.at(dir))
    }

    fn boundary(&self, q: DVec3, a: (f64, usize), b: (f64, usize)) -> Edge {
        let pa = &self.plates[a.1];
        let pb = &self.plates[b.1];
        let normal = (pb.centre - pa.centre).normalize();
        let relative = pa.spin.cross(q) - pb.spin.cross(q);
        let closing = relative.dot(normal);
        let boundary = (a.0 - b.0) / (pa.centre - pb.centre).length();
        let shear = relative.dot(q.cross(normal)).abs();
        Edge { owner: a.1, neighbour: b.1, normal, boundary, closing, shear }
    }

    pub(super) fn warped_edge(&self, q: DVec3) -> Edge {
        let (a, b) = self.nearest_warped(q);
        self.boundary(q, a, b)
    }

    pub fn at(&self, dir: DVec3) -> Province {
        let (q, a, b) = self.nearest(dir);
        let edge = self.boundary(q, a, b);
        match &self.drift {
            Some(drift) => self.evolved_at(dir, &edge, |d| drift.at(d)),
            None => self.province(dir, &edge, self.blend_crust(q, a.0), 0.0,
                [self.plates[a.1].continental, self.plates[b.1].continental], edge.legacy_stresses()),
        }
    }

    /// Where a point sits in a continental rift valley (generator 297), or
    /// `None` away from divergent margins and over oceanic crust.
    pub(super) fn rift_frame(&self, dir: DVec3) -> Option<super::rift_valley::Frame> {
        let (q, a, b) = self.nearest(dir);
        let edge = self.boundary(q, a, b);
        if edge.boundary >= RIFT_WIDTH || edge.closing >= 0.0 { return None; }
        let continental = self.drift.as_ref()?.at(dir).continental;
        let extension = smooth(0.06, 0.5, -edge.closing) * smooth(0.42, 0.72, continental);
        if extension <= 0.0 { return None; }
        let sign = if edge.owner < edge.neighbour { 1.0 } else { -1.0 };
        let axis = edge.normal * sign;
        let (low, high) = (edge.owner.min(edge.neighbour) as u32, edge.owner.max(edge.neighbour) as u32);
        Some(super::rift_valley::Frame {
            cross: sign * edge.boundary,
            strike: (dir - axis * dir.dot(axis)).normalize_or_zero(),
            key: self.seed ^ low.wrapping_mul(0x9e37_79b9) ^ high.wrapping_mul(0x85eb_ca6b) ^ 0x297,
            extension, relief: self.relief,
        })
    }

    pub(super) fn vent_tectonics(&self, dir: DVec3) -> crate::volcano::Tectonics {
        use crate::volcano::Tectonics;
        let (q, a, b) = self.nearest(dir);
        let edge = self.boundary(q, a, b);
        let boundary_m = edge.boundary * engine_core::cube_sphere::atlas::RADIUS_M;
        let mut vent = Tectonics { boundary_m, convergence: edge.closing };
        if self.rifted_margins && edge.closing > 0.0 && let Some(drift) = &self.drift {
            if boundary_m > 4_000.0 {
                vent.convergence = 0.0;
                return vent;
            }
            let reach = 0.065;
            let own = drift.at((dir - edge.normal * (reach - edge.boundary)).normalize());
            let other = drift.at((dir + edge.normal * (reach + edge.boundary)).normalize());
            // Thick continental crust is buoyant. Among oceanic slabs the
            // older, colder slab is more likely to descend.
            let buoyancy = |c: Crust| c.continental - (1.0 - c.continental) * 0.35 * (c.ocean_age_myr / 160.0).sqrt();
            let polarity = smooth(-0.12, 0.12, buoyancy(own) - buoyancy(other));
            vent.convergence *= polarity * (1.0 - smooth(0.42, 0.75, other.continental));
            // The volcanic front follows the overriding slab inland from the trench.
            vent.boundary_m = (boundary_m - 1_200.0).abs();
        }
        vent
    }

    pub(super) fn province_noise(&self, dir: DVec3) -> ProvinceNoise {
        let n = |f, salt| noise_at(dir * f, self.seed.wrapping_add(salt));
        let body = dir * engine_core::cube_sphere::atlas::RADIUS_M;
        ProvinceNoise {
            ridge: 1.0 - (n(37.0, 827) * 2.0 - 1.0).abs(),
            country: smooth(0.35, 0.66, n(18.0, 839)),
            rounded: noise_at(body / 1450.0, self.seed.wrapping_add(853)) * 2.0 - 1.0,
            spine: 1.0 - (noise_at(body / 850.0, self.seed.wrapping_add(857)) * 2.0 - 1.0).abs(),
            carbonate: smooth(0.45, 0.68, n(9.0, 829)),
        }
    }

    pub(super) fn evolved_at(&self, dir: DVec3, edge: &Edge, sample: impl Fn(DVec3) -> Crust) -> Province {
        self.evolved_at_impl(dir, edge, sample, None)
    }

    pub(super) fn evolved_at_with_noise(&self, dir: DVec3, edge: &Edge, noise: &ProvinceNoise,
        sample: impl Fn(DVec3) -> Crust) -> Province
    {
        self.evolved_at_impl(dir, edge, sample, Some(noise))
    }

    fn evolved_at_impl(&self, dir: DVec3, edge: &Edge, sample: impl Fn(DVec3) -> Crust,
        cached: Option<&ProvinceNoise>) -> Province
    {
        let crust = sample(dir);
        let (own, other) = if edge.boundary < 0.095 && edge.closing > 0.0 {
            // Measure crust on both sides of the moving margin, outside the trench.
            let reach = 0.065;
            (sample((dir - edge.normal * (reach - edge.boundary)).normalize()),
                sample((dir + edge.normal * (reach + edge.boundary)).normalize()))
        } else {
            (crust, crust)
        };
        let mut province = self.province_with_noise(dir, edge, crust.continental, crust.relief_m,
            [own.continental, other.continental], edge.stresses(), cached);
        if super::coupled_history(self.cfg.generator_version) {
            province.carbonate = crust.carbonate;
        }
        if self.faults && edge.boundary < 0.095 {
            let (displacement, damage) = self.fault(dir, edge, own, other);
            province.height = (province.height + displacement).max(-58.0);
            province.fault_damage = damage;
            province.fault_displacement_m = displacement;
        }
        if self.traverse && edge.boundary < 0.095 && edge.closing > 0.0 {
            province.height += self.fold_belt(dir, edge, own.continental.min(other.continental));
        }
        if super::geological_history(self.cfg.generator_version)
            || self.cfg.generator_version == super::LAYERED_CANYON_GENERATOR {
            // The advected basement already includes past slip. Keep its
            // offset here so the rock contacts move with that same crust.
            province.fault_displacement_m += crust.slip_m;
            province.fault_damage = province.fault_damage.max(0.7 * smooth(0.6, 14.0, crust.slip_m.abs()));
        }
        province
    }

    /// Surface displacement of a group of ruptures within one geological step.
    pub(super) fn fault_episode(&self, dir: DVec3, edge: &Edge, myr: u16) -> f64 {
        if edge.boundary >= 0.042 { return 0.0; }
        let sign = if edge.owner < edge.neighbour { 1.0 } else { -1.0 };
        let axis = edge.normal * sign;
        let strike = (dir - axis * dir.dot(axis)).normalize_or_zero();
        let key = self.seed ^ (edge.owner.min(edge.neighbour) as u32).wrapping_mul(0x9e37_79b9)
            ^ (edge.owner.max(edge.neighbour) as u32).wrapping_mul(0x85eb_ca6b);
        let segment = noise_at(strike * 36.0, key);
        let phase = noise_at(strike * 18.0, key.wrapping_add(u32::from(myr) * 239));
        let release = smooth(0.52, 0.80, 0.65 * phase + 0.35 * segment);
        let distance = sign * edge.boundary - 0.006 * (segment - 0.5);
        let step = 2.0 * smooth(-0.003, 0.003, distance) - 1.0;
        let taper = 1.0 - smooth(0.009, 0.042, distance.abs());
        let oblique = noise_at(strike * 23.0, key.wrapping_add(41)) - 0.5;
        let vertical = 5.2 * edge.closing.clamp(-1.0, 1.0) + 1.8 * edge.shear.min(1.0) * oblique;
        release * step * taper * vertical
    }

    fn fold_belt(&self, dir: DVec3, edge: &Edge, crust: f64) -> f64 {
        let compression = smooth(0.42, 0.76, crust) * smooth(0.10, 0.82, edge.closing);
        if compression <= 0.0 { return 0.0; }
        let (low, high) = (edge.owner.min(edge.neighbour), edge.owner.max(edge.neighbour));
        let key = self.seed ^ (low as u32).wrapping_mul(0x9e37_79b9) ^ (high as u32).wrapping_mul(0x85eb_ca6b);
        let axis = if edge.owner == low { edge.normal } else { -edge.normal };
        let strike = (dir - axis * dir.dot(axis)).normalize_or_zero();
        let cross = if edge.owner == low { edge.boundary } else { -edge.boundary };
        let cross = cross - 0.023 * (noise_at(strike * 34.0, key) - 0.5);
        let ridge = |centre: f64, width: f64| (1.0 - ((cross - centre) / width).powi(2)).max(0.0).powi(2);
        let saddle = smooth(0.29, 0.61, noise_at(strike * 71.0, key.wrapping_add(19)));
        let front = smooth(0.30, 0.72, noise_at(strike * 52.0, key.wrapping_add(29)));
        // The inner divide and lower thrust front leave a longitudinal
        // basin. Along-strike gaps give that basin outlets and passes.
        compression * (ridge(-0.017, 0.024) * (48.0 + 116.0 * saddle)
            + ridge(0.047, 0.024) * (22.0 + 69.0 * front)
            - ridge(0.016, 0.021) * 20.0)
    }

    /// An asymmetric fault belt on a signed cross-margin coordinate. Its
    /// strike follows the moving margin and segmentation is shared by both sides.
    fn fault(&self, dir: DVec3, edge: &Edge, own: Crust, other: Crust) -> (f64, f64) {
        let canonical = if edge.owner < edge.neighbour { 1.0 } else { -1.0 };
        let axis = edge.normal * canonical;
        let strike = (dir - axis * dir.dot(axis)).normalize_or_zero();
        let low = edge.owner.min(edge.neighbour) as u32;
        let high = edge.owner.max(edge.neighbour) as u32;
        let key = self.seed ^ low.wrapping_mul(0x9e37_79b9) ^ high.wrapping_mul(0x85eb_ca6b);
        let locked = smooth(0.27, 0.76, noise_at(strike * 42.0, key));
        let tilt = noise_at(strike * 19.0, key.wrapping_add(17));
        let compression = smooth(0.12, 0.9, edge.closing);
        let shear = smooth(0.22, 1.0, edge.shear - edge.closing.abs() * 0.35);
        let mixed = smooth(0.16, 0.62, (own.continental - other.continental).abs());
        let upper = if own.continental > other.continental { 1.0 } else { -1.0 };
        let s = upper * edge.boundary;
        let slab_age = if upper > 0.0 { other.ocean_age_myr } else { own.ocean_age_myr };
        let bend = 0.014 + 0.012 * smooth(15.0, 135.0, slab_age);
        let forearc = smooth(-0.012, bend, s) * (1.0 - smooth(0.027, 0.087, s));
        let trench = smooth(-0.060, -0.021, s) * (1.0 - smooth(-0.021, 0.006, s));
        let subduction = 38.0 * forearc - 15.0 * trench;

        // The steeper scarp faces the shortened margin; the opposite slope
        // rolls off over a wider block. Obliquity varies along strike.
        let cross = edge.boundary * canonical - (tilt - 0.5) * 0.027;
        let block = smooth(-0.011, 0.007, cross) * (1.0 - smooth(0.015, 0.075, cross));
        let continent = smooth(0.38, 0.76, own.continental.min(other.continental));
        let edge_weight = 1.0 - smooth(0.058, 0.095, edge.boundary);
        let displacement = compression * (0.35 + 0.65 * locked) * edge_weight
            * (mixed * subduction + (1.0 - mixed) * continent * 31.0 * block);
        let damage = ((compression * (0.32 + 0.68 * locked) + 0.65 * shear)
            * (1.0 - smooth(0.006, 0.033, cross.abs())) * edge_weight).min(1.0);
        (displacement, damage)
    }

    fn province(&self, dir: DVec3, edge: &Edge, crust: f64, history_m: f64, sides: [f64; 2], stresses: [f64; 3]) -> Province {
        self.province_with_noise(dir, edge, crust, history_m, sides, stresses, None)
    }

    fn province_with_noise(&self, dir: DVec3, edge: &Edge, crust: f64, history_m: f64,
        sides: [f64; 2], stresses: [f64; 3], cached: Option<&ProvinceNoise>) -> Province
    {
        let n = |f, salt| noise_at(dir * f, self.seed.wrapping_add(salt));
        let [collision, rift, _] = stresses;
        let Edge { boundary, closing, .. } = *edge;
        let coast = crust - 0.52 - self.ocean;
        let coast = if self.rifted_margins { coast } else {
            coast + (n(3.4 * self.scale, 821) - 0.5) * 0.55
                + (n(11.0 * self.scale, 823) - 0.5) * 0.13
        };
        let land = smooth(-0.025, 0.14, coast);
        let ridge = cached.map_or_else(|| 1.0 - (n(37.0, 827) * 2.0 - 1.0).abs(), |v| v.ridge);
        let relief = self.relief * (0.10 + collision * (0.55 + ridge * 0.45));
        // Regional uplift seeds hills and divides before water is routed.
        // A slower province mask leaves broad plains between hill countries;
        // erosion, not a post-process texture, connects their valleys.
        let body = dir * engine_core::cube_sphere::atlas::RADIUS_M;
        let country = cached.map_or_else(|| smooth(0.35, 0.66, n(18.0, 839)), |v| v.country);
        let rounded = cached.map_or_else(
            || noise_at(body / 1450.0, self.seed.wrapping_add(853)) * 2.0 - 1.0, |v| v.rounded);
        let spine = cached.map_or_else(
            || 1.0 - (noise_at(body / 850.0, self.seed.wrapping_add(857)) * 2.0 - 1.0).abs(), |v| v.spine);
        let regional = land * country * self.relief * 0.5
            * (rounded + 0.65 * (spine * spine - 0.55));
        let sea = f64::from(crate::WATER_LEVEL_METERS);
        let mut height = sea + coast * self.relief * 0.75
            + land * relief - rift * self.relief * 0.13 + regional;
        if history_m != 0.0 {
            height += history_m;
        }
        if self.coastal {
            let margin = 1.0 - smooth(0.045, 0.29, coast.abs());
            if !self.rifted_margins && margin > 0.0 {
                // Broad headlands and embayments share the same height field as drainage.
                let headlands = (n(31.0, 861) - 0.5) * 26.0 + (n(67.0, 863) - 0.5) * 8.0;
                let rift_cut = smooth(0.28, 0.62, rift) * smooth(0.38, 0.68, n(18.0, 877));
                let saddle = smooth(0.48, 0.73, n(21.0, 881)) * (1.0 - smooth(0.18, 0.62, rift));
                height += margin * (headlands - 30.0 * rift_cut + 17.0 * saddle);
            }
            // On mixed-crust boundaries, a broken ridge on the ocean side
            // can emerge as a chain of islands rather than isolated noise peaks.
            let [own, other] = sides;
            let mixed = (own - other).abs();
            let ocean_side = if own < other { boundary } else { -boundary };
            if !self.rifted_margins && mixed > 0.0 && closing > 0.0 && ocean_side > 0.0 && ocean_side < 0.095 {
                let arc = (-((ocean_side - 0.045) / 0.018).powi(2)).exp();
                let summits = smooth(0.36, 0.77, n(45.0, 887));
                height += 72.0 * smooth(0.1, 0.7, closing) * arc * summits;
            }
            if self.shape_shelf && !super::changing_seas(self.cfg.generator_version) && height < sea {
                let depth = sea - height;
                // Preserve the shoreline zero crossing and compress low shelves.
                let shaped = self.shelf_depth(dir, depth);
                let trench = 12.0 * mixed * collision * smooth(6.0, 25.0, depth);
                height = sea - shaped - trench;
            }
        }
        Province { height: height.max(-58.0), collision, rift,
            trench: (sides[0] - sides[1]).abs() * collision,
            carbonate: cached.map_or_else(|| smooth(0.45, 0.68, n(9.0, 829)), |v| v.carbonate)
                * (1.0 - collision.min(1.0)),
            boundary_m: boundary * engine_core::cube_sphere::atlas::RADIUS_M, convergence: closing,
            fault_damage: 0.0, fault_displacement_m: 0.0 }
    }
}

mod snapshot;

#[cfg(test)]
impl Plates {
    pub(in crate::planetary) fn clear_drift_for_history_stencil_test(&mut self) {
        self.drift = None;
    }
}
