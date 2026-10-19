//! Painted player-facing screens and home navigation.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum HomePage {
    #[default]
    Library,
    Create,
    Preferences,
}
pub(super) fn home(
    owner: &mut EguiUi,
    world: &mut WorldType,
    scale: &mut VoxelScalePreset,
    seed: &mut u32,
    render: &mut RenderUiState,
) -> UiAction {
    let ctx = owner.ctx.clone();
    backdrop(&ctx, *seed, *world);
    let mut result = UiAction::None;
    egui::TopBottomPanel::top("home_navigation")
        .frame(
            egui::Frame::new()
                .fill(INK)
                .inner_margin(egui::Margin::symmetric(20, 10)),
        )
        .show(&ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (page, label) in [
                    (HomePage::Library, "Your worlds"),
                    (HomePage::Create, "New world"),
                    (HomePage::Preferences, "Preferences"),
                ] {
                    if tab(ui, label, owner.home_page == page).clicked() {
                        owner.home_page = page;
                    }
                }
                if tab(ui, "Quit game", false).clicked() {
                    result = UiAction::Quit;
                }
            });
        });
    let screen_action = match owner.home_page {
        HomePage::Library => super::world_browser::draw(owner),
        HomePage::Create => new_world(
            &ctx,
            world,
            scale,
            seed,
            render,
            owner.persistence_status.as_deref(),
            &mut owner.new_world_name,
            owner.library_loading,
        ),
        HomePage::Preferences => {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(&ctx,|ui| {
                egui::ScrollArea::vertical().show(ui,|ui| {
                    ui.add_space(20.0);ui.vertical_centered(|ui| {
                        ui.set_max_width((ui.available_width()-32.0).clamp(220.0,720.0));
                        surface().show(ui,|ui| {
                            heading(ui,"Settings","Display, controls and autosave.");
                            settings::graphics_settings(ui,render,scale.meters());
                            ui.add(egui::Slider::new(&mut owner.fov_degrees,50.0..=110.0).text("Field of view"));
                            ui.add(egui::Slider::new(&mut owner.mouse_sensitivity,0.0005..=0.006).logarithmic(true).text("Mouse sensitivity"));
                            ui.checkbox(&mut owner.invert_mouse,"Invert vertical look");ui.checkbox(&mut owner.show_hud,"Show the in-game HUD");
                            settings::saving_settings(ui);
                            settings::audio_settings(ui);
                            if let Some(status)=&owner.preferences_status {ui.label(RichText::new(status).small());}
                            else {ui.small("Changes save automatically.");}
                        });
                    });
                });
            });
            UiAction::None
        }
    };
    if matches!(result, UiAction::None) {
        screen_action
    } else {
        result
    }
}

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
    Graphics,
    Controls,
    Environment,
}
pub(super) fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(11.0).color(GOLD).strong());
}
pub(super) fn heading(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.heading(RichText::new(title).size(24.0).color(PAPER));
    ui.label(RichText::new(detail).color(MUTED));
    ui.add_space(12.0);
}
pub(super) fn action(ui: &mut egui::Ui, text: &str, primary: bool) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width().max(80.0), 34.0), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), text));
    let active = response.hovered() || response.has_focus();
    let pressed = response.is_pointer_button_down_on();
    let fill = if primary {
        if active {
            Color32::from_rgb(108, 85, 57)
        } else {
            Color32::from_rgb(78, 60, 44)
        }
    } else if active {
        Color32::from_rgb(86, 72, 61)
    } else {
        Color32::from_rgb(54, 46, 44)
    };
    let fill = if ui.is_enabled() {
        fill
    } else {
        Color32::from_rgb(43, 39, 39)
    };
    ui.painter().rect_filled(rect, 0, fill);
    // A shallow stone bevel gives controls a consistent raised/pressed state.
    let light_edge = if pressed { INK } else { Color32::from_rgb(137, 113, 85) };
    let dark_edge = if pressed { Color32::from_rgb(137, 113, 85) } else { INK };
    ui.painter().line_segment([rect.left_bottom(), rect.left_top()], Stroke::new(2.0, light_edge));
    ui.painter().line_segment([rect.left_top(), rect.right_top()], Stroke::new(2.0, light_edge));
    ui.painter().line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(2.0, dark_edge));
    ui.painter().line_segment([rect.right_bottom(), rect.left_bottom()], Stroke::new(2.0, dark_edge));
    if primary {
        ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(6.0, 10.0), vec2(3.0, rect.height() - 20.0)), 0, GOLD);
    }
    if active {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            0,
            Stroke::new(1.0, GOLD),
            StrokeKind::Inside,
        );
    }
    let color = if !ui.is_enabled() {
        MUTED
    } else {
        PAPER
    };
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
pub(super) fn surface() -> egui::Frame {
    egui::Frame::new()
        .fill(INK)
        .stroke(Stroke::new(1.0, presentation::EDGE))
        .corner_radius(0)
        .inner_margin(egui::Margin::same(16))
}
fn pause_surface() -> egui::Frame {
    egui::Frame::new()
        .fill(INK)
        .stroke(Stroke::new(1.0, presentation::EDGE))
        .corner_radius(egui::CornerRadius::ZERO)
        .inner_margin(egui::Margin::same(18))
}
pub(super) fn landscape(p: &egui::Painter, r: Rect, seed: u32, kind: WorldType) {
    let tile=(r.width()/144.0).min(r.height()/81.0).floor().max(1.0);
    let top=Color32::from_rgb(66,86,105);
    let mid=Color32::from_rgb(104,112,116);
    p.rect_filled(r,0,top);
    for band in 0..12 {
        let y=r.top()+tile*band as f32;
        p.rect_filled(Rect::from_min_size(pos2(r.left(),y),vec2(r.width(),tile)),0,
            if band%3==0 {mid}else{top});
    }
    let sun=pos2(r.left()+r.width()*0.76,r.top()+r.height()*0.24);
    p.rect_filled(Rect::from_center_size(sun,vec2(tile*8.0,tile*8.0)),0,Color32::from_rgb(237,188,116));
    p.rect_filled(Rect::from_center_size(sun,vec2(tile*10.0,tile*4.0)),0,Color32::from_rgb(250,220,158));
    p.rect_filled(Rect::from_center_size(sun,vec2(tile*4.0,tile*10.0)),0,Color32::from_rgb(250,220,158));
    let layers=[Color32::from_rgb(101,107,92),Color32::from_rgb(76,82,65),Color32::from_rgb(49,58,48),Color32::from_rgb(37,43,39)];
    for (layer,color) in layers.into_iter().enumerate() {
        let step=tile*2.0;
        let columns=(r.width()/step).ceil() as usize;
        let baseline=r.height()*(0.53+layer as f32*0.105);
        for x in 0..columns {
            let segment=x/8;
            let frac=(x%8) as f32/8.0;
            let hash=|n:usize| {
                let mut h=seed.wrapping_add((layer as u32+1).wrapping_mul(0x9E37_79B9))
                    .wrapping_add((n as u32).wrapping_mul(0x85EB_CA6B));
                h^=h>>16;h=h.wrapping_mul(0x7FEB_352D);h^=h>>15;
                (h as f32/u32::MAX as f32-0.5)*r.height()*0.085
            };
            let mut height=baseline+hash(segment)*(1.0-frac)+hash(segment+1)*frac;
            if kind==WorldType::Ridged { height-=height.rem_euclid(tile*3.0)*0.28; }
            if kind==WorldType::Superflat { height=baseline; }
            height=(height/tile).round()*tile;
            let left=r.left()+x as f32*step;
            let y=(r.top()+height).clamp(r.top()+r.height()*0.34,r.bottom());
            p.rect_filled(Rect::from_min_max(pos2(left,y),pos2((left+step).min(r.right()),r.bottom())),0,color);
            // A few blocky treetops anchor the near ridge in voxel terrain.
            if layer==3 && x%13==5 && y<r.bottom()-tile*7.0 {
                let trunk=Rect::from_min_size(pos2(left+tile*0.8,y-tile*2.0),vec2(tile*0.6,tile*2.0));
                p.rect_filled(trunk,0,Color32::from_rgb(93,66,49));
                p.rect_filled(Rect::from_min_max(pos2(left-tile,y-tile*5.0),pos2(left+tile*3.0,y-tile)),0,Color32::from_rgb(89,105,67));
            }
        }
    }
}

fn backdrop(ctx: &egui::Context, seed: u32, kind: WorldType) {
    let p = ctx.layer_painter(egui::LayerId::background());
    let r = ctx.content_rect();
    landscape(&p, r, seed, kind);
    p.rect_filled(r, 0, Color32::from_rgba_unmultiplied(18, 16, 18, 100));
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
    ui.painter().rect_filled(
        r,
        0,
        if selected {
            Color32::from_rgb(76, 60, 44)
        } else {
            Color32::from_rgb(42, 35, 36)
        },
    );
    ui.painter().rect_stroke(
        r,
        0,
        Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected || response.has_focus() {
                MINT
            } else {
                Color32::from_rgb(86, 71, 56)
            },
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
    ctx: &egui::Context,
    world: &mut WorldType,
    scale: &mut VoxelScalePreset,
    seed: &mut u32,
    render: &mut RenderUiState,
    status: Option<&str>,
    name: &mut String,
    loading: bool,
) -> UiAction {
    backdrop(ctx, *seed, *world);
    let mut result = UiAction::None;
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui| {
        let wide=ui.available_width()>1000.0;
        egui::ScrollArea::vertical().id_salt("world_front_end").show(ui,|ui| {
            ui.add_space(if wide{36.0}else{16.0});
            let width=(ui.available_width()-32.0).clamp(200.0,1040.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                surface().show(ui,|ui| {
                    let mut form=|ui:&mut egui::Ui,result:&mut UiAction,world:&mut WorldType,scale:&mut VoxelScalePreset,seed:&mut u32,render:&mut RenderUiState| {
                        caption(ui,"WORLD SETUP");
                        heading(ui,"New world","Choose terrain, block size and seed.");
                        ui.add(egui::TextEdit::singleline(name).hint_text("World name").char_limit(64).desired_width(f32::INFINITY));
                        for (kind,title,detail) in [
                            (WorldType::Hills,"Wildlands","Forests, coastlines and caves to discover."),
                            (WorldType::Ridged,"Highlands","Steep ridges and broad mountain silhouettes."),
                            (WorldType::Superflat,"Open canvas","A level landscape for ambitious builds."),
                        ] {
                            if world_card(ui,kind,*world==kind,*seed,title,detail).clicked(){*world=kind;}
                        }
                        ui.add_space(8.0);caption(ui,"BUILDING PRECISION");
                        let old=scale.meters();
                        ui.horizontal_wrapped(|ui| {
                            for (preset,label) in [(VoxelScalePreset::OneMeter,"Classic · 1 m"),(VoxelScalePreset::HalfMeter,"Fine · 0.5 m"),(VoxelScalePreset::QuarterMeter,"Micro · 0.25 m")] {
                                let response=tab(ui,label,*scale==preset);
                                if response.clicked(){*scale=preset;}
                            }
                        });
                        if old!=scale.meters(){render.settings=render.settings.rescale_blocks(old,scale.meters());}
                        ui.small("Smaller blocks add detail; your height and movement stay the same.");
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("World seed").color(MUTED));
                            ui.add(egui::DragValue::new(seed).range(0..=u32::MAX).speed(1.0));
                            if ui.small_button("Shuffle").clicked(){*seed=next_seed(*seed);}
                        });
                        ui.add_space(12.0);
                        ui.add_enabled_ui(!loading && !name.trim().is_empty(),|ui| {
                            if action(ui,"Create world",true).clicked(){*result=UiAction::StartWorld {name:name.trim().to_owned(),reference:false,world_type:*world,voxel_scale:*scale,seed:*seed};}
                        });
                        if !loading && action(ui,"Create Moss & Ember studio",false).clicked() {
                            *result=UiAction::StartWorld{name:"Moss & Ember".into(),reference:true,world_type:WorldType::Superflat,voxel_scale:*scale,seed:*seed};
                        }
                        if let Some(status)=status {ui.label(RichText::new(status).small().color(MUTED));}
                        ui.collapsing("Display & terrain quality",|ui|settings::graphics_settings(ui,render,scale.meters()));
                    };
                    if wide {
                        ui.columns(2,|cols| {
                            cols[0].set_max_width(410.0);
                            caption(&mut cols[0],"LANDSCAPE");
                            cols[0].add_space(12.0);
                            cols[0].label(RichText::new("Terrain preview").size(26.0).color(PAPER).strong());
                            cols[0].add_space(14.0);
                            cols[0].label("Landscape sketch. The generated terrain varies with the seed.");
                            let (r,_)=cols[0].allocate_exact_size(vec2(cols[0].available_width(),190.0),Sense::hover());
                            landscape(&cols[0].painter().with_clip_rect(r),r,*seed,*world);
                            cols[0].add_space(14.0);
                            cols[0].small("WASD to explore   ·   E for inventory   ·   F to interact");
                            form(&mut cols[1],&mut result,world,scale,seed,render);
                        });
                    } else {
                        caption(ui,"VOXEL WORKSHOP");
                        form(ui,&mut result,world,scale,seed,render);
                    }
                });
            });
            ui.add_space(24.0);
        });
    });
    result
}
pub(super) fn journal(
    ui: &mut egui::Ui,
    expedition: Option<&crate::world::expedition::Expedition>,
    adventure: &crate::world::adventure::View,
) {
    heading(
        ui,
        "Field journal",
        "Objectives, equipment and supplies.",
    );
    let Some(e) = expedition else {
        ui.label("No world loaded.");
        return;
    };
    super::relics::expedition_goal(ui,adventure);
    caption(ui, "WORKSHOP PREPARATION");
    ui.label(RichText::new(e.objective()).size(17.0).color(MINT));
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
        ui.label("Gather salvage and ore to fill your pack.");
    }
}
pub(super) fn pause(
    owner: &mut EguiUi,
    time: &mut TimeUiState,
    physics: &mut PhysicsUiState,
    render: &mut RenderUiState,
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
    egui::Area::new("journey_pause".into())
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .default_size(vec2(width, (screen.height() - 48.0).max(80.0)))
        .show(&ctx, |ui| {
            ui.set_width(width);
            // Areas otherwise inherit a short default height. A ScrollArea
            // clamps to that available height even with a larger max_height.
            ui.set_max_height((screen.height() - 48.0).max(80.0));
            pause_surface().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Paused").size(24.0).strong().color(PAPER));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Esc  Resume").monospace().small().color(MUTED));
                    });
                });
                if let Some(status) = &owner.preferences_status {
                    ui.label(RichText::new(status).small().color(GOLD));
                }
                ui.horizontal_wrapped(|ui| {
                    for (page, label) in [
                        (PausePage::Journey, "Game"),
                        (PausePage::Journal, "Journal"),
                        (PausePage::Graphics, "Display"),
                        (PausePage::Controls, "Controls"),
                        (PausePage::Environment, "World"),
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
                    ui.label(RichText::new(error).small());
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
                            if action(ui, "Save now", false).clicked() {
                                result = UiAction::SaveWorld;
                            }
                            if let Some(status) = &owner.persistence_status {
                                ui.label(RichText::new(status).small().color(MUTED));
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
                            ui.label(RichText::new("World list uses your last save. Save now to keep recent changes.").small().color(MUTED));
                        }
                        PausePage::Journal => journal(ui, owner.expedition.as_ref(), &owner.adventure),
                        PausePage::Graphics => {
                            heading(
                                ui,
                                "Display",
                                "View distance, lighting and frame rate.",
                            );
                            settings::graphics_settings(ui, render, physics.voxel_scale.meters());
                            ui.add(
                                egui::Slider::new(&mut owner.fov_degrees, 50.0..=110.0)
                                    .text("Field of view"),
                            );
                            if let Some(status) = &owner.region_status {
                                ui.collapsing("Storage diagnostics", |ui| {
                                    ui.small(status);
                                });
                            }
                        }
                        PausePage::Controls => {
                            heading(ui, "Controls", "Mouse settings and key bindings.");
                            ui.add(
                                egui::Slider::new(&mut owner.mouse_sensitivity, 0.0005..=0.006)
                                    .logarithmic(true)
                                    .text("Mouse sensitivity"),
                            );
                            ui.checkbox(&mut owner.invert_mouse, "Invert vertical look");
                            settings::audio_settings(ui);
                            presentation::controls(ui);
                        }
                        PausePage::Environment => {
                            heading(
                                ui,
                                "World",
                                "Weather, simulation and saving.",
                            );
                            settings::environment_settings(ui, time, physics, &mut result);
                        }
                    });
            });
        });
    result
}
pub(super) fn loading(ctx: &egui::Context, state: &LoadingUiState) {
    backdrop(ctx, 71, WorldType::Hills);
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.32).max(24.0));
                ui.set_max_width(520.0);
                surface().show(ui, |ui| {
                    caption(ui, "VOXEL WORKSHOP");
                    heading(ui, &state.title, &state.detail);
                    let (r, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 5.0), Sense::hover());
                    ui.painter()
                        .rect_filled(r, 0, Color32::from_rgb(72, 58, 47));
                    let progress = state.progress_01.clamp(0.0, 1.0);
                    ui.painter().rect_filled(
                        Rect::from_min_size(r.min, vec2(r.width() * progress, r.height())),
                        0,
                        MINT,
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!(
                            "{:.0}%  ·  Preparing nearby terrain",
                            progress * 100.0
                        ))
                        .small()
                        .color(MUTED),
                    );
                });
            });
        });
}

pub(super) fn journey_hud(
    ctx: &egui::Context,
    expedition: Option<&crate::world::expedition::Expedition>,
    reserved: Option<Rect>,
) {
    let reserve_id = egui::Id::new("hud_top_reserved");
    ctx.data_mut(|d| d.insert_temp(reserve_id, reserved));
    let Some(e) = expedition else { return };
    // Objectives belong in the journal. Only new interaction feedback needs
    // screen space during play.
    if e.message_seconds > 0.0 && !e.message.is_empty() {
        let width = (ctx.content_rect().width()-56.0).clamp(100.0,480.0);
        let left = ctx.content_rect().center().x-width*0.5-10.0;
        let top = reserved.filter(|r| r.right()+8.0>left)
            .map_or(ctx.content_rect().top()+20.0, |r| r.bottom()+8.0);
        let notice = egui::Area::new("journey_notification".into())
            .anchor(Align2::CENTER_TOP, [0.0, top-ctx.content_rect().top()])
            .interactable(false)
            .show(ctx, |ui| {
                presentation::hud_frame().show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.label(RichText::new(&e.message).color(MINT));
                });
            });
        let occupied = reserved.map_or(notice.response.rect, |r| r.union(notice.response.rect));
        ctx.data_mut(|d| d.insert_temp(reserve_id, Some(occupied)));
    }
}

pub(super) fn tab(ui: &mut egui::Ui, text: &str, selected: bool) -> Response {
    let font = FontId::proportional(14.0);
    let color = if selected { MINT } else { MUTED };
    let label = ui.painter().layout_no_wrap(text.into(), font, color);
    let (r, response) = ui.allocate_exact_size(vec2(label.size().x + 22.0, 32.0), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), text));
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(r, 0, Color32::from_rgb(68, 54, 43));
    }
    ui.painter()
        .galley(r.center() - label.size() * 0.5, label, color);
    if selected || response.has_focus() {
        ui.painter().line_segment(
            [
                r.left_bottom() + vec2(5.0, -1.0),
                r.right_bottom() + vec2(-5.0, -1.0),
            ],
            Stroke::new(2.0, if selected { MINT } else { GOLD }),
        );
    }
    response
}
pub(super) fn storage_notice(ctx: &egui::Context, error: &str) {
    egui::Area::new("world_storage_notice".into())
        .order(egui::Order::Foreground)
        .anchor(Align2::LEFT_TOP, [16.0, 72.0])
        .show(ctx, |ui| {
            presentation::hud_frame().show(ui, |ui| {
                ui.set_max_width((ctx.content_rect().width() - 48.0).clamp(180.0, 420.0));
                ui.label(RichText::new("Saving needs attention").strong().color(GOLD));
                ui.label(RichText::new(error).small().color(PAPER));
                ui.label(
                    RichText::new("Unsaved edits remain in memory. Pause to review your save.")
                        .small()
                        .color(MUTED),
                );
            });
        });
}
