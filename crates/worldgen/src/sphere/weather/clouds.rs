//! Planet cloud density from a latitude climatology plus storm templates.
//!
//! Background cloud follows the circulation belts: a broken convective band
//! at the ITCZ, clear subtropics with cumulus fields and stratocumulus decks,
//! zonally streaked cloud on the storm tracks and thin polar cover. Inside a
//! storm the noise is sampled in the storm's own frame and wound along its
//! spiral, so texture follows the arms and travels with the storm.

use engine_core::planet_frame::PlanetAxis;
use glam::DVec3;

use super::storms::{self, DAY, Kind, Storm, Tornado};
use crate::value_noise3;

/// Flow-map cycle of the background drift: texture resets every period.
const DRIFT_PERIOD: f64 = 3.0 * DAY;
/// Flow-map cycle of a storm's rotation, short enough that the core never over-winds.
const SPIN_PERIOD: f64 = 2.0;

pub struct CloudSky {
    seed: u32,
    axis: PlanetAxis,
    storms: Vec<Storm>,
    tornadoes: Vec<Tornado>,
    time: f64,
    itcz: f64,
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn gauss(x: f64, w: f64) -> f64 {
    (-(x / w) * (x / w)).exp()
}

fn fbm(p: DVec3, octaves: u32, seed: u32) -> f64 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for i in 0..octaves {
        let q = p * freq;
        sum += f64::from(value_noise3(q.x as f32, q.y as f32, q.z as f32, seed.wrapping_add(i * 131))) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm
}

/// Rotates `p` about unit `axis` by `angle`.
fn rotate(p: DVec3, axis: DVec3, angle: f64) -> DVec3 {
    let (s, c) = angle.sin_cos();
    p * c + axis.cross(p) * s + axis * axis.dot(p) * (1.0 - c)
}

/// Zonal drift in radians per model unit: trades, westerlies, polar easterlies.
fn zonal(lat: f64) -> f64 {
    let a = lat.abs().to_degrees();
    0.022 * (gauss(a - 45.0, 14.0) - 0.45 * gauss(a - 12.0, 12.0) - 0.2 * gauss(a - 78.0, 8.0))
}

impl CloudSky {
    pub fn at(seed: u32, axis: PlanetAxis, year_fraction: f64, time: f64) -> Self {
        let itcz = 0.45 * axis.subsolar_latitude(year_fraction);
        let storms = storms::active(seed, &axis, time);
        let tornadoes = storms::tornadoes(seed, &axis, &storms, time, engine_content::climate::storm_dials());
        Self { seed, axis, storms, tornadoes, time, itcz }
    }

    /// Tornadoes alive at this sky's time.
    pub fn tornadoes(&self) -> &[Tornado] {
        &self.tornadoes
    }

    /// Tornadoes whose wind reaches `dir`.
    pub fn tornadoes_near(&self, dir: DVec3) -> impl Iterator<Item = &Tornado> {
        let reach = engine_content::climate::storm_dials().tornado.reach_cores;
        self.tornadoes.iter().filter(move |t| (dir - t.centre).length() <= t.radius * reach)
    }

    pub fn storms(&self) -> &[Storm] {
        &self.storms
    }

    /// Surface wind at a unit direction as a body-frame tangent in radians per model unit.
    ///
    /// The advection flow plus each tropical cyclone's Holland vortex and each
    /// tornado's Rankine vortex, scaled by `flow_per_ms` from `assets/climate.ron`.
    pub fn wind(&self, p: DVec3) -> DVec3 {
        let dials = engine_content::climate::storm_dials();
        let mut v = self.advection(p);
        for s in self.storms.iter().filter(|s| s.kind == Kind::Tropical) {
            v += storms::cyclone_wind(s, p, dials);
        }
        for t in &self.tornadoes {
            v += t.wind_ms(p, &self.axis, dials.tornado.reach_cores) * dials.flow_per_ms;
        }
        v - p * p.dot(v)
    }

    /// Convective (cumulonimbus) share at a unit direction in `0..=1`, from the
    /// same storm catalogue the shell draws: a tropical cyclone by its Holland
    /// wind against the hurricane threshold (calm eye, peak at the eyewall), a
    /// frontal low by its comma head, and the parent cell of any tornado.
    pub fn storm_share(&self, p: DVec3) -> f64 {
        let dials = engine_content::climate::storm_dials();
        let mut share = 0.0f64;
        for s in &self.storms {
            let Some((x, y)) = s.local(p) else { continue };
            let r_local = (x * x + y * y).sqrt();
            let v = match s.kind {
                Kind::Tropical => (s.holland_ms(r_local * s.radius) / dials.tropical.category_ms[0]).min(1.0)
                    * (1.0 - smooth(2.5, 4.0, r_local)),
                Kind::Frontal => 0.8 * s.strength * gauss(r_local, 0.9),
                Kind::High => 0.0,
            };
            share = share.max(v);
        }
        share.max(self.tornado_share(p)).clamp(0.0, 1.0)
    }

    /// Closeness to the nearest tornado in `0..=1`: one at a core, zero at its wind reach.
    pub fn tornado_share(&self, p: DVec3) -> f64 {
        let reach = engine_content::climate::storm_dials().tornado.reach_cores;
        self.tornadoes_near(p)
            .map(|t| 1.0 - (p - t.centre).length() / (t.radius * reach))
            .fold(0.0, f64::max)
            .clamp(0.0, 1.0)
    }

    /// Cloud-texture flow at a unit direction as a body-frame tangent in radians per model unit.
    ///
    /// The same zonal flow and storm rotation that move the texture in `density`, so a
    /// renderer can advect a baked field between bakes and land on the next one.
    pub fn advection(&self, p: DVec3) -> DVec3 {
        let north = self.axis.north;
        let mut v = north.cross(p) * zonal(self.axis.latitude(p));
        for s in &self.storms {
            let Some((x, y)) = s.local(p) else { continue };
            let r2 = x * x + y * y;
            if r2 > 16.0 {
                continue;
            }
            // Inside a storm the template rides the track; only its texture turns about the core.
            let inside = 1.0 - smooth(4.0, 12.0, r2);
            let omega = s.spin / (1.0 + r2);
            let storm = s.drift + (s.east * -y + s.pole * x) * (omega * s.radius);
            v = v.lerp(storm, inside * s.strength.max(0.3));
        }
        v - p * p.dot(v)
    }

    /// Cloud optical density in `0..=1` at a unit direction.
    pub fn density(&self, p: DVec3) -> f32 {
        let lat = self.axis.latitude(p);
        let mut d = self.background(p, lat);
        let mut storm = 0.0f64;
        let mut clear = 0.0f64;
        for s in &self.storms {
            let Some((x, y)) = s.local(p) else { continue };
            let r2 = x * x + y * y;
            if r2 > 16.0 {
                continue;
            }
            let (cloud, dry) = match s.kind {
                Kind::Frontal => self.frontal(s, x, y),
                Kind::Tropical => self.tropical(s, x, y),
                Kind::High => self.high(s, x, y),
            };
            let fade = s.strength * (1.0 - smooth(6.0, 16.0, r2));
            storm = storm.max(cloud * fade);
            clear = clear.max(dry * fade);
        }
        d *= 1.0 - clear;
        d = d.max(storm * (1.0 - clear * 0.65));
        d.clamp(0.0, 1.0) as f32
    }

    fn background(&self, p: DVec3, lat: f64) -> f64 {
        let a = lat.abs().to_degrees();
        let itcz = gauss((lat - self.itcz).to_degrees(), 6.0);
        let track = gauss(a - 52.0, 13.0);
        let subtropic = gauss(a - 24.0, 9.0);
        let polar = smooth(62.0, 85.0, a);
        // Zonal streaks: compress the texture across latitude on the storm tracks.
        let squash = 1.0 + 2.2 * track + 0.8 * subtropic;
        let north = self.axis.north;
        let drift = |phase: f64| {
            let q = rotate(p, north, -zonal(lat) * phase * DRIFT_PERIOD);
            q + north * (q.dot(north) * (squash - 1.0))
        };
        // Two flow-map phases half a cycle apart, so the drift never shears without bound.
        let cycle = self.time / DRIFT_PERIOD;
        let (fa, fb) = (cycle.fract(), (cycle + 0.5).fract());
        let ga = (cycle.floor() as i64) as u32;
        let gb = ((cycle + 0.5).floor() as i64) as u32;
        let wa = 1.0 - (2.0 * fa - 1.0).abs();
        let cellular = subtropic + 0.4 * itcz > 0.02;
        let sample = |q: DVec3, generation: u32| {
            let s = self.seed ^ generation.wrapping_mul(0x9E37_79B9);
            // Domain warp bends blobs into the curled sheets seen from orbit.
            let warp = DVec3::new(fbm(q * 4.0, 1, s ^ 1), fbm(q * 4.0, 1, s ^ 2), fbm(q * 4.0, 1, s ^ 3)) - 0.5;
            let q = q + warp * 0.22;
            let large = fbm(q * 3.0, 3, s);
            let fine = fbm(q * 22.0, 3, s ^ 0x55);
            let cells = if cellular { 1.0 - (2.0 * fbm(q * 48.0, 2, s ^ 0xAA) - 1.0).abs() } else { 0.0 };
            (large, fine, cells)
        };
        let (la, fa_, ca) = sample(drift(fa), ga);
        let (lb, fb_, cb) = sample(drift(fb), gb);
        let large = la * wa + lb * (1.0 - wa);
        let fine = fa_ * wa + fb_ * (1.0 - wa);
        let cells = ca * wa + cb * (1.0 - wa);
        // Stratiform cover rises with the climatology; convection clusters at the ITCZ.
        // Lows carry most storm-track cloud, so the belt itself stays broken.
        let mean = 0.26 + 0.2 * track + 0.18 * polar - 0.08 * subtropic + 0.1 * itcz;
        let body = 0.78 * large + 0.22 * fine;
        let strat = smooth(0.6 - 0.5 * mean, 0.68 - 0.45 * mean, body) * (0.5 + 0.35 * track + 0.25 * polar) * (0.75 + 0.7 * (fine - 0.5));
        let convective = itcz * smooth(0.44, 0.58, large) * smooth(0.42, 0.58, fine) * 1.1;
        let cumulus = (subtropic + 0.4 * itcz) * smooth(0.78, 0.9, cells) * smooth(0.45, 0.6, large) * 0.28;
        let deck = subtropic * smooth(0.62, 0.72, large) * (0.4 + 0.5 * fine);
        strat.max(convective).max(cumulus).max(deck)
    }

    /// Storm-frame fbm wound along the spiral `theta + wind * ln(r)` and turned with the core.
    ///
    /// The turn is `spin / (1 + r^2)` times the time since a flow-map phase began, the angular
    /// velocity `wind` reports; two phases half a cycle apart hide each reset.
    fn storm_noise(s: &Storm, x: f64, y: f64, wind: f64, freq: f64) -> f64 {
        let r2 = x * x + y * y;
        let r = r2.sqrt().max(0.02);
        let theta = y.atan2(x) + wind * r.ln() * gauss(r, 2.2);
        let omega = s.spin / (1.0 + r2);
        let cycle = s.elapsed / SPIN_PERIOD;
        let phase = |offset: f64| {
            let c = cycle + offset;
            let turned = theta - omega * c.fract() * SPIN_PERIOD;
            let q = DVec3::new(r * turned.cos(), r * turned.sin(), s.age * 0.6) * freq;
            fbm(q, 4, s.key ^ (c.floor() as i64 as u32).wrapping_mul(0x2545_F491))
        };
        let w = 1.0 - (2.0 * cycle.fract() - 1.0).abs();
        phase(0.0) * w + phase(0.5) * (1.0 - w)
    }

    /// Comma head, warm-front shield, cold front, dry slot and post-frontal cumulus.
    fn frontal(&self, s: &Storm, x: f64, y: f64) -> (f64, f64) {
        let r = (x * x + y * y).sqrt();
        let wrap = 0.7 + 1.5 * s.age;
        let n = Self::storm_noise(s, x, y, wrap, 2.6);
        let theta = y.atan2(x);
        // One broad arm: the comma head wrapping cloud from the shield round the west of the low.
        let arm = 0.5 + 0.5 * (theta - 0.6 + wrap * r.max(0.05).ln()).cos();
        let head = smooth(0.25, 0.75, arm) * gauss(r, 1.1) + gauss(r, 0.45) * (1.0 - s.age) * 0.6;
        // Shield ahead of the warm front, poleward of it.
        let shield = gauss((x - 1.0) / 1.5, 1.0) * gauss((y - 0.5) / 1.0, 1.0) * smooth(-0.7, 0.2, y + 0.3 * x);
        // Cold front: a band trailing equatorward from the low and bending west, x = f(y).
        let t = ((-0.15 - y) / 2.7).clamp(0.0, 1.0);
        let fx = 0.35 * t - 1.25 * t * t;
        let along = smooth(-0.35, -0.1, -y) * (1.0 - smooth(2.6, 3.1, -y));
        let width = 0.18 + 0.12 * t;
        let wobble = 0.12 * (Self::storm_noise(s, 0.0, y, 0.0, 2.2) - 0.5);
        let dx = x - fx - wobble;
        let front = gauss(dx, width) * along * (1.0 - 0.5 * t);
        let west_of = smooth(0.1, 0.4, -dx) * (1.0 - smooth(1.4, 2.8, -dx)) * along * (1.0 - 0.4 * t);
        let dry_slot = west_of * gauss(r - 1.0, 0.65) * smooth(0.0, 0.4, -y);
        let cells = 1.0 - (2.0 * Self::storm_noise(s, x, y, wrap * 0.3, 16.0) - 1.0).abs();
        let cumulus = west_of * (1.0 - dry_slot) * smooth(0.62, 0.86, cells) * 0.35;
        let texture = 0.65 + 1.1 * (n - 0.5);
        let cloud = (head.max(shield * 0.9).max(front) * texture * 1.45).max(cumulus);
        (cloud.clamp(0.0, 1.0) * (1.0 - dry_slot), dry_slot.max(west_of * 0.45))
    }

    /// Eye, eyewall, lopsided central overcast, broken spiral rain bands and outflow cirrus.
    ///
    /// A ring with a hole reads as a stamp. The overcast is pushed downshear,
    /// its rim follows wound noise, the bands are log-spiral arms of unequal
    /// length that break into convective clumps, and dry moats separate them.
    fn tropical(&self, s: &Storm, x: f64, y: f64) -> (f64, f64) {
        let r = (x * x + y * y).sqrt();
        let theta = y.atan2(x);
        let wrap = 2.4;
        let h = |salt: u32| f64::from(engine_core::hash::hash01(s.key ^ salt));
        let n = Self::storm_noise(s, x, y, wrap, 3.5);
        let fine = Self::storm_noise(s, x, y, wrap * 1.15, 11.0);
        // Shear leans the cloud mass to one side of the centre.
        let lean = h(0x51) * std::f64::consts::TAU;
        let rr = r / (1.0 + (0.16 + 0.14 * h(0x52)) * (theta - lean).cos());
        // A young or dying storm has a ragged, filled centre; the eye opens with strength.
        let open = smooth(0.45, 0.8, s.strength);
        let eye = 0.085 + 0.07 * (1.0 - s.strength);
        // The eye is an oval with a scalloped wall, not a drilled hole.
        let re = r * (1.0 + 0.7 * (fine - 0.5) + 0.3 * (n - 0.5) + 0.14 * (2.0 * (theta - lean)).cos());
        let rim = 0.5 + 0.2 * s.strength + 0.5 * (n - 0.5) + 0.16 * (fine - 0.5);
        let overcast = (1.0 - smooth(rim * 0.72, rim * 1.22, rr)) * (0.78 + 0.5 * (fine - 0.5));
        let eyewall = gauss(re - (eye + 0.06), 0.06) * (0.55 + 0.3 * open) * (0.7 + 0.6 * (fine - 0.5) + 0.3);
        let hollow = 1.0 - open * (1.0 - smooth(eye * 0.55, eye + 0.05, re));
        let core = overcast.max(eyewall) * hollow;

        // Spiral coordinate: constant along a log-spiral arm.
        let spiral = theta + wrap * r.max(0.04).ln() + h(0x53) * std::f64::consts::TAU;
        let principal = smooth(0.52, 0.88, 0.5 + 0.5 * (2.0 * spiral).cos());
        // A third, thinner arm off the pair keeps the storm from mirror symmetry.
        let stray = smooth(0.8, 0.97, 0.5 + 0.5 * (spiral + 1.9).cos()) * smooth(0.9, 1.5, r) * 0.7;
        let band = principal.max(stray);
        let reach = 2.3 + 0.9 * h(0x54);
        let envelope = smooth(0.25, 0.55, rr) * (-(r - 0.6).max(0.0) / 1.5).exp() * (1.0 - smooth(reach, reach + 0.8, r));
        // Bands are lines of cells, denser toward the core, with gaps along the arm.
        let clumps = smooth(0.34, 0.6, n) * (0.6 + 0.9 * (fine - 0.5));
        let arms = band * envelope * clumps * 1.25;

        // Thin outflow shield, wound the other way aloft.
        let streak = Self::storm_noise(s, x, y, -0.9, 5.0);
        let outflow = 0.2 * (-(rr - 0.5).max(0.0) / 0.9).exp() * smooth(0.4, 0.62, streak) * (1.0 - smooth(2.4, 3.2, r));

        let cloud = core.max(arms).max(outflow);
        let clear_eye = open * (1.0 - smooth(eye * 0.5, eye + 0.03, re));
        let moat = envelope * (1.0 - band) * 0.6;
        (cloud.clamp(0.0, 1.0), clear_eye.max(moat).max(gauss(r - 2.9, 0.8) * 0.3))
    }

    fn high(&self, s: &Storm, x: f64, y: f64) -> (f64, f64) {
        let r = (x * x + y * y).sqrt();
        let dry = gauss(r, 1.15) * 0.92;
        // A high is predominantly clearing, not a reversed hurricane. Broken
        // stratiform filaments turn clockwise along the outer subsidence boundary.
        let n = Self::storm_noise(s, x, y, -1.6, 6.0);
        let edge = gauss(r - 1.5, 0.45) * smooth(-0.6, 0.8, y);
        (edge * smooth(0.38, 0.7, n) * 0.44, dry)
    }
}
