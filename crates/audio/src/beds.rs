//! Continuous ambience beds, each cross-faded toward a level the game sets.
use crate::dsp::{advance, coefficient, sine, Noise, OnePole, Svf};
use crate::suit::Suit;

pub const BED_COUNT: usize = 7;

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
    /// Helmet fan, life-support hum and breathing where the air is too thin to carry sound.
    Suit,
    /// Express lift drive: a hum that climbs with speed and a wash that follows load.
    Drive,
}

impl Bed {
    pub const ALL: [Bed; BED_COUNT] = [Bed::Wind, Bed::Cave, Bed::Water, Bed::Rain, Bed::Insects, Bed::Suit, Bed::Drive];
}

/// Below this level a bed is skipped entirely, so silent beds cost nothing.
const AUDIBLE: f32 = 1.0e-4;
/// Cross-fade time constant in seconds.
const FADE_S: f32 = 2.5;

pub(crate) struct Beds {
    pub(crate) room_send: f32,
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
    /// Resonant whistle per channel; strong wind on an exposed edge howls.
    howl: [Svf; 2],
    cave: [f32; 3],
    cave_breath: f32,
    cave_low: OnePole,
    drip_wait: u32,
    drip_age: u32,
    drip_phase: f32,
    drip_hz: f32,
    drip_pan: f32,
    /// Band and ceiling filters per channel.
    water: [Svf; 4],
    swell: [OnePole; 2],
    bubbles: [Bubble; 4],
    /// Wash band and ceiling per channel, then the drop resonance.
    rain: [Svf; 5],
    crackle: f32,
    crackle_pan: f32,
    insect: [f32; 4],
    /// Hurt or exerted, 0 calm to 1; breathing quickens and roughens with it.
    pub(crate) strain: f32,
    suit: Suit,
    /// Fixed filter coefficients: cave rumble, water swell.
    cuts: [f32; 2],
    /// Lift speed and signed drive load, each -1..1 or 0..1, set by the game.
    pub(crate) drive_speed: f32,
    pub(crate) drive_load: f32,
    /// Hum fundamental and the slow beat between its two motors, in turns.
    drive: [f32; 3],
    /// Wash band per channel, then the braking whine.
    drive_bands: [Svf; 3],
    drive_smooth: [f32; 2],
}

/// One stream bubble: a short sine whose pitch rises as its cavity closes.
#[derive(Clone, Copy, Default)]
struct Bubble {
    phase: f32,
    hz: f32,
    age: u32,
    gain: f32,
    pan: f32,
}

/// Bubbles started per second in the water bed.
const BUBBLES_PER_S: f32 = 26.0;

impl Beds {
    pub(crate) fn new(rate: u32) -> Self {
        let rate = rate as f32;
        Self {
            room_send: 0.0,
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
            howl: [Svf::new(620.0, 9.0, rate), Svf::new(660.0, 9.0, rate)],
            cave: [0.0; 3],
            cave_breath: 0.0,
            cave_low: OnePole::default(),
            drip_wait: rate as u32,
            drip_age: u32::MAX,
            drip_phase: 0.0,
            drip_hz: 900.0,
            drip_pan: 0.0,
            water: [
                Svf::new(520.0, 0.6, rate),
                Svf::new(520.0, 0.6, rate),
                Svf::new(1800.0, 0.707, rate),
                Svf::new(1800.0, 0.707, rate),
            ],
            swell: [OnePole::default(); 2],
            bubbles: [Bubble { age: u32::MAX, ..Bubble::default() }; 4],
            rain: [
                Svf::new(1800.0, 0.5, rate),
                Svf::new(1800.0, 0.5, rate),
                Svf::new(4500.0, 0.707, rate),
                Svf::new(4500.0, 0.707, rate),
                Svf::new(2500.0, 2.0, rate),
            ],
            crackle: 0.0,
            crackle_pan: 0.0,
            insect: [0.0, 0.0, 0.0, 0.5],
            strain: 0.0,
            drive_speed: 0.0,
            drive_load: 0.0,
            drive: [0.0, 0.0, 0.0],
            drive_bands: [Svf::new(400.0, 1.2, rate), Svf::new(420.0, 1.2, rate), Svf::new(2400.0, 6.0, rate)],
            drive_smooth: [0.0; 2],
            suit: Suit::new(rate),
            cuts: [90.0, 7.0].map(|hz| coefficient(hz, rate)),
        }
    }

    /// Current cross-faded level of each bed.
    #[cfg(test)]
    pub(crate) fn levels(&self) -> [f32; BED_COUNT] {
        self.level
    }

    #[cfg(test)]
    pub(crate) fn level_mut(&mut self) -> &mut [f32; BED_COUNT] {
        &mut self.level
    }

    pub(crate) fn frame(&mut self) -> [f32; 2] {
        for (level, target) in self.level.iter_mut().zip(self.target) {
            *level += (target - *level) * self.fade;
        }
        self.frame = self.frame.wrapping_add(1);
        let (l, r) = (self.left.next(), self.right.next());
        let mut out = [0.0f32; 2];
        let mut send = 0.0;
        let mut add = |gain: f32, pair: [f32; 2], enclosed: bool| {
            out[0] += pair[0] * gain;
            out[1] += pair[1] * gain;
            if enclosed { send += (pair[0] + pair[1]) * 0.5 * gain; }
        };
        let [wind, cave, water, rain, insects, suit, drive] = self.level;
        if wind > AUDIBLE {
            let pair = self.wind(l, r, wind);
            add(wind, pair, false);
        }
        if cave > AUDIBLE {
            add(cave, self.cave(l), true);
        }
        if water > AUDIBLE {
            add(water, self.water(l, r), true);
        }
        if rain > AUDIBLE {
            add(rain, self.rain(l, r), false);
        }
        if insects > AUDIBLE {
            add(insects, self.insects(), false);
        }
        if suit > AUDIBLE {
            // Inside the helmet: dry, never in the room.
            add(suit, self.suit.frame(self.strain), false);
        }
        if drive > AUDIBLE {
            add(drive, self.drive(l, r), true);
        }
        self.room_send = send;
        out
    }

    fn wind(&mut self, l: f32, r: f32, level: f32) -> [f32; 2] {
        // The cutoff exp() runs at 1/64 of the sample rate; gusts are slow.
        if self.frame.is_multiple_of(64) {
            advance(&mut self.gust[0], 0.071 * 64.0, self.rate);
            advance(&mut self.gust[1], 0.173 * 64.0, self.rate);
            self.gust_now = 0.55 + 0.30 * sine(self.gust[0]) + 0.15 * sine(self.gust[1]);
            self.wind_cut = coefficient(140.0 + 760.0 * self.gust_now, self.rate);
            // The whistle bends up with each gust, a little apart in each ear.
            let hz = 420.0 + 520.0 * self.gust_now;
            self.howl[0].tune(hz, 9.0, self.rate);
            self.howl[1].tune(hz * 1.06, 9.0, self.rate);
        }
        let a = self.wind_cut;
        let first = [self.wind[0].low(l, a), self.wind[2].low(r, a)];
        let left = self.wind[1].low(first[0], a);
        let right = self.wind[3].low(first[1], a);
        let g = self.gust_now * self.gust_now * 2.4;
        // Only loud wind whistles; a calm field keeps the plain rush.
        let howl = ((level - 0.6) / 0.4).clamp(0.0, 1.0) * self.gust_now * 0.5;
        if howl <= 0.0 {
            return [left * g, right * g];
        }
        [left * g + self.howl[0].band(l) * howl, right * g + self.howl[1].band(r) * howl]
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
        let babble = self.cuts[1];
        let (band_l, band_r) = (self.water[0].band(l), self.water[1].band(r));
        let (band_l, band_r) = (self.water[2].low(band_l), self.water[3].low(band_r));
        // Slow random swells make the band read as moving water, not hiss.
        let swell_l = 0.55 + self.swell[0].low(r, babble).abs() * 22.0;
        let swell_r = 0.55 + self.swell[1].low(l, babble).abs() * 22.0;
        let mut out = [band_l * swell_l * 0.8, band_r * swell_r * 0.8];
        // Running water is mostly many small bubbles closing at once.
        if self.events.unit() < BUBBLES_PER_S / self.rate {
            let pick = (self.events.unit() * 4.0) as usize % 4;
            let size = self.events.unit();
            self.bubbles[pick] = Bubble {
                phase: 0.0,
                hz: 280.0 + size * size * 900.0,
                age: 0,
                gain: 0.03 + 0.05 * self.events.unit(),
                pan: self.events.next() * 0.8,
            };
        }
        for bubble in &mut self.bubbles {
            if bubble.age == u32::MAX {
                continue;
            }
            let t = bubble.age as f32 / self.rate;
            advance(&mut bubble.phase, bubble.hz * (1.0 + t * 14.0).min(2.0), self.rate);
            let v = sine(bubble.phase) * bubble.gain * (t / 0.001).min(1.0) * (-t * 55.0).exp();
            out[0] += v * (1.0 - bubble.pan) * 0.5;
            out[1] += v * (1.0 + bubble.pan) * 0.5;
            bubble.age = if t > 0.09 { u32::MAX } else { bubble.age + 1 };
        }
        out
    }

    fn rain(&mut self, l: f32, r: f32) -> [f32; 2] {
        let (wash_l, wash_r) = (self.rain[0].band(l), self.rain[1].band(r));
        let (wash_l, wash_r) = (self.rain[2].low(wash_l), self.rain[3].low(wash_r));
        if self.events.unit() < 0.0009 {
            self.crackle = 0.25 + self.events.unit() * 0.35;
            self.crackle_pan = self.events.next() * 0.8;
        }
        self.crackle *= 0.985;
        // A drop is a short knock, not a raw noise click.
        let tick = self.rain[4].band(self.crackle * l) * 1.2;
        [
            wash_l * 0.32 + tick * (1.0 - self.crackle_pan) * 0.5,
            wash_r * 0.32 + tick * (1.0 + self.crackle_pan) * 0.5,
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

impl Beds {
    /// Lift drive. The hum sits an octave lower at rest and climbs two octaves
    /// with speed; the wash follows load magnitude; braking adds a thin whine.
    fn drive(&mut self, l: f32, r: f32) -> [f32; 2] {
        // Speed and load glide so a phase edge in the profile never clicks.
        let glide = 0.0008;
        self.drive_smooth[0] += (self.drive_speed.clamp(0.0, 1.0) - self.drive_smooth[0]) * glide;
        self.drive_smooth[1] += (self.drive_load.clamp(-1.0, 1.0) - self.drive_smooth[1]) * glide;
        let [speed, load] = self.drive_smooth;
        let hz = 48.0 * (1.0 + 3.0 * speed);
        advance(&mut self.drive[0], hz, self.rate);
        advance(&mut self.drive[1], hz * 1.503, self.rate);
        advance(&mut self.drive[2], 0.6 + 1.1 * speed, self.rate);
        let beat = 0.85 + 0.15 * sine(self.drive[2]);
        let hum = (sine(self.drive[0]) * 0.55 + sine(self.drive[0] * 2.0) * 0.2 + sine(self.drive[1]) * 0.18)
            * (0.05 + 0.07 * speed) * beat;
        if self.frame.is_multiple_of(256) {
            let centre = 320.0 + 1400.0 * speed;
            self.drive_bands[0].tune(centre, 1.2, self.rate);
            self.drive_bands[1].tune(centre * 1.04, 1.2, self.rate);
        }
        let wash_gain = 0.02 + 0.16 * load.abs();
        let wash = [self.drive_bands[0].band(l) * wash_gain, self.drive_bands[1].band(r) * wash_gain];
        let braking = (-load).clamp(0.0, 1.0);
        let whine = self.drive_bands[2].band((l + r) * 0.5) * 0.05 * braking;
        [hum + wash[0] + whine, hum + wash[1] + whine]
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
    fn strain_quickens_the_breath() {
        let breaths = |strain: f32| {
            let mut beds = Beds::new(48_000);
            beds.strain = strain;
            let mut wraps = 0;
            let mut last = 0.0;
            for _ in 0..48_000 * 10 {
                beds.suit.frame(beds.strain);
                let phase = beds.suit.breath_phase();
                if phase < last { wraps += 1; }
                last = phase;
            }
            wraps
        };
        assert!(breaths(1.0) > breaths(0.0) * 2, "{} {}", breaths(1.0), breaths(0.0));
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
