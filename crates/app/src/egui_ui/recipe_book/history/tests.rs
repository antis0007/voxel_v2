use super::*;
use crate::world::{BlockId, inventory::ItemStack};

fn page(item: Item, tab: u8, amount: u32) -> Page {
    Page { item, tab, amount, goal: false, choices: Choices::new() }
}
fn torch() -> Item { Item::Block(BlockId::Torch) }
fn stone() -> Item { Item::Block(BlockId::Stone) }

#[test]
fn recipe_history_restores_goal_amount_and_real_alternate_recipe() {
    let ctx = egui::Context::default();
    let item = index::known_items().iter().copied()
        .find(|&item| index::producing_at(item, 1).len() > 1).expect("catalog alternate recipe");
    let mut goal = page(item, 2, 64);
    goal.goal = true;
    goal.choices.insert(item, (index::default_choice(item) + 1) % index::producing_at(item, 1).len());
    goal.store(&ctx);
    let mut stock = HashMap::new();
    let before = index::plan_with_facilities(item, 64.0, &mut stock, &goal.choices, 1);
    request(&ctx, stone(), true);
    // Queuing a destination must not mutate the page that history will capture.
    assert_eq!(Page::current(&ctx), Some(goal.clone()));
    assert!(take_request(&ctx));
    let destination = Page::current(&ctx).unwrap();
    assert_eq!((destination.item, destination.tab, destination.goal), (stone(), 1, false));
    assert!(go(&ctx, Direction::Back));
    let restored = Page::current(&ctx).unwrap();
    assert_eq!(restored, goal);
    let after = index::plan_with_facilities(item, restored.amount as f32,
        &mut HashMap::new(), &restored.choices, 1);
    assert_eq!(before.missing(), after.missing());
    assert!(go(&ctx, Direction::Forward));
    assert_eq!(Page::current(&ctx), Some(destination));
}

#[test]
fn recipe_history_tabs_and_pin_requests_are_reversible() {
    let ctx = egui::Context::default();
    let sources = page(torch(), 0, 1);
    visit(&ctx, sources.clone());
    let uses = page(torch(), 1, 1);
    visit(&ctx, uses.clone());
    assert!(go(&ctx, Direction::Back));
    assert_eq!(Page::current(&ctx), Some(sources));
    assert!(go(&ctx, Direction::Forward));
    let mut goal = page(torch(), 2, 16); goal.goal = true;
    request_page(&ctx, goal.clone());
    assert!(take_request(&ctx));
    assert!(go(&ctx, Direction::Back));
    assert_eq!(Page::current(&ctx), Some(uses));
    assert!(go(&ctx, Direction::Forward));
    assert_eq!(Page::current(&ctx), Some(goal));
}

#[test]
fn recipe_history_is_bounded_and_new_branch_discards_forward() {
    let ctx = egui::Context::default();
    for amount in 1..=80 { visit(&ctx, page(torch(), 2, amount)); }
    assert_eq!(state(&ctx).back.len(), LIMIT);
    for _ in 0..LIMIT { assert!(go(&ctx, Direction::Back)); }
    assert!(!go(&ctx, Direction::Back));
    assert_eq!(state(&ctx).forward.len(), LIMIT);
    for _ in 0..LIMIT { assert!(go(&ctx, Direction::Forward)); }
    assert_eq!(Page::current(&ctx).unwrap().amount, 80);
    assert!(!go(&ctx, Direction::Forward));
    assert!(go(&ctx, Direction::Back));
    // Reopening the identical page is a no-op, not a new branch.
    visit(&ctx, Page::current(&ctx).unwrap());
    assert_eq!(state(&ctx).forward.len(), 1);
    visit(&ctx, page(stone(), 1, 4));
    assert!(state(&ctx).forward.is_empty());
    assert!(!go(&ctx, Direction::Forward));
}

#[test]
fn recipe_history_latest_request_wins_and_close_keeps_position() {
    let ctx = egui::Context::default();
    let original = page(torch(), 2, 16); original.store(&ctx);
    request(&ctx, stone(), false);
    request(&ctx, stone(), true);
    assert!(take_request(&ctx));
    assert!(!take_request(&ctx));
    assert_eq!(state(&ctx).back, vec![original.clone()]);
    assert_eq!(Page::current(&ctx).unwrap().tab, 1);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(OPEN), false));
    assert!(go(&ctx, Direction::Back));
    assert_eq!(Page::current(&ctx), Some(original));
}

#[test]
fn recipe_history_is_private_and_never_restores_inventory_snapshots() {
    let (a, b) = (egui::Context::default(), egui::Context::default());
    let mut goal = page(torch(), 2, 16); goal.goal = true;
    visit(&a, goal.clone()); visit(&b, page(stone(), 1, 1));
    request(&a, stone(), false); assert!(take_request(&a));
    let mut inv = SurvivalInventory::empty();
    inv.pack[0] = Some(ItemStack::new(torch(), 8));
    let before = inv.clone();
    assert!(go(&a, Direction::Back));
    let restored = Page::current(&a).unwrap();
    let plan = pins::goal_plan(&a, &inv, false, 1, restored.item, restored.amount, &restored.choices);
    assert_eq!(plan.from_stock, 8.0);
    assert_eq!(inv, before);
    assert_eq!(Page::current(&b), Some(page(stone(), 1, 1)));
    assert!(state(&b).back.is_empty());
}

struct Harness {
    ctx: egui::Context,
    inv: SurvivalInventory,
    time: f64,
    focus: bool,
    focus_button: bool,
    text: String,
}
impl Harness {
    fn new() -> Self { Self { ctx: egui::Context::default(), inv: SurvivalInventory::empty(),
        time: 0.0, focus: false, focus_button: false, text: String::new() } }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.02;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO, vec2(900.0, 900.0))), time: Some(self.time), events, ..Default::default() };
        let mut output = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                if self.focus_button { ui.button("Focus").request_focus(); }
                if self.focus { ui.text_edit_singleline(&mut self.text).request_focus(); }
                assert!(!draw_mode(ui, &mut self.inv, false, false, 1));
            });
        });
        output.textures_delta.clear();
    }
    fn click(&mut self, label: &str) {
        let rect = self.ctx.data(|d| d.get_temp::<egui::Rect>(
            egui::Id::new(("recipe_history_button", label)))).unwrap();
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerMoved(rect.center()), egui::Event::PointerButton {
                pos: rect.center(), button: egui::PointerButton::Primary, pressed,
                modifiers: egui::Modifiers::NONE }]);
        }
        self.frame(vec![]);
    }
    fn release(&mut self, key: egui::Key) {
        self.frame(vec![egui::Event::Key { key, physical_key: None, pressed: false,
            repeat: false, modifiers: egui::Modifiers::NONE }]);
    }
    fn key(&mut self, key: egui::Key, repeat: bool) {
        self.frame(vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat,
            modifiers: egui::Modifiers { alt: true, ..egui::Modifiers::NONE } }]);
    }
}

#[test]
fn recipe_history_pointer_controls_restore_the_drawn_page_without_crafting() {
    let mut h = Harness::new();
    let mut goal = page(torch(), 2, 16); goal.goal = true;
    visit(&h.ctx, goal.clone());
    request(&h.ctx, stone(), true); assert!(take_request(&h.ctx));
    let uses = Page::current(&h.ctx).unwrap();
    let before = h.inv.clone();
    h.frame(vec![]); h.frame(vec![]);
    h.click("◀");
    assert_eq!(Page::current(&h.ctx), Some(goal));
    h.click("▶");
    assert_eq!(Page::current(&h.ctx), Some(uses));
    assert_eq!(h.inv, before);
}

#[test]
fn recipe_history_shortcuts_ignore_repeat_and_text_focus() {
    let mut h = Harness::new();
    let a = page(torch(), 0, 1); let b = page(stone(), 1, 1); let c = page(torch(), 2, 4);
    visit(&h.ctx, a.clone()); visit(&h.ctx, b.clone()); visit(&h.ctx, c);
    h.frame(vec![]); h.frame(vec![]);
    h.key(egui::Key::ArrowLeft, false);
    assert_eq!(Page::current(&h.ctx), Some(b.clone()));
    // Egui derives repeats from held keys, not the input event's repeat flag.
    h.key(egui::Key::ArrowLeft, true);
    assert_eq!(Page::current(&h.ctx), Some(b.clone()));
    h.release(egui::Key::ArrowLeft);
    h.key(egui::Key::ArrowLeft, false);
    assert_eq!(Page::current(&h.ctx), Some(a.clone()));
    h.release(egui::Key::ArrowLeft);
    h.key(egui::Key::ArrowRight, false);
    assert_eq!(Page::current(&h.ctx), Some(b));
    h.release(egui::Key::ArrowRight);
    h.focus_button = true; h.frame(vec![]);
    assert!(h.ctx.egui_wants_keyboard_input());
    assert!(!h.ctx.text_edit_focused());
    h.key(egui::Key::ArrowLeft, false);
    assert_eq!(Page::current(&h.ctx), Some(a.clone()));
    h.release(egui::Key::ArrowLeft);
    h.focus_button = false; h.focus = true; h.frame(vec![]); h.frame(vec![]);
    assert!(h.ctx.text_edit_focused());
    h.key(egui::Key::ArrowRight, false);
    assert_eq!(Page::current(&h.ctx), Some(a));
}

#[test]
fn recipe_history_internal_navigation_keeps_floating_window_visibility() {
    let ctx = egui::Context::default();
    for open in [false, true] {
        let original = page(torch(), 2, 16); original.store(&ctx);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(OPEN), open));
        browse(&ctx, stone(), 1);
        assert_eq!(ctx.data(|d| d.get_temp::<bool>(egui::Id::new(OPEN))), Some(open));
        assert!(go(&ctx, Direction::Back));
        assert_eq!(Page::current(&ctx), Some(original));
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(OPEN), false));
    request(&ctx, stone(), false); assert!(take_request(&ctx));
    assert_eq!(ctx.data(|d| d.get_temp::<bool>(egui::Id::new(OPEN))), Some(true));
}
