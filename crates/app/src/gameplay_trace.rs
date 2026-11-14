//! Opt-in wall-time breakdown for live gameplay stalls.
use std::{sync::LazyLock, time::Instant};
mod cpu;
static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var_os("VOXEL_GAMEPLAY_TRACE").is_some());
thread_local! {
    /// Wall time per named stage since the last `take_stages`, main thread only.
    static STAGES: std::cell::RefCell<(Instant, Vec<(String, f64)>)> =
        std::cell::RefCell::new((Instant::now(), Vec::new()));
}
/// Adds wall time to a named stage. Always on: one short list scan per call.
pub fn note(stage: &str, ms: f64) {
    STAGES.with_borrow_mut(|(_, stages)| match stages.iter_mut().find(|(name, _)| name == stage) {
        Some((_, total)) => *total += ms,
        None => stages.push((stage.to_string(), ms)),
    });
}
/// Drains the stage totals with the seconds they cover.
pub fn take_stages() -> (f64, Vec<(String, f64)>) {
    STAGES.with_borrow_mut(|(since, stages)| {
        let seconds = since.elapsed().as_secs_f64();
        *since = Instant::now();
        (seconds, std::mem::take(stages))
    })
}
pub fn frame_stage(stage: &str) {
    static ENABLED: LazyLock<bool> =
        LazyLock::new(|| std::env::var_os("VOXEL_FRAME_STAGE_TRACE").is_some());
    static START: LazyLock<Instant> = LazyLock::new(Instant::now);
    if *ENABLED {
        eprintln!(
            "[frame-stage] t={:.3} {stage}",
            START.elapsed().as_secs_f64()
        );
    }
}
/// Covers early returns and local cleanup before returning to the event pump.
pub struct EventScope(&'static str, Option<Instant>);
impl EventScope {
    pub fn new(event: &'static str) -> Self {
        frame_stage(event);
        Self(event, (*ENABLED && event != "event-redraw").then(Instant::now))
    }
}
impl Drop for EventScope {
    fn drop(&mut self) {
        if let Some(started) = self.1 {
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            if ms >= 12.0 {
                eprintln!("[event-stall] stage={} ms={ms:.2}", self.0);
            }
        }
        frame_stage(match self.0 {
            "event-redraw" => "event-redraw-done",
            "event-resize" => "event-resize-done",
            "event-wait" => "event-wait-done",
            "event-window" => "event-window-done",
            "event-device" => "event-device-done",
            "event-new" => "event-new-done",
            _ => "event-other-done",
        });
    }
}
pub struct Trace {
    last: Option<Instant>,
    cpu: Option<u64>,
}
impl Trace {
    pub fn new() -> Self {
        Self {
            last: Some(Instant::now()),
            cpu: ENABLED.then(cpu::ticks).flatten(),
        }
    }
    pub fn mark(&mut self, stage: &str) {
        if let Some(last) = self.last.as_mut() {
            let cpu_now = cpu::ticks();
            let now = Instant::now();
            let ms = now.duration_since(*last).as_secs_f64() * 1000.0;
            note(stage, ms);
            if *ENABLED && ms >= 12.0 {
                let cpu_ms = cpu_now.zip(self.cpu).and_then(|(b, a)| b.checked_sub(a)).map(|v|v as f64 / 10_000.0);
                if let Some(cpu_ms) = cpu_ms {
                    eprintln!("[gameplay-stall] stage={stage} ms={ms:.2} cpu_ms={cpu_ms:.2}");
                } else {
                    eprintln!("[gameplay-stall] stage={stage} ms={ms:.2} cpu_ms=unavailable");
                }
            }
            *last = now;
            self.cpu = cpu_now;
        }
    }
}
