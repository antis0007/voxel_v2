//! Layered transients, filtered noise, resonant metal and short diffuse tails.
//! These are original procedural sound assets, not borrowed samples.
use super::Cue;
pub const RATE: u32 = 48_000;
pub fn variants() -> Vec<Vec<Vec<f32>>> {
    (0..Cue::COUNT)
        .map(|kind| (0..3).map(|variant| render(kind, variant)).collect())
        .collect()
}

fn render(kind: usize, variant: usize) -> Vec<f32> {
    let length = match kind {
        0 => 0.24,
        1 | 2 => 0.32,
        5 => 0.85,
        6 | 12 => 1.1,
        8 => 0.95,
        14 | 16 | 18 => 1.1,
        15 | 17 | 19 | 21 => 0.9,
        20 => 0.45,
        22 => 0.65,
        23 => 0.42,
        _ => 0.55,
    };
    let mut seed = 0x7f4a7c15u32
        .wrapping_add(kind as u32 * 131)
        .wrapping_add((variant as u32).wrapping_mul(0x9e37_79b9));
    let mut low = 0.0;
    let mut phase = variant as f32 * 0.7;
    let frames = (length * RATE as f32) as usize;
    let mut data = Vec::with_capacity(frames);
    for i in 0..frames {
        let t = i as f32 / RATE as f32;
        let u = t / length;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        low += (noise - low) * 0.12;
        let high = noise - low;
        let attack = (t / 0.004).min(1.0);
        let tail = (1.0 - u).max(0.0).powi(2);
        let pitch = match kind {
            0 => 300.0 - 200.0 * u,
            1 => 115.0 * (-t * 9.0).exp() + 42.0,
            2 => 420.0,
            3 => 950.0 - 680.0 * u,
            4 => 1800.0 - 700.0 * u,
            5 => 95.0 * (-t * 4.0).exp() + 25.0,
            6 => 440.0 + 220.0 * u,
            7 => 220.0 + 110.0 * u,
            8 => 160.0 + 90.0 * (t * 13.0).sin(),
            9 => 80.0,
            10 => 260.0 + 160.0 * u,
            11 => 190.0 * (-t * 5.0).exp() + 35.0,
            12 => {
                523.25
                    * if t < 0.20 {
                        1.0
                    } else if t < 0.40 {
                        1.25
                    } else {
                        1.5
                    }
            }
            14 => 65.0 + 55.0 * u,
            15 => 48.0 + 85.0 * (-t * 14.0).exp(),
            16 => 160.0 + 220.0 * u * u,
            17 => 310.0 - 180.0 * u,
            18 => 540.0 + 90.0 * u,
            19 => 810.0 - 350.0 * u,
            20 => 85.0 + 120.0 * (-t * 12.0).exp(),
            21 => 120.0 - 85.0 * u,
            22 => 220.0 + 660.0 * u,
            23 => 520.0 - 340.0 * u,
            _ => 2200.0 - 1700.0 * u,
        };
        phase += std::f32::consts::TAU * pitch / RATE as f32;
        let tone = phase.sin();
        let metal = (phase * 2.76).sin() * 0.34 + (phase * 4.19).sin() * 0.18;
        let sample = match kind {
            0 => high * 0.35 * (std::f32::consts::PI * u).sin() + low * 0.6,
            1 => {
                tone * 0.65 * (-t * 18.0).exp()
                    + low * 1.6 * (-t * 25.0).exp()
                    + high * 0.25 * (-t * 45.0).exp()
            }
            2 => metal * (-t * 12.0).exp() + high * 0.4 * (-t * 35.0).exp(),
            3 => tone * 0.22 + high * 0.25 * (-t * 8.0).exp() + metal * 0.16,
            4 | 13 => metal * 0.35 + high * 0.33 * (0.5 + 0.5 * (t * 190.0).sin()).powi(6),
            5 => tone * 0.65 + low * 1.1 * (-t * 5.0).exp(),
            6 | 12 => {
                (tone * 0.25 + (phase * 1.5).sin() * 0.15 + (phase * 2.0).sin() * 0.07)
                    * (t * 6.0).sin().abs()
            }
            7 => metal * 0.5 + tone * 0.3,
            8 => low * 0.8 + (phase * 0.5).sin() * 0.2 + high * 0.15 * (t * 51.0).sin().max(0.0),
            9 => tone * 0.45 * (-t * 12.0).exp() + low * 0.9 * (-t * 15.0).exp(),
            10 => (tone * 0.18 + metal * 0.3) * (0.5 + 0.5 * (t * 34.0).sin()),
            // Each guardian has a recognizable material and cadence.
            14 => (low * 1.1 + metal * 0.45 + tone * 0.35) * (0.3 + 0.7 * u) + high * 0.12,
            15 => tone * 0.8 + metal * 0.65 * (-t * 6.0).exp() + low * 1.3 * (-t * 8.0).exp(),
            16 => {
                ((phase * 0.503).sin() * 0.4 + metal * 0.2 + high * 0.2)
                    * (0.45 + 0.55 * (t * 19.0).sin().abs())
            }
            17 => {
                (tone * 0.3 + (phase * 1.007).sin() * 0.3 + high * 0.25)
                    * (0.4 + 0.6 * (t * 28.0).sin().abs())
            }
            18 => {
                (tone * 0.3 + (phase * 1.5).sin() * 0.22 + (phase * 2.01).sin() * 0.1)
                    * (0.35 + 0.65 * u)
            }
            19 => (tone * 0.25 + metal * 0.6) * (-t * 2.0).exp() + high * 0.18 * (-t * 18.0).exp(),
            20 => low * 1.2 * (-t * 8.0).exp() + tone * 0.6 + high * 0.3 * (-t * 12.0).exp(),
            21 => {
                (tone * 0.5 + (phase * 0.507).sin() * 0.35 + low * 0.8)
                    * (0.7 + 0.3 * (t * 31.0).sin())
            }
            22 => (high * 0.3 + metal * 0.35) * (std::f32::consts::PI * u).sin() + tone * 0.15,
            23 => {
                // A short white noise burst over a damped low body:
                // the small-air-pocket "plip" that follows a body
                // breaking the surface.
                high * 0.55 * (-t * 9.0).exp()
                    + low * 1.25 * (-t * 6.0).exp()
                    + tone * 0.18 * (-t * 14.0).exp()
            }
            _ => low * 0.8 + tone * 0.3,
        };
        data.push(sample * attack * tail * 0.65);
    }
    // Quiet, decorrelated early reflections avoid a dry electronic beep.
    for i in (0..data.len()).rev() {
        for (delay, gain) in [(1397, 0.14), (2689, 0.08)] {
            if i >= delay {
                data[i] += data[i - delay] * gain;
            }
        }
    }
    // A short cosine release removes the click that a hard sample end leaves
    // behind, especially after the reflection pass adds energy to the final
    // frames.
    let release = ((RATE as f32 * 0.012) as usize).min(data.len());
    let len = data.len();
    for i in 0..release {
        let t = i as f32 / release.max(1) as f32;
        let fade = t * t * (3.0 - 2.0 * t);
        data[len - 1 - i] *= fade;
    }
    // Put every cue in the same peak range. Gameplay gain still shapes
    // distance and importance, but one cue no longer arrives twice as hot as
    // another merely because its synthesis layers summed higher.
    let peak = data
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    if peak > 1.0e-6 {
        let gain = (0.88 / peak).clamp(0.25, 2.5);
        for sample in &mut data {
            *sample = (*sample * gain).clamp(-0.95, 0.95);
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_has_three_distinct_variants() {
        let bank = variants();
        assert_eq!(bank.len(), Cue::COUNT);
        for cue in &bank {
            assert_eq!(cue.len(), 3);
            assert_ne!(cue[0], cue[1]);
            assert_ne!(cue[1], cue[2]);
            assert_eq!(cue[0].len(), cue[1].len());
        }
    }

    #[test]
    fn cues_are_normalized_declicked_and_finite() {
        for cue in variants() {
            for variant in cue {
                let peak = variant
                    .iter()
                    .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
                assert!(peak <= 0.951, "cue peak {peak} escaped normalization");
                assert!(*variant.last().unwrap() == 0.0, "cue ends with a click");
                assert!(variant.iter().all(|sample| sample.is_finite()));
            }
        }
    }
}
