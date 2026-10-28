//! Automatic quality: step the expensive settings down when frames run slow, and
//! start gently after a graphics-driver reset.
//!
//! Nobody on a struggling GPU goes hunting through menus. The governor watches a
//! measured GPU time while playing (the frame time when the adapter has no GPU
//! timestamps, as on many integrated GPUs) and, after several slow seconds, turns down
//! one setting at a time, cheapest visual loss first, with a pause between steps
//! so each one can take effect. It never turns anything back up on its own: a
//! setting that came back would just bring the slowdown with it. The player can
//! still raise anything in Settings, and can switch the governor off there.
use crate::render::RenderDistanceSettings;
use std::sync::atomic::{AtomicBool, Ordering};

/// A smoothed frame time above this (about 50 fps) counts as slow.
const SLOW_MS: f32 = 20.0;
/// How long frames must stay slow before a step is taken.
const SLOW_FOR_S: f32 = 5.0;
/// Pause after a step, so its effect is what the next decision sees.
const SETTLE_S: f32 = 8.0;
/// A single frame longer than this is a hitch (loading, a stall), not a trend.
const HITCH_MS: f32 = 250.0;
/// Smoothing time constant, seconds.
const SMOOTHING_S: f32 = 1.5;
/// Without GPU timestamps, a frame whose CPU share (the interval minus the time
/// spent waiting for the GPU in acquire and present) exceeds this is CPU-bound,
/// and lowering GPU settings would not speed it up.
const CPU_BOUND_MS: f32 = SLOW_MS * 0.75;
/// Far horizon range after the horizon step, metres.
const SHORT_HORIZON_M: u32 = 12_288;
/// Longest a spawned world waits behind the loader for its first distant
/// horizon. The full range takes about 5 s on the development desktop and longer
/// on a 4-core laptop; past this, play starts and the horizon appears when published.
const HORIZON_WAIT_S: f32 = 6.0;
static HORIZON_WAIT_SINCE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

static AUTO: AtomicBool = AtomicBool::new(true);

#[cfg(test)]
#[path = "quality/outdoor_tests.rs"]
mod outdoor_tests;

pub fn auto_enabled() -> bool {
    AUTO.load(Ordering::Relaxed)
}

pub fn set_auto_enabled(on: bool) {
    AUTO.store(on, Ordering::Relaxed);
}

/// One notch down, in the order they are taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Draw the 3D scene at this percentage of the window (everything scales with
    /// it, clouds included, and the UI stays sharp).
    Resolution(u32),
    NoClouds,
    ShorterView,
    NoBounceLight,
    ShortestView,
    /// Shadow maps off for the session; shadows are not a saved setting.
    NoShadows,
    /// Far horizon range down to `SHORT_HORIZON_M`.
    ShorterHorizon,
}

impl Step {
    /// Cheapest in looks first. Render resolution comes first because it scales
    /// terrain, shadows and clouds together and loses no feature: on a 4090 at 1080p,
    /// 70% took the GPU from 17-25 ms to 13 ms, 50% to 10 ms. Clouds off is the
    /// biggest single lever but removes a feature, so it waits until a shorter view
    /// and a lower resolution have not been enough. Bounce lighting costs a fraction
    /// of a millisecond and a coarser cloud resolution saves almost nothing; both
    /// are tried late or not at all. Shadows and the far horizon come last: both
    /// change the look of the whole scene, and the horizon is mostly CPU build time.
    const ORDER: [Step; 10] = [
        Step::Resolution(85),
        Step::Resolution(70),
        Step::ShorterView,
        Step::Resolution(60),
        Step::NoClouds,
        Step::NoBounceLight,
        Step::ShortestView,
        Step::Resolution(50),
        Step::NoShadows,
        Step::ShorterHorizon,
    ];

    /// What to tell the player, in plain words.
    pub fn notice(self) -> &'static str {
        match self {
            Step::Resolution(85) => "Frame rate was low: drawing at 85% resolution.",
            Step::Resolution(70) => "Frame rate was low: drawing at 70% resolution.",
            Step::Resolution(60) => "Frame rate was low: drawing at 60% resolution.",
            Step::Resolution(_) => "Frame rate was low: drawing at 50% resolution.",
            Step::NoClouds => "Frame rate was low: clouds are now off. Turn them back on in Settings.",
            Step::ShorterView => "Frame rate was low: view distance was shortened.",
            Step::NoBounceLight => "Frame rate was low: bounce lighting is now off.",
            Step::ShortestView => "Frame rate was low: view distance is now at its minimum.",
            Step::NoShadows => "Frame rate was low: shadows are off until restart.",
            Step::ShorterHorizon => "Frame rate was low: far horizon shortened to 12 km.",
        }
    }

    /// Apply the step. Returns false when it changed nothing (already at or past
    /// this setting), so the governor can move straight on to the next one.
    pub fn apply(self, view: &mut RenderDistanceSettings) -> bool {
        use crate::render as r;
        match self {
            Step::Resolution(percent) if r::scene_scale_percent() > percent => {
                r::set_scene_scale_percent(percent);
                true
            }
            Step::NoBounceLight if r::indirect_lighting_enabled() => {
                r::set_indirect_lighting_enabled(false);
                true
            }
            Step::ShorterView => shorten(view, 12),
            Step::NoClouds if r::clouds_enabled() => {
                r::set_clouds_enabled(false);
                true
            }
            Step::ShortestView => shorten(view, 8),
            Step::NoShadows if r::shadow::enabled() => {
                r::shadow::set_enabled(false);
                true
            }
            Step::ShorterHorizon => shorter_horizon(r::horizon_range_m())
                .map(r::set_horizon_range_m)
                .is_some(),
            _ => false,
        }
    }
}

/// Cut the view radius by a quarter, never below `floor` chunks, keeping the
/// full-detail radius inside it.
fn shorten(view: &mut RenderDistanceSettings, floor: i32) -> bool {
    let target = (view.view_distance_chunks * 3 / 4).max(floor);
    if target >= view.view_distance_chunks {
        return false;
    }
    view.view_distance_chunks = target;
    view.lod_start_chunks = view.lod_start_chunks.clamp(2, (target - 1).max(2));
    true
}

/// The range the horizon step sets, if it is shorter than `current`.
fn shorter_horizon(current: u32) -> Option<u32> {
    (current > SHORT_HORIZON_M).then_some(SHORT_HORIZON_M)
}

#[derive(Default)]
pub struct Governor {
    smoothed_ms: f32,
    slow_for: f32,
    settle: f32,
    next: usize,
    /// Last renderer frame fed through the timestamp-free path.
    last_frame_id: u64,
}

impl Governor {
    /// Forget the trend, e.g. when a world opens. Steps already taken stay taken.
    pub fn reset_trend(&mut self) {
        self.smoothed_ms = 0.0;
        self.slow_for = 0.0;
        self.settle = SETTLE_S;
    }

    /// Feed one frame. Returns the step to take now, if any. `playing` is false in
    /// menus and while loading, where frame time says nothing about the game.
    pub fn frame(&mut self, frame_ms: f32, dt_s: f32, playing: bool) -> Option<Step> {
        if !playing || !auto_enabled() || !frame_ms.is_finite() || frame_ms >= HITCH_MS {
            self.slow_for = 0.0;
            return None;
        }
        let k = (dt_s / SMOOTHING_S).clamp(0.0, 1.0);
        self.smoothed_ms = if self.smoothed_ms == 0.0 {
            frame_ms
        } else {
            self.smoothed_ms + (frame_ms - self.smoothed_ms) * k
        };
        if self.settle > 0.0 {
            self.settle -= dt_s;
            self.slow_for = 0.0;
            return None;
        }
        if self.smoothed_ms <= SLOW_MS {
            self.slow_for = 0.0;
            return None;
        }
        self.slow_for += dt_s;
        if self.slow_for < SLOW_FOR_S {
            return None;
        }
        self.slow_for = 0.0;
        self.settle = SETTLE_S;
        let step = Step::ORDER.get(self.next).copied()?;
        self.next += 1;
        Some(step)
    }

    /// Adapt from fresh GPU measurements of the current configuration, or from
    /// GPU-bound frame times when the adapter has no timestamps.
    pub fn gpu_frame(&mut self, stats: crate::render::RenderFrameStats, dt_s: f32, playing: bool) -> Option<Step> {
        if !stats.gpu_timestamps_supported {
            return self.untimed_frame(stats, dt_s, playing);
        }
        if stats.gpu_sample_count == 0 { return None; }
        if stats.gpu_sample_config_id != stats.config_id
            || stats.frame_id.saturating_sub(stats.gpu_sample_frame_id) > 8
            || stats.gpu_pass_total_ms <= 0.0
        {
            self.slow_for = 0.0;
            return None;
        }
        // CPU streaming stalls cannot be cured by blurring the world.
        self.frame(stats.gpu_pass_total_ms, dt_s.min(0.1), playing)
    }

    /// Frame-time fallback. `stats` belong to the previous render, which falls
    /// inside the interval `dt_s`. A frame counts only when the CPU's share fits
    /// `CPU_BOUND_MS`: then the slowness was spent waiting on the GPU, and CPU
    /// streaming stalls, which show up as CPU time, still never blur the world.
    fn untimed_frame(&mut self, stats: crate::render::RenderFrameStats, dt_s: f32, playing: bool) -> Option<Step> {
        if stats.frame_id == self.last_frame_id {
            // No new render since the last call: nothing measured.
            return None;
        }
        self.last_frame_id = stats.frame_id;
        let frame_ms = dt_s * 1000.0;
        let gpu_wait_ms = stats.surface_acquire_ms.max(0.0) + stats.present_ms.max(0.0);
        if !frame_ms.is_finite() || frame_ms - gpu_wait_ms > CPU_BOUND_MS {
            // Left out of the trend rather than resetting it, so an occasional
            // streaming spike does not restart a GPU-bound slow stretch.
            return None;
        }
        self.frame(frame_ms, dt_s.min(0.1), playing)
    }

    /// The next step at once, when the one just returned changed nothing.
    pub fn force_next(&mut self) -> Option<Step> {
        let step = Step::ORDER.get(self.next).copied()?;
        self.next += 1;
        Some(step)
    }

    /// Steps still available.
    pub fn remaining(&self) -> usize {
        Step::ORDER.len().saturating_sub(self.next)
    }
}

/// True while a spawned world should keep waiting for its first distant
/// horizon. `waiting` means the near terrain is ready but the horizon is not;
/// the wait is capped at `HORIZON_WAIT_S` from when that began.
pub fn hold_for_horizon(waiting: bool) -> bool {
    let mut since = HORIZON_WAIT_SINCE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !waiting {
        *since = None;
        return false;
    }
    let started = *since.get_or_insert_with(std::time::Instant::now);
    started.elapsed().as_secs_f32() < HORIZON_WAIT_S
}

/// True once the capped horizon wait has run out and play went ahead without it.
pub fn horizon_wait_expired() -> bool {
    HORIZON_WAIT_SINCE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some_and(|started| started.elapsed().as_secs_f32() >= HORIZON_WAIT_S)
}

/// Marker left when the graphics device was lost, read once at the next start.
fn marker_path() -> std::path::PathBuf {
    std::path::Path::new("saves").join("gpu-reset.flag")
}

/// Record a lost graphics device in the crash log for support and, when
/// `mark_next_launch`, as a marker so the next launch starts with lowered
/// settings. Benchmark and capture runs must not lower the player's session.
pub fn record_gpu_reset(adapter: &str, reason: &str, mark_next_launch: bool) {
    use std::io::Write;
    let _ = std::fs::create_dir_all("saves");
    let line = format!(
        "----\nunix_time={}\nGPU DEVICE LOST adapter={adapter} reason={reason}\n",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    );
    if let Ok(mut log) = std::fs::OpenOptions::new().create(true).append(true).open("saves/crash.log") {
        let _ = log.write_all(line.as_bytes());
    }
    if mark_next_launch {
        let _ = std::fs::write(marker_path(), format!("{adapter}\n{reason}\n"));
    }
}

/// True once, if the previous session ended in a lost graphics device.
pub fn take_gpu_reset() -> bool {
    std::fs::remove_file(marker_path()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(gov: &mut Governor, ms: f32, seconds: f32) -> Vec<Step> {
        let dt = ms / 1000.0;
        let mut out = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            out.extend(gov.frame(ms, dt, true));
            t += dt;
        }
        out
    }

    fn settled() -> Governor {
        let mut g = Governor::default();
        g.reset_trend();
        // Let the post-open settle window pass at a healthy frame time.
        run(&mut g, 8.0, SETTLE_S + 1.0);
        g
    }

    #[test]
    fn fast_frames_never_trigger() {
        set_auto_enabled(true);
        assert!(run(&mut settled(), 12.0, 120.0).is_empty());
    }

    #[test]
    fn sustained_slow_frames_step_down_in_order_with_pauses() {
        set_auto_enabled(true);
        let mut g = settled();
        let steps = run(&mut g, 40.0, 140.0);
        assert_eq!(steps, Step::ORDER.to_vec(), "every step taken once, in order");
        assert_eq!(g.remaining(), 0);
        // Every step after the first waits a settle window plus a slow stretch.
        let mut g = settled();
        let first_window = run(&mut g, 40.0, SLOW_FOR_S + 1.0);
        assert_eq!(first_window.len(), 1, "one step per slow stretch");
    }

    #[test]
    fn hitches_loading_and_menus_are_ignored() {
        set_auto_enabled(true);
        let mut g = settled();
        for _ in 0..2_000 {
            assert!(g.frame(400.0, 0.4, true).is_none(), "a hitch is not a trend");
            assert!(g.frame(60.0, 0.06, false).is_none(), "menus say nothing");
        }
    }

    #[test]
    fn a_brief_dip_does_not_trigger() {
        set_auto_enabled(true);
        let mut g = settled();
        assert!(run(&mut g, 45.0, 3.0).is_empty());
        assert!(run(&mut g, 10.0, 10.0).is_empty());
        assert!(run(&mut g, 45.0, 3.0).is_empty(), "the slow clock restarted");
    }

    #[test]
    fn switched_off_it_does_nothing() {
        set_auto_enabled(false);
        let mut g = settled();
        let steps = run(&mut g, 60.0, 60.0);
        set_auto_enabled(true);
        assert!(steps.is_empty());
    }

    #[test]
    fn resolution_steps_only_ever_lower_the_resolution() {
        let mut view = RenderDistanceSettings { view_distance_chunks: 28, lod_start_chunks: 10, lod_levels: 4 };
        crate::render::set_scene_scale_percent(100);
        assert!(Step::Resolution(85).apply(&mut view));
        assert_eq!(crate::render::scene_scale_percent(), 85);
        assert!(!Step::Resolution(85).apply(&mut view), "already there");
        assert!(Step::Resolution(70).apply(&mut view));
        crate::render::set_scene_scale_percent(60);
        assert!(!Step::Resolution(70).apply(&mut view), "never raises it");
        assert_eq!(crate::render::scene_scale_percent(), 60);
        crate::render::set_scene_scale_percent(100);
    }

    #[test]
    fn no_clouds_turns_clouds_off_once() {
        let mut view = RenderDistanceSettings { view_distance_chunks: 28, lod_start_chunks: 10, lod_levels: 4 };
        crate::render::set_clouds_enabled(true);
        assert!(Step::NoClouds.apply(&mut view));
        assert!(!crate::render::clouds_enabled());
        assert!(!Step::NoClouds.apply(&mut view), "already off: nothing to do");
        assert_eq!(view.view_distance_chunks, 28, "the view is untouched by this step");
    }

    #[test]
    fn horizon_step_only_shortens_a_longer_range() {
        // The range is a global the horizon tests read, so the rule is tested pure.
        assert_eq!(shorter_horizon(crate::render::MAX_HORIZON_RANGE_M), Some(SHORT_HORIZON_M));
        assert_eq!(shorter_horizon(SHORT_HORIZON_M), None, "already there");
        assert_eq!(shorter_horizon(4096), None, "never lengthens it");
    }

    #[test]
    fn horizon_wait_resets_when_nothing_is_pending() {
        assert!(hold_for_horizon(true), "a fresh wait holds");
        assert!(!horizon_wait_expired());
        assert!(!hold_for_horizon(false));
        assert!(!horizon_wait_expired(), "cleared with the wait");
    }

    #[test]
    fn view_steps_shorten_but_respect_floors_and_lod_radius() {
        let mut v = RenderDistanceSettings { view_distance_chunks: 28, lod_start_chunks: 10, lod_levels: 4 };
        assert!(shorten(&mut v, 12));
        assert_eq!(v.view_distance_chunks, 21);
        assert!(shorten(&mut v, 12));
        assert_eq!(v.view_distance_chunks, 15);
        assert!(shorten(&mut v, 12));
        assert_eq!(v.view_distance_chunks, 12);
        assert!(!shorten(&mut v, 12), "already at the floor");
        assert!(v.lod_start_chunks < v.view_distance_chunks && v.lod_start_chunks >= 2);
    }
}
