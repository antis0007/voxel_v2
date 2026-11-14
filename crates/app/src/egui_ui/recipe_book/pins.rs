//! Bounded, session-local crafting goals. Planning never spends or moves stock.
use super::*;

const STATE: &str = "jei_pinned_targets";
const LIMIT: usize = 8;

#[derive(Clone, Debug, PartialEq)]
struct Target {
    item: Item,
    amount: u32,
    choices: Choices,
}

#[derive(Clone, Default)]
struct Pins {
    targets: Vec<Target>,
    active: Option<Item>,
}

fn state(ctx: &egui::Context) -> Pins {
    ctx.data(|d| d.get_temp::<Pins>(egui::Id::new(STATE))).unwrap_or_default()
}

fn save(ctx: &egui::Context, pins: Pins) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(STATE), pins));
}

pub(super) fn contains(ctx: &egui::Context, item: Item) -> bool {
    state(ctx).targets.iter().any(|p| p.item == item)
}

fn put(ctx: &egui::Context, target: Target) -> bool {
    let mut pins = state(ctx);
    if let Some(existing) = pins.targets.iter_mut().find(|p| p.item == target.item) {
        *existing = target.clone();
    } else if pins.targets.len() < LIMIT {
        pins.targets.push(target.clone());
    } else {
        return false;
    }
    pins.active = Some(target.item);
    save(ctx, pins);
    true
}

fn remove(ctx: &egui::Context, item: Item) {
    let mut pins = state(ctx);
    pins.targets.retain(|p| p.item != item);
    if pins.active == Some(item) {
        pins.active = pins.targets.last().map(|p| p.item);
    }
    save(ctx, pins);
}

pub(super) fn toggle(ctx: &egui::Context, item: Item) {
    if contains(ctx, item) {
        remove(ctx, item);
    } else {
        let _ = put(ctx, Target { item, amount: 1, choices: Choices::new() });
    }
}

pub(super) fn button(ui: &mut egui::Ui, item: Item, amount: u32, choices: &Choices) {
    let target = Target { item, amount, choices: choices.clone() };
    let pins = state(ui.ctx());
    let existing = pins.targets.iter().find(|p| p.item == item);
    let same = existing == Some(&target);
    let enabled = existing.is_some() || pins.targets.len() < LIMIT;
    let label = if same { "Unpin" } else if existing.is_some() { "Update Pin" } else { "Pin" };
    if ui.add_enabled(enabled, egui::Button::new(label))
        .on_hover_text(format!("Session Target: {amount}x · Carried Stock Counts"))
        .on_disabled_hover_text("Pinned Targets Full: 8/8").clicked()
    {
        if same { remove(ui.ctx(), item); } else { let _ = put(ui.ctx(), target); }
    }
}

type PlanKey = (u64, u32, bool, i32, Target);

pub(super) fn held_target(survival: &SurvivalInventory, item: Item) -> f32 {
    let equipped = survival.equipment.iter().chain(std::iter::once(&survival.offhand))
        .flatten().filter(|s| s.item == item).fold(0u32, |n, s| n.saturating_add(s.count));
    stock_of(survival, item) + equipped as f32
}

fn plan(ctx: &egui::Context, survival: &SurvivalInventory, bench: bool, size: i32, target: &Target) -> Arc<Plan> {
    plan_cached(ctx, survival, bench, size, target, "jei_pinned_plan")
}

fn plan_cached(ctx: &egui::Context, survival: &SurvivalInventory, bench: bool, size: i32, target: &Target, cache: &str) -> Arc<Plan> {
    let id = egui::Id::new(cache);
    let held = held_target(survival, target.item);
    let key = (stock_key(survival), held.to_bits(), bench, size, target.clone());
    if let Some((previous, plan)) = ctx.data(|d| d.get_temp::<(PlanKey, Arc<Plan>)>(id))
        && previous == key
    {
        return plan;
    }
    let mut stock = index::known_items().iter().map(|&item| (item, stock_of(survival, item)))
        .filter(|(_, n)| *n > 0.0).collect::<HashMap<_, _>>();
    // Always include the exact target, even authored equipment absent from the catalog.
    // Equipped/offhand stock completes this goal, but is never offered as other inputs.
    stock.insert(target.item, held);
    // Unlike the Make tree, a goal includes finished items already carried.
    if bench && target.item != Station::Bench.icon() { stock.insert(Station::Bench.icon(), 1.0); }
    let plan = Arc::new(index::plan_with_facilities(target.item, target.amount as f32,
        &mut stock, &target.choices, size));
    ctx.data_mut(|d| d.insert_temp(id, (key, plan.clone())));
    plan
}

pub(super) fn goal_plan(ctx: &egui::Context, survival: &SurvivalInventory, bench: bool,
    size: i32, item: Item, amount: u32, choices: &Choices) -> Arc<Plan>
{
    plan_cached(ctx, survival, bench, size, &Target { item, amount, choices: choices.clone() }, "jei_pinned_edit_plan")
}

pub(super) fn missing(ctx: &egui::Context, plan: &Arc<Plan>) -> Arc<Vec<(Item, f32)>> {
    missing_cached(ctx, plan, "jei_pinned_edit_missing")
}

fn missing_cached(ctx: &egui::Context, plan: &Arc<Plan>, cache: &str) -> Arc<Vec<(Item, f32)>> {
    let id = egui::Id::new(cache);
    if let Some((previous, missing)) = ctx.data(|d| d.get_temp::<(Arc<Plan>, Arc<Vec<(Item, f32)>>)>(id))
        && Arc::ptr_eq(&previous, plan)
    {
        return missing;
    }
    let missing = Arc::new(plan.missing());
    ctx.data_mut(|d| d.insert_temp(id, (plan.clone(), missing.clone())));
    missing
}

fn open(ctx: &egui::Context, target: &Target) {
    let mut pins = state(ctx);
    pins.active = Some(target.item);
    save(ctx, pins);
    request_page(ctx, history::Page {
        item: target.item, tab: 2, goal: true,
        amount: target.amount, choices: target.choices.clone(),
    });
}

pub(super) fn draw(ui: &mut egui::Ui, survival: &SurvivalInventory, bench: bool, size: i32, nav: &mut Option<Nav>) {
    let pins = state(ui.ctx());
    if pins.targets.is_empty() { return; }
    ui.horizontal(|ui| {
        ui.label(RichText::new("Pinned").size(13.0).color(AMBER))
            .on_hover_text("Session Targets · [A] Pin/Unpin");
        if let Some(active) = pins.active
            && ui.small_button("x").on_hover_text("Unpin Active Target").clicked()
        {
            remove(ui.ctx(), active);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 2.0);
        for target in &pins.targets {
            let mut target_nav = None;
            let response = item_cell(ui, target.item, Some(target.amount as f32), 28.0,
                (pins.active == Some(target.item)).then_some(AMBER), &mut target_nav, survival);
            #[cfg(test)]
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("pin_rect", target.item)), response.rect));
            if response.clicked() { open(ui.ctx(), target); }
            else if target_nav.is_some() { *nav = target_nav; }
        }
    });
    let Some(target) = pins.targets.iter().find(|p| Some(p.item) == pins.active) else { return; };
    let plan = plan(ui.ctx(), survival, bench, size, target);
    let complete = plan.from_stock + 1e-4 >= plan.need;
    ui.add(egui::Label::new(RichText::new(format!("{} · {}/{}", target.item.name(),
        amount_text(held_target(survival, target.item)), target.amount)).size(13.0)
        .color(if complete { JADE } else { PAPER })).truncate());
    if complete {
        ui.label(RichText::new("Complete").size(13.0).color(JADE));
    } else {
        let missing = missing_cached(ui.ctx(), &plan, "jei_pinned_missing");
        if missing.is_empty() {
            ui.label(RichText::new("Ingredients Ready").size(13.0).color(JADE));
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Missing").size(13.0).color(MUTED));
                for &(item, count) in missing.iter().take(7) {
                    item_cell(ui, item, Some(count), 25.0, Some(DANGER), nav, survival);
                }
                if missing.len() > 7 {
                    ui.label(format!("+{}", missing.len() - 7)).on_hover_ui(|ui| {
                        for &(item, count) in missing.iter().skip(7) {
                            ui.label(format!("{} x{}", item.name(), amount_text(count)));
                        }
                    });
                }
            });
        }
    }
    ui.separator();
}

#[cfg(test)]
mod tests;
