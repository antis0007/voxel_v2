//! Decaying debris re-entering over the valley: the "meteors" the lore says are
//! dead satellites coming down. A pure function of the orbital clock and the
//! world seed, so nothing is saved, reloads agree and no state accumulates.
//! Streaks appear only after dark, a few a minute, each lasting under two seconds.
use super::megastructures::{seconds, DAY_SECONDS};
use crate::world::{hash_u32, DayNightCycle, DEFAULT_DAY_LENGTH_SECONDS};
use glam::Vec3;
use std::f32::consts::TAU;

/// Length of a scheduling slot, real seconds. At most one streak starts per slot.
const SLOT: f64 = 20.0;
/// How long a streak lasts, real seconds.
pub const LIFE: f64 = 1.8;
/// Chance that a slot holds a streak.
const CHANCE: f32 = 0.55;
/// Distance of the streak's path from the viewer, metres.
pub const RANGE_M: f32 = 55_000.0;
/// Sun height below which streaks are drawn (twilight and night).
const DARK: f32 = 0.08;

#[derive(Clone, Copy, Debug)]
pub struct Streak {
    pub head: Vec3,
    pub tail: Vec3,
    /// 0 at birth and death, 1 at peak brightness.
    pub fade: f32,
}

fn unit(key: u32, salt: u32) -> f32 {
    (hash_u32(key ^ salt.wrapping_mul(0x9e37_79b9)) >> 8) as f32 / 16_777_216.0
}

/// Real seconds on the orbital clock: the sky runs `DAY_SECONDS` per default day.
fn real_seconds(day: &DayNightCycle) -> f64 {
    seconds(day) * DEFAULT_DAY_LENGTH_SECONDS as f64 / DAY_SECONDS
}

/// Streaks alive now, in the local chart around `viewer`.
pub fn streaks(seed: u32, day: &DayNightCycle, viewer: Vec3) -> Vec<Streak> {
    if day.lighting().sun_dir.y > DARK {
        return Vec::new();
    }
    let now = real_seconds(day);
    let mut out = Vec::new();
    let last = (now / SLOT).floor() as i64;
    // A streak that started in the previous slot can still be alive.
    for slot in [last - 1, last] {
        let key = hash_u32(seed ^ 0x4d45_5445 ^ (slot as u32).wrapping_mul(0x85eb_ca6b));
        if unit(key, 1) > CHANCE {
            continue;
        }
        let start = slot as f64 * SLOT + unit(key, 2) as f64 * (SLOT - LIFE);
        let age = now - start;
        if !(0.0..LIFE).contains(&age) {
            continue;
        }
        let progress = (age / LIFE) as f32;
        let azimuth = unit(key, 3) * TAU;
        let elevation = (20.0 + unit(key, 4) * 60.0).to_radians();
        let direction = Vec3::new(azimuth.sin() * elevation.cos(), elevation.sin(), -azimuth.cos() * elevation.cos());
        let east = direction.cross(Vec3::Y).normalize_or_zero();
        let north = direction.cross(east);
        let heading = unit(key, 5) * TAU;
        let travel = east * heading.cos() + north * heading.sin();
        let base = viewer + direction * RANGE_M;
        let length = RANGE_M * 0.5;
        out.push(Streak {
            head: base + travel * length * progress,
            tail: base + travel * length * (progress - 0.35).max(0.0),
            fade: (progress * std::f32::consts::PI).sin(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Midnight, then `real` seconds later at the default day pace.
    fn at(real: f32) -> DayNightCycle {
        let mut day = DayNightCycle::default();
        day.set_time_of_day(0.0);
        day.advance(real);
        day
    }

    #[test]
    fn none_by_day() {
        let mut day = DayNightCycle::default();
        day.set_time_of_day(0.5);
        for step in 0..2_000 {
            let mut d = day;
            d.advance(step as f32 * 0.05);
            if d.lighting().sun_dir.y > DARK {
                assert!(streaks(7, &d, Vec3::ZERO).is_empty());
            }
        }
    }

    #[test]
    fn a_few_a_minute_after_dark_each_short_lived_and_near() {
        let viewer = Vec3::new(120.0, 80.0, -40.0);
        let mut seen = std::collections::BTreeSet::new();
        let mut frames_with_streak = 0;
        let steps = 1_800;
        // The first 90 s after midnight (the night is a fifth of a 480 s day), every 50 ms.
        for i in 0..steps {
            let day = at(i as f32 * 0.05);
            assert!(day.lighting().sun_dir.y <= DARK, "test window must be dark");
            for s in streaks(7, &day, viewer) {
                frames_with_streak += 1;
                assert!((0.0..=1.0).contains(&s.fade), "{}", s.fade);
                assert!((s.head - viewer).length() < RANGE_M * 1.6, "head {:?}", s.head);
                assert!((s.head - s.tail).length() <= RANGE_M * 0.5 + 1.0);
                seen.insert((i as f64 * 0.05 / SLOT).floor() as i64);
            }
        }
        // 90 s is four and a half slots at a 55% chance: some events, never none.
        assert!((1..=5).contains(&seen.len()), "{} slots had a streak", seen.len());
        // Each event lasts under two seconds, so streak frames stay a small share.
        assert!((frames_with_streak as f64) < steps as f64 * 0.35, "{frames_with_streak}");
    }

    #[test]
    fn the_head_moves_steadily_and_never_jumps() {
        let viewer = Vec3::ZERO;
        let mut track: Vec<Vec3> = Vec::new();
        let mut last_seen = None;
        for i in 0..4_000 {
            let day = at(i as f32 * 0.02);
            if let Some(s) = streaks(7, &day, viewer).first() {
                if let (Some(prev), Some(i0)) = (track.last(), last_seen)
                    && i == i0 + 1
                {
                    // 1.8 s life over 27.5 km: ~15 km/s, so ~300 m per 20 ms.
                    let step = (s.head - *prev).length();
                    assert!(step < 500.0, "head jumped {step} m in one 20 ms frame");
                }
                track.push(s.head);
                last_seen = Some(i);
            }
        }
        assert!(!track.is_empty(), "no streak found in 80 s");
    }

    #[test]
    fn different_worlds_see_different_skies() {
        let day = at(3.0);
        let a: Vec<_> = (0..40).map(|i| streaks(1, &at(i as f32 * 1.0), Vec3::ZERO).len()).collect();
        let b: Vec<_> = (0..40).map(|i| streaks(2, &at(i as f32 * 1.0), Vec3::ZERO).len()).collect();
        let _ = day;
        assert_ne!(a, b);
    }
}
