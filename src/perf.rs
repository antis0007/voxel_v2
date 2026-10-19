use std::collections::VecDeque;

use crate::render::{
    ChunkStreamerStats, RenderDistanceSettings, RenderFrameStats, WeatherUploadStats,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct CpuFrameStats {
    pub ui_ms: f32,
    pub gameplay_ms: f32,
    pub snapshot_ms: f32,
    pub streaming_ms: f32,
    pub culling_ms: f32,
    pub torch_ms: f32,
    pub weather_ms: f32,
    pub uniform_ms: f32,
    pub render_ms: f32,
    pub total_ms: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameCounters {
    pub visible_chunks: usize,
    pub visible_meshes: usize,
    pub draw_chunks: usize,
    pub terrain_triangles: usize,
    pub nearby_torches: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PerfSample {
    pub cpu: CpuFrameStats,
    pub render: RenderFrameStats,
    pub chunk: ChunkStreamerStats,
    pub weather: WeatherUploadStats,
    pub counters: FrameCounters,
}

#[derive(Clone, Debug)]
pub struct PerfSnapshot {
    pub adapter_summary: String,
    pub resolution: [u32; 2],
    pub render_settings: RenderDistanceSettings,
    pub history: Vec<PerfSample>,
    pub sample_count: usize,
    pub fps_avg: f32,
    pub fps_1p_low: f32,
    pub frame_ms_avg: f32,
    pub frame_ms_p99: f32,
    pub frame_ms_max: f32,
    pub cpu_avg: CpuFrameStats,
    pub cpu_latest: CpuFrameStats,
    pub render_avg: RenderFrameStats,
    pub render_latest: RenderFrameStats,
    pub chunk_latest: ChunkStreamerStats,
    pub weather_latest: WeatherUploadStats,
    pub counters_latest: FrameCounters,
}

impl Default for PerfSnapshot {
    fn default() -> Self {
        Self {
            adapter_summary: String::new(),
            resolution: [0, 0],
            render_settings: RenderDistanceSettings {
                view_distance_chunks: 0,
                lod_start_chunks: 0,
                lod_levels: 0,
            },
            history: Vec::new(),
            sample_count: 0,
            fps_avg: 0.0,
            fps_1p_low: 0.0,
            frame_ms_avg: 0.0,
            frame_ms_p99: 0.0,
            frame_ms_max: 0.0,
            cpu_avg: CpuFrameStats::default(),
            cpu_latest: CpuFrameStats::default(),
            render_avg: RenderFrameStats::default(),
            render_latest: RenderFrameStats::default(),
            chunk_latest: ChunkStreamerStats::default(),
            weather_latest: WeatherUploadStats::default(),
            counters_latest: FrameCounters::default(),
        }
    }
}

pub struct PerfTracker {
    history: VecDeque<PerfSample>,
    max_history: usize,
}

impl Default for PerfTracker {
    fn default() -> Self {
        Self::new(240)
    }
}

impl PerfTracker {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history.max(1)),
            max_history: max_history.max(1),
        }
    }

    pub fn push(&mut self, sample: PerfSample) {
        if self.history.len() == self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(sample);
    }

    pub fn snapshot(
        &self,
        adapter_summary: &str,
        resolution: [u32; 2],
        render_settings: RenderDistanceSettings,
    ) -> PerfSnapshot {
        let samples: Vec<_> = self.history.iter().copied().collect();
        snapshot_from_samples(&samples, adapter_summary, resolution, render_settings)
    }
}

pub fn snapshot_from_samples(
    samples: &[PerfSample],
    adapter_summary: &str,
    resolution: [u32; 2],
    render_settings: RenderDistanceSettings,
) -> PerfSnapshot {
    let latest = samples.last().copied().unwrap_or_default();
    if samples.is_empty() {
        return PerfSnapshot {
            adapter_summary: adapter_summary.to_string(),
            resolution,
            render_settings,
            history: samples.to_vec(),
            cpu_latest: latest.cpu,
            render_latest: latest.render,
            chunk_latest: latest.chunk,
            weather_latest: latest.weather,
            counters_latest: latest.counters,
            ..PerfSnapshot::default()
        };
    }

    let sample_count = samples.len();
    let frame_ms_values: Vec<f32> = samples
        .iter()
        .map(|sample| sample.cpu.total_ms.max(0.0))
        .collect();
    let frame_ms_avg = frame_ms_values.iter().copied().sum::<f32>() / sample_count as f32;
    let frame_ms_max = frame_ms_values.iter().copied().fold(0.0f32, f32::max);
    let mut sorted_ms = frame_ms_values.clone();
    sorted_ms.sort_by(|a, b| a.total_cmp(b));
    let p99_index = (((sample_count - 1) as f32) * 0.99).round() as usize;
    let frame_ms_p99 = sorted_ms[p99_index.min(sorted_ms.len() - 1)];
    let fps_avg = if frame_ms_avg > 0.0 {
        1000.0 / frame_ms_avg
    } else {
        0.0
    };
    let fps_1p_low = if frame_ms_p99 > 0.0 {
        1000.0 / frame_ms_p99
    } else {
        0.0
    };

    PerfSnapshot {
        adapter_summary: adapter_summary.to_string(),
        resolution,
        render_settings,
        history: samples.to_vec(),
        sample_count,
        fps_avg,
        fps_1p_low,
        frame_ms_avg,
        frame_ms_p99,
        frame_ms_max,
        cpu_avg: average_cpu(samples),
        cpu_latest: latest.cpu,
        render_avg: average_render(samples),
        render_latest: latest.render,
        chunk_latest: latest.chunk,
        weather_latest: latest.weather,
        counters_latest: latest.counters,
    }
}

fn average_cpu(samples: &[PerfSample]) -> CpuFrameStats {
    let denom = samples.len().max(1) as f32;
    let sum = samples
        .iter()
        .fold(CpuFrameStats::default(), |mut acc, sample| {
            acc.ui_ms += sample.cpu.ui_ms;
            acc.gameplay_ms += sample.cpu.gameplay_ms;
            acc.snapshot_ms += sample.cpu.snapshot_ms;
            acc.streaming_ms += sample.cpu.streaming_ms;
            acc.culling_ms += sample.cpu.culling_ms;
            acc.torch_ms += sample.cpu.torch_ms;
            acc.weather_ms += sample.cpu.weather_ms;
            acc.uniform_ms += sample.cpu.uniform_ms;
            acc.render_ms += sample.cpu.render_ms;
            acc.total_ms += sample.cpu.total_ms;
            acc
        });

    CpuFrameStats {
        ui_ms: sum.ui_ms / denom,
        gameplay_ms: sum.gameplay_ms / denom,
        snapshot_ms: sum.snapshot_ms / denom,
        streaming_ms: sum.streaming_ms / denom,
        culling_ms: sum.culling_ms / denom,
        torch_ms: sum.torch_ms / denom,
        weather_ms: sum.weather_ms / denom,
        uniform_ms: sum.uniform_ms / denom,
        render_ms: sum.render_ms / denom,
        total_ms: sum.total_ms / denom,
    }
}

fn average_render(samples: &[PerfSample]) -> RenderFrameStats {
    let denom = samples.len().max(1) as f32;
    let sum = samples
        .iter()
        .fold(RenderFrameStats::default(), |mut acc, sample| {
            acc.surface_acquire_ms += sample.render.surface_acquire_ms;
            acc.stream_upload_ms += sample.render.stream_upload_ms;
            acc.shadow_encode_ms += sample.render.shadow_encode_ms;
            acc.terrain_encode_ms += sample.render.terrain_encode_ms;
            acc.cloud_encode_ms += sample.render.cloud_encode_ms;
            acc.ui_encode_ms += sample.render.ui_encode_ms;
            acc.overlay_encode_ms += sample.render.overlay_encode_ms;
            acc.submit_ms += sample.render.submit_ms;
            acc.present_ms += sample.render.present_ms;
            acc.terrain_draw_calls += sample.render.terrain_draw_calls;
            acc.terrain_indices += sample.render.terrain_indices;
            acc.shadow_draw_calls += sample.render.shadow_draw_calls;
            acc.shadow_redraw |= sample.render.shadow_redraw;
            acc.cloud_raymarch_passes += sample.render.cloud_raymarch_passes;
            acc.cloud_temporal_passes += sample.render.cloud_temporal_passes;
            acc.outline_vertices += sample.render.outline_vertices;
            acc.invert_outline_vertices += sample.render.invert_outline_vertices;
            acc.ui_vertices += sample.render.ui_vertices;
            acc.gpu_pass_total_ms += sample.render.gpu_pass_total_ms;
            acc.gpu_shadow_ms += sample.render.gpu_shadow_ms;
            acc.gpu_terrain_ms += sample.render.gpu_terrain_ms;
            acc.gpu_indirect_ms += sample.render.gpu_indirect_ms;
            acc.gpu_cloud_raymarch_ms += sample.render.gpu_cloud_raymarch_ms;
            acc.gpu_cloud_temporal_ms += sample.render.gpu_cloud_temporal_ms;
            acc.gpu_sky_ms += sample.render.gpu_sky_ms;
            acc.gpu_cloud_composite_ms += sample.render.gpu_cloud_composite_ms;
            acc.gpu_outline_ms += sample.render.gpu_outline_ms;
            acc.gpu_ui_ms += sample.render.gpu_ui_ms;
            acc.gpu_timestamps_supported |= sample.render.gpu_timestamps_supported;
            acc.cloud_history_invalidations += sample.render.cloud_history_invalidations;
            acc.cloud_history_camera_invalidations +=
                sample.render.cloud_history_camera_invalidations;
            acc.cloud_history_hard_cut_invalidations +=
                sample.render.cloud_history_hard_cut_invalidations;
            acc.cloud_history_weather_invalidations +=
                sample.render.cloud_history_weather_invalidations;
            acc.cloud_history_external_invalidations +=
                sample.render.cloud_history_external_invalidations;
            acc
        });

    RenderFrameStats {
        surface_acquire_ms: sum.surface_acquire_ms / denom,
        stream_upload_ms: sum.stream_upload_ms / denom,
        shadow_encode_ms: sum.shadow_encode_ms / denom,
        terrain_encode_ms: sum.terrain_encode_ms / denom,
        cloud_encode_ms: sum.cloud_encode_ms / denom,
        ui_encode_ms: sum.ui_encode_ms / denom,
        overlay_encode_ms: sum.overlay_encode_ms / denom,
        submit_ms: sum.submit_ms / denom,
        present_ms: sum.present_ms / denom,
        terrain_draw_calls: (sum.terrain_draw_calls as f32 / denom).round() as usize,
        terrain_indices: (sum.terrain_indices as f32 / denom).round() as usize,
        shadow_draw_calls: (sum.shadow_draw_calls as f32 / denom).round() as usize,
        shadow_redraw: sum.shadow_redraw,
        cloud_raymarch_passes: (sum.cloud_raymarch_passes as f32 / denom).round() as usize,
        cloud_temporal_passes: (sum.cloud_temporal_passes as f32 / denom).round() as usize,
        outline_vertices: (sum.outline_vertices as f32 / denom).round() as usize,
        invert_outline_vertices: (sum.invert_outline_vertices as f32 / denom).round() as usize,
        ui_vertices: (sum.ui_vertices as f32 / denom).round() as usize,
        gpu_pass_total_ms: sum.gpu_pass_total_ms / denom,
        gpu_shadow_ms: sum.gpu_shadow_ms / denom,
        gpu_terrain_ms: sum.gpu_terrain_ms / denom,
        gpu_indirect_ms: sum.gpu_indirect_ms / denom,
        gpu_cloud_raymarch_ms: sum.gpu_cloud_raymarch_ms / denom,
        gpu_cloud_temporal_ms: sum.gpu_cloud_temporal_ms / denom,
        gpu_sky_ms: sum.gpu_sky_ms / denom,
        gpu_cloud_composite_ms: sum.gpu_cloud_composite_ms / denom,
        gpu_outline_ms: sum.gpu_outline_ms / denom,
        gpu_ui_ms: sum.gpu_ui_ms / denom,
        gpu_timestamps_supported: sum.gpu_timestamps_supported,
        cloud_history_invalidations: (sum.cloud_history_invalidations as f32 / denom).round()
            as usize,
        cloud_history_camera_invalidations: (sum.cloud_history_camera_invalidations as f32 / denom)
            .round() as usize,
        cloud_history_hard_cut_invalidations: (sum.cloud_history_hard_cut_invalidations as f32
            / denom)
            .round() as usize,
        cloud_history_weather_invalidations: (sum.cloud_history_weather_invalidations as f32
            / denom)
            .round() as usize,
        cloud_history_external_invalidations: (sum.cloud_history_external_invalidations as f32
            / denom)
            .round() as usize,
    }
}
