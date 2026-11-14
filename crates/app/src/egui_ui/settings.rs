//! Shared world creation and pause-menu graphics controls.
use super::{
    PhysicsUiState, RenderUiState, TimeUiState, UiAction, cloud_controls_ui, format_clock,
};
use crate::render::{AtmosphereMode, RenderDebugView, RenderDistanceSettings};

/// Terrain presets in metres, with chunk floors. Chunks are 32 voxels, so a
/// metre-only preset collapsed to 4 chunks at 1 m voxels and put 4 m LOD cells
/// 96 m from the camera. "Fast" saves frame time on clouds and bounce lighting
/// instead of turning nearby trees into blocks.
struct TerrainPreset {
    name: &'static str,
    hint: &'static str,
    view_m: f32,
    full_detail_m: f32,
    min_view_chunks: i32,
    min_full_chunks: i32,
    lod_levels: u8,
    cloud_scale: u32,
    clouds: bool,
    render_scale: u32,
    indirect: bool,
    horizon_m: u32,
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
        name: "Fast",
        hint: "256 m · 85% Resolution",
        view_m: 256.0,
        full_detail_m: 128.0,
        min_view_chunks: 8,
        min_full_chunks: 4,
        lod_levels: 2,
        cloud_scale: 4,
        clouds: true,
        render_scale: 85,
        indirect: false,
        horizon_m: 2048,
    },
    TerrainPreset {
        name: "Balanced",
        hint: "384 m · Native Resolution",
        view_m: 384.0,
        full_detail_m: 160.0,
        min_view_chunks: 12,
        min_full_chunks: 5,
        lod_levels: 3,
        cloud_scale: 4,
        clouds: true,
        render_scale: 100,
        indirect: false,
        horizon_m: 4096,
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
        clouds: true,
        render_scale: 100,
        indirect: true,
        horizon_m: 24576,
    },
];

pub(super) fn graphics_settings(ui: &mut egui::Ui, state: &mut RenderUiState, voxel_meters: f32) {
    let chunk_meters = 32.0 * voxel_meters;
    if ui.button("Reset Graphics").clicked() {
        let preset = &TERRAIN_PRESETS[1];
        state.settings = preset.settings(chunk_meters);
        state.fxaa_enabled = true;
        state.vsync = true;
        state.debug_view = RenderDebugView::default();
        state.atmosphere_mode = AtmosphereMode::default();
        crate::render::set_cloud_render_scale(preset.cloud_scale);
        crate::render::set_clouds_enabled(preset.clouds);
        crate::render::set_scene_scale_percent(preset.render_scale);
        crate::render::set_indirect_lighting_enabled(preset.indirect);
        crate::render::set_horizon_range_m(preset.horizon_m);
        crate::render::set_world_residency_budgets_mib(256, 0);
        crate::render::set_mesh_budget_mib(256);
        crate::render::shadow::set_enabled(true);
        crate::quality::set_auto_enabled(true);
    }
    ui.label("Quality");
    ui.horizontal_wrapped(|ui| {
        for preset in TERRAIN_PRESETS {
            if ui.button(preset.name).on_hover_text(preset.hint).clicked() {
                state.settings = preset.settings(chunk_meters);
                crate::render::set_cloud_render_scale(preset.cloud_scale);
                crate::render::set_clouds_enabled(preset.clouds);
                crate::render::set_scene_scale_percent(preset.render_scale);
                crate::render::set_indirect_lighting_enabled(preset.indirect);
                crate::render::set_horizon_range_m(preset.horizon_m);
            }
        }
    });
    let mut auto = crate::quality::auto_enabled();
    ui.checkbox(&mut auto, "Automatic Quality");
    crate::quality::set_auto_enabled(auto);
    ui.separator();
    let mut resolution = crate::render::scene_scale_percent();
    ui.add(
        egui::Slider::new(&mut resolution, crate::render::MIN_SCENE_SCALE_PERCENT..=100)
            .suffix("%")
            .text("Render Resolution"),
    );
    crate::render::set_scene_scale_percent(resolution);
    let mut clouds_on = crate::render::clouds_enabled();
    ui.checkbox(&mut clouds_on, "Clouds");
    crate::render::set_clouds_enabled(clouds_on);
    let mut indirect = crate::render::indirect_lighting_enabled();
    ui.checkbox(&mut indirect, "Bounce lighting and soft shadows");
    crate::render::set_indirect_lighting_enabled(indirect);
    ui.checkbox(&mut state.fxaa_enabled, "Smooth edges");
    ui.checkbox(&mut state.vsync, "VSync");
    // Radii are whole chunks; the slider works in chunks and labels metres so
    // every stop is a real setting at any voxel scale.
    let s = &mut state.settings;
    ui.add(
        egui::Slider::new(&mut s.view_distance_chunks, 4..=64)
            .custom_formatter(|v, _| format!("{:.0} m", v * chunk_meters as f64))
            .text("View distance"),
    );
    let full_limit = (s.view_distance_chunks - 1).max(2);
    s.lod_start_chunks = s.lod_start_chunks.clamp(2, full_limit);
    ui.collapsing("More options", |ui| {
        let mut clouds = crate::render::cloud_render_scale();
        ui.add_enabled_ui(clouds_on, |ui| {
            egui::ComboBox::from_label("Cloud detail")
                .selected_text(match clouds {
                    2 => "Ultra",
                    4 => "Low",
                    _ => "High",
                })
                .show_ui(ui, |ui| {
                    for (scale, label) in [(2, "Ultra"), (3, "High"), (4, "Low")] {
                        ui.selectable_value(&mut clouds, scale, label);
                    }
                });
        });
        crate::render::set_cloud_render_scale(clouds);
        ui.add(
            egui::Slider::new(&mut s.lod_start_chunks, 2..=full_limit)
                .custom_formatter(|v, _| format!("{:.0} m", v * chunk_meters as f64))
                .text("Sharp detail up to"),
        );
        ui.add(egui::Slider::new(&mut s.lod_levels, 1..=4).text("Distance detail levels"));
        let mut range_km = crate::render::horizon_range_m() as f32 / 1000.0;
        if ui
            .add(
                egui::Slider::new(
                    &mut range_km,
                    crate::render::MIN_HORIZON_RANGE_M as f32 / 1000.0
                        ..=crate::render::MAX_HORIZON_RANGE_M as f32 / 1000.0,
                )
                .logarithmic(true)
                .suffix(" km")
                .text("Far horizon"),
            )
            .changed()
        {
            crate::render::set_horizon_range_m((range_km * 1000.0).round() as u32);
        }
        egui::ComboBox::from_label("Sky")
            .selected_text(state.atmosphere_mode.label())
            .show_ui(ui, |ui| {
                for mode in AtmosphereMode::all() {
                    ui.selectable_value(&mut state.atmosphere_mode, mode, mode.label());
                }
            });
    });
    ui.collapsing("Memory", |ui| {
        let (mut cpu, effective_gpu) = crate::render::world_residency_budgets_mib();
        let mut gpu = crate::render::gpu_residency_limit_mib();
        let mut automatic = gpu == 0;
        if ui.checkbox(&mut automatic, "Automatic GPU memory limit").changed() {
            gpu = if automatic { 0 } else { effective_gpu };
            crate::render::set_world_residency_budgets_mib(cpu, gpu);
        }
        let changed_cpu = ui.add(egui::Slider::new(&mut cpu, 32..=2048).logarithmic(true).text("CPU cache (MiB)")).changed();
        let changed_gpu = !automatic
            && ui.add(egui::Slider::new(&mut gpu, 64..=crate::render::MAX_GPU_MIB).logarithmic(true).text("GPU limit (MiB)")).changed();
        if changed_cpu || changed_gpu {
            crate::render::set_world_residency_budgets_mib(cpu, gpu);
        }
        let budget = crate::render::gpu_residency_status();
        if budget.detected {
            ui.small(format!(
                "Terrain memory limit {} MiB of {} MiB video memory.",
                budget.limit_mib, budget.dedicated_mib
            ));
        } else {
            ui.small("Video memory size unknown; using a safe default.");
        }
        if budget.deferred > 0 {
            ui.colored_label(
                egui::Color32::from_rgb(235, 190, 115),
                format!("{} chunks are waiting for memory. Lower the view distance.", budget.deferred),
            );
        }
        let mut mesh = crate::render::mesh_budget_mib();
        if ui.add(egui::Slider::new(&mut mesh, 160..=2048).logarithmic(true).text("Mesh memory (MiB)")).changed() {
            crate::render::set_mesh_budget_mib(mesh);
        }
        let mut edited = crate::world::regions::edited_budget_mib();
        if ui.add(egui::Slider::new(&mut edited, 16..=1024).logarithmic(true).text("Edited chunks (MiB)")).changed() {
            crate::world::regions::set_edited_budget_mib(edited);
        }
        egui::ComboBox::from_label("Debug view")
            .selected_text(state.debug_view.label())
            .show_ui(ui, |ui| {
                for view in RenderDebugView::all() {
                    ui.selectable_value(&mut state.debug_view, view, view.label());
                }
            });
    });
}

/// Changes the open world's difficulty. The world saves it.
pub(super) fn difficulty_settings(
    ui: &mut egui::Ui,
    world_difficulty: crate::world::adventure::difficulty::Difficulty,
    action: &mut UiAction,
) {
    use crate::world::adventure::difficulty::Difficulty;
    ui.label("Difficulty");
    ui.horizontal_wrapped(|ui| {
        for level in Difficulty::ALL {
            let selected = world_difficulty == level;
            if ui.add(egui::Button::selectable(selected, level.label())).on_hover_text(level.hint()).clicked() && !selected {
                *action = UiAction::SetDifficulty(level);
            }
        }
    });
    ui.small(world_difficulty.hint());
}

pub(super) fn environment_settings(
    ui: &mut egui::Ui,
    time_state: &mut TimeUiState,
    physics_state: &mut PhysicsUiState,
    world_difficulty: crate::world::adventure::difficulty::Difficulty,
    action: &mut UiAction,
) {
    difficulty_settings(ui, world_difficulty, action);
    saving_settings(ui);
    ui.label(format!("Time: {}", format_clock(time_state.time_of_day)));
    ui.add(egui::Slider::new(&mut time_state.time_of_day, 0.0..=1.0).show_value(false));
    ui.horizontal_wrapped(|ui| {
        for (label, time) in [("Dawn", 0.21), ("Noon", 0.50), ("Dusk", 0.746), ("Midnight", 0.0)] {
            if ui.button(label).clicked() {
                time_state.time_of_day = time;
            }
        }
    });
    ui.checkbox(&mut time_state.freeze_time, "Freeze time");
    ui.add(
        egui::Slider::new(&mut time_state.day_length_seconds, 20.0..=1800.0)
            .logarithmic(true)
            .text("Day length (s)"),
    );
    ui.separator();
    cloud_controls_ui(ui, time_state, action);
    ui.separator();
    ui.checkbox(&mut physics_state.enabled, "Flowing water and falling sand")
        .on_hover_text("Turn off to build suspended structures.");
    if physics_state.stats.source_failed {
        ui.colored_label(egui::Color32::LIGHT_YELLOW, "Some world data is unavailable; it will retry.");
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
    let mut music = crate::audio::music_volume();
    if ui.add(egui::Slider::new(&mut music, 0.0..=1.0).text("Music")).changed() {
        crate::audio::set_music_volume(music);
    }
    let mut ambience = crate::audio::ambience_volume();
    if ui.add(egui::Slider::new(&mut ambience, 0.0..=1.0).text("Ambience")).changed() {
        crate::audio::set_ambience_volume(ambience);
    }
    ui.small(crate::audio::status());
}
