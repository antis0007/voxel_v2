//! Carved rune glyphs for the wand page. Shapes and goggle colours come from
//! `assets/runes.ron`; without rune goggles every glyph is drawn in plain ink.
use crate::egui_ui::presentation as style;
use crate::world::adventure::runes;
use egui::{Color32, Rect, RichText, Sense, Stroke, pos2, vec2};
use engine_content::runes::{Attach, GLYPH_GRID, Hue, Role, RuneId};

/// Ink of a rune: its hue through goggles, plain paper otherwise.
pub(super) fn ink(id: RuneId, sight: bool) -> Color32 {
    if sight {
        let [r, g, b] = runes::TABLE.color(id);
        Color32::from_rgb(r, g, b)
    } else {
        style::PAPER
    }
}

/// Draw one glyph filling `rect`, keeping the grid's 4:6 shape.
pub(super) fn paint(painter: &egui::Painter, rect: Rect, id: RuneId, color: Color32) {
    let (w, h) = GLYPH_GRID;
    let unit = (rect.width() / w).min(rect.height() / h);
    let origin = rect.center() - vec2(w, h) * unit * 0.5;
    let stroke = Stroke::new((unit * 0.42).max(1.5), color);
    for (x0, y0, x1, y1) in runes::TABLE.strokes(id) {
        painter.line_segment([origin + vec2(x0, y0) * unit, origin + vec2(x1, y1) * unit], stroke);
    }
}

/// A row of glyphs for a phrase in written order. An event opens a bracket that closes at
/// the end of its child; hovering a glyph shows its stat tooltip.
pub(super) fn strip(ui: &mut egui::Ui, phrase: &engine_content::runes::Phrase, sight: bool, height: f32) {
    let table = &*runes::TABLE;
    let ids = table.written(phrase);
    let cell = vec2(height * 0.72, height);
    let events = ids.iter().filter(|&&id| table.rune(id).role == Role::Event).count();
    let gap = height * 0.14;
    let width = ids.len() as f32 * (cell.x + gap) + events as f32 * cell.x * 0.5;
    let (rect, _) = ui.allocate_exact_size(vec2(width.max(1.0), height), Sense::hover());
    let painter = ui.painter_at(rect.expand(2.0));
    let mut x = rect.left();
    for &id in &ids {
        let at = Rect::from_min_size(pos2(x, rect.top()), cell);
        paint(&painter, at.shrink(height * 0.08), id, ink(id, sight));
        let rune = table.rune(id);
        let hover = ui.interact(at, ui.id().with(("rune_glyph", x as i32)), Sense::hover());
        hover.on_hover_ui(|ui| tooltip(ui, id, sight, 0));
        x += cell.x + gap;
        if rune.role == Role::Event {
            // A thin bar marks where the event's child phrase begins.
            let bar = x - gap * 0.5;
            painter.line_segment(
                [pos2(bar, rect.top() + height * 0.1), pos2(bar, rect.bottom() - height * 0.1)],
                Stroke::new(1.0, ink(id, sight).gamma_multiply(0.5)),
            );
            x += cell.x * 0.5;
        }
    }
}

const ROMAN: [&str; 5] = ["I", "II", "III", "IV", "V"];
/// Tier requirement the held wand does not meet.
const LOCKED: Color32 = Color32::from_rgb(0xd8, 0x7a, 0x5c);

/// A rune above the held wand's tier. With no wand held nothing is locked.
fn locked(rune: &engine_content::runes::Rune, wand: u32) -> bool {
    wand > 0 && rune.tier as u32 > wand
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Form => "Form",
        Role::Effect => "Effect",
        Role::Material => "Material",
        Role::Qualifier => "Qualifier",
        Role::Event => "Event",
    }
}

fn hue_name(hue: Hue) -> &'static str {
    match hue {
        Hue::Form => "Form",
        Hue::Fire => "Fire",
        Hue::Frost => "Frost",
        Hue::Shock => "Shock",
        Hue::Venom => "Venom",
        Hue::Water => "Water",
        Hue::Force => "Force",
        Hue::Life => "Life",
        Hue::Earth => "Earth",
        Hue::Arcane => "Mana",
        Hue::Event => "Event",
    }
}

/// Tooltip of one rune: name, then at most three stat lines read straight off the table.
fn tooltip(ui: &mut egui::Ui, id: RuneId, sight: bool, wand: u32) {
    let rune = runes::TABLE.rune(id);
    ui.label(RichText::new(&rune.name).strong().color(ink(id, sight)));
    let mut kind = role_name(rune.role).to_owned();
    if !matches!(rune.hue, Hue::Form | Hue::Event) {
        kind += &format!("  ·  {}", hue_name(rune.hue));
    }
    if rune.role == Role::Qualifier {
        kind += match rune.on {
            Attach::Form => "  ·  On: Form",
            Attach::Effect => "  ·  On: Effect",
            Attach::Either => "  ·  On: Any",
        };
    }
    ui.label(RichText::new(kind).size(12.0));
    let cost = if rune.role == Role::Qualifier {
        format!("Cost: ×{}", rune.scale)
    } else if rune.upkeep > 0.0 {
        format!("Mana: {:.0}  ·  Upkeep: {:.0}/s", rune.mana, rune.upkeep)
    } else {
        format!("Mana: {:.0}", rune.mana)
    };
    ui.label(RichText::new(cost).size(12.0));
    let tier = ROMAN[(rune.tier.max(1) as usize - 1).min(4)];
    if rune.circle || (rune.card.is_none() && matches!(rune.role, Role::Effect | Role::Material)) {
        ui.label(RichText::new("Requires: Ritual Circle").size(12.0).weak());
    } else if locked(rune, wand) {
        ui.label(RichText::new(format!("Requires: Tier {tier} Wand")).size(12.0).color(LOCKED));
    } else {
        ui.label(RichText::new(format!("Tier: {tier}")).size(12.0).weak());
    }
}

/// Every rune as a carved glyph, grouped by role; qualifiers split by what they attach to.
/// Runes above the held wand's tier are dimmed and runes in the current phrase are underlined.
/// Clicking one adds its word to `text`.
pub(super) fn key(ui: &mut egui::Ui, sight: bool, wand: u32, used: &[RuneId], text: &mut String) {
    let table = &*runes::TABLE;
    let cell = vec2(22.0, 30.0);
    let groups: [(&str, Role, Option<Attach>); 7] = [
        ("Form", Role::Form, None),
        ("Effect", Role::Effect, None),
        ("Material", Role::Material, None),
        ("On Form", Role::Qualifier, Some(Attach::Form)),
        ("On Effect", Role::Qualifier, Some(Attach::Effect)),
        ("On Either", Role::Qualifier, Some(Attach::Either)),
        ("Event", Role::Event, None),
    ];
    for (label, role, attach) in groups {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            ui.add_sized(vec2(64.0, cell.y), egui::Label::new(RichText::new(label).size(11.0).weak()));
            let members = table
                .runes
                .iter()
                .enumerate()
                .filter(|(_, rune)| rune.role == role && attach.is_none_or(|attach| rune.on == attach));
            for (id, rune) in members {
                let id = id as RuneId;
                let (rect, response) = ui.allocate_exact_size(cell, Sense::click());
                if response.hovered() {
                    ui.painter().rect_filled(rect, 3.0, style::PAPER.gamma_multiply(0.08));
                }
                let color = if locked(rune, wand) { ink(id, sight).gamma_multiply(0.3) } else { ink(id, sight) };
                paint(ui.painter(), rect.shrink(4.0), id, color);
                if used.contains(&id) {
                    let y = rect.bottom() - 1.0;
                    ui.painter().line_segment(
                        [pos2(rect.left() + 3.0, y), pos2(rect.right() - 3.0, y)],
                        Stroke::new(2.0, ink(id, sight)),
                    );
                }
                let response = response.on_hover_ui(|ui| tooltip(ui, id, sight, wand));
                if response.clicked() {
                    if !text.is_empty() && !text.ends_with(' ') {
                        text.push(' ');
                    }
                    text.push_str(&rune.key);
                }
            }
        });
    }
}
