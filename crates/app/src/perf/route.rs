//! Full-benchmark diagnostics, separate from warmed world-performance samples.
use super::CpuFrameStats;
use glam::Vec3;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    sync::mpsc::{SyncSender, TrySendError, sync_channel},
    thread::JoinHandle,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct PresentGap {
    pub count: u32,
    pub wait_ms: f32,
    pub redraw_ms: f32,
    pub device_events: u32,
    pub retry_late_ms: f32,
    pub redraw_dispatch_ms: f32,
}

#[derive(Debug)]
pub struct RouteTiming {
    root: PathBuf,
    output: Option<SyncSender<String>>,
    writer: Option<JoinHandle<std::io::Result<()>>>,
    csv_dropped_frames: u64,
    frames: u64,
    loading_frames: u64,
    loading_transitions: u64,
    failed_renders: u64,
    was_ready: bool,
    samples: Vec<[f32; 2]>,
    sum: [f64; 2],
    max: [f32; 2],
    max_at: [f64; 2],
    io_failed: bool,
    finished: bool,
}

impl RouteTiming {
    // At most 2 MiB of quantile samples; exact sums/maxima cover all frames.
    // Quantile truncation and dropped CSV rows are reported independently.
    const MAX_SAMPLES: usize = 262_144;

    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&root)?;
        // Invalidate a prior summary before truncating the CSV. A killed run
        // must never inherit the previous recording's closed summary.
        std::fs::write(
            root.join("route-timing.json"),
            "{\n  \"recording_closed\": false,\n  \"scope\": \"benchmark diagnostics; recording in progress\"\n}\n",
        )?;
        let mut output = BufWriter::new(File::create(root.join("route-frames.csv"))?);
        writeln!(
            output,
            "frame,seconds,horizon_ready,measuring_at_render,physics,render_ok,eye_x_m,eye_y_m,eye_z_m,yaw,pitch,total_ms,ui_ms,gameplay_ms,snapshot_ms,stream_ms,horizon_ms,cull_ms,torch_ms,weather_ms,uniform_ms,render_cpu_ms,width,height,resident_upload_wall_ms,redraw_ms,outside_redraw_ms,gate_retries,gate_wait_ms,gate_retry_redraw_ms,device_events,gpu_retry_late_ms,redraw_dispatch_ms,acquire_ms"
        )?;
        // Disk writes must not stall the frame being measured. Bound queued
        // rows; loss is reported rather than blocking or growing the queue.
        let (sender, rows) = sync_channel::<String>(256);
        let writer = std::thread::Builder::new().name("route-csv".into()).spawn(
            move || -> std::io::Result<()> {
                for row in rows {
                    output.write_all(row.as_bytes())?;
                }
                output.flush()
            },
        )?;
        Ok(Self {
            root,
            output: Some(sender),
            writer: Some(writer),
            csv_dropped_frames: 0,
            frames: 0,
            loading_frames: 0,
            loading_transitions: 0,
            failed_renders: 0,
            was_ready: false,
            samples: Vec::new(),
            sum: [0.0; 2],
            max: [0.0; 2],
            max_at: [0.0; 2],
            io_failed: false,
            finished: false,
        })
    }

    pub fn record(
        &mut self,
        seconds: f64,
        ready: bool,
        measuring: bool,
        physics: bool,
        render_ok: bool,
        eye_m: Vec3,
        yaw: f32,
        pitch: f32,
        cpu: CpuFrameStats,
        size: [u32; 2],
        redraw_ms: f32,
        acquire_ms: f32,
        gap: PresentGap,
    ) {
        if self.finished {
            return;
        }
        let frame = self.frames;
        self.frames += 1;
        self.loading_frames += u64::from(!ready);
        self.loading_transitions += u64::from(!ready && self.was_ready);
        self.failed_renders += u64::from(!render_ok);
        self.was_ready = ready;
        let values = [cpu.total_ms, cpu.horizon_ms];
        if self.samples.len() < Self::MAX_SAMPLES {
            self.samples.push(values);
        }
        for i in 0..2 {
            self.sum[i] += values[i] as f64;
            if values[i] > self.max[i] {
                self.max[i] = values[i];
                self.max_at[i] = seconds;
            }
        }
        if let Some(output) = self.output.as_ref() {
            let row = format!(
                "{frame},{seconds:.6},{ready},{measuring},{physics},{render_ok},{:.4},{:.4},{:.4},{yaw:.6},{pitch:.6},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{},{},{:.4},{redraw_ms:.4},{:.4},{},{:.4},{:.4},{},{:.4},{:.4},{acquire_ms:.4}\n",
                eye_m.x,
                eye_m.y,
                eye_m.z,
                cpu.total_ms,
                cpu.ui_ms,
                cpu.gameplay_ms,
                cpu.snapshot_ms,
                cpu.streaming_ms,
                cpu.horizon_ms,
                cpu.culling_ms,
                cpu.torch_ms,
                cpu.weather_ms,
                cpu.uniform_ms,
                cpu.render_ms,
                size[0],
                size[1],
                cpu.resident_upload_wall_ms,
                (cpu.total_ms - redraw_ms).max(0.0),
                gap.count,
                gap.wait_ms,
                gap.redraw_ms,
                gap.device_events,
                gap.retry_late_ms,
                gap.redraw_dispatch_ms,
            );
            match output.try_send(row) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => self.csv_dropped_frames += 1,
                Err(TrySendError::Disconnected(_)) => {
                    self.io_failed = true;
                    self.csv_dropped_frames += 1;
                    self.output = None;
                }
            }
        } else {
            self.csv_dropped_frames += 1;
        }
    }

    pub fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        // Finalization happens after measurement, never in record(). The
        // native capture launcher imposes the overall shutdown deadline.
        self.output = None;
        if let Some(writer) = self.writer.take() {
            match writer.join() {
                Ok(Ok(())) => {}
                result => {
                    self.io_failed = true;
                    eprintln!("[route-timing] CSV writer failed: {result:?}");
                }
            }
        }
        let quantile = |index: usize, fraction: f64| {
            let mut values: Vec<f32> = self.samples.iter().map(|v| v[index]).collect();
            values.sort_unstable_by(f32::total_cmp);
            let k = ((values.len().saturating_sub(1) as f64 * fraction).round()) as usize;
            values.get(k).copied().unwrap_or(0.0)
        };
        let denominator = self.frames.max(1) as f64;
        let summary = format!(
            concat!(
                "{{\n  \"recording_closed\": true,\n  \"scope\": \"all benchmark render attempts including loading and warmup; total_ms is successful-presentation cadence (redraw wall time on failure); CSV redraw_ms isolates this redraw and outside_redraw_ms includes event-loop gaps, earlier failed attempts and recorder IO; CPU stage timers exclude asynchronous GPU work; diagnostic only\",\n",
                "  \"frames\": {},\n  \"loading_frames\": {},\n  \"ready_to_loading_transitions\": {},\n",
                "  \"failed_renders\": {},\n  \"csv_io_failed\": {},\n  \"csv_dropped_frames\": {},\n  \"quantile_samples\": {},\n  \"quantiles_complete\": {},\n",
                "  \"total_ms\": {{\"mean\": {:.4}, \"p95\": {:.4}, \"p99\": {:.4}, \"max\": {:.4}, \"max_at_seconds\": {:.6}}},\n",
                "  \"horizon_ms\": {{\"mean\": {:.4}, \"p95\": {:.4}, \"p99\": {:.4}, \"max\": {:.4}, \"max_at_seconds\": {:.6}}}\n}}\n"
            ),
            self.frames,
            self.loading_frames,
            self.loading_transitions,
            self.failed_renders,
            self.io_failed,
            self.csv_dropped_frames,
            self.samples.len(),
            self.samples.len() as u64 == self.frames,
            self.sum[0] / denominator,
            quantile(0, 0.95),
            quantile(0, 0.99),
            self.max[0],
            self.max_at[0],
            self.sum[1] / denominator,
            quantile(1, 0.95),
            quantile(1, 0.99),
            self.max[1],
            self.max_at[1]
        );
        if let Err(error) = std::fs::write(self.root.join("route-timing.json"), summary) {
            eprintln!("[route-timing] cannot save summary: {error}");
        }
        println!(
            "[route-timing] frames={} loading={} transitions={} max_horizon_ms={:.4} io_failed={}",
            self.frames, self.loading_frames, self.loading_transitions, self.max[1], self.io_failed
        );
    }
}

impl Drop for RouteTiming {
    fn drop(&mut self) {
        self.finish();
    }
}
