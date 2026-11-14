//! Express lift trajectory: a bounded-jerk S curve that covers a route in a
//! fixed time, and the cabin telemetry derived from it.
//!
//! The motion time `T` is chosen first; the drive is then rated for what that
//! route needs: `a_max = 5·D/T²`, `v_max = 2·D/T`. Five phases with constant
//! jerk, ramps one tenth of `T` long:
//!
//! | phase            | span         | jerk  |
//! |------------------|--------------|-------|
//! | ramp to +a_max   | 0 .. T/10    | +J    |
//! | hold +a_max      | T/10 .. 4T/10| 0     |
//! | swing to −a_max  | 4T/10 .. 6T/10| −J   |
//! | hold −a_max      | 6T/10 .. 9T/10| 0     |
//! | ramp to 0        | 9T/10 .. T   | +J    |
//!
//! Acceleration is continuous, velocity peaks at `T/2` exactly half way along
//! the route, and both ends are at rest. Every evaluation starts from the
//! precomputed phase boundaries, so clients never integrate and never drift.

/// Standard gravity used for g readouts, m/s².
pub const G0: f64 = 9.806_65;

/// One lift route and its motion profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// Route length, metres.
    pub length_m: f64,
    /// Time in motion, seconds.
    pub motion_s: f64,
    /// Door and release time at each end, seconds.
    pub dwell_s: f64,
}

/// Cabin state along the route at one instant.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    /// Distance along the route from the departure terminal, metres.
    pub position_m: f64,
    /// Signed speed along the route, m/s (positive toward the destination).
    pub velocity_mps: f64,
    /// Signed drive acceleration along the route, m/s².
    pub acceleration_mps2: f64,
}

/// What the cabin is doing, for the panel and the drive sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Doors closing, carriage locked, before motion starts.
    Departing,
    Accelerating,
    /// Drive swinging through zero at the route midpoint.
    Coasting,
    Braking,
    /// At rest at the destination, doors releasing.
    Arriving,
    Docked,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Departing => "Departing",
            Self::Accelerating => "Accelerating",
            Self::Coasting => "Coasting",
            Self::Braking => "Braking",
            Self::Arriving => "Arriving",
            Self::Docked => "Docked",
        }
    }
}

/// State at a phase boundary: time, position, velocity, acceleration.
#[derive(Clone, Copy, Debug)]
struct Knot {
    t: f64,
    x: f64,
    v: f64,
    a: f64,
    /// Jerk applied from this knot to the next.
    j: f64,
}

impl Profile {
    /// Two-minute express: 110 s in motion with 5 s of doors at each end.
    pub fn express(length_m: f64) -> Self {
        Self { length_m, motion_s: 110.0, dwell_s: 5.0 }
    }

    /// Whole trip including both dwells, seconds.
    pub fn total_s(&self) -> f64 {
        self.motion_s + 2.0 * self.dwell_s
    }

    /// Peak drive acceleration, m/s².
    pub fn peak_acceleration(&self) -> f64 {
        5.0 * self.length_m / (self.motion_s * self.motion_s)
    }

    /// Peak speed, reached at the route midpoint, m/s.
    pub fn peak_speed(&self) -> f64 {
        2.0 * self.length_m / self.motion_s
    }

    fn ramp_s(&self) -> f64 {
        self.motion_s * 0.1
    }

    /// Phase boundaries in motion time (0 at the start of motion).
    fn knots(&self) -> [Knot; 6] {
        let r = self.ramp_s();
        let a = self.peak_acceleration();
        let j = a / r;
        let mut knots = [Knot { t: 0.0, x: 0.0, v: 0.0, a: 0.0, j }; 6];
        let bounds = [(r, 0.0), (3.0 * r, -j), (2.0 * r, 0.0), (3.0 * r, j), (r, 0.0)];
        for (i, &(span, next_jerk)) in bounds.iter().enumerate() {
            let k = knots[i];
            let next = step(k, span);
            knots[i + 1] = Knot { j: next_jerk, ..next };
        }
        knots
    }

    /// Cabin state at `trip_s` seconds after departure activation.
    /// Before motion the cabin is at the terminal; after motion it is at the
    /// destination at rest.
    pub fn sample(&self, trip_s: f64) -> Sample {
        let t = trip_s - self.dwell_s;
        if t.is_nan() || t <= 0.0 {
            return Sample::default();
        }
        if t >= self.motion_s {
            return Sample { position_m: self.length_m, ..Sample::default() };
        }
        let knots = self.knots();
        let i = knots[1..].iter().take_while(|k| k.t <= t).count();
        let k = step(knots[i], t - knots[i].t);
        Sample { position_m: k.x, velocity_mps: k.v, acceleration_mps2: k.a }
    }

    pub fn phase(&self, trip_s: f64) -> Phase {
        let t = trip_s - self.dwell_s;
        let r = self.ramp_s();
        if trip_s < 0.0 || t < 0.0 {
            Phase::Departing
        } else if t < 4.0 * r {
            Phase::Accelerating
        } else if t < 6.0 * r {
            Phase::Coasting
        } else if t < self.motion_s {
            Phase::Braking
        } else if trip_s < self.total_s() {
            Phase::Arriving
        } else {
            Phase::Docked
        }
    }

    /// Seconds until the exit becomes usable, including the final door release.
    pub fn arrival_s(&self, trip_s: f64) -> f64 {
        (self.total_s() - trip_s).max(0.0)
    }
}

/// Constant-jerk advance of one knot by `dt`.
fn step(k: Knot, dt: f64) -> Knot {
    Knot {
        t: k.t + dt,
        x: k.x + k.v * dt + 0.5 * k.a * dt * dt + k.j * dt * dt * dt / 6.0,
        v: k.v + k.a * dt + 0.5 * k.j * dt * dt,
        a: k.a + k.j * dt,
        j: k.j,
    }
}

/// Cabin gravity the compensating field holds, in g, by phase: a touch heavy
/// on departure, light under braking, exact when docked. Blended with the
/// drive so the cue follows the motion rather than snapping at phase edges.
pub fn cabin_gravity_g(profile: &Profile, sample: &Sample) -> f64 {
    let peak = profile.peak_acceleration();
    if peak <= 0.0 {
        return 1.0;
    }
    1.0 + 0.1 * (sample.acceleration_mps2 / peak).clamp(-1.0, 1.0)
}

/// Readings for the cabin panel, all derived from the same trajectory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Telemetry {
    pub phase: Phase,
    /// Speed relative to the guide, m/s.
    pub speed_mps: f64,
    /// Signed drive acceleration, g.
    pub drive_g: f64,
    /// Effective field inside the compartment, g.
    pub cabin_g: f64,
    /// Height above the planet reference surface, metres.
    pub altitude_m: f64,
    /// Seconds until the exit opens.
    pub arrival_s: f64,
    /// Fraction of the route covered, 0 at departure.
    pub progress: f64,
}

/// Telemetry at `trip_s`. `base_altitude_m` is the departure terminal's
/// altitude; `ascending` flips the altitude reading for a descent.
pub fn telemetry(profile: &Profile, trip_s: f64, base_altitude_m: f64, ascending: bool) -> Telemetry {
    let s = profile.sample(trip_s);
    let along = if ascending { s.position_m } else { profile.length_m - s.position_m };
    let progress = if profile.length_m > 0.0 { s.position_m / profile.length_m } else { 1.0 };
    Telemetry {
        phase: profile.phase(trip_s),
        speed_mps: s.velocity_mps.abs(),
        drive_g: s.acceleration_mps2 / G0,
        cabin_g: cabin_gravity_g(profile, &s),
        altitude_m: base_altitude_m + along,
        arrival_s: profile.arrival_s(trip_s),
        progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTE: Profile = Profile { length_m: 200_000.0, motion_s: 110.0, dwell_s: 5.0 };

    #[test]
    fn ratings_match_the_closed_form() {
        assert!((ROUTE.peak_acceleration() - 5.0 * 200_000.0 / (110.0 * 110.0)).abs() < 1e-9);
        assert!((ROUTE.peak_speed() - 2.0 * 200_000.0 / 110.0).abs() < 1e-9);
        // 200 km in 110 s: 8.4 g and 3.64 km/s.
        assert!((ROUTE.peak_acceleration() / G0 - 8.43).abs() < 0.01);
        assert!((ROUTE.peak_speed() - 3636.4).abs() < 0.1);
    }

    #[test]
    fn ends_are_at_rest_at_both_terminals() {
        let start = ROUTE.sample(ROUTE.dwell_s);
        assert_eq!(start, Sample::default());
        let end = ROUTE.sample(ROUTE.dwell_s + ROUTE.motion_s);
        assert!((end.position_m - ROUTE.length_m).abs() < 1e-6);
        assert_eq!(end.velocity_mps, 0.0);
        assert_eq!(end.acceleration_mps2, 0.0);
        // Just before the end the integrated state has converged on the same place.
        let near = ROUTE.sample(ROUTE.dwell_s + ROUTE.motion_s - 1e-6);
        assert!((near.position_m - ROUTE.length_m).abs() < 1e-3, "{near:?}");
        assert!(near.velocity_mps.abs() < 1e-3 && near.acceleration_mps2.abs() < 1e-3);
    }

    #[test]
    fn midpoint_is_half_way_at_peak_speed_with_zero_drive() {
        let mid = ROUTE.sample(ROUTE.dwell_s + ROUTE.motion_s * 0.5);
        assert!((mid.position_m - ROUTE.length_m * 0.5).abs() < 1e-6, "{mid:?}");
        assert!((mid.velocity_mps - ROUTE.peak_speed()).abs() < 1e-6);
        assert!(mid.acceleration_mps2.abs() < 1e-9);
        assert_eq!(ROUTE.phase(ROUTE.dwell_s + 55.0), Phase::Coasting);
    }

    #[test]
    fn holds_reach_the_rated_drive_and_the_braking_mirror() {
        let a = ROUTE.peak_acceleration();
        for t in [11.0, 20.0, 44.0] {
            assert!((ROUTE.sample(5.0 + t).acceleration_mps2 - a).abs() < 1e-9);
        }
        for t in [66.0, 80.0, 99.0] {
            assert!((ROUTE.sample(5.0 + t).acceleration_mps2 + a).abs() < 1e-9);
        }
    }

    #[test]
    fn acceleration_is_continuous_and_jerk_bounded() {
        let a = ROUTE.peak_acceleration();
        let j = a / 11.0;
        let dt = 0.01;
        let mut prev = ROUTE.sample(5.0);
        let mut t = 5.0 + dt;
        while t < 115.0 {
            let s = ROUTE.sample(t);
            let jerk = (s.acceleration_mps2 - prev.acceleration_mps2) / dt;
            assert!(jerk.abs() <= j * 1.001, "jerk {jerk} at {t}");
            assert!(s.position_m >= prev.position_m - 1e-9, "went backwards at {t}");
            prev = s;
            t += dt;
        }
    }

    #[test]
    fn phases_follow_the_timeline() {
        let p = |t| ROUTE.phase(t);
        assert_eq!(p(0.0), Phase::Departing);
        assert_eq!(p(4.99), Phase::Departing);
        assert_eq!(p(5.0), Phase::Accelerating);
        assert_eq!(p(48.9), Phase::Accelerating);
        assert_eq!(p(49.1), Phase::Coasting);
        assert_eq!(p(71.1), Phase::Braking);
        assert_eq!(p(114.9), Phase::Braking);
        assert_eq!(p(115.0), Phase::Arriving);
        assert_eq!(p(120.0), Phase::Docked);
        assert_eq!(ROUTE.arrival_s(0.0), 120.0);
        assert_eq!(ROUTE.arrival_s(130.0), 0.0);
    }

    #[test]
    fn cabin_gravity_stays_walkable_while_the_drive_pulls_eight_g() {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        let mut t = 0.0;
        while t <= 120.0 {
            let r = telemetry(&ROUTE, t, 0.0, true);
            lo = lo.min(r.cabin_g);
            hi = hi.max(r.cabin_g);
            t += 0.25;
        }
        assert!((lo - 0.9).abs() < 1e-9 && (hi - 1.1).abs() < 1e-9, "{lo} {hi}");
        let docked = telemetry(&ROUTE, 125.0, 0.0, true);
        assert_eq!(docked.cabin_g, 1.0);
        assert_eq!(docked.phase, Phase::Docked);
        assert!(telemetry(&ROUTE, 20.0, 0.0, true).drive_g > 8.0);
    }

    #[test]
    fn descent_altitude_counts_down_from_the_top() {
        let top = telemetry(&ROUTE, 0.0, 100.0, false);
        assert!((top.altitude_m - 200_100.0).abs() < 1e-6);
        let down = telemetry(&ROUTE, 125.0, 100.0, false);
        assert!((down.altitude_m - 100.0).abs() < 1e-6);
        assert!((down.progress - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_short_route_in_two_minutes_needs_under_one_g() {
        let p = Profile::express(18_000.0);
        assert!(p.peak_acceleration() / G0 < 1.0);
        assert!((p.total_s() - 120.0).abs() < 1e-12);
    }
}
