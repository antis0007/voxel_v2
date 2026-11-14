//! Brewing stand widgets: capsule gauge, state pill, recipe pills.
use super::block_page::slot_sized;
use super::presentation as style;
use crate::world::block_data::Stand;
use crate::world::inventory::{FoodKind, Item, SlotAddr};
use crate::world::stations::{brewing, table::BREWING};
use egui::{Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, vec2};

const GAUGE: egui::Vec2 = egui::vec2(104.0, 20.0);
const RECIPE_SLOT: f32 = 24.0;
/// Nine 40 px slots with 2 px gaps: the width of the pack grid below.
pub(super) const PAGE_WIDTH: f32 = 376.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase {
    Idle,
    NoRecipe,
    OutputFull,
    NoWater,
    /// Seconds until the batch finishes.
    Brewing(f32),
}

impl Phase {
    pub(super) fn label(self) -> String {
        match self {
            Self::Idle => "Idle".into(),
            Self::NoRecipe => "No Recipe".into(),
            Self::OutputFull => "Output Full".into(),
            Self::NoWater => "No Water".into(),
            Self::Brewing(s) => format!("Brewing · {s:.1}s"),
        }
    }

    pub(super) fn colour(self) -> Color32 {
        match self {
            Self::Idle => style::MUTED,
            Self::Brewing(_) => style::JADE,
            Self::NoRecipe | Self::OutputFull | Self::NoWater => style::DANGER,
        }
    }
}

pub(super) fn phase(stand: &Stand, water: bool) -> Phase {
    let (Some(base), Some(modifier)) = (stand.base, stand.modifier) else { return Phase::Idle };
    let Some(recipe) = brewing::resolve(base.item, modifier.item) else { return Phase::NoRecipe };
    let out = Item::Food(recipe.output);
    if !stand.output.is_none_or(|o| o.item == out && o.count + recipe.count <= out.max_stack()) {
        return Phase::OutputFull;
    }
    if !water {
        return Phase::NoWater;
    }
    let left = brewing::brew_steps().saturating_sub(stand.steps);
    Phase::Brewing(f32::from(left) * crate::simulation::STEP_SECONDS)
}

/// Liquid colour per potion; the same hues as the potion icons.
pub(super) fn potion_colour(kind: FoodKind) -> Color32 {
    match kind {
        FoodKind::RegenerationPotion => Color32::from_rgb(205, 92, 160),
        FoodKind::SpeedPotion => Color32::from_rgb(120, 190, 225),
        FoodKind::StrengthPotion => Color32::from_rgb(170, 52, 48),
        FoodKind::FireResistancePotion => Color32::from_rgb(232, 150, 60),
        FoodKind::HealingPotion => Color32::from_rgb(224, 74, 82),
        FoodKind::ResistancePotion => Color32::from_rgb(118, 132, 156),
        FoodKind::LeapingPotion => Color32::from_rgb(106, 174, 58),
        FoodKind::HastePotion => Color32::from_rgb(212, 172, 48),
        _ => style::MUTED,
    }
}

/// Colour of the batch the stand holds, if its slots name a recipe.
pub(super) fn batch_colour(stand: &Stand) -> Color32 {
    stand
        .base
        .zip(stand.modifier)
        .and_then(|(b, m)| brewing::resolve(b.item, m.item))
        .map_or(style::MUTED, |r| potion_colour(r.output))
}

fn capsule(height: f32) -> CornerRadius {
    CornerRadius::same((height / 2.0).round().clamp(0.0, 255.0) as u8)
}

/// Horizontal capsule filled to `progress` with bubbles rising while `active`.
pub(super) fn gauge(ui: &mut egui::Ui, progress: f32, colour: Color32, active: bool) {
    let (rect, _) = ui.allocate_exact_size(GAUGE, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, capsule(rect.height()), style::INK);
    let inner = rect.shrink(3.0);
    let progress = progress.clamp(0.0, 1.0);
    let fill_w = if progress > 0.0 { (inner.width() * progress).max(inner.height()) } else { 0.0 };
    let fill = Rect::from_min_size(inner.min, vec2(fill_w, inner.height()));
    if fill_w > 0.0 {
        painter.rect_filled(fill, capsule(inner.height()), colour);
        let sheen = Rect::from_min_size(fill.min + vec2(inner.height() / 2.0, 2.0), vec2((fill_w - inner.height()).max(0.0), 2.0));
        painter.rect_filled(sheen, CornerRadius::same(1), Color32::from_white_alpha(70));
    }
    if active && fill_w > 0.0 {
        let t = ui.input(|i| i.time) as f32;
        for k in 0..4 {
            let phase = (t * 0.7 + k as f32 * 0.27).fract();
            let x = fill.left() + inner.height() / 2.0 + ((k as f32 * 0.31 + 0.13).fract()) * (fill_w - inner.height()).max(1.0);
            let y = inner.bottom() - 2.0 - phase * (inner.height() - 4.0);
            painter.circle_filled(egui::pos2(x, y), 1.5, Color32::from_white_alpha(150));
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
    }
    painter.rect_stroke(rect, capsule(rect.height()), Stroke::new(1.0, style::EDGE), StrokeKind::Inside);
}

/// Text in a capsule outlined in its own colour.
pub(super) fn pill(ui: &mut egui::Ui, text: &str, colour: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(12.0), colour);
    let size = galley.size() + vec2(18.0, 6.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, capsule(rect.height()), colour.gamma_multiply(0.16));
    painter.rect_stroke(rect, capsule(rect.height()), Stroke::new(1.0, colour), StrokeKind::Inside);
    painter.galley(rect.center() - galley.size() / 2.0, galley, colour);
    response
}

/// One recipe as `base + modifier = potion`; true when clicked.
fn recipe_pill(ui: &mut egui::Ui, recipe: &crate::world::stations::table::Brew, ready: bool) -> bool {
    let colour = if ready { potion_colour(recipe.output) } else { style::EDGE };
    let mut clicked = false;
    let frame = egui::Frame::new()
        .fill(colour.gamma_multiply(if ready { 0.18 } else { 0.06 }))
        .stroke(Stroke::new(1.0, colour))
        .corner_radius(CornerRadius::same(15))
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let stack = |item| Some(crate::world::inventory::ItemStack::new(item, 1));
                clicked |= slot_sized(ui, stack(recipe.base), RECIPE_SLOT).clicked();
                ui.label(egui::RichText::new("+").color(style::MUTED));
                clicked |= slot_sized(ui, stack(recipe.modifier), RECIPE_SLOT).clicked();
                ui.label(egui::RichText::new("=").color(style::MUTED));
                let out = crate::world::inventory::ItemStack::new(Item::Food(recipe.output), recipe.count);
                clicked |= slot_sized(ui, Some(out), RECIPE_SLOT).clicked();
            });
        });
    clicked | (ready && ui.interact(frame.response.rect, frame.response.id, Sense::click()).clicked())
}

/// Recipe pills; returns the pack slots to load when a ready pill is clicked.
pub(super) fn recipes(ui: &mut egui::Ui, stand: &Stand, pack: &[Option<crate::world::inventory::ItemStack>], addr: impl Fn(usize) -> SlotAddr) -> Vec<SlotAddr> {
    let mut load = Vec::new();
    let has = |item: Item, held: Option<crate::world::inventory::ItemStack>| {
        held.is_some_and(|s| s.item == item) || pack.iter().flatten().any(|s| s.item == item)
    };
    // Frames do not wrap inside a wrapped layout, so rows are laid out by hand.
    for row in BREWING.recipes.chunks(2) {
        ui.horizontal(|ui| {
            for recipe in row {
                let ready = has(recipe.base, stand.base) && has(recipe.modifier, stand.modifier);
                if recipe_pill(ui, recipe, ready) && ready {
                    for (item, held) in [(recipe.base, stand.base), (recipe.modifier, stand.modifier)] {
                        if held.is_some_and(|s| s.item == item) {
                            continue;
                        }
                        if let Some(i) = pack.iter().position(|s| s.is_some_and(|s| s.item == item)) {
                            load.push(addr(i));
                        }
                    }
                }
            }
        });
    }
    load
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::inventory::ItemStack;

    fn loaded(base: Item, modifier: Item) -> Stand {
        Stand { base: Some(ItemStack::new(base, 1)), modifier: Some(ItemStack::new(modifier, 1)), output: None, steps: 0 }
    }

    fn first() -> (Item, Item, FoodKind) {
        let r = BREWING.recipes.first().expect("a brewing row");
        (r.base, r.modifier, r.output)
    }

    #[test]
    fn phase_names_the_blocking_state() {
        let (b, m, out) = first();
        assert_eq!(phase(&Stand::default(), true), Phase::Idle);
        assert_eq!(phase(&loaded(b, b), true), Phase::NoRecipe);
        assert_eq!(phase(&loaded(b, m), false), Phase::NoWater);
        let mut full = loaded(b, m);
        full.output = Some(ItemStack::new(Item::Food(out), Item::Food(out).max_stack()));
        assert_eq!(phase(&full, true), Phase::OutputFull);
        assert!(matches!(phase(&loaded(b, m), true), Phase::Brewing(s) if (s - BREWING.seconds).abs() < 0.1));
    }

    #[test]
    fn brewing_time_counts_down_with_the_step_counter() {
        let (b, m, _) = first();
        let mut stand = loaded(b, m);
        stand.steps = brewing::brew_steps() / 2;
        let Phase::Brewing(left) = phase(&stand, true) else { panic!("brewing") };
        assert!((left - BREWING.seconds / 2.0).abs() < 0.1, "{left}");
    }

    #[test]
    fn every_potion_has_its_own_liquid_colour() {
        let colours: Vec<_> = BREWING.recipes.iter().map(|r| potion_colour(r.output)).collect();
        for (i, c) in colours.iter().enumerate() {
            assert_ne!(*c, style::MUTED, "row {i}");
            assert!(colours[..i].iter().all(|o| o != c), "row {i} repeats a colour");
        }
    }

    #[test]
    fn labels_follow_the_state_grammar() {
        for p in [Phase::Idle, Phase::NoRecipe, Phase::OutputFull, Phase::NoWater, Phase::Brewing(12.34)] {
            let text = p.label();
            assert!(!text.contains('!') && text.chars().next().is_some_and(char::is_uppercase), "{text}");
        }
        assert_eq!(Phase::Brewing(12.34).label(), "Brewing · 12.3s");
    }
}
