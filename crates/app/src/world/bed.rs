//! Beds: sleeping skips the night and sets the respawn point to the bed.
//! Sun angle comes from `DayNightCycle`: noon at 0.5, midnight at 0.0.
use super::DayNightCycle;
use glam::Vec3;

/// Sunrise: the first moment of morning, where a sleeper wakes.
pub const DAWN: f32 = 0.25;
/// Sunset: sleeping from here to midnight rolls into the next calendar day.
pub const DUSK: f32 = 0.75;

#[derive(Debug, PartialEq, Eq)]
pub enum SleepError {
    /// Beds only take sleepers after sunset.
    Daytime,
}

/// True while the sun is below the horizon.
pub fn is_night(time_of_day: f32) -> bool {
    let t = time_of_day.rem_euclid(1.0);
    !(DAWN..DUSK).contains(&t)
}

/// Sleep until dawn at `bed_feet`. Returns whether the calendar day advanced.
/// The respawn point changes only when sleep succeeds.
pub fn sleep(
    cycle: &mut DayNightCycle,
    respawn: &mut Option<Vec3>,
    bed_feet: Vec3,
) -> Result<bool, SleepError> {
    let t = cycle.time_of_day();
    if !is_night(t) {
        return Err(SleepError::Daytime);
    }
    let next_day = t >= DUSK;
    if next_day {
        // Field access: child module of `world`, and no getter exists at HEAD.
        cycle.set_day_index(cycle.day_index + 1);
    }
    cycle.set_time_of_day(DAWN);
    *respawn = Some(bed_feet);
    Ok(next_day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_covers_sunset_to_sunrise_only() {
        for t in [0.75, 0.9, 0.999, 0.0, 0.1, 0.2499] {
            assert!(is_night(t), "{t} should be night");
        }
        for t in [0.2501, 0.5, 0.7499] {
            assert!(!is_night(t), "{t} should be day");
        }
        assert!(is_night(-0.1), "negative times wrap into the night");
    }

    #[test]
    fn daytime_refuses_and_leaves_everything_unchanged() {
        let mut cycle = DayNightCycle::new(0.5, 480.0);
        let mut respawn = Some(Vec3::new(1.0, 2.0, 3.0));
        let before = respawn;
        assert_eq!(sleep(&mut cycle, &mut respawn, Vec3::ZERO), Err(SleepError::Daytime));
        assert_eq!(respawn, before);
        assert!((cycle.time_of_day() - 0.5).abs() < 1e-6);
        assert_eq!(cycle.day_index,0);
    }

    #[test]
    fn sleeping_after_sunset_skips_into_the_next_day() {
        let mut cycle = DayNightCycle::new(0.85, 480.0);
        cycle.set_day_index(4);
        let mut respawn = None;
        let bed = Vec3::new(10.0, 80.0, -6.0);
        assert_eq!(sleep(&mut cycle, &mut respawn, bed), Ok(true));
        assert_eq!(cycle.day_index,5);
        assert!((cycle.time_of_day() - DAWN).abs() < 1e-6);
        assert_eq!(respawn, Some(bed));
    }

    #[test]
    fn sleeping_after_midnight_stays_on_the_same_day() {
        let mut cycle = DayNightCycle::new(0.1, 480.0);
        cycle.set_day_index(4);
        let mut respawn = None;
        assert_eq!(sleep(&mut cycle, &mut respawn, Vec3::ZERO), Ok(false));
        assert_eq!(cycle.day_index,4);
        assert!((cycle.time_of_day() - DAWN).abs() < 1e-6);
    }
}
