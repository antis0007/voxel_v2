//! Airborne tracking for the movement check: hover detection and fall height.
//!
//! The server decides when the body left and regained support from its own cells,
//! so a client cannot hover by repeating a pose or skip a fall by claiming ground.

/// Height a pose may sit above the gravity curve, in voxels.
pub(super) const SLACK: f32 = 0.5;
/// Farthest one float correction moves the body down; later ones continue.
pub(super) const MAX_DROP: f32 = 16.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Air {
    /// Highest feet height in the current arc.
    peak: f32,
    /// Seconds since the arc started: support, wall contact or impulse.
    since: f32,
    /// Highest feet height since the body last stood on support.
    top: f32,
    airborne: bool,
    landing: Option<f32>,
}

impl Air {
    pub(super) fn new(y: f32) -> Self {
        Self { peak: y, since: 0.0, top: y, airborne: false, landing: None }
    }

    pub(super) fn advance(&mut self, dt: f32) {
        // Capped so a client withholding its ack cannot grow the required drop without limit.
        self.since = (self.since + dt).min(600.0);
    }

    /// Highest feet height the body may report now.
    pub(super) fn ceiling(&self, gravity: f32, terminal: f32, hang: f32) -> f32 {
        self.peak - required_drop(self.since, gravity, terminal, hang) + SLACK
    }

    /// Starts a new arc at `y`: support, wall contact or an upward impulse.
    pub(super) fn restart(&mut self, y: f32) {
        self.peak = y;
        self.since = 0.0;
    }

    pub(super) fn accept(&mut self, y: f32, grounded: bool, touching: bool) {
        if grounded {
            if self.airborne && self.top > y {
                self.landing = Some(self.top - y);
            }
            self.airborne = false;
            self.top = y;
            self.restart(y);
            return;
        }
        self.airborne = true;
        self.top = self.top.max(y);
        if touching {
            self.restart(y);
        } else {
            // Rising lifts the peak but keeps the clock, so creeping upward by tiny
            // steps cannot refresh the hang window.
            self.peak = self.peak.max(y);
        }
    }

    /// Moves every stored height by `dh` when the frame of measurement changes.
    pub(super) fn shift(&mut self, dh: f32) {
        self.peak += dh;
        self.top += dh;
    }

    pub(super) fn take_landing(&mut self) -> Option<f32> {
        self.landing.take()
    }
}

/// Least drop below the peak after `t` seconds: none for `hang`, then free fall
/// whose speed stops growing at `terminal`.
pub(super) fn required_drop(t: f32, gravity: f32, terminal: f32, hang: f32) -> f32 {
    let s = (t - hang).max(0.0);
    let reach = terminal / gravity.max(f32::EPSILON);
    if s <= reach {
        0.5 * gravity * s * s
    } else {
        0.5 * terminal * reach + terminal * (s - reach)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drop_curve_holds_then_falls_then_caps_at_terminal_speed() {
        assert_eq!(required_drop(0.2, 20.0, 40.0, 0.25), 0.0);
        assert!((required_drop(1.25, 20.0, 40.0, 0.25) - 10.0).abs() < 1e-4);
        // Terminal speed is reached after 2 s of fall; each later second adds 40.
        let at = |t| required_drop(t, 20.0, 40.0, 0.25);
        assert!((at(4.25) - at(3.25) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn landing_reports_the_height_from_the_top_of_the_arc() {
        let mut air = Air::new(0.0);
        for y in [1.0, 2.0, 1.5, -6.0] {
            air.accept(y, false, false);
        }
        air.accept(-8.0, true, false);
        assert_eq!(air.take_landing(), Some(10.0));
        assert_eq!(air.take_landing(), None);
    }
}
