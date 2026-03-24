use egui::{Align2, Color32, RichText};
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
    StartWorld {
        world_type: WorldType,
        voxel_scale: VoxelScalePreset,
        seed: u32,
    },
    SetWeatherPreset(crate::world::WeatherPreset),
    RandomizeWeather,
    Resume,
    BackToWorldSelect,
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

    ui.strong("Low Clouds  (0 – 2000 m)");
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
            .custom_formatter(|v, _| format_span_m(v as f32, 900.0, 6000.0)),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.base_height, 0.05..=0.72)
            .text("  Low-layer Base")
            .custom_formatter(|v, _| format_height_m(v as f32, 350.0, 2200.0)),
    );
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.layer_thickness, 0.12..=1.0)
            .text("  Low-layer Depth")
            .custom_formatter(|v, _| format_span_m(v as f32, 250.0, 1800.0)),
    );

    ui.separator();

    ui.strong("Mid Clouds  (2000 – 7000 m)");
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
            .custom_formatter(|v, _| format_height_m(v as f32, 2500.0, 7500.0)),
    );

    ui.separator();

    ui.strong("High Clouds  (>6000 m)");
    ui.label(
        egui::RichText::new("Cirrus: thin ice-crystal wisps, 22° halo around sun")
            .small()
            .weak(),
    );
    ui.add(egui::Slider::new(&mut time_state.cloud_controls.cirrus, 0.0..=0.75).text("Cirrus"));
    ui.add(
        egui::Slider::new(&mut time_state.cloud_controls.layer_separation, 0.08..=0.72)
            .text("  High-layer Base")
            .custom_formatter(|v, _| format_height_m(v as f32, 6500.0, 12500.0)),
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
        ui.label("Tab palette");
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
        .corner_radius(egui::CornerRadius::same(8))
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
    ui.checkbox(
        &mut creative_state.shift_lock,
        "Shift lock (use Alt to descend)",
    );

    ui.separator();
    ui.label(RichText::new("Controls").strong());
    ui.label(
        "LMB breaks, RMB places. Ctrl + Scroll changes tool size. Shift + Scroll changes reach.",
    );
    ui.label("Scroll cycles hotbar slots. 1-9 selects slots. Drag hotbar slots to reorder them.");
    ui.label("B toggles this toolkit, Tab toggles the palette, Q toggles the tool switcher.");
}

fn creative_tools_ui_plain(
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
        ui.label("Tab palette");
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
    ui.label("Tools");
    ui.horizontal_wrapped(|ui| {
        for tool in ToolKind::ALL {
            let texture = tool_textures.for_tool(tool);
            let button = egui::Button::image_and_text(
                egui::Image::new((texture.texture.id(), egui::vec2(24.0, 24.0)))
                    .texture_options(egui::TextureOptions::NEAREST),
                tool.label(),
            )
            .selected(creative_state.tool == tool)
            .min_size(egui::vec2(120.0, 40.0));
            if ui.add(button).clicked() {
                creative_state.tool = tool;
            }
        }
    });
    ui.separator();
    ui.label(format!("Selected tool: {}", creative_state.tool.label()));
    let mut hovered_brush_shape = None;
    let mut hovered_area_shape = None;
    tool_settings_ui(
        ui,
        creative_state,
        &mut hovered_brush_shape,
        &mut hovered_area_shape,
    );
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
    ui.checkbox(
        &mut creative_state.shift_lock,
        "Shift lock (use Alt to descend)",
    );
    ui.separator();
    ui.label(RichText::new("Controls").strong());
    ui.label(
        "LMB breaks, RMB places. Ctrl + Scroll changes tool size. Shift + Scroll changes reach.",
    );
    ui.label("Scroll cycles hotbar slots. 1-9 selects slots. Drag hotbar slots to reorder them.");
    ui.label("B toggles this toolkit, Tab toggles the palette, Q toggles the tool switcher.");
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
        }
    }

    fn for_tool(&self, tool: ToolKind) -> &ToolTexture {
        match tool {
            ToolKind::Hand => &self.hand,
            ToolKind::Brush => &self.brush,
            ToolKind::BuildersWand => &self.builders_wand,
            ToolKind::DestructorWand => &self.destructor_wand,
            ToolKind::AreaTool => &self.area_tool,
        }
    }
}

#[derive(Default)]
struct HeldToolPoseState {
    action_phase: f32,
    equip_phase: f32,
    bob_time: f32,
    last_tool: Option<ToolKind>,
    last_action: Option<ToolUseAction>,
    action_kind: Option<ToolUseAction>,
}

impl HeldToolPoseState {
    fn update(&mut self, tool: ToolKind, action: Option<ToolUseAction>, move_speed: f32, dt: f32) {
        if self.last_tool != Some(tool) {
            self.last_tool = Some(tool);
            self.equip_phase = 1.0;
            self.action_phase = self.action_phase.min(0.28);
        }
        if action.is_some() && self.last_action.is_none() {
            self.action_phase = 1.0;
            self.action_kind = action;
        }
        self.last_action = action;
        self.action_phase = (self.action_phase - dt * 4.8).max(0.0);
        if self.action_phase <= 0.0 {
            self.action_kind = None;
        }
        self.equip_phase = (self.equip_phase - dt * 4.8).max(0.0);

        // Relaxed bobbing logic to match Minecraft footprint frequency better
        self.bob_time += dt * 0.6 + (move_speed * dt) * 0.5;
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
) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
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
    let bob_x = (walk_t * 1.0).sin() * 0.006;
    let bob_y = (walk_t * 2.0).cos().abs() * 0.012;
    let bob_pitch = (walk_t * 2.0).sin() * 0.005;
    let bob_roll = (walk_t * 1.0).cos() * 0.005;

    let base_mat = Mat4::from_translation(Vec3::new(bob_x, bob_y, 0.0))
        * Mat4::from_quat(Quat::from_euler(
            glam::EulerRot::YXZ,
            0.0,
            bob_pitch,
            bob_roll,
        ));

    let mut faces = Vec::new();
    let pi = std::f32::consts::PI;

    // Minecraft uses sin(sqrt(t) * pi) for the snappy strike
    let swing_g = (swing_t.sqrt() * pi).sin();
    let swing_f = (swing_t * pi).sin();
    let equip_y = -equip * 0.6;

    if tool == ToolKind::Hand {
        // Empty Hand - 1:1 Minecraft 1.15+ Transform Math
        let f = 1.0_f32; // 1.0 for right hand

        let x_swing_pos = -0.3 * swing_g;
        let y_swing_pos = 0.4 * (swing_t.sqrt() * pi * 2.0).sin();
        let z_swing_pos = -0.4 * swing_f;

        let mut mat = base_mat
            * Mat4::from_translation(Vec3::new(
                f * (x_swing_pos + 0.64),
                y_swing_pos - 0.6 + equip_y,
                z_swing_pos - 0.72,
            ));

        // 2. Base yaw
        mat *= Mat4::from_quat(Quat::from_rotation_y((f * 45.0_f32).to_radians()));

        // 3. Swing animations (rotations)
        let z_swing_rot = (swing_t * swing_t * pi).sin();
        let y_swing_rot = swing_g;
        mat *= Mat4::from_quat(Quat::from_rotation_y((f * y_swing_rot * 70.0).to_radians()));
        mat *= Mat4::from_quat(Quat::from_rotation_z(
            (f * z_swing_rot * -20.0).to_radians(),
        ));

        // 4. Complex arm setup rotations (MC legacy/FPS view specific)
        mat *= Mat4::from_translation(Vec3::new(f * -1.0, 3.6, 3.5));
        mat *= Mat4::from_quat(Quat::from_rotation_z((f * 120.0).to_radians()));
        mat *= Mat4::from_quat(Quat::from_rotation_x(200.0_f32.to_radians()));
        mat *= Mat4::from_quat(Quat::from_rotation_y((f * -135.0).to_radians()));
        mat *= Mat4::from_translation(Vec3::new(f * 5.6, 0.0, 0.0));

        // 5. ModelPart pivot offset (divided by 16 for meter scale)
        mat *= Mat4::from_translation(Vec3::new(-5.0 / 16.0, 2.0 / 16.0, 0.0));

        let skin_color = Color32::from_rgb(212, 163, 125);
        let sleeve_color = Color32::from_rgb(52, 101, 164); // Classic steve blue shirt

        // Box definitions (divided by 16 for meter scale)
        // MC right arm bounding box: -3.0, -2.0, -2.0 to 1.0, 10.0, 2.0
        // Size = 4, 12, 4. Center = -1, 4, 0
        let arm_size = Vec3::new(4.0 / 16.0, 12.0 / 16.0, 4.0 / 16.0);
        let arm_center = Vec3::new(-1.0 / 16.0, 4.0 / 16.0, 0.0);

        // MC sleeve covers top half
        let sleeve_size = Vec3::new(4.5 / 16.0, 6.0 / 16.0, 4.5 / 16.0);
        let sleeve_center = Vec3::new(-1.0 / 16.0, 1.0 / 16.0, 0.0);

        add_3d_box(
            &mut faces, mat, proj, rect, arm_center, arm_size, skin_color,
        );
        add_3d_box(
            &mut faces,
            mat,
            proj,
            rect,
            sleeve_center,
            sleeve_size,
            sleeve_color,
        );
    } else {
        // Held Tool - 1:1 Minecraft Transform Math
        let f = 1.0_f32; // 1.0 for right hand

        // Base transform (from shoulder to camera)
        let pivot_pos = Vec3::new(f * 0.56, -0.52 + equip_y, -0.72);
        let mut mat = base_mat * Mat4::from_translation(pivot_pos);

        // Strike translation: move the tool forward, down, and left
        let y_swing_rot = (swing_t * swing_t * pi).sin();
        let x_swing = -0.4 * swing_g;
        let y_swing = 0.2 * (swing_t.sqrt() * pi * 2.0).sin();
        let z_swing = -0.2 * swing_f;
        mat *= Mat4::from_translation(Vec3::new(f * x_swing, y_swing, z_swing));

        // Strike rotation
        mat *= Mat4::from_quat(Quat::from_rotation_y(
            (f * (45.0 - y_swing_rot * 20.0)).to_radians(),
        ));
        mat *= Mat4::from_quat(Quat::from_rotation_z((f * swing_g * -20.0).to_radians()));
        mat *= Mat4::from_quat(Quat::from_rotation_x((swing_g * -80.0).to_radians()));
        mat *= Mat4::from_quat(Quat::from_rotation_y((f * -45.0).to_radians()));

        // Exact Minecraft `firstperson_righthand` JSON display transform
        let item_rot = Quat::from_rotation_x(0.0_f32.to_radians())
            * Quat::from_rotation_y(-90.0_f32.to_radians())
            * Quat::from_rotation_z(25.0_f32.to_radians());

        let item_scale = Vec3::splat(0.38);
        let item_trans = Vec3::new(1.13 / 16.0, 3.2 / 16.0, 1.13 / 16.0);
        let item_center = Vec3::new(0.5, 0.5, 0.5);

        let local_mat = Mat4::from_translation(item_trans)
            * Mat4::from_quat(item_rot)
            * Mat4::from_scale(item_scale)
            * Mat4::from_translation(-item_center);

        let final_mat = mat * local_mat;

        // Standard item quad (0..1), face at Z = 0.5
        let tw = 1.0;
        let th = 1.0;
        let v_local = [
            Vec3::new(0.0, 0.0, 0.5), // bottom left
            Vec3::new(tw, 0.0, 0.5),  // bottom right
            Vec3::new(tw, th, 0.5),   // top right
            Vec3::new(0.0, th, 0.5),  // top left
        ];

        // Map UVs to the standard texture coordinates
        let uvs = [
            egui::pos2(0.0, 1.0), // bottom left
            egui::pos2(1.0, 1.0), // bottom right
            egui::pos2(1.0, 0.0), // top right
            egui::pos2(0.0, 0.0), // top left
        ];

        // Draw single highly-subdivided quad to prevent affine warping without Z-fighting
        add_3d_quad(
            &mut faces,
            final_mat,
            proj,
            rect,
            v_local,
            uvs,
            texture.texture.id(),
            Color32::WHITE,
        );
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
            painter.add(egui::Shape::convex_polygon(
                face.vertices.to_vec(),
                face.color,
                egui::Stroke::new(1.0, face.color),
            ));
        }
    }
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
        .corner_radius(egui::CornerRadius::same(255))
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
        .corner_radius(egui::CornerRadius::same(10))
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
        .corner_radius(egui::CornerRadius::same(10))
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
            let rounding = egui::CornerRadius::same(8);
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
    }
}

fn tool_shortcut_label(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Hand => "Z",
        ToolKind::Brush => "X",
        ToolKind::BuildersWand => "C",
        ToolKind::DestructorWand => "V",
        ToolKind::AreaTool => "G",
    }
}

fn tool_description(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Hand => "Single-voxel editing at the cursor target.",
        ToolKind::Brush => "Radius-based sculpting for fast terrain painting and erasing.",
        ToolKind::BuildersWand => "Flood-style placement that grows from the target surface.",
        ToolKind::DestructorWand => "Flood-style removal for stripping large connected regions.",
        ToolKind::AreaTool => "Footprint-based bulk edits for discs, pads, and layered cuts.",
    }
}

fn tool_detail_summary(creative_state: &CreativeState, tool: ToolKind) -> String {
    match tool {
        ToolKind::Hand => format!("Range {:.1}m", creative_state.placement_distance),
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
    }
}

fn tool_settings_ui(
    ui: &mut egui::Ui,
    creative_state: &mut CreativeState,
    hovered_brush_shape: &mut Option<BrushShape>,
    hovered_area_shape: &mut Option<AreaFootprintShape>,
) {
    *hovered_brush_shape = None;
    *hovered_area_shape = None;

    match creative_state.tool {
        ToolKind::Hand => {
            ui.label("Direct voxel edit");
            ui.add(
                egui::Slider::new(&mut creative_state.placement_distance, 2.0..=24.0)
                    .text("Reach (m)"),
            );
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
            ui.add(egui::Slider::new(&mut creative_state.brush_radius, 0..=8).text("Radius"));
            ui.add(
                egui::Slider::new(&mut creative_state.placement_distance, 2.0..=24.0)
                    .text("Reach (m)"),
            );
            ui.add(
                egui::Slider::new(&mut creative_state.preview_cap, 64..=4096).text("Preview cap"),
            );
        }
        ToolKind::BuildersWand | ToolKind::DestructorWand => {
            let cap_label = if matches!(creative_state.tool, ToolKind::BuildersWand) {
                "Placement cap"
            } else {
                "Removal cap"
            };
            ui.add(egui::Slider::new(&mut creative_state.wand_max_blocks, 8..=512).text(cap_label));
            ui.add(
                egui::Slider::new(&mut creative_state.placement_distance, 2.0..=24.0)
                    .text("Reach (m)"),
            );
            ui.add(
                egui::Slider::new(&mut creative_state.preview_cap, 64..=4096).text("Preview cap"),
            );
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
            ui.add(egui::Slider::new(&mut creative_state.area_tool.radius, 0..=12).text("Radius"));
            ui.add(
                egui::Slider::new(&mut creative_state.area_tool.thickness, 1..=8).text("Thickness"),
            );
            ui.add(
                egui::Slider::new(&mut creative_state.placement_distance, 2.0..=24.0)
                    .text("Reach (m)"),
            );
            ui.add(
                egui::Slider::new(&mut creative_state.preview_cap, 64..=4096).text("Preview cap"),
            );
        }
    }
}

fn draw_voxel_icon(painter: &egui::Painter, rect: egui::Rect, block: BlockId) {
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

    painter.add(egui::Shape::convex_polygon(
        left_pts,
        left,
        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(0, 0, 0, 80)),
    ));
    painter.add(egui::Shape::convex_polygon(
        right_pts,
        right,
        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(0, 0, 0, 100)),
    ));
    painter.add(egui::Shape::convex_polygon(
        top_pts,
        top,
        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 28)),
    ));
}

fn draw_palette_material_card(ui: &mut egui::Ui, size: [f32; 2], block: BlockId) -> egui::Response {
    let desired = egui::vec2(size[0], size[1]);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let painter = ui.painter();
    let rounding = egui::CornerRadius::same(0);
    painter.rect_filled(
        rect,
        rounding,
        Color32::from_rgba_premultiplied(18, 22, 30, 238),
    );
    painter.rect_stroke(
        rect,
        rounding,
        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 28)),
        egui::StrokeKind::Outside,
    );

    let icon_rect = egui::Rect::from_min_max(
        rect.min + egui::vec2(10.0, 10.0),
        egui::pos2(rect.max.x - 10.0, rect.min.y + size[1] * 0.70),
    );
    draw_voxel_icon(painter, icon_rect, block);

    painter.text(
        egui::pos2(rect.center().x, rect.max.y - 16.0),
        Align2::CENTER_CENTER,
        block.display_name(),
        egui::FontId::proportional(11.0),
        Color32::WHITE,
    );
    response.on_hover_text(block.display_name())
}

fn draw_hotbar_slot_button(
    ui: &mut egui::Ui,
    block: BlockId,
    slot: usize,
    selected: bool,
    drag_targeted: bool,
) -> egui::Response {
    let size = 70.0;
    let desired = egui::vec2(size, size);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let painter = ui.painter();
    let rounding = egui::CornerRadius::same(4);

    // Background
    painter.rect_filled(
        rect,
        rounding,
        Color32::from_rgba_premultiplied(30, 30, 30, 160),
    );

    // Border
    let stroke_color = if drag_targeted {
        Color32::from_rgb(255, 214, 96)
    } else if selected {
        Color32::from_rgb(255, 255, 255)
    } else {
        Color32::from_rgba_premultiplied(120, 120, 120, 100)
    };

    let stroke_width = if selected || drag_targeted { 5.0 } else { 3.0 };
    painter.rect_stroke(
        rect,
        rounding,
        egui::Stroke::new(stroke_width, stroke_color),
        egui::StrokeKind::Inside,
    );

    // Number indicator
    painter.text(
        rect.min + egui::vec2(8.0, 6.0),
        Align2::LEFT_TOP,
        hotbar_key_label(slot),
        egui::FontId::proportional(22.0),
        if selected {
            Color32::WHITE
        } else {
            Color32::from_rgba_unmultiplied(200, 200, 200, 180)
        },
    );

    // Block icon
    if block != BlockId::Air {
        let icon_rect = egui::Rect::from_center_size(
            rect.center() + egui::vec2(0.0, 2.0),
            egui::vec2(48.0, 48.0),
        );
        draw_voxel_icon(painter, icon_rect, block);
    }

    response.on_hover_text(block.display_name())
}

fn draw_hotbar_row(
    ui: &mut egui::Ui,
    creative_state: &mut CreativeState,
    player: &mut Player,
    drag_source: &mut Option<DragSource>,
    drag_target_slot: &mut Option<usize>,
) {
    *drag_target_slot = None;
    let hotbar = *creative_state.hotbar();
    for (slot, block) in hotbar.into_iter().enumerate() {
        let response = draw_hotbar_slot_button(
            ui,
            block,
            slot,
            slot == creative_state.selected_slot,
            *drag_target_slot == Some(slot),
        );
        if response.clicked() {
            creative_state.select_slot(slot);
            player.selected_block = creative_state.selected_block();
        }
        if response.drag_started() {
            *drag_source = Some(DragSource::Hotbar(slot));
        }
        if drag_source.is_some() && response.hovered() {
            *drag_target_slot = Some(slot);
        }
    }
}

fn material_palette_ui(
    ctx: &egui::Context,
    creative_state: &mut CreativeState,
    player: &mut Player,
    drag_source: &mut Option<DragSource>,
    hovered_palette_block: &mut Option<BlockId>,
) {
    egui::Window::new("Material Palette")
        .anchor(Align2::CENTER_CENTER, [0.0, 20.0])
        .default_width(920.0)
        .min_width(760.0)
        .max_width(1040.0)
        .max_height(480.0)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            *hovered_palette_block = None;
            ui.horizontal_wrapped(|ui| {
                ui.label("Pick a material, or drag one onto a hotbar slot.");
                ui.separator();
                ui.label("Press 1-9 to select a destination slot.");
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    ui.horizontal_wrapped(|ui| {
                        for def in material_defs().iter().filter(|def| def.tags.selectable) {
                            let response = draw_palette_material_card(ui, [112.0, 116.0], def.id);
                            if response.clicked() {
                                creative_state
                                    .assign_hotbar_slot(creative_state.selected_slot, def.id);
                                player.selected_block = creative_state.selected_block();
                            }
                            if response.hovered() {
                                *hovered_palette_block = Some(def.id);
                            }
                            if response.drag_started() {
                                *drag_source = Some(DragSource::Palette(def.id));
                            }
                        }
                    });
                });

            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "Selected slot {}: {}",
                    hotbar_key_label(creative_state.selected_slot),
                    creative_state.selected_block().display_name()
                ));
                if let Some(block) = hovered_palette_block.as_ref() {
                    ui.separator();
                    ui.colored_label(material_color32(*block), block.display_name());
                }
            });
        });
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
                                .corner_radius(egui::CornerRadius::same(8))
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
                        .corner_radius(egui::CornerRadius::same(8))
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
                                    "Ctrl + Scroll size",
                                    Color32::from_rgb(102, 150, 240),
                                );
                                status_chip(
                                    ui,
                                    "Shift + Scroll reach",
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
    let preview_tool = creative_state.tool;
    egui::Window::new("Quick Select")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_width(540.0);
            ui.label("Q closes this menu. Click to switch tools.");
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for tool in ToolKind::ALL {
                    let texture = tool_textures.for_tool(tool);
                    let button = egui::Button::image_and_text(
                        egui::Image::new((texture.texture.id(), egui::vec2(28.0, 28.0)))
                            .texture_options(egui::TextureOptions::NEAREST),
                        tool.label(),
                    )
                    .selected(preview_tool == tool)
                    .min_size(egui::vec2(150.0, 52.0));
                    let response = ui.add(button);
                    if response.hovered() {
                        *hovered_tool = Some(tool);
                    }
                    if response.clicked() {
                        creative_state.tool = tool;
                    }
                }
            });
            ui.separator();
            match preview_tool {
                ToolKind::Hand => {
                    ui.label("Hand");
                    ui.label("Default single-block interaction.");
                }
                ToolKind::Brush => {
                    ui.label("Brush shape");
                    ui.horizontal_wrapped(|ui| {
                        for shape in BrushShape::ALL {
                            let selected = creative_state.brush_shape == shape;
                            let response = ui.selectable_label(selected, shape.label());
                            if response.hovered() {
                                *hovered_brush_shape = Some(shape);
                            }
                            if response.clicked() {
                                creative_state.brush_shape = shape;
                                *hovered_brush_shape = Some(shape);
                            }
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut creative_state.brush_radius, 0..=8)
                            .text("Brush radius"),
                    );
                }
                ToolKind::AreaTool => {
                    ui.label("Area footprint");
                    ui.horizontal_wrapped(|ui| {
                        for shape in AreaFootprintShape::ALL {
                            let selected = creative_state.area_tool.shape == shape;
                            let response = ui.selectable_label(selected, shape.label());
                            if response.hovered() {
                                *hovered_area_shape = Some(shape);
                            }
                            if response.clicked() {
                                creative_state.area_tool.shape = shape;
                                *hovered_area_shape = Some(shape);
                            }
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut creative_state.area_tool.radius, 0..=12)
                            .text("Area radius"),
                    );
                    ui.add(
                        egui::Slider::new(&mut creative_state.area_tool.thickness, 1..=4)
                            .text("Thickness"),
                    );
                }
                ToolKind::BuildersWand | ToolKind::DestructorWand => {
                    ui.add(
                        egui::Slider::new(&mut creative_state.wand_max_blocks, 8..=512)
                            .text("Connected block cap"),
                    );
                }
            }
            ui.separator();
            ui.label("Z Hand, X Brush, C Builder, V Destructor, G Area.");
        });
}

pub struct EguiUi {
    pub ctx: egui::Context,
    state: egui_winit::State,
    renderer: Renderer,
    pixels_per_point: f32,
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
}

impl EguiUi {
    pub fn new(
        window: &Window,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let ctx = egui::Context::default();

        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );

        let renderer = Renderer::new(device, surface_format, RendererOptions::default());
        let tool_textures = ToolTextures::load(&ctx, "assets");

        Self {
            ctx,
            state,
            renderer,
            pixels_per_point: window.scale_factor() as f32,
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
        }
    }

    pub fn on_window_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    pub fn wants_input(&self) -> bool {
        self.ctx.wants_pointer_input() || self.ctx.wants_keyboard_input()
    }

    pub fn begin_frame(&mut self, window: &Window) {
        self.pixels_per_point = window.scale_factor() as f32;
        let raw_input = self.state.take_egui_input(window);
        self.ctx.begin_pass(raw_input);
    }

    pub fn build_loading_ui(&mut self, loading: &LoadingUiState) {
        egui::CentralPanel::default().show(&self.ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.18).max(40.0));

                ui.heading(RichText::new(&loading.title).size(28.0).strong());
                ui.add_space(10.0);

                ui.label(
                    RichText::new(&loading.detail)
                        .size(15.0)
                        .color(Color32::from_rgb(200, 205, 215)),
                );

                ui.add_space(18.0);

                let progress = loading.progress_01.clamp(0.0, 1.0);
                ui.add_sized(
                    [420.0, 18.0],
                    egui::ProgressBar::new(progress)
                        .show_percentage()
                        .animate(true),
                );

                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("{:.0}%", progress * 100.0))
                        .color(Color32::from_rgb(140, 180, 255)),
                );

                ui.add_space(18.0);
                ui.label(
                    RichText::new(
                        "The window stays responsive while renderer resources are prepared.",
                    )
                    .small()
                    .weak(),
                );
            });
        });
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
                    .corner_radius(egui::CornerRadius::same(255))
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
        let time_label = format_clock(time_state.time_of_day);
        let move_speed = player.vel.xz().length();
        player.selected_block = creative_state.selected_block();
        self.held_tool_pose
            .update(creative_state.tool, using_tool, move_speed, frame_dt);

        if matches!(mode, UiMode::Loading) {
            return UiAction::None;
        }

        if matches!(mode, UiMode::Paused) {
            egui::TopBottomPanel::top("top_bar").show(&self.ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("FPS: {}", fps)).strong());
                    ui.separator();
                    ui.label(format!(
                        "Pos: {:.1}, {:.1}, {:.1}",
                        player.pos.x, player.pos.y, player.pos.z
                    ));
                    ui.separator();
                    ui.label(format!(
                        "Mode: {}",
                        if player.flying { "Flying" } else { "Walking" }
                    ));
                    ui.separator();
                    ui.label(format!(
                        "Block: {}",
                        creative_state.selected_block().display_name()
                    ));
                    ui.separator();
                    let tool_texture = self.tool_textures.for_tool(creative_state.tool);
                    ui.add(
                        egui::Image::new((tool_texture.texture.id(), egui::vec2(18.0, 18.0)))
                            .texture_options(egui::TextureOptions::NEAREST),
                    );
                    ui.label(format!("Tool: {}", creative_state.tool.label()));
                    ui.separator();
                    ui.label(creative_state.cursor_summary());
                    ui.separator();
                    ui.label("B tools");
                    ui.separator();
                    ui.label("Tab palette");
                    ui.separator();
                    ui.label("Q quick");
                    ui.separator();
                    ui.label(RichText::new(format!("Time: {time_label}")).strong());
                    ui.separator();
                    let physics_text = if physics_state.enabled {
                        RichText::new("Physics: RUNNING").color(Color32::LIGHT_GREEN)
                    } else {
                        RichText::new("Physics: PAUSED").color(Color32::LIGHT_RED)
                    };
                    ui.label(physics_text);
                    ui.separator();
                    ui.label(format!("Voxel: {}", physics_state.voxel_scale.label()));
                    ui.separator();
                    ui.label(format!(
                        "Frame {:.2} ms avg / {:.2} ms p99",
                        perf.frame_ms_avg, perf.frame_ms_p99
                    ));
                    ui.separator();
                    ui.label("Render");
                    ui.add_sized(
                        [110.0, 18.0],
                        egui::Slider::new(&mut render_state.settings.view_distance_chunks, 12..=96)
                            .text("Chunks"),
                    );
                    ui.add_sized(
                        [120.0, 18.0],
                        egui::Slider::new(&mut render_state.settings.lod_start_chunks, 4..=96)
                            .text("LOD Start"),
                    );
                    ui.add_sized(
                        [95.0, 18.0],
                        egui::Slider::new(&mut render_state.settings.lod_levels, 1..=3)
                            .text("LOD Lvls"),
                    );
                    egui::ComboBox::from_label("Debug View")
                        .selected_text(render_state.debug_view.label())
                        .show_ui(ui, |ui| {
                            for view in RenderDebugView::all() {
                                ui.selectable_value(
                                    &mut render_state.debug_view,
                                    view,
                                    view.label(),
                                );
                            }
                        });
                    egui::ComboBox::from_label("Atmosphere")
                        .selected_text(render_state.atmosphere_mode.label())
                        .show_ui(ui, |ui| {
                            for mode in AtmosphereMode::all() {
                                ui.selectable_value(
                                    &mut render_state.atmosphere_mode,
                                    mode,
                                    mode.label(),
                                );
                            }
                        });
                });
            });

            egui::TopBottomPanel::bottom("bottom_bar").show(&self.ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    draw_hotbar_row(
                        ui,
                        creative_state,
                        player,
                        &mut self.drag_source,
                        &mut self.drag_target_slot,
                    );
                });
            });
        } else if matches!(mode, UiMode::Playing) {
            egui::Area::new("play_status".into())
                .anchor(Align2::LEFT_TOP, [14.0, 14.0])
                .interactable(false)
                .show(&self.ctx, |ui| {
                    ui.group(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{} FPS", fps)).strong());
                            ui.separator();
                            ui.label(format!("Time {time_label}"));
                            ui.separator();
                            ui.label(format!("Voxel {}", physics_state.voxel_scale.label()));
                            ui.separator();
                            ui.label(creative_state.cursor_summary());
                        });
                    });
                });

            egui::Area::new("hotbar_overlay".into())
                .anchor(Align2::CENTER_BOTTOM, [0.0, -24.0])
                .interactable(creative_state.show_panel || creative_state.show_palette)
                .show(&self.ctx, |ui| {
                    egui::Frame::NONE.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            draw_hotbar_row(
                                ui,
                                creative_state,
                                player,
                                &mut self.drag_source,
                                &mut self.drag_target_slot,
                            );
                        });
                    });
                });
        }

        match mode {
            UiMode::Playing => {
                if creative_state.show_panel {
                    egui::Window::new("Creative Tools")
                        .default_pos([18.0, 104.0])
                        .default_size([344.0, 420.0])
                        .min_size([280.0, 260.0])
                        .resizable(true)
                        .movable(true)
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

                if creative_state.show_palette {
                    material_palette_ui(
                        &self.ctx,
                        creative_state,
                        player,
                        &mut self.drag_source,
                        &mut self.hovered_palette_block,
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

                if !creative_state.any_ui_open() {
                    egui::Area::new("play_hint".into())
                        .anchor(Align2::LEFT_BOTTOM, [14.0, -18.0])
                        .interactable(false)
                        .show(&self.ctx, |ui| {
                            ui.label(
                                RichText::new("B tools  Tab palette  Q quick")
                                    .small()
                                    .color(Color32::from_rgba_premultiplied(235, 238, 242, 180)),
                            );
                        });
                }

                if self.drag_source.is_some() && self.ctx.input(|i| i.pointer.any_released()) {
                    if let (Some(source), Some(target)) =
                        (self.drag_source.take(), self.drag_target_slot)
                    {
                        match source {
                            DragSource::Palette(block) => {
                                creative_state.assign_hotbar_slot(target, block);
                            }
                            DragSource::Hotbar(from) => {
                                creative_state.swap_hotbar_slots(from, target);
                            }
                        }
                        player.selected_block = creative_state.selected_block();
                    }
                    self.drag_target_slot = None;
                }

                egui::Area::new("crosshair_area".into())
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .interactable(false)
                    .show(&self.ctx, |ui| {
                        let (rect, _resp) =
                            ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                        let center = rect.center();
                        let painter = ui.painter();

                        let shadow =
                            egui::Stroke::new(3.0, Color32::from_rgba_premultiplied(0, 0, 0, 120));
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
                            Color32::from_rgba_premultiplied(240, 240, 240, 255),
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
                );
            }
            UiMode::Paused => {
                egui::Window::new("Creative Lighting")
                    .default_pos([12.0, 92.0])
                    .default_size([368.0, 520.0])
                    .min_size([300.0, 280.0])
                    .resizable(true)
                    .movable(true)
                    .vscroll(true)
                    .show(&self.ctx, |ui| {
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
                        if ui.button("Dusk").clicked() {
                            time_state.time_of_day = 0.79;
                        }
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
                        cloud_controls_ui(ui, time_state, &mut action);
                        ui.separator();
                        ui.checkbox(&mut physics_state.enabled, "Fluid physics (P)");
                    });

                if creative_state.show_panel {
                    egui::Window::new("Creative Tools")
                        .default_pos([12.0, 420.0])
                        .default_size([344.0, 420.0])
                        .min_size([280.0, 260.0])
                        .resizable(true)
                        .movable(true)
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

                if creative_state.show_palette {
                    material_palette_ui(
                        &self.ctx,
                        creative_state,
                        player,
                        &mut self.drag_source,
                        &mut self.hovered_palette_block,
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

                if self.drag_source.is_some() && self.ctx.input(|i| i.pointer.any_released()) {
                    if let (Some(source), Some(target)) =
                        (self.drag_source.take(), self.drag_target_slot)
                    {
                        match source {
                            DragSource::Palette(block) => {
                                creative_state.assign_hotbar_slot(target, block);
                            }
                            DragSource::Hotbar(from) => {
                                creative_state.swap_hotbar_slots(from, target);
                            }
                        }
                        player.selected_block = creative_state.selected_block();
                    }
                    self.drag_target_slot = None;
                }

                egui::Window::new("Paused")
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .collapsible(false)
                    .resizable(false)
                    .show(&self.ctx, |ui| {
                        ui.label("Double-tap jump to toggle flying.");
                        ui.separator();

                        if ui.button("Resume").clicked() {
                            action = UiAction::Resume;
                        }
                        if ui.button("Back to World Select").clicked() {
                            action = UiAction::BackToWorldSelect;
                        }
                        if ui.button("Quit").clicked() {
                            action = UiAction::Quit;
                        }
                    });
            }
            UiMode::WorldSelect => {
                egui::Window::new("Create World")
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .collapsible(false)
                    .resizable(false)
                    .show(&self.ctx, |ui| {
                        ui.heading("Select world type");
                        ui.separator();

                        ui.radio_value(selected_world, WorldType::Superflat, "Superflat");
                        ui.radio_value(selected_world, WorldType::Hills, "Hills");
                        ui.radio_value(selected_world, WorldType::Ridged, "Ridged");

                        ui.separator();
                        ui.heading("Voxel Scale");
                        for preset in VoxelScalePreset::ALL {
                            ui.radio_value(selected_voxel_scale, preset, preset.label());
                        }
                        ui.label(
                            RichText::new(
                                "Terrain and foliage are sampled in meter-space; smaller voxels increase fidelity without changing biome layout.",
                            )
                            .color(Color32::LIGHT_BLUE),
                        );

                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Seed");
                            ui.add(
                                egui::DragValue::new(selected_seed)
                                    .speed(1.0)
                                    .range(0..=u32::MAX),
                            );
                            if ui.button("Randomize").clicked() {
                                *selected_seed = next_seed(*selected_seed);
                            }
                        });
                        ui.label(
                            RichText::new(format!("Hex: 0x{selected_seed:08X}"))
                                .color(Color32::LIGHT_BLUE),
                        );

                        ui.separator();
                        ui.label(RichText::new("Biomes are active.").color(Color32::LIGHT_GREEN));
                        ui.label("Double-tap jump toggles flying in-game.");

                        ui.separator();
                        if ui.button("Start World").clicked() {
                            action = UiAction::StartWorld {
                                world_type: *selected_world,
                                voxel_scale: *selected_voxel_scale,
                                seed: *selected_seed,
                            };
                        }
                    });
            }
            UiMode::Loading => {}
        }

        if matches!(mode, UiMode::Paused | UiMode::Playing) {
            self.build_debug_performance_panel(mode, perf);
        }

        if let Some(DragSource::Palette(block)) = self.drag_source {
            if let Some(pointer_pos) = self.ctx.input(|i| i.pointer.interact_pos()) {
                let painter = self.ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("material_drag_preview"),
                ));
                let rect = egui::Rect::from_center_size(
                    pointer_pos + egui::vec2(14.0, 14.0),
                    egui::vec2(18.0, 18.0),
                );
                painter.rect_filled(rect, egui::CornerRadius::same(3), material_color32(block));
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::same(3),
                    egui::Stroke::new(1.0, Color32::BLACK),
                    egui::StrokeKind::Outside,
                );
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
        self.shapes = self
            .ctx
            .tessellate(full_output.shapes, self.ctx.pixels_per_point());

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: self.pixels_per_point,
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
