use super::*;
use crate::world::{BlockId, inventory::{ItemStack, SlotAddr}};

fn fixture() -> SurvivalInventory {
    let mut inv = SurvivalInventory::empty();
    inv.pack[0] = Some(ItemStack::new(Item::Block(BlockId::Stone), 7));
    inv.pack[3] = Some(ItemStack::new(Item::Block(BlockId::Planks), 12));
    inv.hotbar[2] = Some(ItemStack::new(Item::Block(BlockId::Stone), 64));
    let mut chest = [None; crate::world::containers::SLOTS];
    chest[4] = Some(ItemStack::new(Item::Block(BlockId::Stone), 3));
    chest[8] = Some(ItemStack::new(Item::Block(BlockId::Planks), 5));
    inv.container = Some((3, chest));
    inv.craft_grid[0] = Some(ItemStack::new(Item::Block(BlockId::Stone), 2));
    inv.cursor = Some(ItemStack::new(Item::Block(BlockId::Stone), 4));
    inv.toggle_protected(SlotAddr::Pack(0));
    inv
}

#[test]
fn locate_counts_visible_storage_without_mutating_inventory() {
    let inv = fixture();
    let before = inv.clone();
    let mut search = Search::default();
    search.update("STONE", true);
    assert_eq!(search.counts(&inv), (3, 5));
    assert_eq!(inv, before);
    let mut closed = inv;
    closed.container = None;
    assert_eq!(search.counts(&closed), (2, 3));
    closed.pack[0] = None;
    assert_eq!(search.counts(&closed), (1, 2));
}

#[test]
fn locate_reuses_catalog_query_and_invalidates_old_matches() {
    let mut search = Search::default();
    let stone = Item::Block(BlockId::Stone);
    for query in ["stone", "planks | stone", "stone -planks", "@base stone", "#building stone", "zzqx"] {
        search.update(query, true);
        assert_eq!(search.matches(stone), Some(technology::matches(stone, query)));
        assert_eq!(search.matches(stone), Some(technology::matches(stone, query)));
    }
    search.update("stone", true);
    assert_eq!(search.matches(stone), Some(true));
    search.update("planks", true);
    assert_eq!(search.matches(stone), Some(false));
    search.update("planks", false);
    assert_eq!(search.matches(stone), None);
    search.update("  ", true);
    assert!(!search.enabled);
    assert_eq!(search.matches(stone), None);
}

struct Harness {
    ctx: egui::Context,
    query: String,
    time: f64,
    rects: [Rect; 3],
    inv: SurvivalInventory,
}
impl Harness {
    fn new() -> Self {
        Self { ctx: egui::Context::default(), query: "stone".into(), time: 0.0,
            rects: [Rect::NOTHING; 3], inv: fixture() }
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.05;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(640.0, 480.0))),
            time: Some(self.time), events, focused: true, ..Default::default() };
        let mut output = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                self.rects = row(ui, &mut self.query, 364.0).map(|r| r.rect);
                summary(ui, &self.inv);
                let rect = Rect::from_min_size(egui::pos2(20.0, 160.0), vec2(54.0, 54.0));
                super::super::draw_survival_slot(ui, &mut self.inv, SlotAddr::Pack(0), rect, 3.0, false,
                    &mut None, &mut None, &mut Vec::new(), &mut None);
            });
        });
        output.textures_delta.clear();
    }
    fn click(&mut self, index: usize) {
        let pos = self.rects[index].center();
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]);
        }
    }
}

#[test]
fn locate_pointer_toggle_clear_and_empty_query_restore_default() {
    let mut h = Harness::new();
    h.frame(vec![]); h.frame(vec![]);
    let before = h.inv.clone();
    assert!(!with_search(&h.ctx, |s| s.enabled));
    h.click(2);
    assert!(with_search(&h.ctx, |s| s.enabled));
    assert_eq!(with_search(&h.ctx, |s| s.counts(&h.inv)), (3, 5));
    h.click(2);
    assert!(!with_search(&h.ctx, |s| s.enabled));
    h.click(2);
    assert!(with_search(&h.ctx, |s| s.enabled));
    h.click(1);
    assert_eq!(h.query, "");
    assert!(!with_search(&h.ctx, |s| s.enabled));
    h.click(2);
    assert!(!with_search(&h.ctx, |s| s.enabled));
    assert_eq!(h.inv, before);
}

#[test]
fn locate_typing_over_a_slot_does_not_toggle_its_lock() {
    let mut h = Harness::new();
    h.frame(vec![]); h.frame(vec![]);
    h.click(0);
    assert!(h.ctx.egui_wants_keyboard_input());
    let before = h.inv.clone();
    h.frame(vec![egui::Event::PointerMoved(egui::pos2(40.0, 180.0)),
        egui::Event::Text("l".into()),
        egui::Event::Key { key: egui::Key::L, physical_key: None, pressed: true, repeat: false,
            modifiers: egui::Modifiers::NONE }]);
    assert!(h.query.contains('l'));
    assert_eq!(h.inv, before);
}

#[test]
fn locate_slot_lock_shortcut_remains_available_without_editor_focus() {
    let mut h = Harness::new();
    h.frame(vec![]); h.frame(vec![]);
    assert!(!h.ctx.egui_wants_keyboard_input());
    let mut expected = h.inv.clone();
    expected.toggle_protected(SlotAddr::Pack(0));
    h.frame(vec![egui::Event::PointerMoved(egui::pos2(40.0, 180.0)),
        egui::Event::Key { key: egui::Key::L, physical_key: None, pressed: true, repeat: false,
            modifiers: egui::Modifiers::NONE }]);
    assert_eq!(h.inv, expected);
}
