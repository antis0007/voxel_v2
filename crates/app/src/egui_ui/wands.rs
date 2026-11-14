use super::{
    UiAction,
    icons::{self, Icon},
    presentation as style,
};
use crate::world::adventure::{self, View, wands};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, vec2};

pub(super) mod board;
mod rune_desk;
mod rune_glyphs;

/// Every hover card uses this width. A card that grows to fit the spell under
/// the pointer resizes the panel around it.
const HOVER_W: f32 = 304.0;

/// Fixed library tabs. Keeping one category visible at a time makes the board
/// short enough to stay put while a spell is dragged from it.
const PALETTE_COUNT: usize = adventure::spell_defs::Palette::TABS.len();
static PALETTE_TABS: std::sync::LazyLock<[(&str, Vec<u32>); PALETTE_COUNT]> =
    std::sync::LazyLock::new(|| {
        adventure::spell_defs::Palette::TABS.map(|(palette, title)| {
            let spells = adventure::spell_defs::SPELLS.iter()
                .filter(|spell| spell.palette == palette)
                .map(|spell| spell.id)
                .collect();
            (title, spells)
        })
    });

#[derive(Clone, Copy, Debug)]
pub(super) enum WandDrag {
    DeckSlot(usize, u32),
    PaletteSpell(u32),
}

/// One painted row of fixed height. Text is clipped instead of wrapping, so a
/// longer value cannot make the card taller.
fn stat_row(ui: &mut egui::Ui, label: &str, value: String, value_color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(HOVER_W, 21.0), egui::Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::monospace(11.0),
        style::MUTED,
    );
    painter.text(
        rect.right_center(),
        Align2::RIGHT_CENTER,
        value,
        FontId::monospace(12.0),
        value_color,
    );
}

fn clipped_line(ui: &mut egui::Ui, text: &str, font: FontId, color: Color32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(HOVER_W, height), egui::Sense::hover());
    ui.painter().with_clip_rect(rect).text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        text,
        font,
        color,
    );
}

fn signed_sec(v: f32) -> String {
    if v.abs() < 0.005 {
        "0.00 s".to_string()
    } else if v > 0.0 {
        format!("+{v:.2} s")
    } else {
        format!("{v:.2} s")
    }
}

fn shot_tags(mods: &wands::ShotMods) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if mods.homing && !mods.seek_ally {
        tags.push("Homing");
    }
    if mods.seek_ally {
        tags.push("Seek");
    }
    if mods.orbit || mods.bounces > 0 {
        tags.push("Ricochet");
    }
    if mods.bore {
        tags.push("Bore");
    }
    if mods.trail {
        tags.push("Wake");
    }
    if mods.volatile {
        tags.push("Volatile");
    }
    if mods.siphon {
        tags.push("Siphon");
    }
    if mods.blood {
        tags.push("Blood");
    }
    if mods.unstable {
        tags.push("Unstable");
    }
    match mods.path {
        1 => tags.push("Curve"),
        2 => tags.push("Wiggle"),
        3 => tags.push("Arc"),
        _ => {}
    }
    if mods.radius > 1.05 {
        tags.push("Wide");
    }
    if mods.life_mul > 1.05 {
        tags.push("Long");
    }
    if mods.speed_mul > 1.05 {
        tags.push("Fast");
    }
    if mods.damage_mul > 1.05 {
        tags.push("Heavy");
    }
    if mods.pierce > 0 {
        tags.push("Pierce");
    }
    if let Some(element) = mods.element {
        tags.push(match element {
            crate::world::adventure::status::Element::Fire => "Fire",
            crate::world::adventure::status::Element::Frost => "Frost",
            crate::world::adventure::status::Element::Shock => "Shock",
            crate::world::adventure::status::Element::Toxic => "Toxin",
            crate::world::adventure::status::Element::Wet => "Wet",
        });
    }
    if mods.chain {
        tags.push("Chain");
    }
    if mods.shrapnel {
        tags.push("Shrapnel");
    }
    tags
}

fn preview_cast(view: &View) -> wands::CastBlockEval {
    let mut loadout = view.loadout;
    for slot in 0..wands::capacity(view.wand) {
        if !view.can_wield(loadout.slot(slot)) {
            loadout.set_slot(slot, wands::EMPTY);
        }
    }
    wands::evaluate_cast_block(
        view.wand, view.rare_spells, view.discoveries, view.rank, &loadout, loadout.cursor,
    )
}

fn cast_program(eval: &wands::CastBlockEval) -> String {
    if eval.actions.is_empty() {
        return "Next Cast · Empty".to_string();
    }
    let mut chain = Vec::new();
    for act in &eval.actions {
        let tags = shot_tags(&act.mods);
        let prefix = if tags.is_empty() {
            String::new()
        } else {
            format!("[{}] ", tags.join("+"))
        };
        let mut step = format!("{}{}", prefix, adventure::spell_name(act.spell));
        if let Some(payload) = act.payload {
            let payload_tags = shot_tags(&act.payload_mods);
            step.push_str(" → ");
            if !payload_tags.is_empty() {
                step.push('[');
                step.push_str(&payload_tags.join("+"));
                step.push_str("] ");
            }
            step.push_str(adventure::spell_name(payload));
        }
        if let Some(follow) = act.follow {
            step.push_str(" → ");
            step.push_str(adventure::spell_name(follow));
        }
        chain.push(step);
    }
    format!(
        "Next Cast · {} Mana · {}",
        adventure::spell_defs::card::amount(eval.mana_cost),
        chain.join(" + ")
    )
}

fn signed_deg(v: f32) -> String {
    if v.abs() < 0.05 {
        "0.0 DEG".to_string()
    } else if v > 0.0 {
        format!("+{v:.1} DEG")
    } else {
        format!("{v:.1} DEG")
    }
}

/// Draws visual Noita/Wand-Sim style Cast-Block brackets and Trigger-Payload tethers
/// underneath a horizontal row of wand slot rectangles (`slot_rects[0..cap]`).
pub(super) fn paint_cast_block_brackets(
    painter: &egui::Painter,
    slot_rects: &[Rect],
    eval: &wands::CastBlockEval,
) {
    if slot_rects.is_empty() || eval.actions.is_empty() {
        return;
    }

    // Collect all slot indices participating in the evaluated cast block.
    let mut block_slots: Vec<usize> = eval.modifier_slots.clone();
    for act in &eval.actions {
        if !block_slots.contains(&act.slot) {
            block_slots.push(act.slot);
        }
        if let Some(p_slot) = act.payload_slot
            && !block_slots.contains(&p_slot) {
                block_slots.push(p_slot);
            }
    }

    // 1. Primary Cast-Block Bracket when modifiers or multicasts bind 2+ slots together
    if block_slots.len() > 1 {
        let min_idx = *block_slots.iter().min().unwrap_or(&0);
        let max_idx = *block_slots.iter().max().unwrap_or(&0);
        if let (Some(r_start), Some(r_end)) = (slot_rects.get(min_idx), slot_rects.get(max_idx)) {
            let y = r_start.bottom().round() + 2.0;
            let x0 = r_start.left() + 3.0;
            let x1 = r_end.right() - 3.0;
            let stroke = Stroke::new(1.0, style::AMBER);
            painter.line_segment([Pos2::new(x0, y - 3.0), Pos2::new(x0, y)], stroke);
            painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], stroke);
            painter.line_segment([Pos2::new(x1, y), Pos2::new(x1, y - 3.0)], stroke);
            for &idx in &block_slots {
                if let Some(r) = slot_rects.get(idx) {
                    painter.rect_filled(Rect::from_min_size(Pos2::new(r.center().x.floor(), y - 1.0), vec2(1.0, 2.0)), 0, style::AMBER);
                }
            }
        }
    }

    // 2. Secondary cyan Trigger -> Payload tether arc
    for act in &eval.actions {
        if let Some(p_slot) = act.payload_slot
            && let (Some(r_carrier), Some(r_payload)) =
                (slot_rects.get(act.slot), slot_rects.get(p_slot))
            {
                let y = r_carrier.top() - 2.0;
                let c0 = Pos2::new(r_carrier.center().x, y);
                let c1 = Pos2::new(r_payload.center().x, y);
                let tether_col = Color32::from_rgb(255, 168, 72);
                let stroke = Stroke::new(1.0, tether_col);
                painter.line_segment([Pos2::new(c0.x, y + 3.0), c0], stroke);
                painter.line_segment([c0, c1], stroke);
                painter.line_segment([c1, Pos2::new(c1.x, y + 3.0)], stroke);
            }
    }
}

/// Renders a Noita-style Wand Hover Card displaying all 8 wand statistics,
/// Always-Cast passive, live next-cast-block evaluation, and the visual spell deck.
/// Every row is a fixed height, including empty ones, so the card does not
/// change size as the pointer moves across the wand.
pub(super) fn show_wand_hover_card(ui: &mut egui::Ui, view: &View) {
    let stats = crate::world::adventure::wand_items::stats_for(view.wand, &view.loadout, view.rank);
    let eval = preview_cast(view);

    ui.set_width(HOVER_W);
    ui.set_max_width(HOVER_W);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    let (header, _) = ui.allocate_exact_size(vec2(HOVER_W, 20.0), egui::Sense::hover());
    let [r, g, b] = adventure::spell_color(view.spell);
    let gem_col = Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
    ui.painter()
        .circle_filled(header.left_center() + vec2(6.0, 0.0), 4.0, gem_col);
    let header_text = ui.painter().with_clip_rect(header.shrink2(vec2(16.0, 0.0)));
    header_text.text(
        header.left_center() + vec2(16.0, 0.0),
        Align2::LEFT_CENTER,
        stats.name,
        FontId::proportional(14.0),
        style::AMBER,
    );
    header_text.text(
        header.right_center(),
        Align2::RIGHT_CENTER,
        format!("Tier {}", view.wand),
        FontId::monospace(10.5),
        style::MANA,
    );
    if !ui.input(|input| input.modifiers.shift) {
        ui.label(RichText::new(format!("{} Slots · {:.2}s Cast",
            stats.capacity, stats.cast_delay)).size(11.0).color(style::PAPER));
        super::description_copy::show(ui, engine_content::descriptions::get("wand"),
            &format!("{} mana", adventure::spell_defs::card::amount(eval.mana_cost)), false);
        ui.label(RichText::new("[Shift] Details").size(11.0).color(style::MUTED));
        return;
    }
    super::description_copy::show(ui, engine_content::descriptions::get("wand"),
        &format!("{} mana", adventure::spell_defs::card::amount(eval.mana_cost)), true);
    clipped_line(
        ui,
        stats.chassis,
        FontId::proportional(11.0),
        style::MUTED,
        16.0,
    );
    let (rule, _) = ui.allocate_exact_size(vec2(HOVER_W, 8.0), egui::Sense::hover());
    ui.painter().hline(
        rule.x_range(),
        rule.center().y,
        Stroke::new(1.0, style::EDGE),
    );

    // Noita 8-Stat Table
    stat_row(
        ui,
        "Shuffle",
        if stats.shuffle { "Yes" } else { "No" }.to_string(),
        if stats.shuffle {
            style::AMBER
        } else {
            style::JADE
        },
    );
    stat_row(
        ui,
        "Spells/Cast",
        format!("{}", stats.spells_per_cast),
        style::PAPER,
    );
    stat_row(
        ui,
        "Cast Delay",
        format!("{:.2} s  (next {:.2} s)", stats.cast_delay, eval.cast_delay),
        if eval.cast_delay < stats.cast_delay {
            style::JADE
        } else {
            style::PAPER
        },
    );
    stat_row(
        ui,
        "Recharge Time",
        format!("{:.2} s", stats.recharge_time),
        if stats.recharge_time <= 0.22 {
            style::JADE
        } else {
            style::PAPER
        },
    );
    stat_row(
        ui,
        "Mana Max",
        format!("{:.0}", stats.mana_max),
        style::MANA,
    );
    stat_row(
        ui,
        "Mana Charge Spd",
        format!("{:.0}/s", stats.mana_charge_speed),
        style::MANA,
    );
    stat_row(ui, "Capacity", format!("{}", stats.capacity), style::PAPER);
    stat_row(
        ui,
        "Spread",
        format!("{:.1} DEG", stats.spread_deg),
        if stats.spread_deg <= 0.5 {
            style::JADE
        } else {
            style::AMBER
        },
    );

    let always_name = stats
        .always_cast
        .map(adventure::spell_name)
        .unwrap_or("None");
    let (always_rect, _) = ui.allocate_exact_size(vec2(HOVER_W, 18.0), egui::Sense::hover());
    let always_paint = ui.painter().with_clip_rect(always_rect);
    always_paint.text(
        always_rect.left_center(),
        Align2::LEFT_CENTER,
        "Always Casts",
        FontId::monospace(11.0),
        style::AMBER,
    );
    always_paint.text(
        always_rect.right_center() - vec2(18.0, 0.0),
        Align2::RIGHT_CENTER,
        always_name,
        FontId::monospace(11.0),
        style::AMBER,
    );
    if let Some(always) = stats.always_cast {
        let icon = Rect::from_center_size(
            always_rect.right_center() - vec2(8.0, 0.0),
            vec2(16.0, 16.0),
        );
        icons::paint(ui.painter(), icon, Icon::Spell(always));
    }

    let (rule, _) = ui.allocate_exact_size(vec2(HOVER_W, 8.0), egui::Sense::hover());
    ui.painter().hline(
        rule.x_range(),
        rule.center().y,
        Stroke::new(1.0, style::EDGE),
    );

    // One reserved row. Slots are painted inside it and never wrap onto a
    // second line, which used to grow the card under the pointer.
    let (deck, _) = ui.allocate_exact_size(vec2(HOVER_W, 28.0), egui::Sense::hover());
    let cap = wands::capacity(view.wand).min(8);
    let mut slot_rects = Vec::with_capacity(cap);
    for idx in 0..cap {
        let spell = view.loadout.slot(idx);
        let slot = Rect::from_min_size(
            deck.left_top() + vec2(idx as f32 * 27.0, 2.0),
            vec2(24.0, 24.0),
        );
        slot_rects.push(slot);
        icons::frame(ui.painter(), slot, idx == view.loadout.cursor);
        if spell != wands::EMPTY {
            icons::paint(ui.painter(), slot.shrink(2.5), Icon::Spell(spell));
        } else {
            ui.painter()
                .rect_filled(slot.shrink(3.0), 0, Color32::from_black_alpha(175));
        }
        ui.painter().text(
            slot.left_top() + vec2(2.0, 1.0),
            Align2::LEFT_TOP,
            format!("{}", idx + 1),
            FontId::proportional(10.0),
            if idx == view.loadout.cursor {
                style::AMBER
            } else {
                style::MUTED
            },
        );
    }
    paint_cast_block_brackets(ui.painter(), &slot_rects, &eval);

    let program = cast_program(&eval);
    let program_color = if eval.actions.is_empty() {
        style::MUTED
    } else if eval.mana_cost > view.mana {
        style::DANGER
    } else {
        style::MANA
    };
    clipped_line(ui, &program, FontId::monospace(10.5), program_color, 16.0);
}

/// Compact spell card: automatic stats, functional copy, cost, and Alt details.
fn review_card_config() -> Option<(u32, bool)> {
    static REVIEW: std::sync::OnceLock<Option<(u32, bool)>> = std::sync::OnceLock::new();
    *REVIEW.get_or_init(|| {
        if !crate::parse_benchmark_config().enabled || super::grimoire::review_page().is_none() {
            return None;
        }
        let spell = std::env::var("VOXEL_REVIEW_SPELL_CARD").ok()?.parse().ok()?;
        adventure::spell_defs::def(spell)?;
        let expanded = std::env::var("VOXEL_REVIEW_CODEX").is_ok_and(|value| value == "1");
        Some((spell, expanded))
    })
}

/// Captures the real hover-card renderer in an unsaved benchmark review only.
pub(super) fn review_spell_card(ctx: &egui::Context, view: &View) {
    let Some((spell, _)) = review_card_config() else { return; };
    egui::Area::new(egui::Id::new("spell-card-native-review"))
        .order(egui::Order::Foreground)
        .anchor(Align2::RIGHT_TOP, vec2(-24.0, 64.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(style::INK)
                .stroke(Stroke::new(1.0, style::EDGE))
                .inner_margin(12)
                .show(ui, |ui| show_spell_hover_card(ui, spell, view));
        });
}

pub(super) fn show_spell_hover_card(ui: &mut egui::Ui, spell: u32, view: &View) {
    let available = view.can_wield(spell);
    let is_mod = wands::is_modifier(spell);

    ui.set_width(HOVER_W);
    ui.set_max_width(HOVER_W);
    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
    ui.horizontal_top(|ui| {
        let (icon, _) = ui.allocate_exact_size(vec2(36.0, 36.0), egui::Sense::hover());
        icons::frame(ui.painter(), icon, true);
        icons::paint(ui.painter(), icon.shrink(3.0), Icon::Spell(spell));
        ui.vertical(|ui| {
            ui.set_width(HOVER_W - 44.0);
            ui.label(RichText::new(adventure::spell_name(spell)).size(16.0).color(style::PAPER));
            ui.label(RichText::new(adventure::spell_kind_tag(spell)).size(12.0)
                .color(if is_mod { style::AMBER } else { style::MANA }));
        });
    });
    ui.add_space(4.0);

    if let Some(entry) = adventure::spell_defs::card::description(spell) {
        compact_spell_body(ui, spell, view, entry, available);
    }
}

/// Migrated cards use the same sparse, validated copy as inventory items.
/// Values are generated by casting rules; Shift expands numeric mechanics.
fn compact_spell_body(
    ui: &mut egui::Ui,
    spell: u32,
    view: &View,
    entry: &engine_content::descriptions::Description,
    available: bool,
) {
    let card = adventure::spell_defs::card::read(spell, view.discoveries, view.rank);
    ui.spacing_mut().item_spacing.y = 6.0;
    if !card.stats.is_empty() {
        ui.label(RichText::new(&card.stats).size(13.0).color(style::PAPER));
    }
    let expanded = ui.input(|input| input.modifiers.shift)
        || review_card_config().is_some_and(|(id, expanded)| id == spell && expanded);
    let cost = adventure::concentration::requirements(spell).map_or_else(|| card.cost.clone(), |(upkeep, _)| {
        let amount = adventure::spell_defs::card::amount;
        let upkeep = adventure::concentration::binding_upkeep(upkeep, view.loadout.mode);
        format!("{} mana + {} mana/s", amount(adventure::inscriptions::cost(spell, view.discoveries)), amount(upkeep))
    });
    super::description_copy::show(ui, Some(entry), &cost, expanded);
    if spell == wands::EMPTY { return; }
    if spell == adventure::fieldcraft::RECALL {
        let cast = crate::keybinds::hint(crate::keybinds::Action::Cast);
        let sneak = crate::keybinds::hint(crate::keybinds::Action::Sneak);
        ui.weak(format!("Starting spell · Single card · [{sneak}] + [{cast}] Draw link"));
    }
    if adventure::concentration::requirements(spell).is_some() {
        let note = if wands::is_grimoire(view.loadout.mode) { "Hold Cast · Single card" } else { "Grimoire required" };
        ui.label(RichText::new(note).size(12.0).color(style::MANA));
    }
    if !available {
        if !view.spell_log.grants(spell) {
            let source = crate::world::progression::study::Lesson::for_spell(spell)
                .map_or_else(|| "Site clears · first guardian defeats".to_owned(),
                    |lesson| format!("Bookcase · {}", lesson.title()));
            ui.label(RichText::new(source).size(10.0).color(style::AMBER));
        }
        if !adventure::spell_available(spell, view.wand, view.rare_spells) {
            let message = match spell {
                6 => "Source: Archive reliquary".to_owned(),
                7 => "Source: Crystal cathedral reliquary".to_owned(),
                _ => format!("Wand tier: {}", adventure::discovery::spell_tier(spell)),
            };
            ui.label(RichText::new(message).size(10.0).color(style::AMBER));
        }
    }
    if expanded {
        ui.separator();
        for (label, value) in card.details {
            stat_row(ui, label, value, style::PAPER);
        }
        for (label, value) in [
            ("Cast delay", adventure::spell_cast_delay_delta(spell)),
            ("Recharge", adventure::spell_recharge_delta(spell)),
        ] {
            if value != 0.0 {
                stat_row(ui, label, signed_sec(value), if value < 0.0 { style::JADE } else { style::AMBER });
            }
        }
        let spread = adventure::spell_spread_delta(spell);
        if spread != 0.0 {
            stat_row(ui, "Spread", signed_deg(spread), style::PAPER);
        }
    } else {
        ui.add_space(2.0);
        ui.label(RichText::new("[Shift] Details").size(11.0).color(style::MUTED));
    }
}

/// The wand page; see `board` for the layout.
pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    board::draw(ui, view, action);
}

/// How many of these spells the player has found. Spells held back by rare
/// magic (6 and 7) count as found, matching what the log lets through.
fn found_count(spells: &[u32], log: &crate::world::adventure::discovery::SpellLog) -> usize {
    spells.iter().filter(|&&id| log.grants(id)).count()
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    /// A spell that exists but is on no tab can never be placed on a wand, which
    /// is exactly how a new spell silently ships unusable.
    #[test]
    fn every_defined_spell_is_reachable_from_the_palette() {
        let listed: std::collections::HashSet<u32> = PALETTE_TABS
            .iter()
            .flat_map(|(_, list)| list.iter().copied())
            .collect();
        for id in 0..=adventure::MAX_SPELL_ID {
            if id == 8 || adventure::spell_name(id) == "Unknown spell" {
                continue;
            }
            assert!(listed.contains(&id), "{} ({id}) is on no palette tab", adventure::spell_name(id));
        }
        for id in &listed {
            assert_ne!(adventure::spell_name(*id), "Unknown spell", "tab lists undefined spell {id}");
        }
    }

    #[test]
    fn found_count_reflects_the_log() {
        use crate::world::adventure::discovery::SpellLog;
        let start = SpellLog::starter();
        let list = &PALETTE_TABS[0].1;
        assert_eq!(found_count(list, &SpellLog::all()), list.len());
        // Starter: Spark and Frost lance are bolts; Spore bloom is not found,
        // while rift well (6) is never blocked by the log.
        let bolts = found_count(list, &start);
        assert_eq!(bolts, 3, "spark, frost lance and rift well");
        assert_eq!(found_count(&[3, 5, 29], &start), 0);
    }
}
