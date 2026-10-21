//! Underground magic zones. Depth below the surface is cut into layers; each
//! layer rolls an archetype and a set of sub-aspects from the world seed, then
//! jitters every numeric trait, so two worlds (and two layers of one world)
//! never share the same environment. Lateral pockets add a further sub-aspect
//! so one layer is not uniform end to end. Everything is a pure function of
//! `(seed, position)`, so the renderer, spells and audio can all ask the same
//! question without storing any state.
use super::{hash_u32, hash01, smoothstep};

/// Metres below the local surface before the first zone begins.
pub const SHALLOW_M: f32 = 14.0;
/// Vertical thickness of each layer in metres.
pub const LAYER_M: f32 = 52.0;
/// Half-width of the cross-fade between neighbouring layers in metres.
const BLEND_M: f32 = 9.0;
/// Horizontal size of a lateral pocket cell in metres.
const POCKET_M: f32 = 88.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Archetype {
    Ember,
    Flooded,
    Crystal,
    Rot,
    Frost,
    Void,
    Storm,
    Gilded,
}

const ARCHETYPES: [Archetype; 8] = [
    Archetype::Ember,
    Archetype::Flooded,
    Archetype::Crystal,
    Archetype::Rot,
    Archetype::Frost,
    Archetype::Void,
    Archetype::Storm,
    Archetype::Gilded,
];

impl Archetype {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ember => "Ember Deep",
            Self::Flooded => "Drowned Reach",
            Self::Crystal => "Prism Vault",
            Self::Rot => "Sporefall",
            Self::Frost => "Hushed Ice",
            Self::Void => "Hollow Dark",
            Self::Storm => "Static Hall",
            Self::Gilded => "Gilded Hush",
        }
    }

    /// One line shown when the player descends into the layer.
    pub fn entry_line(self) -> &'static str {
        match self {
            Self::Ember => {
                "The rock is warm underhand. Sparks drift upward and do not come back down."
            }
            Self::Flooded => {
                "Water beads on every wall. Your light bends and swims. Sound arrives late."
            }
            Self::Crystal => "Facets throw your own light back at you, a little changed each time.",
            Self::Rot => "The air is thick and sweet. Something below you is breathing out spores.",
            Self::Frost => {
                "Your breath hangs in front of you. Everything here seems to be listening."
            }
            Self::Void => {
                "It is dark. You are likely to be eaten by a grue. Your spells feel hungrier."
            }
            Self::Storm => {
                "The hair on your arms lifts. Light crawls along the ceiling in thin blue threads."
            }
            Self::Gilded => {
                "Gold leaf lines the walls and swallows every echo. You step softly without trying."
            }
        }
    }

    /// Base veil colour, desaturation, pulse rate (Hz), swirl, spell gain.
    fn base(self) -> Traits {
        let (veil, veil_amount, desat, pulse, swirl, gain) = match self {
            Self::Ember => ([1.00, 0.36, 0.10], 0.20, 0.00, 0.35, 0.30, 1.10),
            Self::Flooded => ([0.10, 0.45, 0.62], 0.26, 0.10, 0.18, 0.80, 0.90),
            Self::Crystal => ([0.62, 0.40, 1.00], 0.16, 0.00, 0.55, 0.40, 1.25),
            Self::Rot => ([0.42, 0.62, 0.14], 0.24, 0.15, 0.25, 0.55, 1.00),
            Self::Frost => ([0.55, 0.80, 1.00], 0.18, 0.30, 0.10, 0.20, 0.85),
            Self::Void => ([0.25, 0.10, 0.45], 0.30, 0.45, 0.14, 1.00, 1.35),
            Self::Storm => ([0.70, 0.78, 1.00], 0.14, 0.05, 1.10, 0.35, 1.20),
            Self::Gilded => ([1.00, 0.80, 0.34], 0.17, 0.05, 0.22, 0.15, 1.05),
        };
        Traits {
            veil,
            veil_amount,
            desat,
            pulse,
            swirl,
            gain,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aspect {
    /// Slow breathing glow.
    Breathing,
    /// Sparks and motes rise instead of fall.
    Updraft,
    /// Spell light lingers and spreads.
    Resonant,
    /// Spell light is swallowed faster than normal.
    Muffled,
    /// Colour drains toward grey.
    Washed,
    /// Hard light flicker.
    Guttering,
    /// Veil colour rotates slowly through neighbouring hues.
    Drifting,
    /// Spells cast here arrive stronger but cost dread (damped sight).
    Hungry,
}

const ASPECTS: [Aspect; 8] = [
    Aspect::Breathing,
    Aspect::Updraft,
    Aspect::Resonant,
    Aspect::Muffled,
    Aspect::Washed,
    Aspect::Guttering,
    Aspect::Drifting,
    Aspect::Hungry,
];

impl Aspect {
    pub fn name(self) -> &'static str {
        match self {
            Self::Breathing => "breathing",
            Self::Updraft => "updraft",
            Self::Resonant => "resonant",
            Self::Muffled => "muffled",
            Self::Washed => "washed",
            Self::Guttering => "guttering",
            Self::Drifting => "drifting",
            Self::Hungry => "hungry",
        }
    }

    fn bit(self) -> u8 {
        1 << ASPECTS.iter().position(|a| *a == self).unwrap_or(0)
    }
}

/// Up to three aspects held as a bit set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Aspects(u8);

impl Aspects {
    pub fn has(self, aspect: Aspect) -> bool {
        self.0 & aspect.bit() != 0
    }

    pub fn list(self) -> Vec<Aspect> {
        ASPECTS.iter().copied().filter(|a| self.has(*a)).collect()
    }

    fn with(self, aspect: Aspect) -> Self {
        Self(self.0 | aspect.bit())
    }
}

/// Numeric look of a zone. Blendable, so the camera can cross a layer seam
/// without a pop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Traits {
    pub veil: [f32; 3],
    pub veil_amount: f32,
    pub desat: f32,
    pub pulse: f32,
    pub swirl: f32,
    /// Multiplier on screen-space spell strength.
    pub gain: f32,
}

impl Traits {
    fn lerp(self, other: Traits, t: f32) -> Traits {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Traits {
            veil: [
                l(self.veil[0], other.veil[0]),
                l(self.veil[1], other.veil[1]),
                l(self.veil[2], other.veil[2]),
            ],
            veil_amount: l(self.veil_amount, other.veil_amount),
            desat: l(self.desat, other.desat),
            pulse: l(self.pulse, other.pulse),
            swirl: l(self.swirl, other.swirl),
            gain: l(self.gain, other.gain),
        }
    }

    const NONE: Traits = Traits {
        veil: [1.0, 1.0, 1.0],
        veil_amount: 0.0,
        desat: 0.0,
        pulse: 0.0,
        swirl: 0.0,
        gain: 1.0,
    };
}

/// One resolved layer, before position blending.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub index: u32,
    pub archetype: Archetype,
    pub aspects: Aspects,
    pub traits: Traits,
}

/// What the renderer needs at one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneSample {
    /// Layer the point is nearest the centre of.
    pub layer: Layer,
    pub traits: Traits,
    /// 0 at the shallow edge, 1 once fully inside the first layer.
    pub depth_weight: f32,
}

fn jitter(seed: u32, salt: u32, spread: f32) -> f32 {
    1.0 + (hash01(hash_u32(seed ^ salt)) * 2.0 - 1.0) * spread
}

fn layer_hash(seed: u32, index: u32) -> u32 {
    hash_u32(seed ^ index.wrapping_mul(0x9e37_79b1) ^ 0x5a17_0001)
}

/// Walks down from layer 0 so each layer can avoid the archetype that was
/// actually chosen above it, not merely the one it first rolled.
fn archetype_index(seed: u32, index: u32) -> usize {
    let n = ARCHETYPES.len();
    let mut prev = usize::MAX;
    let mut pick = 0;
    for i in 0..=index {
        let h = layer_hash(seed, i);
        pick = (h % n as u32) as usize;
        if pick == prev {
            pick = (pick + 1 + (hash_u32(h) % (n as u32 - 1)) as usize) % n;
        }
        prev = pick;
    }
    pick
}

/// Resolve layer `index` for a world. Deterministic in `(seed, index)`.
pub fn layer(seed: u32, index: u32) -> Layer {
    let base = layer_hash(seed, index);
    let pick = archetype_index(seed, index);
    let archetype = ARCHETYPES[pick];

    let mut aspects = Aspects::default();
    let count = 1 + (hash_u32(base ^ 0x77) % 3) as usize;
    for i in 0..count {
        let a = ASPECTS[(hash_u32(base ^ (0x100 + i as u32)) % ASPECTS.len() as u32) as usize];
        aspects = aspects.with(a);
    }

    let mut t = archetype.base();
    t.veil_amount *= jitter(base, 0x11, 0.30);
    t.desat = (t.desat * jitter(base, 0x12, 0.40)).clamp(0.0, 0.8);
    t.pulse *= jitter(base, 0x13, 0.35);
    t.swirl *= jitter(base, 0x14, 0.45);
    t.gain *= jitter(base, 0x15, 0.15);
    // Per-layer hue wobble so two Ember layers are still different embers.
    let wobble = (hash01(base ^ 0x16) - 0.5) * 0.22;
    t.veil = [
        (t.veil[0] * (1.0 + wobble)).clamp(0.0, 1.2),
        (t.veil[1] * (1.0 - wobble * 0.5)).clamp(0.0, 1.2),
        (t.veil[2] * (1.0 - wobble)).clamp(0.0, 1.2),
    ];
    apply_aspects(&mut t, aspects);
    Layer {
        index,
        archetype,
        aspects,
        traits: t,
    }
}

fn apply_aspects(t: &mut Traits, aspects: Aspects) {
    if aspects.has(Aspect::Breathing) {
        t.pulse = t.pulse.min(0.3) + 0.12;
        t.veil_amount *= 1.15;
    }
    if aspects.has(Aspect::Washed) {
        t.desat = (t.desat + 0.25).min(0.8);
    }
    if aspects.has(Aspect::Guttering) {
        t.pulse += 0.9;
    }
    if aspects.has(Aspect::Resonant) {
        t.gain *= 1.15;
        t.swirl *= 1.2;
    }
    if aspects.has(Aspect::Muffled) {
        t.gain *= 0.8;
        t.veil_amount *= 0.9;
    }
    if aspects.has(Aspect::Hungry) {
        t.gain *= 1.25;
        t.desat = (t.desat + 0.1).min(0.8);
    }
}

/// A lateral pocket nudges a layer's traits depending on where in the layer
/// the point lies, so one layer reads as patchy rather than uniform.
fn pocket(seed: u32, layer_index: u32, x: f32, z: f32) -> (Aspect, f32) {
    let cx = (x / POCKET_M).floor();
    let cz = (z / POCKET_M).floor();
    let h = hash_u32(
        seed ^ (cx as i32 as u32).wrapping_mul(0x85eb_ca6b)
            ^ (cz as i32 as u32).wrapping_mul(0xc2b2_ae35)
            ^ layer_index.wrapping_mul(0x27d4_eb2f),
    );
    let aspect = ASPECTS[(h % ASPECTS.len() as u32) as usize];
    // Strongest at the cell centre, absent at its edges.
    let fx = (x / POCKET_M - cx - 0.5).abs() * 2.0;
    let fz = (z / POCKET_M - cz - 0.5).abs() * 2.0;
    let weight = 1.0 - smoothstep(0.45, 1.0, fx.max(fz));
    (aspect, weight * if h & 0x100 != 0 { 1.0 } else { 0.0 })
}

/// Zone at a point. `depth_m` is metres below the local surface (negative or
/// small values mean no zone). `x`/`z` are world metres.
pub fn sample(seed: u32, x: f32, z: f32, depth_m: f32) -> Option<ZoneSample> {
    if depth_m <= SHALLOW_M * 0.5 {
        return None;
    }
    let depth_weight = smoothstep(SHALLOW_M * 0.5, SHALLOW_M * 1.5, depth_m);
    let rel = (depth_m - SHALLOW_M).max(0.0);
    let index = (rel / LAYER_M).floor() as u32;
    let into = rel - index as f32 * LAYER_M;

    let here = layer(seed, index);
    let mut traits = here.traits;
    if into > LAYER_M - BLEND_M {
        let t = smoothstep(LAYER_M - BLEND_M, LAYER_M, into);
        traits = traits.lerp(layer(seed, index + 1).traits, t * 0.5);
    } else if into < BLEND_M && index > 0 {
        let t = 1.0 - smoothstep(0.0, BLEND_M, into);
        traits = traits.lerp(layer(seed, index - 1).traits, t * 0.5);
    }

    let (aspect, w) = pocket(seed, index, x, z);
    let mut aspects = here.aspects;
    if w > 0.05 && !aspects.has(aspect) {
        let mut extra = traits;
        apply_aspects(&mut extra, Aspects::default().with(aspect));
        traits = traits.lerp(extra, w);
        aspects = aspects.with(aspect);
    }

    traits = Traits::NONE.lerp(traits, depth_weight);
    Some(ZoneSample {
        layer: Layer { aspects, ..here },
        traits,
        depth_weight,
    })
}

/// How a zone bends spell particles. Neutral outside any zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fx {
    /// Multiplies particle gravity; negative makes motes rise.
    pub gravity_mul: f32,
    pub life_mul: f32,
    pub size_mul: f32,
    pub glow_mul: f32,
    /// 0..1 chance-free flicker depth applied to glow.
    pub flicker: f32,
    pub tint: [f32; 3],
    /// How far particle colour is pulled toward `tint`.
    pub tint_k: f32,
}

impl Fx {
    pub const NEUTRAL: Fx = Fx {
        gravity_mul: 1.0,
        life_mul: 1.0,
        size_mul: 1.0,
        glow_mul: 1.0,
        flicker: 0.0,
        tint: [1.0, 1.0, 1.0],
        tint_k: 0.0,
    };

    /// Particle colour after the zone tint.
    pub fn tinted(&self, c: [f32; 3]) -> [f32; 3] {
        let k = self.tint_k;
        [
            c[0] + (self.tint[0] - c[0]) * k,
            c[1] + (self.tint[1] - c[1]) * k,
            c[2] + (self.tint[2] - c[2]) * k,
        ]
    }

    /// Glow for a particle spawned at `phase` (any per-spawn varying value).
    pub fn glow(&self, base: f32, phase: f32) -> f32 {
        let wobble = 0.5 + 0.5 * (phase * 37.0).sin();
        base * self.glow_mul * (1.0 - self.flicker * wobble)
    }
}

impl Default for Fx {
    fn default() -> Self {
        Fx::NEUTRAL
    }
}

pub fn effects(sample: Option<&ZoneSample>) -> Fx {
    let Some(s) = sample else { return Fx::NEUTRAL };
    let w = s.depth_weight;
    let mut fx = Fx::NEUTRAL;
    match s.layer.archetype {
        Archetype::Ember => fx.gravity_mul = -0.35,
        Archetype::Flooded => {
            fx.gravity_mul = 0.45;
            fx.life_mul = 1.15;
        }
        Archetype::Frost => fx.life_mul = 1.2,
        Archetype::Storm => fx.glow_mul = 1.3,
        Archetype::Void => fx.size_mul = 1.15,
        _ => {}
    }
    let a = s.layer.aspects;
    if a.has(Aspect::Updraft) {
        fx.gravity_mul = -0.6;
    }
    if a.has(Aspect::Resonant) {
        fx.life_mul *= 1.35;
        fx.size_mul *= 1.1;
    }
    if a.has(Aspect::Muffled) {
        fx.life_mul *= 0.7;
        fx.glow_mul *= 0.75;
    }
    if a.has(Aspect::Guttering) {
        fx.flicker = 0.55;
    }
    if a.has(Aspect::Hungry) {
        fx.size_mul *= 0.85;
        fx.glow_mul *= 1.3;
    }
    fx.tint = s.traits.veil;
    fx.tint_k = (s.traits.veil_amount * 1.8).clamp(0.0, 0.5);
    // Fade every effect in with depth so the surface never changes.
    let n = Fx::NEUTRAL;
    let l = |a: f32, b: f32| a + (b - a) * w;
    Fx {
        gravity_mul: l(n.gravity_mul, fx.gravity_mul),
        life_mul: l(n.life_mul, fx.life_mul),
        size_mul: l(n.size_mul, fx.size_mul),
        glow_mul: l(n.glow_mul, fx.glow_mul),
        flicker: fx.flicker * w,
        tint: fx.tint,
        tint_k: fx.tint_k * w,
    }
}

/// Shader-ready grade: `[veil r, g, b, amount]` and `[desat, pulse, swirl, gain]`.
pub type Grade = ([f32; 4], [f32; 4]);

pub const NO_GRADE: Grade = ([1.0, 1.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]);

pub fn grade(sample: Option<&ZoneSample>) -> Grade {
    match sample {
        None => NO_GRADE,
        Some(s) => {
            let t = s.traits;
            (
                [t.veil[0], t.veil[1], t.veil[2], t.veil_amount],
                [t.desat, t.pulse, t.swirl, t.gain],
            )
        }
    }
}

/// Spell colour pulled toward the zone veil so magic looks native to the layer.
pub fn tint_spell(color: [f32; 3], sample: Option<&ZoneSample>) -> [f32; 3] {
    let Some(s) = sample else { return color };
    let k = (s.traits.veil_amount * 2.2).clamp(0.0, 0.6) * s.depth_weight;
    [
        color[0] + (s.traits.veil[0] - color[0]) * k,
        color[1] + (s.traits.veil[1] - color[1]) * k,
        color[2] + (s.traits.veil[2] - color[2]) * k,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_has_no_zone() {
        assert!(sample(7, 0.0, 0.0, 0.0).is_none());
        assert!(sample(7, 0.0, 0.0, -30.0).is_none());
    }

    #[test]
    fn deterministic_per_seed() {
        assert_eq!(layer(42, 3), layer(42, 3));
        assert_eq!(sample(42, 10.0, 20.0, 90.0), sample(42, 10.0, 20.0, 90.0));
    }

    #[test]
    fn adjacent_layers_differ() {
        for seed in 0..200 {
            for i in 1..10 {
                assert_ne!(
                    layer(seed, i).archetype,
                    layer(seed, i - 1).archetype,
                    "seed {seed} layer {i}"
                );
            }
        }
    }

    #[test]
    fn worlds_vary() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..64 {
            seen.insert(layer(seed, 0).archetype as u8);
        }
        assert!(
            seen.len() >= 5,
            "only {} archetypes across 64 seeds",
            seen.len()
        );
        // Same archetype, different numbers.
        let veils: std::collections::HashSet<u32> = (0..64)
            .map(|s| layer(s, 0))
            .filter(|l| l.archetype == Archetype::Ember)
            .map(|l| l.traits.veil_amount.to_bits())
            .collect();
        assert!(veils.len() > 1);
    }

    #[test]
    fn seam_is_continuous() {
        // Walk down through several layer seams and bound the per-step jump.
        let mut prev = sample(11, 5.0, 5.0, 20.0).unwrap().traits;
        let mut d = 20.0;
        while d < 300.0 {
            d += 0.25;
            let t = sample(11, 5.0, 5.0, d).unwrap().traits;
            assert!(
                (t.veil_amount - prev.veil_amount).abs() < 0.08,
                "veil jump at {d}"
            );
            assert!((t.veil[1] - prev.veil[1]).abs() < 0.2, "hue jump at {d}");
            prev = t;
        }
    }

    #[test]
    fn effects_neutral_on_surface_and_bounded_below() {
        assert_eq!(effects(None), Fx::NEUTRAL);
        for seed in 0..100 {
            let z = sample(seed, 3.0, 4.0, 120.0).unwrap();
            let fx = effects(Some(&z));
            assert!(
                fx.life_mul > 0.4 && fx.life_mul < 2.0,
                "life {}",
                fx.life_mul
            );
            assert!(fx.size_mul > 0.5 && fx.size_mul < 1.6);
            assert!(fx.tint_k >= 0.0 && fx.tint_k <= 0.5);
            let c = fx.tinted([0.2, 0.4, 0.9]);
            assert!(c.iter().all(|v| v.is_finite() && *v >= 0.0));
        }
        let shallow = effects(sample(5, 0.0, 0.0, SHALLOW_M * 0.6).as_ref());
        assert!((shallow.gravity_mul - 1.0).abs() < 0.6);
    }

    #[test]
    fn grade_fades_in_from_surface() {
        let shallow = sample(3, 0.0, 0.0, SHALLOW_M * 0.6).unwrap();
        let deep = sample(3, 0.0, 0.0, 120.0).unwrap();
        assert!(shallow.traits.veil_amount < deep.traits.veil_amount * 0.6 + 0.01);
        assert_eq!(grade(None), NO_GRADE);
    }
}
