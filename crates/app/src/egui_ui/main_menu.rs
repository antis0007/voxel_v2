//! Title menu: the wordmark and one list of actions over the vista.
//!
//! Vanilla register (UI_RULES): a name and a short stack of equal rows. No
//! tagline (UI_RULES text rule 6), no icons, no cards. The default action
//! carries a pixel caret so Enter's target is visible; every other row shows it only while hovered or
//! focused. Rows are plain text with a hard pixel shadow until the pointer
//! reaches them, then a strip of panel glass eases in behind the label.
use egui::{Rect, RichText, vec2};

#[derive(Default)]
pub(super) enum Intent {
    #[default]
    None,
    Continue,
    Load,
    New,
    Online,
    Settings,
    Quit,
}

#[derive(Clone, Copy, PartialEq)]
enum Weight { Primary, Secondary, Quiet }

impl Weight {
    fn height(self) -> f32 {
        match self { Self::Primary => 40.0, Self::Secondary => 36.0, Self::Quiet => 30.0 }
    }
    fn size(self) -> f32 {
        match self { Self::Primary => 22.0, Self::Secondary => 20.0, Self::Quiet => 16.0 }
    }
}

/// Rows are narrower than the title column: a short measure reads as a list,
/// a wide one as a form.
const LIST_WIDTH: f32 = 300.0;
const WORDMARK_SIZE: f32 = 76.0;
/// Labels start after the caret gutter; notes under the list align with them.
const LABEL_INSET: f32 = 26.0;
/// Space between the wordmark and the first row, and before the utility rows.
const TITLE_GAP: f32 = 30.0;
const GROUP_GAP: f32 = 18.0;

pub(super) fn draw(
    root: &mut egui::Ui, recent: Option<&str>, busy: bool, status: Option<&str>,
) -> Intent {
    let ctx = root.ctx().clone();
    let palette = super::themes::current(&ctx);
    let mut result = Intent::None;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(palette.recess).inner_margin(egui::Margin::same(24)))
        .show(root, |ui| {
            let screen = ctx.content_rect();
            // The loading screen shares this column so the vista continues without a cut.
            let (left, column_width, quiet_edge) = super::menu_scene::column(screen);
            let width = column_width.min(LIST_WIDTH);
            let indent = (left - ui.max_rect().left()).max(0.0);
            super::menu_scene::paint(ui.painter(), screen, quiet_edge, 0.0);
            // Build version in the far corner, as quiet as the vista allows.
            let version = concat!("v", env!("CARGO_PKG_VERSION"));
            let corner = screen.right_bottom() - vec2(16.0, 12.0);
            for (offset, color) in [(vec2(1.0, 1.0), egui::Color32::from_black_alpha(170)),
                (vec2(0.0, 0.0), palette.muted())] {
                ui.painter().text(corner + offset, egui::Align2::RIGHT_BOTTOM, version,
                    egui::FontId::proportional(12.0), color);
            }

            let mut rows: Vec<(&str, Option<&str>, Weight, Intent)> = Vec::new();
            match recent {
                Some(world) => {
                    rows.push(("Continue", Some(world), Weight::Primary, Intent::Continue));
                    rows.push(("Worlds", None, Weight::Secondary, Intent::Load));
                    rows.push(("New World", None, Weight::Secondary, Intent::New));
                }
                None => {
                    rows.push(("New World", None, Weight::Primary, Intent::New));
                    rows.push(("Worlds", None, Weight::Secondary, Intent::Load));
                }
            }
            rows.push(("Multiplayer", None, Weight::Secondary, Intent::Online));
            let list: f32 = rows.iter().map(|r| r.2.height()).sum();
            let stack_height = WORDMARK_SIZE + TITLE_GAP + list + GROUP_GAP
                + 2.0 * Weight::Quiet.height()
                + if busy { 26.0 } else { 0.0 }
                + if status.is_some() { 40.0 } else { 0.0 };
            // A title block reads as centred slightly above the true middle.
            ui.add_space(((ui.available_height() - stack_height) * 0.44).max(0.0));
            let column = Rect::from_min_size(
                egui::pos2(ui.max_rect().left() + indent, ui.cursor().top()),
                vec2(column_width, ui.available_height()));
            ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
                ui.set_max_width(column_width);
                egui::ScrollArea::vertical().id_salt("title_menu").show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(column_width);
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        wordmark(ui, column_width, &palette);
                        ui.add_space(TITLE_GAP);
                        for (label, detail, weight, intent) in rows {
                            let response = action(ui, width, label, detail, weight, !busy);
                            if weight == Weight::Primary {
                                // The exporter uses the real primary hit target for focus captures.
                                ctx.data_mut(|d| d.insert_temp(egui::Id::new("title-primary-rect"), response.rect));
                            }
                            if response.clicked() || (weight == Weight::Primary && !busy
                                && ctx.memory(|m| m.focused().is_none())
                                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))) {
                                result = intent;
                            }
                        }
                        ui.add_space(GROUP_GAP);
                        if action(ui, width, "Settings", None, Weight::Quiet, true).clicked() {
                            result = Intent::Settings;
                        }
                        if action(ui, width, "Quit", None, Weight::Quiet, true).clicked() {
                            result = Intent::Quit;
                        }
                        if busy {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(LABEL_INSET);
                                ui.label(RichText::new("Loading").color(palette.muted()));
                            });
                        }
                        if let Some(status) = status {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(LABEL_INSET);
                                ui.label(RichText::new(status).color(super::presentation::DANGER));
                            });
                        }
                    });
                });
            });
        });
    result
}

/// Spaced capitals in the heading face with a hard drop shadow; shrinks to fit
/// narrow windows instead of wrapping.
fn wordmark(ui: &mut egui::Ui, width: f32, palette: &super::themes::Palette) {
    use egui::text::{LayoutJob, TextFormat};
    let mut font = egui::TextStyle::Heading.resolve(ui.style());
    let layout = |ui: &egui::Ui, size: f32, font: &mut egui::FontId| {
        font.size = size;
        let mut job = LayoutJob::default();
        job.append("AXIOM", 0.0, TextFormat {
            font_id: font.clone(),
            color: palette.text(),
            extra_letter_spacing: (size * 0.14).round(),
            ..Default::default()
        });
        ui.painter().layout_job(job)
    };
    let mut galley = layout(ui, WORDMARK_SIZE, &mut font);
    if galley.size().x > width {
        let fitted = (WORDMARK_SIZE * width / galley.size().x).floor().max(24.0);
        galley = layout(ui, fitted, &mut font);
    }
    let (r, _) = ui.allocate_exact_size(vec2(width, galley.size().y), egui::Sense::hover());
    let shadow = egui::Color32::from_black_alpha(190);
    ui.painter().galley_with_override_text_color(r.min + vec2(3.0, 4.0), galley.clone(), shadow);
    ui.painter().galley(r.min, galley, palette.text());
}


/// Chunky three-point pixel caret pointing right, centred on `y`.
fn caret(p: &egui::Painter, x: f32, y: f32, color: egui::Color32) {
    for (column, half) in [(0.0, 7.5), (3.0, 4.5), (6.0, 1.5)] {
        let r = Rect::from_min_max(egui::pos2(x + column, y - half), egui::pos2(x + column + 3.0, y + half));
        p.rect_filled(super::grimoire::snap(r), 0, color);
    }
}

fn action(ui: &mut egui::Ui, width: f32, label: &str, detail: Option<&str>,
    weight: Weight, enabled: bool) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(vec2(width, weight.height()),
        if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled,
        detail.map_or_else(|| label.to_owned(), |world| format!("{label}: {world}"))));
    let palette = super::themes::current(ui.ctx());
    let feedback = super::motion::feedback(&response, false, enabled);
    let lift = if enabled { feedback.hover } else { 0.0 };
    let pressed = enabled && response.is_pointer_button_down_on();
    let p = ui.painter();
    if lift > 0.01 {
        // One hue of glass behind the hovered row, darkened while pressed.
        let tint = super::themes::blend(palette.panel, palette.recess, 0.5 * feedback.press);
        p.rect_filled(r, 0, egui::Color32::from_rgba_unmultiplied(
            tint.r(), tint.g(), tint.b(), (200.0 * lift) as u8));
        p.rect_filled(Rect::from_min_size(r.min, vec2(2.0, r.height())), 0,
            palette.accent.gamma_multiply(lift));
    }
    let default_target = weight == Weight::Primary && ui.memory(|m| m.focused().is_none());
    if enabled && (default_target || response.has_focus() || response.hovered()) {
        caret(p, r.left() + 8.0, r.center().y.round(), palette.accent);
    }
    let text = if !enabled {
        palette.muted().gamma_multiply(0.7)
    } else if weight == Weight::Quiet && lift < 0.5 {
        palette.muted()
    } else {
        palette.text()
    };
    let offset = if pressed { 1.0 } else { 0.0 };
    let font = egui::FontId::proportional(weight.size());
    let at = egui::pos2(r.left() + LABEL_INSET, r.center().y + offset);
    let clip = p.with_clip_rect(Rect::from_min_max(egui::pos2(r.left(), r.top()), r.max - vec2(8.0, 0.0)));
    let galley = clip.layout_no_wrap(label.to_owned(), font, text);
    let top = egui::pos2(at.x, (at.y - galley.size().y * 0.5).round());
    // Hard shadow only while the row is bare; on glass it would read as a blur.
    if lift < 0.5 {
        clip.galley_with_override_text_color(top + vec2(1.0, 2.0), galley.clone(),
            egui::Color32::from_black_alpha(170));
    }
    let label_right = top.x + galley.size().x;
    clip.galley(top, galley, text);
    if let Some(detail) = detail {
        // The world name follows the label on its baseline, cut to the row.
        let room = (r.right() - 12.0 - label_right - 14.0).max(1.0);
        let font = egui::FontId::proportional(14.0);
        let mut job = egui::text::LayoutJob::simple(detail.to_owned(), font.clone(), palette.muted(), room);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        let galley = clip.layout_job(job);
        let at = egui::pos2(label_right + 14.0, (r.center().y + offset - galley.size().y * 0.5 + 2.0).round());
        if lift < 0.5 {
            clip.galley_with_override_text_color(at + vec2(1.0, 1.0), galley.clone(),
                egui::Color32::from_black_alpha(170));
        }
        clip.galley(at, galley, palette.muted());
        let full = clip.layout_no_wrap(detail.to_owned(), font, palette.muted()).size().x;
        if full > room { response.clone().on_hover_text(detail); }
    }
    response
}
