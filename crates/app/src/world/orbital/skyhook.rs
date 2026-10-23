//! Rotating momentum-exchange skyhooks (rotovators). The hub is on a real
//! circular, equatorial, prograde orbit. The tether spins in the orbit plane, so
//! at the bottom of each turn the tip's orbital speed is cancelled and the tip
//! stops relative to the turning ground for an instant (the cusp of a cycloid).
//! Tether length is tuned so a whole number of turns fit in one synodic pass,
//! which puts a cusp over the same ground port on every pass.
//!
//! Time runs on the orbital clock in world seconds, and distances on the
//! compressed sky planet (see `megastructures::SKY_SCALE`).
use super::megastructures::{DAY_SECONDS, MU, PLANET_RADIUS_M, SKY_SCALE};
use crate::world::{DayNightCycle, hash_u32};
use glam::{DVec3, Vec3};
use std::f64::consts::{PI, TAU};

/// Planet spin on the orbital clock: one turn per world day.
pub const EARTH_RATE: f64 = TAU / DAY_SECONDS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    /// Single stage. Catches cargo pods off one guarded port.
    Courier,
    /// Single stage. Catches sealed containers and bulk freight off a port.
    Talon,
    /// Two stages. A magitech drop line descends from the tip during the dwell
    /// to lift whole districts.
    Ark,
}

impl Size {
    pub const ALL: [Size; 3] = [Size::Courier, Size::Talon, Size::Ark];
    pub fn name(self) -> &'static str {
        match self {
            Self::Courier => "Needle rotovator",
            Self::Talon => "Talon rotovator",
            Self::Ark => "Ark hook (two-stage)",
        }
    }
    /// Nominal tether length; the resonant solve adjusts it by a few percent.
    pub fn nominal_tether_m(self) -> f64 {
        (match self {
            Self::Courier => 160_000.0,
            Self::Talon => 420_000.0,
            Self::Ark => 1_150_000.0,
        }) / SKY_SCALE
    }
    /// Tip altitude at the cusp: just above the denser air.
    pub fn cusp_altitude_m(self) -> f64 {
        (match self {
            Self::Courier => 90_000.0,
            Self::Talon => 80_000.0,
            Self::Ark => 70_000.0,
        }) / SKY_SCALE
    }
    /// Second-stage drop line paid out from the tip during the dwell; 0 for
    /// single-stage hooks, whose pods climb to the tip on their own.
    pub fn drop_line_m(self) -> f64 {
        match self {
            Self::Ark => 70_000.0 / SKY_SCALE,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hook {
    pub size: Size,
    pub tether_m: f64,
    pub hub_radius_m: f64,
    /// Hub mean motion, rad/s.
    pub n: f64,
    /// Tether spin rate in the orbit plane, rad/s (same sense as the orbit).
    pub spin: f64,
    /// Inertial hub angle and tether angle at t = 0.
    theta0: f64,
    psi0: f64,
    pub port_lon: f64,
}

fn unit(key: u32) -> f64 {
    (hash_u32(key) >> 8) as f64 / 16_777_216.0
}

/// Unit vector for in-plane angle `a`; a = 0 is +Y (chart up at the origin) and
/// increasing angles move east, matching the observer frame's planet spin.
fn dir(a: f64) -> DVec3 {
    DVec3::new(a.sin(), a.cos(), 0.0)
}

impl Hook {
    /// Spin and orbit for tether `l`, set so the tip has zero velocity
    /// relative to the ground at the cusp.
    fn kinematics(size: Size, l: f64) -> (f64, f64, f64) {
        let tip = PLANET_RADIUS_M + size.cusp_altitude_m();
        let r = tip + l;
        let n = (MU / r.powi(3)).sqrt();
        (r, n, (n * r - EARTH_RATE * tip) / l)
    }

    /// Cusp turns per synodic pass (a real number); resonance needs an integer.
    fn turns(size: Size, l: f64) -> f64 {
        let (_, n, spin) = Self::kinematics(size, l);
        (spin - n) / (n - EARTH_RATE)
    }

    pub fn new(size: Size, port_lon: f64, pass_time: f64) -> Self {
        let l0 = size.nominal_tether_m();
        let k = Self::turns(size, l0).round().max(1.0);
        // turns() falls monotonically with length over the useful range.
        let (mut lo, mut hi) = (l0 * 0.5, l0 * 1.6);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if Self::turns(size, mid) > k { lo = mid } else { hi = mid }
        }
        let tether_m = 0.5 * (lo + hi);
        let (hub_radius_m, n, spin) = Self::kinematics(size, tether_m);
        // At pass_time the hub is over the port and the tether points down.
        let theta = EARTH_RATE * pass_time + port_lon;
        let (theta0, psi0) = (theta - n * pass_time, theta + PI - spin * pass_time);
        Self { size, tether_m, hub_radius_m, n, spin, theta0, psi0, port_lon }
    }

    pub fn hub(&self, t: f64) -> DVec3 {
        dir(self.theta0 + self.n * t) * self.hub_radius_m
    }
    /// Point at fraction `s` along the tether (0 hub, 1 tip), inertial frame.
    pub fn along(&self, t: f64, s: f64) -> DVec3 {
        self.hub(t) + dir(self.psi0 + self.spin * t) * self.tether_m * s
    }
    pub fn tip(&self, t: f64) -> DVec3 {
        self.along(t, 1.0)
    }
    pub fn turn_period(&self) -> f64 {
        TAU / (self.spin - self.n)
    }
    pub fn pass_period(&self) -> f64 {
        TAU / (self.n - EARTH_RATE)
    }
    /// Seconds until the next cusp anywhere on the planet.
    #[cfg(test)]
    pub fn next_cusp(&self, t: f64) -> f64 {
        let rel = (self.psi0 - self.theta0 + (self.spin - self.n) * t - PI).rem_euclid(TAU);
        (TAU - rel).rem_euclid(TAU) / (self.spin - self.n)
    }
    /// Seconds until the next cusp over the home port.
    pub fn next_port_cusp(&self, t: f64) -> f64 {
        let rel = (self.theta0 + (self.n - EARTH_RATE) * t - self.port_lon).rem_euclid(TAU);
        (TAU - rel).rem_euclid(TAU) / (self.n - EARTH_RATE)
    }
    /// Centripetal load on the tip relative to the hub, in g.
    #[cfg(test)]
    pub fn tip_load_g(&self) -> f64 {
        self.spin * self.spin * self.tether_m / 9.81
    }
}

/// The orbital clock in world seconds (DAY_SECONDS per world day). Planet spin,
/// orbits and the sun all advance on it, so their frames stay in step.
pub fn clock(day: &DayNightCycle) -> f64 {
    (day.day_index as f64 + day.time_of_day() as f64) * DAY_SECONDS
}

pub fn hooks(seed: u32) -> [Hook; 3] {
    Size::ALL.map(|size| {
        let key = hash_u32(seed ^ 0x5348_4b00 ^ size as u32);
        // Home ports sit within a few km of the spawn chart at the equator.
        let port_lon = (unit(key) - 0.5) * 6_000.0 / PLANET_RADIUS_M;
        Hook::new(size, port_lon, 600.0 + unit(key ^ 1) * 3_600.0)
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub hook: Hook,
    /// Hub and tip in the local chart (metres, relative to the chart origin).
    pub hub: Vec3,
    pub tip: Vec3,
    pub tip_altitude_m: f32,
    pub hub_elevation_deg: f32,
    pub next_port_cusp_s: f32,
}

impl Contact {
    /// Where the cargo pod is, if one is in flight. It rises from the ground port
    /// up a column over the port during `POD_LIFT_TURNS` of a tether turn before the cusp,
    /// rides the grapple for half a turn, then is hauled in to the hub. Pure
    /// function of the orbital clock, so nothing is saved and reloads agree.
    pub fn pod(&self, port: Vec3) -> Option<Vec3> {
        let turn = self.hook.turn_period() as f32;
        let to_next = self.next_port_cusp_s;
        let since = self.hook.pass_period() as f32 - to_next;
        let ease = |t: f32| t * t * (3.0 - 2.0 * t);
        let (lift, ride, haul) = (turn * POD_LIFT_TURNS, turn * 0.5, turn * 0.25);
        if to_next < lift {
            // The tip is far off to the side until the cusp, so the pod climbs a
            // column over the port to the cusp point, where the tip stops to meet it.
            let cusp = Vec3::new(port.x, self.hook.size.cusp_altitude_m() as f32, port.z);
            return Some(port.lerp(cusp, ease(1.0 - to_next / lift)));
        }
        if since < ride {
            return Some(self.tip);
        }
        if since < ride + haul {
            return Some(self.tip.lerp(self.hub, ease((since - ride) / haul)));
        }
        None
    }
}

/// Lead time of the pod's climb, as a fraction of one tether turn.
const POD_LIFT_TURNS: f32 = 0.3;

/// Rotate an inertial point into the planet-fixed frame at clock `t`.
fn fixed(p: DVec3, t: f64) -> DVec3 {
    let a = EARTH_RATE * t;
    DVec3::new(p.x * a.cos() - p.y * a.sin(), p.x * a.sin() + p.y * a.cos(), p.z)
}

/// Equatorial chart: x east, y up, z south; the origin is lat 0, lon 0.
fn local(p: DVec3, viewer: Vec3) -> Vec3 {
    let lon = viewer.x as f64 / PLANET_RADIUS_M;
    let lat = -(viewer.z as f64) / PLANET_RADIUS_M;
    let up = DVec3::new(lon.sin() * lat.cos(), lon.cos() * lat.cos(), -lat.sin());
    let east = DVec3::new(lon.cos(), -lon.sin(), 0.0);
    let south = DVec3::new(lon.sin() * lat.sin(), lon.cos() * lat.sin(), lat.cos());
    let d = p - up * (PLANET_RADIUS_M + viewer.y as f64);
    viewer + Vec3::new(d.dot(east) as f32, d.dot(up) as f32, d.dot(south) as f32)
}

pub fn contacts(seed: u32, day: &DayNightCycle, viewer: Vec3) -> Vec<Contact> {
    let t = clock(day);
    hooks(seed)
        .into_iter()
        .map(|hook| {
            let hub = local(fixed(hook.hub(t), t), viewer);
            let tip = local(fixed(hook.tip(t), t), viewer);
            let rel = hub - viewer;
            Contact {
                hook,
                hub,
                tip,
                tip_altitude_m: (hook.tip(t).length() - PLANET_RADIUS_M) as f32,
                hub_elevation_deg: (rel.y / rel.length()).clamp(-1.0, 1.0).asin().to_degrees(),
                next_port_cusp_s: hook.next_port_cusp(t) as f32,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ground_velocity(p: DVec3) -> DVec3 {
        DVec3::Z.cross(p) * -EARTH_RATE
    }

    #[test]
    fn tip_stops_over_port_with_resonant_repeat() {
        for size in Size::ALL {
            let pass = 1234.0;
            let hook = Hook::new(size, 0.0004, pass);
            for k in 0..3 {
                let t = pass + k as f64 * hook.pass_period();
                let tip = hook.tip(t);
                let alt = tip.length() - PLANET_RADIUS_M;
                assert!((alt - size.cusp_altitude_m()).abs() < 50.0, "{size:?} alt {alt}");
                let h = 1e-3;
                let v = (hook.tip(t + h) - hook.tip(t - h)) / (2.0 * h);
                let rel = (v - ground_velocity(tip)).length();
                assert!(rel < 1.0, "{size:?} pass {k}: tip moves {rel} m/s over ground");
                let ground = fixed(tip, t).normalize();
                assert!((ground - dir(0.0004)).length() < 1e-5, "{size:?} cusp drifted off port");
            }
        }
    }

    fn contact_at(to_next: f32) -> Contact {
        let hook = Hook::new(Size::Talon, 0.0, 0.0);
        Contact {
            hook,
            hub: Vec3::new(0.0, 60_000.0, 0.0),
            tip: Vec3::new(0.0, 20_000.0, 0.0),
            tip_altitude_m: 20_000.0,
            hub_elevation_deg: 90.0,
            next_port_cusp_s: to_next,
        }
    }

    #[test]
    fn pod_rises_to_the_tip_rides_and_is_hauled_to_the_hub() {
        let port = Vec3::new(0.0, 100.0, 0.0);
        let turn = Hook::new(Size::Talon, 0.0, 0.0).turn_period() as f32;
        let pass = Hook::new(Size::Talon, 0.0, 0.0).pass_period() as f32;
        let at = |to_next: f32| contact_at(to_next).pod(port);
        // Meets the tip at the cusp, with no jump across it.
        let before = at(1e-3).unwrap();
        let after = at(pass - 1e-3).unwrap();
        assert!((before - contact_at(0.0).tip).length() < 5.0, "{before}");
        assert!((after - before).length() < 5.0);
        // Starts at the port when the lift begins, and nothing flies between laps.
        assert!((at(turn * POD_LIFT_TURNS - 1e-3).unwrap() - port).length() < 50.0);
        assert!(at(pass * 0.5).is_none());
        // Hauled all the way in to the hub by the end of the ride and haul.
        let end = at(pass - turn * 0.75 + 1e-3).unwrap();
        assert!((end - contact_at(0.0).hub).length() < 50.0, "{end}");
    }

    #[test]
    fn rotation_and_passes_are_watchable_on_the_default_day() {
        let real = crate::world::DEFAULT_DAY_LENGTH_SECONDS as f64 / DAY_SECONDS;
        for size in Size::ALL {
            let hook = Hook::new(size, 0.0, 0.0);
            let turn = hook.turn_period();
            assert!((3.0..120.0).contains(&(turn * real)), "{size:?} turn {}s real", turn * real);
            let pass = hook.pass_period() * real;
            assert!((120.0..600.0).contains(&pass), "{size:?} pass {pass}s real");
            assert!((hook.tether_m / size.nominal_tether_m() - 1.0).abs() < 0.3);
            assert!(hook.tip_load_g() < 40.0, "{size:?} {} g", hook.tip_load_g());
            assert!(hook.next_port_cusp(1.0) > hook.pass_period() - 2.0);
            assert!(hook.next_cusp(0.0) < 1e-6 || hook.next_cusp(0.0) > turn - 1e-3);
        }
    }
}
