//! Per-finger grip pose for the viewmodel hand.
//!
//! The Blender hand is bound as a closed fist and its authored `open` clip eases
//! every finger a short way out of it. A grip is therefore a scale of that turn
//! per finger: `0` keeps the fist, `1` is the authored open pose, larger values
//! flatten the hand, and a small negative value squeezes. Presets live in
//! `assets/runtime/viewmodels/grips.ron`; the held item picks one
//! (`held_item::grip`) and `player::viewmodel` eases toward it so a hotbar swap
//! unwraps and re-wraps the fingers instead of snapping.
use serde::Deserialize;
use std::sync::LazyLock;

/// Thumb, index, middle, ring, little.
pub const FINGERS: usize = 5;
/// Range a finger's openness is clamped to: past the top the joints fold backwards.
pub const OPEN_RANGE: (f32, f32) = (-0.5, 4.0);

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct Fingers(pub [f32; FINGERS]);

impl Default for Fingers {
    fn default() -> Self { Self::CLOSED }
}

impl Fingers {
    /// The bind pose: every finger curled shut.
    pub const CLOSED: Self = Self([0.0; FINGERS]);

    pub const fn uniform(open: f32) -> Self { Self([open; FINGERS]) }

    /// Finite values inside `OPEN_RANGE`; a bad value falls back to the closed fist.
    pub fn sanitized(self) -> Self {
        Self(self.0.map(|o| if o.is_finite() { o.clamp(OPEN_RANGE.0, OPEN_RANGE.1) } else { 0.0 }))
    }

    pub fn is_closed(self) -> bool { self.0.iter().all(|o| o.abs() < 1e-4) }

    /// Exponential approach, independent of how the time is partitioned.
    pub fn approach(&mut self, target: Self, rate: f32, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 || !rate.is_finite() { return; }
        let k = 1.0 - (-rate * dt).exp();
        let target = target.sanitized();
        for (value, goal) in self.0.iter_mut().zip(target.0) { *value += (goal - *value) * k; }
        *self = self.sanitized();
    }

    /// Index of the finger a rig joint belongs to, from its name (`Index1`, `Thumb2`, ...).
    pub fn of_joint(name: &str) -> Option<usize> {
        ["Thumb", "Index", "Middle", "Ring", "Little"].iter().position(|f| name.starts_with(f))
    }
}

/// What the hand is doing; the held item maps onto one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    /// Nothing held: a loose, relaxed hand.
    Bare,
    /// Slim upright shaft in the fist.
    Torch,
    /// A cube cradled with the thumb over the top.
    Block,
    /// Long tool handle: the authored fist.
    Shaft,
    /// Sword grip: the authored fist, squeezed a touch.
    Blade,
    /// Flat sprite items (food, gems, maps) between thumb and fingers.
    Sprite,
    /// A bucket or pan handle hooked by the fingers.
    Handle,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presets {
    /// Finger approach rate per second.
    pub ease_rate: f32,
    bare: Fingers, torch: Fingers, block: Fingers, shaft: Fingers,
    blade: Fingers, sprite: Fingers, handle: Fingers,
    /// Ready stance of a held blade: turn (pitch, yaw, roll radians; applied x, then y, then z)
    /// and a camera-space nudge (right, up, forward metres) from the common hold point.
    blade_turn: [f32; 3],
    blade_seat: [f32; 3],
}

pub static PRESETS: LazyLock<Presets> = LazyLock::new(|| {
    ron::from_str(include_str!("../../../../../assets/runtime/viewmodels/grips.ron"))
        .unwrap_or_else(|error| panic!("invalid grip presets: {error}"))
});

/// Target finger pose for a grip.
pub fn preset(grip: Grip) -> Fingers {
    let p = &*PRESETS;
    match grip {
        Grip::Bare => p.bare, Grip::Torch => p.torch, Grip::Block => p.block,
        Grip::Shaft => p.shaft, Grip::Blade => p.blade, Grip::Sprite => p.sprite,
        Grip::Handle => p.handle,
    }.sanitized()
}

/// Blade ready stance as (pitch, yaw, roll, seat right, up, forward).
/// `VOXEL_REVIEW_BLADE=p,y,r[,x,y,z]` overrides it in benchmark captures.
pub fn blade_stance() -> ([f32; 3], [f32; 3]) {
    static OVERRIDE: LazyLock<Option<([f32; 3], [f32; 3])>> = LazyLock::new(|| {
        if !std::env::args().any(|a| a.starts_with("--benchmark-")) { return None; }
        let text = std::env::var("VOXEL_REVIEW_BLADE").ok()?;
        let v: Vec<f32> = text.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let (turn, seat) = match v[..] {
            [p, y, r] => ([p, y, r], PRESETS.blade_seat),
            [p, y, r, x, u, f] => ([p, y, r], [x, u, f]),
            _ => { eprintln!("[viewmodel] VOXEL_REVIEW_BLADE needs 3 or 6 numbers, got {text:?}"); return None; }
        };
        (turn.iter().chain(&seat).all(|n| n.is_finite())).then_some((turn, seat))
    });
    (*OVERRIDE).unwrap_or((PRESETS.blade_turn, PRESETS.blade_seat))
}

/// `VOXEL_REVIEW_FINGERS=thumb,index,middle,ring,little` pins the main hand in benchmark
/// captures, so a grip is tuned from a capture without a rebuild; one value sets all five.
pub fn review_override() -> Option<Fingers> {
    static OVERRIDE: LazyLock<Option<Fingers>> = LazyLock::new(|| {
        if !std::env::args().any(|a| a.starts_with("--benchmark-")) { return None; }
        let text = std::env::var("VOXEL_REVIEW_FINGERS").ok()?;
        let values: Vec<f32> = text.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        match values[..] {
            [all] => Some(Fingers::uniform(all)),
            [a, b, c, d, e] => Some(Fingers([a, b, c, d, e])),
            _ => { eprintln!("[viewmodel] VOXEL_REVIEW_FINGERS needs 1 or 5 numbers, got {text:?}"); None }
        }
    });
    *OVERRIDE
}
