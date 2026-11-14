//! Rune phrase editor on the wand page: type or pick a phrase, read its compiled cost,
//! and bind it to the held wand (native rune path), draw it as a circle, or, for phrases
//! the native path cannot run yet, inscribe it onto the wand as deck cards.
use crate::egui_ui::{UiAction, presentation as style};
use crate::world::{
    adventure::{self, View, runes},
    factory::Action,
};
use egui::{Color32, RichText, vec2};

static REVIEW_DRAWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const ERROR: Color32 = Color32::from_rgb(0xd8, 0x7a, 0x5c);
const ROMAN: [&str; 5] = ["I", "II", "III", "IV", "V"];

/// A phrase as a chip: its carved glyphs, then the same phrase in plain words.
/// Clicking loads the phrase text; hover shows it as written for the grammar.
fn pick(ui: &mut egui::Ui, label: &str, phrase: &str, text: &mut String, sight: bool) {
    let table = &*runes::TABLE;
    let ids = table.compile(phrase).map(|program| table.written(&program.root)).unwrap_or_default();
    let (glyph_h, glyph_w, pad) = (16.0, 11.5, 5.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), egui::FontId::proportional(11.0), style::PAPER);
    let glyphs = ids.len() as f32 * (glyph_w + 1.5);
    let size = vec2(pad * 2.0 + glyphs + if ids.is_empty() { 0.0 } else { 5.0 } + galley.size().x, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(rect, visuals.corner_radius, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    let mut x = rect.left() + pad;
    for &id in &ids {
        let cell = egui::Rect::from_min_size(egui::pos2(x, rect.center().y - glyph_h * 0.5), vec2(glyph_w, glyph_h));
        super::rune_glyphs::paint(ui.painter(), cell.shrink(1.0), id, super::rune_glyphs::ink(id, sight));
        x += glyph_w + 1.5;
    }
    if !ids.is_empty() {
        x += 5.0;
    }
    ui.painter().galley(egui::pos2(x, rect.center().y - galley.size().y * 0.5), galley, style::PAPER);
    if response.on_hover_text(phrase).clicked() {
        *text = phrase.to_owned();
    }
}

/// Opt-in review capture: start the editor on this phrase.
fn review_phrase() -> String {
    std::env::var("VOXEL_REVIEW_RUNES").unwrap_or_default()
}

/// Phrase source rendered as plain words: `BOLT[HOMING] then SHOCK[CHAIN]` reads as
/// `Bolt (Homing) → Shock (Chain)`. Display only; the grammar text is untouched.
fn words(code: &str) -> String {
    let mut out = String::with_capacity(code.len() + 8);
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if word.eq_ignore_ascii_case("then") {
            out.push('→');
        } else {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(&chars.as_str().to_lowercase());
            }
        }
        word.clear();
    };
    for c in code.chars() {
        if c.is_alphanumeric() || c == '_' {
            word.push(if c == '_' { ' ' } else { c });
            continue;
        }
        flush(&mut word, &mut out);
        match c {
            '[' => out.push_str(" ("),
            ']' => out.push(')'),
            ',' => out.push_str(", "),
            '{' => out.push('['),
            '}' => out.push(']'),
            c => out.push(c),
        }
    }
    flush(&mut word, &mut out);
    out
}

pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut UiAction, width: f32) {
    let id = egui::Id::new("wand_rune_phrase");
    let mut text = ui.ctx().data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(review_phrase);
    let table = &*runes::TABLE;
    ui.label(RichText::new("Rune Phrase").size(13.0).color(style::PAPER));
    if let Some(bound) = &view.rune_phrase {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Bound: {}", words(bound))).size(12.0).color(style::PAPER));
            if ui.add(egui::Button::new("Release").min_size(vec2(0.0, 22.0))).clicked() {
                *action = UiAction::Factory(Action::BindRunes(u64::MAX));
            }
        });
        if let Ok(program) = table.compile(bound) {
            super::rune_glyphs::strip(ui, &program.root, view.rune_sight, 22.0);
        }
    }
    let (compiled, deck, native) = ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut text)
                .font(egui::TextStyle::Monospace)
                .char_limit(160)
                .hint_text("BOLT then FIRE")
                .desired_width(width - 96.0),
        );
        let compiled = (!text.trim().is_empty()).then(|| table.compile(&text));
        let deck = match &compiled {
            Some(Ok(program)) => Some(runes::deck_for(program, view)),
            _ => None,
        };
        let packed = deck.as_ref().and_then(|deck| deck.as_ref().ok()).and_then(|cards| runes::pack(cards));
        let native = match &compiled {
            Some(Ok(program)) if !program.circle => Some(
                adventure::rune_cast::check(program, view.wand, &|card| view.can_wield(card))
                    .and_then(|()| runes::pack_program(program).ok_or_else(|| "Too Long To Bind".to_owned())),
            ),
            _ => None,
        };
        let circle = match &compiled {
            Some(Ok(program)) if program.circle => Some(runes::pack_phrase(program)),
            _ => None,
        };
        if let Some(Some(word)) = circle
            && std::env::var_os("VOXEL_REVIEW_RUNES").is_some()
            && !REVIEW_DRAWN.swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            // Review capture: start the ritual once without a click.
            *action = UiAction::Factory(Action::DrawCircle(word));
        }
        if let Some(word) = circle {
            // A circle phrase is cast on the ground, not stored on the wand.
            if ui.add_enabled(word.is_some() && view.wand > 0, egui::Button::new("Draw Circle").min_size(vec2(84.0, 30.0))).clicked()
                && let Some(word) = word
            {
                *action = UiAction::Factory(Action::DrawCircle(word));
            }
        } else if let Some(Ok(word)) = native {
            // The native path runs the phrase as written, events included.
            if ui.add(egui::Button::new("Bind").min_size(vec2(84.0, 30.0))).clicked() {
                *action = UiAction::Factory(Action::BindRunes(word));
            }
        } else if ui.add_enabled(packed.is_some(), egui::Button::new("Inscribe").min_size(vec2(84.0, 30.0))).clicked()
            && let Some(word) = packed
        {
            *action = UiAction::Factory(Action::InscribeWand(word));
        }
        (compiled, deck, native)
    })
    .inner;
    let used: Vec<_> = match &compiled {
        Some(Ok(program)) => table.written(&program.root),
        _ => Vec::new(),
    };
    match compiled {
        None => {}
        Some(Err(error)) => {
            ui.label(RichText::new(format!("Column {}: {}", error.at + 1, error.message)).size(12.0).color(ERROR));
        }
        Some(Ok(program)) => {
            let mut line = format!("Runes: {}  ·  Mana: {:.0}", program.slots, program.mana);
            if program.sustained() {
                line += &format!("  ·  Upkeep: {:.0}/s", program.upkeep);
            }
            match program.tier {
                Some(tier) => line += &format!("  ·  Tier: {}", ROMAN[(tier as usize - 1).min(4)]),
                None => line += "  ·  Tier: Ritual Core",
            }
            if std::env::var_os("VOXEL_REVIEW_RUNES").is_some() && std::env::var_os("VOXEL_REVIEW_RUNEKEY").is_none() {
                ui.scroll_to_cursor(Some(egui::Align::TOP));
            }
            ui.label(RichText::new(line).size(12.0).color(style::PAPER));
            ui.label(RichText::new(words(&text)).size(13.0).color(style::PAPER));
            super::rune_glyphs::strip(ui, &program.root, view.rune_sight, 26.0);
            let slots = format!("Rune Slots: {}/{}", program.slots, runes::rune_capacity(view.wand));
            match &native {
                Some(Ok(_)) => {
                    ui.label(RichText::new(format!("Ready To Bind  ·  {slots}")).size(12.0).color(style::PAPER));
                }
                Some(Err(reason)) => {
                    // Red only when nothing else can carry the phrase; with a deck fallback it is a note.
                    let text = RichText::new(format!("{reason}  ·  {slots}")).size(12.0);
                    ui.label(if matches!(deck, Some(Ok(_))) { text.weak() } else { text.color(ERROR) });
                }
                None => {}
            }
            match deck {
                // A native phrase binds as written; its deck form is not offered.
                Some(Ok(_)) if matches!(native, Some(Ok(_))) => {}
                // The deck's own reason is noise next to a native verdict.
                Some(Err(_)) if native.is_some() && !program.circle => {}
                Some(Ok(cards)) => {
                    let names = cards.iter().map(|&card| adventure::spell_name(card)).collect::<Vec<_>>().join(" → ");
                    ui.label(RichText::new(format!("Deck: {names}")).size(12.0).weak());
                }
                Some(Err(_)) if program.circle => {
                    let (seconds, radius) = adventure::circles::preview(&program);
                    ui.label(RichText::new(format!("Ritual: {seconds:.0}s  ·  Radius: {radius:.1} m")).size(12.0).weak());
                }
                Some(Err(reason)) => {
                    ui.label(RichText::new(reason).size(12.0).color(ERROR));
                }
                None => {}
            }
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for phrase in &table.presets {
            pick(ui, &words(phrase), phrase, &mut text, view.rune_sight);
        }
        for recipe in &table.recipes {
            if view.can_wield(recipe.spell) {
                pick(ui, adventure::spell_name(recipe.spell), &recipe.phrase, &mut text, view.rune_sight);
            }
        }
    });
    egui::CollapsingHeader::new(RichText::new("Rune Key").size(12.0).color(style::PAPER))
        .id_salt("wand_rune_key")
        .default_open(std::env::var_os("VOXEL_REVIEW_RUNEKEY").is_some())
        .show(ui, |ui| super::rune_glyphs::key(ui, view.rune_sight, view.wand, &used, &mut text));
    ui.ctx().data_mut(|d| d.insert_temp(id, text));
    if std::env::var_os("VOXEL_REVIEW_RUNEKEY").is_some() {
        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
    }
}
