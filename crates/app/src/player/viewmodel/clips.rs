//! Held-item clips loaded from `assets/viewmodel_clips.ron`.
//! Melee and mining time their hits from the impact markers, so drawn contact and the hit agree.
use glam::{Quat, Vec3};
use serde::Deserialize;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};

const CLIPS_RON: &str = include_str!("../../../../../assets/viewmodel_clips.ron");

/// Shape of the motion into a key.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum Ease {
    Linear,
    /// Accelerates into the key: a cut still gathering speed at contact.
    In,
    /// Settles into the key: a wind-up that slows as it loads.
    Out,
    #[default]
    InOut,
}

impl Ease {
    fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::In => t * t * t,
            Self::Out => 1.0 - (1.0 - t).powi(3),
            Self::InOut => t * t * (3.0 - 2.0 * t),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub t: f32,
    #[serde(default)]
    pub offset: [f32; 3],
    #[serde(default)]
    pub turn: [f32; 3],
    #[serde(default)]
    pub ease: Ease,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub duration: f32,
    #[serde(default)]
    pub impact: Option<f32>,
    #[serde(default)]
    pub looped: bool,
    pub keys: Vec<Key>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    idle: Clip,
    equip: Clip,
    forehand: Clip,
    backhand: Clip,
    overhead: Clip,
    chop: Clip,
    #[serde(rename = "use")]
    use_item: Clip,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    ron::from_str(CLIPS_RON).unwrap_or_else(|error| panic!("invalid assets/viewmodel_clips.ron: {error}"))
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Name { Idle, Equip, Forehand, Backhand, Overhead, Chop, Use }

impl Name {
    #[cfg(test)]
    pub const ALL: [Self; 7] = [Self::Idle, Self::Equip, Self::Forehand, Self::Backhand, Self::Overhead, Self::Chop, Self::Use];

    pub fn clip(self) -> &'static Clip {
        let t = &*TABLE;
        match self {
            Self::Idle => &t.idle,
            Self::Equip => &t.equip,
            Self::Forehand => &t.forehand,
            Self::Backhand => &t.backhand,
            Self::Overhead => &t.overhead,
            Self::Chop => &t.chop,
            Self::Use => &t.use_item,
        }
    }

    /// Combo step 1 is a forehand, 2 a backhand, 3 the overhead finisher.
    pub fn stroke(backswing: bool, finisher: bool) -> Self {
        if finisher { Self::Overhead } else if backswing { Self::Backhand } else { Self::Forehand }
    }

    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "idle" => Self::Idle,
            "equip" => Self::Equip,
            "forehand" => Self::Forehand,
            "backhand" => Self::Backhand,
            "overhead" => Self::Overhead,
            "chop" => Self::Chop,
            "use" => Self::Use,
            _ => return None,
        })
    }
}

/// Offset in metres (right, up, forward) and a turn about camera axes, both about the hold point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub offset: Vec3,
    pub rotation: Quat,
}

impl Default for Pose {
    fn default() -> Self { Self { offset: Vec3::ZERO, rotation: Quat::IDENTITY } }
}

impl Pose {
    pub fn then(self, other: Self) -> Self {
        Self { offset: self.offset + other.offset, rotation: self.rotation * other.rotation }
    }

    fn of(key: &Key) -> Self {
        Self { offset: Vec3::from_array(key.offset), rotation: Quat::from_scaled_axis(Vec3::from_array(key.turn)) }
    }
}

impl Clip {
    /// Impact as a fraction of the clip, so contact scales with swing speed.
    pub fn impact_fraction(&self) -> f32 {
        self.impact.map_or(0.0, |at| at / self.duration)
    }

    pub fn sample(&self, t: f32) -> Pose {
        let Some(last) = self.keys.len().checked_sub(1) else { return Pose::default() };
        let t = if !t.is_finite() { 0.0 } else if self.looped { t.rem_euclid(self.duration) } else { t.clamp(0.0, self.duration) };
        let b = self.keys.partition_point(|k| k.t <= t).min(last);
        let a = b.saturating_sub(1);
        let (ka, kb) = (&self.keys[a], &self.keys[b]);
        let span = kb.t - ka.t;
        let mix = if span > 0.0 { kb.ease.apply((t - ka.t) / span) } else { 1.0 };
        let (pa, pb) = (Pose::of(ka), Pose::of(kb));
        Pose { offset: pa.offset.lerp(pb.offset, mix), rotation: pa.rotation.slerp(pb.rotation, mix) }
    }
}

/// Shot index the benchmark capture loop publishes, so a stepped review shows one step per image.
pub static REVIEW_FRAME: AtomicU32 = AtomicU32::new(0);

/// Benchmark runs pin a clip with `VOXEL_REVIEW_CLIP=<name>[:seconds]`. Without seconds the clip
/// advances `VOXEL_REVIEW_CLIP_STEP` seconds (default 1/60) per captured frame and repeats.
pub fn review() -> Option<(Name, f32)> {
    static REVIEW: LazyLock<Option<(Name, Option<f32>, f32)>> = LazyLock::new(|| {
        if !std::env::args().any(|a| a.starts_with("--benchmark-")) { return None; }
        let raw = std::env::var("VOXEL_REVIEW_CLIP").ok()?;
        let (name, at) = raw.split_once(':').map_or((raw.as_str(), None), |(n, t)| (n, t.parse().ok()));
        let step = std::env::var("VOXEL_REVIEW_CLIP_STEP").ok().and_then(|s| s.parse().ok())
            .filter(|s: &f32| s.is_finite() && *s > 0.0).unwrap_or(1.0 / 60.0);
        Some((Name::parse(name)?, at, step))
    });
    let (name, at, step) = (*REVIEW)?;
    let frames = (name.clip().duration / step).floor() as u32 + 1;
    Some((name, at.unwrap_or_else(|| (REVIEW_FRAME.load(Ordering::Relaxed) % frames) as f32 * step)))
}

#[cfg(test)]
mod tests;
