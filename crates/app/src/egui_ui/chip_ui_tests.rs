//! The designer driven by synthesized input on a headless egui context:
//! the same widgets, hit tests and key handling the window uses.
use super::super::UiAction;
use super::{Editor, Tool, id, open, window};
use crate::world::factory::View;
use crate::world::factory::chips::{Action, registry};
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};
use engine_chip::compile::compile;
use engine_chip::library::Library;
use engine_chip::presets::sentinel;
use std::sync::Arc;

struct Harness {
    ctx: egui::Context,
    view: View,
    pointer: Pos2,
    mods: Modifiers,
}

impl Harness {
    fn new() -> Self {
        let mut lib = Library::default();
        lib.add_project("Sentinel", sentinel(registry()));
        let view = View { chips: Some(Arc::new(lib)), ..View::default() };
        let ctx = egui::Context::default();
        open(&ctx);
        let mut h = Self { ctx, view, pointer: pos2(10.0, 10.0), mods: Modifiers::NONE };
        h.frame(Vec::new());
        h.frame(Vec::new());
        h
    }
    fn frame(&mut self, mut events: Vec<Event>) -> UiAction {
        let mut action = UiAction::None;
        events.insert(0, Event::ModifiersChanged(self.mods));
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
            events,
            ..RawInput::default()
        };
        self.ctx
            .run_ui(input, |ui| window(ui.ctx(), &self.view, &mut action))
            .textures_delta
            .clear();
        // Apply the drafts the world would store, so generations stay in step.
        if let UiAction::Chip(Action::SaveDraft { project, design }) = &action
            && let Some(lib) = self.view.chips.as_mut()
        {
            Arc::make_mut(lib).edit(*project, |d| *d = design.clone());
        }
        action
    }
    fn editor(&self) -> Editor {
        self.ctx.data_mut(|d| d.get_temp::<Editor>(id())).unwrap()
    }
    fn set(&self, f: impl FnOnce(&mut Editor)) {
        self.ctx.data_mut(|d| f(d.get_temp_mut_or_default::<Editor>(id())));
    }
    fn site(&self, x: u8, y: u8) -> Pos2 {
        let (o, cell) = self.editor().canvas.unwrap();
        o + vec2((f32::from(x) + 0.5) * cell, (f32::from(y) + 0.5) * cell)
    }
    fn click(&mut self, at: Pos2, button: PointerButton) -> Vec<UiAction> {
        self.pointer = at;
        let press = |pressed| Event::PointerButton { pos: at, button, pressed, modifiers: Modifiers::NONE };
        vec![
            self.frame(vec![Event::PointerMoved(at)]),
            self.frame(vec![press(true)]),
            self.frame(vec![press(false)]),
            self.frame(Vec::new()),
        ]
    }
    fn key(&mut self, key: Key, modifiers: Modifiers) {
        let at = self.pointer;
        let ev = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        self.mods = modifiers;
        self.frame(vec![Event::PointerMoved(at), ev(true)]);
        self.frame(vec![ev(false)]);
        self.mods = Modifiers::NONE;
    }
}

#[test]
fn designer_places_undoes_detaches_and_zooms_from_input() {
    let mut h = Harness::new();
    let ed = h.editor();
    assert_eq!(ed.project, Some(1), "the library's project opens");
    let parts = ed.design.parts.len();
    assert!(ed.canvas.is_some());

    // Place a watchdog on a free site with the mouse.
    h.set(|e| e.tool = Tool::Place("watchdog".into()));
    let at = h.site(9, 7);
    let actions = h.click(at, PointerButton::Primary);
    let ed = h.editor();
    assert_eq!(ed.design.parts.len(), parts + 1, "click placed a module");
    assert!(actions.iter().any(|a| matches!(a, UiAction::Chip(Action::SaveDraft { .. }))), "the draft is synced");

    // [Ctrl+Z] with the pointer over the designer undoes it.
    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(h.editor().design.parts.len(), parts);

    // [RMB] on the ROM's supply port with Connect detaches it.
    h.set(|e| e.tool = Tool::Wire);
    let rom = h.editor().design.parts.iter().find(|p| p.module == "rom256").unwrap().clone();
    let at = h.site(rom.x, rom.y);
    h.click(at, PointerButton::Secondary);
    let ed = h.editor();
    let c = compile(&ed.design, registry());
    assert!(c.errors().any(|e| e.message.starts_with("Unpowered: Mask ROM")), "{:?}", c.diagnostics);

    // [Ctrl]+wheel over the canvas zooms.
    let before = ed.zoom;
    let at = h.site(5, 5);
    h.mods = Modifiers::COMMAND;
    h.frame(vec![
        Event::PointerMoved(at),
        Event::MouseWheel { phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown, unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, 40.0), modifiers: Modifiers::COMMAND },
    ]);
    assert!(h.editor().zoom > before, "zoom {} -> {}", before, h.editor().zoom);
}

#[test]
fn designer_releases_only_the_draft_it_shows() {
    let mut h = Harness::new();
    h.set(|e| e.tool = Tool::Place("watchdog".into()));
    let at = h.site(9, 7);
    h.click(at, PointerButton::Primary);
    let ed = h.editor();
    let lib = h.view.chips.as_ref().unwrap();
    assert_eq!(lib.project(1).unwrap().draft, ed.design, "draft synced through SaveDraft");
    assert_eq!(ed.generation, lib.project(1).unwrap().generation, "editor adopted the library generation");
    assert!(ed.pending.is_none());
}
