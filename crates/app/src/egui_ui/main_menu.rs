//! Title menu: the wordmark and one list of actions over the vista.
//!
//! Vanilla register (UI_RULES): a name, one plain line saying what the game
//! holds (owner request 2026-11-14), and a short stack of equal rows. No icons, no cards. The default action
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
/// One plain line under the wordmark: what the game contains.
const TAGLINE: &str = "One world, from valley to orbit";
const TAGLINE_HEIGHT: f32 = 26.0;
/// Extra row height for a quiet second line under a label.
const NOTE_HEIGHT: f32 = 16.0;

pub(super) fn draw(
    root: &mut egui::Ui, recent: Option<(&str, &str)>, busy: bool, status: Option<&str>,
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

            // Each row: label, world name beside it, a quiet second line, weight, intent.
            let mut rows: Vec<(&str, Option<&str>, Option<&str>, Weight, Intent)> = Vec::new();
            match recent {
                Some((world, summary)) => {
                    // The world type and age under Continue: a player with several
                    // test worlds needs to know which one Enter resumes.
                    rows.push(("Continue", Some(world), Some(summary), Weight::Primary, Intent::Continue));
                    rows.push(("Worlds", None, None, Weight::Secondary, Intent::Load));
                    rows.push(("New World", None, None, Weight::Secondary, Intent::New));
                }
                None => {
                    rows.push(("New World", None, None, Weight::Primary, Intent::New));
                    rows.push(("Worlds", None, None, Weight::Secondary, Intent::Load));
                }
            }
            rows.push(("Multiplayer", None, None, Weight::Secondary, Intent::Online));
            let list: f32 = rows.iter().map(|r| r.3.height() + if r.2.is_some() { NOTE_HEIGHT } else { 0.0 }).sum();
            let stack_height = WORDMARK_SIZE + TAGLINE_HEIGHT + TITLE_GAP + list + GROUP_GAP
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
                        tagline(ui, column_width, &palette);
                        ui.add_space(TITLE_GAP);
                        for (label, detail, note, weight, intent) in rows {
                            let response = action(ui, width, label, detail, note, weight, !busy);
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
                        if action(ui, width, "Settings", None, None, Weight::Quiet, true).clicked() {
                            result = Intent::Settings;
                        }
                        if action(ui, width, "Quit", None, None, Weight::Quiet, true).clicked() {
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
    let step = (galley.size().y / 40.0).round().max(1.0);
    // The extrusion is part of the mark, so the row makes room for it.
    let (r, _) = ui.allocate_exact_size(vec2(width, galley.size().y + 3.0 * step), egui::Sense::hover());
    // Weight without a mark: a three-step pixel extrusion in a dark accent tone
    // under the letters, then the long drop shadow beyond it. The plain heading
    // face at menu-row colour read as a placeholder beside the rows.
    let shadow = egui::Color32::from_black_alpha(190);
    let a = palette.accent;
    let depth = egui::Color32::from_rgb(a.r() / 3, a.g() / 3, a.b() / 3);
    ui.painter().galley_with_override_text_color(r.min + vec2(3.0, 4.0) * step + vec2(1.0, 1.0), galley.clone(), shadow);
    for i in (1..=3).rev() {
        ui.painter().galley_with_override_text_color(r.min + vec2(i as f32, i as f32) * step, galley.clone(), depth);
    }
    ui.painter().galley(r.min, galley, palette.text());
}


/// One muted line under the name, cut to the column rather than wrapped.
fn tagline(ui: &mut egui::Ui, width: f32, palette: &super::themes::Palette) {
    let font = egui::FontId::proportional(15.0);
    let (r, _) = ui.allocate_exact_size(vec2(width, TAGLINE_HEIGHT), egui::Sense::hover());
    let clip = ui.painter().with_clip_rect(r);
    let mut galley = clip.layout_no_wrap(TAGLINE.to_owned(), font, palette.muted());
    let room = (width - 6.0).max(40.0);
    if galley.size().x > room {
        // Shrink to the column instead of cutting the last word.
        let fitted = (15.0 * room / galley.size().x).floor().max(10.0);
        galley = clip.layout_no_wrap(TAGLINE.to_owned(), egui::FontId::proportional(fitted), palette.muted());
    }
    let at = egui::pos2(r.left() + 3.0, (r.center().y + 2.0 - galley.size().y * 0.5).round());
    clip.galley_with_override_text_color(at + vec2(1.0, 1.0), galley.clone(), egui::Color32::from_black_alpha(170));
    clip.galley(at, galley, palette.muted());
}

/// Chunky three-point pixel caret pointing right, centred on `y`.
fn caret(p: &egui::Painter, x: f32, y: f32, color: egui::Color32) {
    for (column, half) in [(0.0, 7.5), (3.0, 4.5), (6.0, 1.5)] {
        let r = Rect::from_min_max(egui::pos2(x + column, y - half), egui::pos2(x + column + 3.0, y + half));
        p.rect_filled(super::grimoire::snap(r), 0, color);
    }
}

fn action(ui: &mut egui::Ui, width: f32, label: &str, detail: Option<&str>, note: Option<&str>,
    weight: Weight, enabled: bool) -> egui::Response {
    let height = weight.height() + if note.is_some() { NOTE_HEIGHT } else { 0.0 };
    let (r, response) = ui.allocate_exact_size(vec2(width, height),
        if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    // The label line keeps the row's usual height; a note sits under it.
    let line = Rect::from_min_size(r.min, vec2(r.width(), weight.height()));
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
        caret(p, r.left() + 8.0, line.center().y.round(), palette.accent);
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
    let at = egui::pos2(r.left() + LABEL_INSET, line.center().y + offset);
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
        let at = egui::pos2(label_right + 14.0, (line.center().y + offset - galley.size().y * 0.5 + 2.0).round());
        if lift < 0.5 {
            clip.galley_with_override_text_color(at + vec2(1.0, 1.0), galley.clone(),
                egui::Color32::from_black_alpha(170));
        }
        clip.galley(at, galley, palette.muted());
        let full = clip.layout_no_wrap(detail.to_owned(), font, palette.muted()).size().x;
        if full > room { response.clone().on_hover_text(detail); }
    }
    if let Some(note) = note {
        let galley = clip.layout_no_wrap(note.to_owned(), egui::FontId::proportional(12.0), palette.muted());
        let at = egui::pos2(r.left() + LABEL_INSET, (line.bottom() - 6.0 + offset).round());
        if lift < 0.5 {
            clip.galley_with_override_text_color(at + vec2(1.0, 1.0), galley.clone(),
                egui::Color32::from_black_alpha(170));
        }
        clip.galley(at, galley, palette.muted());
    }
    response
}
