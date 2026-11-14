//! Remote player playback through the netcode jitter buffer.
use engine_netcode::interp::{Interpolate, Playback, SnapshotBuffer};
use glam::{Quat, Vec3};
use std::sync::OnceLock;
use std::time::Instant;

/// Seconds since this process first asked; each peer's clock has its own origin,
/// which the buffer's offset estimate absorbs.
pub(super) fn clock_s() -> f64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    // Zero is reserved for unstamped poses.
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64() + 1.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Sample {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub gravity_frame: Quat,
    pub flying: bool,
}

impl Interpolate for Sample {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let orientation = t.clamp(0.0, 1.0);
        Self {
            position: a.position + (b.position - a.position) * t,
            yaw: blend_yaw(a.yaw, b.yaw, orientation),
            pitch: a.pitch + (b.pitch - a.pitch) * orientation,
            gravity_frame: a
                .gravity_frame
                .slerp(b.gravity_frame, orientation)
                .normalize(),
            flying: if t >= 1.0 { b.flying } else { a.flying },
        }
    }
}

pub(super) fn blend_yaw(from: f32, to: f32, t: f32) -> f32 {
    let turn =
        (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    from + turn * t
}

pub(super) fn buffer() -> SnapshotBuffer<Sample> {
    SnapshotBuffer::new(Playback::DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jittery_poses_play_back_smoothly_behind_the_newest() {
        let mut b = buffer();
        // Host stamps at 30 Hz, 1 unit per snapshot, transit alternating 40/60 ms.
        let arrival = |i: usize| i as f64 / 30.0 + if i % 2 == 0 { 0.04 } else { 0.06 };
        let (mut next, mut last) = (0, f32::MIN);
        for step in 0..240 {
            let now = step as f64 / 120.0;
            while arrival(next) <= now {
                b.push(
                    arrival(next),
                    next as f64 / 30.0,
                    Sample {
                        position: Vec3::X * next as f32,
                        yaw: 0.0,
                        pitch: 0.0,
                        gravity_frame: Quat::IDENTITY,
                        flying: false,
                    },
                );
                next += 1;
            }
            b.advance(now);
            let Some(x) = b.sample().map(|s| s.position.x) else {
                continue;
            };
            assert!(
                x >= last - 1e-4,
                "playback moved backward: {x} after {last}"
            );
            if step > 60 {
                let behind = (next - 1) as f32 - x;
                assert!(
                    (0.5..7.0).contains(&behind),
                    "step {step}: {behind} snapshots behind"
                );
            }
            last = x;
        }
    }

    #[test]
    fn yaw_takes_the_short_way_across_the_wrap() {
        let a = Sample {
            position: Vec3::ZERO,
            yaw: 3.0,
            pitch: 0.0,
            gravity_frame: Quat::IDENTITY,
            flying: false,
        };
        let b = Sample { yaw: -3.0, ..a };
        let mid = Sample::lerp(&a, &b, 0.5).yaw;
        assert!((mid.abs() - std::f32::consts::PI).abs() < 0.01, "{mid}");
    }
}
