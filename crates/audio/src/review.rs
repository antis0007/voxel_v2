//! Spectral checks on every rendered sound, plus a WAV dump for listening.
//!
//! `VOXEL_AUDIO_DUMP=<dir> cargo test -p engine_audio dump_audio_wav -- --ignored`
//! writes each step, bed and cue at its in-game level.
use crate::beds::{Bed, Beds};
use crate::reverb::Reverb;
use crate::steps::{bank, index, Contact, Surface, VARIANTS};
use crate::synth;

const RATE: u32 = 48_000;

/// Spectral centroid in hertz and the energy share above 5 kHz.
pub(crate) fn brightness(samples: &[f32]) -> (f32, f32) {
    let n = 4096;
    let window: Vec<f32> = (0..n)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos())
        .collect();
    let mut power = vec![0.0f64; n / 2];
    let mut start = 0;
    while start < samples.len() {
        let mut re: Vec<f32> = (0..n).map(|i| samples.get(start + i).copied().unwrap_or(0.0) * window[i]).collect();
        let mut im = vec![0.0f32; n];
        fft(&mut re, &mut im);
        for (k, p) in power.iter_mut().enumerate() {
            *p += f64::from(re[k] * re[k] + im[k] * im[k]);
        }
        start += n / 2;
    }
    let hz = |k: usize| k as f64 * f64::from(RATE) / n as f64;
    let total: f64 = power.iter().sum::<f64>().max(1.0e-12);
    let centroid = power.iter().enumerate().map(|(k, p)| hz(k) * p).sum::<f64>() / total;
    let high = power.iter().enumerate().filter(|(k, _)| hz(*k) > 5000.0).map(|(_, p)| p).sum::<f64>() / total;
    (centroid as f32, high as f32)
}

fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -std::f32::consts::TAU / len as f32;
        for chunk in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (angle * k as f32).sin_cos();
                let (a, b) = (chunk + k, chunk + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

fn bed_render(bed: Bed, seconds: u32) -> Vec<[f32; 2]> {
    let mut beds = Beds::new(RATE);
    beds.target[bed as usize] = 1.0;
    beds.level_mut()[bed as usize] = 1.0;
    (0..RATE * seconds).map(|_| beds.frame()).collect()
}

fn left(frames: &[[f32; 2]]) -> Vec<f32> {
    frames.iter().map(|f| f[0]).collect()
}

/// Every sound with its name, mono, at its in-game mix level.
fn catalogue() -> Vec<(String, Vec<f32>)> {
    let effects = 0.65;
    let mut out = Vec::new();
    let steps = bank(RATE);
    for contact in Contact::ALL {
        let level = match contact {
            Contact::Step => 1.0,
            Contact::Hit => 0.5,
            Contact::Break => 1.6,
            Contact::Place => 1.3,
        };
        for surface in Surface::ALL {
            for variant in 0..VARIANTS {
                let render = &steps[index(contact, surface, variant)];
                let name = format!("{}_{surface:?}_{variant}", format!("{contact:?}").to_lowercase());
                out.push((name, render.iter().map(|x| x * 0.35 * level * effects).collect()));
            }
        }
    }
    for bed in Bed::ALL {
        out.push((format!("bed_{bed:?}"), left(&bed_render(bed, 6)).iter().map(|x| x * 0.6).collect()));
    }
    for family in [crate::Family::Whistle, crate::Family::Trill, crate::Family::Chatter, crate::Family::Caw] {
        let mut chorus = crate::fauna::Chorus::new(RATE);
        let voice = crate::Fauna { birds: 1.0, bird_hz: 3000.0, calls_per_s: 2.0, trill: 0.5, frogs: 0.0, frog_hz: 300.0, family };
        chorus.set(voice, crate::Placement { canopy: [1.0; 8], ..Default::default() });
        let frames: Vec<[f32; 2]> = (0..RATE * 8).map(|_| chorus.frame()).collect();
        out.push((format!("fauna_{family:?}").to_lowercase(), left(&frames).iter().map(|x| x * 0.6).collect()));
    }
    let cues = synth::variants();
    for (index, cue) in cues.iter().enumerate() {
        out.push((format!("cue_{index:02}"), cue[0].iter().map(|x| x * 0.62 * effects).collect()));
    }
    out
}

fn write_wav(path: &std::path::Path, samples: &[f32]) -> std::io::Result<()> {
    let pcm: Vec<i16> = samples.iter().map(|x| (x.clamp(-1.0, 1.0) * 24000.0) as i16).collect();
    crate::tap::write_wav(path, 1, RATE, &pcm)
}

#[test]
#[ignore = "writes WAV files; set VOXEL_AUDIO_DUMP"]
fn dump_audio_wav() {
    let Some(dir) = std::env::var_os("VOXEL_AUDIO_DUMP") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, samples) in catalogue() {
        let (centroid, high) = brightness(&samples);
        let peak = samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        println!("{name:<16} centroid {centroid:>6.0} Hz  >5k {:>5.1}%  peak {peak:.3}  rms {rms:.4}", high * 100.0);
        write_wav(&dir.join(format!("{name}.wav")), &samples).unwrap();
    }
    // A walk: eight strides on each surface at 2 strides per second.
    let steps = bank(RATE);
    for surface in Surface::ALL {
        let mut walk = vec![0.0f32; RATE as usize * 4];
        for stride in 0..8 {
            let render = &steps[index(Contact::Step, surface, crate::hash(stride, surface as u32) as usize)];
            let at = stride as usize * RATE as usize / 2;
            for (i, x) in render.iter().enumerate() {
                walk[at + i] += x * 0.35 * 0.65;
            }
        }
        write_wav(&dir.join(format!("walk_{surface:?}.wav")), &walk).unwrap();
    }
    write_wav(&dir.join("room_open_break_stone.wav"), &in_room(0.0)).unwrap();
    write_wav(&dir.join("room_cave_break_stone.wav"), &in_room(1.0)).unwrap();
}

/// A break on stone, dry plus the room the mixer adds, for one enclosure.
fn in_room(space: f32) -> Vec<f32> {
    let steps = bank(RATE);
    let dry = &steps[index(Contact::Break, Surface::Stone, 0)];
    let mut reverb = Reverb::new(RATE);
    reverb.approach(space, 1.0);
    (0..RATE as usize * 2)
        .map(|i| {
            let x = dry.get(i).copied().unwrap_or(0.0) * 0.35 * 1.6 * 0.65;
            x + reverb.process(x * 0.5)[0]
        })
        .collect()
}

#[test]
fn caves_add_an_audible_tail_and_open_air_stays_nearly_dry() {
    let tail = |space: f32| {
        let mix = in_room(space);
        let rms = |r: std::ops::Range<usize>| (mix[r.clone()].iter().map(|x| x * x).sum::<f32>() / r.len() as f32).sqrt();
        let dry_len = bank(RATE)[index(Contact::Break, Surface::Stone, 0)].len();
        20.0 * (rms(dry_len..dry_len + 24_000) / rms(0..dry_len)).log10()
    };
    let (open, cave) = (tail(0.0), tail(1.0));
    println!("tail after the dry sound: open {open:.1} dB, cave {cave:.1} dB");
    assert!(open < -24.0, "open air tail {open:.1} dB");
    assert!((-18.0..-6.0).contains(&cave), "cave tail {cave:.1} dB");
}

#[test]
fn footsteps_and_water_stay_soft_on_top() {
    for (name, samples) in catalogue() {
        let (centroid, high) = brightness(&samples);
        let water = name.contains("Water") || name == "cue_23" || name.starts_with("bed_Rain");
        let contact = ["step_", "hit_", "break_", "place_"].iter().any(|p| name.starts_with(p));
        if contact || water {
            assert!(high < 0.12, "{name}: {:.1}% of energy above 5 kHz", high * 100.0);
            assert!(centroid < 3000.0, "{name}: centroid {centroid:.0} Hz");
        }
    }
}

#[test]
fn white_noise_reads_as_bright() {
    let mut noise = crate::dsp::Noise::new(7);
    let samples: Vec<f32> = (0..RATE).map(|_| noise.next()).collect();
    let (centroid, high) = brightness(&samples);
    assert!(centroid > 10_000.0 && high > 0.7, "{centroid} {high}");
}
