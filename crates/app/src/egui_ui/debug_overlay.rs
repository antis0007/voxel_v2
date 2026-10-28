//! F3 readout: plain text lines on translucent strips, no window chrome.
use egui::{Align, Align2, Color32, FontId, Layout, Rect, Sense, vec2};
use engine_core::{CHUNK_SIZE, floor_div};
use glam::{IVec3, Vec3};

use crate::perf::PerfSnapshot;

const FONT: f32 = 12.0;
const STRIP: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 150);
const TEXT: Color32 = Color32::from_rgb(224, 224, 224);
const WARN: Color32 = Color32::from_rgb(255, 196, 92);
const BAD: Color32 = Color32::from_rgb(255, 110, 96);
const GRAPH: [f32; 2] = [180.0, 34.0];
const RIGHT_BOTTOM: &str = "debug_overlay_right_bottom";

pub(super) struct Readout<'a> {
    pub fps: u32,
    pub perf: &'a PerfSnapshot,
    pub pos: Vec3,
    pub yaw: f32,
    pub scale_label: &'a str,
    pub biome: &'a str,
    pub elevation_m: f32,
    pub time_label: &'a str,
    pub environment_lag: u64,
    pub cave_culled: usize,
    pub cursor: String,
    pub pending_cells: usize,
}

/// Draws both columns; returns the left column's rect for HUD stacking.
pub(super) fn draw(ctx: &egui::Context, r: &Readout) -> Rect {
    let left = egui::Area::new("debug_overlay_left".into())
        .anchor(Align2::LEFT_TOP, [4.0, 4.0])
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
            for (text, color) in left_lines(r) {
                line(ui, &text, color);
            }
        });
    let right = egui::Area::new("debug_overlay_right".into())
        .anchor(Align2::RIGHT_TOP, [-4.0, 4.0])
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
                for (text, color) in right_lines(r.perf) {
                    line(ui, &text, color);
                }
                ui.add_space(2.0);
                frame_graph(ui, r.perf);
            });
        });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(RIGHT_BOTTOM), right.response.rect.bottom()));
    left.response.rect
}

/// Bottom edge of last frame's right column, so the minimap can sit below it.
pub(super) fn right_bottom(ctx: &egui::Context) -> Option<f32> {
    ctx.data(|d| d.get_temp(egui::Id::new(RIGHT_BOTTOM)))
}

fn left_lines(r: &Readout) -> Vec<(String, Color32)> {
    let p = r.perf;
    let block = r.pos.floor().as_ivec3();
    let chunk = IVec3::new(
        floor_div(block.x, CHUNK_SIZE),
        floor_div(block.y, CHUNK_SIZE),
        floor_div(block.z, CHUNK_SIZE),
    );
    let fps_color = if p.fps_1p_low > 0.0 && p.fps_1p_low < 30.0 { BAD } else { TEXT };
    let mut out = vec![
        (
            format!(
                "{} fps  {:.1} ms  p95 {:.1}  1% {:.0}",
                r.fps, p.frame_ms_avg, p.frame_ms_p95, p.fps_1p_low
            ),
            fps_color,
        ),
        (format!("XYZ {:.2} / {:.2} / {:.2}", r.pos.x, r.pos.y, r.pos.z), TEXT),
        (format!("Block {} {} {}", block.x, block.y, block.z), TEXT),
        (format!("Chunk {} {} {}  in {} {} {}", chunk.x, chunk.y, chunk.z,
            block.x - chunk.x * CHUNK_SIZE, block.y - chunk.y * CHUNK_SIZE,
            block.z - chunk.z * CHUNK_SIZE), TEXT),
        (format!("Facing {}  {:.1}°", facing(r.yaw), r.yaw.to_degrees().rem_euclid(360.0)), TEXT),
        (format!("Biome {}  {:.0} m", r.biome, r.elevation_m), TEXT),
        (format!("{}  voxel {}", r.time_label, r.scale_label), TEXT),
        (format!("Cave-culled {}", r.cave_culled), TEXT),
    ];
    if r.environment_lag > 1 {
        out.push((format!("Environment {} s behind", r.environment_lag), BAD));
    }
    if !r.cursor.is_empty() {
        out.push((r.cursor.clone(), TEXT));
    }
    if r.pending_cells > 0 {
        out.push((format!("{} cells queued", r.pending_cells), WARN));
    }
    out
}

fn right_lines(p: &PerfSnapshot) -> Vec<(String, Color32)> {
    let mut out = Vec::new();
    if !p.adapter_summary.is_empty() {
        out.push((p.adapter_summary.clone(), TEXT));
    }
    out.push((format!("{}x{}", p.resolution[0], p.resolution[1]), TEXT));
    let cpu = p.cpu_avg.total_ms;
    if p.render_avg.gpu_timestamps_supported {
        let gpu = p.render_avg.gpu_pass_total_ms;
        let bound = if gpu > p.frame_ms_avg * 0.75 { "GPU-bound" } else { "CPU-bound" };
        out.push((format!("CPU {cpu:.1} ms  GPU {gpu:.1} ms  {bound}"), TEXT));
        out.push((
            format!(
                "terrain {:.1}  cloud {:.1}  shadow {:.1}  sky {:.1}",
                p.render_avg.gpu_terrain_ms,
                p.render_avg.gpu_cloud_raymarch_ms,
                p.render_avg.gpu_shadow_ms,
                p.render_avg.gpu_sky_ms
            ),
            TEXT,
        ));
    } else {
        out.push((format!("CPU {cpu:.1} ms  GPU timing off"), TEXT));
    }
    out.push((
        format!(
            "sim {:.1}  stream {:.1}  horizon {:.1}  wait {:.1}",
            p.cpu_avg.gameplay_ms,
            p.cpu_avg.streaming_ms,
            p.cpu_avg.horizon_ms,
            p.render_avg.surface_acquire_ms
        ),
        TEXT,
    ));
    let c = &p.counters_latest;
    out.push((
        format!(
            "{} tris  {} draws",
            thousands(c.terrain_triangles),
            p.render_latest.terrain_draw_calls
        ),
        TEXT,
    ));
    let missing = c.visible_missing;
    out.push((
        format!("Chunks {} visible  {} missing", c.visible_chunks, missing),
        if missing > 0 { WARN } else { TEXT },
    ));
    let s = &p.chunk_latest;
    out.push((
        format!(
            "gen {}  mesh {}  queued {}  GPU {} MiB",
            s.generating,
            s.meshing,
            s.requested,
            s.gpu_resident_bytes >> 20
        ),
        TEXT,
    ));
    out
}

fn line(ui: &mut egui::Ui, text: &str, color: Color32) {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::monospace(FONT), color);
    let (rect, _) = ui.allocate_exact_size(galley.size() + vec2(6.0, 2.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, STRIP);
    ui.painter().galley(rect.min + vec2(3.0, 1.0), galley, color);
}

/// Per-frame bars of total frame time against 60 and 30 fps lines.
fn frame_graph(ui: &mut egui::Ui, p: &PerfSnapshot) {
    let (rect, _) = ui.allocate_exact_size(vec2(GRAPH[0], GRAPH[1]), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0, STRIP);
    let scale_ms = 50.0;
    let y_of = |ms: f32| rect.bottom() - (ms / scale_ms).min(1.0) * rect.height();
    let history = &p.history;
    let n = history.len().min(GRAPH[0] as usize);
    for (i, sample) in history[history.len() - n..].iter().enumerate() {
        let ms = sample.cpu.total_ms;
        let x = rect.right() - (n - i) as f32;
        let color = if ms > 33.4 { BAD } else if ms > 16.7 { WARN } else { Color32::from_rgb(96, 200, 120) };
        painter.line_segment([egui::pos2(x, rect.bottom()), egui::pos2(x, y_of(ms))], (1.0, color));
    }
    for ms in [16.7, 33.3] {
        let y = y_of(ms);
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            (1.0, Color32::from_white_alpha(60)),
        );
    }
}

fn facing(yaw: f32) -> &'static str {
    // Yaw 0 looks north (-z) and grows toward east (+x).
    const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let octant = (yaw.to_degrees().rem_euclid(360.0) / 45.0).round() as usize % 8;
    NAMES[octant]
}

fn thousands(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.2} M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1} k", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_matches_minimap_convention() {
        assert_eq!(facing(0.0), "N");
        assert_eq!(facing(std::f32::consts::FRAC_PI_2), "E");
        assert_eq!(facing(std::f32::consts::PI), "S");
        assert_eq!(facing(-std::f32::consts::FRAC_PI_2), "W");
    }

    #[test]
    fn left_lines_report_negative_chunk_coordinates() {
        let perf = PerfSnapshot::default();
        let r = Readout {
            fps: 60,
            perf: &perf,
            pos: Vec3::new(-0.5, 70.2, -33.0),
            yaw: 0.0,
            scale_label: "",
            biome: "",
            elevation_m: 0.0,
            time_label: "",
            environment_lag: 0,
            cave_culled: 0,
            cursor: String::new(),
            pending_cells: 0,
        };
        let lines = left_lines(&r);
        assert!(lines.iter().any(|(t, _)| t == "Block -1 70 -33"));
        assert!(lines.iter().any(|(t, _)| t.starts_with("Chunk -1 2 -2  in 31 6 31")));
    }
}
