//! Distance-based ground gait and fixed-length limb joints.
use glam::Vec3;
use std::f32::consts::{PI, TAU};

pub(crate) fn step(phase: f32, stride: f32, height: f32, weight: f32) -> (f32, f32) {
    step_with_stance(phase, stride, height, weight, 0.6)
}

fn step_with_stance(phase: f32, stride: f32, height: f32, weight: f32, stance: f32) -> (f32, f32) {
    let t = phase.rem_euclid(TAU) / TAU;
    let weight = weight.clamp(0.0, 1.0);
    if t < stance {
        ((t / stance * 2.0 - 1.0) * stride * weight, 0.0)
    } else {
        let swing = (t - stance) / (1.0 - stance);
        let ease = swing * swing * (3.0 - 2.0 * swing);
        ((1.0 - ease * 2.0) * stride * weight,
            (swing * PI).sin().powi(2) * height * weight)
    }
}

pub(super) fn distance_step(distance: f32, stride: f32, height: f32, offset: f32, weight: f32) -> (f32, f32) {
    step(distance * (PI * 0.6 / stride) + offset, stride, height, weight)
}

/// Four-beat lateral walk: three feet support the body while one swings.
/// Order is front-left, front-right, rear-left, rear-right. The cadence keeps
/// planted feet stationary in world space, independent of stride or gait fade.
/// Horizontal stance is distance-owned: fading it towards a neutral pose drags
/// planted hooves. Only lift fades on stopping; feet retain their last stance.
pub(crate) fn hoof_step(distance: f32, stride: f32, height: f32, leg: usize, weight: f32) -> (f32, f32) {
    const STANCE: f32 = 0.75;
    let offset = [0.0, 0.5, 0.25, 0.75][leg] * TAU;
    let (travel, lift) = step_with_stance(
        distance * (PI * STANCE / stride) + offset, stride, height, 1.0, STANCE);
    (travel, lift * weight.clamp(0.0, 1.0))
}

pub(super) fn ground_motion(movement: Vec3, up: Vec3) -> Vec3 {
    movement - up * movement.dot(up)
}

pub(super) fn gait_weight(current: f32, distance: f32, speed: f32, dt: f32) -> f32 {
    if !dt.is_finite() || dt <= 0.0 { return current; }
    let target = (distance / dt / speed.max(0.01)).clamp(0.0, 1.0);
    current + (target - current) * (1.0 - (-dt * 8.0).exp())
}

/// Equal-length links stay joined throughout the foot's swing.
pub(crate) fn two_bone(hip: Vec3, target: Vec3, link: f32, bend_sign: f32) -> (Vec3, Vec3) {
    let delta = (target - hip).clamp_length_max(link * 2.0 - 1e-5);
    let foot = hip + delta;
    let bend = (link * link - delta.length_squared() * 0.25).max(0.0).sqrt();
    let direction = Vec3::new(0.0, -delta.z, delta.y).normalize_or_zero();
    ((hip + foot) * 0.5 + direction * bend * bend_sign, foot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) { assert!((a - b).abs() < 0.0001, "{a} != {b}"); }

    #[test]
    fn planted_feet_cancel_ground_translation_for_each_stride() {
        for stride in [0.11, 0.12, 0.16, 0.20, 0.26, 0.30, 0.34, 0.42] {
            let mut prior = None;
            for i in 0..100 {
                let distance = stride * 2.0 * i as f32 / 100.0;
                let (foot, lift) = distance_step(distance, stride, 0.12, 0.0, 1.0);
                near(lift, 0.0);
                let world_z = -distance + foot;
                if let Some(last) = prior { near(world_z, last); }
                prior = Some(world_z);
            }
        }
    }

    #[test]
    fn hoof_walk_has_four_separate_swings_and_three_supports() {
        for stride in [0.2, 0.26, 0.34] {
            let mut swings = [0; 4];
            for sample in 0..400 {
                let distance = sample as f32 / 400.0 * 2.0 * stride / 0.75;
                let mut airborne = 0;
                for leg in 0..4 {
                    let (foot, lift) = hoof_step(distance, stride, 0.12, leg, 1.0);
                    let (next, next_lift) = hoof_step(distance + 0.00001, stride, 0.12, leg, 1.0);
                    if lift > 0.00001 { airborne += 1; swings[leg] += 1; }
                    if lift == 0.0 && next_lift == 0.0 {
                        near(foot - distance, next - distance - 0.00001);
                    }
                    assert!(foot.is_finite() && lift >= 0.0);
                    assert_eq!(hoof_step(distance, stride, 0.12, leg, 0.0), (foot, 0.0));
                }
                assert!(airborne <= 1, "{airborne} feet swinging at distance {distance}");
            }
            assert!(swings.iter().all(|count| *count > 90));
        }
    }

    #[test]
    fn stopping_retains_horizontal_footprints_at_every_phase() {
        for stride in [0.2, 0.26, 0.34] {
            for sample in 0..360 {
                let distance = sample as f32 / 360.0 * 2.0 * stride / 0.75;
                for leg in 0..4 {
                    let original = hoof_step(distance, stride, 0.12, leg, 1.0);
                    let mut weight = 1.0;
                    let mut previous_lift = original.1;
                    for _ in 0..120 {
                        weight = gait_weight(weight, 0.0, 1.0, 1.0 / 120.0);
                        let foot = hoof_step(distance, stride, 0.12, leg, weight);
                        assert_eq!(foot.0, original.0);
                        assert!(foot.1 <= previous_lift);
                        previous_lift = foot.1;
                    }
                    assert!(previous_lift < 0.00005);
                }
            }
        }
    }

    #[test]
    fn starting_keeps_stance_planted_while_lift_blends_in() {
        for (stride, speed) in [(0.2, 1.5), (0.26, 0.9), (0.34, 1.8)] {
            for hz in [30, 60, 120, 240] {
                let dt = 1.0 / hz as f32;
                let mut distance = 0.0;
                let mut weight = 0.0;
                for _ in 0..hz {
                    let next_distance = distance + speed * dt;
                    let next_weight = gait_weight(weight, speed * dt, speed, dt);
                    for leg in 0..4 {
                        let a = hoof_step(distance, stride, 0.12, leg, weight);
                        let b = hoof_step(next_distance, stride, 0.12, leg, next_weight);
                        // Classify stance from the unblended cycle, not zero startup lift.
                        if hoof_step(distance, stride, 0.12, leg, 1.0).1 == 0.0
                            && hoof_step(next_distance, stride, 0.12, leg, 1.0).1 == 0.0 {
                            near(a.0 - distance, b.0 - next_distance);
                        }
                    }
                    distance = next_distance;
                    weight = next_weight;
                }
            }
        }
    }

    #[test]
    fn hoof_walk_boundaries_are_continuous() {
        let stride = 0.26;
        let cycle = 2.0 * stride / 0.75;
        for leg in 0..4 {
            for quarter in 0..=4 {
                let boundary = cycle * quarter as f32 / 4.0;
                let a = hoof_step(boundary - 0.000001, stride, 0.12, leg, 1.0);
                let b = hoof_step(boundary + 0.000001, stride, 0.12, leg, 1.0);
                near(a.0, b.0);
                near(a.1, b.1);
            }
        }
    }

    #[test]
    fn stance_swing_and_cycle_boundaries_are_continuous() {
        for boundary in [0.0, TAU * 0.6, TAU] {
            let a = step(boundary - 0.00001, 0.3, 0.12, 1.0);
            let b = step(boundary + 0.00001, 0.3, 0.12, 1.0);
            near(a.0, b.0);
            near(a.1, b.1);
        }
        for i in 0..100 {
            assert_eq!(step(i as f32, 0.3, 0.12, 0.0), (0.0, 0.0));
        }
    }

    #[test]
    fn gait_start_and_stop_are_frame_rate_independent() {
        let simulate = |hz: u32| {
            let dt = 1.0 / hz as f32;
            let mut weight = 0.0;
            let mut phase = 0.0;
            for _ in 0..hz {
                phase += 2.0 * dt;
                weight = gait_weight(weight, 2.0 * dt, 2.0, dt);
            }
            let moving = weight;
            for _ in 0..hz { weight = gait_weight(weight, 0.0, 2.0, dt); }
            (phase, moving, weight)
        };
        let reference = simulate(120);
        for hz in [30, 60, 144, 240] {
            let actual = simulate(hz);
            near(actual.0, reference.0);
            near(actual.1, reference.1);
            near(actual.2, reference.2);
        }
        assert_eq!(gait_weight(0.5, 1.0, 2.0, 0.0), 0.5);
    }

    #[test]
    fn vertical_steps_do_not_advance_ground_gait() {
        for up in [Vec3::Y, Vec3::X, Vec3::new(1.0, 2.0, 3.0).normalize()] {
            near(ground_motion(up * 5.0, up).length(), 0.0);
            let tangent = up.any_orthonormal_vector();
            near(ground_motion(tangent + up, up).length(), 1.0);
        }
    }

    #[test]
    fn animal_links_keep_their_length_in_every_pose() {
        for (hip_y, stride) in [(0.761_f32, 0.26_f32), (0.438, 0.2), (0.818, 0.34)] {
            let hip = Vec3::new(0.2, hip_y, 0.4);
            let link = ((hip_y - 0.06).powi(2) + stride.powi(2)).sqrt() * 0.5 + 0.012;
            for i in 0..360 {
                let (travel, lift) = step(i as f32 * TAU / 360.0, stride, 0.12, 1.0);
                let target = Vec3::new(hip.x, 0.06 + lift, hip.z + travel);
                for sign in [-1.0, 1.0] {
                    let (knee, foot) = two_bone(hip, target, link, sign);
                    assert!(knee.is_finite() && foot.is_finite());
                    near(hip.distance(knee), link);
                    near(knee.distance(foot), link);
                    near(foot.distance(target), 0.0);
                }
            }
        }
    }
}
