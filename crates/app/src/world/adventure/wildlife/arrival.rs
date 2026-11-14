//! Terminal travel budget for a calm animal's direct wander goal.
//! No persistent state: collision/navigation still decides whether a step fits.
pub(super) fn step(requested: f32, remaining: f32, radius: f32) -> f32 {
    requested.min((remaining - radius).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_arrivals_never_cross_or_reverse_around_the_goal() {
        for speed in [0.9_f32, 1.5, 1.8, 8.0] {
            for dt in [1.0_f32 / 120.0, 1.0 / 60.0, 0.1] {
                let mut position = 0.0;
                for _ in 0..1600 {
                    let advance = step(speed * dt, 5.0 - position, 0.35);
                    assert!(advance >= 0.0 && advance <= speed * dt);
                    position += advance;
                    assert!(position <= 4.65 + 0.000001);
                }
                assert!((position - 4.65).abs() < 0.000001);
                assert!(step(speed * dt, 5.0 - position, 0.35) < 0.000001);
            }
        }
    }

    #[test]
    fn final_step_is_clamped_without_changing_travel_far_from_goal() {
        assert_eq!(step(0.15, 4.0, 0.35), 0.15);
        assert!((step(0.15, 0.40, 0.35) - 0.05).abs() < 0.000001);
        for remaining in [0.0, 0.1, 0.35] {
            assert_eq!(step(0.15, remaining, 0.35), 0.0);
        }
    }

    #[test]
    fn new_goal_resumes_without_a_latch() {
        assert_eq!(step(0.15, 0.35, 0.35), 0.0);
        assert_eq!(step(0.15, 8.0, 0.35), 0.15);
    }
}
