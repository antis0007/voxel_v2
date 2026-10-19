use egui::{Align2, Color32, RichText};
mod presentation;
mod front_end;
mod world_browser;
mod factory;
mod inventory;
mod adventure;
mod relics;
mod wands;
mod icons;
mod settings;
mod tool_hud;
use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor};
use glam::Vec3Swizzles;
use glam::{IVec3, Mat4, Quat, Vec3};
use std::path::Path;
use winit::{event::WindowEvent, window::Window};

use crate::creative::{AreaFootprintShape, BrushShape, CreativeState, ToolKind};
use crate::fluid::{FluidStats, VoxelScalePreset};
use crate::material::material_defs;
use crate::perf::{PerfSample, PerfSnapshot};
use crate::player::{FlyMode, Player};
use crate::render::{AtmosphereMode, RenderDebugView, RenderDistanceSettings};
use crate::world::{BlockId, CloudControls, PLAYER_HEIGHT, WorldType};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Loading,
    WorldSelect,
    Paused,
    Playing,
}

pub enum UiAction {
    None,
    Factory(crate::world::factory::Action),
    DropInventory(usize),
    StartWorld {
        name: String,
        reference: bool,
        world_type: WorldType,
        voxel_scale: VoxelScalePreset,
        seed: u32,
    },
    SetWeatherPreset(crate::world::WeatherPreset),
    RandomizeWeather,
    Resume,
    BackToWorldSelect,
    SaveWorld,
    OpenWorld(String),
    RefreshWorlds,
    RenameWorld { id: String, name: String },
    FavoriteWorld { id: String, favorite: bool },
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolUseAction {
    Place,
    Break,
}

#[derive(Clone, Debug)]
pub struct LoadingUiState {
    pub title: String,
    pub detail: String,
    pub progress_01: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct TimeUiState {
    pub time_of_day: f32,
    pub day_length_seconds: f32,
    pub freeze_time: bool,
    pub cloud_controls: CloudControls,
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicsUiState {
    pub enabled: bool,
    pub stats: FluidStats,
    pub voxel_scale: VoxelScalePreset,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderUiState {
    pub fxaa_enabled: bool,
    pub vsync: bool,
    pub settings: RenderDistanceSettings,
    pub debug_view: RenderDebugView,
    pub atmosphere_mode: AtmosphereMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragSource {
    Hotbar(usize),
    Palette(BlockId),
}

fn format_clock(time_of_day: f32) -> String {
    let minutes = ((time_of_day.rem_euclid(1.0) * 24.0 * 60.0).round() as i32).rem_euclid(24 * 60);
    let hour = minutes / 60;
    let minute = minutes % 60;
    format!("{hour:02}:{minute:02}")
}

fn next_seed(seed: u32) -> u32 {
    let mut x = seed ^ 0x9E37_79B9;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    if x == 0 { 0xA341_316C } else { x }
}

fn format_height_m(norm: f32, min_m: f32, max_m: f32) -> String {
    let meters = egui::lerp(min_m..=max_m, norm.clamp(0.0, 1.0));
    if meters >= 1000.0 {
        format!("{:.1} km", meters / 1000.0)
    } else {
        format!("{meters:.0} m")
    }
}

fn format_span_m(norm: f32, min_m: f32, max_m: f32) -> String {
    format_height_m(norm, min_m, max_m)
}

fn cloud_controls_ui(ui: &mut egui::Ui, time_state: &mut TimeUiState, action: &mut UiAction) {
    ui.strong("Weather System");
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.coverage, 0.0..=1.0)
            .text("Sky Coverage")
            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.density, 0.0..=1.0)
            .text("Cloud Density")
            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
    );
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut time_state.cloud_controls.wind_speed, 0.0..=1.0)
                .text("Wind Speed"),
        );
        ui.add(
            egui::Slider::new(&mut time_state.cloud_controls.wind_dir_angle, 0.0..=360.0)
                .text("° Dir"),
        );
    });

    ui.separator();

    ui.strong("Low Clouds  (70 – 250 m)");
    ui.label(
        egui::RichText::new("Stratus: flat grey sheets, no vertical development")
            .small()
            .weak(),
    );
    ui.add(egui::Slider::new(&mut time_state.cloud_controls.stratus, 0.0..=1.0).text("Stratus"));
    ui.label(
        egui::RichText::new("Cumulus: puffy, flat base, cauliflower tops")
            .small()
            .weak(),
    );
    ui.add(egui::Slider::new(&mut time_state.cloud_controls.cumulus, 0.0..=1.0).text("Cumulus"));
    ui.label(
        egui::RichText::new("Cumulonimbus: tall storm towers with anvil top")
            .small()
            .weak(),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.cumulonimbus, 0.0..=1.0)
            .text("Cumulonimbus (Storm)"),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.anvil_spread, 0.0..=1.0)
            .text("  Anvil Spread"),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.storm_thickness, 0.18..=1.0)
            .text("  Storm Tower Height")
            .custom_formatter(|v, _| format_span_m(v as f32, 90.0, 280.0)),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.base_height, 0.05..=0.72)
            .text("  Low-layer Base")
            .custom_formatter(|v, _| format_height_m(v as f32, 70.0, 220.0)),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.layer_thickness, 0.12..=1.0)
            .text("  Low-layer Depth")
            .custom_formatter(|v, _| format_span_m(v as f32, 40.0, 180.0)),
    );

    ui.separator();

    ui.strong("Mid Clouds  (180 – 340 m)");
    ui.label(
        egui::RichText::new("Altocumulus: grey/white cellular patches, mackerel sky")
            .small()
            .weak(),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.altocumulus, 0.0..=1.0)
            .text("Altocumulus"),
    );
    ui.label(
        egui::RichText::new("Altostratus: uniform grey veil, diffuses sunlight")
            .small()
            .weak(),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.altostratus, 0.0..=1.0)
            .text("Altostratus"),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.mid_layer_height, 0.20..=0.65)
            .text("  Mid-layer Base")
            .custom_formatter(|v, _| format_height_m(v as f32, 180.0, 320.0)),
    );

    ui.separator();

    ui.strong("High Clouds  (300 – 480 m)");
    ui.label(
        egui::RichText::new("Cirrus: thin ice-crystal wisps, 22° halo around sun")
            .small()
            .weak(),
    );
    ui.add(egui::Slider::new(&mut time_state.cloud_controls.cirrus, 0.0..=0.75).text("Cirrus"));
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.layer_separation, 0.08..=0.72)
            .text("  High-layer Base")
            .custom_formatter(|v, _| format_height_m(v as f32, 300.0, 460.0)),
    );

    ui.separator();

    ui.strong("Appearance");
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.erosion, 0.0..=1.0).text("Edge Erosion"),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.detail_strength, 0.0..=1.0)
            .text("Detail Sharpness"),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.silver_lining, 0.0..=1.0)
            .text("Silver Lining"),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.wind_shear, 0.0..=1.0)
            .text("Wind Shear / Layer Twist"),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.advection, 0.0..=1.0)
            .text("Advection Speed"),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.noise_scale, 0.0..=1.0).text("Cloud Size"),
    );

    ui.separator();

    ui.strong("Weather Presets");
    ui.horizontal(|ui| {
        if ui.button("Clear").clicked() {
            *action = UiAction::SetWeatherPreset(crate::world::WeatherPreset::Clear);
        }
        if ui.button("Partly Cloudy").clicked() {
            *action = UiAction::SetWeatherPreset(crate::world::WeatherPreset::PartlyCloudy);
        }
        if ui.button("Overcast").clicked() {
            *action = UiAction::SetWeatherPreset(crate::world::WeatherPreset::Overcast);
        }
        if ui.button("Stormy").clicked() {
            *action = UiAction::SetWeatherPreset(crate::world::WeatherPreset::Stormy);
        }
        if ui.button("Winter").clicked() {
            *action = UiAction::SetWeatherPreset(crate::world::WeatherPreset::WinterSky);
        }
    });

    if ui.button("Randomize Weather").clicked() {
        *action = UiAction::RandomizeWeather;
    }
}

fn creative_tools_ui(
    ui: &mut egui::Ui,
    creative_state: &mut CreativeState,
    physics_state: &PhysicsUiState,
    player: &mut Player,
    tool_textures: &ToolTextures,
) {
    ui.label("Creative tools");
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "Selected material: {}",
            creative_state.selected_block().display_name()
        ));
        ui.separator();
        ui.label(format!(
            "Hotbar slot: {}",
            hotbar_key_label(creative_state.selected_slot)
        ));
        ui.separator();
        ui.label("E inventory");
        ui.separator();
        ui.label("Q quick select");
    });
    ui.horizontal(|ui| {
        let palette_label = if creative_state.show_palette {
            "Hide Palette"
        } else {
            "Show Palette"
        };
        if ui.button(palette_label).clicked() {
            creative_state.show_palette = !creative_state.show_palette;
        }
        ui.label("Q toggles the quick menu.");
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        for tool in ToolKind::ALL {
            let texture = tool_textures.for_tool(tool);
            let mut button = egui::Button::image_and_text(
                egui::Image::new((texture.texture.id(), egui::vec2(24.0, 24.0)))
                    .texture_options(egui::TextureOptions::NEAREST),
                tool.label(),
            )
            .min_size(egui::vec2(120.0, 40.0));
            if creative_state.tool == tool {
                button = button.fill(Color32::from_rgb(64, 94, 138));
            }
            if ui.add(button).clicked() {
                creative_state.tool = tool;
            }
        }
    });

    ui.add_space(6.0);
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(18, 22, 28, 228))
        .stroke(egui::Stroke::new(
            1.0,
            tool_accent_color(creative_state.tool).linear_multiply(0.55),
        ))
        .corner_radius(egui::CornerRadius::same(0))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{}  ·  {}",
                    creative_state.tool.label(),
                    tool_detail_summary(creative_state, creative_state.tool)
                ))
                .strong()
                .color(tool_accent_color(creative_state.tool)),
            );
            let mut hovered_brush_shape = None;
            let mut hovered_area_shape = None;
            tool_settings_ui(
                ui,
                creative_state,
                &mut hovered_brush_shape,
                &mut hovered_area_shape,
            );
        });

    ui.separator();
    ui.label(RichText::new("Player").strong());
    ui.add(
        egui::Slider::new(&mut player.scale, 0.25..=3.0)
            .text("Player scale")
            .step_by(0.05),
    );
    ui.label(format!(
        "Effective height: {:.2} m ({:.2} voxels)",
        PLAYER_HEIGHT * player.scale,
        (PLAYER_HEIGHT * player.scale) / physics_state.voxel_scale.meters().max(1e-6),
    ));
    ui.horizontal(|ui| {
        ui.label("Fly mode");
        ui.selectable_value(
            &mut player.fly_mode,
            FlyMode::Creative,
            FlyMode::Creative.label(),
        );
        ui.selectable_value(
            &mut player.fly_mode,
            FlyMode::Noclip,
            FlyMode::Noclip.label(),
        );
    });
    ui.small("Shift: sneak / descend. Alt: placement preview.");

    ui.separator();
    ui.label(RichText::new("Controls").strong());
    ui.label(
        "LMB breaks or detonates, RMB places. Alt + Scroll changes tool size. Ctrl + Scroll changes reach.",
    );
    ui.label("Scroll cycles hotbar slots. 1-9 selects slots. Drag hotbar slots to reorder them.");
    ui.label("B toggles this toolkit, E toggles inventory, Q toggles the tool switcher.");
}

fn creative_tools_ui_plain(
    ui: &mut egui::Ui,
    creative_state: &mut CreativeState,
    physics_state: &PhysicsUiState,
    player: &mut Player,
    tool_textures: &ToolTextures,
) {
    ui.horizontal_wrapped(|ui| {
        for tool in ToolKind::ALL {
            if ui.selectable_label(creative_state.tool == tool,
                format!("{} {}", tool_shortcut_label(tool), tool.label())).clicked() {
                creative_state.tool = tool;
            }
        }
    });
    ui.separator();
    let mut brush = None;
    let mut area = None;
    tool_settings_ui(ui, creative_state, &mut brush, &mut area);
    ui.separator();
    ui.small("LMB remove · RMB place · B close");
    ui.collapsing("Player settings", |ui| {
        ui.add(egui::Slider::new(&mut player.scale, 0.25..=3.0).text("Player scale"));
        ui.selectable_value(&mut player.fly_mode, FlyMode::Creative, "Flight");
        ui.selectable_value(&mut player.fly_mode, FlyMode::Noclip, "Noclip");
    });
    let _ = (physics_state, tool_textures);
}

#[derive(Clone)]
struct ToolTexture {
    texture: egui::TextureHandle,
    size: [usize; 2],
}

#[derive(Clone)]
struct ToolTextures {
    hand: ToolTexture,
    brush: ToolTexture,
    builders_wand: ToolTexture,
    destructor_wand: ToolTexture,
    area_tool: ToolTexture,
    explosive_wand: ToolTexture,
}

impl ToolTextures {
    fn load(ctx: &egui::Context, dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        Self {
            hand: load_tool_texture(ctx, dir, "hand", [206, 168, 132, 255]),
            brush: load_tool_texture(ctx, dir, "brush", [120, 190, 255, 255]),
            builders_wand: load_tool_texture(ctx, dir, "builders_wand", [130, 225, 120, 255]),
            destructor_wand: load_tool_texture(ctx, dir, "destructor_wand", [255, 140, 120, 255]),
            area_tool: load_tool_texture(ctx, dir, "area_tool", [255, 220, 120, 255]),
            explosive_wand: load_tool_texture(ctx, dir, "explosive_wand", [255, 110, 35, 255]),
        }
    }

    fn for_tool(&self, tool: ToolKind) -> &ToolTexture {
        match tool {
            ToolKind::Hand => &self.hand,
            ToolKind::Brush => &self.brush,
            ToolKind::BuildersWand => &self.builders_wand,
            ToolKind::DestructorWand => &self.destructor_wand,
            ToolKind::AreaTool => &self.area_tool,
            ToolKind::ExplosiveWand => &self.explosive_wand,
        }
    }
}

#[derive(Default)]
struct HeldToolPoseState {
    action_phase: f32,
    equip_phase: f32,
    bob_time: f32,
    bob_weight: f32,
    last_tool: Option<ToolKind>,
    last_action: Option<ToolUseAction>,
    action_kind: Option<ToolUseAction>,
}

impl HeldToolPoseState {
    fn update(&mut self, tool: ToolKind, action: Option<ToolUseAction>, move_speed_mps: f32, dt: f32) {
        if self.last_tool != Some(tool) {
            self.last_tool = Some(tool);
            self.equip_phase = 1.0;
            self.action_phase = self.action_phase.min(0.28);
        }
        if action.is_some() && self.action_phase <= 0.0 {
            self.action_phase = 1.0;
            self.action_kind = action;
        }
        self.last_action = action;
        let duration = if self.action_kind == Some(ToolUseAction::Place) { 0.26 } else { 0.34 };
        self.action_phase = (self.action_phase - dt / duration).max(0.0);
        if self.action_phase <= 0.0 {
            self.action_kind = None;
        }
        self.equip_phase = (self.equip_phase - dt * 4.8).max(0.0);

        // Advance the gait by distance, not voxel-block speed, so its cadence
        // stays stable when the world uses 0.5m or 0.25m cells.
        let speed = move_speed_mps.clamp(0.0, 8.0);
        let stride_m = 3.8 + speed * 0.12;
        self.bob_time += dt * std::f32::consts::TAU * speed / stride_m.max(1.0);
        let gait_t = ((speed - 0.15) / 0.60).clamp(0.0, 1.0);
        let gait_t = gait_t * gait_t * (3.0 - 2.0 * gait_t);
        self.bob_weight += (gait_t - self.bob_weight) * (1.0 - (-dt * 12.0).exp());
    }
}

fn load_tool_texture(
    ctx: &egui::Context,
    dir: &Path,
    file_stem: &str,
    fallback: [u8; 4],
) -> ToolTexture {
    let path = dir.join(format!("{file_stem}.png"));
    let image = std::fs::read(path)
        .ok()
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .map(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw())
        })
        .unwrap_or_else(|| fallback_tool_texture(fallback));
    let size = image.size;
    let texture = ctx.load_texture(
        format!("tool_{file_stem}"),
        image,
        egui::TextureOptions::NEAREST,
    );
    ToolTexture { texture, size }
}

fn fallback_tool_texture(color: [u8; 4]) -> egui::ColorImage {
    let size = [16, 16];
    let mut pixels = vec![Color32::TRANSPARENT; size[0] * size[1]];
    for y in 0..size[1] {
        for x in 0..size[0] {
            let idx = y * size[0] + x;
            let border = x == 0 || y == 0 || x == size[0] - 1 || y == size[1] - 1;
            let checker = ((x / 4) + (y / 4)) % 2 == 0;
            pixels[idx] = if border {
                Color32::from_rgba_premultiplied(12, 12, 12, 235)
            } else if checker {
                Color32::from_rgba_premultiplied(color[0], color[1], color[2], color[3])
            } else {
                Color32::from_rgba_premultiplied(
                    color[0].saturating_sub(24),
                    color[1].saturating_sub(24),
                    color[2].saturating_sub(24),
                    color[3],
                )
            };
        }
    }
    egui::ColorImage {
        size,
        pixels,
        source_size: egui::vec2(size[0] as f32, size[1] as f32),
    }
}

struct ProjectedFace {
    vertices: [egui::Pos2; 4],
    uvs: Option<[egui::Pos2; 4]>,
    color: Color32,
    texture: Option<egui::TextureId>,
    depth: f32,
}

fn draw_held_tool_overlay(
    ctx: &egui::Context,
    pose: &HeldToolPoseState,
    texture: &ToolTexture,
    tool: ToolKind,
    time_of_day: f32,
) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("held_tool_overlay"),
    ));
    let rect = ctx.content_rect();

    // 1. Timing and animation states
    // action_phase drops 1.0 -> 0.0. swing_t maps to MC's attackValue 0.0 -> 1.0
    let swing_t = if pose.action_phase > 0.0 {
        1.0 - pose.action_phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let equip = pose.equip_phase.clamp(0.0, 1.0);
    let walk_t = pose.bob_time;

    // True 3D projection mapping
    let aspect = rect.width() / rect.height().max(1.0);
    let proj = Mat4::perspective_rh(70.0_f32.to_radians(), aspect, 0.01, 10.0);

    // 2. Base camera bobbing
    let bob_x = (walk_t * 1.0).sin() * 0.006 * pose.bob_weight;
    let bob_y = walk_t.cos() * 0.004 * pose.bob_weight;
    let bob_pitch = walk_t.sin() * 0.002 * pose.bob_weight;
    let bob_roll = walk_t.cos() * 0.002 * pose.bob_weight;

    let base_mat = Mat4::from_translation(Vec3::new(bob_x, bob_y, 0.0))
        * Mat4::from_quat(Quat::from_euler(
            glam::EulerRot::YXZ,
            0.0,
            bob_pitch,
            bob_roll,
        ));

    let mut faces = Vec::new();


    // Smooth anticipation, a short committed stroke, then a slower recovery.
    // No full-axis spins: the wrist stays attached to the forearm.
    let smooth = |t: f32| { let t = t.clamp(0.0, 1.0); t*t*(3.0-2.0*t) };
    let stroke = if swing_t < 0.28 { -0.18 * smooth(swing_t / 0.28) }
        else if swing_t < 0.48 { -0.18 + 1.18 * smooth((swing_t - 0.28) / 0.20) }
        else { 1.0 - smooth((swing_t - 0.48) / 0.52) };
    let placing = pose.action_kind == Some(ToolUseAction::Place);
    let swing_g = stroke;
    let swing_f = stroke;
    let equip_y = -equip * 0.6;

    if tool == ToolKind::Hand {
        // Compact expedition glove: articulated knuckles, stitched leather cuff
        // and a mineral-teal sleeve. Keep its silhouette below the sight line.
        let mat = base_mat
            * Mat4::from_translation(Vec3::new(0.82 - swing_g * if placing { 0.025 } else { 0.09 },
                -0.69 + swing_f * if placing { 0.035 } else { -0.035 } + equip_y,
                -1.26 - swing_f * if placing { 0.19 } else { 0.12 }))
            * Mat4::from_rotation_z((-10.0 - swing_g * if placing { 3.0 } else { 9.0 }).to_radians())
            * Mat4::from_rotation_x((-24.0 + swing_f * if placing { -6.0 } else { 10.0 }).to_radians())
            * Mat4::from_rotation_y(-22.0_f32.to_radians())
            * Mat4::from_scale(Vec3::splat(0.85));
        let sun_height = ((time_of_day - 0.25) * std::f32::consts::TAU).sin().max(0.0);
        let illumination = 0.30 + 0.70 * sun_height.sqrt();
        let leather = shade_color(Color32::from_rgb(111, 83, 57), illumination);
        let seam = shade_color(Color32::from_rgb(155, 125, 82), illumination);
        let cloth = shade_color(Color32::from_rgb(49, 73, 66), illumination);
        let dark = shade_color(Color32::from_rgb(65, 52, 41), illumination);
        let copper = shade_color(Color32::from_rgb(162, 113, 62), illumination);
        for (center, size, color) in [
            (Vec3::new(0.0,-0.34,0.0), Vec3::new(0.22,0.53,0.22), cloth),
            (Vec3::new(0.0,-0.12,0.0), Vec3::new(0.23,0.14,0.23), dark),
            (Vec3::new(0.0,-0.05,0.0), Vec3::new(0.235,0.025,0.235), seam),
            (Vec3::new(0.0,0.095,0.0), Vec3::new(0.185,0.25,0.19), leather),
            (Vec3::new(0.018,0.08,0.099), Vec3::new(0.10,0.13,0.012), dark),
            (Vec3::new(0.018,0.09,0.108), Vec3::new(0.08,0.10,0.010), leather),
            (Vec3::new(-0.108,0.055,0.015), Vec3::new(0.065,0.13,0.12), leather),
            (Vec3::new(0.0,-0.12,0.123), Vec3::new(0.065,0.065,0.018), copper),
            (Vec3::new(0.0,-0.12,0.135), Vec3::new(0.035,0.035,0.01), dark),
        ] {
            add_3d_box(&mut faces, mat, proj, rect, center, size, color);
        }
        for finger in 0..4 {
            let x = -0.069 + finger as f32 * 0.046;
            let lift = if finger == 1 || finger == 2 { 0.012 } else { 0.0 };
            let open = if placing { stroke.max(0.0) } else { 0.0 };
            add_3d_box(&mut faces, mat, proj, rect, Vec3::new(x,0.23+lift+open*0.025,-open*0.035),
                Vec3::new(0.039,0.065+open*0.035,0.15), leather);
            add_3d_box(&mut faces, mat, proj, rect, Vec3::new(x,0.16,0.099),
                Vec3::new(0.024,0.045,0.012), seam);
        }
        for stitch in 0..5 {
            add_3d_box(&mut faces, mat, proj, rect,
                Vec3::new(-0.078 + stitch as f32 * 0.039,-0.025,0.104),
                Vec3::new(0.012,0.018,0.012), seam);
        }
    } else {
        // Solid, camera-relative wand with a quiet wrist pivot instead of a spinning sprite.
        let accent = match tool {
            ToolKind::BuildersWand => Color32::from_rgb(99, 204, 196),
            ToolKind::DestructorWand => Color32::from_rgb(225, 132, 99),
            ToolKind::ExplosiveWand => Color32::from_rgb(236, 176, 76),
            _ => Color32::from_rgb(161, 164, 216),
        };
        let mat = base_mat * Mat4::from_translation(Vec3::new(
            0.65 - stroke * 0.045, -0.48 + equip_y - stroke * 0.025, -1.3 - stroke * 0.14))
            * Mat4::from_rotation_z((-12.0 - stroke * 7.0).to_radians())
            * Mat4::from_rotation_x((-14.0 + stroke * 10.0).to_radians());
        let wood = Color32::from_rgb(89, 65, 48);
        let metal = Color32::from_rgb(159, 129, 80);
        for (center, size, color) in [
            (Vec3::new(0.0,-0.21,0.0), Vec3::new(0.055,0.58,0.055),wood),
            (Vec3::new(0.0,-0.09,0.0), Vec3::new(0.075,0.14,0.075),Color32::from_rgb(61,47,42)),
            (Vec3::new(0.0,0.06,0.0), Vec3::new(0.09,0.045,0.09),metal),
            (Vec3::new(0.0,0.15,0.0), Vec3::new(0.085,0.13,0.085),accent),
            (Vec3::new(0.0,0.23,0.0), Vec3::new(0.045,0.04,0.045),accent),
            (Vec3::new(-0.05,0.13,0.0), Vec3::new(0.018,0.13,0.06),metal),
            (Vec3::new(0.05,0.13,0.0), Vec3::new(0.018,0.13,0.06),metal),
            (Vec3::new(0.035,-0.13,0.045), Vec3::new(0.13,0.15,0.12),Color32::from_rgb(111,83,57)),
            (Vec3::new(0.06,-0.35,0.055), Vec3::new(0.15,0.29,0.15),Color32::from_rgb(49,73,66)),
        ] { add_3d_box(&mut faces, mat, proj, rect, center, size, color); }
    }
    // True Depth sorting (Painter's Algorithm) guarantees perfect face overlapping
    faces.sort_by(|a, b| {
        b.depth
            .partial_cmp(&a.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for face in faces {
        if let (Some(uvs), Some(tex_id)) = (face.uvs, face.texture) {
            let mut mesh = egui::Mesh::with_texture(tex_id);
            for i in 0..4 {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: face.vertices[i],
                    uv: uvs[i],
                    color: face.color,
                });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(egui::Shape::mesh(mesh));
        } else {
            // Solid triangles avoid enormous stroke miters on edge-on seams.
            let mut mesh = egui::Mesh::default();
            for pos in face.vertices {
                mesh.colored_vertex(pos, face.color);
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}

fn draw_held_weapon_overlay(ctx: &egui::Context, tier: u32, pose: &HeldToolPoseState) {
    if tier == 0 { return; }
    let screen=ctx.content_rect();
    let attack=if pose.action_phase>0.0 { (1.0-pose.action_phase).clamp(0.0,1.0) } else { 0.0 };
    let swing=(attack*std::f32::consts::PI).sin();
    let size=egui::vec2(84.0,84.0);
    let origin=screen.right_bottom()-egui::vec2(size.x+18.0, size.y+88.0)+egui::vec2(-swing*26.0,swing*18.0);
    let rect=egui::Rect::from_min_size(origin,size);
    let painter=ctx.layer_painter(egui::LayerId::new(egui::Order::Background,egui::Id::new("held_pixel_sword")));
    icons::paint(&painter,rect,icons::Icon::Sword(tier));
}

fn add_3d_box(
    faces: &mut Vec<ProjectedFace>,
    transform: Mat4,
    proj: Mat4,
    rect: egui::Rect,
    center: Vec3,
    size: Vec3,
    base_color: Color32,
) {
    let h = size * 0.5;
    let c = [
        Vec3::new(-h.x, -h.y, -h.z), // 0: L B B
        Vec3::new(h.x, -h.y, -h.z),  // 1: R B B
        Vec3::new(h.x, h.y, -h.z),   // 2: R T B
        Vec3::new(-h.x, h.y, -h.z),  // 3: L T B
        Vec3::new(-h.x, -h.y, h.z),  // 4: L B F
        Vec3::new(h.x, -h.y, h.z),   // 5: R B F
        Vec3::new(h.x, h.y, h.z),    // 6: R T F
        Vec3::new(-h.x, h.y, h.z),   // 7: L T F
    ];
    let face_data = [
        ([7, 6, 5, 4], Vec3::Z, 0.95),  // Front
        ([2, 3, 0, 1], -Vec3::Z, 0.70), // Back
        ([3, 2, 6, 7], Vec3::Y, 1.15),  // Top
        ([4, 5, 1, 0], -Vec3::Y, 0.50), // Bottom
        ([6, 2, 1, 5], Vec3::X, 0.85),  // Right
        ([3, 7, 4, 0], -Vec3::X, 0.85), // Left
    ];

    for (indices, normal, shade) in face_data {
        let mut v_world = [Vec3::ZERO; 4];
        for i in 0..4 {
            v_world[i] = transform.transform_point3(center + c[indices[i]]);
        }

        // True 3D Backface Culling
        let n_world = transform.transform_vector3(normal).normalize_or_zero();
        let face_center = (v_world[0] + v_world[1] + v_world[2] + v_world[3]) * 0.25;
        if face_center.dot(n_world) > 0.0 {
            continue;
        }

        let mut v_screen = [egui::Pos2::ZERO; 4];
        let mut avg_z = 0.0;
        let mut clipped = false;

        for i in 0..4 {
            let clip = proj * v_world[i].extend(1.0);
            if clip.w <= 0.01 {
                clipped = true;
                break;
            }
            let ndc = clip.truncate() / clip.w;
            avg_z += clip.w;
            v_screen[i] = egui::pos2(
                (ndc.x * 0.5 + 0.5) * rect.width() + rect.min.x,
                (-ndc.y * 0.5 + 0.5) * rect.height() + rect.min.y,
            );
        }
        if clipped {
            continue;
        }

        faces.push(ProjectedFace {
            vertices: v_screen,
            uvs: None,
            color: shade_color(base_color, shade),
            texture: None,
            depth: avg_z * 0.25,
        });
    }
}

fn add_3d_quad(
    faces: &mut Vec<ProjectedFace>,
    transform: Mat4,
    proj: Mat4,
    rect: egui::Rect,
    v_local: [Vec3; 4],
    uvs: [egui::Pos2; 4],
    texture: egui::TextureId,
    color: Color32,
) {
    // Subdivide the quad to minimize affine texture mapping distortion in egui
    let sub_divs = 8;
    for y in 0..sub_divs {
        for x in 0..sub_divs {
            let tx0 = x as f32 / sub_divs as f32;
            let tx1 = (x + 1) as f32 / sub_divs as f32;
            let ty0 = y as f32 / sub_divs as f32;
            let ty1 = (y + 1) as f32 / sub_divs as f32;

            let lerp_v = |tx: f32, ty: f32| {
                let bottom = v_local[0].lerp(v_local[1], tx);
                let top = v_local[3].lerp(v_local[2], tx);
                bottom.lerp(top, ty)
            };

            let lerp_uv = |tx: f32, ty: f32| {
                let bottom = uvs[0].lerp(uvs[1], tx);
                let top = uvs[3].lerp(uvs[2], tx);
                bottom.lerp(top, ty)
            };

            let sub_v = [
                lerp_v(tx0, ty0),
                lerp_v(tx1, ty0),
                lerp_v(tx1, ty1),
                lerp_v(tx0, ty1),
            ];

            let sub_uvs = [
                lerp_uv(tx0, ty0),
                lerp_uv(tx1, ty0),
                lerp_uv(tx1, ty1),
                lerp_uv(tx0, ty1),
            ];

            let mut v_world = [Vec3::ZERO; 4];
            for i in 0..4 {
                v_world[i] = transform.transform_point3(sub_v[i]);
            }

            let mut v_screen = [egui::Pos2::ZERO; 4];
            let mut avg_z = 0.0;
            let mut clipped = false;

            for i in 0..4 {
                let clip = proj * v_world[i].extend(1.0);
                if clip.w <= 0.01 {
                    clipped = true;
                    break;
                }
                let ndc = clip.truncate() / clip.w;
                avg_z += clip.w;
                v_screen[i] = egui::pos2(
                    (ndc.x * 0.5 + 0.5) * rect.width() + rect.min.x,
                    (-ndc.y * 0.5 + 0.5) * rect.height() + rect.min.y,
                );
            }

            if !clipped {
                faces.push(ProjectedFace {
                    vertices: v_screen,
                    uvs: Some(sub_uvs),
                    color,
                    texture: Some(texture),
                    depth: avg_z * 0.25,
                });
            }
        }
    }
}

fn material_color32(block: BlockId) -> Color32 {
    let color = block.color(IVec3::ZERO);
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_premultiplied(to_u8(color[0]), to_u8(color[1]), to_u8(color[2]), 255)
}

fn shade_color(color: Color32, factor: f32) -> Color32 {
    let scale = |c: u8| ((c as f32 * factor).clamp(0.0, 255.0)).round() as u8;
    Color32::from_rgba_premultiplied(
        scale(color.r()),
        scale(color.g()),
        scale(color.b()),
        color.a(),
    )
}

fn hotbar_key_label(slot: usize) -> &'static str {
    match slot {
        0 => "1",
        1 => "2",
        2 => "3",
        3 => "4",
        4 => "5",
        5 => "6",
        6 => "7",
        7 => "8",
        8 => "9",
        _ => "?",
    }
}

fn status_chip(ui: &mut egui::Ui, text: impl Into<String>, accent: Color32) {
    egui::Frame::new()
        .fill(accent.linear_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, accent.linear_multiply(0.72)))
        .corner_radius(egui::CornerRadius::same(0))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(Color32::WHITE));
        });
}

fn format_compact_count(value: usize) -> String {
    match value {
        1_000_000.. => format!("{:.2}M", value as f32 / 1_000_000.0),
        10_000.. => format!("{:.1}k", value as f32 / 1_000.0),
        _ => value.to_string(),
    }
}

fn format_compact_bytes(bytes: usize) -> String {
    const KIB: f32 = 1024.0;
    const MIB: f32 = KIB * 1024.0;

    let bytes_f32 = bytes as f32;
    if bytes_f32 >= MIB {
        format!("{:.2} MiB", bytes_f32 / MIB)
    } else if bytes_f32 >= KIB {
        format!("{:.1} KiB", bytes_f32 / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn format_graph_metric(value: f32, unit: &str) -> String {
    match unit {
        "ms" => format!("{value:.2} ms"),
        "chunks" | "draws" => format!("{value:.0}"),
        "k tris" => format!("{value:.1}k"),
        _ if unit.is_empty() => format!("{value:.2}"),
        _ => format!("{value:.1} {unit}"),
    }
}

fn perf_metric_card(
    ui: &mut egui::Ui,
    label: &str,
    value: impl Into<String>,
    detail: impl Into<String>,
    accent: Color32,
) {
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(18, 22, 30, 236))
        .stroke(egui::Stroke::new(1.0, accent.linear_multiply(0.82)))
        .corner_radius(egui::CornerRadius::same(0))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(170.0, 74.0));
            ui.label(
                RichText::new(label)
                    .small()
                    .color(Color32::from_rgb(184, 192, 202)),
            );
            ui.label(
                RichText::new(value.into())
                    .size(24.0)
                    .strong()
                    .color(accent),
            );
            ui.label(
                RichText::new(detail.into())
                    .small()
                    .color(Color32::from_rgb(146, 155, 168)),
            );
        });
}

fn perf_table_header(ui: &mut egui::Ui) {
    ui.label(RichText::new("Metric").small().strong());
    ui.label(RichText::new("Avg").small().strong());
    ui.label(RichText::new("Latest").small().strong());
    ui.end_row();
}

fn perf_table_row(
    ui: &mut egui::Ui,
    label: &str,
    avg: impl Into<String>,
    latest: impl Into<String>,
) {
    ui.label(RichText::new(label).color(Color32::from_rgb(210, 216, 224)));
    ui.label(RichText::new(avg.into()).monospace());
    ui.label(RichText::new(latest.into()).monospace());
    ui.end_row();
}

fn perf_value_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.label(RichText::new(label).color(Color32::from_rgb(210, 216, 224)));
    ui.label(RichText::new(value.into()).monospace());
    ui.end_row();
}

fn perf_history_series<F>(history: &[PerfSample], mut map: F) -> Vec<f32>
where
    F: FnMut(&PerfSample) -> f32,
{
    history.iter().map(|sample| map(sample).max(0.0)).collect()
}

fn perf_history_count_series<F>(history: &[PerfSample], mut map: F) -> Vec<f32>
where
    F: FnMut(&PerfSample) -> usize,
{
    history.iter().map(|sample| map(sample) as f32).collect()
}

fn draw_perf_graph(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    lines: &[(&str, &[f32], Color32)],
    unit: &str,
    target_line: Option<f32>,
) {
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(18, 22, 30, 236))
        .stroke(egui::Stroke::new(
            1.0,
            Color32::from_rgba_premultiplied(255, 255, 255, 28),
        ))
        .corner_radius(egui::CornerRadius::same(0))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(title).strong().size(16.0));
                for (label, values, color) in lines {
                    if let Some(last) = values.last().copied() {
                        status_chip(
                            ui,
                            format!("{label} {}", format_graph_metric(last, unit)),
                            *color,
                        );
                    }
                }
            });
            if !subtitle.is_empty() {
                ui.label(
                    RichText::new(subtitle)
                        .small()
                        .color(Color32::from_rgb(146, 155, 168)),
                );
            }
            ui.add_space(8.0);

            let desired = egui::vec2(ui.available_width().max(120.0), 156.0);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let rounding = egui::CornerRadius::same(0);
            painter.rect_filled(
                rect,
                rounding,
                Color32::from_rgba_premultiplied(11, 14, 20, 240),
            );
            painter.rect_stroke(
                rect,
                rounding,
                egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 18)),
                egui::StrokeKind::Inside,
            );

            let sample_count = lines
                .iter()
                .map(|(_, values, _)| values.len())
                .max()
                .unwrap_or(0);
            if sample_count == 0 {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Collecting samples...",
                    egui::FontId::proportional(13.0),
                    Color32::from_rgb(136, 145, 158),
                );
                return;
            }

            let max_value = lines
                .iter()
                .flat_map(|(_, values, _)| values.iter().copied())
                .chain(target_line)
                .fold(0.0f32, f32::max)
                .max(1.0);
            let plot_rect = rect.shrink2(egui::vec2(10.0, 10.0));
            let grid_color = Color32::from_rgba_premultiplied(255, 255, 255, 14);

            for step in 0..=3 {
                let t = step as f32 / 3.0;
                let y = egui::lerp(plot_rect.bottom()..=plot_rect.top(), t);
                painter.line_segment(
                    [
                        egui::pos2(plot_rect.left(), y),
                        egui::pos2(plot_rect.right(), y),
                    ],
                    egui::Stroke::new(1.0, grid_color),
                );
            }

            if let Some(target) = target_line {
                let y = egui::lerp(
                    plot_rect.bottom()..=plot_rect.top(),
                    (target / max_value).clamp(0.0, 1.0),
                );
                painter.line_segment(
                    [
                        egui::pos2(plot_rect.left(), y),
                        egui::pos2(plot_rect.right(), y),
                    ],
                    egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 215, 96, 130)),
                );
                painter.text(
                    egui::pos2(plot_rect.right() - 6.0, y - 2.0),
                    Align2::RIGHT_BOTTOM,
                    format_graph_metric(target, unit),
                    egui::FontId::proportional(10.0),
                    Color32::from_rgb(255, 222, 132),
                );
            }

            painter.text(
                egui::pos2(plot_rect.left() + 4.0, plot_rect.top() + 4.0),
                Align2::LEFT_TOP,
                format!("Max {}", format_graph_metric(max_value, unit)),
                egui::FontId::proportional(10.0),
                Color32::from_rgb(126, 135, 148),
            );

            for (_label, values, color) in lines {
                if values.is_empty() {
                    continue;
                }

                let points: Vec<_> = if values.len() == 1 {
                    vec![egui::pos2(
                        plot_rect.left(),
                        egui::lerp(
                            plot_rect.bottom()..=plot_rect.top(),
                            (values[0] / max_value).clamp(0.0, 1.0),
                        ),
                    )]
                } else {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            let t = index as f32 / (values.len() - 1) as f32;
                            egui::pos2(
                                egui::lerp(plot_rect.left()..=plot_rect.right(), t),
                                egui::lerp(
                                    plot_rect.bottom()..=plot_rect.top(),
                                    (*value / max_value).clamp(0.0, 1.0),
                                ),
                            )
                        })
                        .collect()
                };

                if points.len() == 1 {
                    painter.circle_filled(points[0], 3.5, *color);
                } else {
                    painter.add(egui::Shape::line(points, egui::Stroke::new(2.0, *color)));
                }
            }
        });
}

fn tool_accent_color(tool: ToolKind) -> Color32 {
    match tool {
        ToolKind::Hand => Color32::from_rgb(202, 170, 138),
        ToolKind::Brush => Color32::from_rgb(120, 190, 255),
        ToolKind::BuildersWand => Color32::from_rgb(130, 225, 120),
        ToolKind::DestructorWand => Color32::from_rgb(255, 140, 120),
        ToolKind::AreaTool => Color32::from_rgb(255, 220, 120),
        ToolKind::ExplosiveWand => Color32::from_rgb(255, 110, 35),
    }
}

fn tool_shortcut_label(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Hand => "Z",
        ToolKind::Brush => "X",
        ToolKind::BuildersWand => "C",
        ToolKind::DestructorWand => "V",
        ToolKind::AreaTool => "G",
        ToolKind::ExplosiveWand => "H",
    }
}

fn tool_description(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Hand => "Snap-aligned building and mining, with a single-voxel detail mode.",
        ToolKind::Brush => "Radius-based sculpting for fast terrain painting and erasing.",
        ToolKind::BuildersWand => "Flood-style placement that grows from the target surface.",
        ToolKind::DestructorWand => "Flood-style removal for stripping large connected regions.",
        ToolKind::AreaTool => "Footprint-based bulk edits for discs, pads, and layered cuts.",
        ToolKind::ExplosiveWand => "Detonate the highlighted sphere, clearing blocks and shifting nearby terrain.",
    }
}

fn tool_detail_summary(creative_state: &CreativeState, tool: ToolKind) -> String {
    match tool {
        ToolKind::Hand => format!("{:.2}m width · {:.1}m reach",if creative_state.size_in_metres {creative_state.hand_size_m} else {creative_state.active_voxel_m},creative_state.placement_distance),
        ToolKind::Brush => format!(
            "{} radius {} @ {:.1}m",
            creative_state.brush_shape.label(),
            creative_state.brush_radius,
            creative_state.placement_distance
        ),
        ToolKind::BuildersWand => format!(
            "Place cap {} @ {:.1}m",
            creative_state.wand_max_blocks, creative_state.placement_distance
        ),
        ToolKind::DestructorWand => format!(
            "Break cap {} @ {:.1}m",
            creative_state.wand_max_blocks, creative_state.placement_distance
        ),
        ToolKind::AreaTool => format!(
            "{} radius {} x thickness {} @ {:.1}m",
            creative_state.area_tool.shape.label(),
            creative_state.area_tool.radius,
            creative_state.area_tool.thickness,
            creative_state.placement_distance
        ),
        ToolKind::ExplosiveWand => format!(
            "Radius {} blocks @ {:.1}m reach",
            creative_state.explosion_radius, creative_state.placement_distance
        ),
    }
}

fn tool_scale_ui(ui:&mut egui::Ui,c:&mut CreativeState) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Tool sizes");
        ui.selectable_value(&mut c.size_in_metres,true,"World metres");
        ui.selectable_value(&mut c.size_in_metres,false,"Voxel units");
    });
    ui.small(format!("Voxel size: {:.2} m. Reach stays in metres.",c.active_voxel_m));
    if c.pending_cells()>0 {ui.label(format!("Working: {} voxels remaining",c.pending_cells()));}
}
fn tool_settings_ui(
    ui: &mut egui::Ui,
    creative_state: &mut CreativeState,
    hovered_brush_shape: &mut Option<BrushShape>,
    hovered_area_shape: &mut Option<AreaFootprintShape>,
) {
    *hovered_brush_shape = None;
    *hovered_area_shape = None;

    ui.label(RichText::new("Reach").strong());
    ui.add(egui::Slider::new(&mut creative_state.placement_distance, 2.0..=24.0)
        .step_by(0.5).suffix(" m").text("Maximum distance"));
    ui.horizontal(|ui| {
        for metres in [3.0, 5.0, 10.0, 16.0, 24.0] {
            if ui.selectable_label((creative_state.placement_distance - metres).abs() < 0.01,
                format!("{metres:.0} m")).clicked() {
                creative_state.placement_distance = metres;
            }
        }
    });
    ui.small("While aiming: Ctrl + wheel or [ / ]. Alt + wheel changes size.");
    ui.separator();
    tool_scale_ui(ui,creative_state);
    let radius_limit=creative_state.radius_limit();
    let units=creative_state.size_unit();
    match creative_state.tool {
        ToolKind::Hand => {
            ui.label("Snap-aligned mining and building");
            let max_width = creative_state.hand_width_limit();
            ui.add_enabled(creative_state.size_in_metres,
                egui::Slider::new(&mut creative_state.hand_size_m, creative_state.active_voxel_m..=max_width)
                    .step_by(creative_state.active_voxel_m as f64).text("Width (m)"));
        }
        ToolKind::Brush => {
            ui.label("Brush shape");
            ui.horizontal(|ui| {
                for shape in BrushShape::ALL {
                    let response =
                        ui.selectable_label(creative_state.brush_shape == shape, shape.label());
                    if response.hovered() {
                        *hovered_brush_shape = Some(shape);
                    }
                    if response.clicked() {
                        creative_state.brush_shape = shape;
                    }
                }
            });
            ui.add(egui::Slider::new(&mut creative_state.brush_radius, 0..=radius_limit).text(format!("Radius ({units})")));

        }
        ToolKind::BuildersWand | ToolKind::DestructorWand => {
            let cap_label = if matches!(creative_state.tool, ToolKind::BuildersWand) {
                "Placement cap"
            } else {
                "Removal cap"
            };
            ui.small("Follows connected matching material on the aimed face.");
            ui.add(egui::Slider::new(&mut creative_state.wand_max_blocks, 1..=512)
                .text(format!("{cap_label} ({})", if creative_state.size_in_metres { "m²" } else { "faces" })));
            ui.horizontal(|ui| {
                for cap in [1, 4, 8, 16, 64, 256] {
                    if ui.selectable_label(creative_state.wand_max_blocks == cap, cap.to_string()).clicked() {
                        creative_state.wand_max_blocks = cap;
                    }
                }
            });

        }
        ToolKind::AreaTool => {
            ui.label("Footprint");
            ui.horizontal(|ui| {
                for shape in AreaFootprintShape::ALL {
                    let response =
                        ui.selectable_label(creative_state.area_tool.shape == shape, shape.label());
                    if response.hovered() {
                        *hovered_area_shape = Some(shape);
                    }
                    if response.clicked() {
                        creative_state.area_tool.shape = shape;
                    }
                }
            });
            ui.add(egui::Slider::new(&mut creative_state.area_tool.radius, 0..=radius_limit).text(format!("Radius ({units})")));
            ui.add(
                egui::Slider::new(&mut creative_state.area_tool.thickness, 1..=8).text(format!("Depth ({units})")),
            );

        }
        ToolKind::ExplosiveWand => {
            ui.label("Blast size");
            ui.add(egui::Slider::new(&mut creative_state.explosion_radius, 1..=radius_limit).text(format!("Blast radius ({units})")));
            ui.small("Left click detonates. Nearby sand and gravel fall; water flows into the opening.");
        }
    }
}

fn draw_block_icon_face(
    painter: &egui::Painter,
    face: &[egui::Pos2],
    block: BlockId,
    color: Color32,
    top: bool,
) {
    let point = |u: f32, v: f32| face[0] + (face[1] - face[0]) * u + (face[3] - face[0]) * v;
    for y in 0..8 {
        for x in 0..8 {
            let hash = ((x * 13 + y * 7 + block.as_u32() * 3) % 11) as f32;
            let mut tint = shade_color(color, 0.96 + hash * 0.008);
            match block {
                BlockId::Grass | BlockId::SnowGrass if !top && y > 1 + (x%3==0) as u32 => {
                    tint = shade_color(material_color32(BlockId::Dirt), 0.78 + hash * 0.012);
                }
                BlockId::Log if top => {
                    tint = shade_color(
                        material_color32(BlockId::Planks),
                        if ((x as f32-3.5).hypot(y as f32-3.5) as u32)%2==0 { 1.02 } else { 0.85 },
                    );
                }
                BlockId::Log => tint = shade_color(color, if (x+y/4)%4==0 {0.69}else{0.99}),
                BlockId::Planks if y % 3 == 2 => tint = shade_color(color, 0.66),
                BlockId::StoneBricks | BlockId::WeatheredBricks if y % 4 == 3 || (x + y / 4 * 4) % 8 == 0 => {
                    tint = shade_color(color, 0.58)
                }
                BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush => {
                    tint = shade_color(color, match (x%3,y%3) {(0,0)=>1.18,(2,2)=>0.76,_=>0.96});
                }
                BlockId::Sand | BlockId::Sandstone => tint = shade_color(color,0.96+(y%3) as f32*0.025),
                BlockId::CoalOre if (x/2+y/3)%3==1 && hash<7.0 => tint = Color32::from_rgb(47, 54, 56),
                BlockId::IronOre if (x/2+y/3)%3==1 && hash<7.0 => tint = Color32::from_rgb(164, 112, 77),
                BlockId::CopperOre if (x/2+y/3)%3==1 && hash<7.0 => tint = Color32::from_rgb(178, 128, 78),
                BlockId::Stone | BlockId::Basalt | BlockId::Limestone => {
                    tint = shade_color(color, 0.89+((x/3+y/2)%3) as f32*0.065);
                }
                _ => {}
            }
            let u = x as f32 / 8.0;
            let v = y as f32 / 8.0;
            let mut mesh=egui::Mesh::default();
            for p in [point(u,v),point(u+0.125,v),point(u+0.125,v+0.125),point(u,v+0.125)] {mesh.colored_vertex(p,tint);}
            mesh.add_triangle(0,1,2);mesh.add_triangle(0,2,3);
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}

fn draw_voxel_icon(painter: &egui::Painter, rect: egui::Rect, block: BlockId) {
    if block.is_torch() {
        icons::paint(painter, rect, icons::Icon::Torch);
        return;
    }
    if let Some(kind)=crate::world::factory::Kind::from_block(block) {
        icons::paint(painter,rect,icons::Icon::Machine(kind)); return;
    }
    let base = material_color32(block);
    let top = shade_color(base, 1.18);
    let left = shade_color(base, 0.92);
    let right = shade_color(base, 0.74);
    let cube = rect.width().min(rect.height()) * 0.72;
    let half_w = cube * 0.46;
    let half_h = cube * 0.23;
    let depth = cube * 0.44;
    let center = egui::pos2(rect.center().x, rect.center().y - cube * 0.12);

    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(
                center.x - half_w * 0.75,
                center.y + half_h + depth + cube * 0.35,
            ),
            egui::pos2(
                center.x + half_w * 0.75,
                center.y + half_h + depth + cube * 0.35,
            ),
            egui::pos2(
                center.x + half_w * 0.45,
                center.y + half_h + depth + cube * 0.8,
            ),
            egui::pos2(
                center.x - half_w * 0.45,
                center.y + half_h + depth + cube * 0.8,
            ),
        ],
        Color32::from_rgba_premultiplied(0, 0, 0, 34),
        egui::Stroke::NONE,
    ));

    let top_pts = vec![
        egui::pos2(center.x, center.y - half_h),
        egui::pos2(center.x + half_w, center.y),
        egui::pos2(center.x, center.y + half_h),
        egui::pos2(center.x - half_w, center.y),
    ];
    let left_pts = vec![
        egui::pos2(center.x - half_w, center.y),
        egui::pos2(center.x, center.y + half_h),
        egui::pos2(center.x, center.y + half_h + depth),
        egui::pos2(center.x - half_w, center.y + depth),
    ];
    let right_pts = vec![
        egui::pos2(center.x + half_w, center.y),
        egui::pos2(center.x, center.y + half_h),
        egui::pos2(center.x, center.y + half_h + depth),
        egui::pos2(center.x + half_w, center.y + depth),
    ];

    draw_block_icon_face(painter, &left_pts, block, left, false);
    draw_block_icon_face(painter, &right_pts, block, right, false);
    draw_block_icon_face(painter, &top_pts, block, top, true);
}

fn draw_palette_material_card(ui: &mut egui::Ui, size: [f32; 2], block: BlockId) -> egui::Response {
    let desired = egui::vec2(size[0], size[1]);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    response.widget_info(||egui::WidgetInfo::labeled(egui::WidgetType::Button,ui.is_enabled(),block.display_name()));
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let painter = ui.painter();
    let rounding = egui::CornerRadius::same(0);
    painter.rect_filled(
        rect,
        rounding,
        if response.hovered() || response.has_focus() {
            Color32::from_rgb(78, 62, 46)
        } else { Color32::from_rgb(42, 35, 33) },
    );
    painter.rect_stroke(
        rect,
        rounding,
        egui::Stroke::new(1.0, if response.hovered() || response.has_focus() {
            presentation::AMBER
        } else { presentation::EDGE }),
        egui::StrokeKind::Outside,
    );

    let icon_rect = egui::Rect::from_min_max(
        rect.min + egui::vec2(10.0, 10.0),
        egui::pos2(rect.max.x - 10.0, rect.min.y + size[1] * 0.70),
    );
    draw_voxel_icon(painter, icon_rect, block);

    let name = painter.layout(block.display_name().to_owned(),
        egui::FontId::proportional(12.0), presentation::PAPER, rect.width()-8.0);
    painter.with_clip_rect(rect.shrink(3.0)).galley(
        egui::pos2(rect.center().x-name.size().x*0.5, rect.bottom()-25.0), name, Color32::WHITE);
    response.on_hover_text(block.display_name())
}

fn draw_hotbar_slot_button(
    ui: &mut egui::Ui,
    block: BlockId,
    survival_stack: Option<crate::world::inventory::ItemStack>,
    use_survival: bool,
    slot: usize,
    selected: bool,
    drag_targeted: bool,
) -> egui::Response {
    let viewport = ui.ctx().content_rect();
    let size = ((viewport.width()-32.0)/9.0-2.0).clamp(10.0,38.0)
        .min((viewport.height()*0.10).clamp(20.0,38.0));
    let remaining = 9 - slot;
    let size = size.min(((ui.available_width() - ui.spacing().item_spacing.x * (remaining-1) as f32)
        / remaining as f32).max(10.0));
    let desired = egui::vec2(size, size);
    let hover_label = if use_survival {
        survival_stack.map_or_else(
            || format!("Slot {}: Empty", slot + 1),
            |s| format!("{} × {}", s.item.name(), s.count),
        )
    } else {
        format!("Slot {}: {}", slot + 1, block.display_name())
    };
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    response.widget_info(||egui::WidgetInfo::labeled(egui::WidgetType::Button,ui.is_enabled(),&hover_label));
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let painter = ui.painter();
    let rounding = egui::CornerRadius::same(0);

    icons::frame(painter,rect,selected);
    if drag_targeted { painter.rect_stroke(rect,rounding,egui::Stroke::new(2.0,presentation::AMBER),egui::StrokeKind::Inside); }

    // Number indicator
    painter.text(
        rect.min + egui::vec2(4.0, 2.0),
        Align2::LEFT_TOP,
        hotbar_key_label(slot),
        egui::FontId::monospace(11.5),
        if selected {
            presentation::PAPER
        } else {
            presentation::MUTED
        },
    );

    if use_survival {
        if let Some(stack) = survival_stack {
            let icon_rect = egui::Rect::from_center_size(
                rect.center() + egui::vec2(0.0, 1.5),
                egui::vec2(size * 0.66, size * 0.66),
            );
            inventory::draw_item_icon(painter, icon_rect, stack.item);
            if stack.count > 1 {
                let count_str = format!("{}", stack.count);
                let font = egui::FontId::monospace((size * 0.30).clamp(9.5, 11.5));
                let pos = rect.right_bottom() - egui::vec2(2.5, 1.5);
                painter.text(
                    pos + egui::vec2(1.0, 1.0),
                    Align2::RIGHT_BOTTOM,
                    &count_str,
                    font.clone(),
                    presentation::INK,
                );
                painter.text(pos, Align2::RIGHT_BOTTOM, count_str, font, presentation::PAPER);
            }
        }
    } else if block != BlockId::Air {
        let icon_rect = egui::Rect::from_center_size(
            rect.center() + egui::vec2(0.0, 2.0),
            egui::vec2(size * 0.66, size * 0.66),
        );
        draw_voxel_icon(painter, icon_rect, block);
    }

    response.on_hover_text(hover_label)
}

fn draw_hotbar_row(
    ui: &mut egui::Ui,
    creative_state: &mut CreativeState,
    player: &mut Player,
    survival_inv: Option<&crate::world::inventory::SurvivalInventory>,
    drag_source: &mut Option<DragSource>,
    drag_target_slot: &mut Option<usize>,
) {
    *drag_target_slot = None;
    let use_survival = survival_inv.is_some()
        && creative_state.tool == ToolKind::Hand
        && !player.flying
        && !creative_state.show_panel;
    let hotbar = *creative_state.hotbar();
    for (slot, block) in hotbar.into_iter().enumerate() {
        let survival_stack = survival_inv.and_then(|inv| inv.hotbar[slot]);
        let response = draw_hotbar_slot_button(
            ui,
            block,
            survival_stack,
            use_survival,
            slot,
            slot == creative_state.selected_slot,
            *drag_target_slot == Some(slot),
        );
        if response.clicked() {
            creative_state.select_slot(slot);
            if creative_state.show_palette { creative_state.tool = ToolKind::Hand; }
            if use_survival {
                if let Some(crate::world::inventory::ItemStack {
                    item: crate::world::inventory::Item::Block(b),
                    ..
                }) = survival_stack
                {
                    player.selected_block = b;
                }
            } else {
                player.selected_block = creative_state.selected_block();
            }
        }
        if response.drag_started() {
            *drag_source = Some(DragSource::Hotbar(slot));
        }
        if crate::torch_review::mode() == "drag" {
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("review_hotbar", slot)), response.rect));
        }
        if drag_source.is_some() && response.contains_pointer() {
            *drag_target_slot = Some(slot);
            ui.painter().rect_stroke(response.rect, 0, egui::Stroke::new(2.0, presentation::AMBER), egui::StrokeKind::Inside);
        }
    }
}

fn quick_tool_ui(
    ctx: &egui::Context,
    creative_state: &mut CreativeState,
    tool_textures: &ToolTextures,
    hovered_tool: &mut Option<ToolKind>,
    hovered_brush_shape: &mut Option<BrushShape>,
    hovered_area_shape: &mut Option<AreaFootprintShape>,
) {
    *hovered_tool = None;
    egui::Window::new("Tool Toggle")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .show(ctx, |ui| {
            ui.set_min_width(760.0);
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading("Tool Toggle");
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Q closes this switcher. Pick a tool, then tune its behavior.",
                        )
                        .small()
                        .color(Color32::from_rgb(194, 201, 210)),
                    );
                });

                ui.separator();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(314.0);
                        for tool in ToolKind::ALL {
                            let texture = tool_textures.for_tool(tool);
                            let selected = creative_state.tool == tool;
                            let accent = tool_accent_color(tool);
                            egui::Frame::new()
                                .fill(if selected {
                                    accent.linear_multiply(0.18)
                                } else {
                                    Color32::from_rgba_premultiplied(18, 22, 28, 224)
                                })
                                .stroke(egui::Stroke::new(
                                    if selected { 1.4 } else { 1.0 },
                                    if selected {
                                        accent
                                    } else {
                                        Color32::from_rgba_premultiplied(255, 255, 255, 22)
                                    },
                                ))
                                .corner_radius(egui::CornerRadius::same(0))
                                .inner_margin(egui::Margin::same(10))
                                .show(ui, |ui| {
                                    let response = ui.add_sized(
                                        [ui.available_width(), 52.0],
                                        egui::Button::image_and_text(
                                            egui::Image::new((
                                                texture.texture.id(),
                                                egui::vec2(28.0, 28.0),
                                            ))
                                            .texture_options(egui::TextureOptions::NEAREST),
                                            format!(
                                                "{}  [{}]",
                                                tool.label(),
                                                tool_shortcut_label(tool)
                                            ),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(egui::Stroke::NONE),
                                    );
                                    if response.hovered() {
                                        *hovered_tool = Some(tool);
                                    }
                                    if response.clicked() {
                                        creative_state.tool = tool;
                                    }
                                    ui.label(
                                        RichText::new(tool_description(tool))
                                            .small()
                                            .color(Color32::from_rgb(190, 197, 205)),
                                    );
                                    ui.label(
                                        RichText::new(tool_detail_summary(creative_state, tool))
                                            .small()
                                            .color(accent),
                                    );
                                });
                            ui.add_space(4.0);
                        }
                    });

                    ui.add_space(8.0);

                    egui::Frame::new()
                        .fill(Color32::from_rgba_premultiplied(18, 22, 28, 228))
                        .stroke(egui::Stroke::new(
                            1.0,
                            tool_accent_color(creative_state.tool).linear_multiply(0.55),
                        ))
                        .corner_radius(egui::CornerRadius::same(0))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(390.0);
                            ui.horizontal(|ui| {
                                let texture = tool_textures.for_tool(creative_state.tool);
                                ui.add(
                                    egui::Image::new((
                                        texture.texture.id(),
                                        egui::vec2(32.0, 32.0),
                                    ))
                                    .texture_options(egui::TextureOptions::NEAREST),
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(creative_state.tool.label())
                                            .size(24.0)
                                            .strong()
                                            .color(tool_accent_color(creative_state.tool)),
                                    );
                                    ui.label(
                                        RichText::new(tool_description(creative_state.tool))
                                            .small()
                                            .color(Color32::from_rgb(196, 202, 210)),
                                    );
                                });
                            });
                            ui.add_space(8.0);
                            if !matches!(creative_state.tool, ToolKind::Hand) {
                                status_chip(
                                    ui,
                                    format!(
                                        "Active material {}",
                                        creative_state.selected_block().display_name()
                                    ),
                                    material_color32(creative_state.selected_block()),
                                );
                                ui.add_space(8.0);
                            }
                            tool_settings_ui(
                                ui,
                                creative_state,
                                hovered_brush_shape,
                                hovered_area_shape,
                            );
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                status_chip(
                                    ui,
                                    "Alt + Scroll size",
                                    Color32::from_rgb(102, 150, 240),
                                );
                                status_chip(
                                    ui,
                                    "Ctrl + Scroll reach",
                                    Color32::from_rgb(120, 186, 134),
                                );
                                status_chip(ui, "1-9 hotbar slot", Color32::from_rgb(225, 176, 92));
                            });
                        });
                });
            });
        });
}

fn quick_tool_ui_plain(
    ctx: &egui::Context,
    creative_state: &mut CreativeState,
    tool_textures: &ToolTextures,
    hovered_tool: &mut Option<ToolKind>,
    hovered_brush_shape: &mut Option<BrushShape>,
    hovered_area_shape: &mut Option<AreaFootprintShape>,
) {
    *hovered_tool = None;
    *hovered_brush_shape = None;
    *hovered_area_shape = None;
    egui::Window::new("Choose tool")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false).resizable(false)
        .show(ctx, |ui| {
            ui.set_width(320.0);
            ui.horizontal_wrapped(|ui| {
                for tool in ToolKind::ALL {
                    let response = ui.add(egui::Button::new(format!("{}  {}",tool_shortcut_label(tool),tool.label()))
                        .selected(creative_state.tool == tool).min_size(egui::vec2(150.0,32.0)));
                    if response.hovered() { *hovered_tool = Some(tool); }
                    if response.clicked() { creative_state.tool=tool; creative_state.show_quick_select=false; }
                }
            });
            ui.small("B adjusts reach and size. Q closes.");
        });
    let _ = tool_textures;
}

pub struct EguiUi {
    pub library_entries: std::sync::Arc<Vec<crate::world::storage::WorldEntry>>,
    pub library_busy: bool,
    pub library_loading: bool,
    pub library_status: Option<String>,
    pub library_revision: u64,
    pub preferences_status: Option<String>,
    pub new_world_name: String,
    home_page: front_end::HomePage,
    browser: world_browser::BrowserState,
    pub expedition: Option<crate::world::expedition::Expedition>,
    pub factory: crate::world::factory::View,
    pub adventure: crate::world::adventure::View,
    pub survival: crate::world::inventory::SurvivalInventory,
    pub survival_eat_timer:f32,
    pub survival_armor_points:u32,
    pub biome_label: &'static str,
    pub elevation_m: f32,
    pub show_hud: bool,
    pub target_block: Option<BlockId>,
    pub target_cell_temp_c: Option<f32>,
    pub tool_feedback: tool_hud::Feedback,
    pub fov_degrees: f32,
    pub mouse_sensitivity: f32,
    pub invert_mouse: bool,
    pub persistence_status: Option<String>,
    pub region_status: Option<String>,
    pub region_error: Option<String>,
    pub ctx: egui::Context,
    state: egui_winit::State,
    renderer: Renderer,
    shapes: Vec<egui::ClippedPrimitive>,
    textures_delta: egui::TexturesDelta,
    tool_textures: ToolTextures,
    held_tool_pose: HeldToolPoseState,
    drag_source: Option<DragSource>,
    drag_target_slot: Option<usize>,
    hovered_palette_block: Option<BlockId>,
    hovered_quick_tool: Option<ToolKind>,
    hovered_quick_brush_shape: Option<BrushShape>,
    hovered_quick_area_shape: Option<AreaFootprintShape>,
    show_debug_performance_panel: bool,
    pause_page: front_end::PausePage,
}

impl EguiUi {
    pub fn new(
        window: &Window,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let ctx = egui::Context::default();
        presentation::apply_theme(&ctx);

        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );

        let renderer = Renderer::new(device, surface_format.remove_srgb_suffix(), RendererOptions::default());
        let tool_textures = ToolTextures::load(&ctx, "assets");

        Self {
            library_entries: std::sync::Arc::default(),
            library_busy: false,
            library_loading: false,
            library_status: None,
            library_revision: 0,
            preferences_status: None,
            new_world_name: "New world".into(),
            home_page: front_end::HomePage::default(),
            browser: world_browser::BrowserState::default(),
            show_hud: true,
            expedition: None,
            factory: Default::default(),
            adventure: Default::default(),
            survival: Default::default(),
            survival_eat_timer:0.0,survival_armor_points:0,
            biome_label: "Wilderness",
            elevation_m: 0.0,
            target_block: None,
            target_cell_temp_c: None,
            tool_feedback: Default::default(),
            fov_degrees: 70.0,
            mouse_sensitivity: 0.0025,
            invert_mouse: false,
            persistence_status: None,
            region_status: None,
            region_error: None,
            ctx,
            state,
            renderer,
            shapes: Vec::new(),
            textures_delta: egui::TexturesDelta::default(),
            tool_textures,
            held_tool_pose: HeldToolPoseState::default(),
            drag_source: None,
            drag_target_slot: None,
            hovered_palette_block: None,
            hovered_quick_tool: None,
            hovered_quick_brush_shape: None,
            hovered_quick_area_shape: None,
            show_debug_performance_panel: false,
            pause_page: front_end::PausePage::default(),
        }
    }

    pub fn on_window_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    pub fn wants_input(&self) -> bool {
        self.ctx.wants_pointer_input() || self.ctx.wants_keyboard_input()
    }

    pub fn toggle_debug(&mut self) {
        self.show_debug_performance_panel = !self.show_debug_performance_panel;
    }

    pub fn begin_frame(&mut self, window: &Window) {
        // Whole physical-pixel UI scaling avoids different blur at 125/150% DPI.
        let native=window.scale_factor() as f32;
        self.ctx.set_zoom_factor(native.round().max(1.0)/native);
        let mut raw_input = self.state.take_egui_input(window);
        if crate::torch_review::mode() == "drag" {
            crate::torch_review::drag_input(&self.ctx, &mut raw_input);
        }
        crate::material_studio::review_input(&self.ctx,&mut raw_input);
        self.ctx.begin_pass(raw_input);
    }

    pub fn build_loading_ui(&mut self, loading: &LoadingUiState) {
        front_end::loading(&self.ctx,loading);
    }

    fn build_performance_ui(ui: &mut egui::Ui, perf: &PerfSnapshot) {
        let history = perf.history.as_slice();
        let has_gpu_timestamps = perf.render_avg.gpu_timestamps_supported;
        let has_weather_history = history
            .iter()
            .any(|sample| sample.weather.dirty_rects > 0 || sample.weather.cpu_upload_ms > 0.0);

        let frame_ms = perf_history_series(history, |sample| sample.cpu.total_ms);
        let render_call_ms = perf_history_series(history, |sample| sample.cpu.render_ms);
        let gpu_pass_ms = perf_history_series(history, |sample| sample.render.gpu_pass_total_ms);
        let gpu_cloud_ms =
            perf_history_series(history, |sample| sample.render.gpu_cloud_raymarch_ms);
        let gpu_terrain_ms = perf_history_series(history, |sample| sample.render.gpu_terrain_ms);
        let upload_ms = perf_history_series(history, |sample| sample.render.stream_upload_ms);
        let weather_upload_ms = perf_history_series(history, |sample| sample.weather.cpu_upload_ms);
        let visible_chunks =
            perf_history_count_series(history, |sample| sample.counters.visible_chunks);
        let draw_calls =
            perf_history_count_series(history, |sample| sample.render.terrain_draw_calls);
        let terrain_tris_k = perf_history_series(history, |sample| {
            sample.counters.terrain_triangles as f32 / 1000.0
        });

        ui.horizontal_wrapped(|ui| {
            if !perf.adapter_summary.is_empty() {
                status_chip(
                    ui,
                    format!("GPU {}", perf.adapter_summary),
                    Color32::from_rgb(96, 164, 255),
                );
            }
            status_chip(
                ui,
                format!("{}x{}", perf.resolution[0], perf.resolution[1]),
                Color32::from_rgb(128, 204, 255),
            );
            status_chip(
                ui,
                format!("{} samples", perf.sample_count),
                Color32::from_rgb(133, 212, 148),
            );
            status_chip(
                ui,
                if has_gpu_timestamps {
                    "GPU timestamps ON"
                } else {
                    "GPU timestamps OFF"
                },
                if has_gpu_timestamps {
                    Color32::from_rgb(255, 206, 120)
                } else {
                    Color32::from_rgb(184, 127, 127)
                },
            );
        });
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            perf_metric_card(
                ui,
                "Average FPS",
                format!("{:.1}", perf.fps_avg),
                format!("{:.2} ms / frame", perf.frame_ms_avg),
                Color32::from_rgb(114, 205, 136),
            );
            perf_metric_card(
                ui,
                "1% Low",
                format!("{:.1} FPS", perf.fps_1p_low),
                format!("p99 {:.2} ms", perf.frame_ms_p99),
                Color32::from_rgb(255, 202, 108),
            );
            perf_metric_card(
                ui,
                "Frame Worst",
                format!("{:.2} ms", perf.frame_ms_max),
                format!("latest {:.2} ms", perf.cpu_latest.total_ms),
                Color32::from_rgb(255, 146, 120),
            );
            perf_metric_card(
                ui,
                if has_gpu_timestamps {
                    "GPU Passes"
                } else {
                    "Render Call"
                },
                if has_gpu_timestamps {
                    format!("{:.2} ms", perf.render_latest.gpu_pass_total_ms)
                } else {
                    format!("{:.2} ms", perf.cpu_latest.render_ms)
                },
                if has_gpu_timestamps {
                    format!("avg {:.2} ms", perf.render_avg.gpu_pass_total_ms)
                } else {
                    format!("avg {:.2} ms", perf.cpu_avg.render_ms)
                },
                Color32::from_rgb(104, 168, 255),
            );
            perf_metric_card(
                ui,
                "Cloud Raymarch",
                format!("{:.2} ms", perf.render_latest.gpu_cloud_raymarch_ms),
                format!("avg {:.2} ms", perf.render_avg.gpu_cloud_raymarch_ms),
                Color32::from_rgb(186, 140, 255),
            );
        });

        ui.add_space(10.0);
        draw_perf_graph(
            ui,
            "Frame Timeline",
            "Rolling history for total frame time, CPU render call time, and measured GPU pass time.",
            &[
                (
                    "Frame",
                    frame_ms.as_slice(),
                    Color32::from_rgb(114, 205, 136),
                ),
                (
                    "Render Call",
                    render_call_ms.as_slice(),
                    Color32::from_rgb(255, 202, 108),
                ),
                (
                    "GPU Passes",
                    gpu_pass_ms.as_slice(),
                    Color32::from_rgb(104, 168, 255),
                ),
            ],
            "ms",
            Some(16.67),
        );
        ui.add_space(8.0);

        if has_gpu_timestamps {
            draw_perf_graph(
                ui,
                "GPU Hot Passes",
                "Cloud raymarch is separated from terrain so you can see when the frame is cloud-bound.",
                &[
                    (
                        "GPU Total",
                        gpu_pass_ms.as_slice(),
                        Color32::from_rgb(96, 164, 255),
                    ),
                    (
                        "Cloud",
                        gpu_cloud_ms.as_slice(),
                        Color32::from_rgb(186, 140, 255),
                    ),
                    (
                        "Terrain",
                        gpu_terrain_ms.as_slice(),
                        Color32::from_rgb(120, 216, 160),
                    ),
                ],
                "ms",
                None,
            );
            ui.add_space(8.0);
        }

        draw_perf_graph(
            ui,
            "Scene Activity",
            "Visible chunk count, terrain draw calls, and terrain triangles in thousands.",
            &[
                (
                    "Chunks",
                    visible_chunks.as_slice(),
                    Color32::from_rgb(114, 205, 136),
                ),
                (
                    "Draws",
                    draw_calls.as_slice(),
                    Color32::from_rgb(255, 202, 108),
                ),
                (
                    "Triangles (k)",
                    terrain_tris_k.as_slice(),
                    Color32::from_rgb(104, 168, 255),
                ),
            ],
            "",
            None,
        );

        egui::CollapsingHeader::new("Frame Timing Breakdown")
            .default_open(true)
            .show(ui, |ui| {
                ui.columns(2, |columns| {
                    columns[0].label(RichText::new("CPU Stages").strong());
                    egui::Grid::new("perf_cpu_grid")
                        .num_columns(3)
                        .spacing(egui::vec2(12.0, 6.0))
                        .striped(true)
                        .show(&mut columns[0], |ui| {
                            perf_table_header(ui);
                            perf_table_row(
                                ui,
                                "UI",
                                format!("{:.2} ms", perf.cpu_avg.ui_ms),
                                format!("{:.2} ms", perf.cpu_latest.ui_ms),
                            );
                            perf_table_row(
                                ui,
                                "Gameplay",
                                format!("{:.2} ms", perf.cpu_avg.gameplay_ms),
                                format!("{:.2} ms", perf.cpu_latest.gameplay_ms),
                            );
                            perf_table_row(
                                ui,
                                "World Snapshot",
                                format!("{:.2} ms", perf.cpu_avg.snapshot_ms),
                                format!("{:.2} ms", perf.cpu_latest.snapshot_ms),
                            );
                            perf_table_row(
                                ui,
                                "Streaming",
                                format!("{:.2} ms", perf.cpu_avg.streaming_ms),
                                format!("{:.2} ms", perf.cpu_latest.streaming_ms),
                            );
                            perf_table_row(
                                ui,
                                "Culling",
                                format!("{:.2} ms", perf.cpu_avg.culling_ms),
                                format!("{:.2} ms", perf.cpu_latest.culling_ms),
                            );
                            perf_table_row(
                                ui,
                                "Torches",
                                format!("{:.2} ms", perf.cpu_avg.torch_ms),
                                format!("{:.2} ms", perf.cpu_latest.torch_ms),
                            );
                            perf_table_row(
                                ui,
                                "Weather",
                                format!("{:.2} ms", perf.cpu_avg.weather_ms),
                                format!("{:.2} ms", perf.cpu_latest.weather_ms),
                            );
                            perf_table_row(
                                ui,
                                "Uniform Upload",
                                format!("{:.2} ms", perf.cpu_avg.uniform_ms),
                                format!("{:.2} ms", perf.cpu_latest.uniform_ms),
                            );
                            perf_table_row(
                                ui,
                                "Render Call",
                                format!("{:.2} ms", perf.cpu_avg.render_ms),
                                format!("{:.2} ms", perf.cpu_latest.render_ms),
                            );
                            perf_table_row(
                                ui,
                                "Frame Total",
                                format!("{:.2} ms", perf.cpu_avg.total_ms),
                                format!("{:.2} ms", perf.cpu_latest.total_ms),
                            );
                        });

                    columns[1].label(RichText::new("Render CPU Work").strong());
                    egui::Grid::new("perf_render_cpu_grid")
                        .num_columns(3)
                        .spacing(egui::vec2(12.0, 6.0))
                        .striped(true)
                        .show(&mut columns[1], |ui| {
                            perf_table_header(ui);
                            perf_table_row(
                                ui,
                                "Surface Acquire",
                                format!("{:.2} ms", perf.render_avg.surface_acquire_ms),
                                format!("{:.2} ms", perf.render_latest.surface_acquire_ms),
                            );
                            perf_table_row(
                                ui,
                                "Stream Uploads",
                                format!("{:.2} ms", perf.render_avg.stream_upload_ms),
                                format!("{:.2} ms", perf.render_latest.stream_upload_ms),
                            );
                            perf_table_row(
                                ui,
                                "Shadow Encode",
                                format!("{:.2} ms", perf.render_avg.shadow_encode_ms),
                                format!("{:.2} ms", perf.render_latest.shadow_encode_ms),
                            );
                            perf_table_row(
                                ui,
                                "Terrain Encode",
                                format!("{:.2} ms", perf.render_avg.terrain_encode_ms),
                                format!("{:.2} ms", perf.render_latest.terrain_encode_ms),
                            );
                            perf_table_row(
                                ui,
                                "Cloud Encode",
                                format!("{:.2} ms", perf.render_avg.cloud_encode_ms),
                                format!("{:.2} ms", perf.render_latest.cloud_encode_ms),
                            );
                            perf_table_row(
                                ui,
                                "UI Encode",
                                format!("{:.2} ms", perf.render_avg.ui_encode_ms),
                                format!("{:.2} ms", perf.render_latest.ui_encode_ms),
                            );
                            perf_table_row(
                                ui,
                                "Overlay Encode",
                                format!("{:.2} ms", perf.render_avg.overlay_encode_ms),
                                format!("{:.2} ms", perf.render_latest.overlay_encode_ms),
                            );
                            perf_table_row(
                                ui,
                                "Submit",
                                format!("{:.2} ms", perf.render_avg.submit_ms),
                                format!("{:.2} ms", perf.render_latest.submit_ms),
                            );
                            perf_table_row(
                                ui,
                                "Present",
                                format!("{:.2} ms", perf.render_avg.present_ms),
                                format!("{:.2} ms", perf.render_latest.present_ms),
                            );
                        });
                });
            });

        egui::CollapsingHeader::new("GPU Pass Breakdown")
            .default_open(has_gpu_timestamps)
            .show(ui, |ui| {
                if has_gpu_timestamps {
                    egui::Grid::new("perf_gpu_grid")
                        .num_columns(3)
                        .spacing(egui::vec2(12.0, 6.0))
                        .striped(true)
                        .show(ui, |ui| {
                            perf_table_header(ui);
                            perf_table_row(ui, "Total GPU Passes", format!("{:.2} ms", perf.render_avg.gpu_pass_total_ms), format!("{:.2} ms", perf.render_latest.gpu_pass_total_ms));
                            perf_table_row(ui, "Shadow", format!("{:.2} ms", perf.render_avg.gpu_shadow_ms), format!("{:.2} ms", perf.render_latest.gpu_shadow_ms));
                            perf_table_row(ui, "Terrain", format!("{:.2} ms", perf.render_avg.gpu_terrain_ms), format!("{:.2} ms", perf.render_latest.gpu_terrain_ms));
                            perf_table_row(ui, "Cloud Raymarch", format!("{:.2} ms", perf.render_avg.gpu_cloud_raymarch_ms), format!("{:.2} ms", perf.render_latest.gpu_cloud_raymarch_ms));
                            perf_table_row(ui, "Cloud Temporal", format!("{:.2} ms", perf.render_avg.gpu_cloud_temporal_ms), format!("{:.2} ms", perf.render_latest.gpu_cloud_temporal_ms));
                            perf_table_row(ui, "Sky", format!("{:.2} ms", perf.render_avg.gpu_sky_ms), format!("{:.2} ms", perf.render_latest.gpu_sky_ms));
                            perf_table_row(ui, "Cloud Composite", format!("{:.2} ms", perf.render_avg.gpu_cloud_composite_ms), format!("{:.2} ms", perf.render_latest.gpu_cloud_composite_ms));
                            perf_table_row(ui, "Outline", format!("{:.2} ms", perf.render_avg.gpu_outline_ms), format!("{:.2} ms", perf.render_latest.gpu_outline_ms));
                            perf_table_row(ui, "UI", format!("{:.2} ms", perf.render_avg.gpu_ui_ms), format!("{:.2} ms", perf.render_latest.gpu_ui_ms));
                        });
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        status_chip(
                            ui,
                            format!(
                                "shadow redraw {}",
                                if perf.render_latest.shadow_redraw {
                                    "yes"
                                } else {
                                    "cached"
                                }
                            ),
                            Color32::from_rgb(255, 202, 108),
                        );
                        status_chip(
                            ui,
                            format!("raymarch passes {}", perf.render_latest.cloud_raymarch_passes),
                            Color32::from_rgb(186, 140, 255),
                        );
                        status_chip(
                            ui,
                            format!("temporal passes {}", perf.render_latest.cloud_temporal_passes),
                            Color32::from_rgb(96, 164, 255),
                        );
                    });
                } else {
                    ui.label(
                        RichText::new(
                            "GPU timestamp queries are unavailable on the current backend, so this section only has CPU-side render timings.",
                        )
                        .small()
                        .color(Color32::from_rgb(170, 176, 188)),
                    );
                }
            });

        egui::CollapsingHeader::new("Scene and Streaming")
            .default_open(true)
            .show(ui, |ui| {
                ui.columns(2, |columns| {
                    columns[0].label(RichText::new("Scene Metrics").strong());
                    egui::Grid::new("perf_scene_grid")
                        .num_columns(2)
                        .spacing(egui::vec2(12.0, 6.0))
                        .striped(true)
                        .show(&mut columns[0], |ui| {
                            perf_value_row(
                                ui,
                                "Resolution",
                                format!("{} x {}", perf.resolution[0], perf.resolution[1]),
                            );
                            perf_value_row(
                                ui,
                                "View Distance",
                                format!("{} chunks", perf.render_settings.view_distance_chunks),
                            );
                            perf_value_row(
                                ui,
                                "LOD Start",
                                format!("{} chunks", perf.render_settings.lod_start_chunks),
                            );
                            perf_value_row(
                                ui,
                                "LOD Levels",
                                perf.render_settings.lod_levels.to_string(),
                            );
                            perf_value_row(
                                ui,
                                "Visible Chunks",
                                format_compact_count(perf.counters_latest.visible_chunks),
                            );
                            perf_value_row(
                                ui,
                                "GPU Meshes",
                                format_compact_count(perf.counters_latest.visible_meshes),
                            );
                            perf_value_row(
                                ui,
                                "Draw Calls",
                                format_compact_count(perf.counters_latest.draw_chunks),
                            );
                            perf_value_row(
                                ui,
                                "Terrain Triangles",
                                format_compact_count(perf.counters_latest.terrain_triangles),
                            );
                            perf_value_row(
                                ui,
                                "Nearby Torches",
                                format_compact_count(perf.counters_latest.nearby_torches),
                            );
                            perf_value_row(
                                ui,
                                "Outline Vertices",
                                format_compact_count(perf.render_latest.outline_vertices),
                            );
                            perf_value_row(
                                ui,
                                "UI Vertices",
                                format_compact_count(perf.render_latest.ui_vertices),
                            );
                        });

                    columns[1].label(RichText::new("Chunk Pipeline").strong());
                    egui::Grid::new("perf_chunk_grid")
                        .num_columns(2)
                        .spacing(egui::vec2(12.0, 6.0))
                        .striped(true)
                        .show(&mut columns[1], |ui| {
                            perf_value_row(
                                ui,
                                "Terrain Generated",
                                format_compact_count(perf.chunk_latest.terrain_generated),
                            );
                            perf_value_row(
                                ui,
                                "Terrain Scheduled",
                                format_compact_count(perf.chunk_latest.terrain_scheduled),
                            );
                            perf_value_row(
                                ui,
                                "Mesh Dispatched",
                                format_compact_count(perf.chunk_latest.mesh_dispatched),
                            );
                            perf_value_row(
                                ui,
                                "Meshes Uploaded",
                                format_compact_count(perf.chunk_latest.meshes_uploaded),
                            );
                            perf_value_row(
                                ui,
                                "Uploaded Vertices",
                                format_compact_count(perf.chunk_latest.uploaded_vertices),
                            );
                            perf_value_row(
                                ui,
                                "Uploaded Indices",
                                format_compact_count(perf.chunk_latest.uploaded_indices),
                            );
                            perf_value_row(
                                ui,
                                "Requested",
                                format_compact_count(perf.chunk_latest.requested),
                            );
                            perf_value_row(
                                ui,
                                "GPU Generating",
                                format_compact_count(perf.chunk_latest.generating),
                            );
                            perf_value_row(
                                ui,
                                "CPU Meshing",
                                format_compact_count(perf.chunk_latest.meshing),
                            );
                            perf_value_row(
                                ui,
                                "Ready Chunks",
                                format_compact_count(perf.chunk_latest.ready_chunks),
                            );
                            perf_value_row(
                                ui,
                                "Ready Meshes",
                                format_compact_count(perf.chunk_latest.ready_meshes),
                            );
                            perf_value_row(
                                ui,
                                "Pending Schedule",
                                format_compact_count(perf.chunk_latest.pending_schedule),
                            );
                            perf_value_row(
                                ui,
                                "LOD Reuses",
                                format_compact_count(perf.chunk_latest.lod_cache_hits),
                            );
                            perf_value_row(
                                ui,
                                "CPU Voxel Cache",
                                format!(
                                    "{} / {}",
                                    format_compact_bytes(perf.chunk_latest.cpu_resident_bytes),
                                    format_compact_bytes(perf.chunk_latest.cpu_budget_bytes)
                                ),
                            );
                            perf_value_row(
                                ui,
                                "GPU Terrain Residency",
                                format!(
                                    "{} / {}",
                                    format_compact_bytes(perf.chunk_latest.gpu_resident_bytes),
                                    format_compact_bytes(perf.chunk_latest.gpu_budget_bytes)
                                ),
                            );
                            perf_value_row(
                                ui,
                                "Budget Deferred",
                                format_compact_count(perf.chunk_latest.budget_deferred),
                            );
                            perf_value_row(
                                ui,
                                "CPU / GPU Evictions",
                                format!(
                                    "{} / {}",
                                    perf.chunk_latest.cpu_evictions,
                                    perf.chunk_latest.gpu_evictions
                                ),
                            );
                            perf_value_row(
                                ui,
                                "Inactive LOD Cache",
                                format_compact_bytes(perf.chunk_latest.lod_cache_bytes),
                            );
                            perf_value_row(
                                ui,
                                "Generation Buffers",
                                format_compact_bytes(perf.chunk_latest.terrain_buffer_bytes),
                            );
                            perf_value_row(
                                ui,
                                "Mesh Workers + Results",
                                format!(
                                    "{} / {}",
                                    format_compact_bytes(perf.chunk_latest.queued_mesh_bytes),
                                    format_compact_bytes(
                                        crate::render::mesh_budget_mib() * 1024 * 1024
                                    )
                                ),
                            );
                        });
                });
            });

        egui::CollapsingHeader::new("Weather Uploads")
            .default_open(has_weather_history || perf.weather_latest.dirty_rects > 0)
            .show(ui, |ui| {
                draw_perf_graph(
                    ui,
                    "Weather and Upload Cost",
                    "CPU-side weather clipmap upload time and streaming upload encode time.",
                    &[
                        (
                            "Weather Upload",
                            weather_upload_ms.as_slice(),
                            Color32::from_rgb(96, 164, 255),
                        ),
                        (
                            "Stream Upload",
                            upload_ms.as_slice(),
                            Color32::from_rgb(255, 202, 108),
                        ),
                    ],
                    "ms",
                    None,
                );
                ui.add_space(8.0);
                egui::Grid::new("perf_weather_grid")
                    .num_columns(2)
                    .spacing(egui::vec2(12.0, 6.0))
                    .striped(true)
                    .show(ui, |ui| {
                        perf_value_row(
                            ui,
                            "Clipmap Batches",
                            format_compact_count(perf.weather_latest.clipmap_batches),
                        );
                        perf_value_row(
                            ui,
                            "Dirty Rects",
                            format_compact_count(perf.weather_latest.dirty_rects),
                        );
                        perf_value_row(
                            ui,
                            "Uploaded Bytes",
                            format_compact_bytes(perf.weather_latest.uploaded_bytes),
                        );
                        perf_value_row(
                            ui,
                            "CPU Upload",
                            format!("{:.2} ms", perf.weather_latest.cpu_upload_ms),
                        );
                    });
            });
    }

    fn build_debug_performance_panel(&mut self, mode: UiMode, perf: &PerfSnapshot) {
        let launcher_y = if matches!(mode, UiMode::Paused) {
            50.0
        } else {
            14.0
        };

        egui::Area::new("debug_perf_launcher".into())
            .anchor(Align2::RIGHT_TOP, [-14.0, launcher_y])
            .show(&self.ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_premultiplied(18, 22, 30, 232))
                    .stroke(egui::Stroke::new(
                        1.0,
                        if self.show_debug_performance_panel {
                            Color32::from_rgb(96, 164, 255)
                        } else {
                            Color32::from_rgba_premultiplied(255, 255, 255, 28)
                        },
                    ))
                    .corner_radius(egui::CornerRadius::same(0))
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        let button = ui.selectable_label(
                            self.show_debug_performance_panel,
                            RichText::new("Debug / Performance").strong(),
                        );
                        if button.clicked() {
                            self.show_debug_performance_panel = !self.show_debug_performance_panel;
                        }
                    });
            });

        if self.show_debug_performance_panel {
            let content_rect = self.ctx.content_rect();
            let default_pos = egui::pos2(
                (content_rect.right() - 472.0).max(content_rect.left() + 12.0),
                content_rect.top() + launcher_y + 36.0,
            );
            let default_height = (content_rect.height() * 0.72).clamp(360.0, 760.0);

            egui::Window::new("Debug / Performance")
                .id(egui::Id::new("debug_performance_panel"))
                .default_pos(default_pos)
                .default_size([460.0, default_height])
                .min_size([360.0, 260.0])
                .resizable(true)
                .movable(true)
                .collapsible(true)
                .vscroll(true)
                .open(&mut self.show_debug_performance_panel)
                .show(&self.ctx, |ui| {
                    Self::build_performance_ui(ui, perf);
                });
        }
    }

    pub fn build_ui(
        &mut self,
        mode: UiMode,
        selected_world: &mut WorldType,
        selected_voxel_scale: &mut VoxelScalePreset,
        selected_seed: &mut u32,
        fps: u32,
        frame_dt: f32,
        player: &mut Player,
        time_state: &mut TimeUiState,
        physics_state: &mut PhysicsUiState,
        render_state: &mut RenderUiState,
        perf: &PerfSnapshot,
        creative_state: &mut CreativeState,
        using_tool: Option<ToolUseAction>,
    ) -> UiAction {
        let mut action = UiAction::None;
        if !creative_state.any_ui_open() || self.ctx.input(|i| !i.focused || i.key_pressed(egui::Key::Escape)) {
            self.drag_source = None;
            self.drag_target_slot = None;
        }
        if matches!(mode,UiMode::Playing) {
            if let Some(error)=&self.region_error {front_end::storage_notice(&self.ctx,error);}
        }
        let time_label = format_clock(time_state.time_of_day);
        let move_speed_mps = player.vel.xz().length() * selected_voxel_scale.meters();
        player.selected_block = creative_state.selected_block();
        self.held_tool_pose
            .update(creative_state.tool, using_tool, move_speed_mps, frame_dt);

        if matches!(mode, UiMode::Loading) {
            return UiAction::None;
        }

        if matches!(mode,UiMode::WorldSelect) {
            return front_end::home(self,selected_world,selected_voxel_scale,selected_seed,render_state);
        }
        if matches!(mode,UiMode::Paused) {
            let action=front_end::pause(self,time_state,physics_state,render_state);
            if self.show_debug_performance_panel {self.build_debug_performance_panel(mode,perf);}
            return action;
        }
        if matches!(mode, UiMode::Playing) {
            if self.show_hud {
                let mut status_rect = None;
                if self.show_debug_performance_panel {
                let status = egui::Area::new("play_status".into())
                    .anchor(Align2::LEFT_TOP, [16.0, 16.0])
                    .interactable(false)
                    .show(&self.ctx, |ui| {
                        presentation::hud_frame().show(ui, |ui| {
                            if self.show_debug_performance_panel {
                                ui.label(RichText::new(self.biome_label).size(14.0).color(Color32::from_rgb(230,235,216)));
                                ui.small(format!("{time_label}  ·  {:.0} m",self.elevation_m));
                            }
                            ui.small(creative_state.cursor_summary());
                            if creative_state.pending_cells()>0 {ui.small(format!("{} cells queued",creative_state.pending_cells()));}
                        });
                    });
                status_rect = Some(status.response.rect);
                }
                front_end::journey_hud(&self.ctx,self.expedition.as_ref(),status_rect);
                if !creative_state.any_ui_open() && !self.show_debug_performance_panel {
                    tool_hud::draw(&self.ctx, creative_state, &self.tool_feedback);
                }
                if !creative_state.any_ui_open() {
                    if let Some(enemy) = self.adventure.enemy_focus.filter(|_| self.adventure.boss.is_none()) {
                        egui::Area::new("target_name".into())
                            .anchor(Align2::CENTER_TOP, [0.0, 10.0])
                            .interactable(false)
                            .show(&self.ctx, |ui| {
                                presentation::hud_frame().show(ui, |ui| {
                                    ui.set_min_width(172.0);
                                    ui.set_max_width(260.0);
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 2.0;
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(enemy.name)
                                                    .size(12.5)
                                                    .strong()
                                                    .color(presentation::PAPER),
                                            );
                                            if enemy.frozen {
                                                ui.label(
                                                    RichText::new("FROZEN")
                                                        .size(9.5)
                                                        .strong()
                                                        .color(Color32::from_rgb(120, 214, 248)),
                                                );
                                            }
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.monospace(
                                                        RichText::new(format!(
                                                            "{:.0}/{:.0}",
                                                            enemy.health.max(0.0),
                                                            enemy.maximum
                                                        ))
                                                        .size(10.5)
                                                        .color(presentation::AMBER),
                                                    );
                                                },
                                            );
                                        });
                                        let ratio = (enemy.health / enemy.maximum.max(1.0)).clamp(0.0, 1.0);
                                        let (bar_r, _) = ui.allocate_exact_size(
                                            egui::vec2(ui.available_width().max(150.0), 6.0),
                                            egui::Sense::hover(),
                                        );
                                        let p = ui.painter();
                                        p.rect_filled(bar_r, 0, Color32::from_rgb(28, 16, 18));
                                        let mut fill = bar_r;
                                        fill.set_width(bar_r.width() * ratio);
                                        p.rect_filled(
                                            fill,
                                            0,
                                            if enemy.frozen {
                                                Color32::from_rgb(92, 188, 224)
                                            } else {
                                                presentation::DANGER
                                            },
                                        );
                                        p.rect_stroke(
                                            bar_r,
                                            0,
                                            egui::Stroke::new(1.0, presentation::EDGE),
                                            egui::StrokeKind::Inside,
                                        );
                                    });
                                });
                            });
                    } else if let Some(block) = self.target_block.filter(|block| *block != BlockId::Air) {
                        egui::Area::new("target_name".into())
                            .anchor(Align2::CENTER_TOP, [0.0, 10.0])
                            .interactable(false)
                            .show(&self.ctx, |ui| {
                                presentation::hud_frame().show(ui, |ui| {
                                    ui.set_max_width(276.0);
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui.allocate_exact_size(
                                            egui::vec2(22.0, 22.0),
                                            egui::Sense::hover(),
                                        );
                                        draw_voxel_icon(ui.painter(), r, block);
                                        ui.vertical(|ui| {
                                            ui.spacing_mut().item_spacing.y = 1.5;
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(block.display_name())
                                                        .size(12.5)
                                                        .strong()
                                                        .color(presentation::PAPER),
                                                );
                                                if let Some(temp_c) = self.target_cell_temp_c {
                                                    ui.monospace(
                                                        RichText::new(format!("{temp_c:.0}°C"))
                                                            .size(10.0)
                                                            .color(Color32::from_rgb(245, 134, 62)),
                                                    );
                                                }
                                            });
                                            let sub = if let Some(expedition) = &self.expedition {
                                                let h = crate::world::expedition::hint(block, expedition.tool_tier);
                                                if !h.is_empty() {
                                                    h
                                                } else {
                                                    crate::world::inventory::Item::Block(block).category()
                                                }
                                            } else {
                                                crate::world::inventory::Item::Block(block).category()
                                            };
                                            if let Some((_, node)) = self.factory.focused.as_ref().filter(|(_, n)| n.kind.block() == block) {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} · E {} · H₂O {:.1}m³",
                                                        node.status,
                                                        node.energy,
                                                        node.water as f32 / 512.0
                                                    ))
                                                    .size(10.0)
                                                    .color(presentation::JADE),
                                                );
                                            } else {
                                                ui.label(
                                                    RichText::new(sub)
                                                        .size(10.5)
                                                        .color(presentation::MUTED),
                                                );
                                            }
                                        });
                                    });
                                });
                            });
                    }
                }
                if self.show_debug_performance_panel {
                    egui::Area::new("debug_readout".into())
                        .anchor(Align2::RIGHT_TOP, [-16.0, 16.0])
                        .interactable(false)
                        .show(&self.ctx, |ui| {
                            presentation::hud_frame().show(ui, |ui| {
                                ui.monospace(format!("{fps} FPS  |  {:.1} ms", perf.frame_ms_avg));
                                ui.monospace(format!(
                                    "XYZ  {:.1}  {:.1}  {:.1}",
                                    player.pos.x, player.pos.y, player.pos.z
                                ));
                                ui.small(format!(
                                    "{time_label}  |  {}  |  F3 to hide",
                                    physics_state.voxel_scale.label()
                                ));
                            });
                        });
                }
            }
            if !creative_state.show_palette && (self.show_hud || creative_state.any_ui_open()) {
                let hotbar = egui::Area::new("hotbar_overlay".into())
                    .anchor(Align2::CENTER_BOTTOM, [0.0, -10.0])
                    .interactable(creative_state.show_panel || creative_state.show_palette)
                    .show(&self.ctx, |ui| {
                        egui::Frame::NONE.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                draw_hotbar_row(
                                    ui,
                                    creative_state,
                                    player,
                                    Some(&self.survival),
                                    &mut self.drag_source,
                                    &mut self.drag_target_slot,
                                );
                            });
                        });
                    });
                if self.show_hud && !creative_state.any_ui_open() {
                    let held_food = self.survival.hotbar[creative_state.selected_slot]
                        .and_then(|stack| match stack.item {
                            crate::world::inventory::Item::Food(food) => Some(food),
                            _ => None,
                        });
                    let vitals = adventure::hud(
                        &self.ctx,
                        &self.adventure,
                        hotbar.response.rect,
                        player.yaw,
                        self.survival.eat_timer,
                        self.survival.armor_points(),
                        held_food,
                        self.ctx.wants_pointer_input(),
                    );
                    let use_survival = creative_state.tool == ToolKind::Hand
                        && !player.flying
                        && !creative_state.show_panel;
                    // Item identity appears on selection, then clears the view.
                    let label: String = if use_survival {
                        self.survival.hotbar[creative_state.selected_slot]
                            .map_or_else(String::new, |s| s.item.name().to_owned())
                    } else if creative_state.tool == ToolKind::Hand {
                        creative_state.selected_block().display_name().to_owned()
                    } else {
                        creative_state.tool.label().to_owned()
                    };
                    let id = egui::Id::new("selected_item_notice");
                    let now = self.ctx.input(|i| i.time);
                    let mut notice = self
                        .ctx
                        .data_mut(|d| d.get_temp::<(String, f64)>(id))
                        .unwrap_or_else(|| (label.clone(), now));
                    if notice.0 != label {
                        notice = (label.clone(), now);
                    }
                    self.ctx.data_mut(|d| d.insert_temp(id, notice.clone()));
                    if !label.is_empty() && now - notice.1 < 1.5 {
                        let p = self
                            .ctx
                            .layer_painter(egui::LayerId::new(egui::Order::Foreground, id));
                        let pos =
                            egui::pos2(self.ctx.content_rect().center().x, vitals.top() - 6.0);
                        p.text(
                            pos + egui::vec2(1.0, 1.0),
                            Align2::CENTER_BOTTOM,
                            &label,
                            egui::FontId::proportional(15.0),
                            presentation::INK,
                        );
                        p.text(
                            pos,
                            Align2::CENTER_BOTTOM,
                            &label,
                            egui::FontId::proportional(15.0),
                            presentation::PAPER,
                        );
                    }

                    // Minimal pickup / overflow badge right beside the hotbar
                    if let Some(pickup) = &self.survival.pickup_notice {
                        egui::Area::new("survival_pickup_toast".into())
                            .anchor(
                                Align2::LEFT_BOTTOM,
                                [
                                    (hotbar.response.rect.right() + 10.0)
                                        .min(self.ctx.content_rect().right() - 150.0),
                                    -12.0,
                                ],
                            )
                            .interactable(false)
                            .show(&self.ctx, |ui| {
                                presentation::hud_frame().show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui.allocate_exact_size(
                                            egui::vec2(18.0, 18.0),
                                            egui::Sense::hover(),
                                        );
                                        inventory::draw_item_icon(ui.painter(), r, pickup.item);
                                        if pickup.overflow {
                                            ui.label(
                                                RichText::new("PACK FULL")
                                                    .size(11.5)
                                                    .strong()
                                                    .color(presentation::AMBER),
                                            );
                                        } else {
                                            ui.label(
                                                RichText::new(format!(
                                                    "+{} {}",
                                                    pickup.count,
                                                    pickup.item.name()
                                                ))
                                                .size(11.5)
                                                .color(presentation::JADE),
                                            );
                                        }
                                    });
                                });
                            });
                    }
                }
            }
        }
        match mode {
            UiMode::Playing => {
                if creative_state.show_panel {
                    egui::Window::new("Tool settings").id(egui::Id::new("compact_tools_v2"))
                        .default_pos([18.0, 18.0])
                        .default_size([350.0, 320.0])
                        .min_size([280.0, 260.0])
                        .resizable(true)
                        .movable(true)
                        .vscroll(true)
                        .max_height((self.ctx.content_rect().height() - 48.0).max(180.0))
                        .show(&self.ctx, |ui| {
                            creative_tools_ui_plain(
                                ui,
                                creative_state,
                                physics_state,
                                player,
                                &self.tool_textures,
                            );
                        });
                }

                if creative_state.show_material_editor { crate::material_studio::draw(&self.ctx, &mut creative_state.show_material_editor, time_state, &mut action); }
                if creative_state.show_palette {
                    inventory::draw(
                        &self.ctx,
                        creative_state,
                        player,
                        &mut self.survival,
                        &mut self.drag_source,
                        &mut self.drag_target_slot,
                        &mut self.hovered_palette_block,
                        self.expedition.as_ref(),
                        &self.factory,
                        &self.adventure,
                        &mut action,
                    );
                } else {
                    self.hovered_palette_block = None;
                    if matches!(self.drag_source, Some(DragSource::Palette(_))) {
                        self.drag_source = None;
                    }
                }

                if creative_state.show_quick_select {
                    quick_tool_ui_plain(
                        &self.ctx,
                        creative_state,
                        &self.tool_textures,
                        &mut self.hovered_quick_tool,
                        &mut self.hovered_quick_brush_shape,
                        &mut self.hovered_quick_area_shape,
                    );
                } else {
                    self.hovered_quick_tool = None;
                    self.hovered_quick_brush_shape = None;
                    self.hovered_quick_area_shape = None;
                }

                if self.show_hud && !creative_state.any_ui_open()
                    && self.ctx.content_rect().width() >= 1040.0 {
                    egui::Area::new("play_hint".into())
                        .anchor(Align2::LEFT_BOTTOM, [14.0, -18.0])
                        .interactable(false)
                        .show(&self.ctx, |ui| {
                            ui.label(
                                RichText::new("B tools  E inventory  Q quick")
                                    .small()
                                    .color(Color32::from_rgba_premultiplied(235, 238, 242, 180)),
                            );
                        });
                }

                if self.drag_source.is_some() && self.ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                    if let (Some(source), Some(target)) =
                        (self.drag_source.take(), self.drag_target_slot)
                    {
                        match source {
                            DragSource::Palette(block) => {
                                creative_state.assign_hotbar_slot(target, block);
                                creative_state.tool = ToolKind::Hand;
                            }
                            DragSource::Hotbar(from) => {
                                creative_state.swap_hotbar_slots(from, target);
                            }
                        }
                        player.selected_block = creative_state.selected_block();
                    }
                    self.drag_target_slot = None;
                }

                if self.show_hud && !creative_state.any_ui_open() {
                    egui::Area::new("crosshair_area".into())
                        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                        .interactable(false)
                        .show(&self.ctx, |ui| {
                            let (rect, _resp) = ui
                                .allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                            let center = rect.center();
                            let painter = ui.painter();

                            let shadow = egui::Stroke::new(
                                3.0,
                                Color32::from_rgba_premultiplied(0, 0, 0, 120),
                            );
                            painter.line_segment(
                                [center - egui::vec2(8.0, 0.0), center + egui::vec2(8.0, 0.0)],
                                shadow,
                            );
                            painter.line_segment(
                                [center - egui::vec2(0.0, 8.0), center + egui::vec2(0.0, 8.0)],
                                shadow,
                            );

                            let stroke = egui::Stroke::new(
                                1.5,
                                self.tool_feedback.color(),
                            );
                            painter.line_segment(
                                [center - egui::vec2(7.0, 0.0), center + egui::vec2(7.0, 0.0)],
                                stroke,
                            );
                            painter.line_segment(
                                [center - egui::vec2(0.0, 7.0), center + egui::vec2(0.0, 7.0)],
                                stroke,
                            );
                        });
                    draw_held_tool_overlay(
                        &self.ctx,
                        &self.held_tool_pose,
                        self.tool_textures.for_tool(creative_state.tool),
                        creative_state.tool,
                        time_state.time_of_day,
                    );
                    if creative_state.tool == ToolKind::Hand && self.survival.hotbar[creative_state.selected_slot].is_some_and(|s| matches!(s.item, crate::world::inventory::Item::Weapon(_))) {
                        draw_held_weapon_overlay(&self.ctx, self.adventure.weapon, &self.held_tool_pose);
                    }
                }
            }
            UiMode::Paused | UiMode::WorldSelect | UiMode::Loading => {}
        }

        if self.show_debug_performance_panel {
            self.build_debug_performance_panel(mode, perf);
        }

        if crate::torch_review::mode() == "drag" {
            crate::torch_review::check_drag(creative_state, self.drag_source.is_some());
        }
        if let Some(block) = self.drag_source.map(|source| match source {
            DragSource::Palette(block) => block,
            DragSource::Hotbar(slot) => creative_state.hotbar()[slot],
        }) {
            if let Some(pointer_pos) = self.ctx.input(|i| i.pointer.interact_pos()) {
                let painter = self.ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("material_drag_preview"),
                ));
                let rect = egui::Rect::from_center_size(
                    pointer_pos + egui::vec2(14.0, 14.0),
                    egui::vec2(32.0, 32.0),
                );
                draw_voxel_icon(&painter, rect, block);
            }
        }

        action
    }

    pub fn end_frame(
        &mut self,
        window: &Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let full_output = self.ctx.end_pass();
        self.state
            .handle_platform_output(window, full_output.platform_output);

        self.textures_delta = full_output.textures_delta;
        for (id,delta) in &mut self.textures_delta.set {
            if *id == egui::TextureId::Managed(0) { delta.options=egui::TextureOptions::NEAREST; }
        }
        self.shapes = self
            .ctx
            .tessellate(full_output.shapes, self.ctx.pixels_per_point());

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: self.ctx.pixels_per_point(),
        };

        for (id, image_delta) in &self.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        let command_buffers =
            self.renderer
                .update_buffers(device, queue, encoder, &self.shapes, &screen_descriptor);

        {
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            let mut rpass = rpass.forget_lifetime();
            self.renderer
                .render(&mut rpass, &self.shapes, &screen_descriptor);
        }

        queue.submit(command_buffers);

        for id in &self.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }

    pub fn end_frame_no_paint(&mut self, window: &Window) {
        let full_output = self.ctx.end_pass();
        self.state
            .handle_platform_output(window, full_output.platform_output);
        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}
