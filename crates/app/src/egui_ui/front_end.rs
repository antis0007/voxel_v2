//! Painted player-facing screens and home navigation.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum HomePage {
    #[default]
    Title,
    Library,
    Create,
    Online,
    Preferences,
}
pub(super) fn home(
    root: &mut egui::Ui,
    owner: &mut EguiUi,
    world: &mut WorldType,
    scale: &mut VoxelScalePreset,
    seed: &mut u32,
    render: &mut RenderUiState,
) -> UiAction {
    let ctx = owner.ctx.clone();
    if owner.home_page == HomePage::Title {
        let recent = owner.library_entries.iter().filter(|e|e.saved > 0).max_by_key(|e|e.saved);
        let summary = recent.map(super::world_browser::summary);
        return match super::main_menu::draw(root, recent.map(|e|e.name.as_str()).zip(summary.as_deref()), owner.library_loading,
            owner.library_status.as_deref().or(owner.persistence_status.as_deref())) {
            super::main_menu::Intent::Continue => recent.map_or(UiAction::None,|e|UiAction::OpenWorld(e.id.clone())),
            super::main_menu::Intent::Load => { owner.home_page = HomePage::Library; UiAction::None },
            super::main_menu::Intent::New => {
                *world = WorldType::Planet;
                owner.home_page = HomePage::Create;
                UiAction::None
            },
            super::main_menu::Intent::Online => { owner.home_page = HomePage::Online; UiAction::None },
            super::main_menu::Intent::Settings => { owner.home_page = HomePage::Preferences; UiAction::None },
            super::main_menu::Intent::Quit => UiAction::Quit,
            super::main_menu::Intent::None => UiAction::None,
        };
    }
    backdrop(&ctx);
    let mut result = UiAction::None;
    if let Some(page) = home_nav(root, &ctx, owner.home_page, &mut result) {
        if page == HomePage::Create && owner.home_page != page {
            *world = WorldType::Planet;
        }
        owner.home_page = page;
    }
    let screen_action = match owner.home_page {
        HomePage::Title => UiAction::None,
        HomePage::Library => super::world_browser::draw(root, owner),
        HomePage::Online => super::online_browser::page(root, owner),
        HomePage::Create => new_world(
            root,
            world,
            scale,
            seed,
            render,
            owner.persistence_status.as_deref(),
            &mut owner.new_world_name,
            &mut owner.new_world_difficulty,
            &mut owner.new_world_mode,
            &mut owner.new_world_systems,
            &mut owner.planet_editor,
            owner.library_loading,
        ),
        HomePage::Preferences => {
            settings_page(root, &mut SettingsFields {
                fov_degrees: &mut owner.fov_degrees,
                mouse_sensitivity: &mut owner.mouse_sensitivity,
                invert_mouse: &mut owner.invert_mouse,
                show_hud: &mut owner.show_hud,
                ui_scale: &mut owner.ui_scale,
                hotbar_scale: &mut owner.hotbar_scale,
                offhand_right: &mut owner.offhand_right,
            }, render, scale.meters(), owner.preferences_status.as_deref());
            UiAction::None
        }
    };
    if matches!(result, UiAction::None) {
        screen_action
    } else {
        result
    }
}

/// Navigation strip over the home pages; returns the page picked this frame.
fn home_nav(root: &mut egui::Ui, ctx: &egui::Context, current: HomePage, result: &mut UiAction) -> Option<HomePage> {
    let mut picked = None;
    egui::Panel::top("home_navigation")
        .frame(
            egui::Frame::new()
                .fill(super::themes::current(ctx).recess)
                .inner_margin(egui::Margin::symmetric(20, 10)),
        )
        .show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (page, label) in [
                    (HomePage::Title, "Main Menu"),
                    (HomePage::Library, "Worlds"),
                    (HomePage::Create, "New World"),
                    (HomePage::Online, "Multiplayer"),
                    (HomePage::Preferences, "Settings"),
                ] {
                    if tab(ui, label, current == page).clicked() { picked = Some(page); }
                }
                if tab(ui, "Quit", false).clicked() {
                    *result = UiAction::Quit;
                }
            });
        });
    picked
}
/// Title-menu settings page: Audio, Controls, Graphics and Theme tabs.
fn settings_page(root: &mut egui::Ui, f: &mut SettingsFields, render: &mut RenderUiState, scale_m: f32, status: Option<&str>) {
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width((ui.available_width() - 32.0).clamp(220.0, 720.0));
                surface(ui.ctx()).show(ui, |ui| {
                    heading(ui, "Settings", "");
                    settings_tabs(ui, f, render, scale_m);
                    if let Some(status) = status {
                        ui.label(RichText::new(status).small());
                    }
                });
            });
        });
    });
}
/// Offline fixture for the menu exporter: a home page over the dimmed vista with default state.
pub(super) fn preview(root: &mut egui::Ui, page: &str, settings_tab: u8) {
    let ctx = root.ctx().clone();
    backdrop(&ctx);
    let current = match page { "new-world" => HomePage::Create, p if p.starts_with("settings") => HomePage::Preferences, _ => HomePage::Online };
    home_nav(root, &ctx, current, &mut UiAction::None);
    let mut render = RenderUiState {
        fxaa_enabled: true,
        vsync: true,
        settings: crate::render::render_distance_settings(),
        debug_view: RenderDebugView::default(),
        atmosphere_mode: AtmosphereMode::default(),
    };
    match current {
        HomePage::Create => {
            let mut world = WorldType::Planet;
            let mut scale = VoxelScalePreset::OneMeter;
            let mut seed = 1;
            let mut name = String::from("New world");
            let mut difficulty = Default::default();
            let mut mode = Default::default();
            let mut systems = Default::default();
            let mut planet = Default::default();
            let _ = new_world(root, &mut world, &mut scale, &mut seed, &mut render, None, &mut name,
                &mut difficulty, &mut mode, &mut systems, &mut planet, false);
        }
        HomePage::Online => super::online_browser::preview(root),
        _ => {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("home_settings_tab"), settings_tab));
            let (mut fov, mut sens, mut invert, mut hud, mut ui_scale, mut hotbar, mut offhand) =
                (70.0, 0.0025, false, true, crate::preferences::UI_SCALE_DEFAULT, 1.0, false);
            settings_page(root, &mut SettingsFields {
                fov_degrees: &mut fov, mouse_sensitivity: &mut sens, invert_mouse: &mut invert,
                show_hud: &mut hud, ui_scale: &mut ui_scale, hotbar_scale: &mut hotbar,
                offhand_right: &mut offhand,
            }, &mut render, scale_m(), None);
        }
    }
}
fn scale_m() -> f32 { VoxelScalePreset::OneMeter.meters() }
use super::*;
use egui::{FontId, Rect, Response, Sense, Stroke, StrokeKind, pos2, vec2};

const INK: Color32 = presentation::INK;
const PAPER: Color32 = presentation::PAPER;
const MINT: Color32 = presentation::JADE;
const MUTED: Color32 = presentation::MUTED;
const GOLD: Color32 = presentation::AMBER;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum PausePage {
    #[default]
    Journey,
    Journal,
    Log,
    Graphics,
    Controls,
    Environment,
    Online,
}
pub(super) fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(13.0).color(super::themes::current(ui.ctx()).accent).strong());
}
pub(super) fn heading(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.heading(RichText::new(title).size(24.0).color(PAPER));
    if !detail.is_empty() { ui.label(RichText::new(detail).color(MUTED)); }
    ui.add_space(12.0);
}
pub(super) fn action(ui: &mut egui::Ui, text: &str, primary: bool) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width().max(80.0), 34.0), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), text));
    let active = response.hovered() || response.has_focus();
    let pressed = response.is_pointer_button_down_on();
    let feedback = super::motion::feedback(&response, primary, ui.is_enabled());
    let palette = super::themes::current(ui.ctx());
    let fill = if ui.is_enabled() {
        palette.control(feedback)
    } else {
        super::themes::current(ui.ctx()).recess
    };
    ui.painter().rect_filled(rect, 0, fill);
    if ui.is_enabled() {
        super::prism::edge(ui.painter(), rect,
            (0.3 + feedback.hover * 0.55) * (1.0 - feedback.press * 0.4), false);
    }
    // A shallow stone bevel gives controls a consistent raised/pressed state.
    let themed_edge = if super::themes::get(ui.ctx()) == 0 {
        Color32::from_rgb(137, 113, 85)
    } else { super::themes::current(ui.ctx()).edge };
    let light_edge = if pressed {
        INK
    } else {
        themed_edge
    };
    let dark_edge = if pressed {
        themed_edge
    } else {
        INK
    };
    ui.painter().line_segment(
        [rect.left_bottom(), rect.left_top()],
        Stroke::new(2.0, light_edge),
    );
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(2.0, light_edge),
    );
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(2.0, dark_edge),
    );
    ui.painter().line_segment(
        [rect.right_bottom(), rect.left_bottom()],
        Stroke::new(2.0, dark_edge),
    );
    if primary {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min + vec2(6.0, 10.0), vec2(3.0, rect.height() - 20.0)),
            0,
            super::themes::current(ui.ctx()).accent,
        );
    }
    if active {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            0,
            Stroke::new(1.0, super::themes::current(ui.ctx()).accent),
            StrokeKind::Inside,
        );
    }
    let color = if !ui.is_enabled() { MUTED } else { PAPER };
    let font = FontId::proportional(16.0);
    let label = ui
        .painter()
        .layout_no_wrap(text.into(), font.clone(), color);
    let clip = Rect::from_min_max(rect.min + vec2(18.0, 0.0), rect.max - vec2(36.0, 0.0));
    ui.painter().with_clip_rect(clip).galley(
        pos2(rect.left() + 18.0, rect.center().y - label.size().y / 2.0),
        label,
        color,
    );
    ui.painter().text(
        pos2(rect.right() - 20.0, rect.center().y),
        Align2::CENTER_CENTER,
        "›",
        font,
        color,
    );
    response
}
pub(super) fn surface(ctx: &egui::Context) -> super::prism::LitFrame {
    let p = super::themes::current(ctx);
    let original = super::themes::get(ctx) == 0;
    super::prism::LitFrame(egui::Frame::new()
        .fill(if original { INK } else { p.panel })
        .stroke(Stroke::new(1.0, if original { presentation::EDGE } else { p.edge }))
        .corner_radius(0)
        .inner_margin(egui::Margin::same(16)))
}
fn pause_surface(ctx: &egui::Context) -> super::prism::LitFrame {
    let p = super::themes::current(ctx);
    let original = super::themes::get(ctx) == 0;
    super::prism::LitFrame(egui::Frame::new()
        .fill(if original { INK } else { p.panel })
        .stroke(Stroke::new(1.0, if original { presentation::EDGE } else { p.edge }))
        .corner_radius(egui::CornerRadius::ZERO)
        .inner_margin(egui::Margin::same(18)))
}
pub(super) fn landscape(p: &egui::Painter, r: Rect, seed: u32, kind: WorldType) {
    if kind == WorldType::Manyfold {
        // Looking down the tube: nested depth rings shrink to a vanishing core,
        // the wall wraps with terrain, and sparks drift along the axis.
        p.rect_filled(r, 0, Color32::from_rgb(14, 19, 26));
        let radius = r.height().min(r.width()) * 0.46;
        let tile = (radius * 0.10).max(2.0);
        let time = p.ctx().input(|i| i.time as f32);
        let c = r.center();
        for (ring, depth) in [1.0_f32, 0.78, 0.58, 0.40, 0.25, 0.13].into_iter().enumerate() {
            let rad = radius * depth;
            let n = (72.0 * depth).max(12.0) as usize;
            let t = (tile * depth).max(1.0);
            for i in 0..n {
                let a = i as f32 * std::f32::consts::TAU / n as f32 + ring as f32 * 0.21;
                let lower = a.sin() > 0.0;
                let base = if lower { [133.0, 126.0, 104.0] } else { [101.0, 128.0, 76.0] };
                let shade = 0.35 + 0.65 * depth;
                let glow = if i % 9 == 0 { 1.6 } else { 1.0 };
                let col = Color32::from_rgb(
                    (base[0] * shade * glow).min(255.0) as u8,
                    (base[1] * shade * glow).min(255.0) as u8,
                    (base[2] * shade * glow).min(255.0) as u8);
                let q = c + vec2(a.cos(), a.sin()) * rad;
                p.rect_filled(Rect::from_center_size(q, vec2(t, t)), 0, col);
            }
        }
        p.rect_filled(Rect::from_center_size(c, vec2(tile * 0.8, tile * 0.8)), 0, Color32::from_rgb(250, 226, 160));
        for k in 0..18 {
            let age = (time * 0.25 + k as f32 * 0.137).fract();
            let a = k as f32 * 2.399;
            let q = c + vec2(a.cos(), a.sin()) * radius * (0.12 + 0.8 * age);
            let s = (tile * 0.5 * (0.4 + age)).max(1.0);
            p.rect_filled(Rect::from_center_size(q, vec2(s, s)), 0,
                Color32::from_rgb(237, 188, 116).gamma_multiply(1.0 - age * 0.7));
        }
        p.ctx().request_repaint();
        return;
    }
    if kind == WorldType::Hall {
        p.rect_filled(r, 0, Color32::from_rgb(18, 10, 16));
        let y = r.top() + r.height() * 0.62;
        p.rect_filled(Rect::from_min_size(pos2(r.left() + r.width() * 0.18, y), vec2(r.width() * 0.64, r.height() * 0.08)), 0, Color32::from_rgb(214, 206, 190));
        p.rect_filled(Rect::from_min_size(pos2(r.left() + r.width() * 0.46, r.top() + r.height() * 0.28), vec2(r.width() * 0.08, r.height() * 0.34)), 0, Color32::from_rgb(168, 96, 48));
        return;
    }
    let tile = (r.width() / 144.0).min(r.height() / 81.0).floor().max(1.0);
    let top = Color32::from_rgb(66, 86, 105);
    let mid = Color32::from_rgb(104, 112, 116);
    p.rect_filled(r, 0, top);
    for band in 0..12 {
        let y = r.top() + tile * band as f32;
        p.rect_filled(
            Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), tile)),
            0,
            if band % 3 == 0 { mid } else { top },
        );
    }
    let sun = pos2(r.left() + r.width() * 0.76, r.top() + r.height() * 0.24);
    p.rect_filled(
        Rect::from_center_size(sun, vec2(tile * 8.0, tile * 8.0)),
        0,
        Color32::from_rgb(237, 188, 116),
    );
    p.rect_filled(
        Rect::from_center_size(sun, vec2(tile * 10.0, tile * 4.0)),
        0,
        Color32::from_rgb(250, 220, 158),
    );
    p.rect_filled(
        Rect::from_center_size(sun, vec2(tile * 4.0, tile * 10.0)),
        0,
        Color32::from_rgb(250, 220, 158),
    );
    let layers = [
        Color32::from_rgb(101, 107, 92),
        Color32::from_rgb(76, 82, 65),
        Color32::from_rgb(49, 58, 48),
        Color32::from_rgb(37, 43, 39),
    ];
    for (layer, color) in layers.into_iter().enumerate() {
        let step = tile * 2.0;
        let columns = (r.width() / step).ceil() as usize;
        let baseline = r.height() * (0.53 + layer as f32 * 0.105);
        for x in 0..columns {
            let segment = x / 8;
            let frac = (x % 8) as f32 / 8.0;
            let hash = |n: usize| {
                let mut h = seed
                    .wrapping_add((layer as u32 + 1).wrapping_mul(0x9E37_79B9))
                    .wrapping_add((n as u32).wrapping_mul(0x85EB_CA6B));
                h ^= h >> 16;
                h = h.wrapping_mul(0x7FEB_352D);
                h ^= h >> 15;
                (h as f32 / u32::MAX as f32 - 0.5) * r.height() * 0.085
            };
            let mut height = baseline + hash(segment) * (1.0 - frac) + hash(segment + 1) * frac;
            if kind == WorldType::Superflat {
                height = baseline;
            }
            height = (height / tile).round() * tile;
            let left = r.left() + x as f32 * step;
            let y = (r.top() + height).clamp(r.top() + r.height() * 0.34, r.bottom());
            p.rect_filled(
                Rect::from_min_max(
                    pos2(left, y),
                    pos2((left + step).min(r.right()), r.bottom()),
                ),
                0,
                color,
            );
            // A few blocky treetops anchor the near ridge in voxel terrain.
            if layer == 3 && x % 13 == 5 && y < r.bottom() - tile * 7.0 {
                let trunk = Rect::from_min_size(
                    pos2(left + tile * 0.8, y - tile * 2.0),
                    vec2(tile * 0.6, tile * 2.0),
                );
                p.rect_filled(trunk, 0, Color32::from_rgb(93, 66, 49));
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(left - tile, y - tile * 5.0),
                        pos2(left + tile * 3.0, y - tile),
                    ),
                    0,
                    Color32::from_rgb(89, 105, 67),
                );
            }
        }
    }
}

/// Every home page stands in front of the title vista, dimmed evenly so
/// panels and text keep their contrast; leaving the title is not a cut to a
/// flat fill.
pub(super) fn backdrop(ctx: &egui::Context) {
    let p = ctx.layer_painter(egui::LayerId::background());
    let r = ctx.content_rect();
    p.rect_filled(r, 0, super::themes::current(ctx).recess);
    super::menu_scene::paint(&p, r, -1.0, 0.42);
}
fn world_card(
    ui: &mut egui::Ui,
    kind: WorldType,
    selected: bool,
    seed: u32,
    title: &str,
    detail: &str,
) -> Response {
    let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 82.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), title)
    });
    let palette = super::themes::current(ui.ctx());
    let feedback = super::motion::feedback(&response, selected, ui.is_enabled());
    ui.painter().rect_filled(
        r,
        0,
        palette.control(feedback),
    );
    ui.painter().rect_stroke(
        r,
        0,
        Stroke::new(
            1.0 + feedback.selection,
            palette.control_edge(feedback),
        ),
        StrokeKind::Inside,
    );
    let art = Rect::from_min_size(r.min + vec2(8.0, 8.0), vec2(66.0, 66.0));
    landscape(&ui.painter().with_clip_rect(art), art, seed, kind);
    let x = r.left() + 88.0;
    ui.painter().text(
        pos2(x, r.top() + 14.0),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(16.0),
        PAPER,
    );
    let galley = ui.painter().layout(
        detail.into(),
        FontId::proportional(12.0),
        MUTED,
        (r.width() - 108.0).max(40.0),
    );
    ui.painter()
        .with_clip_rect(r.shrink(6.0))
        .galley(pos2(x, r.top() + 38.0), galley, MUTED);
    response
}
fn new_world(
    root: &mut egui::Ui,
    world: &mut WorldType,
    scale: &mut VoxelScalePreset,
    seed: &mut u32,
    render: &mut RenderUiState,
    status: Option<&str>,
    name: &mut String,
    difficulty: &mut crate::world::adventure::difficulty::Difficulty,
    mode: &mut crate::world::rules::GameMode,
    systems: &mut crate::world::systems::Systems,
    planet: &mut super::planet_editor::State,
    loading: bool,
) -> UiAction {
    let mut result = UiAction::None;
    egui::Panel::bottom("world_create_action").show(root, |ui| {
        ui.add_enabled_ui(!loading && !name.trim().is_empty(), |ui| {
            if action(ui, "Create world", true).clicked() {
                result = UiAction::StartWorld {
                    name: name.trim().to_owned(), reference: false, world_type: *world,
                    voxel_scale: *scale, seed: *seed, difficulty: *difficulty, mode: *mode, systems: *systems,
                    planet: (*world == WorldType::Planet).then_some(planet.settings),
                };
            }
        });
    });
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(root,|ui| {
        let wide=ui.available_width()>1000.0;
        egui::ScrollArea::vertical().id_salt("world_front_end").show(ui,|ui| {
            ui.add_space(if wide{36.0}else{16.0});
            let width=(ui.available_width()-32.0).clamp(200.0,560.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                surface(ui.ctx()).show(ui,|ui| {
                    let mut form=|ui:&mut egui::Ui,world:&mut WorldType,scale:&mut VoxelScalePreset,seed:&mut u32,render:&mut RenderUiState| {
                        heading(ui,"New world","");
                        ui.add(egui::TextEdit::singleline(name).hint_text("World name").char_limit(64).desired_width(f32::INFINITY));
                        let kinds = [
                            (WorldType::Planet,"Planet","Continents, climate and caves"),
                            (WorldType::Superflat,"Flat","Flat building ground."),
                            (WorldType::Manyfold,"Manyfold","Shell maze"),
                            (WorldType::Hall,"Hall","Hanging precinct · two wardens"),
                        ];
                        ui.horizontal_wrapped(|ui| {
                            let selected_world = *world;
                            for (kind, title, _) in kinds.into_iter().filter(|k| !matches!(k.0, WorldType::Manyfold | WorldType::Hall) || selected_world == k.0) {
                                if tab(ui, title, *world == kind).clicked() { *world = kind; }
                            }
                        });
                        if let Some((kind, title, detail)) = kinds.into_iter().find(|(kind, _, _)| *world == *kind) {
                            world_card(ui, kind, true, *seed, title, detail);
                        }
                        ui.add_space(6.0);caption(ui,"DIFFICULTY");
                        ui.horizontal_wrapped(|ui| {
                            for level in crate::world::adventure::difficulty::Difficulty::ALL {
                                let response=tab(ui,level.label(),*difficulty==level).on_hover_text(level.hint());
                                if response.clicked(){*difficulty=level;}
                            }
                        });
                        ui.add_space(6.0);caption(ui,"GAME MODE");
                        ui.horizontal_wrapped(|ui| {
                            for choice in crate::world::rules::GameMode::ALL {
                                if tab(ui,choice.label(),*mode==choice).on_hover_text(choice.hint()).clicked(){*mode=choice;}
                            }
                        });
                        ui.collapsing("Advanced",|ui| {
                            caption(ui,"WORLD TYPE");
                            ui.horizontal_wrapped(|ui| {
                                for (kind, title, _) in kinds {
                                    if tab(ui, title, *world == kind).clicked() { *world = kind; }
                                }
                            });
                            ui.add_space(8.0);caption(ui,"BLOCK SIZE");
                            let old=scale.meters();
                            ui.horizontal_wrapped(|ui| {
                                for (preset,label) in [(VoxelScalePreset::OneMeter,"Classic · 1 m"),(VoxelScalePreset::HalfMeter,"Fine · 0.5 m"),(VoxelScalePreset::QuarterMeter,"Micro · 0.25 m")] {
                                    let response=tab(ui,label,*scale==preset);
                                    if response.clicked(){*scale=preset;}
                                }
                            });
                            if old!=scale.meters(){render.settings=render.settings.rescale_blocks(old,scale.meters());}
                            ui.small("Smaller blocks need more memory.");
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("World seed").color(MUTED));
                            ui.add(egui::DragValue::new(seed).range(0..=u32::MAX).speed(1.0));
                            if ui.small_button("Shuffle").clicked(){*seed=next_seed(*seed);}
                        });
                        if *world == WorldType::Planet {
                            ui.add_space(8.0);
                            super::planet_editor::draw(ui, planet, seed, *scale);
                        }
                            ui.add_space(6.0);
                        ui.add_space(6.0);caption(ui,"SYSTEMS");
                        ui.horizontal_wrapped(|ui| {
                            for def in crate::world::systems::catalog() {
                                let on=systems.enabled(&def.id);
                                if tab(ui,&def.label,on).on_hover_text(&def.hint).clicked(){systems.set(&def.id,!on);}
                            }
                        });
                        let off=systems.off_ids().len();
                        ui.small(if off==0 {"Every System On".to_string()} else {format!("{off} Off · Fixed at Creation")});
                        });
                        ui.add_space(12.0);
                        if let Some(status)=status {ui.label(RichText::new(status).small().color(MUTED));}
                        ui.collapsing("Display & terrain quality",|ui|settings::graphics_settings(ui,render,scale.meters()));
                    };
                    form(ui,world,scale,seed,render);
                });
            });
            ui.add_space(24.0);
        });
    });
    result
}
/// Player preferences that the title and pause settings pages edit in place.
pub(super) struct SettingsFields<'a> {
    fov_degrees: &'a mut f32,
    mouse_sensitivity: &'a mut f32,
    invert_mouse: &'a mut bool,
    show_hud: &'a mut bool,
    ui_scale: &'a mut f32,
    hotbar_scale: &'a mut f32,
    offhand_right: &'a mut bool,
}
/// Audio, Controls, Graphics and Theme tabs; the same panels the in-game settings use.
fn settings_tabs(ui: &mut egui::Ui, f: &mut SettingsFields, render: &mut RenderUiState, scale_m: f32) {
    let tab_id = egui::Id::new("home_settings_tab");
    let mut current: u8 = ui.ctx().data_mut(|d| d.get_temp(tab_id).unwrap_or(0));
    ui.horizontal_wrapped(|ui| {
        for (i, label) in ["Audio", "Controls", "Graphics", "Theme"].into_iter().enumerate() {
            if tab(ui, label, current == i as u8).clicked() { current = i as u8; }
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(tab_id, current));
    ui.add_space(8.0);
    match current {
        0 => settings::audio_settings(ui),
        1 => {
            ui.add(
                egui::Slider::new(&mut *f.fov_degrees, 50.0..=110.0)
                    .text("Field of view"),
            );
            ui.add(
                egui::Slider::new(&mut *f.mouse_sensitivity, 0.0005..=0.006)
                    .logarithmic(true)
                    .text("Mouse sensitivity"),
            );
            ui.checkbox(&mut *f.invert_mouse, "Invert vertical look");
            super::controls::comfort(ui);
            ui.add_space(8.0);
            caption(ui, "KEY BINDINGS");
            super::controls::bindings(ui);
        }
        2 => {
            settings::graphics_settings(ui, render, scale_m);
            ui.checkbox(&mut *f.show_hud, "Show the in-game HUD");
            super::magic_fx::motion_setting(ui);
            super::typography::controls(ui, &mut *f.ui_scale);
            ui.add(
                egui::Slider::new(&mut *f.hotbar_scale, 0.7..=1.6)
                    .step_by(0.05)
                    .text("Hotbar size"),
            );
            ui.checkbox(&mut *f.offhand_right, "Offhand on right");
        }
        _ => {
            super::themes::controls(ui);
            ui.separator();
            settings::saving_settings(ui);
        }
    }
}
pub(super) fn journal(
    ui: &mut egui::Ui,
    expedition: Option<&crate::world::expedition::Expedition>,
    adventure: &crate::world::adventure::View,
) {
    heading(ui, "Quest Log", "");
    let Some(e) = expedition else {
        ui.label("No world loaded.");
        return;
    };
    super::quest_log::draw(ui, false);
    ui.add_space(12.0);
    super::relics::expedition_goal(ui, adventure);
    if e.lamp {
        ui.label(format!(
            "Lumen Lantern: {} · [{}]",
            if e.lamp_enabled() { "On" } else { "Off" },
            crate::keybinds::hint(crate::keybinds::Action::Lamp)
        ));
    }
    caption(ui, "WORKSHOP PREPARATION");
    if !e.objective().is_empty() {
        ui.label(RichText::new(e.objective()).size(17.0).color(MINT));
    }
    ui.add_space(12.0);
    for (name, complete) in [
        ("Discover the ruined workshop", e.workshop_found),
        ("Find a lumen lantern", e.lamp),
        ("Assemble a bronze bore", e.tool_tier >= 1),
        ("Craft a resonant bore", e.tool_tier >= 2),
    ] {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 31.0), Sense::hover());
        ui.painter().circle_stroke(
            r.left_center() + vec2(9.0, 0.0),
            5.0,
            Stroke::new(1.5, if complete { MINT } else { MUTED }),
        );
        if complete {
            ui.painter()
                .circle_filled(r.left_center() + vec2(9.0, 0.0), 2.7, MINT);
        }
        ui.painter().text(
            r.left_center() + vec2(26.0, 0.0),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(14.0),
            if complete { MUTED } else { PAPER },
        );
    }
    ui.add_space(14.0);
    caption(ui, "SUPPLIES");
    let mut any = false;
    ui.horizontal_wrapped(|ui| {
        for (i, name) in crate::world::expedition::NAMES.iter().enumerate() {
            let amount = e.resources[i];
            if amount == 0 {
                continue;
            }
            any = true;
            egui::Frame::new()
                .fill(Color32::from_rgb(49, 41, 39))
                .corner_radius(0)
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_min_width(115.0);
                    ui.label(
                        RichText::new(format!("{:.2}", amount as f32 / 64.0))
                            .size(22.0)
                            .color(GOLD),
                    );
                    ui.label(RichText::new(*name).small().color(MUTED));
                });
        }
    });
    if !any {
        ui.label("No supplies.");
    }
}
fn base_protection(ui: &mut egui::Ui, world: &mut crate::world::World, player: &crate::player::Player) {
    use crate::world::{farming::base::Bounds, interaction::REACH, raycast_block};

    ui.collapsing("Base Protection", |ui| {
        let scale = world.voxel_scale.meters();
        let target = raycast_block(world, player.eye(world.voxel_scale), player.forward_dir(), REACH / scale)
            .map(|(cell, _)| cell);
        let base = &mut world.farming.base;
        if let Some(bounds) = base.bounds {
            let size = bounds.size().as_vec3() * scale;
            ui.label(format!("{:.1} × {:.1} × {:.1} m", size.x, size.y, size.z));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(target.is_some(), egui::Button::new("First Corner")).clicked() {
                base.first_corner = target;
            }
            let bounds = base.first_corner.zip(target).map(|(a, b)| Bounds::new(a, b))
                .filter(|bounds| bounds.valid());
            if ui.add_enabled(bounds.is_some(), egui::Button::new("Opposite Corner")).clicked() {
                base.bounds = bounds;
                base.first_corner = None;
            }
            if ui.add_enabled(base.bounds.is_some() || base.first_corner.is_some(), egui::Button::new("Clear Bounds")).clicked() {
                base.bounds = None;
                base.first_corner = None;
            }
        });
        if let Some(p) = base.first_corner {
            ui.weak(format!("First Corner · {}, {}, {}", p.x, p.y, p.z));
        }
    });
}

pub(super) fn pause(
    owner: &mut EguiUi,
    time: &mut TimeUiState,
    physics: &mut PhysicsUiState,
    render: &mut RenderUiState,
    world: Option<&mut crate::world::World>,
    player: &crate::player::Player,
) -> UiAction {
    let ctx = owner.ctx.clone();
    let mut result = UiAction::None;
    let screen = ctx.content_rect();
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen,
        0,
        Color32::from_rgba_unmultiplied(16, 13, 20, 205),
    );
    // Width is the content width; reserve the frame margins as well.
    let width = (screen.width() - 76.0).clamp(1.0, 560.0);
    egui::Area::new(egui::Id::new("journey_pause"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .default_size(vec2(width, (screen.height() - 48.0).max(80.0)))
        .show(&ctx, |ui| {
            ui.set_width(width);
            // Areas otherwise inherit a short default height. A ScrollArea
            // clamps to that available height even with a larger max_height.
            ui.set_max_height((screen.height() - 48.0).max(80.0));
            pause_surface(ui.ctx()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Paused").size(24.0).strong().color(PAPER));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Esc  Resume").monospace().small().color(MUTED));
                    });
                });
                super::themes::quick_switch(ui);
                if let Some(status) = &owner.preferences_status {
                    ui.label(RichText::new(status).small().color(GOLD));
                }
                ui.horizontal_wrapped(|ui| {
                    for (page, label) in [
                        (PausePage::Journey, "Game"),
                        (PausePage::Journal, "Quests"),
                        (PausePage::Log, "Log"),
                        (PausePage::Graphics, "Display"),
                        (PausePage::Controls, "Controls"),
                        (PausePage::Environment, "World"),
                        (PausePage::Online, "Friends"),
                    ] {
                        if tab(ui, label, owner.pause_page == page).clicked() {
                            owner.pause_page = page;
                        }
                    }
                });
                ui.add_space(5.0);
                ui.separator();
                if let Some(error) = &owner.region_error {
                    ui.label(
                        RichText::new("World storage needs attention")
                            .color(GOLD)
                            .strong(),
                    );
                    ui.label(RichText::new(plain_storage_error(ui.ctx(), error)).small());
                }
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .id_salt(("pause_content", owner.pause_page as u8))
                    .max_height((screen.height() - (ui.cursor().top() - ui.min_rect().top()) - 76.0).max(48.0))
                    .show(ui, |ui| match owner.pause_page {
                        PausePage::Journey => {
                            if action(ui, "Resume", true).clicked() {
                                result = UiAction::Resume;
                            }
                            ui.add_space(6.0);
                            let mut rules = owner.world_rules;
                            if ui.checkbox(&mut rules.keep_inventory, "Keep inventory")
                                .on_hover_text("Keep carried items on future deaths").changed() {
                                result = UiAction::SetRules(rules);
                            }
                            if action(ui, "Save now", false).clicked() {
                                result = UiAction::SaveWorld;
                            }
                            if let Some(status) = &owner.persistence_status {
                                ui.label(RichText::new(status).small().color(MUTED));
                            }
                            if let Some(world) = world {
                                base_protection(ui, world, player);
                            }
                            ui.add_space(6.0);
                            if super::coop_panel::summary(ui, &owner.coop) {
                                owner.pause_page = PausePage::Online;
                            }
                            ui.add_space(10.0);
                            ui.separator();
                            if ui.available_width() >= 400.0 {
                                ui.columns(2, |columns| {
                                    if action(&mut columns[0], "World list", false).clicked() {
                                        result = UiAction::BackToWorldSelect;
                                    }
                                    if action(&mut columns[1], "Save and quit", false).clicked() {
                                        result = UiAction::Quit;
                                    }
                                });
                            } else {
                                if action(ui, "World list", false).clicked() { result = UiAction::BackToWorldSelect; }
                                if action(ui, "Save and quit", false).clicked() { result = UiAction::Quit; }
                            }
                            ui.label(RichText::new("Unsaved Changes").small().color(MUTED));
                        }
                        PausePage::Journal => journal(ui, owner.expedition.as_ref(), &owner.adventure),
                        PausePage::Log => super::event_log::page(ui, &owner.event_log),
                        PausePage::Graphics => {
                            heading(
                                ui,
                                "Display",
                                "",
                            );
                            super::themes::controls(ui);
                            ui.separator();
                            settings::graphics_settings(ui, render, physics.voxel_scale.meters());
                            super::magic_fx::motion_setting(ui);
                            ui.add(
                                egui::Slider::new(&mut owner.fov_degrees, 50.0..=110.0)
                                    .text("Field of view"),
                            );
                            super::typography::controls(ui, &mut owner.ui_scale);
                            ui.add(
                                egui::Slider::new(&mut owner.hotbar_scale, 0.7..=1.6)
                                    .step_by(0.05)
                                    .text("Hotbar size"),
                            );
                            ui.checkbox(&mut owner.offhand_right, "Offhand on right");
                            if let Some(status) = &owner.region_status {
                                ui.collapsing("Storage diagnostics", |ui| {
                                    ui.small(status);
                                });
                            }
                        }
                        PausePage::Controls => {
                            heading(ui, "Controls", "");
                            ui.label("Chat: [T] / [Enter] · Commands: [/] · Help: /help");
                            ui.add(
                                egui::Slider::new(&mut owner.mouse_sensitivity, 0.0005..=0.006)
                                    .logarithmic(true)
                                    .text("Mouse sensitivity"),
                            );
                            ui.checkbox(&mut owner.invert_mouse, "Invert vertical look");
                            super::controls::comfort(ui);
                            settings::audio_settings(ui);
                            presentation::controls(ui);
                            ui.separator();
                            super::controls::bindings(ui);
                        }
                        PausePage::Environment => {
                            heading(
                                ui,
                                "World",
                                "",
                            );
                            super::world_rules::draw(ui, owner.world_rules, &mut result);
                            settings::environment_settings(ui, time, physics, owner.world_difficulty, &mut result);
                        }
                        PausePage::Online => {
                            heading(ui, "Friends", "");
                            if let Some(request) = super::coop_panel::pause_page(ui, &mut owner.coop) {
                                result = UiAction::Coop(request);
                            }
                        }
                    });
            });
        });
    result
}
pub(super) fn journey_hud(
    ctx: &egui::Context,
    expedition: Option<&crate::world::expedition::Expedition>,
    reserved: Option<Rect>,
    eye: [f32; 3],
) {
    let reserve_id = egui::Id::new("hud_top_reserved");
    ctx.data_mut(|d| d.insert_temp(reserve_id, reserved));
    let Some(e) = expedition else { return };
    // Objectives belong in the journal. Only new interaction feedback needs
    // screen space during play.
    if e.message_seconds > 0.0 && !e.message.is_empty() {
        let width = (ctx.content_rect().width() - 56.0).clamp(100.0, 320.0);
        let left = ctx.content_rect().center().x - width * 0.5 - 10.0;
        let slot = super::hud_layout::place(ctx, "journey");
        let top = reserved
            .filter(|r| r.right() + 8.0 > left)
            .map_or(slot.offset.y, |r| (r.bottom() + 8.0 - ctx.content_rect().top()).max(slot.offset.y));
        let notice = egui::Area::new(egui::Id::new("journey_notification"))
            .anchor(slot.align, [slot.offset.x, top])
            .interactable(false)
            .show(ctx, |ui| {
                presentation::hud_frame(ui.ctx()).show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.label(RichText::new(&e.message).size(12.0).color(MINT));
                    // Detail lines live in the pause Log; the banner points at the event.
                    if e.at.is_some() || !e.detail.is_empty() {
                        let mut line = e.at.map(|at| super::event_log::bearing(eye, at)).unwrap_or_default();
                        if !e.detail.is_empty() {
                            if !line.is_empty() { line.push_str(" · "); }
                            line.push_str("[Esc] Log");
                        }
                        ui.label(RichText::new(line).size(11.0).color(MUTED));
                    }
                });
            });
        super::hud_layout::claim(ctx, "journey", notice.response.rect);
        let occupied = reserved.map_or(notice.response.rect, |r| r.union(notice.response.rect));
        ctx.data_mut(|d| d.insert_temp(reserve_id, Some(occupied)));
    }
}

pub(super) fn tab(ui: &mut egui::Ui, text: &str, selected: bool) -> Response {
    let palette = super::themes::current(ui.ctx());
    let font = FontId::proportional(14.0);
    let color = if selected { palette.accent } else { MUTED };
    let label = ui.painter().layout_no_wrap(text.into(), font, color);
    let (r, response) = ui.allocate_exact_size(vec2(label.size().x + 22.0, 32.0), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), text));
    let feedback = super::motion::feedback(&response, selected, ui.is_enabled());
    if feedback.selection.max(feedback.hover) > 0.001 {
        ui.painter()
            .rect_filled(r, 0, palette.control(feedback)
                .gamma_multiply(feedback.selection.max(feedback.hover)));
    }
    ui.painter()
        .galley(r.center() - label.size() * 0.5, label, color);
    if feedback.selection.max(feedback.hover) > 0.001 {
        let extent = (r.width() - 10.0) * feedback.selection.max(feedback.hover);
        ui.painter().line_segment(
            [
                pos2(r.center().x - extent * 0.5, r.bottom() - 1.0),
                pos2(r.center().x + extent * 0.5, r.bottom() - 1.0),
            ],
            Stroke::new(2.0, palette.accent),
        );
    }
    response
}
/// Plain sentence for a save failure. The raw engine text, with its OS error
/// code, goes to the log once per change instead of onto the screen.
fn plain_storage_error(ctx: &egui::Context, raw: &str) -> String {
    let id = egui::Id::new("storage_error_logged");
    if ctx.data(|d| d.get_temp::<String>(id)).as_deref() != Some(raw) {
        eprintln!("[save] {raw}");
        ctx.data_mut(|d| d.insert_temp(id, raw.to_owned()));
    }
    let lower = raw.to_lowercase();
    let cause = if ["os error 112)", "not enough space", "disk is full", "no space left"].iter().any(|k| lower.contains(k)) {
        "the disk is full"
    } else if ["os error 5)", "access is denied", "permission denied", "read-only"].iter().any(|k| lower.contains(k)) {
        "the save folder is read-only"
    } else if lower.contains("region write failed") || lower.contains("os error") {
        "the save folder cannot be written"
    } else {
        return raw.to_owned();
    };
    format!("Could not save: {cause}")
}

pub(super) fn storage_notice(ctx: &egui::Context, error: &str) {
    egui::Area::new(egui::Id::new("world_storage_notice"))
        .order(egui::Order::Foreground)
        .anchor(Align2::LEFT_TOP, [16.0, 72.0])
        .show(ctx, |ui| {
            presentation::hud_frame(ui.ctx()).show(ui, |ui| {
                ui.set_max_width((ctx.content_rect().width() - 48.0).clamp(180.0, 420.0));
                ui.label(RichText::new("Saving needs attention").strong().color(GOLD));
                ui.label(RichText::new(plain_storage_error(ctx, error)).small().color(PAPER));
                ui.label(
                    RichText::new("Unsaved Changes")
                        .small()
                        .color(MUTED),
                );
            });
        });
}
