use super::*;
use crate::world::{BlockId, inventory::ItemStack};

struct Harness {
    ctx: egui::Context,
    inv: SurvivalInventory,
    view: crate::world::adventure::View,
    action: UiAction,
    time: f64,
    text: String,
    focus_text: bool,
    bench: bool,
}
impl Harness {
    fn new(tab: usize) -> Self {
        let ctx = egui::Context::default();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(TAB_ID), tab));
        Self { ctx, inv: SurvivalInventory::empty(), view: Default::default(),
            action: UiAction::None, time: 0.0, text: String::new(), focus_text: false, bench: true }
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.action = UiAction::None;
        self.time += 0.02;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO, egui::vec2(1200.0, 1200.0))), time: Some(self.time), events, ..Default::default() };
        let mut output = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                if self.focus_text { ui.text_edit_singleline(&mut self.text).request_focus(); }
                page(ui, &self.view, &self.inv, self.bench, &mut self.action);
            });
        });
        output.textures_delta.clear();
    }
    fn rect(&self, item: Item) -> egui::Rect {
        self.ctx.data(|d| d.get_temp(egui::Id::new(("forge_recipe_link", item)))).unwrap()
    }
    fn key_event(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }
    }
    fn hover_key(&mut self, item: Item, key: egui::Key) {
        self.frame(vec![egui::Event::PointerMoved(self.rect(item).center()),
            Self::key_event(key, true, egui::Modifiers::NONE)]);
    }
    fn release(&mut self, key: egui::Key) {
        self.frame(vec![Self::key_event(key, false, egui::Modifiers::NONE)]);
    }
    fn request(&self, item: Item, uses: bool) {
        assert!(super::super::recipe_book::take_request(&self.ctx));
        assert_eq!(self.ctx.data(|d| d.get_temp::<Option<Item>>(egui::Id::new("jei_selected"))), Some(Some(item)));
        assert_eq!(self.ctx.data(|d| d.get_temp::<u8>(egui::Id::new("jei_tab"))), Some(u8::from(uses)));
        assert!(matches!(self.action, UiAction::None));
    }
}
fn material(id: &str) -> u8 { MATERIALS.iter().position(|m| m.id.0 == id).unwrap() as u8 }
fn part() -> Item { Item::Part { kind: 0, material: material("wood") } }

#[test]
fn forge_links_part_builder_sources_uses_and_cost_inspection_are_read_only() {
    let mut h = Harness::new(1);
    h.frame(vec![]); h.frame(vec![]);
    let before = h.inv.clone();
    h.hover_key(part(), egui::Key::R); h.request(part(), false);
    // A held key must not create another request after the first was consumed.
    h.hover_key(part(), egui::Key::R);
    assert!(!super::super::recipe_book::take_request(&h.ctx));
    h.release(egui::Key::R);
    h.hover_key(part(), egui::Key::U); h.request(part(), true);
    h.release(egui::Key::U);
    h.hover_key(Item::Block(BlockId::Log), egui::Key::R);
    h.request(Item::Block(BlockId::Log), false);
    assert_eq!(h.inv, before);
    assert_eq!(h.ctx.data(|d| d.get_temp::<usize>(egui::Id::new("tool_station_part"))), Some(0));
}

#[test]
fn forge_links_missing_assembly_slot_and_mold_have_exact_identities() {
    let mut h = Harness::new(0);
    let wood = material("wood");
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("tool_station_choices"), (0usize, [wood; 8], 0usize)));
    h.frame(vec![]); h.frame(vec![]);
    let before = h.inv.clone();
    let kind = kind_index(&PARTS.schemas[0].parts[0].kind) as u8;
    let item = Item::Part { kind, material: wood };
    h.hover_key(item, egui::Key::R); h.request(item, false);
    assert_eq!(h.ctx.data(|d| d.get_temp::<(usize,[u8;8],usize)>(egui::Id::new("tool_station_choices"))), Some((0,[wood;8],0)));
    h.release(egui::Key::R);
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new(TAB_ID), 1usize));
    h.frame(vec![]); h.frame(vec![]);
    h.hover_key(Item::Cast(0), egui::Key::U); h.request(Item::Cast(0), true);
    assert_eq!(h.inv, before);
}

#[test]
fn forge_links_ignore_text_entry_modifiers_and_unhovered_items() {
    let mut h = Harness::new(1);
    h.frame(vec![]); h.frame(vec![]);
    h.frame(vec![egui::Event::PointerMoved(egui::pos2(1195.0, 1195.0)),
        Harness::key_event(egui::Key::R, true, egui::Modifiers::NONE)]);
    assert!(!super::super::recipe_book::take_request(&h.ctx));
    h.release(egui::Key::R);
    let at = h.rect(part()).center();
    h.frame(vec![egui::Event::PointerMoved(at), Harness::key_event(egui::Key::R, true,
        egui::Modifiers { ctrl: true, command: true, ..egui::Modifiers::NONE })]);
    assert!(!super::super::recipe_book::take_request(&h.ctx));
    h.release(egui::Key::R);
    h.focus_text = true; h.frame(vec![]); h.frame(vec![]);
    assert!(h.ctx.text_edit_focused());
    h.hover_key(part(), egui::Key::R);
    assert!(!super::super::recipe_book::take_request(&h.ctx));
    assert!(matches!(h.action, UiAction::None));
}

fn service_harness() -> (Harness, Item, u8) {
    let mut h = Harness::new(2);
    let def = &PARTS.schemas[0];
    let wood = material("wood");
    let tool = ToolAssembly::new(&def.id, &vec![MATERIALS[wood as usize].id.clone(); def.parts.len()]).unwrap();
    h.view.tool_bank.push(tool);
    let owned = Item::Assembled { id: 0, schema: 0 };
    h.inv.hotbar[0] = Some(ItemStack::new(owned, 1));
    let kind = kind_index(&def.parts[0].kind) as u8;
    let replacement = material("stone");
    let item = Item::Part { kind, material: replacement };
    h.inv.pack[0] = Some(ItemStack::new(item, 1));
    h.frame(vec![]); h.frame(vec![]);
    (h, item, replacement)
}

#[test]
fn forge_links_service_inspection_never_replaces_parts_and_normal_click_still_does() {
    let (mut h, item, replacement) = service_harness();
    let before = h.inv.clone();
    h.hover_key(Item::Assembled { id: 0, schema: 0 }, egui::Key::U);
    h.request(Item::Assembled { id: 0, schema: 0 }, true);
    h.release(egui::Key::U);
    let at = h.rect(item).center();
    let pointer = |pressed| egui::Event::PointerButton { pos: at,
        button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    h.frame(vec![egui::Event::PointerMoved(at), pointer(true)]);
    h.frame(vec![egui::Event::PointerMoved(at), pointer(false),
        Harness::key_event(egui::Key::R, true, egui::Modifiers::NONE)]);
    h.request(item, false);
    assert_eq!(h.inv, before);
    h.release(egui::Key::R);
    h.frame(vec![egui::Event::PointerMoved(at), pointer(true)]);
    h.frame(vec![egui::Event::PointerMoved(at), pointer(false)]);
    assert!(matches!(h.action, UiAction::Factory(crate::world::factory::Action::ToolWorkshop(
        Action::Replace(0, 0, m))) if m == replacement));
    // The UI only emits the existing authoritative command; it never spends stock.
    assert_eq!(h.inv, before);
}

#[test]
fn forge_links_personal_contexts_never_share_requests_or_invent_owned_tools() {
    let mut a = Harness::new(1); let mut b = Harness::new(1);
    a.frame(vec![]); a.frame(vec![]); b.frame(vec![]); b.frame(vec![]);
    a.hover_key(part(), egui::Key::R);
    assert!(!super::super::recipe_book::take_request(&b.ctx));
    a.request(part(), false);
    let mut requested = true;
    let mut output = b.ctx.run_ui(egui::RawInput::default(), |root| {
        egui::CentralPanel::default().show(root, |ui| {
            let response = ui.button("Preview");
            requested = recipe_links::owned(ui, &response, &b.inv, 3000);
        });
    });
    output.textures_delta.clear();
    assert!(!requested);
    assert!(!super::super::recipe_book::take_request(&b.ctx));
}
#[test]
fn forge_links_mold_buttons_inspect_without_crafting_even_away_from_workbench() {
    let mut h = Harness::new(1);
    h.frame(vec![]); h.frame(vec![]);
    let before = h.inv.clone();
    let item = Item::Cast(255);
    let at = h.rect(item).center();
    let pointer = |pressed| egui::Event::PointerButton { pos: at,
        button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    h.frame(vec![egui::Event::PointerMoved(at), pointer(true)]);
    h.frame(vec![egui::Event::PointerMoved(at), pointer(false),
        Harness::key_event(egui::Key::R, true, egui::Modifiers::NONE)]);
    h.request(item, false);
    h.release(egui::Key::R);
    h.frame(vec![egui::Event::PointerMoved(at), pointer(true)]);
    h.frame(vec![egui::Event::PointerMoved(at), pointer(false)]);
    assert!(matches!(h.action, UiAction::Factory(crate::world::factory::Action::ToolWorkshop(Action::Mold(255)))));
    h.bench = false; h.frame(vec![]); h.frame(vec![]);
    h.hover_key(item, egui::Key::R); h.request(item, false);
    h.release(egui::Key::R);
    h.hover_key(Item::Cast(0), egui::Key::U); h.request(Item::Cast(0), true);
    assert_eq!(h.inv, before);
}
