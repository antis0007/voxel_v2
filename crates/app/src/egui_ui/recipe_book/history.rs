//! Session-local recipe navigation; view snapshots never contain inventory.
use super::*;

const STATE: &str = "jei_navigation";
const LIMIT: usize = 24;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Page {
    pub item: Item,
    pub tab: u8,
    pub goal: bool,
    pub amount: u32,
    pub choices: Choices,
}

impl Page {
    pub(super) fn current(ctx: &egui::Context) -> Option<Self> {
        ctx.data(|d| Some(Self {
            item: d.get_temp::<Option<Item>>(egui::Id::new(SELECTED)).flatten()?,
            tab: d.get_temp::<u8>(egui::Id::new(TAB)).unwrap_or(0),
            goal: d.get_temp::<bool>(egui::Id::new(GOAL_VIEW)).unwrap_or(false),
            amount: d.get_temp::<u32>(egui::Id::new(AMOUNT)).unwrap_or(1),
            choices: d.get_temp::<Choices>(egui::Id::new(CHOICES)).unwrap_or_default(),
        }))
    }

    pub(super) fn browse(ctx: &egui::Context, item: Item, tab: u8) -> Self {
        let previous = Self::current(ctx);
        Self { item, tab, goal: false, amount: previous.as_ref().map_or(1, |p| p.amount),
            choices: previous.map_or_else(Choices::new, |p| p.choices) }
    }

    pub(super) fn store(&self, ctx: &egui::Context) {
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new(SELECTED), Some(self.item));
            d.insert_temp(egui::Id::new(TAB), self.tab);
            d.insert_temp(egui::Id::new(GOAL_VIEW), self.goal);
            d.insert_temp(egui::Id::new(AMOUNT), self.amount);
            d.insert_temp(egui::Id::new(CHOICES), self.choices.clone());
        });
    }
}

#[derive(Clone, Default)]
struct History {
    back: Vec<Page>,
    forward: Vec<Page>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Direction { Back, Forward }

fn state(ctx: &egui::Context) -> History {
    ctx.data(|d| d.get_temp(egui::Id::new(STATE))).unwrap_or_default()
}

fn push(stack: &mut Vec<Page>, page: Page) {
    if stack.len() == LIMIT { stack.remove(0); }
    stack.push(page);
}

/// Navigation inside an already-open view must not open another window.
pub(super) fn browse(ctx: &egui::Context, item: Item, tab: u8) {
    visit(ctx, Page::browse(ctx, item, tab));
}

pub(super) fn visit(ctx: &egui::Context, next: Page) {
    let previous = Page::current(ctx);
    if previous.as_ref() == Some(&next) { return; }
    let mut history = state(ctx);
    if let Some(previous) = previous { push(&mut history.back, previous); }
    history.forward.clear();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(STATE), history));
    next.store(ctx);
}

pub(super) fn go(ctx: &egui::Context, direction: Direction) -> bool {
    let mut history = state(ctx);
    let destination = match direction {
        Direction::Back => history.back.pop(),
        Direction::Forward => history.forward.pop(),
    };
    let Some(destination) = destination else { return false; };
    if let Some(current) = Page::current(ctx) {
        match direction {
            Direction::Back => push(&mut history.forward, current),
            Direction::Forward => push(&mut history.back, current),
        }
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(STATE), history));
    destination.store(ctx);
    true
}

fn shortcut(ctx: &egui::Context) -> Option<Direction> {
    if ctx.text_edit_focused() { return None; }
    ctx.input(|i| i.events.iter().find_map(|event| match event {
        egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. }
            if modifiers.alt && !modifiers.ctrl && !modifiers.command && !modifiers.shift => match key {
                egui::Key::ArrowLeft => Some(Direction::Back),
                egui::Key::ArrowRight => Some(Direction::Forward),
                _ => None,
            },
        _ => None,
    }))
}

pub(super) fn toolbar(ui: &mut egui::Ui) -> Option<Direction> {
    let (back, forward) = ui.ctx().data_mut(|d| {
        let history = d.get_temp_mut_or_default::<History>(egui::Id::new(STATE));
        (!history.back.is_empty(), !history.forward.is_empty())
    });
    let mut direction = shortcut(ui.ctx());
    for (way, available, label, hint) in [
        (Direction::Back, back, "◀", "Back · [Alt]+[Left]"),
        (Direction::Forward, forward, "▶", "Forward · [Alt]+[Right]"),
    ] {
        let response = ui.add_enabled(available, egui::Button::new(label).small()).on_hover_text(hint);
        #[cfg(test)]
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("recipe_history_button", label)), response.rect));
        if response.clicked() { direction = Some(way); }
    }
    direction
}

#[cfg(test)]
mod tests;