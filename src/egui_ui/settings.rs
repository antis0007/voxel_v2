//! Shared world creation and pause-menu graphics controls.
use super::{
    PhysicsUiState, RenderUiState, TimeUiState, UiAction, cloud_controls_ui, format_clock,
};
use crate::render::{AtmosphereMode, RenderDebugView, RenderDistanceSettings};

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
            2 => "High",
            4 => "Fast",
            _ => "Balanced",
        })
        .show_ui(ui, |ui| {
            for (scale, label) in [(2, "High"), (3, "Balanced"), (4, "Fast")] {
                ui.selectable_value(&mut clouds, scale, label);
            }
        });
    crate::render::set_cloud_render_scale(clouds);
    ui.small("Lower cloud detail improves frame rate; terrain and UI stay sharp.");
    ui.label("Terrain detail");
    ui.horizontal_wrapped(|ui| {
        for (name, distance, lod_start) in [
            ("Performance", 128.0, 48.0),
            ("Balanced", 512.0, 96.0),
            ("Far view", 2048.0, 128.0),
        ] {
            if ui.button(name).clicked() {
                state.settings = RenderDistanceSettings {
                    view_distance_chunks: ((distance / chunk_meters).round() as i32).clamp(4, 64),
                    lod_start_chunks: ((lod_start / chunk_meters).round() as i32).max(2),
                    lod_levels: 3,
                };
            }
        }
    });
    let mut meters = state.settings.view_distance_chunks as f32 * chunk_meters;
    if ui
        .add(
            egui::Slider::new(&mut meters, chunk_meters * 4.0..=chunk_meters * 64.0)
                .step_by(chunk_meters as f64)
                .text("View radius (m)"),
        )
        .changed()
    {
        state.settings.view_distance_chunks = (meters / chunk_meters).round() as i32;
    }
    ui.label(format!(
        "Terrain radius: {:.0} m",
        state.settings.view_distance_chunks as f32 * 32.0 * voxel_meters
    ));
    if state.settings.view_distance_chunks > 16 {
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
        let limit = state.settings.view_distance_chunks - 1;
        state.settings.lod_start_chunks = state.settings.lod_start_chunks.clamp(2, limit);
        ui.add(
            egui::Slider::new(&mut state.settings.lod_start_chunks, 2..=limit)
                .text("Full detail radius"),
        );
        ui.add(egui::Slider::new(&mut state.settings.lod_levels, 1..=4).text("Detail levels"));
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
