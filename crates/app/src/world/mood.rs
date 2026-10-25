//! Passing sky events and the slow seasonal grade. Both are pure functions of
//! (weather seed, day index, time of day), so they never touch a save file and
//! two players on one seed see the same meteor shower at the same hour.
//!
//! A day is cut into six four-hour windows. Each window rolls once for an event
//! suited to its hour (night, dawn, day, dusk) and the event fades in and out
//! inside the window, so there is never a hard cut at a window edge.
use super::{LightingState, hash_u32, hash01, lerp_color, smoothstep};

const SLOTS_PER_DAY: f32 = 6.0;
const EVENT_CHANCE: f32 = 0.34;
/// Days per season. Four seasons make a twenty-day year.
const SEASON_DAYS: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkyEvent {
    MeteorShower,
    Aurora,
    BloodMoon,
    PearlFog,
    Rainbow,
    GoldenHaze,
    Emberglow,
    VioletHour,
}

impl SkyEvent {
    pub fn name(self) -> &'static str {
        match self {
            Self::MeteorShower => "Meteor shower",
            Self::Aurora => "Aurora",
            Self::BloodMoon => "Blood moon",
            Self::PearlFog => "Pearl fog",
            Self::Rainbow => "Rainbow",
            Self::GoldenHaze => "Golden haze",
            Self::Emberglow => "Emberglow",
            Self::VioletHour => "Violet hour",
        }
    }

    pub fn announcement(self) -> &'static str {
        match self {
            Self::MeteorShower => "Meteor shower",
            _ => "",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveEvent {
    pub kind: SkyEvent,
    /// 0..1 after the fade envelope and a per-event strength roll.
    pub intensity: f32,
}

const NIGHT: [SkyEvent; 3] = [
    SkyEvent::MeteorShower,
    SkyEvent::Aurora,
    SkyEvent::BloodMoon,
];
const DAWN: [SkyEvent; 3] = [SkyEvent::PearlFog, SkyEvent::GoldenHaze, SkyEvent::Rainbow];
const DAY: [SkyEvent; 2] = [SkyEvent::Rainbow, SkyEvent::GoldenHaze];
const DUSK: [SkyEvent; 3] = [SkyEvent::Emberglow, SkyEvent::VioletHour, SkyEvent::Aurora];

/// The event running at this moment, if any.
pub fn active_event(weather_seed: u32, day_index: u32, time_of_day: f32) -> Option<ActiveEvent> {
    let t = time_of_day.rem_euclid(1.0) * SLOTS_PER_DAY;
    let slot = (t.floor() as u32).min(5);
    let phase = t - slot as f32;
    let key = hash_u32(
        weather_seed
            ^ day_index.wrapping_mul(0x9E37_79B9)
            ^ slot.wrapping_mul(0x85EB_CA6B).rotate_left(7),
    );
    if hash01(key ^ 0x0A11_CE5E) >= EVENT_CHANCE {
        return None;
    }
    let pool: &[SkyEvent] = match slot {
        0 | 5 => &NIGHT,
        1 => &DAWN,
        2 | 3 => &DAY,
        _ => &DUSK,
    };
    let kind = pool[(hash01(key ^ 0x51DE_0001) * pool.len() as f32) as usize % pool.len()];
    let envelope = smoothstep(0.0, 0.20, phase) * (1.0 - smoothstep(0.70, 1.0, phase));
    let strength = 0.62 + 0.38 * hash01(key ^ 0xC0FF_EE11);
    let intensity = envelope * strength;
    (intensity > 0.01).then_some(ActiveEvent { kind, intensity })
}

/// Foliage and sky grade for one point in the year.
#[derive(Clone, Copy, Debug)]
pub struct SeasonGrade {
    pub grass: [f32; 3],
    pub foliage: [f32; 3],
    /// -1 cold and clean .. +1 warm and hazy. Nudges sky, fog and sun.
    pub warmth: f32,
}

const SEASONS: [SeasonGrade; 4] = [
    // Spring: lime shoots, washed clear air.
    SeasonGrade {
        grass: [0.96, 1.12, 0.80],
        foliage: [0.98, 1.14, 0.84],
        warmth: -0.15,
    },
    // Summer: deep and saturated, a little haze.
    SeasonGrade {
        grass: [0.94, 1.02, 0.86],
        foliage: [0.92, 1.00, 0.88],
        warmth: 0.30,
    },
    // Autumn: amber leaves, rust in the canopy, low gold light.
    SeasonGrade {
        grass: [1.18, 0.98, 0.70],
        foliage: [1.42, 0.84, 0.50],
        warmth: 0.75,
    },
    // Winter: frost-bleached, cool.
    SeasonGrade {
        grass: [0.90, 1.00, 1.10],
        foliage: [0.86, 0.98, 1.10],
        warmth: -0.70,
    },
];

pub fn season_grade(day_index: u32, time_of_day: f32) -> SeasonGrade {
    let year = (day_index as f32 + time_of_day.rem_euclid(1.0)) / SEASON_DAYS;
    let wrapped = year.rem_euclid(4.0);
    let a = wrapped.floor() as usize % 4;
    let b = (a + 1) % 4;
    let t = smoothstep(0.55, 1.0, wrapped - wrapped.floor());
    let (sa, sb) = (SEASONS[a], SEASONS[b]);
    SeasonGrade {
        grass: lerp_color(sa.grass, sb.grass, t),
        foliage: lerp_color(sa.foliage, sb.foliage, t),
        warmth: sa.warmth + (sb.warmth - sa.warmth) * t,
    }
}

impl LightingState {
    /// Layer the season and any running sky event over the weather-driven base.
    pub fn with_mood(mut self, weather_seed: u32, day_index: u32, time_of_day: f32) -> Self {
        let grade = season_grade(day_index, time_of_day);
        self.grass_tint = grade.grass;
        self.foliage_tint = grade.foliage;
        let day = self.daylight;
        // Warmth reads in the daytime horizon and the sun; it should not tint night.
        let warm = grade.warmth * day * 0.55;
        let push = |c: [f32; 3], amount: f32| -> [f32; 3] {
            [
                (c[0] * (1.0 + 0.10 * amount)).min(1.4),
                c[1] * (1.0 + 0.01 * amount),
                (c[2] * (1.0 - 0.14 * amount)).max(0.0),
            ]
        };
        self.sky_horizon = push(self.sky_horizon, warm);
        self.fog_color = push(self.fog_color, warm);
        self.sun_color = push(self.sun_color, warm * 0.6);

        let Some(event) = active_event(weather_seed, day_index, time_of_day) else {
            return self;
        };
        let i = event.intensity;
        match event.kind {
            SkyEvent::MeteorShower => {
                self.mood[2] = i;
                self.sky_top = lerp_color(self.sky_top, [0.02, 0.03, 0.10], 0.5 * i);
            }
            SkyEvent::Aurora => {
                self.mood[1] = i;
                self.fog_color = lerp_color(self.fog_color, [0.03, 0.16, 0.14], 0.22 * i);
            }
            SkyEvent::BloodMoon => {
                self.moon_color = lerp_color(self.moon_color, [0.90, 0.17, 0.10], 0.9 * i);
                self.moon_intensity *= 1.0 + 0.35 * i;
                self.sky_horizon = lerp_color(self.sky_horizon, [0.20, 0.03, 0.03], 0.55 * i);
                self.fog_color = lerp_color(self.fog_color, [0.16, 0.025, 0.025], 0.50 * i);
                self.mood_tint = [1.18, 0.62, 0.58, 0.55 * i];
            }
            SkyEvent::PearlFog => {
                let pearl = [0.80, 0.80, 0.86];
                self.fog_start *= 1.0 - 0.62 * i;
                self.fog_end *= 1.0 - 0.58 * i;
                self.fog_color = lerp_color(self.fog_color, pearl, 0.65 * i);
                self.sky_horizon = lerp_color(self.sky_horizon, pearl, 0.50 * i);
                self.saturation *= 1.0 - 0.14 * i;
            }
            SkyEvent::Rainbow => {
                self.mood[3] = i;
                self.saturation *= 1.0 + 0.08 * i;
            }
            SkyEvent::GoldenHaze => {
                let gold = [1.0, 0.76, 0.40];
                self.sky_horizon = lerp_color(self.sky_horizon, gold, 0.45 * i);
                self.fog_color = lerp_color(self.fog_color, gold, 0.50 * i);
                self.fog_end *= 1.0 - 0.28 * i;
                self.sun_color = lerp_color(self.sun_color, [1.0, 0.86, 0.62], 0.7 * i);
                self.saturation *= 1.0 + 0.10 * i;
                self.mood_tint = [1.12, 1.02, 0.84, 0.45 * i];
            }
            SkyEvent::Emberglow => {
                self.sky_horizon = lerp_color(self.sky_horizon, [1.0, 0.30, 0.10], 0.5 * i);
                self.sky_top = lerp_color(self.sky_top, [0.30, 0.09, 0.26], 0.30 * i);
                self.fog_color = lerp_color(self.fog_color, [0.70, 0.22, 0.10], 0.45 * i);
                self.mood_tint = [1.20, 0.82, 0.70, 0.50 * i];
            }
            SkyEvent::VioletHour => {
                self.sky_top = lerp_color(self.sky_top, [0.22, 0.09, 0.50], 0.55 * i);
                self.sky_horizon = lerp_color(self.sky_horizon, [0.88, 0.40, 0.70], 0.50 * i);
                self.fog_color = lerp_color(self.fog_color, [0.42, 0.22, 0.52], 0.45 * i);
                self.mood_tint = [1.02, 0.80, 1.22, 0.50 * i];
            }
        }
        self
    }
}

/// What hangs in the air at the player's position: fireflies on warm nights over
/// grass and hills, pollen on summer days over plains, diamond dust over snow, and
/// embers under a blood moon or emberglow dusk. Zero underground.
pub fn ambience(
    cfg: &super::WorldSampleConfig,
    eye_m: glam::Vec3,
    lighting: &LightingState,
    event: Option<ActiveEvent>,
) -> [f32; 4] {
    let scale = cfg.voxel_meters().max(1e-3);
    let w = super::biome_weights(
        cfg,
        (eye_m.x / scale).floor() as i32,
        (eye_m.z / scale).floor() as i32,
    );
    let day = lighting.daylight;
    let night = 1.0 - day;
    // Winter foliage runs blue; read the season from the tint rather than recompute it.
    let winter = smoothstep(
        0.10,
        0.30,
        lighting.foliage_tint[2] - lighting.foliage_tint[0],
    );
    let fireflies =
        smoothstep(0.55, 0.92, night) * (w.plains + w.hills * 0.7) * (1.0 - winter * 0.8);
    let pollen = day * day * w.plains * (1.0 - winter);
    let dust = day * w.snow;
    let embers = event
        .filter(|e| matches!(e.kind, SkyEvent::BloodMoon | SkyEvent::Emberglow))
        .map_or(0.0, |e| e.intensity * 0.8);
    [fireflies * 0.9, pollen * 0.6, dust * 0.9, embers]
}

/// Where the `n`th star of a shower lands, as a metre offset from the player.
/// Always 30 to 80 m away: far enough to be a trek, near enough to be found.
pub fn star_landing(weather_seed: u32, n: u32) -> (f32, f32) {
    let angle = hash01(weather_seed ^ n.wrapping_mul(0x9E37_79B9) ^ 0x57A7) * std::f32::consts::TAU;
    let dist = 30.0 + 50.0 * hash01(weather_seed ^ n.wrapping_mul(0x85EB_CA6B) ^ 0x1D1);
    (angle.sin() * dist, -angle.cos() * dist)
}

/// "A star fell to the north-east." North is -z, matching the compass strip.
pub fn compass_line(dx: f32, dz: f32) -> &'static str {
    let bearing = dx.atan2(-dz).to_degrees().rem_euclid(360.0);
    match ((bearing + 22.5) / 45.0) as usize % 8 {
        0 => "A star fell to the north.",
        1 => "A star fell to the north-east.",
        2 => "A star fell to the east.",
        3 => "A star fell to the south-east.",
        4 => "A star fell to the south.",
        5 => "A star fell to the south-west.",
        6 => "A star fell to the west.",
        _ => "A star fell to the north-west.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallen_stars_land_in_range_and_are_named_by_bearing() {
        for n in 0..200u32 {
            let (dx, dz) = star_landing(99, n);
            let d = dx.hypot(dz);
            assert!((29.9..=80.1).contains(&d), "{d}");
        }
        assert_eq!(compass_line(0.0, -40.0), "A star fell to the north.");
        assert_eq!(compass_line(40.0, 0.0), "A star fell to the east.");
        assert_eq!(compass_line(0.0, 40.0), "A star fell to the south.");
        assert_eq!(compass_line(-30.0, -30.0), "A star fell to the north-west.");
    }

    #[test]
    fn events_are_deterministic_and_fade_at_slot_edges() {
        let mut seen = 0;
        for day in 0..200u32 {
            for step in 0..240u32 {
                let t = step as f32 / 240.0;
                let a = active_event(0xA341_316C, day, t);
                assert_eq!(a, active_event(0xA341_316C, day, t));
                if let Some(e) = a {
                    seen += 1;
                    assert!((0.0..=1.0).contains(&e.intensity));
                }
            }
            // Window boundaries are exact multiples of 1/6 of a day.
            for slot in 1..6 {
                let edge = slot as f32 / 6.0;
                if let Some(e) = active_event(0xA341_316C, day, edge + 1e-5) {
                    assert!(e.intensity < 0.05, "event pops in at a window edge");
                }
            }
        }
        assert!(seen > 1000, "events should be common enough to meet");
        assert!(seen < 240 * 200 * 2 / 3, "events should not fill the sky");
    }

    #[test]
    fn events_match_the_hour() {
        for day in 0..400u32 {
            for step in 0..120u32 {
                let t = step as f32 / 120.0;
                if let Some(e) = active_event(7, day, t) {
                    let night = !(1.0 / 6.0..5.0 / 6.0).contains(&t);
                    if matches!(e.kind, SkyEvent::MeteorShower | SkyEvent::BloodMoon) {
                        assert!(night, "{:?} at t={t}", e.kind);
                    }
                    if matches!(
                        e.kind,
                        SkyEvent::GoldenHaze | SkyEvent::Rainbow | SkyEvent::PearlFog
                    ) {
                        assert!((1.0 / 6.0..4.0 / 6.0).contains(&t), "{:?} at t={t}", e.kind);
                    }
                }
            }
        }
    }

    #[test]
    fn seasons_blend_without_jumps() {
        let mut prev = season_grade(0, 0.0);
        for step in 1..4000u32 {
            let t = step as f32 * 0.01;
            let g = season_grade(t.floor() as u32, t.fract());
            for k in 0..3 {
                assert!((g.foliage[k] - prev.foliage[k]).abs() < 0.03);
            }
            prev = g;
        }
    }

    #[test]
    #[ignore = "prints review days for the default benchmark seed"]
    fn print_review_days() {
        use std::collections::HashMap;
        let mut best: HashMap<&'static str, (u32, f32, f32)> = HashMap::new();
        for day in 0..120u32 {
            for step in 0..96u32 {
                let t = step as f32 / 96.0;
                if let Some(e) = active_event(0xA341_316C, day, t) {
                    let entry = best.entry(e.kind.name()).or_insert((day, t, 0.0));
                    if e.intensity > entry.2 {
                        *entry = (day, t, e.intensity);
                    }
                }
            }
        }
        let mut rows: Vec<_> = best.into_iter().collect();
        rows.sort_by_key(|r| r.0);
        for (name, (day, t, i)) in rows {
            println!(
                "EVENT {name}: --benchmark-review-day={day} --benchmark-review-time={t:.4} (intensity {i:.2})"
            );
        }
    }
}
