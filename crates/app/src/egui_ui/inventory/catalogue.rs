//! Creative category navigation and compact item slots. No inventory writes here.
use super::*;

pub(super) fn matches(item: Item, query: &str) -> bool {
    let group = match item {
        Item::Block(block) => category(block),
        _ => creative_item_group(item),
    };
    query
        .split_whitespace()
        .all(|term| contains_ignore_case(item.name(), term) || contains_ignore_case(group, term))
}

pub(super) fn tabs(ui: &mut egui::Ui, mode: &mut u8, group: &mut String) -> bool {
    let mut changed = false;
    let tabs = [
        (0, "All", "Blocks", Item::Block(BlockId::Grass)),
        (0, "Terrain", "Terrain", Item::Block(BlockId::Stone)),
        (0, "Building", "Building", Item::Block(BlockId::Planks)),
        (0, "Nature", "Nature", Item::Block(BlockId::Sapling)),
        (0, "Lighting", "Lighting", Item::Block(BlockId::Torch)),
        (0, "Machines", "Machines", Item::Block(BlockId::Crusher)),
        (1, "All", "Items", Item::Resource(8)),
        (1, "Materials", "Materials", Item::Resource(6)),
        (1, "Weapons", "Weapons", Item::Weapon(2)),
        (
            1,
            "Armour",
            "Armour",
            Item::Armor(EquipSlot::Head, ArmorTier::Steel),
        ),
        (1, "Tools", "Tools", Item::Tool(8)),
        (
            1,
            "Food",
            "Food",
            Item::Food(crate::world::inventory::FoodKind::ALL[0]),
        ),
    ];
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
        for (tab_mode, tab_group, label, item) in tabs {
            let selected = *mode == tab_mode && group.as_str() == tab_group;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
            screen::tab(ui.painter(), rect, 1.0, selected);
            draw_item_icon(ui.painter(), rect.shrink(4.0), item);
            if response.on_hover_text(label).clicked() {
                *mode = tab_mode;
                *group = tab_group.into();
                changed = true;
            }
        }
    });
    changed
}

pub(super) fn slot(ui: &mut egui::Ui, size: [f32; 2], item: Item) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(size[0], size[1]), egui::Sense::click_and_drag());
    screen::cell(ui.painter(), rect);
    // One unit per side: the vanilla 16-in-18 icon fill (32 of 36 points).
    draw_item_icon(ui.painter(), rect.shrink(rect.width() / 18.0), item);
    if response.hovered() {
        screen::hover(ui.painter(), rect, 1.0);
    }
    response
}

pub(super) fn shortcut(ui: &mut egui::Ui, response: &egui::Response) -> Option<usize> {
    if !response.hovered() || ui.ctx().egui_wants_keyboard_input() {
        return None;
    }
    [
        egui::Key::Num1,
        egui::Key::Num2,
        egui::Key::Num3,
        egui::Key::Num4,
        egui::Key::Num5,
        egui::Key::Num6,
        egui::Key::Num7,
        egui::Key::Num8,
        egui::Key::Num9,
    ]
    .into_iter()
    .position(|key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)))
}
