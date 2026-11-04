//! Per-surface footstep recipes.
use super::{Contact, Surface};

/// How one surface's step is built. Crunch is a cloud of noise grains, each
/// band-passed at a random centre; the body is low-passed noise; modes are
/// struck resonances. Everything ends under a low-pass ceiling, because raw
/// noise up to Nyquist is what made the old steps read as fizz.
pub(super) struct Recipe {
    pub(super) length: f32,
    /// Grain centres are drawn log-uniformly from this band, in hertz.
    pub(super) band: [f32; 2],
    pub(super) q: f32,
    /// Grains per second at the heel strike.
    pub(super) grains: f32,
    pub(super) grain_s: f32,
    /// Decay time of the heel grain cloud; the toe roll is a softer second cloud.
    pub(super) spread: f32,
    pub(super) toe: Option<f32>,
    pub(super) crunch: f32,
    pub(super) body_hz: f32,
    pub(super) body: f32,
    pub(super) body_s: f32,
    /// Struck resonances: hertz, Q, gain.
    pub(super) modes: &'static [(f32, f32, f32)],
    pub(super) ceiling: f32,
    /// Water only: a swept band of noise and two closing bubbles.
    pub(super) swish: f32,
}

pub(super) fn recipe(surface: Surface) -> Recipe {
    let base = Recipe {
        length: 0.16,
        band: [1000.0, 3000.0],
        q: 1.2,
        grains: 2000.0,
        grain_s: 0.0008,
        spread: 0.02,
        toe: Some(0.055),
        crunch: 3.0,
        body_hz: 260.0,
        body: 6.0,
        body_s: 0.02,
        modes: &[],
        ceiling: 4000.0,
        swish: 0.0,
    };
    match surface {
        Surface::Stone => Recipe {
            length: 0.14,
            band: [1400.0, 3600.0],
            q: 1.4,
            grains: 1800.0,
            grain_s: 0.0007,
            spread: 0.012,
            toe: Some(0.045),
            body_hz: 420.0,
            body_s: 0.012,
            modes: &[(310.0, 6.0, 0.10)],
            ceiling: 5000.0,
            ..base
        },
        Surface::Soil => Recipe {
            band: [500.0, 1800.0],
            q: 1.0,
            grains: 2200.0,
            grain_s: 0.0012,
            spread: 0.025,
            toe: Some(0.06),
            body: 8.0,
            body_s: 0.025,
            ceiling: 3000.0,
            ..base
        },
        Surface::Grass => Recipe {
            length: 0.20,
            band: [1400.0, 4000.0],
            q: 1.8,
            grains: 2600.0,
            grain_s: 0.0009,
            spread: 0.045,
            toe: Some(0.07),
            crunch: 3.2,
            body_hz: 280.0,
            body: 4.0,
            ceiling: 5000.0,
            ..base
        },
        Surface::Sand => Recipe {
            length: 0.22,
            band: [800.0, 2600.0],
            q: 0.8,
            grains: 4000.0,
            grain_s: 0.0006,
            spread: 0.06,
            toe: Some(0.08),
            crunch: 2.6,
            body_hz: 220.0,
            body: 3.0,
            body_s: 0.03,
            ceiling: 3800.0,
            ..base
        },
        Surface::Snow => Recipe {
            length: 0.24,
            band: [600.0, 2000.0],
            q: 3.0,
            grains: 900.0,
            grain_s: 0.0025,
            spread: 0.07,
            toe: Some(0.09),
            crunch: 3.5,
            body_hz: 220.0,
            body: 3.0,
            body_s: 0.04,
            ceiling: 3200.0,
            ..base
        },
        Surface::Wood => Recipe {
            length: 0.17,
            band: [900.0, 2400.0],
            q: 1.0,
            grains: 700.0,
            grain_s: 0.0006,
            spread: 0.01,
            toe: Some(0.05),
            crunch: 1.6,
            body_hz: 300.0,
            body: 5.0,
            body_s: 0.015,
            // Hollow knock: two plank modes near 1:2.3.
            modes: &[(240.0, 20.0, 0.10), (560.0, 24.0, 0.07)],
            ceiling: 4200.0,
            ..base
        },
        Surface::Metal => Recipe {
            length: 0.32,
            band: [1500.0, 3500.0],
            grains: 800.0,
            grain_s: 0.0005,
            spread: 0.008,
            toe: Some(0.05),
            crunch: 1.4,
            body: 5.0,
            body_s: 0.015,
            // Inharmonic plate partials ring for tens of milliseconds.
            modes: &[(520.0, 120.0, 0.05), (1371.0, 150.0, 0.03), (2236.0, 150.0, 0.02)],
            ceiling: 5500.0,
            ..base
        },
        Surface::Water => Recipe {
            length: 0.30,
            band: [1000.0, 3000.0],
            q: 2.0,
            grains: 300.0,
            grain_s: 0.0015,
            spread: 0.08,
            toe: None,
            crunch: 1.0,
            body_hz: 300.0,
            body: 5.0,
            body_s: 0.06,
            ceiling: 3200.0,
            swish: 3.0,
            ..base
        },
    }
}

/// Reshapes a surface's step into another contact with the same material.
pub(super) fn shaped(r: Recipe, contact: Contact) -> Recipe {
    match contact {
        Contact::Step => r,
        // Debris keeps falling after the first crack: a long, dense cloud
        // with a second crumble where a step would roll to the toe.
        Contact::Break => Recipe {
            length: r.length * 1.8,
            grains: r.grains * 1.3,
            spread: r.spread * 2.2,
            toe: Some(r.toe.unwrap_or(0.06) * 1.7),
            crunch: r.crunch * 1.15,
            body: r.body * 1.2,
            body_s: r.body_s * 1.6,
            ..r
        },
        // A block seats with weight and little scatter, duller than a step.
        Contact::Place => Recipe {
            length: r.length * 0.75,
            grains: r.grains * 0.8,
            spread: r.spread * 0.6,
            toe: None,
            crunch: r.crunch * 1.2,
            body: r.body * 1.1,
            ceiling: r.ceiling * 0.9,
            ..r
        },
        Contact::Hit => Recipe {
            length: r.length * 0.55,
            grains: r.grains * 0.8,
            spread: r.spread * 0.5,
            toe: None,
            body: r.body * 0.8,
            ..r
        },
    }
}
