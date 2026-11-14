//! Planet recipes and a debounced preview of the actual drainage solve.
use egui::{ColorImage, RichText, TextureHandle, TextureOptions, vec2};
use engine_core::VoxelScalePreset;
use engine_voxel::{CUSTOM_PLANET_GENERATOR_VERSION, PLANET_DIALS, PlanetSettings, WorldSampleConfig, WorldType};
use std::{sync::mpsc, time::{Duration, Instant}};

const PRESETS: [(&str, [u8; PLANET_DIALS]); 8] = [
    ("Temperate", [50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50]),
    ("Archipelago", [82, 12, 32, 60, 65, 65, 22, 70, 45, 50, 36, 65]),
    ("Alpine", [30, 65, 95, 22, 30, 60, 70, 45, 65, 75, 45, 75]),
    ("Desert", [15, 82, 40, 75, 95, 8, 60, 8, 50, 65, 75, 12]),
    ("Rainforest", [46, 58, 65, 80, 80, 95, 12, 100, 65, 55, 70, 85]),
    ("Frozen", [44, 65, 65, 65, 0, 55, 95, 16, 55, 60, 50, 70]),
    ("Cavernous", [32, 60, 70, 40, 48, 48, 45, 40, 100, 90, 58, 90]),
    ("Relic fields", [30, 72, 25, 85, 58, 42, 35, 25, 65, 65, 100, 45]),
];
const LABELS: [&str; PLANET_DIALS] = [
    "Ocean extent", "Continent size", "Mountain height", "Erosion", "Temperature",
    "Rainfall", "Seasonality", "Forest density", "Natural caves", "Ore deposits", "Monuments", "Lake depth",
];
const HINTS: [&str; PLANET_DIALS] = [
    "Land balance · Inland seas and coasts",
    "Small islands · Large landmasses",
    "Summit scale above sea level",
    "Weak carving · Strong river carving",
    "Mean temperature offset · Latitude and altitude still apply",
    "Rainfall multiplier · Drainage and vegetation",
    "Axial tilt · Seasonal temperature range",
    "Woodland multiplier · Climate still limits growth",
    "Natural cave frequency · Zero removes natural cave carving",
    "Deposit abundance · Rock and depth still apply",
    "Surveyor monument placement · Suitable dry ground required",
    "Maximum depth of solved inland lakes",
];
const GROUPS: [(&str, &[usize]); 4] = [
    ("Land & Water", &[0, 1, 2, 3, 11]),
    ("Climate", &[4, 5, 6]),
    ("Life & Relics", &[7, 10]),
    ("Underground", &[8, 9]),
];

struct Maps { images: [ColorImage; 4], land: u32, peak: f32 }
struct Pending { cfg: WorldSampleConfig, rx: mpsc::Receiver<Maps> }

pub(super) struct State {
    pub settings: PlanetSettings,
    locks: [bool; PLANET_DIALS],
    seed_lock: bool,
    group: usize,
    layer: usize,
    previous: Option<(u32, PlanetSettings)>,
    recipe: String,
    recipe_error: Option<&'static str>,
    desired: Option<WorldSampleConfig>,
    changed_at: Instant,
    pending: Option<Pending>,
    shown: Option<WorldSampleConfig>,
    maps: Option<Maps>,
    textures: Vec<TextureHandle>,
    preview_error: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            settings: PlanetSettings::default(), locks: [false; PLANET_DIALS], seed_lock: false,
            group: 0, layer: 0, previous: None, recipe: String::new(), recipe_error: None,
            desired: None, changed_at: Instant::now(), pending: None, shown: None,
            maps: None, textures: Vec::new(), preview_error: false,
        }
    }
}

fn value(i: usize, v: u8) -> String {
    let u = f64::from(v) / 100.0;
    match i {
        2 => format!("{:.0} m", 80.0 + u * 400.0),
        4 => format!("{:+.0} °C", (u - 0.5) * 40.0),
        6 => format!("{:.1}°", u * 47.0),
        5 | 7 | 8 | 9 => format!("{}%", u32::from(v) * 2),
        11 => format!("{:.0} m", 2.0 + u * 56.0),
        _ => format!("{v}%"),
    }
}

/// Length of the simulated geological history. The full length is stored as
/// zero ("generator default") so an untouched slider keeps the planet's identity.
fn history_slider(ui: &mut egui::Ui, settings: &mut PlanetSettings) {
    use engine_voxel::{FULL_HISTORY_MYR as full, HISTORY_MYR_RANGE, HISTORY_MYR_STEP};
    let mut myr = settings.history_myr().unwrap_or(full);
    ui.horizontal(|ui| {
        ui.label("History").on_hover_text("Simulated geological time. Shorter histories load faster");
        ui.label(RichText::new(format!("{myr} Myr")).strong());
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 36.0).max(40.0);
        ui.add(egui::Slider::new(&mut myr, HISTORY_MYR_RANGE).step_by(f64::from(HISTORY_MYR_STEP)).show_value(false));
        if ui.small_button("↺").on_hover_text("Default value").clicked() { myr = full; }
    });
    settings.history_myr = PlanetSettings::stored_history(myr);
}

pub(super) fn draw(ui: &mut egui::Ui, state: &mut State, seed: &mut u32, scale: VoxelScalePreset) {
    let before = (*seed, state.settings);
    super::front_end::caption(ui, "PLANET");
    ui.horizontal_wrapped(|ui| {
        ui.label("Civilization");
        ui.selectable_value(&mut state.settings.civilization, engine_voxel::CivilizationMode::Living, "Living")
            .on_hover_text("Settlements, residents, trade, and nations");
        ui.selectable_value(&mut state.settings.civilization, engine_voxel::CivilizationMode::Wilderness, "Wilderness")
            .on_hover_text("Wildlife and ruins, without living settlements or nations");
    });
    history_slider(ui, &mut state.settings);
    ui.horizontal_wrapped(|ui| {
        for (name, values) in PRESETS {
            if ui.selectable_label(state.settings.values == values, name).clicked() {
                for (i, v) in values.into_iter().enumerate() {
                    if !state.locks[i] { state.settings.values[i] = v; }
                }
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Reroll").on_hover_text("Unlocked settings and seed").clicked() {
            let salt = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos();
            let roll_seed = seed.wrapping_add(salt).rotate_left(13) ^ 0xa341316c;
            let rolled = PlanetSettings::randomized(roll_seed);
            if !state.seed_lock { *seed = roll_seed; }
            for i in 0..PLANET_DIALS {
                if !state.locks[i] { state.settings.values[i] = rolled.values[i]; }
            }
        }
        if ui.add_enabled(state.previous.is_some(), egui::Button::new("Undo")).clicked()
            && let Some(previous) = state.previous.take() {
                (*seed, state.settings) = previous;
        }
        ui.checkbox(&mut state.seed_lock, "Seed lock");
        if ui.small_button("Unlock all").clicked() { state.locks.fill(false); state.seed_lock = false; }
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        for (i, (label, _)) in GROUPS.iter().enumerate() { ui.selectable_value(&mut state.group, i, *label); }
    });
    for &i in GROUPS[state.group].1 {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.label(LABELS[i]).on_hover_text(HINTS[i]);
                ui.label(RichText::new(value(i, state.settings.values[i])).strong());
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.locks[i], "Lock").on_hover_text("Retained by presets and rerolls");
                ui.spacing_mut().slider_width = (ui.available_width() - 36.0).max(40.0);
                ui.add(egui::Slider::new(&mut state.settings.values[i], 0..=100)
                    .show_value(false)).on_hover_text(HINTS[i]);
                if ui.small_button("↺").on_hover_text("Default value").clicked() { state.settings.values[i] = 50; }
            });
        });
    }
    if before != (*seed, state.settings) { state.previous = Some(before); }
    let cfg = WorldSampleConfig {
        generator_version: CUSTOM_PLANET_GENERATOR_VERSION, seed: *seed, world_type: WorldType::Planet,
        voxel_scale: scale, planet: Some(state.settings),
    };
    preview(ui, state, cfg);
    ui.collapsing("Planet recipe", |ui| {
        if ui.button("Copy recipe").clicked() { ui.ctx().copy_text(state.settings.recipe(*seed)); }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut state.recipe).hint_text("AX136-…").char_limit(64).desired_width((ui.available_width() - 65.0).max(80.0)));
            if ui.button("Apply").clicked() {
                match PlanetSettings::from_recipe(&state.recipe) {
                    Ok((s, settings)) => { state.previous = Some((*seed, state.settings)); *seed = s; state.settings = settings; state.recipe_error = None; }
                    Err(error) => state.recipe_error = Some(error),
                }
            }
        });
        if let Some(error) = state.recipe_error { ui.colored_label(super::presentation::DANGER, error); }
    });
}

fn preview(ui: &mut egui::Ui, state: &mut State, cfg: WorldSampleConfig) {
    if state.desired != Some(cfg) {
        state.desired = Some(cfg);
        state.changed_at = Instant::now();
        state.preview_error = false;
    }
    if let Some(pending) = &state.pending {
        match pending.rx.try_recv() {
            Ok(maps) => {
                if pending.cfg == cfg {
                    state.textures = maps.images.iter().enumerate().map(|(i, im)|
                        ui.ctx().load_texture(format!("planet-editor-{i}"), im.clone(), TextureOptions::NEAREST)).collect();
                    state.shown = Some(cfg); state.maps = Some(maps);
                }
                state.pending = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                state.preview_error = pending.cfg == cfg;
                state.pending = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    if state.pending.is_none() && state.shown != Some(cfg) && state.changed_at.elapsed() >= Duration::from_millis(400) && !state.preview_error {
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new().name("planet-editor".into()).spawn(move || { let _ = tx.send(maps(cfg)); }) {
            Ok(_) => state.pending = Some(Pending { cfg, rx }),
            Err(_) => state.preview_error = true,
        }
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Continent").strong());
        for (i, label) in ["Terrain", "Height", "Temperature", "Rainfall"].into_iter().enumerate() {
            ui.selectable_value(&mut state.layer, i, label);
        }
    });
    // A previous recipe's map is hidden while the current recipe solves.
    let side = ui.available_width().min(360.0);
    if state.shown == Some(cfg) && state.textures.len() == 4 {
        ui.add(egui::Image::new(&state.textures[state.layer]).fit_to_exact_size(vec2(side, side)));
        if let Some(maps) = &state.maps {
            ui.label(format!("Land {}% · Peak {:.0} m", maps.land, maps.peak));
        }
        ui.small(match state.layer {
            1 => "Sea · Lowlands · Highlands",
            2 => "Blue −20°C · Green 10°C · Red 40°C",
            3 => "Ochre 0% · Green 50% · Blue 100%",
            _ => "Ocean · Lakes · Rivers · Land",
        });
    } else {
        let (r, _) = ui.allocate_exact_size(vec2(side, side), egui::Sense::hover());
        ui.painter().rect_filled(r, 0, super::themes::current(ui.ctx()).recess);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
            if state.preview_error { "Preview unavailable" } else { "Generating map…" }, egui::FontId::proportional(16.0), super::themes::current(ui.ctx()).muted());
        if state.preview_error && ui.button("Retry preview").clicked() { state.preview_error = false; }
    }
    if state.shown != Some(cfg) && !state.preview_error {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
fn maps(cfg: WorldSampleConfig) -> Maps {
    let p = engine_worldgen::geo::planet_for(&cfg);
    let n = p.grid.n;
    let side = n.min(256);
    let mut rgb: [Vec<u8>; 4] = std::array::from_fn(|_| Vec::with_capacity(side * side * 3));
    let mut land = 0usize;
    let mut peak = 0.0_f32;
    for y in 0..side { for x in 0..side {
        let (gx, gy) = (x * n / side, y * n / side);
        let i = gy * n + gx;
        let h = p.height[i];
        let water = h <= engine_voxel::WATER_LEVEL_METERS || p.lake[i] > h + 0.5;
        if !water { land += 1; }
        peak = peak.max(h - engine_voxel::WATER_LEVEL_METERS);
        let wet = p.wet[i];
        let temp = p.maat[i];
        let water_rgb = mix([58.0, 108.0, 139.0], [18.0, 45.0, 80.0], (p.lake[i].max(12.0) - h) / 58.0);
        let forest = cfg.planet_settings().map_or(1.0, |s| (s.unit(7) * 2.0) as f32);
        let green = mix([100.0, 138.0, 78.0], [40.0, 88.0, 57.0], wet * forest);
        let base = if water { water_rgb }
            else if p.discharge[i] > 2_500_000.0 { [92.0, 170.0, 199.0] }
            else if temp < -5.0 { [202.0, 218.0, 218.0] }
            else { mix(mix([176.0, 145.0, 88.0], green, wet), [171.0, 164.0, 147.0], ((h - 150.0) / 330.0).clamp(0.0, 1.0)) };
        let neighbor = p.height[gy.saturating_sub(1) * n + gx.saturating_sub(1)];
        let shade = if water { 1.0 } else { (0.86 + (neighbor - h) * 0.015).clamp(0.5, 1.25) };
        let heat = if temp < 10.0 { mix([60.0, 108.0, 193.0], [125.0, 173.0, 92.0], (temp + 20.0) / 30.0) }
            else { mix([125.0, 173.0, 92.0], [204.0, 78.0, 52.0], (temp - 10.0) / 30.0) };
        let rain = if wet < 0.5 { mix([180.0, 142.0, 73.0], [73.0, 151.0, 87.0], wet * 2.0) }
            else { mix([73.0, 151.0, 87.0], [68.0, 142.0, 207.0], wet * 2.0 - 1.0) };
        let elevation = if water { water_rgb } else { mix([62.0, 100.0, 68.0], [239.0, 226.0, 195.0], (h - 12.0) / 480.0) };
        for (j, color) in [base.map(|v| v * shade), elevation, heat, rain].into_iter().enumerate() {
            rgb[j].extend(color.map(|v| v.clamp(0.0, 255.0) as u8));
        }
    }}
    Maps { images: rgb.map(|bytes| ColorImage::from_rgb([side, side], &bytes)),
        land: (land * 100 / (side * side)) as u32, peak }
}
