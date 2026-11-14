use super::*;
use crate::world::{BlockId, inventory::ItemStack};

fn target(item: Item, amount: u32) -> Target {
    Target { item, amount, choices: Choices::new() }
}

#[test]
fn pinned_targets_bound_duplicates_updates_and_removal() {
    let ctx = egui::Context::default();
    for n in 0..LIMIT { assert!(put(&ctx, target(Item::Resource(n as u8), 1))); }
    assert!(!put(&ctx, target(Item::Resource(10), 1)));
    assert!(put(&ctx, target(Item::Resource(0), 64)));
    let pins = state(&ctx);
    assert_eq!(pins.targets.len(), LIMIT);
    assert_eq!(pins.targets[0].amount, 64);
    assert_eq!(pins.active, Some(Item::Resource(0)));
    remove(&ctx, Item::Resource(0));
    assert_eq!(state(&ctx).active, Some(Item::Resource(7)));
    for n in 1..LIMIT { remove(&ctx, Item::Resource(n as u8)); }
    assert!(state(&ctx).active.is_none());
}

#[test]
fn pinned_targets_stock_progress_cache_and_completion_are_read_only() {
    let ctx = egui::Context::default();
    let target = target(Item::Resource(0), 3);
    let mut inv = SurvivalInventory::empty();
    inv.pack[0] = Some(ItemStack::new(target.item, 1));
    inv.resource_remainders[0] = 32;
    let before = inv.clone();
    let first = plan(&ctx, &inv, false, 1, &target);
    assert_eq!(first.from_stock, 1.5);
    assert_eq!(first.missing(), vec![(target.item, 1.5)]);
    assert!(Arc::ptr_eq(&first, &plan(&ctx, &inv, false, 1, &target)));
    assert_eq!(inv, before);
    inv.pack[0] = Some(ItemStack::new(target.item, 3));
    let complete = plan(&ctx, &inv, false, 1, &target);
    assert!(!Arc::ptr_eq(&first, &complete));
    assert_eq!(complete.from_stock, complete.need);
    assert!(complete.missing().is_empty());
    let resized = plan(&ctx, &inv, false, 4, &target);
    assert!(!Arc::ptr_eq(&complete, &resized));
    let bench = plan(&ctx, &inv, true, 4, &target);
    assert!(!Arc::ptr_eq(&bench, &resized));
}

#[test]
fn pinned_targets_placed_bench_is_not_a_carried_goal() {
    let ctx = egui::Context::default();
    let inv = SurvivalInventory::empty();
    let target = target(Station::Bench.icon(), 1);
    let result = plan(&ctx, &inv, true, 1, &target);
    assert_eq!(result.from_stock, 0.0);
    assert!(!result.missing().is_empty());
}

#[test]
fn pinned_targets_reopen_restores_amount_choices_and_tree_after_browsing() {
    let ctx = egui::Context::default();
    let mut goal = target(Item::Block(BlockId::Torch), 16);
    goal.choices.insert(Item::Resource(6), 1);
    assert!(put(&ctx, goal.clone()));
    request(&ctx, Item::Block(BlockId::Stone), true);
    assert!(take_request(&ctx));
    open(&ctx, &goal);
    assert!(take_request(&ctx));
    assert!(!take_request(&ctx));
    assert_eq!(ctx.data(|d| d.get_temp::<Option<Item>>(egui::Id::new(SELECTED))), Some(Some(goal.item)));
    assert_eq!(ctx.data(|d| d.get_temp::<u8>(egui::Id::new(TAB))), Some(2));
    assert_eq!(ctx.data(|d| d.get_temp::<u32>(egui::Id::new(AMOUNT))), Some(16));
    assert_eq!(ctx.data(|d| d.get_temp::<Choices>(egui::Id::new(CHOICES))), Some(goal.choices));
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(OPEN), false));
    assert_eq!(state(&ctx).targets.len(), 1);
}

struct Harness {
    ctx: egui::Context,
    inv: SurvivalInventory,
    time: f64,
    rect: egui::Rect,
    editor: egui::Rect,
    query: String,
    draw_pins: bool,
}

impl Harness {
    fn new() -> Self {
        Self { ctx: egui::Context::default(), inv: SurvivalInventory::empty(), time: 0.0,
            rect: egui::Rect::NOTHING, editor: egui::Rect::NOTHING, query: String::new(), draw_pins: false }
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.05;
        let mut output = self.ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(640.0, 480.0))),
            time: Some(self.time), events, focused: true, ..Default::default()
        }, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                self.editor = ui.text_edit_singleline(&mut self.query).rect;
                self.rect = item_cell(ui, Item::Block(BlockId::Torch), None, 32.0,
                    None, &mut None, &self.inv).rect;
                if self.draw_pins { draw(ui, &self.inv, false, 1, &mut None); }
            });
        });
        output.textures_delta.clear();
    }
    fn click(&mut self, pos: egui::Pos2) {
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]);
        }
    }
    fn key(&mut self, repeat: bool) {
        self.frame(vec![egui::Event::PointerMoved(self.rect.center()), egui::Event::Key {
            key: egui::Key::A, physical_key: None, pressed: true, repeat, modifiers: egui::Modifiers::NONE }]);
    }
}

#[test]
fn pinned_targets_hover_shortcut_ignores_repeat_and_text_focus() {
    let mut h = Harness::new(); h.frame(vec![]); h.frame(vec![]);
    let before = h.inv.clone();
    h.key(false);
    assert!(contains(&h.ctx, Item::Block(BlockId::Torch)));
    h.key(true);
    assert!(contains(&h.ctx, Item::Block(BlockId::Torch)));
    h.frame(vec![egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: false,
        repeat: false, modifiers: egui::Modifiers::NONE }]);
    h.key(false);
    assert!(!contains(&h.ctx, Item::Block(BlockId::Torch)));
    h.click(h.editor.center());
    assert!(h.ctx.egui_wants_keyboard_input());
    h.key(false);
    assert!(!contains(&h.ctx, Item::Block(BlockId::Torch)));
    assert_eq!(h.inv, before);
}

#[test]
fn pinned_targets_pointer_reopens_plan_without_crafting() {
    let mut h = Harness::new();
    let goal = target(Item::Block(BlockId::Torch), 16);
    put(&h.ctx, goal.clone()); h.draw_pins = true;
    h.frame(vec![]); h.frame(vec![]);
    let before = h.inv.clone();
    let rect = h.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("pin_rect", goal.item)))).unwrap();
    h.click(rect.center());
    assert!(take_request(&h.ctx));
    assert_eq!(h.ctx.data(|d| d.get_temp::<u8>(egui::Id::new(TAB))), Some(2));
    assert_eq!(h.ctx.data(|d| d.get_temp::<u32>(egui::Id::new(AMOUNT))), Some(16));
    assert_eq!(h.inv, before);
}

#[test]
fn pinned_targets_partial_finished_stock_matches_open_goal_tree() {
    let ctx = egui::Context::default();
    let goal = target(Item::Block(BlockId::Torch), 16);
    let mut inv = SurvivalInventory::empty();
    inv.pack[0] = Some(ItemStack::new(goal.item, 8));
    put(&ctx, goal.clone());
    let sidebar = plan(&ctx, &inv, false, 1, &goal);
    let sidebar_missing = missing(&ctx, &sidebar);
    open(&ctx, &goal);
    assert!(take_request(&ctx));
    assert_eq!(ctx.data(|d| d.get_temp::<bool>(egui::Id::new(GOAL_VIEW))), Some(true));
    let tree = goal_plan(&ctx, &inv, false, 1, goal.item, goal.amount, &goal.choices);
    assert_eq!(tree.need - tree.from_stock, 8.0);
    assert_eq!(sidebar.missing(), tree.missing());
    assert_eq!(*sidebar_missing, *missing(&ctx, &tree));
    request(&ctx, goal.item, false);
    assert!(take_request(&ctx));
    assert_eq!(ctx.data(|d| d.get_temp::<bool>(egui::Id::new(GOAL_VIEW))), Some(false));
    assert_eq!(tree_plan(&ctx, &inv, false, goal.item, 16, &Choices::new(), 1).from_stock, 0.0);
}

#[test]
fn pinned_targets_authored_and_equipped_items_count_without_spending_gear() {
    let ctx = egui::Context::default();
    let item = Item::Wand { id: 65001, look: 0 };
    assert!(!index::known_items().contains(&item));
    let goal = target(item, 1);
    let mut inv = SurvivalInventory::empty();
    let empty = plan(&ctx, &inv, false, 1, &goal);
    assert_eq!(empty.from_stock, 0.0);
    inv.offhand = Some(ItemStack::new(item, 1));
    let before = inv.clone();
    let held = plan(&ctx, &inv, false, 1, &goal);
    assert_eq!(held.from_stock, 1.0);
    assert!(!Arc::ptr_eq(&empty, &held));
    assert!(missing(&ctx, &held).is_empty());
    assert_eq!(inv, before);
    inv.offhand = None;
    inv.equipment[0] = Some(ItemStack::new(item, 1));
    assert_eq!(plan(&ctx, &inv, false, 1, &goal).from_stock, 1.0);
    inv.equipment[0] = None;
    assert_eq!(plan(&ctx, &inv, false, 1, &goal).from_stock, 0.0);
    // An offhand log cannot become a consumed input for a different goal.
    let plank = target(Item::Block(BlockId::Planks), 4);
    let need = missing(&ctx, &plan(&ctx, &inv, false, 1, &plank));
    inv.offhand = Some(ItemStack::new(Item::Block(BlockId::Log), 64));
    assert_eq!(*missing(&ctx, &plan(&ctx, &inv, false, 1, &plank)), *need);
}

#[test]
fn pinned_targets_sidebar_and_edited_tree_keep_independent_caches() {
    let ctx = egui::Context::default();
    let inv = SurvivalInventory::empty();
    let goal = target(Item::Block(BlockId::Torch), 16);
    let sidebar = plan(&ctx, &inv, false, 1, &goal);
    let sidebar_missing = missing_cached(&ctx, &sidebar, "jei_pinned_missing");
    let editor = goal_plan(&ctx, &inv, false, 1, goal.item, 64, &goal.choices);
    let editor_missing = missing(&ctx, &editor);
    for _ in 0..30 {
        let same_sidebar = plan(&ctx, &inv, false, 1, &goal);
        assert!(Arc::ptr_eq(&sidebar, &same_sidebar));
        assert!(Arc::ptr_eq(&sidebar_missing, &missing_cached(&ctx, &same_sidebar, "jei_pinned_missing")));
        let same_editor = goal_plan(&ctx, &inv, false, 1, goal.item, 64, &goal.choices);
        assert!(Arc::ptr_eq(&editor, &same_editor));
        assert!(Arc::ptr_eq(&editor_missing, &missing(&ctx, &same_editor)));
    }
}
