//! Layered transients, filtered noise, resonant metal and short diffuse tails.
//! These are original procedural sound assets, not borrowed samples.
use super::Cue;
use crate::dsp::Svf;
pub const RATE: u32 = 48_000;
pub fn variants() -> Vec<Vec<Vec<f32>>> {
    (0..Cue::COUNT)
        .map(|kind| (0..3).map(|variant| render(kind, variant)).collect())
        .collect()
}

fn render(kind: usize, variant: usize) -> Vec<f32> {
    let length = match kind {
        0 => 0.20,
        1 | 2 => 0.32,
        5 => 0.85,
        6 | 12 => 1.1,
        8 => 0.95,
        14 | 16 | 18 => 1.1,
        15 | 17 | 19 | 21 => 0.9,
        20 => 0.45,
        22 => 0.65,
        23 => 0.42,
        24 => 0.24,
        25 => 0.75,
        26 => 0.34,
        27 => 0.50,
        28 => 0.30,
        29 => 0.36,
        30 => 0.62,
        _ => 0.55,
    };
    let mut seed = 0x7f4a7c15u32
        .wrapping_add(kind as u32 * 131)
        .wrapping_add((variant as u32).wrapping_mul(0x9e37_79b9));
    let mut low = 0.0;
    let mut phase = variant as f32 * 0.7;
    let mut phase_two = variant as f32 * 1.91;
    let mut low_noise = 0.0;
    // Noise above this ceiling is what made swings, splashes and shatters
    // hiss like broadband static.
    let mut top = Svf::new(5500.0, 0.707, RATE as f32);
    let mut spray = Svf::new(2000.0, 0.9, RATE as f32);
    let frames = (length * RATE as f32) as usize;
    let mut data = Vec::with_capacity(frames);
    for i in 0..frames {
        let t = i as f32 / RATE as f32;
        let u = t / length;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let white = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        // Slew the deterministic noise source before it reaches any cue layer;
        // raw xorshift noise produced a brittle, almost digital edge.
        low_noise += (white - low_noise) * 0.018;
        let noise = low_noise;
        low += (noise - low) * 0.12;
        let high = top.low(white) - noise;
        let soft_noise = noise;
        let slow_boss_windup = matches!(kind, 14 | 16 | 18);
        let attack = (t / if matches!(kind, 3 | 4 | 8 | 21) { 0.012 } else if slow_boss_windup { 0.055 } else { 0.004 }).min(1.0);
        let tail = (1.0 - u).max(0.0).powi(if matches!(kind, 3 | 4 | 8 | 21 | 14 | 16 | 18) { 3 } else { 2 });
        let pitch = match kind {
            0 => 300.0 - 200.0 * u,
            1 => 115.0 * (-t * 9.0).exp() + 42.0,
            2 => 420.0,
            3 => 480.0 - 160.0 * u,
            4 => 760.0 - 270.0 * u,
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
            14 => 92.0 + 54.0 * u,
            15 => 58.0 + 38.0 * (-t * 10.0).exp(),
            16 => 176.0 + 116.0 * u * u,
            17 => 392.0 - 198.0 * u,
            18 => 280.0 + 205.0 * u,
            19 => 620.0 - 320.0 * u,
            20 => 85.0 + 120.0 * (-t * 12.0).exp(),
            21 => 120.0 - 85.0 * u,
            22 => 220.0 + 660.0 * u,
            23 => 520.0 - 340.0 * u,
            24 => 170.0 + 100.0 * (-t * 35.0).exp(),
            25 => 110.0 + 40.0 * (-t * 5.0).exp(),
            // Dash: thruster chirp sweeping up an octave and a half.
            26 => 180.0 + 520.0 * (1.0 - (-t * 14.0).exp()),
            // Slide: a low rumble that sags with the slide speed.
            27 => 70.0 - 22.0 * u,
            // Air jump: bright rising puff.
            28 => 260.0 + 340.0 * u.sqrt(),
            // Land: body drop.
            29 => 62.0 * (-t * 7.0).exp() + 34.0,
            // Bleat: voiced, rising then sagging.
            30 => 290.0 + 40.0 * (std::f32::consts::PI * u).sin() - 30.0 * u,
            _ => 2200.0 - 1700.0 * u,
        };
        phase += std::f32::consts::TAU * pitch / RATE as f32;
        let second_pitch = match kind {
            3 => pitch * (1.498 + variant as f32 * 0.008),
            4 => pitch * (1.255 + variant as f32 * 0.006),
            8 => pitch * 0.503,
            21 => pitch * 0.667,
            _ => pitch * 1.51,
        };
        phase_two += std::f32::consts::TAU * second_pitch / RATE as f32;
        let tone = phase.sin();
        let tone_two = phase_two.sin();
        let metal = (phase * 2.76).sin() * 0.34 + (phase * 4.19).sin() * 0.18;
        let sample = match kind {
            0 => high * 0.35 * (std::f32::consts::PI * u).sin() + low * 0.6,
            1 => {
                tone * 0.65 * (-t * 18.0).exp()
                    + low * 1.6 * (-t * 25.0).exp()
                    + high * 0.25 * (-t * 45.0).exp()
            }
            2 => metal * (-t * 12.0).exp() + high * 0.4 * (-t * 35.0).exp(),
            // Spark: warm, short, lightly granular pulse instead of a sharp
            // high-pitched electronic chirp.
            3 => (tone * 0.27 + tone_two * 0.10) * (-t * 7.5).exp()
                + low * 0.34 * (-t * 15.0).exp()
                + soft_noise * 0.045 * (-t * 19.0).exp(),
            // Frost: glassy but soft, with a low icy body and a restrained
            // upper shimmer rather than broadband fizz.
            4 => (tone * 0.22 + tone_two * 0.13) * (-t * 3.4).exp()
                + (phase * 2.01).sin() * 0.055 * (-t * 4.0).exp()
                + low * 0.24 * (-t * 5.0).exp()
                + soft_noise * 0.045 * (-t * 9.0).exp(),
            13 => metal * 0.35 + high * 0.33 * (0.5 + 0.5 * (t * 190.0).sin()).powi(6),
            5 => tone * 0.65 + low * 1.1 * (-t * 5.0).exp(),
            6 | 12 => {
                (tone * 0.25 + (phase * 1.5).sin() * 0.15 + (phase * 2.0).sin() * 0.07)
                    * (t * 6.0).sin().abs()
            }
            7 => metal * 0.5 + tone * 0.3,
            // Bloom: rounded breathy bloom with a slow organic swell.
            8 => low * 0.55 * (-t * 1.8).exp()
                + (tone_two * 0.28 + tone * 0.14) * (-t * 1.2).exp()
                + soft_noise * 0.065 * (t * 17.0).sin().max(0.0),
            9 => tone * 0.45 * (-t * 12.0).exp() + low * 0.9 * (-t * 15.0).exp(),
            10 => (tone * 0.18 + metal * 0.3) * (0.5 + 0.5 * (t * 34.0).sin()),
            // Cinder Castellan: iron draw and compact hammer release.
            14 => {
                let swell = (std::f32::consts::PI * u * 0.82).sin().max(0.0);
                (tone * 0.24 + tone_two * 0.12 + low * 0.34) * swell
                    + metal * 0.08 * swell
                    + soft_noise * 0.04 * swell
            }
            15 => {
                let body = (-t * 13.0).exp();
                let ring = (-t * 5.0).exp();
                tone * 0.46 * body + low * 0.58 * body
                    + (tone_two * 0.18 + metal * 0.12) * ring
                    + high * 0.025 * (-t * 30.0).exp()
            }
            // Glassweaver: hollow rising glass resonance, then a clear
            // two-note release rather than a pulsing alarm-like tone.
            16 => {
                let swell = (std::f32::consts::PI * u * 0.72).sin().max(0.0);
                (tone * 0.21 + tone_two * 0.16 + (phase * 0.503).sin() * 0.12)
                    * swell
                    + soft_noise * 0.025 * swell
            }
            17 => {
                let decay = (-t * 3.8).exp();
                (tone * 0.24 + tone_two * 0.20 + (phase * 2.01).sin() * 0.08)
                    * decay
                    + low * 0.10 * (-t * 8.0).exp()
            }
            // Prism Hart: airy crystal focus, followed by a bright rounded
            // shard sweep with a brief low anchor.
            18 => {
                let swell = (std::f32::consts::PI * u * 0.78).sin().max(0.0);
                (tone * 0.18 + tone_two * 0.17 + (phase * 1.997).sin() * 0.055)
                    * swell
                    + low * 0.10 * swell
            }
            19 => {
                let decay = (-t * 5.0).exp();
                (tone * 0.22 + tone_two * 0.15 + (phase * 2.01).sin() * 0.075)
                    * decay
                    + low * 0.13 * (-t * 9.0).exp()
            }
            20 => low * 1.2 * (-t * 8.0).exp() + tone * 0.6 + high * 0.3 * (-t * 12.0).exp(),
            21 => {
                // Rift: descending, softened harmonic pull with no noisy edge.
                (tone * 0.25 + tone_two * 0.18 + low * 0.38)
                    * (0.82 + 0.18 * (t * 11.0).sin())
            }
            22 => (high * 0.3 + metal * 0.35) * (std::f32::consts::PI * u).sin() + tone * 0.15,
            23 => {
                // Spray falling through a band over a damped low body:
                // the small-air-pocket "plip" that follows a body
                // breaking the surface.
                if i % 32 == 0 {
                    spray.tune(2000.0 - 1400.0 * u.sqrt(), 0.9, RATE as f32);
                }
                spray.band(white) * 0.75 * (-t * 9.0).exp()
                    + low * 1.25 * (-t * 6.0).exp()
                    + tone * 0.18 * (-t * 14.0).exp()
            }
            24 => low * 1.5 * (-t * 18.0).exp()
                + high * 0.32 * (-t * 70.0).exp()
                + tone * 0.20 * (-t * 28.0).exp(),
            25 => low * 1.8 * (-t * 3.0).exp()
                + soft_noise * 0.6 * (-t * 5.0).exp()
                + tone * 0.09 * (-t * 8.0).exp(),
            26 => {
                // Air rushes past: high noise swells then falls away, over a
                // short tonal thruster chirp and a faint metal ring.
                let rush = (std::f32::consts::PI * u.powf(0.6)).sin();
                high * 0.42 * rush
                    + soft_noise * 0.55 * rush
                    + (tone * 0.20 + metal * 0.06) * (-t * 11.0).exp()
            }
            27 => {
                // Grit: amplitude-modulated noise like boots on gravel, under a
                // low rumble. Starts hard, then settles.
                let grit = 0.55 + 0.45 * (t * 47.0 + (t * 13.0).sin() * 2.0).sin();
                high * 0.22 * grit * (-t * 3.0).exp()
                    + low * 1.1 * (-t * 2.2).exp()
                    + tone * 0.10 * (-t * 4.0).exp()
            }
            28 => {
                // A soft puff of thrust with a rising, rounded tone.
                soft_noise * 0.75 * (-t * 14.0).exp()
                    + high * 0.12 * (-t * 30.0).exp()
                    + (tone * 0.24 + tone_two * 0.08) * (std::f32::consts::PI * u).sin()
            }
            29 => {
                // Thump: a sub hit and a short gritty scuff.
                tone * 0.70 * (-t * 16.0).exp()
                    + low * 1.6 * (-t * 12.0).exp()
                    + high * 0.18 * (-t * 40.0).exp()
            }
            30 => {
                // Fast tremolo and odd harmonics give the nasal, wavering voice.
                let tremolo = 0.55 + 0.45 * (t * std::f32::consts::TAU * 13.0).sin();
                let voice = tone * 0.45 + (phase * 3.0).sin() * 0.22 + (phase * 5.0).sin() * 0.12;
                (voice * tremolo + soft_noise * 0.25) * (std::f32::consts::PI * u.sqrt()).sin()
            }
            _ => low * 0.8 + tone * 0.3,
        };
        data.push(sample * attack * tail * 0.65);
    }
    // A short cosine release removes the click that a hard sample end leaves.
    // Room tails come from the mixer's reverb, so cues stay dry here; fixed
    // baked echoes at 29 and 56 ms combed every cue the same way.
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
    fn movement_cues_have_their_own_character() {
        let bank = variants();
        let energy = |cue: Cue| {
            let s = &bank[cue as usize][0];
            s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32
        };
        for cue in [Cue::Dash, Cue::Slide, Cue::AirJump, Cue::Land] {
            assert!(energy(cue) > 1e-3, "{cue:?} is audible");
        }
        // The fallback branch would make every new cue the same sound.
        assert_ne!(bank[Cue::Dash as usize][0], bank[Cue::AirJump as usize][0]);
        assert_ne!(bank[Cue::Slide as usize][0].len(), bank[Cue::Land as usize][0].len());
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
