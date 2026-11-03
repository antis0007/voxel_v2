//! Jitter buffer for remote entities: plays snapshots back slightly in the past
//! and interpolates between the two that bracket the playback time.
//!
//! Playback delay follows measured arrival jitter, so a clean link stays near the
//! minimum and a noisy one widens up to the maximum instead of stuttering. The
//! playback clock is steered by a few percent rather than jumped, so motion never
//! visibly skips unless the error exceeds `snap_s`.
use glam::Vec3;
use std::collections::VecDeque;

mod parented;
pub use parented::world_pose;

/// State that can be blended. `t` above 1 extrapolates along `a -> b`.
pub trait Interpolate: Copy {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self;
}

impl Interpolate for Vec3 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        *a + (*b - *a) * t
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    pub min_delay_s: f64,
    pub max_delay_s: f64,
    /// Delay added per second of measured mean jitter.
    pub jitter_gain: f64,
    pub max_extrapolate_s: f64,
    /// Clock error beyond which playback jumps instead of steering.
    pub snap_s: f64,
    /// Largest playback speed change used to steer, as a fraction.
    pub max_warp: f64,
    pub capacity: usize,
}

impl Playback {
    /// Snapshots at 20-60 Hz over a home connection.
    pub const DEFAULT: Self = Self {
        min_delay_s: 0.075,
        max_delay_s: 0.2,
        jitter_gain: 3.0,
        max_extrapolate_s: 0.05,
        snap_s: 0.25,
        max_warp: 0.08,
        capacity: 32,
    };
}

pub struct SnapshotBuffer<T> {
    playback: Playback,
    entries: VecDeque<(f64, T)>,
    /// Smallest recent (arrival - server time): transit plus clock offset.
    offset: Option<f64>,
    jitter: f64,
    /// Playback position in server time.
    render: Option<f64>,
    last_now: f64,
}

impl<T: Interpolate> SnapshotBuffer<T> {
    pub fn new(playback: Playback) -> Self {
        Self { playback, entries: VecDeque::new(), offset: None, jitter: 0.0, render: None, last_now: 0.0 }
    }

    /// Records a snapshot stamped `server_s` that arrived at local time `now_s`.
    /// Late or duplicate snapshots older than playback are ignored.
    pub fn push(&mut self, now_s: f64, server_s: f64, state: T) {
        let sample = now_s - server_s;
        let offset = match self.offset {
            // Follow drops at once; forget a stale minimum slowly.
            Some(o) if sample >= o => o + (sample - o) * 0.002,
            _ => sample,
        };
        self.offset = Some(offset);
        self.jitter += ((sample - offset) - self.jitter) * 0.1;
        if self.render.is_some_and(|r| server_s <= r) {
            return;
        }
        let at = self.entries.partition_point(|(t, _)| *t < server_s);
        if self.entries.get(at).is_some_and(|(t, _)| *t == server_s) {
            return;
        }
        self.entries.insert(at, (server_s, state));
        while self.entries.len() > self.playback.capacity {
            self.entries.pop_front();
        }
    }

    /// Current playback delay behind the newest expected snapshot, in seconds.
    pub fn delay_s(&self) -> f64 {
        let p = self.playback;
        (p.min_delay_s + self.jitter * p.jitter_gain).clamp(p.min_delay_s, p.max_delay_s)
    }

    /// Moves playback to local time `now_s`.
    pub fn advance(&mut self, now_s: f64) {
        let dt = (now_s - self.last_now).max(0.0);
        self.last_now = now_s;
        let Some(offset) = self.offset else { return };
        let target = now_s - offset - self.delay_s();
        let render = match self.render {
            Some(r) if (target - r).abs() <= self.playback.snap_s => {
                let warp = ((target - r) / 0.5).clamp(-self.playback.max_warp, self.playback.max_warp);
                r + dt * (1.0 + warp)
            }
            _ => target,
        };
        self.render = Some(render);
        // Keep one entry at or before playback as the left bracket.
        while self.entries.len() > 2 && self.entries[1].0 <= render {
            self.entries.pop_front();
        }
    }

    /// State at the playback time, or `None` before the first snapshot.
    pub fn sample(&self) -> Option<T> {
        self.sample_at(self.render?)
    }

    /// State at server time `render`, bracketed and extrapolated as `sample` does.
    pub fn sample_at(&self, render: f64) -> Option<T> {
        let first = self.entries.front()?;
        if render <= first.0 || self.entries.len() == 1 {
            return Some(first.1);
        }
        let right = self.entries.partition_point(|(t, _)| *t <= render);
        let (a, b) = if right < self.entries.len() {
            (&self.entries[right - 1], &self.entries[right])
        } else {
            // Past the newest snapshot: extrapolate briefly, then hold.
            let n = self.entries.len();
            let limit = self.entries[n - 1].0 + self.playback.max_extrapolate_s;
            let (a, b) = (&self.entries[n - 2], &self.entries[n - 1]);
            let t = (render.min(limit) - a.0) / (b.0 - a.0);
            return Some(T::lerp(&a.1, &b.1, t as f32));
        };
        Some(T::lerp(&a.1, &b.1, ((render - a.0) / (b.0 - a.0)) as f32))
    }

    pub fn playback_time(&self) -> Option<f64> {
        self.render
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::{Impairment, Link};

    /// A body circling at 6 voxels per second, the speed of a sprint.
    fn truth(t: f64) -> Vec3 {
        let a = t * 0.5;
        Vec3::new(a.cos() as f32, 0.0, a.sin() as f32) * 12.0
    }

    struct Run {
        max_error: f32,
        max_step: f32,
        max_delay: f64,
    }

    /// Server sends at `send_hz` through `impairment`; client renders at 144 Hz.
    fn run(impairment: Impairment, send_hz: f64, seconds: f64) -> Run {
        let mut link = Link::new(impairment, 99);
        let mut buffer = SnapshotBuffer::<Vec3>::new(Playback::DEFAULT);
        let (mut next_send, mut previous) = (0.0, None::<Vec3>);
        let mut out = Run { max_error: 0.0, max_step: 0.0, max_delay: 0.0 };
        let frame = 1.0 / 144.0;
        let mut now = 0.0;
        while now < seconds {
            while next_send <= now {
                let mut bytes = next_send.to_le_bytes().to_vec();
                bytes.extend(truth(next_send).to_array().iter().flat_map(|c| c.to_le_bytes()));
                link.send((next_send * 1e6) as u64, bytes);
                next_send += 1.0 / send_hz;
            }
            while let Some(p) = link.poll((now * 1e6) as u64) {
                let server = f64::from_le_bytes(p[..8].try_into().unwrap());
                let c = |i: usize| f32::from_le_bytes(p[8 + i * 4..12 + i * 4].try_into().unwrap());
                buffer.push(now, server, Vec3::new(c(0), c(1), c(2)));
            }
            buffer.advance(now);
            if now > 2.0 {
                let shown = buffer.sample().unwrap();
                out.max_error = out.max_error.max((shown - truth(buffer.playback_time().unwrap())).length());
                if let Some(p) = previous {
                    out.max_step = out.max_step.max((shown - p).length());
                }
                out.max_delay = out.max_delay.max(buffer.delay_s());
                previous = Some(shown);
            }
            now += frame;
        }
        out
    }

    #[test]
    fn a_clean_link_tracks_the_path_at_minimum_delay() {
        let r = run(Impairment { latency_us: 40_000, ..Default::default() }, 30.0, 10.0);
        assert!(r.max_error < 0.05, "error {}", r.max_error);
        // Only 144 Hz frame quantisation of arrivals reads as jitter here.
        assert!(r.max_delay < Playback::DEFAULT.min_delay_s + 0.02, "delay {}", r.max_delay);
    }

    #[test]
    fn broadband_loss_and_jitter_stay_smooth() {
        let r = run(Impairment::BROADBAND, 30.0, 20.0);
        // Sprint speed is 6 voxels/s, so one 144 Hz frame moves 0.042 voxels.
        // A visible stall-then-jump would need several frames of motion in one.
        assert!(r.max_step < 0.06, "step {}", r.max_step);
        assert!(r.max_error < 0.25, "error {}", r.max_error);
        assert!(r.max_delay <= Playback::DEFAULT.max_delay_s);
    }

    #[test]
    fn heavy_jitter_widens_the_delay() {
        let noisy = Impairment { latency_us: 60_000, jitter_us: 60_000, ..Default::default() };
        let r = run(noisy, 30.0, 10.0);
        assert!(r.max_delay > Playback::DEFAULT.min_delay_s + 0.02, "delay {}", r.max_delay);
        assert!(r.max_step < 0.08, "step {}", r.max_step);
    }

    #[test]
    fn a_stall_extrapolates_briefly_then_holds() {
        let mut b = SnapshotBuffer::<Vec3>::new(Playback::DEFAULT);
        for i in 0..10 {
            let t = f64::from(i) / 30.0;
            b.push(t + 0.05, t, Vec3::X * i as f32);
        }
        b.advance(0.0);
        b.advance(2.0);
        let held = b.sample().unwrap();
        let limit = 9.0 + Playback::DEFAULT.max_extrapolate_s as f32 * 30.0;
        assert!((held.x - limit).abs() < 1e-3, "{held:?}");
    }

    #[test]
    fn late_and_duplicate_snapshots_are_ignored() {
        let mut b = SnapshotBuffer::<Vec3>::new(Playback::DEFAULT);
        b.push(0.10, 0.0, Vec3::ZERO);
        b.push(0.20, 0.1, Vec3::X);
        b.push(0.21, 0.1, Vec3::Y * 9.0);
        b.advance(0.0);
        b.advance(1.0);
        b.push(1.0, 0.05, Vec3::Z * 9.0);
        let x = 1.0 + Playback::DEFAULT.max_extrapolate_s as f32 * 10.0;
        assert!((b.sample().unwrap() - Vec3::X * x).length() < 1e-5);
    }
}
