//! Continuous ambience beds, each cross-faded toward a level the game sets.
use crate::dsp::{advance, coefficient, sine, Noise, OnePole};

pub const BED_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bed {
    /// Outdoor air; the game raises it with altitude and storms.
    Wind,
    /// Low drone and water drips in enclosed rock.
    Cave,
    /// Flowing water near rivers, lakes and the sea.
    Water,
    Rain,
    /// Night insects outdoors.
    Insects,
}

impl Bed {
    pub const ALL: [Bed; BED_COUNT] = [Bed::Wind, Bed::Cave, Bed::Water, Bed::Rain, Bed::Insects];
}

/// Below this level a bed is skipped entirely, so silent beds cost nothing.
const AUDIBLE: f32 = 1.0e-4;
/// Cross-fade time constant in seconds.
const FADE_S: f32 = 2.5;

pub(crate) struct Beds {
    rate: f32,
    pub(crate) target: [f32; BED_COUNT],
    level: [f32; BED_COUNT],
    fade: f32,
    left: Noise,
    right: Noise,
    events: Noise,
    frame: u32,
    gust: [f32; 2],
    gust_now: f32,
    wind_cut: f32,
    wind: [OnePole; 4],
    cave: [f32; 3],
    cave_breath: f32,
    cave_low: OnePole,
    drip_wait: u32,
    drip_age: u32,
    drip_phase: f32,
    drip_hz: f32,
    drip_pan: f32,
    water: [OnePole; 6],
    rain: [OnePole; 2],
    crackle: f32,
    crackle_pan: f32,
    insect: [f32; 4],
    /// Fixed filter coefficients: cave rumble, water band high and low, water swell, rain.
    cuts: [f32; 5],
}

impl Beds {
    pub(crate) fn new(rate: u32) -> Self {
        let rate = rate as f32;
        Self {
            rate,
            target: [0.0; BED_COUNT],
            level: [0.0; BED_COUNT],
            fade: 1.0 - (-1.0 / (FADE_S * rate)).exp(),
            left: Noise::new(0x51ed_270b),
            right: Noise::new(0x2545_f491),
            events: Noise::new(0x9e37_79b9),
            frame: 0,
            gust: [0.0, 0.37],
            gust_now: 0.5,
            wind_cut: coefficient(300.0, rate),
            wind: [OnePole::default(); 4],
            cave: [0.0; 3],
            cave_breath: 0.0,
            cave_low: OnePole::default(),
            drip_wait: rate as u32,
            drip_age: u32::MAX,
            drip_phase: 0.0,
            drip_hz: 900.0,
            drip_pan: 0.0,
            water: [OnePole::default(); 6],
            rain: [OnePole::default(); 2],
            crackle: 0.0,
            crackle_pan: 0.0,
            insect: [0.0, 0.0, 0.0, 0.5],
            cuts: [90.0, 1400.0, 260.0, 7.0, 2400.0].map(|hz| coefficient(hz, rate)),
        }
    }

    /// Current cross-faded level of each bed.
    #[cfg(test)]
    pub(crate) fn levels(&self) -> [f32; BED_COUNT] {
        self.level
    }

    pub(crate) fn frame(&mut self) -> [f32; 2] {
        for (level, target) in self.level.iter_mut().zip(self.target) {
            *level += (target - *level) * self.fade;
        }
        self.frame = self.frame.wrapping_add(1);
        let (l, r) = (self.left.next(), self.right.next());
        let mut out = [0.0f32; 2];
        let mut add = |gain: f32, pair: [f32; 2]| {
            out[0] += pair[0] * gain;
            out[1] += pair[1] * gain;
        };
        let [wind, cave, water, rain, insects] = self.level;
        if wind > AUDIBLE {
            add(wind, self.wind(l, r));
        }
        if cave > AUDIBLE {
            add(cave, self.cave(l));
        }
        if water > AUDIBLE {
            add(water, self.water(l, r));
        }
        if rain > AUDIBLE {
            add(rain, self.rain(l, r));
        }
        if insects > AUDIBLE {
            add(insects, self.insects());
        }
        out
    }

    fn wind(&mut self, l: f32, r: f32) -> [f32; 2] {
        // The cutoff exp() runs at 1/64 of the sample rate; gusts are slow.
        if self.frame.is_multiple_of(64) {
            advance(&mut self.gust[0], 0.071 * 64.0, self.rate);
            advance(&mut self.gust[1], 0.173 * 64.0, self.rate);
            self.gust_now = 0.55 + 0.30 * sine(self.gust[0]) + 0.15 * sine(self.gust[1]);
            self.wind_cut = coefficient(140.0 + 760.0 * self.gust_now, self.rate);
        }
        let a = self.wind_cut;
        let first = [self.wind[0].low(l, a), self.wind[2].low(r, a)];
        let left = self.wind[1].low(first[0], a);
        let right = self.wind[3].low(first[1], a);
        let g = self.gust_now * self.gust_now * 2.4;
        [left * g, right * g]
    }

    fn cave(&mut self, l: f32) -> [f32; 2] {
        advance(&mut self.cave[0], 49.0, self.rate);
        advance(&mut self.cave[1], 73.6, self.rate);
        advance(&mut self.cave[2], 98.3, self.rate);
        advance(&mut self.cave_breath, 0.05, self.rate);
        let breath = 0.75 + 0.25 * sine(self.cave_breath);
        let drone = (sine(self.cave[0]) * 0.5 + sine(self.cave[1]) * 0.3 + sine(self.cave[2]) * 0.12)
            * breath
            * 0.16;
        let rumble = self.cave_low.low(l, self.cuts[0]) * 1.6;
        let mut drip = 0.0;
        if self.drip_age == u32::MAX {
            self.drip_wait = self.drip_wait.saturating_sub(1);
            if self.drip_wait == 0 {
                self.drip_age = 0;
                self.drip_hz = 700.0 + self.events.unit() * 900.0;
                self.drip_pan = self.events.next() * 0.7;
                self.drip_wait = ((0.6 + self.events.unit() * 3.4) * self.rate) as u32;
            }
        } else {
            let t = self.drip_age as f32 / self.rate;
            // A drop rises in pitch as the bubble it traps collapses.
            advance(&mut self.drip_phase, self.drip_hz * (1.0 + t * 18.0).min(2.2), self.rate);
            drip = sine(self.drip_phase) * (-t * 38.0).exp() * 0.32;
            self.drip_age += 1;
            if t > 0.18 {
                self.drip_age = u32::MAX;
            }
        }
        let mono = drone + rumble;
        [
            mono + drip * (1.0 - self.drip_pan) * 0.5,
            mono + drip * (1.0 + self.drip_pan) * 0.5,
        ]
    }

    fn water(&mut self, l: f32, r: f32) -> [f32; 2] {
        let [_, hi, lo, babble, _] = self.cuts;
        let band_l = self.water[0].low(l, hi) - self.water[1].low(l, lo);
        let band_r = self.water[2].low(r, hi) - self.water[3].low(r, lo);
        // Slow random swells make the band read as moving water, not hiss.
        let swell_l = 0.55 + self.water[4].low(r, babble).abs() * 22.0;
        let swell_r = 0.55 + self.water[5].low(l, babble).abs() * 22.0;
        [band_l * swell_l * 0.9, band_r * swell_r * 0.9]
    }

    fn rain(&mut self, l: f32, r: f32) -> [f32; 2] {
        let a = self.cuts[4];
        let hiss_l = l - self.rain[0].low(l, a);
        let hiss_r = r - self.rain[1].low(r, a);
        if self.events.unit() < 0.0009 {
            self.crackle = 0.25 + self.events.unit() * 0.35;
            self.crackle_pan = self.events.next() * 0.8;
        }
        self.crackle *= 0.985;
        let tick = self.crackle * l;
        [
            hiss_l * 0.16 + tick * (1.0 - self.crackle_pan) * 0.5,
            hiss_r * 0.16 + tick * (1.0 + self.crackle_pan) * 0.5,
        ]
    }

    fn insects(&mut self) -> [f32; 2] {
        advance(&mut self.insect[0], 4300.0, self.rate);
        advance(&mut self.insect[1], 3650.0, self.rate);
        advance(&mut self.insect[2], 0.83, self.rate);
        advance(&mut self.insect[3], 0.61, self.rate);
        // Each insect pulses at about 20 Hz in a burst over part of its cycle.
        let chirp = |group: f32, rate: f32| {
            if group > 0.32 {
                return 0.0;
            }
            let pulse = sine(group * rate).max(0.0);
            pulse * pulse
        };
        let a = sine(self.insect[0]) * chirp(self.insect[2], 24.0) * 0.17;
        let b = sine(self.insect[1]) * chirp(self.insect[3], 19.0) * 0.15;
        [a * 0.8 + b * 0.25, a * 0.25 + b * 0.8]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(bed: Bed) -> f32 {
        let mut beds = Beds::new(48_000);
        beds.target[bed as usize] = 1.0;
        beds.level[bed as usize] = 1.0;
        let mut sum = 0.0f64;
        let frames = 48_000 * 4;
        for _ in 0..frames {
            let [l, r] = beds.frame();
            assert!(l.is_finite() && r.is_finite());
            sum += (l * l + r * r) as f64 * 0.5;
        }
        (sum / frames as f64).sqrt() as f32
    }

    #[test]
    fn every_bed_sits_in_a_quiet_background_range() {
        for bed in Bed::ALL {
            let level = rms(bed);
            println!("{bed:?} rms {level:.4}");
            assert!((0.012..0.2).contains(&level), "{bed:?} rms {level}");
        }
    }

    #[test]
    fn silent_beds_output_silence_and_levels_cross_fade() {
        let mut beds = Beds::new(48_000);
        for _ in 0..4800 {
            assert_eq!(beds.frame(), [0.0, 0.0]);
        }
        beds.target[Bed::Wind as usize] = 1.0;
        for _ in 0..48_000 {
            beds.frame();
        }
        let after_one_second = beds.levels()[Bed::Wind as usize];
        assert!((0.2..0.5).contains(&after_one_second), "fade {after_one_second}");
    }
}
