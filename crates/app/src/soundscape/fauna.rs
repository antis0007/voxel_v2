//! Animal ambience voice at the listener, from the biome table in `assets/biome_sounds.ron`.
use super::ambience::Surroundings;
use crate::audio::{Fauna, Placement};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Blend weights for the biome under the eye and the four points 16 m out.
const BLEND: [f32; 5] = [0.4, 0.15, 0.15, 0.15, 0.15];
/// Distance of the four outer biome samples, metres.
pub const BLEND_REACH_M: f32 = 16.0;

const TABLE: &str = include_str!("../../../../assets/biome_sounds.ron");
static VOICES: LazyLock<HashMap<String, Fauna>> = LazyLock::new(|| ron::from_str(TABLE).unwrap_or_default());

/// Listener conditions that thin out animal calls.
pub struct Listening {
    pub daylight: f32,
    pub underground: f32,
    pub rain: f32,
    pub air: f32,
}

/// Blends the voices of the biome under the eye and four around it, so a border fades in space.
/// A name missing from the table contributes silence at its weight; the centre's family wins.
pub fn blended(names: [&str; 5]) -> Fauna {
    let voice = |name: &str| VOICES.get(name).copied().unwrap_or_default();
    let mut out = names.iter().zip(BLEND).fold(Fauna::default(), |acc, (name, w)| {
        let v = voice(name);
        let l = |a: f32, b: f32| a + b * w;
        Fauna {
            birds: l(acc.birds, v.birds),
            bird_hz: l(acc.bird_hz, v.bird_hz),
            calls_per_s: l(acc.calls_per_s, v.calls_per_s),
            trill: l(acc.trill, v.trill),
            frogs: l(acc.frogs, v.frogs),
            frog_hz: l(acc.frog_hz, v.frog_hz),
            family: acc.family,
        }
    });
    out.family = voice(names[0]).family;
    out
}

/// Voice and placement for the blended biome under the listener's conditions.
/// Birds need canopy in some sector and frogs still water; bare ground is quiet.
pub fn voice(base: Fauna, at: &Listening, around: &Surroundings, yaw: f32) -> (Fauna, Placement) {
    let open = (1.0 - at.underground).clamp(0.0, 1.0) * at.air.clamp(0.0, 1.0);
    let day = at.daylight.clamp(0.0, 1.0);
    let peak = |w: &[f32; 8]| w.iter().copied().fold(0.0f32, f32::max).clamp(0.0, 1.0);
    // Birds fall quiet in heavy rain; frogs call through it.
    let birds = base.birds * day * (1.0 - 0.8 * at.rain.clamp(0.0, 1.0)) * open * peak(&around.canopy);
    let frogs = base.frogs * (0.25 + 0.75 * (1.0 - day)) * open * peak(&around.still_water);
    (Fauna { birds, frogs, ..base }, Placement { canopy: around.canopy, water: around.still_water, yaw })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ambience::Surroundings;

    const DAY: Listening = Listening { daylight: 1.0, underground: 0.0, rain: 0.0, air: 1.0 };

    #[test]
    fn table_parses_and_covers_every_biome() {
        let voices: HashMap<String, Fauna> = ron::from_str(TABLE).expect("biome_sounds.ron parses");
        for name in ["Ocean", "Coast", "Marsh", "Meadow", "Forest", "Taiga", "Desert", "Badlands",
            "Tundra", "Alpine", "Savanna", "Rainforest", "Heathland", "Fen", "LostPlateau",
            "CycadMarsh", "CloudForest", "RedwoodGrove", "OvergrownGardens", "FungalGrove", "LastShore"] {
            assert!(voices.contains_key(name), "{name} missing");
        }
        assert!(voices.values().any(|v| v.family == crate::audio::Family::Caw));
        // A caw is band-passed at 1.2 to 1.8 kHz; a centre pitch outside that pins every call to the edge.
        for (name, v) in voices.iter().filter(|(_, v)| v.family == crate::audio::Family::Caw) {
            assert!((1_400.0..=1_600.0).contains(&v.bird_hz), "{name} bird_hz {}", v.bird_hz);
        }
    }

    #[test]
    fn a_forest_over_bare_ground_has_no_birds() {
        let base = blended(["Forest"; 5]);
        let (v, p) = voice(base, &DAY, &Surroundings::default(), 0.0);
        assert_eq!(v.birds, 0.0);
        assert_eq!(p.canopy, [0.0; 8]);
        let mut s = Surroundings::default();
        s.canopy[3] = 0.8;
        let (v, p) = voice(base, &DAY, &s, 0.25);
        assert!(v.birds > 0.5 && p.canopy[3] == 0.8 && p.yaw == 0.25);
    }

    #[test]
    fn borders_blend_toward_the_mean_and_tolerate_unknown_names() {
        let mid = blended(["Desert", "Rainforest", "Rainforest", "Desert", "Desert"]);
        let (d, r) = (blended(["Desert"; 5]), blended(["Rainforest"; 5]));
        assert!(mid.calls_per_s > d.calls_per_s && mid.calls_per_s < r.calls_per_s);
        assert_eq!(mid.family, d.family);
        let partial = blended(["Forest", "Nowhere", "Nowhere", "Nowhere", "Nowhere"]);
        assert!((partial.birds - 0.4 * blended(["Forest"; 5]).birds).abs() < 1e-4);
    }

    #[test]
    fn rainforest_is_busier_than_desert_and_caves_are_silent() {
        let full = Surroundings { canopy: [1.0; 8], still_water: [1.0; 8], ..Default::default() };
        let v = |name: &str, at: &Listening| voice(blended([name; 5]), at, &full, 0.0).0;
        assert!(v("Rainforest", &DAY).birds > v("Desert", &DAY).birds * 4.0);
        let cave = Listening { underground: 1.0, ..DAY };
        assert_eq!(v("Forest", &cave).birds, 0.0);
        let night = Listening { daylight: 0.0, ..DAY };
        assert_eq!(v("Forest", &night).birds, 0.0);
        assert!(v("Fen", &night).frogs > v("Fen", &DAY).frogs);
    }
}
