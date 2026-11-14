//! Compressed oceanic hotspot tracks. Ages are elapsed millions of years
//! since an edifice began growing. Plate motion is scaled to the game planet.
use super::SEA;
use super::tectonics::{Plates, smooth};
use super::volcanism::{Bins, CELL_M, bins, cell};
use crate::planet::noise_at;
use crate::volcano::{Kind, Vent};
use engine_core::cube_sphere::atlas;
use glam::{DQuat, DVec3};

const RADIUS: f64 = atlas::RADIUS_M;
const AGES_MYR: [f64; 7] = [0.05, 0.86, 1.65, 2.9, 4.5, 6.2, 8.4];
const ACTIVE_AGE: usize = 1;

struct Shield {
    dir: DVec3,
    along: DVec3,
    across: DVec3,
    exposure: f64,
    radius: f64,
    rise: f64,
    bend: f64,
    failure: f64,
    seed: u32,
    age_myr: f64,
    water_depth: f64,
}

impl Shield {
    fn evolving(dir: DVec3, along: DVec3, age_myr: f64, water_depth: f64, seed: u32) -> Self {
        let mut shield = Self::new(dir, along, age_myr, water_depth, seed);
        let growth = smooth(0.0, 0.99, age_myr);
        let built = water_depth * (0.34 + 0.66 * growth) + (224.0 + 30.0 * roll(seed, 1)) * growth;
        // Cooling subsidence decelerates with sqrt(age); there is no fixed
        // birthday at which every island is forced below the sea surface.
        let subsidence = 38.0 * ((age_myr + 1.0).sqrt() - 2.0_f64.sqrt()).max(0.0);
        shield.rise = (built - subsidence).max(0.0);
        shield
    }

    fn new(dir: DVec3, along: DVec3, age_myr: f64, water_depth: f64, seed: u32) -> Self {
        let growth = smooth(0.0, 0.99, age_myr);
        let sinking = smooth(1.55, 8.4, age_myr);
        let radius = (1000.0 + 1300.0 * growth) * (0.88 + 0.24 * roll(seed, 0));
        let built = water_depth * (0.34 + 0.66 * growth) + (224.0 + 30.0 * roll(seed, 1)) * growth;
        let rise = built + (water_depth * 0.64 - built) * sinking;
        let failure_age = 1.05 + 2.4 * roll(seed, 2);
        Self {
            dir,
            along,
            across: dir.cross(along),
            exposure: smooth(AGES_MYR[ACTIVE_AGE], 4.8, age_myr),
            radius,
            rise,
            bend: (roll(seed, 3) - 0.5) * 0.95,
            failure: smooth(failure_age, failure_age + 0.35, age_myr),
            seed,
            age_myr,
            water_depth,
        }
    }

    fn relief(&self, dir: DVec3) -> f64 {
        let offset = (dir - self.dir) * (RADIUS / self.radius);
        let distance_squared = offset.length_squared();
        if distance_squared >= 1.95_f64.powi(2) {
            return 0.0;
        }
        let x = offset.dot(self.along);
        let y = offset.dot(self.across);
        let body = dir * RADIUS;
        let rough = noise_at(body / 720.0, self.seed) - 0.5;
        let dome = |r2: f64| (1.0 - r2).max(0.0).powf(1.5);

        // Opposite rift zones differ in length and curve in opposite directions.
        // They build broad lava aprons that can coalesce across the next saddle.
        let axial = if x > 0.0 { 1.13 } else { 0.61 };
        let core = dome(
            (x / axial).powi(2)
                + ((y - self.bend * x) / 0.75).powi(2)
                + rough * (x * x + y * y) * 0.48,
        );
        let rift_y = y - self.bend * (x + x * x * 0.35);
        let long_rift = dome(((x - 0.49) / 1.18).powi(2) + (rift_y / 0.37).powi(2));
        let short_rift = dome(((x + 0.24) / 0.56).powi(2) + ((y + 0.36 * x) / 0.41).powi(2));
        let edifice = (core * 0.73 + long_rift * 0.30 + short_rift * 0.16).min(1.0);

        // A single flank fails across the rift, leaving a broken headwall and
        // a broad debris apron downslope. The headwall is jagged at map scale.
        let side = if self.seed & 1 == 0 { y } else { -y };
        let headwall = 0.18 + x * x * 0.56 + rough * 0.27;
        let scar =
            smooth(headwall, headwall + 0.19, side) * (1.0 - smooth(0.35, 0.91, (x - 0.13).abs()));
        let debris = dome(((x - 0.23) / 0.85).powi(2) + ((side - 0.93) / 0.55).powi(2));
        self.rise
            * (edifice * (1.0 - self.failure * scar * 0.69) + self.failure * debris * 0.085)
            * (1.0 - smooth(1.68_f64.powi(2), 1.95_f64.powi(2), distance_squared))
    }
}

fn roll(seed: u32, salt: u32) -> f64 {
    f64::from(crate::hash_u32(
        seed.wrapping_add(salt.wrapping_mul(0x9e37_79b9)),
    )) / f64::from(u32::MAX)
}

#[derive(Clone, Copy, Default)]
pub(super) struct Sample {
    pub lift: f64,
    pub exposure: f64,
}

pub(super) struct Tracks {
    shields: Vec<Shield>,
    bins: Bins,
    vents: Vec<Vent>,
}

impl Tracks {
    /// Reconstruct the connected volcanic platform before its mapped cooling
    /// subsidence. The same rift lobes and plate-carried centres define both.
    pub(super) fn island_history(&self, dir: DVec3) -> (f64, f64) {
        let mut old_sum = 0.0;
        let mut old_max = 0.0_f64;
        let mut new_sum = 0.0;
        let mut new_max = 0.0_f64;
        let mut age = 0.0;
        let mut weight = 0.0;
        let mut young_lift = 0.0_f64;
        for &i in self.bins.get(&cell(dir, CELL_M)).into_iter().flatten() {
            let shield = &self.shields[i];
            let present = shield.relief(dir);
            if shield.age_myr <= 1.0 { young_lift = young_lift.max(present); }
            let early = if shield.age_myr > 1.0 {
                Shield::new(shield.dir, shield.along, 1.0, shield.water_depth, shield.seed).relief(dir)
            } else { present };
            old_sum += early; old_max = old_max.max(early);
            new_sum += present; new_max = new_max.max(present);
            let share = early.powi(4);
            age += share * (shield.age_myr - 1.0).max(0.0);
            weight += share;
        }
        let missing = (old_max + (old_sum - old_max) * 0.16 - new_max - (new_sum - new_max) * 0.16).max(0.0);
        if young_lift > 4.0 { return (0.0, 0.0); }
        (missing, age / weight.max(1e-12))
    }

    pub fn new(seed: u32, plates: &Plates) -> Self {
        let mut shields: Vec<Shield> = Vec::new();
        let mut vents = Vec::new();
        let spawn = (atlas::to_body(DVec3::ZERO) / RADIUS).normalize();
        for i in 0..320u32 {
            let key = seed.wrapping_add(i.wrapping_mul(0xd1b5_4a35));
            let y = 2.0 * roll(key, 10) - 1.0;
            let longitude = roll(key, 11) * std::f64::consts::TAU;
            let r = (1.0 - y * y).sqrt();
            let head = DVec3::new(r * longitude.cos(), y, r * longitude.sin());
            let Some((plate, spin)) = plates.oceanic_motion(head) else {
                continue;
            };
            let axis = spin.normalize_or_zero();
            let lever = axis.cross(head).length();
            if lever < 0.30 {
                continue;
            }
            let resolved = super::resolved_history(plates.config().generator_version);
            // Longer-lived plumes form old island chains at their carrier's
            // actual speed; do not accelerate plates to space out landmarks.
            let mut ages = AGES_MYR;
            if resolved {
                for j in 2..ages.len() {
                    ages[j] = ages[j - 1] + (2.4 + (j - 2) as f64 * 3.8) * (0.8 + 0.4 * roll(key, 90 + j as u32));
                }
            }
            let speed = 1650.0 + 390.0 * spin.length();
            let drift = speed / (RADIUS * lever);
            let track = if resolved {
                let plume = plates.carried_rotation(plate, ages[ACTIVE_AGE]).conjugate() * head;
                ages.map(|age| plates.carried_rotation(plate, age) * plume)
            } else if let Some(prior) = plates.previous_spin(plate) {
                let turn = f64::from(super::drift::MOTION_CHANGE_AGE_MYR);
                let rotation = |age: f64| {
                    DQuat::from_axis_angle(axis, age.min(turn) * drift)
                        * DQuat::from_scaled_axis(prior * ((age - turn).max(0.0) * drift / spin.length()))
                };
                let plume = rotation(AGES_MYR[ACTIVE_AGE]).conjugate() * head;
                AGES_MYR.map(|age| rotation(age) * plume)
            } else {
                AGES_MYR.map(|age| {
                    DQuat::from_axis_angle(axis, (age - AGES_MYR[ACTIVE_AGE]) * drift) * head
                })
            };
            if track.iter().any(|&dir| {
                (dir - spawn).length() * RADIUS < 9500.0
                    || plates.oceanic_motion(dir).is_none_or(|(p, _)| p != plate)
                    || shields
                        .iter()
                        .any(|s| (s.dir - dir).length() * RADIUS < 4900.0)
            }) {
                continue;
            }
            let depths = track.map(|dir| SEA - plates.at(dir).height);
            if depths.iter().any(|&depth| depth < 24.0) {
                continue;
            }

            for (j, (dir, age)) in track.into_iter().zip(ages).enumerate() {
                let motion = if age > f64::from(super::drift::MOTION_CHANGE_AGE_MYR) {
                    plates.previous_spin(plate).map_or(axis, |prior| {
                        DQuat::from_axis_angle(axis, f64::from(super::drift::MOTION_CHANGE_AGE_MYR) * drift) * prior
                    })
                } else { axis };
                let along = if resolved {
                    let younger = plates.carried_rotation(plate, (age - 0.05).max(0.0));
                    let older = plates.carried_rotation(plate, age + 0.05);
                    let plume = plates.carried_rotation(plate, age).conjugate() * dir;
                    (older * plume - younger * plume).normalize_or(axis.cross(dir).normalize_or_zero())
                } else { motion.cross(dir).normalize() };
                let make = if resolved { Shield::evolving } else { Shield::new };
                shields.push(make(
                    dir,
                    along,
                    age,
                    depths[j],
                    key.wrapping_add(j as u32 * 701),
                ));
            }
            vents.push(Vent {
                dir: head,
                kind: Kind::Shield,
                active: true,
                radius_m: 1050.0,
                height_m: 220.0,
            });
            if vents.len() == 3 {
                break;
            }
        }
        let bins = bins(shields.iter().map(|s| (s.dir, s.radius * 1.95)), CELL_M);
        Self {
            shields,
            bins,
            vents,
        }
    }

    pub fn active_vents(&self) -> &[Vent] {
        &self.vents
    }

    pub fn weathered_shields(&self) -> impl Iterator<Item = Vent> + '_ {
        self.shields.iter().filter(|shield| shield.exposure > 0.25).map(|shield| Vent {
            dir: shield.dir,
            kind: Kind::Shield,
            active: false,
            radius_m: (shield.radius * 0.65).min(1500.0),
            height_m: shield.rise,
        })
    }

    pub fn active_rifts(&self) -> impl Iterator<Item = (Vent, DVec3, u32)> + '_ {
        self.vents.iter().copied().zip(self.shields.chunks_exact(AGES_MYR.len()))
            .map(|(vent, shields)| {
                let shield = &shields[ACTIVE_AGE];
                (vent, shield.along, shield.seed)
            })
    }

    pub fn sample(&self, dir: DVec3) -> Sample {
        let mut sample = Sample::default();
        let mut sum = 0.0;
        let mut time = 0.0;
        let mut weight = 0.0;
        for &i in self.bins.get(&cell(dir, CELL_M)).into_iter().flatten() {
            let shield = &self.shields[i];
            let lift = shield.relief(dir);
            sum += lift;
            sample.lift = sample.lift.max(lift);
            // The larger deposit sets the surface age where shields overlap.
            // Blend near equal thicknesses to avoid a hard seam in incision.
            let share = lift.powi(4);
            time += share * shield.exposure;
            weight += share;
        }
        if weight > 0.0 {
            sample.exposure = time / weight;
        }
        sample.lift += (sum - sample.lift) * 0.16;
        sample
    }
}
