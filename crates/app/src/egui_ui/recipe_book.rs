//! Shared JEI-style item catalogue on the right of inventory; recipe details
//! and a production-tree calculator in a separate sheet. Hover any item and
//! press **R** for its recipes or **U** for its uses.
//!
//! Data comes only from `world::recipe_index`; this module draws and keeps a
//! little view state in egui's temp storage.
use super::inventory::draw_item_icon;
use super::presentation::{self, AMBER, DANGER, EDGE, JADE, MANA, MUTED, PAPER};
use crate::world::factory::power;
use crate::world::inventory::{CRAFT_SLOTS, CraftingRecipe, Item, SlotAddr, SurvivalInventory, all_recipes};
use crate::world::recipe_index::{self as index, Choices, Entry, Group, Plan, Station};
use crate::world::technology;
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, vec2};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

mod history;
mod pins;

use crate::world::progression::study::{Lesson, View as StudyView};

const STUDY_VIEW: &str = "bookcase_study_view";
const STUDY_REQUEST: &str = "bookcase_study_request";

pub(crate) fn publish_study(ctx: &egui::Context, view: StudyView) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(STUDY_VIEW), view));
}

pub(super) fn take_study(ctx: &egui::Context) -> Option<(Lesson, bool, bool)> {
    ctx.data_mut(|d| d.remove_temp::<Option<(Lesson, bool, bool)>>(egui::Id::new(STUDY_REQUEST))).flatten()
}

pub(super) fn study(ui: &mut egui::Ui) {
    let Some(view) = ui.ctx().data(|d| d.get_temp::<StudyView>(egui::Id::new(STUDY_VIEW))) else { return };
    if view.lessons & crate::world::progression::study::CONDUCTION_OBSERVED != 0 {
        ui.collapsing("Arcana · Observations", |ui| {
            ui.label("Thunder Charge → Connected water → Storm Surge");
            ui.label("Storm Surge · Tier III");
        });
    }
    egui::CollapsingHeader::new(format!("Bookcase study · {} XP", view.xp))
        .id_salt("bookcase_study")
        .default_open(true)
        .show(ui, |ui| {
        if !view.can_study {
            ui.weak("Bookcase required");
        }
        for lesson in Lesson::ALL {
            let learned = lesson.learned(view.lessons, view.horticulture);
            let ready = lesson.ready(view.lessons, view.horticulture);
            ui.horizontal_wrapped(|ui| {
                ui.label(lesson.title());
                ui.label(RichText::new(lesson.detail()).color(MUTED));
                if learned {
                    ui.label(RichText::new("Learned").color(JADE));
                    if ui.add_enabled(view.can_study && view.supplies & (1 << lesson as u8) != 0,
                        egui::Button::new(format!("Research · +{} XP", lesson.research_xp()))).clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(STUDY_REQUEST), Some((lesson, true, true))));
                    }
                } else {
                    if let Some(parent) = lesson.project().and_then(|row| row.requires)
                        && !ready {
                        ui.weak(format!("Requires: {}", parent.title()));
                    }
                    if ui.add_enabled(view.can_study && ready && view.xp >= lesson.cost(), egui::Button::new(format!("Study · {} XP", lesson.cost()))).clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(STUDY_REQUEST), Some((lesson, false, false))));
                    }
                    if ui.add_enabled(view.can_study && ready && view.supplies & (1 << lesson as u8) != 0,
                        egui::Button::new("Submit supplies")).clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(STUDY_REQUEST), Some((lesson, true, false))));
                    }
                }
            });
            if let Some(project) = lesson.project() {
                ui.horizontal_wrapped(|ui| {
                    for (index, &(item, count)) in project.samples.iter().enumerate() {
                        let owned = view.supplied[lesson as usize][index];
                        let (rect, response) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                        draw_item_icon(ui.painter(), rect, item);
                        response.on_hover_text(item.name());
                        ui.label(RichText::new(format!("{owned}/{count}"))
                            .color(if owned >= count { JADE } else { MUTED }));
                    }
                });
            }
        }
    });
}

const REQUEST: &str = "jei_request";
const SELECTED: &str = "jei_selected";
const TAB: &str = "jei_tab";
const QUERY: &str = "jei_query";
const AMOUNT: &str = "jei_amount";
const CHOICES: &str = "jei_choices";
const GOAL_VIEW: &str = "jei_goal_view";
const GUIDE_OPEN: &str = "magic_guide_open";

pub(super) fn guide_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(GUIDE_OPEN)).unwrap_or(false))
}

pub(crate) fn open_magic_guide(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("inventory_page"), 6_u32);
        d.insert_temp(egui::Id::new(GUIDE_OPEN), true);
    });
}

/// Back to the recipe list; the guide stays closed until its button is pressed.
pub(crate) fn close_magic_guide(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(GUIDE_OPEN), false));
}

fn portal_guide(ui: &mut egui::Ui) {
    use crate::world::BlockId as B;
    egui::CollapsingHeader::new("Portals").show(ui, |ui| {
        let mut hell = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("portal_guide_hell"))).unwrap_or(true);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut hell, true, "Hell");
            ui.selectable_value(&mut hell, false, "Crystal Link");
        });
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("portal_guide_hell"), hell));
        let material = if hell { B::Basalt } else { B::Crystal };
        // Physical dimensions, rather than a block count that breaks at finer
        // voxel scales. Corners are optional for both kinds of frame.
        egui::Grid::new("portal_structure").spacing([2.0, 2.0]).min_col_width(22.0).min_row_height(22.0).show(ui, |ui| {
            for y in 0..5 { for x in 0..4 {
                let edge = y == 0 || y == 4 || x == 0 || x == 3;
                let corner = (y == 0 || y == 4) && (x == 0 || x == 3);
                let (r, response) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                if edge && !corner {
                    draw_item_icon(ui.painter(), r, Item::Block(material));
                    response.on_hover_text(Item::Block(material).name());
                } else if !edge {
                    ui.painter().rect_filled(r, 0.0, if hell { Color32::from_rgb(85, 18, 16) } else { Color32::from_rgb(45, 30, 75) });
                }
            } ui.end_row(); }
        });
        ui.label("2 m × 3 m opening · Optional corners");
        ui.collapsing("Field notes", |ui| {
            ui.label("Hills · Look for ruined basalt arches and stairs below ground.");
            ui.label("Near the surface Hell ruin, look for the Hall's departure chart.");
            ui.label("Hall · Defeat the Furnace Saint and Chain Astronomer to open the Nether and Manyfold charts.");
            ui.label("Surveyors called the crystal links windows. The basalt gates carried freight across realms.");
            if ui.button("Bookcase recipe").clicked() {
                request(ui.ctx(), Item::Block(B::Bookcase), false);
            }
            if let Some(view) = ui.ctx().data(|d| d.get_temp::<StudyView>(egui::Id::new(STUDY_VIEW)))
                && Lesson::Thresholds.learned(view.lessons, view.horticulture)
            {
                ui.label("Matching crystal openings need the same size. Keep both exits clear.");
                ui.label("Spark must strike basalt from within 5 m. Use a one-cell-thick frame.");
                if let Some((x, z)) = view.sanctuary {
                    ui.label(format!("Surveyed sanctuary · X {x} m · Z {z} m"));
                } else {
                    ui.label("Read a porter's record to survey sanctuaries in the Hills.");
                }
            }
        });
        ui.collapsing("Threshold research and achievements", |ui| {
            super::records::thresholds(ui);
        });
        if hell {
            ui.label("Cast Spark at the basalt frame.");
            ui.label("[Interact] Cross");
            ui.collapsing("Frame limits", |ui| {
                ui.label("2–4 m wide · 3–5 m tall");
                ui.label("First crossing builds a return frame.");
                ui.label("Online guests use linked gates.");
            });
            ui.collapsing("Survival", |ui| {
                ui.label("Hot ash · Overhead cover");
                ui.label("Amber Caps · Shaded Fungal Soil");
                ui.label("Amber Cap → Fire Resistance");
                ui.label("Bed · [Interact] Set spawn");
            });
        } else {
            ui.label("Build two matching crystal frames.");
            if ui.button("Resonant crystal block recipe").clicked() {
                request(ui.ctx(), Item::Block(B::Crystal), false);
            }
            ui.label("[Interact] Link each frame · Walk through");
        }
    });
}
const OPEN: &str = "jei_detail_open";
/// Layer preview matches the shell validated by world::smeltery.
fn smeltery_structure(ui: &mut egui::Ui) {
    use crate::world::BlockId as B;
    ui.collapsing("Structure · 5×5×3", |ui| {
        let id = egui::Id::new("smeltery_preview_layer");
        let mut layer = ui.ctx().data(|d| d.get_temp::<u8>(id)).unwrap_or(1);
        ui.horizontal(|ui| {
            for (i,label) in ["Floor", "Lower Wall", "Upper Wall"].into_iter().enumerate() {
                ui.selectable_value(&mut layer, i as u8, label);
            }
        });
        ui.ctx().data_mut(|d| d.insert_temp(id, layer));
        egui::Grid::new("smeltery_structure_grid").spacing([4.0,4.0]).min_col_width(28.0).min_row_height(28.0).show(ui, |ui| {
            for z in 0..5 { for x in 0..5 {
                let block = if layer==0 || x==0 || x==4 || z==0 || z==4 {
                    Some(if layer==1 && z==4 && x==2 { B::SmelteryController }
                        else if layer==1 && z==4 && x==3 { B::SmelteryDrain } else { B::SearedBrick })
                } else { None };
                let (rect,response)=ui.allocate_exact_size(vec2(28.0,28.0),Sense::hover());
                if let Some(b)=block { draw_item_icon(ui.painter(),rect,Item::Block(b)); response.on_hover_text(Item::Block(b).name()); }
            } ui.end_row(); }
        });
        ui.label("55 Seared Bricks · 1 Controller · 1 Drain");
        ui.label("Drain → Faucet · Cast 2 Cells Below Faucet");
        ui.label("Bronze · 3 Copper + 1 Tin");
    });
}
/// The inventory page's ghost: a grid recipe shown without its ingredients.
const GHOST: &str = "survival_ghost_recipe";
pub(super) const SIDEBAR_WIDTH: f32 = 364.0;
const GROUP: &str = "jei_group";

/// Search metadata is immutable; only craftable filtering depends on inventory.
fn filtered_items(ctx: &egui::Context, query: &str, survival: &SurvivalInventory, bench: bool, ready: bool) -> Arc<Vec<Item>> {
    let query = query.trim().to_lowercase();
    let group = ctx.data(|d| d.get_temp::<Option<Group>>(egui::Id::new(GROUP))).flatten();
    let key = (query.clone(), ready.then(|| stock_key(survival)), bench, group);
    let id = egui::Id::new(("jei_filtered_items", ready));
    type Cached = ((String, Option<u64>, bool, Option<Group>), Arc<Vec<Item>>);
    if let Some((old, items)) = ctx.data(|d| d.get_temp::<Cached>(id)) {
        if old == key { return items; }
    }
    let craftable = key.1.map(|stock| ready_items(ctx, survival, bench, stock));
    let items = Arc::new(index::known_items().iter().copied().filter(|&item| {
        group.is_none_or(|g| Group::of(item) == g)
            && craftable.as_ref().is_none_or(|set| set.contains(&item))
            && technology::matches(item, &query)
    }).collect());
    ctx.data_mut(|d| d.insert_temp(id, (key, Arc::clone(&items))));
    items
}

fn search_row(ui: &mut egui::Ui, query: &mut String, width: f32) {
    super::inventory::search::row(ui, query, width);
}

/// Ask the browser to open on `item`: its uses when `uses`, else its recipes.
pub(super) fn request(ctx: &egui::Context, item: Item, uses: bool) {
    request_tab(ctx, item, u8::from(uses));
}

fn request_tab(ctx: &egui::Context, item: Item, tab: u8) {
    request_page(ctx, history::Page::browse(ctx, item, tab));
}

fn request_page(ctx: &egui::Context, page: history::Page) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(REQUEST), Some(page)));
}

/// True once when a request is waiting; the inventory switches to this page.
pub(super) fn take_request(ctx: &egui::Context) -> bool {
    let pending = ctx.data_mut(|d| d.remove_temp::<Option<history::Page>>(egui::Id::new(REQUEST))).flatten();
    if let Some(page) = pending {
        history::visit(ctx, page);
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new(OPEN), true);
            d.insert_temp(egui::Id::new(GUIDE_OPEN), false);
        });
        return true;
    }
    false
}

/// The same catalogue stays beside the crafting grid, `height` tall. Browsing
/// never crafts or creates items; ingredient transfer remains the recipe [+].
pub(super) fn sidebar(ui: &mut egui::Ui, survival: &SurvivalInventory, bench: bool, height: f32, size: i32) {
    let ctx = ui.ctx().clone();
    let heading_top = ui.cursor().top();
    ui.label(RichText::new("Items").color(PAPER));
    let height = (height - (ui.cursor().top() - heading_top)).max(80.0);
    let mut query = ctx.data_mut(|d| {
        d.get_temp::<String>(egui::Id::new(QUERY))
            .unwrap_or_default()
    });
    let selected = ctx
        .data_mut(|d| d.get_temp::<Option<Item>>(egui::Id::new(SELECTED)))
        .flatten();
    let mut nav = None;
    let ready_id = egui::Id::new("jei_ready_only");
    let mut ready = ctx.data_mut(|d| d.get_temp::<bool>(ready_id).unwrap_or(false));
    ui.set_width(SIDEBAR_WIDTH);
    search_row(ui, &mut query, SIDEBAR_WIDTH);
    let tabs_top = ui.cursor().top();
    pins::draw(ui, survival, bench, size, &mut nav);
    group_tabs(ui, SIDEBAR_WIDTH);
    let tabs_h = ui.cursor().top() - tabs_top;
    let items = filtered_items(&ctx, &query, survival, bench, ready);
    // The page header, group tabs and search row take the rest of the container's height.
    let grid_h = (height - 64.0 - tabs_h).max(96.0);
    paged_items(ui, "inventory_jei", &items, SIDEBAR_WIDTH, grid_h, selected, false, &mut nav, survival, |ui| {
        ui.toggle_value(&mut ready, "Ready").on_hover_text("Craftable Now");
        super::inventory::search::summary(ui, survival);
    });
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new(QUERY), query);
        d.insert_temp(ready_id, ready);
    });
    match nav {
        Some(Nav::Recipes(item)) => request(&ctx, item, false),
        Some(Nav::Uses(item)) => request(&ctx, item, true),
        None => {}
    }
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

/// Items with a grid recipe the carried stock covers, cached per stock and bench.
fn ready_items(ctx: &egui::Context, survival: &SurvivalInventory, bench: bool, stock: u64) -> Arc<HashSet<Item>> {
    let id = egui::Id::new("jei_ready_items");
    let key = (stock, bench);
    if let Some((cached, set)) = ctx.data(|d| d.get_temp::<((u64, bool), Arc<HashSet<Item>>)>(id))
        && cached == key
    {
        return set;
    }
    let mut counts = HashMap::<Item, u32>::new();
    for stack in survival.hotbar.iter().chain(&survival.pack).chain(&survival.craft_grid)
        .chain(std::iter::once(&survival.cursor)).flatten()
    {
        let count = counts.entry(stack.item).or_default();
        *count = count.saturating_add(stack.count);
    }
    let set: Arc<HashSet<Item>> = Arc::new(
        index::known_items()
            .iter()
            .copied()
            .filter(|&item| {
                index::producing(item).iter().any(|entry| {
                    entry.grid.is_some_and(|grid| {
                        let recipe = &all_recipes()[grid];
                        (bench || !recipe.needs_bench())
                            && recipe.ingredients.iter().all(|&(i, n)| counts.get(&i).copied().unwrap_or(0) >= n)
                    })
                })
            })
            .collect(),
    );
    ctx.data_mut(|d| d.insert_temp(id, (key, set.clone())));
    set
}

type TreeKey = (u64, bool, Item, u32, Choices, i32);

/// The Tree tab's plan, rebuilt only when stock, target, amount or choices change.
fn tree_plan(ctx: &egui::Context, survival: &SurvivalInventory, bench: bool, item: Item, amount: u32, choices: &Choices, size: i32) -> Arc<Plan> {
    let id = egui::Id::new("jei_tree_plan");
    let key: TreeKey = (stock_key(survival), bench, item, amount, choices.clone(), size);
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
    if bench {
        stock.insert(Station::Bench.icon(), 1.0);
    }
    let plan = Arc::new(index::plan_with_facilities(item, amount as f32, &mut stock, choices, size));
    ctx.data_mut(|d| d.insert_temp(id, (key, plan.clone())));
    plan
}

/// One icon tab per catalogue group, plus All; a click on the open tab clears it.
fn group_tabs(ui: &mut egui::Ui, width: f32) {
    let id = egui::Id::new(GROUP);
    let mut group = ui.ctx().data(|d| d.get_temp::<Option<Group>>(id)).flatten();
    ui.horizontal_wrapped(|ui| {
        ui.set_max_width(width);
        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
        if ui.selectable_label(group.is_none(), RichText::new("All").size(13.0)).clicked() {
            group = None;
        }
        for g in Group::ALL {
            let on = group == Some(g);
            // Same frame as the page tabs; 32 px inside so sheet sprites draw at 2x.
            let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
            super::inventory::screen::tab(ui.painter(), rect, 1.0, on);
            draw_item_icon(ui.painter(), rect.shrink(2.0), g.icon());
            if resp.on_hover_text(g.name()).clicked() {
                group = if on { None } else { Some(g) };
            }
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, group));
}

/// JEI's item list: a fixed grid that pages instead of scrolling. `header`
/// draws on the left of the `< n / m >` row; the wheel over the grid turns
/// pages. `dots` marks items the player carries.
#[allow(clippy::too_many_arguments)]
fn paged_items(
    ui: &mut egui::Ui,
    id: &str,
    items: &Arc<Vec<Item>>,
    width: f32,
    height: f32,
    selected: Option<Item>,
    dots: bool,
    nav: &mut Option<Nav>,
    survival: &SurvivalInventory,
    header: impl FnOnce(&mut egui::Ui),
) {
    // Sprites paint at whole texels, so the icon area holds a whole multiple of
    // 16 px item sprites and of 14 px block sprites (drawn at 0.86 of it); a
    // fixed 36 pt cell drew both at one texel per pixel at 1 ppp.
    let ppp = ui.ctx().pixels_per_point();
    let px = (31.0 * ppp / 16.3).round().max(1.0) * 16.3 / ppp / 0.86;
    let pitch = px + 4.0;
    let cols = ((width + 4.0) / pitch).floor().max(1.0) as usize;
    let rows = ((height + 4.0) / pitch).floor().max(1.0) as usize;
    let per_page = cols * rows;
    let pages = items.len().div_ceil(per_page).max(1);
    let page_id = egui::Id::new((id, "page"));
    let mut page = ui.ctx().data(|d| d.get_temp::<usize>(page_id)).unwrap_or(0).min(pages - 1);
    let signature_id = egui::Id::new((id, "results"));
    let previous = ui.ctx().data(|d| d.get_temp::<Arc<Vec<Item>>>(signature_id));
    if previous.as_ref().is_none_or(|old| !Arc::ptr_eq(old, items)) {
        if previous.as_deref() != Some(items.as_ref()) {
            page = 0;
        }
        ui.ctx().data_mut(|d| d.insert_temp(signature_id, Arc::clone(items)));
    }
    ui.horizontal(|ui| {
        ui.set_width(width);
        header(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(page + 1 < pages, egui::Button::new(">").small()).clicked() {
                page += 1;
            }
            ui.label(RichText::new(format!("{} / {pages}", page + 1)).size(13.0).color(MUTED));
            if ui.add_enabled(page > 0, egui::Button::new("<").small()).clicked() {
                page -= 1;
            }
        });
    });
    let (grid, _) = ui.allocate_exact_size(vec2(width, rows as f32 * pitch), Sense::hover());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(grid));
    if items.is_empty() { child.label(RichText::new("No Matches").color(MUTED)); }
    egui::Grid::new((id, "grid")).spacing([4.0, 4.0]).min_col_width(px).show(&mut child, |ui| {
        for (k, &item) in items.iter().skip(page * per_page).take(per_page).enumerate() {
            let edge = (selected == Some(item)).then_some(AMBER);
            let r = item_cell(ui, item, None, px, edge, nav, survival);
            if dots && stock_of(survival, item) > 0.0 {
                ui.painter().rect_filled(egui::Rect::from_min_size(r.rect.left_top() + vec2(2.0, 2.0), vec2(4.0, 4.0)), 0, JADE);
            }
            if (k + 1) % cols == 0 {
                ui.end_row();
            }
        }
    });
    if ui.rect_contains_pointer(grid) {
        let wheel = ui.input(|i| i.smooth_scroll_delta().y);
        if wheel < 0.0 && page + 1 < pages {
            page += 1;
        } else if wheel > 0.0 && page > 0 {
            page -= 1;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(page_id, page));
}

pub(super) fn detail_window(ctx: &egui::Context, survival: &mut SurvivalInventory, bench: bool, size: i32) {
    let id = egui::Id::new(OPEN);
    let mut open = ctx.data_mut(|d| d.get_temp::<bool>(id).unwrap_or(false));
    if open {
        let mut staged = false;
        let response = egui::Window::new("Recipes")
            .id(egui::Id::new("jei_detail_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .max_height((ctx.content_rect().height() - 32.0).max(120.0))
            .max_width((ctx.content_rect().width() - 32.0).max(200.0))
            .default_pos(ctx.content_rect().center() - vec2(280.0, 220.0))
            .show(ctx, |ui| {
                staged = draw_mode(ui, survival, bench, false, size);
            });
        if let Some(response) = response {
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("jei_detail_rect"), response.response.rect)
            });
        }
        if staged {
            open = false;
        }
    }
    if !open {
        ctx.data_mut(|d| d.remove::<egui::Rect>(egui::Id::new("jei_detail_rect")));
    }
    ctx.data_mut(|d| d.insert_temp(id, open));
}

use super::icons::amount_text;

/// Whole items the player carries, counting partial resource remainders.
fn stock_of(survival: &SurvivalInventory, item: Item) -> f32 {
    let whole = survival.count_item(item) as f32;
    match item {
        Item::Resource(i) => {
            whole
                + survival
                    .resource_remainders
                    .get(i as usize)
                    .copied()
                    .unwrap_or(0) as f32
                    / 64.0
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
    p.rect_filled(rect, 0, if super::themes::get(p.ctx()) == 0 { Color32::from_rgb(24, 21, 25) }
        else { super::themes::current(p.ctx()).recess });
    p.rect_stroke(
        rect,
        0,
        Stroke::new(
            1.0,
            highlight.unwrap_or(if resp.hovered() { AMBER } else { EDGE }),
        ),
        egui::StrokeKind::Inside,
    );
    draw_item_icon(p, rect.shrink(px * 0.07), item);
    if let Some(n) = amount {
        p.text(
            rect.right_bottom() - vec2(1.5, 0.5),
            Align2::RIGHT_BOTTOM,
            amount_text(n),
            // Scales with the cell so a four-character amount stays inside it.
            FontId::monospace((px * 0.34).clamp(8.0, 12.0)),
            PAPER,
        );
    }
    let resp = resp.on_hover_ui(|ui| {
        ui.label(RichText::new(item.name()).color(PAPER).size(14.0));
        super::description_copy::item(ui, item);
        ui.weak(if pins::contains(ui.ctx(), item) { "[A] Unpin" } else { "[A] Pin" });
        if ui.input(|i| i.modifiers.shift) {
            ui.label(
                RichText::new(format!(
                    "Carried {}x",
                    amount_text(stock_of(survival, item))
                ))
                .size(13.0)
                .color(MUTED),
            );
        }
    });
    let shortcuts = resp.hovered() && !ui.ctx().egui_wants_keyboard_input();
    let (recipes, uses) = ui.input(|i| {
        (
            shortcuts && i.key_pressed(egui::Key::R),
            shortcuts && i.key_pressed(egui::Key::U),
        )
    });
    if shortcuts && ui.input(|i| i.events.iter().any(|event| matches!(event,
        egui::Event::Key { key: egui::Key::A, pressed: true, repeat: false, modifiers, .. }
            if !modifiers.command && !modifiers.alt))) {
        pins::toggle(ui.ctx(), item);
    }
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
            ui.label(RichText::new(text).size(13.0).color(colour));
        });
}

fn harvest_sources(ui: &mut egui::Ui, item: Item, size: i32, nav: &mut Option<Nav>, survival: &SurvivalInventory) -> bool {
    use crate::world::farming::table::{self, Planting};
    let sources = index::harvest_sources(item);
    let crop = match item {
        Item::Block(block) => table::by_seed(block).map(|(_, crop)| crop),
        _ => None,
    };
    if sources.is_empty() && crop.is_none() {
        return false;
    }
    ui.label(RichText::new("Harvest Sources").size(13.0).color(AMBER));
    let location = match item {
        Item::Resource(0 | 3) => Some("Look for copper and coal in exposed rock and cave walls."),
        Item::Resource(2) => Some("Search underground cave walls for iron veins."),
        Item::Block(crate::world::BlockId::Stone) => Some("Mine exposed stone on slopes or at cave entrances."),
        _ => None,
    };
    if let Some(location) = location {
        ui.weak(location);
    }
    let cells = index::cells_per_harvest_item(item, size);
    if cells > 1 {
        ui.label(RichText::new(format!("{cells} voxels per whole item")).size(13.0).color(MUTED));
    }
    for &block in sources {
        ui.horizontal_wrapped(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
            draw_item_icon(ui.painter(), rect, Item::Block(block));
            ui.label(RichText::new(block.display_name()).size(13.0).color(PAPER));
            if let Some(tool) = index::harvest_tool(block) {
                item_cell(ui, tool, None, 22.0, None, nav, survival);
                ui.label(RichText::new(format!("{} or better", tool.name())).size(13.0).color(MUTED));
            }
        });
    }
    if let Some(crop) = crop {
        if crop.wild_seed_per_mille > 0 {
            ui.label(RichText::new("Seed drop from wild plants").size(13.0).color(MUTED));
        }
        if crop.planting == Planting::Soil && crop.tree.is_none() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Grow on").size(13.0).color(MUTED));
                for &soil in &crop.soil {
                    item_cell(ui, Item::Block(soil), None, 22.0, None, nav, survival);
                }
                chip(ui, &format!("Light {}–{}", crop.light_min, crop.light_max), MUTED);
            });
        }
    }
    ui.separator();
    true
}

/// A grid recipe as the crafting grid lays it out: 2 × 2 for hand rows,
/// 3 × 3 for bench rows, empty cells kept so the shape reads at a glance.
fn pattern_grid(
    ui: &mut egui::Ui,
    recipe: &CraftingRecipe,
    focus: Item,
    nav: &mut Option<Nav>,
    survival: &SurvivalInventory,
) {
    let bench = recipe.needs_bench();
    let cols = if bench { 3 } else { 2 };
    let mut cells = [None; CRAFT_SLOTS];
    for (i, item, n) in recipe.cells(bench, 1).unwrap_or_default() {
        cells[i] = Some((item, n));
    }
    let px = 26.0;
    egui::Grid::new(("jei_pattern", recipe.name)).spacing([4.0, 4.0]).min_col_width(px).show(ui, |ui| {
        for r in 0..cols {
            for c in 0..cols {
                match cells[r * 3 + c] {
                    Some((item, n)) => {
                        let need = recipe.ingredients.iter().find(|(i, _)| *i == item).map_or(n, |&(_, k)| k);
                        let edge = (item == focus)
                            .then_some(AMBER)
                            .or((survival.count_item(item) < need).then_some(DANGER));
                        let amount = (n > 1).then_some(n as f32);
                        item_cell(ui, item, amount, px, edge, nav, survival);
                    }
                    None => {
                        let (rect, _) = ui.allocate_exact_size(vec2(px, px), Sense::hover());
                        ui.painter().rect_filled(rect, 0, if super::themes::get(ui.ctx()) == 0 { Color32::from_rgb(24, 21, 25) }
                            else { super::themes::current(ui.ctx()).recess });
                        ui.painter().rect_stroke(rect, 0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
                    }
                }
            }
            ui.end_row();
        }
    });
}

/// Reuse recorded operations so bulk crafting also replays online.
fn transfer_recipe(survival: &mut SurvivalInventory, recipe: &CraftingRecipe, bench: bool, bulk: bool) -> bool {
    let limit = recipe.cells(bench, 1).unwrap_or_default().iter()
        .map(|&(_, item, count)| survival.stack_limit(item) / count.max(1))
        .min().unwrap_or(0);
    let mut moved = false;
    for _ in 0..if bulk { limit } else { 1 } {
        let before = survival.craft_grid;
        if !survival.stage_recipe_into_grid(recipe, bench) { break; }
        moved |= survival.craft_grid != before;
        if survival.craft_grid == before { break; }
    }
    moved
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
        .fill(if super::themes::get(ui.ctx()) == 0 { Color32::from_rgb(33, 28, 32) }
            else { super::themes::current(ui.ctx()).panel })
        .stroke(Stroke::new(1.0, if super::themes::get(ui.ctx()) == 0 { EDGE }
            else { super::themes::current(ui.ctx()).edge }))
        .inner_margin(egui::Margin::same(5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
            ui.horizontal(|ui| {
                if e.station != Station::Hand {
                    item_cell(ui, e.station.icon(), None, 22.0, None, nav, survival);
                }
                ui.label(RichText::new(e.station.name()).size(13.0).color(JADE));
                ui.label(RichText::new(&e.label).size(13.0).color(PAPER));
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
                            "Ingredient Transfer · [Shift] Max"
                        } else {
                            "Missing Ingredients · Show Pattern"
                        };
                        if ui
                            .add_enabled(reachable, egui::Button::new("+").small())
                            .on_hover_text(tip)
                            .on_disabled_hover_text(tip)
                            .clicked()
                        {
                            staged = if craftable {
                                transfer_recipe(survival, recipe, bench, ui.input(|i| i.modifiers.shift))
                            } else {
                                // Vanilla recipe book: an unaffordable recipe shows as a
                                // ghost in an emptied grid.
                                for i in 0..CRAFT_SLOTS {
                                    let _ = survival.shift_click_slot(SlotAddr::CraftGrid(i));
                                }
                                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(GHOST), Some(grid)));
                                true
                            };
                        }
                    });
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                match e.grid {
                    Some(grid) => pattern_grid(ui, &all_recipes()[grid], focus, nav, survival),
                    None => {
                        for &(item, n) in &e.inputs {
                            let enough = stock_of(survival, item) + 1e-4 >= n;
                            let edge = (item == focus)
                                .then_some(AMBER)
                                .or((!enough).then_some(DANGER));
                            item_cell(ui, item, Some(n), 30.0, edge, nav, survival);
                        }
                    }
                }
                ui.label(RichText::new("→").size(15.0).color(MUTED));
                if e.outputs.is_empty() {
                    chip(
                        ui,
                        &format!("⚡ {}", power::fmt_joules(e.yields as f64)),
                        MANA,
                    );
                }
                for &(item, n) in &e.outputs {
                    let edge = (item == focus).then_some(AMBER);
                    // Recipe outputs craft; catalogue and ingredient cells navigate.
                    // Keep [R], [U] and right-click navigation on the output too.
                    let mut output_nav = None;
                    let response = item_cell(ui, item, Some(n), 30.0, edge, &mut output_nav, survival);
                    if let Some(grid) = e.grid {
                        let recipe = &all_recipes()[grid];
                        let reachable = bench || !recipe.needs_bench();
                        if response.clicked() {
                            if reachable {
                                let _ = survival.craft_selected_recipe(grid, ui.input(|i| i.modifiers.shift), bench);
                            }
                        } else if output_nav.is_some() {
                            *nav = output_nav;
                        }
                        response.on_hover_text(if reachable {
                            "[LMB] Craft · [Shift]+[LMB] Craft Max · [RMB] Uses"
                        } else { "Workbench Required" });
                    } else if output_nav.is_some() {
                        *nav = output_nav;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(3.0, 2.0);
                if let Some(grid) = e.grid {
                    let recipe = &all_recipes()[grid];
                    let available = recipe.ingredients.iter()
                        .map(|&(item, n)| survival.count_item(item) / n.max(1)).min().unwrap_or(0);
                    let reachable = bench || !recipe.needs_bench();
                    if ui.add_enabled(reachable && available > 0, egui::Button::new("Craft Max"))
                        .on_hover_text(format!("Output → Inventory · Limit: {} Batches", crate::world::inventory::MAX_SELECTED_BATCHES)).clicked() {
                        let _ = survival.craft_selected_recipe(grid, true, bench);
                    }
                    chip(ui, &format!("Available: {}", available.saturating_mul(recipe.output.count)),
                        if available > 0 { JADE } else { MUTED });
                    if !reachable { chip(ui, "Workbench Required", DANGER); }
                }
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
                        &power::fmt_power(
                            e.yields as f64 / (e.seconds as f64 / power::TICK_SECONDS),
                        ),
                        MANA,
                    );
                }
                if matches!(e.station, Station::Multiblock(_)) {
                    chip(ui, "Multiblock", AMBER);
                }
            });
            if !e.note.is_empty() {
                ui.label(RichText::new(e.note).size(13.0).color(MUTED));
            }
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
                    .size(13.0)
                    .color(PAPER),
            );
            if node.from_stock > 0.0 {
                chip(ui, &format!("covered {}", amount_text(node.from_stock)), JADE);
            }
            match node.recipe {
                Some(r) => {
                    chip(
                        ui,
                        &format!("{} ×{}", r.station.name(), node.batches),
                        MUTED,
                    );
                    if node.options > 1
                        && ui
                            .small_button("⟳")
                            .on_hover_text(format!(
                                "Next Recipe · {} Options",
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
pub(super) fn draw(ui: &mut egui::Ui, survival: &mut SurvivalInventory, bench: bool, size: i32) -> bool {
    draw_mode(ui, survival, bench, true, size)
}

fn draw_mode(
    ui: &mut egui::Ui,
    survival: &mut SurvivalInventory,
    bench: bool,
    catalogue: bool,
    size: i32,
) -> bool {
    if catalogue {
        let mut open = guide_open(ui.ctx());
        ui.horizontal(|ui| {
            ui.selectable_value(&mut open, false, "Recipes");
            ui.selectable_value(&mut open, true, "Field Guide");
        });
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(GUIDE_OPEN), open));
        if open {
            super::magic_guide::draw(ui, size);
            return false;
        }
    }
    let ctx = ui.ctx().clone();
    let get = |id: &str| egui::Id::new(id);
    let selected = ctx
        .data_mut(|d| d.get_temp::<Option<Item>>(get(SELECTED)))
        .flatten();
    // Portal notes only belong on the pages of items a portal frame uses.
    {
        use crate::world::BlockId as B;
        let portal_item = matches!(selected, Some(Item::Block(B::Basalt | B::Crystal | B::Bookcase)));
        let portal_query = ctx.data(|d| d.get_temp::<String>(get(QUERY))).is_some_and(|q| q.to_lowercase().contains("portal"));
        if portal_item || portal_query {
            portal_guide(ui);
        }
    }
    let tab = ctx.data_mut(|d| d.get_temp::<u8>(get(TAB)).unwrap_or(0));
    let goal_view = ctx.data(|d| d.get_temp::<bool>(get(GOAL_VIEW)).unwrap_or(false));
    let mut query = ctx.data_mut(|d| d.get_temp::<String>(get(QUERY)).unwrap_or_default());
    let mut amount = ctx.data_mut(|d| d.get_temp::<u32>(get(AMOUNT)).unwrap_or(1));
    let mut choices = ctx.data_mut(|d| d.get_temp::<Choices>(get(CHOICES)).unwrap_or_default());
    let mut history_direction = None;
    let mut next_tab = None;
    let mut nav: Option<Nav> = None;
    let mut staged = false;

    let width = ui.available_width();
    let side_by_side = catalogue && width >= 520.0;
    let list_w = if side_by_side { (width * 0.42).clamp(204.0, 444.0) } else { width };
    // Lists take what the window has left, leaving room for the tab strip
    // (two rows at the default scale) and the footer.
    let height = ui
        .available_height()
        .min(ui.ctx().content_rect().height() - 250.0)
        .max(180.0);
    ui.with_layout(if side_by_side { egui::Layout::left_to_right(egui::Align::Min) }
        else { egui::Layout::top_down(egui::Align::Min) }, |ui| {
        // Left: the selected item, at a fixed width beside the item list.
        // Two item gaps and the separator's own width sit between the columns.
        let gap = ui.spacing().item_spacing.x * 2.0 + 8.0;
        let detail_w = if side_by_side { width - list_w - gap } else { width };
        ui.vertical(|ui| {
            ui.set_width(detail_w.max(160.0));
            let Some(item) = selected else {
                ui.add_space(20.0);
                ui.label(RichText::new("Recipes").color(MUTED));
                return;
            };
            ui.horizontal(|ui| {
                history_direction = history::toolbar(ui);
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
                                amount_text(if goal_view { pins::held_target(survival, item) } else { stock_of(survival, item) })
                            ))
                            .size(13.0)
                            .color(JADE),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                });
            });
            let made = index::producing_at(item, size);
            let used = index::using_at(item, size);
            ui.horizontal_wrapped(|ui| {
                for (k, label) in [
                    (0u8, "Sources".to_string()),
                    (1, format!("Uses {}", used.len())),
                    (2, "Recipe Tree".to_string()),
                ] {
                    if ui.selectable_label(tab == k, label).clicked() {
                        next_tab = Some(k);
                    }
                }
            });
            if item == Item::Block(crate::world::BlockId::SmelteryController) {
                smeltery_structure(ui);
            }
            egui::ScrollArea::vertical()
                .id_salt(("jei_detail", tab))
                .max_height(height - 40.0)
                .auto_shrink([false, false])
                .show(ui, |ui| match tab {
                    0 | 1 => {
                        let list = if tab == 0 { &made } else { &used };
                        let shows_harvest = tab == 0 && harvest_sources(ui, item, size, &mut nav, survival);
                        if list.is_empty() && !shows_harvest {
                            ui.label(
                                RichText::new(if tab == 0 { "No Indexed Sources" } else { "No Indexed Uses" })
                                    .color(MUTED),
                            );
                        }
                        for e in list.iter() {
                            staged |= recipe_card(ui, e, item, survival, bench, &mut nav);
                        }
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(if goal_view { "Target" } else { "Make" }).size(13.0).color(MUTED));
                            for n in [1u32, 4, 16, 64] {
                                if ui.selectable_label(amount == n, format!("×{n}")).clicked() {
                                    amount = n;
                                }
                            }
                            if !choices.is_empty()
                                && ui
                                    .small_button("Reset")
                                    .on_hover_text("Back to the default recipes")
                                    .clicked()
                            {
                                choices.clear();
                            }
                        });
                        pins::button(ui, item, amount, &choices);
                        let plan = if goal_view {
                            pins::goal_plan(&ctx, survival, bench, size, item, amount, &choices)
                        } else {
                            tree_plan(&ctx, survival, bench, item, amount, &choices, size)
                        };
                        let facilities = technology::facilities(&plan);
                        if !facilities.is_empty() {
                            ui.label(RichText::new("Stations").size(13.0).color(MUTED));
                            ui.horizontal_wrapped(|ui| {
                                for station in facilities {
                                    let held = survival.count_item(station) > 0
                                        || (bench && station == Station::Bench.icon());
                                    item_cell(
                                        ui,
                                        station,
                                        None,
                                        28.0,
                                        Some(if held { JADE } else { MUTED }),
                                        &mut nav,
                                        survival,
                                    );
                                }
                            });
                        }
                        let (seconds, energy) = plan.totals();
                        ui.horizontal_wrapped(|ui| {
                            chip(ui, &format!("{seconds:.0}s"), PAPER);
                            if energy > 0.0 {
                                chip(ui, &power::fmt_energy(energy as f64), AMBER);
                            }
                        });
                        let missing = if goal_view { pins::missing(&ctx, &plan) } else { Arc::new(plan.missing()) };
                        if missing.is_empty() {
                            ui.label(RichText::new(if goal_view && plan.from_stock + 1e-4 >= plan.need {
                                "Complete"
                            } else { "Ingredients Ready" }).size(13.0).color(JADE));
                        } else {
                            ui.label(RichText::new("Missing Materials").size(13.0).color(DANGER));
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                                for (i, n) in missing.iter() {
                                    item_cell(
                                        ui,
                                        *i,
                                        Some(*n),
                                        28.0,
                                        Some(DANGER),
                                        &mut nav,
                                        survival,
                                    );
                                }
                            });
                        }
                        ui.separator();
                        tree_row(ui, &plan, "jei_tree", &mut choices, &mut nav, survival);
                    }
                });
        });
        // Right: item list, paged as in JEI. Search sits beneath it.
        if catalogue {
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(list_w);
                search_row(ui, &mut query, list_w);
                let tabs_top = ui.cursor().top();
                group_tabs(ui, list_w);
                let tabs_h = ui.cursor().top() - tabs_top;
                let items = filtered_items(&ctx, &query, survival, bench, false);
                paged_items(ui, "jei", &items, list_w, height - 24.0 - tabs_h, selected, true, &mut nav, survival, |ui| {
                    ui.label(RichText::new("Items").size(13.0).color(AMBER));
                    super::inventory::search::summary(ui, survival);
                });
            });
        }
    });

    // Commit edits before navigation so Back restores the exact plan just left.
    if let Some(item) = selected {
        history::Page { item, tab, goal: goal_view, amount, choices }.store(&ctx);
    }
    ctx.data_mut(|d| d.insert_temp(get(QUERY), query));
    if let Some(direction) = history_direction {
        history::go(&ctx, direction);
    } else if let Some(nav) = nav {
        match nav {
            Nav::Recipes(item) => history::browse(&ctx, item, 0),
            Nav::Uses(item) => history::browse(&ctx, item, 1),
        }
    } else if let Some(tab) = next_tab
        && let Some(mut page) = history::Page::current(&ctx)
    {
        page.tab = tab;
        history::visit(&ctx, page);
    }
    let _ = presentation::INK;
    staged
}
