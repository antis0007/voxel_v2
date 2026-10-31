//! JEI-style recipe browser: every item on the left; recipes, uses and a
//! crafting-tree calculator on the right. Hover any item in the inventory and
//! press **R** for its recipes or **U** for its uses.
//!
//! Data comes only from `world::recipe_index`; this module draws and keeps a
//! little view state in egui's temp storage.
use super::inventory::draw_item_icon;
use super::presentation::{self, AMBER, DANGER, EDGE, JADE, MANA, MUTED, PAPER};
use crate::world::factory::power;
use crate::world::inventory::{Item, SurvivalInventory, all_recipes};
use crate::world::recipe_index::{self as index, Choices, Entry, Plan, Station};
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, vec2};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const REQUEST: &str = "jei_request";
const SELECTED: &str = "jei_selected";
const TAB: &str = "jei_tab";
const QUERY: &str = "jei_query";
const AMOUNT: &str = "jei_amount";
const CHOICES: &str = "jei_choices";
const HISTORY: &str = "jei_history";

/// Ask the browser to open on `item`: its uses when `uses`, else its recipes.
pub(super) fn request(ctx: &egui::Context, item: Item, uses: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(REQUEST), Some((item, uses))));
}

/// True once when a request is waiting; the inventory switches to this page.
pub(super) fn take_request(ctx: &egui::Context) -> bool {
    let pending = ctx
        .data_mut(|d| d.get_temp::<Option<(Item, bool)>>(egui::Id::new(REQUEST)))
        .flatten();
    if let Some((item, uses)) = pending {
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new(REQUEST), None::<(Item, bool)>);
            d.insert_temp(egui::Id::new(SELECTED), Some(item));
            d.insert_temp(egui::Id::new(TAB), u8::from(uses));
        });
        return true;
    }
    false
}

/// Hash of every stack `count_item` and `stock_of` read; changes with the stock.
fn stock_key(survival: &SurvivalInventory) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let slots = survival.hotbar.iter().chain(&survival.pack).chain(&survival.craft_grid);
    for slot in slots.chain(std::iter::once(&survival.cursor)) {
        slot.map(|s| (s.item, s.count)).hash(&mut h);
    }
    survival.resource_remainders.hash(&mut h);
    h.finish()
}

type TreeKey = (u64, bool, Item, u32, Choices);

/// The Tree tab's plan, rebuilt only when stock, target, amount or choices change.
fn tree_plan(ctx: &egui::Context, survival: &SurvivalInventory, bench: bool, item: Item, amount: u32, choices: &Choices) -> Arc<Plan> {
    let id = egui::Id::new("jei_tree_plan");
    let key: TreeKey = (stock_key(survival), bench, item, amount, choices.clone());
    if let Some((cached, plan)) = ctx.data(|d| d.get_temp::<(TreeKey, Arc<Plan>)>(id))
        && cached == key
    {
        return plan;
    }
    let mut stock: HashMap<Item, f32> = index::known_items()
        .iter()
        .map(|&i| (i, stock_of(survival, i)))
        .filter(|(_, n)| *n > 0.0)
        .collect();
    // The target itself is what is wanted, not what is used up.
    stock.remove(&item);
    let plan = Arc::new(index::plan(item, amount as f32, &mut stock, choices));
    ctx.data_mut(|d| d.insert_temp(id, (key, plan.clone())));
    plan
}

fn amount_text(n: f32) -> String {
    if (n - n.round()).abs() < 1e-3 {
        format!("{}", n.round() as i64)
    } else if n >= 10.0 {
        format!("{n:.1}")
    } else {
        let s = format!("{n:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Whole items the player carries, counting partial resource remainders.
fn stock_of(survival: &SurvivalInventory, item: Item) -> f32 {
    let whole = survival.count_item(item) as f32;
    match item {
        Item::Resource(i) => {
            whole + survival.resource_remainders.get(i as usize).copied().unwrap_or(0) as f32 / 64.0
        }
        _ => whole,
    }
}

/// What a click on an item inside the browser asks for.
enum Nav {
    Recipes(Item),
    Uses(Item),
}

/// One item cell. Left click → recipes, right click → uses (as in JEI).
fn item_cell(
    ui: &mut egui::Ui,
    item: Item,
    amount: Option<f32>,
    px: f32,
    highlight: Option<Color32>,
    nav: &mut Option<Nav>,
    survival: &SurvivalInventory,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(px, px), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 0, Color32::from_rgb(24, 21, 25));
    p.rect_stroke(
        rect,
        0,
        Stroke::new(1.0, highlight.unwrap_or(if resp.hovered() { AMBER } else { EDGE })),
        egui::StrokeKind::Inside,
    );
    draw_item_icon(p, rect.shrink(px * 0.14), item);
    if let Some(n) = amount {
        p.text(
            rect.right_bottom() - vec2(1.5, 0.5),
            Align2::RIGHT_BOTTOM,
            amount_text(n),
            FontId::monospace(10.0),
            PAPER,
        );
    }
    let resp = resp.on_hover_ui(|ui| {
        ui.label(RichText::new(item.name()).color(PAPER).size(12.5));
        super::description_copy::item(ui, item);
        if ui.input(|i| i.modifiers.shift) {
            ui.label(RichText::new(format!("Carried {}x", amount_text(stock_of(survival, item))))
                .size(11.0).color(MUTED));
        }
    });
    let (recipes, uses) = ui.input(|i| (
        resp.hovered() && !ui.ctx().wants_keyboard_input() && i.key_pressed(egui::Key::R),
        resp.hovered() && !ui.ctx().wants_keyboard_input() && i.key_pressed(egui::Key::U),
    ));
    if resp.clicked() || recipes {
        *nav = Some(Nav::Recipes(item));
    } else if resp.secondary_clicked() || uses {
        *nav = Some(Nav::Uses(item));
    }
    resp
}

fn chip(ui: &mut egui::Ui, text: &str, colour: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 8))
        .stroke(Stroke::new(1.0, EDGE))
        .inner_margin(egui::Margin::symmetric(4, 0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(colour));
        });
}

/// Returns true when the player staged a grid recipe (go to the inventory).
fn recipe_card(
    ui: &mut egui::Ui,
    e: &'static Entry,
    focus: Item,
    survival: &mut SurvivalInventory,
    bench: bool,
    nav: &mut Option<Nav>,
) -> bool {
    let mut staged = false;
    egui::Frame::new()
        .fill(Color32::from_rgb(33, 28, 32))
        .stroke(Stroke::new(1.0, EDGE))
        .inner_margin(egui::Margin::same(5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                draw_item_icon(ui.painter(), r, e.station.icon());
                ui.label(RichText::new(e.station.name()).size(11.0).color(JADE));
                ui.label(RichText::new(&e.label).size(11.0).color(PAPER));
                if let Some(grid) = e.grid {
                    let recipe = &all_recipes()[grid];
                    let reachable = bench || !recipe.needs_bench();
                    let craftable = reachable
                        && recipe
                            .ingredients
                            .iter()
                            .all(|&(it, n)| survival.count_item(it) >= n);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let tip = if !reachable {
                            "Workbench Required"
                        } else if craftable {
                            "Ingredient Transfer"
                        } else {
                            "Missing Ingredients"
                        };
                        if ui
                            .add_enabled(craftable, egui::Button::new("+").small())
                            .on_hover_text(tip)
                            .on_disabled_hover_text(tip)
                            .clicked()
                            && survival.stage_recipe_into_grid(recipe, bench)
                        {
                            staged = true;
                        }
                    });
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                for &(item, n) in &e.inputs {
                    let enough = stock_of(survival, item) + 1e-4 >= n;
                    let edge = (item == focus).then_some(AMBER).or((!enough).then_some(DANGER));
                    item_cell(ui, item, Some(n), 30.0, edge, nav, survival);
                }
                ui.label(RichText::new("→").size(15.0).color(MUTED));
                if e.outputs.is_empty() {
                    chip(ui, &format!("⚡ {}", power::fmt_joules(e.yields as f64)), MANA);
                }
                for &(item, n) in &e.outputs {
                    let edge = (item == focus).then_some(AMBER);
                    item_cell(ui, item, Some(n), 30.0, edge, nav, survival);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(3.0, 2.0);
                if e.seconds > 0.0 {
                    chip(ui, &format!("{:.1} s", e.seconds), PAPER);
                }
                if e.energy > 0.0 {
                    chip(ui, &power::fmt_power(e.draw_kw() as f64), AMBER);
                    chip(ui, &power::fmt_joules(e.energy as f64), MUTED);
                }
                if e.yields > 0.0 && e.seconds > 0.0 {
                    chip(
                        ui,
                        &power::fmt_power(e.yields as f64 / (e.seconds as f64 / power::TICK_SECONDS)),
                        MANA,
                    );
                }
                if matches!(e.station, Station::Multiblock(_)) {
                    chip(ui, "Multiblock", AMBER);
                }
            });
        });
    staged
}

fn tree_row(
    ui: &mut egui::Ui,
    node: &Plan,
    path: &str,
    choices: &mut Choices,
    nav: &mut Option<Nav>,
    survival: &SurvivalInventory,
) {
    let rest = node.need - node.from_stock;
    let header = |ui: &mut egui::Ui, choices: &mut Choices, nav: &mut Option<Nav>| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
            item_cell(ui, node.item, None, 20.0, None, nav, survival);
            ui.label(
                RichText::new(format!("{} ×{}", node.item.name(), amount_text(node.need)))
                    .size(11.0)
                    .color(PAPER),
            );
            if node.from_stock > 0.0 {
                chip(ui, &format!("have {}", amount_text(node.from_stock)), JADE);
            }
            match node.recipe {
                Some(r) => {
                    chip(ui, &format!("{} ×{}", r.station.name(), node.batches), MUTED);
                    if node.options > 1
                        && ui
                            .small_button("⟳")
                            .on_hover_text(format!(
                                "Use another recipe ({} available)",
                                node.options
                            ))
                            .clicked()
                    {
                        let current = choices
                            .get(&node.item)
                            .copied()
                            .unwrap_or_else(|| index::default_choice(node.item));
                        choices.insert(node.item, (current + 1) % node.options);
                    }
                }
                None if rest > 1e-4 => {
                    let text = if node.cycle {
                        format!("loop · find {}", amount_text(rest))
                    } else {
                        format!("gather {}", amount_text(rest))
                    };
                    chip(ui, &text, DANGER);
                }
                None => {}
            }
        });
    };
    if node.children.is_empty() {
        ui.horizontal(|ui| {
            // Line leaves up with the branch rows' toggle arrows.
            ui.add_space(ui.spacing().icon_width + ui.spacing().item_spacing.x);
            header(ui, choices, nav);
        });
        return;
    }
    let id = ui.make_persistent_id(path);
    egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        path.matches('/').count() < 3,
    )
    .show_header(ui, |ui| header(ui, choices, nav))
    .body(|ui| {
        for (k, c) in node.children.iter().enumerate() {
            tree_row(ui, c, &format!("{path}/{k}"), choices, nav, survival);
        }
    });
}

/// Draws the page. Returns true when a recipe was staged into the grid, so
/// the caller can switch back to the inventory.
pub(super) fn draw(ui: &mut egui::Ui, survival: &mut SurvivalInventory, bench: bool) -> bool {
    let ctx = ui.ctx().clone();
    let get = |id: &str| egui::Id::new(id);
    let mut selected = ctx
        .data_mut(|d| d.get_temp::<Option<Item>>(get(SELECTED)))
        .flatten();
    let mut tab = ctx.data_mut(|d| d.get_temp::<u8>(get(TAB)).unwrap_or(0));
    let mut query = ctx.data_mut(|d| d.get_temp::<String>(get(QUERY)).unwrap_or_default());
    let mut amount = ctx.data_mut(|d| d.get_temp::<u32>(get(AMOUNT)).unwrap_or(1));
    let mut choices = ctx.data_mut(|d| d.get_temp::<Choices>(get(CHOICES)).unwrap_or_default());
    let mut history = ctx.data_mut(|d| d.get_temp::<Vec<Item>>(get(HISTORY)).unwrap_or_default());
    let mut nav: Option<Nav> = None;
    let mut staged = false;

    let width = ui.available_width();
    let list_w = (width * 0.38).clamp(170.0, 330.0);
    // Lists take what the window has left, leaving room for the tab strip
    // (two rows at the default scale) and the footer.
    let height = ui
        .available_height()
        .min(ui.ctx().content_rect().height() - 250.0)
        .max(180.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        // Right: item panel. Search remains anchored beneath its scroll area.
        ui.vertical(|ui| {
            ui.set_width(list_w);
            ui.horizontal(|ui| {
                ui.label(RichText::new("ITEMS").size(11.0).color(AMBER));
            });
            let q = query.trim().to_lowercase();
            let px = 30.0;
            let cols = ((list_w + 2.0) / (px + 2.0)).floor().max(1.0) as usize;
            egui::ScrollArea::vertical()
                .id_salt("jei_items")
                .max_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("jei_item_grid").spacing([2.0, 2.0]).show(ui, |ui| {
                        let mut col = 0;
                        for &item in index::known_items() {
                            let matches = if let Some(station) = q.strip_prefix('@') {
                                index::producing(item)
                                    .iter()
                                    .any(|e| e.station.name().to_lowercase().contains(station))
                            } else {
                                q.is_empty()
                                    || item.name().to_lowercase().contains(&q)
                                    || item.category().to_lowercase().contains(&q)
                            };
                            if !matches {
                                continue;
                            }
                            let carried = stock_of(survival, item);
                            let edge = (selected == Some(item)).then_some(AMBER);
                            let r = item_cell(ui, item, None, px, edge, &mut nav, survival);
                            if carried > 0.0 {
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(r.rect.left_top() + vec2(2.0, 2.0), vec2(4.0, 4.0)),
                                    0,
                                    JADE,
                                );
                            }
                            col += 1;
                            if col % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });
                });
            ui.add(egui::TextEdit::singleline(&mut query)
                .hint_text("Search")
                .desired_width(list_w));
        });
        ui.separator();
        // Right: the selected item.
        ui.vertical(|ui| {
            let Some(item) = selected else {
                ui.add_space(20.0);
                ui.label(RichText::new("Recipes").color(MUTED));
                return;
            };
            ui.horizontal(|ui| {
                if !history.is_empty()
                    && ui.small_button("◀").on_hover_text("Back").clicked()
                {
                    selected = history.pop();
                    return;
                }
                let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                draw_item_icon(ui.painter(), r, item);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add(egui::Label::new(RichText::new(item.name()).size(14.0).color(PAPER)).wrap_mode(egui::TextWrapMode::Extend));
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "{} · carried {}",
                                item.category(),
                                amount_text(stock_of(survival, item))
                            ))
                            .size(11.0)
                            .color(JADE),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                });
            });
            let made = index::producing(item);
            let used = index::using(item);
            ui.horizontal(|ui| {
                for (k, label) in [
                    (0u8, format!("Recipes {}", made.len())),
                    (1, format!("Uses {}", used.len())),
                    (2, "Tree".to_string()),
                ] {
                    if ui.selectable_label(tab == k, label).clicked() {
                        tab = k;
                    }
                }
            });
            egui::ScrollArea::vertical()
                .id_salt(("jei_detail", tab))
                .max_height(height - 40.0)
                .auto_shrink([false, false])
                .show(ui, |ui| match tab {
                    0 | 1 => {
                        let list = if tab == 0 { &made } else { &used };
                        if list.is_empty() {
                            ui.label(
                                RichText::new(if tab == 0 {
                                    "Gathered, not made: mine it, harvest it or find it in caches."
                                } else {
                                    "Nothing uses this yet."
                                })
                                .color(MUTED),
                            );
                        }
                        for e in list.iter() {
                            staged |= recipe_card(ui, e, item, survival, bench, &mut nav);
                        }
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Make").size(11.0).color(MUTED));
                            for n in [1u32, 4, 16, 64] {
                                if ui.selectable_label(amount == n, format!("×{n}")).clicked() {
                                    amount = n;
                                }
                            }
                            if !choices.is_empty()
                                && ui.small_button("Reset").on_hover_text("Back to the default recipes").clicked()
                            {
                                choices.clear();
                            }
                        });
                        let plan = tree_plan(&ctx, survival, bench, item, amount, &choices);
                        let (seconds, energy) = plan.totals();
                        ui.horizontal_wrapped(|ui| {
                            chip(ui, &format!("{seconds:.0}s"), PAPER);
                            if energy > 0.0 {
                                chip(ui, &power::fmt_energy(energy as f64), AMBER);
                            }
                        });
                        let missing = plan.missing();
                        if missing.is_empty() {
                            ui.label(RichText::new("Materials Ready").size(11.0).color(JADE));
                        } else {
                            ui.label(RichText::new("Missing Materials").size(11.0).color(DANGER));
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                                for (i, n) in &missing {
                                    item_cell(ui, *i, Some(*n), 28.0, Some(DANGER), &mut nav, survival);
                                }
                            });
                        }
                        ui.separator();
                        tree_row(ui, &plan, "jei_tree", &mut choices, &mut nav, survival);
                    }
                });
        });
    });

    match nav {
        Some(Nav::Recipes(item)) | Some(Nav::Uses(item)) => {
            if selected != Some(item) {
                if let Some(prev) = selected {
                    history.push(prev);
                    if history.len() > 24 {
                        history.remove(0);
                    }
                }
                selected = Some(item);
            }
            tab = u8::from(matches!(nav, Some(Nav::Uses(_))));
        }
        None => {}
    }
    ctx.data_mut(|d| {
        d.insert_temp(get(SELECTED), selected);
        d.insert_temp(get(TAB), tab);
        d.insert_temp(get(QUERY), query);
        d.insert_temp(get(AMOUNT), amount);
        d.insert_temp(get(CHOICES), choices);
        d.insert_temp(get(HISTORY), history);
    });
    let _ = presentation::INK;
    staged
}
