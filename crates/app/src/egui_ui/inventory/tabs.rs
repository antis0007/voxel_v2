//! Labelled tabs across the top of the inventory window, in four groups
//! (Craft, Magic, Factory, World). A tab appears when the player reaches the
//! system it opens; the page on screen and every page in creative stay listed.
use super::{screen, unlocks::Unlocks};
use crate::egui_ui::icons::{self, Icon};
use crate::world::inventory::Item;
use engine_content::BlockId;
use egui::{Align2, FontId, Rect, Sense, Vec2, pos2, vec2};

enum Glyph {
    Item(Item),
    Icon(Icon),
    Book,
    Orbit,
    Chart,
    Globe,
}

const CRAFT: usize = 0;
const MAGIC: usize = 1;
const FACTORY: usize = 2;
const WORLD: usize = 3;
const GROUPS: [&str; 4] = ["Craft", "Magic", "Factory", "World"];

/// Group, page id, label, tooltip, icon. Order is the order on screen.
const TABS: [(usize, u32, &str, &str, Glyph); 11] = [
    (CRAFT, 2, "Inventory", "Inventory", Glyph::Item(Item::Block(BlockId::StorageCrate))),
    (CRAFT, 6, "Recipes", "Recipes · Uses · Recipe Tree", Glyph::Book),
    (CRAFT, 0, "Workbench", "Workbench", Glyph::Item(Item::Block(BlockId::Workbench))),
    (CRAFT, 5, "Forge", "Forge", Glyph::Icon(Icon::Sword(2))),
    (MAGIC, 1, "Wand", "Wand", Glyph::Icon(Icon::Wand(2))),
    (FACTORY, 3, "Machines", "Machines", Glyph::Item(Item::Block(BlockId::Crusher))),
    (FACTORY, 7, "Production", "Production", Glyph::Item(Item::Block(BlockId::CoalGenerator))),
    (WORLD, 8, "Orbital", "Orbital", Glyph::Orbit),
    (WORLD, 10, "Statistics", "Statistics", Glyph::Chart),
    (WORLD, MAPS, "Maps", "Maps", Glyph::Globe),
    (WORLD, 4, "Creative", "Creative", Glyph::Item(Item::Block(BlockId::Grass))),
];

const CATALOGUE: u32 = 4;
const WAND: u32 = 1;
/// Not a page: opens the map hub (region, planet, system) over the game.
pub(super) const MAPS: u32 = 11;
/// Tab height in GUI units: an 11-unit icon on its original pixel grid and a pixel of margin.
/// Only the selected tab carries its label; the rest are icons with a tooltip.
const HEIGHT: f32 = 10.0;
/// Icon edge in GUI units.
const ICON: f32 = 8.0;
/// Gap between groups, in GUI units.
const GROUP_GAP: f32 = 3.0;

type Tab = (usize, u32, &'static str, &'static str, Glyph);

/// Draws the groups and returns the page clicked this frame, if any.
pub(super) fn strip(ui: &mut egui::Ui, page: u32, unit: f32, catalogue: bool, unlocks: Unlocks) -> Option<u32> {
    let systems = crate::world::systems::Systems::active();
    let shown = |t: &Tab| {
        (catalogue || t.1 != CATALOGUE) && systems.tab_visible(t.1) && (unlocks.page_open(t.1) || t.1 == page)
    };
    // Reserve the close control and keep the additional records/guide tabs
    // inside the pack window at smaller logical widths.
    // The wand page puts a Guide button beside the close control.
    let reserved = match page {
        WAND => 30.0 + 64.0,
        _ => 30.0,
    };
    let avail = (ui.available_width() - reserved).max(80.0 * unit);
    let font = FontId::proportional((3.5 * unit).clamp(9.0, 13.0));
    let tab_h = HEIGHT * unit;
    let row_h = tab_h + unit;
    // Icon-only tabs; the selected one widens to carry its label. A group
    // that does not fit starts a new row.
    let tab_w = |label: &str, selected: bool| {
        if !selected {
            return HEIGHT * unit;
        }
        let text = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), screen::LABEL).size().x;
        text + (ICON + 4.0) * unit
    };
    let mut placed: Vec<(f32, usize, f32, &Tab)> = Vec::new();
    let (mut x, mut row) = (0.0_f32, 0usize);
    for g in 0..GROUPS.len() {
        let tabs: Vec<&Tab> = TABS.iter().filter(|t| t.0 == g && shown(t)).collect();
        if tabs.is_empty() {
            continue;
        }
        let widths: Vec<f32> = tabs.iter().map(|t| tab_w(t.2, page == t.1)).collect();
        let group_w = widths.iter().sum::<f32>() + unit * (widths.len() as f32 - 1.0);
        if x > 0.0 && x + group_w > avail {
            x = 0.0;
            row += 1;
        }
        for (t, w) in tabs.into_iter().zip(widths) {
            placed.push((x, row, w, t));
            x += w + unit;
        }
        x += (GROUP_GAP - 1.0) * unit;
    }
    let rows = row + 1;
    let (outer, _) = ui.allocate_exact_size(vec2(avail, rows as f32 * row_h - unit), Sense::hover());
    let at = |x: f32, row: usize| pos2(outer.left() + x, outer.top() + row as f32 * row_h);
    let mut picked = None;
    for (x, row, w, tab) in placed {
        let (_, id, label, tip, glyph) = tab;
        let selected = page == *id;
        let rect = Rect::from_min_size(at(x, row) + vec2(0.0, 0.0), vec2(w, tab_h));
        let response = ui.interact(rect, egui::Id::new(("inventory_tab", *id)), Sense::click());
        let p = ui.painter();
        screen::tab(p, rect, unit, selected);
        let icon_x = if selected { rect.left() + 1.0 * unit } else { rect.center().x - ICON * 0.5 * unit };
        let icon = Rect::from_min_size(
            pos2(icon_x, rect.top() + 1.0 * unit),
            Vec2::splat(ICON * unit),
        );
        match glyph {
            Glyph::Item(item) => super::draw_item_icon(p, icon, *item),
            Glyph::Icon(glyph) => icons::paint(p, icon, *glyph),
            Glyph::Book => screen::book_glyph(p, icon.min, icon.width() / 11.0),
            Glyph::Orbit => screen::orbit_glyph(p, icon.min, icon.width() / 11.0),
            Glyph::Chart => screen::chart_glyph(p, icon.min, icon.width() / 11.0),
            Glyph::Globe => screen::globe_glyph(p, icon.min, icon.width() / 11.0),
        }
        if selected {
            p.text(
                pos2(icon.right() + 1.5 * unit, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                font.clone(),
                crate::egui_ui::grimoire::GOLD,
            );
        }
        if response.hovered() && !selected {
            screen::hover(p, rect, unit);
        }
        if response.on_hover_text(*tip).clicked() && !selected {
            picked = Some(*id);
        }
    }
    picked
}
