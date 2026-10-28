use super::*;
use crate::render::RenderFrameStats;

fn sample(ms: f32) -> RenderFrameStats {
    RenderFrameStats {
        frame_id: 12, config_id: 7,
        gpu_timestamps_supported: true, gpu_sample_count: 1,
        gpu_sample_frame_id: 9, gpu_sample_config_id: 7,
        gpu_pass_total_ms: ms, ..RenderFrameStats::default()
    }
}

#[test]
fn outdoor_cpu_stalls_do_not_lower_resolution() {
    set_auto_enabled(true);
    let mut governor = Governor::default();
    for _ in 0..600 {
        assert!(governor.gpu_frame(sample(8.0), 0.08, true).is_none());
    }
    assert_eq!(governor.remaining(), Step::ORDER.len());
}

#[test]
fn outdoor_quality_rejects_stale_or_unmatched_gpu_samples() {
    set_auto_enabled(true);
    for stats in [
        RenderFrameStats { gpu_sample_count: 0, ..sample(35.0) },
        RenderFrameStats { gpu_sample_config_id: 8, ..sample(35.0) },
        RenderFrameStats { gpu_sample_frame_id: 1, ..sample(35.0) },
    ] {
        let mut governor = Governor::default();
        for _ in 0..600 { assert!(governor.gpu_frame(stats, 0.05, true).is_none()); }
    }
    let mut governor = Governor::default();
    assert!((0..300).any(|_| governor.gpu_frame(sample(35.0), 0.05, true).is_some()));
}

/// One untimed frame: `wait_ms` of a 40 ms interval spent in acquire and present.
fn untimed(frame_id: u64, wait_ms: f32) -> RenderFrameStats {
    RenderFrameStats { frame_id, surface_acquire_ms: wait_ms, ..RenderFrameStats::default() }
}

#[test]
fn without_timestamps_gpu_bound_frames_step_down() {
    set_auto_enabled(true);
    let mut governor = Governor::default();
    let stepped = (1..=600).any(|id| governor.gpu_frame(untimed(id, 30.0), 0.04, true).is_some());
    assert!(stepped, "40 ms frames waiting 30 ms on the GPU are GPU-bound");
}

#[test]
fn without_timestamps_cpu_bound_frames_never_step_down() {
    set_auto_enabled(true);
    let mut governor = Governor::default();
    for id in 1..=600 {
        assert!(governor.gpu_frame(untimed(id, 2.0), 0.04, true).is_none(), "38 ms of CPU work");
    }
    // A repeated frame id means no new render was measured.
    let mut governor = Governor::default();
    for _ in 0..600 {
        assert!(governor.gpu_frame(untimed(7, 30.0), 0.04, true).is_none());
    }
    assert_eq!(governor.remaining(), Step::ORDER.len());
}

#[test]
fn outdoor_quality_tolerates_delayed_readbacks_without_reusing_them() {
    set_auto_enabled(true);
    let mut governor = Governor::default();
    let missing = RenderFrameStats { gpu_sample_count: 0, ..sample(35.0) };
    let mut changed = false;
    for _ in 0..300 {
        assert!(governor.gpu_frame(missing, 0.05, true).is_none());
        changed |= governor.gpu_frame(sample(35.0), 0.05, true).is_some();
    }
    assert!(changed);
}
