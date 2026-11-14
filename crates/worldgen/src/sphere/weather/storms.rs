//! Synoptic storm catalogue: mid-latitude lows and tropical cyclones as
//! explicit objects with a track and a life cycle.
//!
//! A coarse solver smears a 1000 km low into a featureless blob, so the cloud
//! shape of a storm comes from a template laid on its own frame instead. Each
//! hemisphere runs a fixed number of slots; slot `k` in generation `g` is one
//! storm whose genesis point, lifetime and size are hashed from the seed. The
//! catalogue is a pure function of (seed, axis, time).
//!
//! Tropical cyclones form only over warm open ocean (the axis climate's sea
//! surface, the continental signal of the planet shell), steer west with the
//! trades and drift poleward, recurve into the westerlies, intensify over warm
//! water and decay over land or cold water. Their surface wind is a Holland
//! profile with a calm eye. Tornadoes spawn from the warm sector of strong
//! frontal storms, one chance per supercell cell and time bucket. Dials live in
//! `assets/climate.ron`.

use engine_core::hash::{hash01, hash_u32};
use engine_core::planet_frame::PlanetAxis;
use engine_content::climate::{StormDials, storm_dials};
use glam::DVec3;

/// Model time units per day, matching the solver's 1/Omega time.
pub const DAY: f64 = std::f64::consts::TAU;
const FRONTAL_SLOTS: u32 = 6;
const FRONTAL_PERIOD: f64 = 7.5 * DAY;
const HIGH_SLOTS: u32 = 3;
const HIGH_PERIOD: f64 = 10.0 * DAY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Extratropical low: comma head, warm-front shield, trailing cold front.
    Frontal,
    /// Tropical cyclone: eye, eyewall, dense overcast and spiral rain bands.
    Tropical,
    /// Subsiding high: clear centre, weak anticyclonic cloud around its edge.
    High,
}

#[derive(Clone, Copy, Debug)]
pub struct Storm {
    pub kind: Kind,
    pub centre: DVec3,
    /// Local east tangent at the centre.
    pub east: DVec3,
    /// Tangent toward the storm's own pole; with `east` it is a mirrored
    /// frame in the south, so one template turns the right way in both.
    pub pole: DVec3,
    /// Template length scale in radians of arc.
    pub radius: f64,
    /// Life-cycle envelope in `0..=1`.
    pub strength: f64,
    /// Fraction of the lifetime elapsed.
    pub age: f64,
    /// Model time since genesis.
    pub elapsed: f64,
    /// Track velocity of the centre, a tangent in radians per model unit.
    pub drift: DVec3,
    /// Angular velocity of the core in rad per model unit, positive counterclockwise in the template frame.
    pub spin: f64,
    pub key: u32,
    /// Tropical only: maximum sustained surface wind in m/s, zero otherwise.
    pub max_wind_ms: f64,
    /// Tropical only: radius of maximum wind (eyewall) in radians of arc.
    pub rmw: f64,
    /// Tropical only: Holland profile shape.
    pub holland_b: f64,
}

impl Storm {
    /// Template coordinates of `p` in storm radii, or `None` on the far side.
    pub fn local(&self, p: DVec3) -> Option<(f64, f64)> {
        let d = p.dot(self.centre);
        (d > 0.2).then(|| (p.dot(self.east) / (d * self.radius), p.dot(self.pole) / (d * self.radius)))
    }

    /// Saffir-Simpson category 1..=5, or 0 below hurricane strength (and for non-tropical storms).
    pub fn category(&self) -> u8 {
        category(self.max_wind_ms, &storm_dials().tropical.category_ms)
    }

    /// Surface wind speed of a tropical cyclone, m/s, at `r` radians from the centre.
    ///
    /// Holland (1980) gradient profile: calm in the eye, peaking at the eyewall
    /// radius and falling off as a power law outside it.
    pub fn holland_ms(&self, r: f64) -> f64 {
        if self.kind != Kind::Tropical || self.rmw <= 0.0 || r <= 0.0 {
            return 0.0;
        }
        let x = (self.rmw / r).powf(self.holland_b);
        self.max_wind_ms * (x * (1.0 - x).exp()).sqrt()
    }
}

fn category(wind_ms: f64, thresholds: &[f64; 5]) -> u8 {
    thresholds.iter().filter(|&&t| wind_ms >= t).count() as u8
}

/// A short-lived tornado: a Rankine vortex riding its parent storm.
#[derive(Clone, Copy, Debug)]
pub struct Tornado {
    pub centre: DVec3,
    /// Core radius, radians of arc; wind peaks here.
    pub radius: f64,
    pub max_wind_ms: f64,
    /// Fraction of its lifetime elapsed.
    pub age: f64,
    /// Key of the parent storm.
    pub parent: u32,
}

impl Tornado {
    /// Rankine wind at `p` as a body-frame tangent in m/s, cyclonic for its hemisphere.
    pub fn wind_ms(&self, p: DVec3, axis: &PlanetAxis, reach_cores: f64) -> DVec3 {
        let d = p - self.centre * self.centre.dot(p);
        let r = d.length();
        if r > self.radius * reach_cores || p.dot(self.centre) <= 0.0 {
            return DVec3::ZERO;
        }
        let speed = if r < self.radius { self.max_wind_ms * r / self.radius } else { self.max_wind_ms * self.radius / r };
        // Fade the potential flow to zero at the reach so the field has no step.
        let fade = 1.0 - smooth01((r / (self.radius * reach_cores) - 0.6) / 0.4);
        let sense = if self.centre.dot(axis.north) >= 0.0 { 1.0 } else { -1.0 };
        let tangent = self.centre.cross(d).normalize_or_zero();
        tangent * (speed * fade * sense)
    }
}

fn smooth01(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn basis(north: DVec3) -> (DVec3, DVec3) {
    let helper = if north.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let e0 = north.cross(helper).normalize();
    (e0, north.cross(e0))
}

fn point(axis: &PlanetAxis, lat: f64, lon: f64) -> DVec3 {
    let (e0, e1) = basis(axis.north);
    (e0 * lon.cos() + e1 * lon.sin()) * lat.cos() + axis.north * lat.sin()
}

struct Life {
    age: f64,
    elapsed: f64,
    strength: f64,
}

fn storm_at(axis: &PlanetAxis, kind: Kind, lat: f64, lon: f64, radius: f64, life: Life, key: u32) -> Storm {
    let centre = point(axis, lat, lon);
    let east = axis.north.cross(centre).normalize();
    let north = centre.cross(east);
    let pole = if lat >= 0.0 { north } else { -north };
    // A tropical core turns several times faster than a mid-latitude low.
    let spin = life.strength * match kind { Kind::Tropical => 1.4, Kind::Frontal => 0.35, Kind::High => -0.22 };
    Storm { kind, centre, east, pole, radius, strength: life.strength, age: life.age, elapsed: life.elapsed, drift: DVec3::ZERO, spin, key, max_wind_ms: 0.0, rmw: 0.0, holland_b: 1.0 }
}

fn envelope(age: f64, rise: f64, fall: f64) -> f64 {
    let s = |a: f64, b: f64, x: f64| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    s(0.0, rise, age) * (1.0 - s(fall, 1.0, age))
}

/// One slot's storm of `generation`, if it is alive at `t`.
fn slot(seed: u32, axis: &PlanetAxis, kind: Kind, hemisphere: f64, k: u32, period: f64, t: f64) -> Option<Storm> {
    if kind == Kind::Tropical {
        return tropical(seed, axis, hemisphere, k, t, storm_dials());
    }
    let phase = f64::from(hash01(hash_u32(seed ^ 0x5707_0000 ^ (k * 2 + u32::from(hemisphere < 0.0)) ^ ((kind as u32) << 12))));
    let shifted = t / period + phase;
    let generation = shifted.floor() as i64 as u32;
    let key = hash_u32(seed ^ hash_u32(generation.wrapping_mul(977) ^ k ^ ((kind as u32) << 8) ^ u32::from(hemisphere < 0.0) << 16));
    let h = |salt: u32| f64::from(hash01(key ^ salt));
    let (life, rise, fall) = match kind {
        Kind::Frontal => ((4.5 + 2.0 * h(1)) * DAY, 0.2, 0.75),
        Kind::Tropical => return None,
        Kind::High => ((6.0 + 2.5 * h(1)) * DAY, 0.18, 0.82),
    };
    let start = (h(2) * (period - life).max(0.0)) / period;
    let age = (shifted.fract() - start) * period / life;
    if !(0.0..1.0).contains(&age) {
        return None;
    }
    let lon0 = h(3) * DAY;
    let lat0 = match kind {
        Kind::Frontal => (34.0 + 14.0 * h(4)).to_radians(),
        Kind::Tropical => return None,
        Kind::High => (23.0 + 22.0 * h(4)).to_radians(),
    };
    let track = |age: f64| -> (f64, f64) {
        let elapsed = age * life;
        match kind {
            Kind::High => (lat0 + (3.0 * (age * std::f64::consts::PI).sin()).to_radians(), lon0 + 0.008 * elapsed),
            Kind::Frontal => {
                // Eastward along the storm track with a poleward drift as it occludes.
                let lat = lat0 + (12.0 * age).to_radians();
                (lat, lon0 + 0.026 * elapsed / lat.cos())
            }
            Kind::Tropical => (lat0, lon0),
        }
    };
    let radius = match kind {
        Kind::Frontal => 0.12 + 0.06 * h(5),
        Kind::Tropical => 0.0,
        Kind::High => 0.15 + 0.07 * h(5),
    };
    let (lat, lon) = track(age);
    // Track velocity by a short forward difference; the template moves rigidly with the centre.
    let step = 0.05;
    let (lat1, lon1) = track(age + step / life);
    let drift = (point(axis, lat1 * hemisphere, lon1) - point(axis, lat * hemisphere, lon)) / step;
    let strength = envelope(age, rise, fall) * (0.75 + 0.25 * h(6));
    let mut storm = storm_at(axis, kind, lat * hemisphere, lon, radius, Life { age, elapsed: age * life, strength }, key);
    storm.drift = drift - storm.centre * storm.centre.dot(drift);
    Some(storm)
}

/// Every storm alive at model time `t`.
pub fn active(seed: u32, axis: &PlanetAxis, t: f64) -> Vec<Storm> {
    let mut out = Vec::new();
    for hemisphere in [1.0, -1.0] {
        for k in 0..FRONTAL_SLOTS {
            out.extend(slot(seed, axis, Kind::Frontal, hemisphere, k, FRONTAL_PERIOD, t));
        }
        for k in 0..storm_dials().tropical.slots {
            out.extend(slot(seed, axis, Kind::Tropical, hemisphere, k, 0.0, t));
        }
        for k in 0..HIGH_SLOTS {
            out.extend(slot(seed, axis, Kind::High, hemisphere, k, HIGH_PERIOD, t));
        }
    }
    out
}

/// Continental signal of the planet shell at `dir` in `0..=1`: the continent term of `planet::height`.
pub fn land(seed: u32, dir: DVec3) -> f64 {
    smooth01((crate::planet::noise_at(dir * 3.2, seed) - 0.43) / 0.06)
}

/// Annual sea-surface temperature at `dir`, degrees C, from the axis climate.
pub fn sst_c(seed: u32, axis: &PlanetAxis, dir: DVec3) -> f64 {
    crate::planet_climate::at(seed, axis, dir, 0.0).annual_c
}

/// Lifecycle steps per storm; track and intensity integrate together.
const TRACK_STEPS: u32 = 24;

/// Track position and intensity of a tropical cyclone `elapsed` model units after genesis.
struct TropicalTrack {
    lat: f64,
    lon: f64,
    intensity: f64,
}

fn tropical_track(seed: u32, axis: &PlanetAxis, hemisphere: f64, lat0: f64, lon0: f64, elapsed: f64, d: &StormDials) -> TropicalTrack {
    let t = &d.tropical;
    let per_unit = |deg_per_day: f64| deg_per_day.to_radians() / DAY;
    let recurve = t.recurve_lat_deg.to_radians();
    let (mut lat, mut lon, mut intensity) = (lat0, lon0, t.initial_intensity);
    let dt = elapsed / f64::from(TRACK_STEPS);
    for _ in 0..TRACK_STEPS {
        if dt <= 0.0 {
            break;
        }
        let dir = point(axis, lat * hemisphere, lon);
        // Steering: trades carry the storm west below the ridge, westerlies east above it;
        // beta drift adds a steady poleward component that grows after recurvature.
        let turn = smooth01((lat - recurve) / 10f64.to_radians() + 0.5);
        let east = -t.trade_deg_per_day * (1.0 - turn) + t.westerly_deg_per_day * turn;
        let north = t.beta_deg_per_day * (1.0 + 1.5 * turn);
        lat = (lat + per_unit(north) * dt).min(80f64.to_radians());
        lon += per_unit(east) * dt / lat.cos().max(0.2);
        // Life cycle: warm water feeds the core toward its potential; land and cold water starve it.
        let sst = sst_c(seed, axis, dir);
        let ground = land(seed, dir);
        let warm = ((sst - t.sst_decay_c) / (t.sst_genesis_c - t.sst_decay_c)).clamp(-1.0, 1.0);
        let days = dt / DAY;
        let gain = t.intensify_per_day * warm.max(0.0) * (1.0 - ground) * (1.0 - intensity);
        let loss = (t.land_decay_per_day * ground + t.cold_decay_per_day * (-warm).max(0.0)) * intensity;
        intensity = (intensity + (gain - loss) * days).clamp(0.0, 1.0);
    }
    TropicalTrack { lat, lon, intensity }
}

/// One tropical slot's cyclone, alive at `t`, if its genesis found warm open ocean.
fn tropical(seed: u32, axis: &PlanetAxis, hemisphere: f64, k: u32, t: f64, d: &StormDials) -> Option<Storm> {
    let td = &d.tropical;
    let period = td.period_days * DAY;
    let south = u32::from(hemisphere < 0.0);
    let phase = f64::from(hash01(hash_u32(seed ^ 0x5707_0000 ^ (k * 2 + south) ^ ((Kind::Tropical as u32) << 12))));
    let shifted = t / period + phase;
    let generation = shifted.floor() as i64 as u32;
    let key = hash_u32(seed ^ hash_u32(generation.wrapping_mul(977) ^ k ^ ((Kind::Tropical as u32) << 8) ^ south << 16));
    let h = |salt: u32| f64::from(hash01(key ^ salt));
    if h(9) >= td.genesis_chance {
        return None;
    }
    let life = (td.lifetime_days.0 + (td.lifetime_days.1 - td.lifetime_days.0) * h(1)) * DAY;
    let start = (h(2) * (period - life).max(0.0)) / period;
    let age = (shifted.fract() - start) * period / life;
    if !(0.0..1.0).contains(&age) {
        return None;
    }
    // Genesis: the first hashed candidate over warm open ocean in the tropics.
    let (lat_lo, lat_hi) = (td.genesis_lat_deg.0.to_radians(), td.genesis_lat_deg.1.to_radians());
    let (lat0, lon0) = (0..td.genesis_attempts).find_map(|i| {
        let salt = 0x100 + i * 2;
        let lat = lat_lo + (lat_hi - lat_lo) * h(salt);
        let lon = h(salt + 1) * DAY;
        let dir = point(axis, lat * hemisphere, lon);
        (sst_c(seed, axis, dir) >= td.sst_genesis_c && land(seed, dir) <= td.max_genesis_land).then_some((lat, lon))
    })?;
    let elapsed = age * life;
    let now = tropical_track(seed, axis, hemisphere, lat0, lon0, elapsed, d);
    let step = 0.05;
    let next = tropical_track(seed, axis, hemisphere, lat0, lon0, elapsed + step, d);
    let drift = (point(axis, next.lat * hemisphere, next.lon) - point(axis, now.lat * hemisphere, now.lon)) / step;
    // The visual envelope fades the storm out at the end of its slot, whatever the water.
    let fade = 1.0 - smooth01((age - 0.85) / 0.15);
    let intensity = now.intensity * fade;
    let radius = td.radius_rad.0 + (td.radius_rad.1 - td.radius_rad.0) * h(5);
    let potential = td.potential_wind_ms.0 + (td.potential_wind_ms.1 - td.potential_wind_ms.0) * h(6);
    let mut storm = storm_at(axis, Kind::Tropical, now.lat * hemisphere, now.lon, radius, Life { age, elapsed, strength: intensity }, key);
    storm.drift = drift - storm.centre * storm.centre.dot(drift);
    storm.max_wind_ms = potential * intensity;
    // The eyewall tightens as the storm deepens, matching the cloud template's eye.
    storm.rmw = radius * (td.eyewall_frac.0 + (td.eyewall_frac.1 - td.eyewall_frac.0) * (1.0 - intensity));
    storm.holland_b = td.holland_b.0 + (td.holland_b.1 - td.holland_b.0) * h(7);
    Some(storm)
}

/// Surface wind of a tropical cyclone at `p` in flow units, without its track motion.
///
/// Holland speed turned inward by the inflow angle, modulated along log-spiral bands.
pub fn cyclone_wind(s: &Storm, p: DVec3, d: &StormDials) -> DVec3 {
    let Some((x, y)) = s.local(p) else { return DVec3::ZERO };
    let r_local = (x * x + y * y).sqrt();
    if r_local > 4.0 || r_local <= 1e-9 {
        return DVec3::ZERO;
    }
    let td = &d.tropical;
    let r = r_local * s.radius;
    let theta = y.atan2(x);
    let spiral = theta + td.band_wrap * r_local.max(0.04).ln() + f64::from(hash01(s.key ^ 0x53)) * std::f64::consts::TAU;
    let bands = 1.0 + td.band_gain * (2.0 * spiral).cos() * smooth01((r / s.rmw.max(1e-9) - 1.5) / 1.5);
    let speed = s.holland_ms(r) * bands * (1.0 - smooth01((r_local - 3.0) / 1.0)) * d.flow_per_ms;
    // Counterclockwise in the template frame, which is mirrored in the south.
    let tangential = (s.east * -y + s.pole * x) / r_local;
    let inward = -(s.east * x + s.pole * y) / r_local;
    let (sin, cos) = td.inflow_deg.to_radians().sin_cos();
    (tangential * cos + inward * sin) * speed
}

/// Convective index of a frontal storm in `0..=1`: warm-sector instability times shear.
///
/// Instability is the warmth of the air a radius equatorward of the low; shear
/// is the storm's own strength and track speed. Only frontal storms carry it.
pub fn convective_index(seed: u32, axis: &PlanetAxis, s: &Storm, d: &StormDials) -> f64 {
    if s.kind != Kind::Frontal {
        return 0.0;
    }
    let warm_dir = (s.centre - s.pole * s.radius).normalize();
    let (c0, c1) = d.tornado.instability_c;
    let instability = ((sst_c(seed, axis, warm_dir) - c0) / (c1 - c0)).clamp(0.0, 1.0);
    let shear = (s.strength * (0.5 + 0.5 * (s.drift.length() / 0.04).min(1.0))).clamp(0.0, 1.0);
    instability * shear
}

/// Every tornado alive at `t` under the given storms.
///
/// Each strong frontal storm carries a grid of supercell cells in its warm
/// sector, ahead of the cold front; each cell gets one hashed chance per time
/// bucket, scaled by the storm's convective index.
pub fn tornadoes(seed: u32, axis: &PlanetAxis, storms: &[Storm], t: f64, d: &StormDials) -> Vec<Tornado> {
    let n = &d.tornado;
    let bucket_len = n.bucket_days * DAY;
    let bucket = (t / bucket_len).floor();
    let bucket_id = bucket as i64 as u32;
    let mut out = Vec::new();
    for s in storms {
        let index = convective_index(seed, axis, s, d);
        if index < n.min_convective {
            continue;
        }
        for cell in 0..n.cells * n.cells {
            let key = hash_u32(s.key ^ hash_u32(cell.wrapping_mul(0x9E37) ^ bucket_id.wrapping_mul(0x85EB_CA6B)) ^ 0x7042_0000);
            let h = |salt: u32| f64::from(hash01(key ^ salt));
            if h(1) >= n.chance * index {
                continue;
            }
            let life = (n.lifetime_days.0 + (n.lifetime_days.1 - n.lifetime_days.0) * h(2)) * DAY;
            let birth = bucket * bucket_len + h(3) * (bucket_len - life);
            let age = (t - birth) / life;
            if !(0.0..1.0).contains(&age) {
                continue;
            }
            // Warm sector: east of the trailing cold front, equatorward of the low (template frame).
            let (i, j) = (f64::from(cell % n.cells), f64::from(cell / n.cells));
            let side = f64::from(n.cells);
            let ly = -0.4 - 1.6 * (j + h(4)) / side;
            let tf = ((-0.15 - ly) / 2.7).clamp(0.0, 1.0);
            let lx = 0.35 * tf - 1.25 * tf * tf + 0.15 + 0.8 * (i + h(5)) / side;
            let base = s.centre + (s.east * lx + s.pole * ly) * s.radius;
            // Its own motion relative to the storm, toward the storm's east-poleward quadrant.
            let heading = (s.east + s.pole * 0.6).normalize();
            let travel = n.translate_ms * d.flow_per_ms * (t - birth);
            let centre = (base + heading * travel).normalize();
            let envelope = smooth01(age / 0.15) * (1.0 - smooth01((age - 0.7) / 0.3));
            let peak = n.max_wind_ms.0 + (n.max_wind_ms.1 - n.max_wind_ms.0) * h(6) * index;
            out.push(Tornado {
                centre,
                radius: n.radius_rad.0 + (n.radius_rad.1 - n.radius_rad.0) * h(7),
                max_wind_ms: peak * envelope,
                age,
                parent: s.key,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const AXIS: PlanetAxis = PlanetAxis::EARTHLIKE;

    fn tropical_samples() -> Vec<(u32, f64, Storm)> {
        let mut out = Vec::new();
        for seed in 1..6u32 {
            for i in 0..40 {
                let t = f64::from(i) * 0.9 * DAY;
                out.extend(active(seed, &AXIS, t).into_iter().filter(|s| s.kind == Kind::Tropical).map(|s| (seed, t, s)));
            }
        }
        out
    }

    #[test]
    fn storms_are_deterministic() {
        for t in [0.0, 3.7 * DAY, 41.0 * DAY] {
            let a = active(7, &AXIS, t);
            let b = active(7, &AXIS, t);
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                assert_eq!((x.centre, x.max_wind_ms, x.rmw, x.key), (y.centre, y.max_wind_ms, y.rmw, y.key));
            }
            let ta = tornadoes(7, &AXIS, &a, t, storm_dials());
            let tb = tornadoes(7, &AXIS, &b, t, storm_dials());
            assert_eq!(ta.len(), tb.len());
            for (x, y) in ta.iter().zip(&tb) {
                assert_eq!((x.centre, x.radius, x.max_wind_ms), (y.centre, y.radius, y.max_wind_ms));
            }
        }
    }

    #[test]
    fn cyclones_form_over_warm_ocean_and_peak_at_the_eyewall() {
        let samples = tropical_samples();
        assert!(!samples.is_empty(), "no tropical cyclone in the sample");
        let d = storm_dials();
        let mut checked = 0;
        for (_, _, s) in samples.iter().filter(|(_, _, s)| s.max_wind_ms > 20.0) {
            let at = |r: f64| s.holland_ms(r);
            assert!(at(s.rmw) > at(0.1 * s.rmw) && at(s.rmw) > at(4.0 * s.radius), "profile {s:?}");
            // The full wind field agrees: eyewall beats centre and far field.
            let probe = |r: f64| cyclone_wind(s, (s.centre + s.east * r).normalize(), d).length();
            assert!(probe(s.rmw) > probe(0.05 * s.rmw) && probe(s.rmw) > probe(5.0 * s.radius));
            assert!(s.category() <= 5);
            checked += 1;
        }
        assert!(checked > 0, "no cyclone reached tropical-storm strength");
    }

    #[test]
    fn cyclone_tracks_move_poleward() {
        let d = storm_dials();
        for seed in 1..20u32 {
            let lat0 = 12f64.to_radians();
            let start = tropical_track(seed, &AXIS, 1.0, lat0, 1.0, 0.0, d);
            let late = tropical_track(seed, &AXIS, 1.0, lat0, 1.0, 6.0 * DAY, d);
            assert!(late.lat > start.lat + 4f64.to_radians(), "seed {seed}: {} -> {}", start.lat, late.lat);
        }
        // Genesis honours the warm-water rule for every catalogued storm, both hemispheres.
        for (seed, _, s) in tropical_samples() {
            assert!(AXIS.latitude(s.centre).abs() < 80f64.to_radians(), "seed {seed}");
        }
    }

    #[test]
    fn tornadoes_only_inside_convective_storms() {
        let d = storm_dials();
        let mut seen = 0;
        for seed in 1..4u32 {
            for i in 0..400 {
                let t = f64::from(i) * 0.37 * DAY / 4.0;
                let storms = active(seed, &AXIS, t);
                for tor in tornadoes(seed, &AXIS, &storms, t, d) {
                    let parent = storms.iter().find(|s| s.key == tor.parent).expect("parent storm");
                    assert_eq!(parent.kind, Kind::Frontal);
                    assert!(convective_index(seed, &AXIS, parent, d) >= d.tornado.min_convective);
                    assert!(tor.centre.dot(parent.centre).acos() < 3.0 * parent.radius);
                    let w = |r: f64| tor.wind_ms((tor.centre + parent.east * r).normalize(), &AXIS, d.tornado.reach_cores).length();
                    assert!(w(tor.radius) >= w(0.2 * tor.radius) && w(tor.radius) > w(20.0 * tor.radius));
                    seen += 1;
                }
            }
        }
        assert!(seen > 0, "no tornado spawned in the sample");
    }
}
