//! Shared world creation and pause-menu graphics controls.
use super::{
    PhysicsUiState, RenderUiState, TimeUiState, UiAction, cloud_controls_ui, format_clock,
};
use crate::render::{AtmosphereMode, RenderDebugView, RenderDistanceSettings};

/// Terrain presets in metres, with chunk floors. Chunks are 32 voxels, so a
/// metre-only preset collapsed to 4 chunks at 1 m voxels and put 4 m LOD cells
/// 96 m from the camera. "Performance" now saves frame time on clouds and
/// bounce lighting instead of turning nearby trees into blocks.
struct TerrainPreset {
    name: &'static str,
    hint: &'static str,
    view_m: f32,
    full_detail_m: f32,
    min_view_chunks: i32,
    min_full_chunks: i32,
    lod_levels: u8,
    cloud_scale: u32,
    indirect: bool,
}

impl TerrainPreset {
    fn settings(&self, chunk_meters: f32) -> RenderDistanceSettings {
        let view = ((self.view_m / chunk_meters).round() as i32)
            .max(self.min_view_chunks)
            .clamp(4, 64);
        RenderDistanceSettings {
            view_distance_chunks: view,
            lod_start_chunks: ((self.full_detail_m / chunk_meters).round() as i32)
                .max(self.min_full_chunks)
                .clamp(2, view - 1),
            lod_levels: self.lod_levels,
        }
    }
}

const TERRAIN_PRESETS: [TerrainPreset; 3] = [
    TerrainPreset {
        name: "Performance",
        hint: "Short view, sharp nearby terrain, cheaper clouds, no bounce lighting.",
        view_m: 256.0,
        full_detail_m: 128.0,
        min_view_chunks: 8,
        min_full_chunks: 4,
        lod_levels: 2,
        cloud_scale: 4,
        indirect: false,
    },
    TerrainPreset {
        name: "Balanced",
        hint: "Default view distance and lighting.",
        view_m: 896.0,
        full_detail_m: 320.0,
        min_view_chunks: 16,
        min_full_chunks: 6,
        lod_levels: 4,
        cloud_scale: 3,
        indirect: true,
    },
    TerrainPreset {
        name: "Far view",
        hint: "Maximum terrain radius. Needs memory and a fast GPU.",
        view_m: 2048.0,
        full_detail_m: 384.0,
        min_view_chunks: 32,
        min_full_chunks: 8,
        lod_levels: 4,
        cloud_scale: 3,
        indirect: true,
    },
];

pub(super) fn graphics_settings(ui: &mut egui::Ui, state: &mut RenderUiState, voxel_meters: f32) {
    let chunk_meters = 32.0 * voxel_meters;
    ui.checkbox(&mut state.fxaa_enabled, "Smooth edges (FXAA)");
    let mut indirect = crate::render::indirect_lighting_enabled();
    ui.checkbox(&mut indirect, "Diffuse bounce and contact shading")
        .on_hover_text(
            "Adds nearby color bounce and soft contact shadows. Disable for more performance.",
        );
    crate::render::set_indirect_lighting_enabled(indirect);
    ui.checkbox(&mut state.vsync, "VSync (match display refresh)");
    let mut clouds = crate::render::cloud_render_scale();
    egui::ComboBox::from_label("Cloud detail")
        .selected_text(match clouds {
            2 => "Ultra",
            4 => "Balanced",
            _ => "High",
        })
        .show_ui(ui, |ui| {
            for (scale, label) in [(2, "Ultra"), (3, "High"), (4, "Balanced")] {
                ui.selectable_value(&mut clouds, scale, label);
            }
        });
    crate::render::set_cloud_render_scale(clouds);
    ui.small("High is the default. Cloud images keep at least 360 rows; Ultra ray-marches the most pixels.");
    ui.label("Terrain detail");
    ui.horizontal_wrapped(|ui| {
        for preset in TERRAIN_PRESETS {
            if ui.button(preset.name).on_hover_text(preset.hint).clicked() {
                state.settings = preset.settings(chunk_meters);
                crate::render::set_cloud_render_scale(preset.cloud_scale);
                crate::render::set_indirect_lighting_enabled(preset.indirect);
            }
        }
    });
    // Radii are whole chunks; the sliders work in chunks and label metres so
    // every stop is a real setting at any voxel scale.
    let s = &mut state.settings;
    ui.add(
        egui::Slider::new(&mut s.view_distance_chunks, 4..=64)
            .custom_formatter(|v, _| format!("{:.0} m", v * chunk_meters as f64))
            .text("View radius"),
    );
    let full_limit = (s.view_distance_chunks - 1).max(2);
    s.lod_start_chunks = s.lod_start_chunks.clamp(2, full_limit);
    ui.add(
        egui::Slider::new(&mut s.lod_start_chunks, 2..=full_limit)
            .custom_formatter(|v, _| format!("{:.0} m", v * chunk_meters as f64))
            .text("Full detail radius"),
    )
    .on_hover_text("Inside this radius terrain and trees are meshed voxel for voxel.");
    ui.add(egui::Slider::new(&mut s.lod_levels, 1..=4).text("Detail levels"))
        .on_hover_text(
            "Beyond full detail, distant terrain uses 2, 4 and 8 voxel cells. \
             Fewer levels look sharper and cost more triangles.",
        );
    let side = (2 * s.view_distance_chunks + 1) as usize;
    let full_side = (2 * s.lod_start_chunks + 1) as usize;
    ui.small(format!(
        "{} terrain columns, {} at full detail",
        side * side,
        full_side * full_side
    ));
    if s.view_distance_chunks > 16 {
        ui.label("Long distances increase terrain memory and loading cost substantially.");
    }
    ui.collapsing("Advanced rendering", |ui| {
        let (mut cpu, effective_gpu) = crate::render::world_residency_budgets_mib();
        let mut gpu = crate::render::gpu_residency_limit_mib();
        let mut automatic = gpu == 0;
        if ui.checkbox(&mut automatic,"Automatic GPU terrain budget (available VRAM)").changed() {
            gpu = if automatic {0} else {effective_gpu};
            crate::render::set_world_residency_budgets_mib(cpu,gpu);
        }
        let changed_cpu = ui.add(egui::Slider::new(&mut cpu, 32..=2048).logarithmic(true).text("CPU voxel cache (MiB)")).changed();
        let changed_gpu = !automatic && ui.add(egui::Slider::new(&mut gpu, 64..=crate::render::MAX_GPU_MIB).logarithmic(true).text("GPU terrain limit (MiB)")).changed();
        if changed_cpu || changed_gpu { crate::render::set_world_residency_budgets_mib(cpu, gpu); }
        let budget=crate::render::gpu_residency_status();
        ui.small(format!("Terrain allowance: {} MiB{}. Allocated only as terrain loads.",budget.limit_mib,
            if budget.automatic {" (automatic)"} else {" (manual limit, constrained by VRAM pressure)"}));
        if budget.detected {
            ui.small(format!("VRAM: {} MiB dedicated · OS allowance {} MiB · process {} MiB · terrain ceiling {} MiB{}",
                budget.dedicated_mib,budget.os_budget_mib,budget.process_usage_mib,budget.ceiling_mib,
                if budget.live {""} else {" (driver estimate)"}));
        } else { ui.small("VRAM information unavailable: conservative automatic fallback. A manual limit is available."); }
        if budget.deferred>0 {
            ui.colored_label(egui::Color32::from_rgb(235,190,115),format!("{} terrain chunks exceed the current memory allowance. Reduce terrain radius or increase a manual limit; automatic mode already leaves VRAM headroom.",budget.deferred));
        }
        ui.small("Near terrain takes priority. CPU includes cached source voxels and water levels held by mesh workers. Generation queues, mesh scratch, edited worlds and other rendering resources are separate.");
        let mut mesh = crate::render::mesh_budget_mib();
        if ui.add(egui::Slider::new(&mut mesh,160..=2048).logarithmic(true).text("Mesh heap: workers + results (MiB)")).changed() {crate::render::set_mesh_budget_mib(mesh);}
        let mut edited = crate::world::regions::edited_budget_mib();
        if ui.add(egui::Slider::new(&mut edited,16..=1024).logarithmic(true).text("Edited chunk cache target (MiB)")).changed() {crate::world::regions::set_edited_budget_mib(edited);}
        ui.small("Mesh memory is reserved before construction. Edited chunks are paged only after durable writeback; I/O failures retain unsaved data.");
        egui::ComboBox::from_label("Atmosphere")
            .selected_text(state.atmosphere_mode.label())
            .show_ui(ui, |ui| {
                for mode in AtmosphereMode::all() {
                    ui.selectable_value(&mut state.atmosphere_mode, mode, mode.label());
                }
            });
        egui::ComboBox::from_label("Diagnostics")
            .selected_text(state.debug_view.label())
            .show_ui(ui, |ui| {
                for view in RenderDebugView::all() {
                    ui.selectable_value(&mut state.debug_view, view, view.label());
                }
            });
    });
}

pub(super) fn environment_settings(
    ui: &mut egui::Ui,
    time_state: &mut TimeUiState,
    physics_state: &mut PhysicsUiState,
    action: &mut UiAction,
) {
    saving_settings(ui);
    ui.label("Day/Night Controls");
    ui.separator();

    ui.label(format!("Clock: {}", format_clock(time_state.time_of_day)));
    ui.add(
        egui::Slider::new(&mut time_state.time_of_day, 0.0..=1.0)
            .text("Time of day")
            .show_value(false),
    );
    if ui.button("Dawn").clicked() {
        time_state.time_of_day = 0.21;
    }
    if ui.button("Noon").clicked() {
        time_state.time_of_day = 0.50;
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Golden hour").clicked() {
            time_state.time_of_day = 0.728;
        }
        if ui.button("Sunset").clicked() {
            time_state.time_of_day = 0.746;
        }
        if ui.button("Afterglow").clicked() {
            time_state.time_of_day = 0.754;
        }
    });
    if ui.button("Midnight").clicked() {
        time_state.time_of_day = 0.00;
    }

    ui.separator();
    ui.checkbox(&mut time_state.freeze_time, "Freeze time");
    ui.add(
        egui::Slider::new(&mut time_state.day_length_seconds, 20.0..=1800.0)
            .logarithmic(true)
            .text("Day length (s)"),
    );
    ui.separator();
    cloud_controls_ui(ui, time_state, action);
    ui.separator();
    ui.checkbox(
        &mut physics_state.enabled,
        "Water flow and falling sand / gravel",
    );
    ui.small("Active water and falling materials continue across the world. Nearby activity gets priority; distant activity shares a fair work budget. Pause cellular physics to build suspended structures.");
    ui.small(format!(
        "{} active chunks · {} queued cells · {} updates · {} source loads",
        physics_state.stats.active_chunks,
        physics_state.stats.pending_updates,
        physics_state.stats.applied_updates_last_step,
        physics_state.stats.inflight_chunks
    ));
    ui.small(format!(
        "Simulation sources: {:.1} / 32 MiB",
        physics_state.stats.source_bytes as f32 / 1048576.0
    ));
    if physics_state.stats.recovering {
        ui.small("Resuming stored activity in the background…");
    }
    if physics_state.stats.writeback_pressure {
        ui.small(
            "Simulation is waiting for edited-chunk writeback. Unfinished activity is retained.",
        );
    }
    if physics_state.stats.source_failed {
        ui.colored_label(egui::Color32::LIGHT_YELLOW,"Some simulation data is unavailable. Unfinished work is retained and loads will retry.");
    }
}

pub(super) fn saving_settings(ui: &mut egui::Ui) {
    ui.collapsing("Saving", |ui| {
        let mut enabled = crate::world::storage::autosave_enabled();
        if ui.checkbox(&mut enabled, "Autosave world").changed() {
            crate::world::storage::set_autosave_enabled(enabled);
        }
        let mut seconds = crate::world::storage::autosave_seconds();
        if ui
            .add(egui::Slider::new(&mut seconds, 10..=300).text("Autosave interval (seconds)"))
            .changed()
        {
            crate::world::storage::set_autosave_seconds(seconds);
        }
        ui.small("Keeps recent recovery checkpoints. Closing the game waits for a final save.");
    });
}

pub(super) fn audio_settings(ui: &mut egui::Ui) {
    let mut volume = crate::audio::volume();
    if ui
        .add(egui::Slider::new(&mut volume, 0.0..=1.0).text("Sound effects"))
        .changed()
    {
        crate::audio::set_volume(volume);
    }
    ui.small(crate::audio::status());
}
