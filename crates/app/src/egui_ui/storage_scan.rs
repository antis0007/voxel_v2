//! Local storage terminal. View sorting never mutates physical inventories.
use super::presentation as style;
use crate::world::scanner::{self, commands::Action};
use crate::world::inventory::{self, Item};
use crate::world::World;
use glam::{IVec3, Vec3};

const SLOT_PX: f32 = 26.0;
const COLUMNS: usize = 12;

fn compact(n: u64) -> String {
    match n {
        0..=9_999 => n.to_string(),
        10_000..=999_999 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

#[derive(Clone, Default)]
struct Panel {
    view: scanner::view::View,
    query: String,
    category: String,
    tab: u8,
    selected: Option<Item>,
    source: Option<IVec3>,
    quantity: u32,
    sort_count: bool,
    feedback: String,
    recipe: Option<usize>,
    batches: u32,
    craftable_only: bool,
}

pub fn panel(ui: &mut egui::Ui, world: &mut World, anchor: IVec3, eye_m: Vec3, network: bool) {
    let id = egui::Id::new(("storage_terminal", anchor.to_array()));
    let mut state: Panel = ui.ctx().data_mut(|d| d.remove_temp(id)).unwrap_or_else(|| {
        let mut panel = Panel::default();
        if std::env::var_os("VOXEL_REVIEW_SCANNER_CRAFT").is_some() {
            panel.tab = 2; panel.query = "Seared Brick".into(); panel.batches = 3;
        }
        panel
    });
    state.view.refresh(world, anchor, ui.input(|i| i.time));
    let index = state.view.index.clone();
    if state.selected.is_none() { state.selected = index.stock.first().map(|s| s.item); }
    let mut action = None;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.tab, 0, "Items");
        ui.selectable_value(&mut state.tab, 1, "Containers");
        ui.selectable_value(&mut state.tab, 2, "Craft");
        ui.small(format!("{} nearby", index.containers.len())).on_hover_text(format!("16 m · {} free slots", index.free_slots));
    });
    if !world.inventory.overflow.is_empty() && ui.button(format!("Recover · {} stacks", world.inventory.overflow.len())).clicked() {
        action = Some(Action::Recover);
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut state.query).hint_text("Search").desired_width(182.0));
        ui.toggle_value(&mut state.sort_count, "#").on_hover_text("Sort by quantity");
        egui::ComboBox::from_id_salt("scanner_category").width(92.0)
            .selected_text(if state.category.is_empty() { "All" } else { &state.category })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.category, String::new(), "All categories");
                let categories: std::collections::BTreeSet<_> = if state.tab == 2 {
                    inventory::all_recipes().iter().map(|r| r.output.item.category()).collect()
                } else { index.stock.iter().map(|s| s.item.category()).collect() };
                for category in categories { ui.selectable_value(&mut state.category, category.to_owned(), category); }
            });
    });
    if state.tab == 0 {
        let mut shown: Vec<_> = index.search(&state.query).filter(|s| state.category.is_empty() || s.item.category() == state.category).collect();
        if state.sort_count { shown.sort_by(|a,b| b.total.cmp(&a.total).then(inventory::sorting::sort_key(a.item).cmp(&inventory::sorting::sort_key(b.item)))); }
        egui::ScrollArea::vertical().id_salt("scanner_items").max_height(82.0).min_scrolled_height(SLOT_PX).auto_shrink([false, true]).show(ui, |ui| {
            super::slot_grid::SlotGrid::new(COLUMNS, SLOT_PX).show(ui, "scanner_grid", shown.len(), |ui, i, rect, response| {
                let stock = shown[i];
                let response = cell(ui, rect, response, stock, state.selected == Some(stock.item)).on_hover_text(format!("{} · {}
Available: {} · Reserved: {}
[Shift+LMB] Stack · [RMB] 1", stock.item.name(), stock.total, stock.available, stock.total - stock.available));
                if response.clicked() { state.selected = Some(stock.item); state.source = None; }
                if response.double_clicked() || (response.clicked() && ui.input(|i| i.modifiers.shift)) { action = Some(Action::Withdraw(stock.item, world.inventory.stack_limit(stock.item), None)); }
                if response.secondary_clicked() { action = Some(Action::Withdraw(stock.item, 1, None)); }
            });
            if shown.is_empty() { ui.label("No matching items"); }
        });
        if let Some(item) = state.selected {
            let stock = index.stock.iter().find(|s| s.item == item);
            let available = stock.map_or(0, |s| match state.source {
                None => s.available,
                Some(pos) => s.sources.iter().find(|(i,_)| index.containers[*i].pos == pos)
                    .map_or(0, |&(_,n)| u64::from(n.saturating_sub(scanner::policy::get(world, pos).reserve))),
            }).min(u64::from(u32::MAX)) as u32;
            let fit = available.min(world.inventory.remaining_capacity_for(item));
            ui.horizontal(|ui| {
                ui.label(item.name());
                ui.small(format!("{} available", compact(u64::from(available))));
            });
            ui.horizontal(|ui| {
                for (label, n) in [("1", 1), ("Half", available.min(world.inventory.stack_limit(item)).div_ceil(2)), ("Stack", world.inventory.stack_limit(item)), ("Max", u32::MAX)] {
                    if ui.add_enabled(fit > 0, egui::Button::new(label)).clicked() { action = Some(Action::Withdraw(item, n, state.source)); }
                }
                ui.add(egui::DragValue::new(&mut state.quantity).range(1..=u32::MAX));
                if ui.add_enabled(fit > 0, egui::Button::new("Take")).clicked() { action = Some(Action::Withdraw(item, state.quantity.max(1), state.source)); }
            if let Some(stock) = stock {
                ui.menu_button("Sources", |ui| {
                    ui.selectable_value(&mut state.source, None, "All locations");
                    for &(i, count) in &stock.sources {
                        let c = &index.containers[i];
                        if ui.selectable_value(&mut state.source, Some(c.pos), format!("{count} · {:.0} m", c.distance_m)).clicked() { locate(ui.ctx(), c.pos); }
                    }
                });
            }
            });
        }
    } else if state.tab == 1 {
        let centre = (anchor.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
        let mut containers: Vec<_> = index.containers.iter().map(|c| c.pos).collect();
        for &(x,y,z) in world.block_data.storage_policies.keys() {
            let p = IVec3::new(x,y,z);
            if !containers.contains(&p) && ((p.as_vec3()+Vec3::splat(0.5))*world.voxel_scale.meters()).distance(centre) <= scanner::RADIUS_M {
                containers.push(p);
            }
        }
        egui::ScrollArea::vertical().id_salt("scanner_containers").max_height(136.0).show(ui, |ui| {
            for pos in containers {
                let mut p = scanner::policy::get(world, pos);
                let old = p.clone();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut p.enabled, "");
                    if ui.selectable_value(&mut state.source, Some(pos), format!("Chest · {}, {}, {}", pos.x,pos.y,pos.z)).clicked() { locate(ui.ctx(), pos); }
                    ui.label("Priority"); ui.add(egui::DragValue::new(&mut p.priority));
                });
                if state.source == Some(pos) {
                    ui.horizontal(|ui| {
                        ui.label(p.assigned.map_or("Any item", Item::name));
                        if ui.add_enabled(state.selected.is_some(), egui::Button::new("Assign selected")).clicked() { p.assigned = state.selected; }
                        if ui.button("Clear").clicked() { p.assigned = None; p.category = None; }
                    });
                    ui.horizontal_wrapped(|ui| {
                        use inventory::stacks::StackClass;
                        ui.selectable_value(&mut p.category, None, "Any category");
                        for (label, class) in [("Materials", StackClass::Material), ("Small parts", StackClass::Small), ("Components", StackClass::Component), ("Food", StackClass::Food)] {
                            ui.selectable_value(&mut p.category, Some(class), label);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Reserve"); ui.add(egui::DragValue::new(&mut p.reserve));
                        ui.label("Target"); ui.add(egui::DragValue::new(&mut p.target));
                    });
                    if let Some(c) = index.containers.iter().find(|c| c.pos == pos) {
                        let subset = scanner::index(world, &[*c]);
                        super::slot_grid::SlotGrid::new(COLUMNS, SLOT_PX).show(ui, "scanner_subset", subset.stock.len(), |ui, i, rect, response| {
                            let stock = &subset.stock[i];
                            if cell(ui, rect, response, stock, false).on_hover_text(stock.item.name()).clicked() { state.selected=Some(stock.item); state.tab=0; }
                        });
                    }
                }
                if p != old { action = Some(Action::Configure(pos, p)); }
            }
        });
    } else {
        let supplies = scanner::crafting::Supplies::new(&world.inventory, &index);
        let bench = scanner::crafting::at_bench(world, eye_m);
        let query = state.query.to_lowercase();
        ui.checkbox(&mut state.craftable_only, "Craftable");
        let recipes: Vec<_> = inventory::all_recipes().iter().enumerate().filter(|(_,r)| {
            query.split_whitespace().all(|term| r.name.to_lowercase().contains(term))
                && (state.category.is_empty() || r.output.item.category() == state.category)
                && (!state.craftable_only || supplies.batches(r, bench) > 0)
        }).collect();
        if !recipes.iter().any(|(id,_)| Some(*id) == state.recipe) {
            state.recipe = recipes.first().map(|(id,_)| *id);
        }
        egui::ScrollArea::vertical().id_salt("scanner_craft").max_height(54.0).show(ui, |ui| {
            if recipes.is_empty() { ui.label("No matching recipes"); }
            for (id, recipe) in recipes {
                ui.horizontal(|ui| {
                    super::block_page::slot_sized(ui, Some(recipe.output), SLOT_PX);
                    ui.selectable_value(&mut state.recipe, Some(id), recipe.name);
                    ui.label(format!("{}×", supplies.batches(recipe, bench))).on_hover_text("Batches from available ingredients");
                });
            }
        });
        if let Some((id, recipe)) = state.recipe.and_then(|id| inventory::all_recipes().get(id).map(|r| (id,r))) {
            state.batches = state.batches.clamp(1, scanner::crafting::MAX_BATCHES);
            let available = supplies.batches(recipe, bench);
            ui.label(format!("{} × {}", recipe.name, u64::from(recipe.output.count) * u64::from(state.batches)));
            ui.horizontal_wrapped(|ui| {
                for &(item, count) in recipe.ingredients {
                    let required = u64::from(count) * u64::from(state.batches);
                    let held = supplies.count(item);
                    super::block_page::slot_sized(ui, Some(inventory::ItemStack { item, count: count.saturating_mul(state.batches) }), SLOT_PX);
                    let text = format!("{held}/{required}");
                    if held < required { ui.colored_label(egui::Color32::from_rgb(235,130,110), text); }
                    else { ui.label(text); }
                }
            });
            if recipe.needs_bench() && !bench { ui.label("Workbench required"); }
            ui.horizontal(|ui| {
                ui.label("Batches");
                ui.add(egui::DragValue::new(&mut state.batches).range(1..=scanner::crafting::MAX_BATCHES));
                if ui.add_enabled(available > 0, egui::Button::new("Max")).clicked() { state.batches = available; }
                if ui.add_enabled(available >= state.batches, egui::Button::new("Craft")).clicked() {
                    action = Some(Action::CraftBatch(id, state.batches));
                }
            });
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Deposit").on_hover_text("Smart deposit").clicked() { action = Some(Action::SmartDeposit); }
        if ui.button("Restock").clicked() { action = Some(Action::Restock); }
        if ui.button("Consolidate").on_hover_text(format!("{} slots freed", state.view.freed)).clicked() { action = Some(Action::Consolidate); }
        ui.menu_button("More", |ui| {
            for (label, cmd) in [("Quick deposit", Action::QuickDeposit), ("Deposit all", Action::DepositAll)] {
                if ui.button(label).clicked() { action = Some(cmd); ui.close(); }
            }
            ui.separator();
        if !network {
            ui.horizontal(|ui| {
                ui.label("Stacks");
                let mut balance = world.rules.stacks;
                for value in inventory::stacks::Balance::ALL { ui.selectable_value(&mut balance, value, value.name()); }
                if balance != world.rules.stacks {
                    world.rules.stacks = balance;
                    world.inventory.migrate_balance(balance);
                    world.sync_from_inventory();
                    state.view.invalidate();
                }
            });
        }
        if let Some(item) = state.selected {
            ui.horizontal(|ui| {
                ui.label("Keep carried");
                let mut n = world.inventory.minimum(item);
                if ui.add(egui::DragValue::new(&mut n)).changed() { action = Some(Action::Minimum(item, n)); }
            });
        }
        });
    });
    if let Some(action) = action {
        if network {
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("scanner_request"), (anchor, action)));
            state.feedback = "Requested".into();
        } else {
            let organize = matches!(action, Action::Consolidate);
            let craft = matches!(action, Action::Craft(_) | Action::CraftBatch(..));
            let configure = matches!(action, Action::Configure(..) | Action::Minimum(..));
            state.feedback = match scanner::commands::apply(world, anchor, eye_m, action) {
                Ok(_) if configure => "Saved".into(),
                Ok(n) if organize => format!("{n} slots freed"),
                Ok(n) if craft => format!("{n} crafted"),
                Ok(0) => "No items moved".into(),
                Ok(n) => format!("{n} moved"),
                Err(e) => e.to_string(),
            };
        }
        state.view.invalidate();
    }
    if !state.feedback.is_empty() { ui.label(&state.feedback); }
    ui.ctx().data_mut(|d| d.insert_temp(id, state));
}

fn locate(ctx: &egui::Context, pos: IVec3) {
    let until = ctx.input(|i| i.time) + 12.0;
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("scanner_location"), (pos, until)));
}

pub fn highlight(ctx: &egui::Context, world: &World, view_proj: glam::Mat4) {
    let Some((pos, until)) = ctx.data_mut(|d| d.get_temp::<(IVec3,f64)>(egui::Id::new("scanner_location"))) else { return };
    if ctx.input(|i| i.time) > until || world.get(pos).chest_tier().is_none() { return; }
    let c = view_proj * (pos.as_vec3() + Vec3::splat(0.5)).extend(1.0);
    if c.w <= 0.0 { return; }
    let screen = ctx.content_rect();
    let at = egui::pos2(screen.left()+(c.x/c.w*0.5+0.5)*screen.width(),screen.top()+(0.5-c.y/c.w*0.5)*screen.height());
    let painter=ctx.layer_painter(egui::LayerId::background());
    painter.circle_stroke(at, 18.0, egui::Stroke::new(2.0, style::PAPER));
    painter.text(at+egui::vec2(0.0,22.0),egui::Align2::CENTER_TOP,"Chest",egui::FontId::proportional(14.0),style::PAPER);
}

fn cell(ui: &egui::Ui, rect: egui::Rect, response: egui::Response, stock: &scanner::Stock, selected: bool) -> egui::Response {
    super::icons::frame(ui.painter(), rect, response.hovered());
    super::inventory::draw_item_icon(ui.painter(), rect.shrink(3.0), stock.item);
    if selected { ui.painter().rect_stroke(rect, 0, egui::Stroke::new(1.0, style::AMBER), egui::StrokeKind::Inside); }
    ui.painter().text(rect.right_bottom()-egui::vec2(2.0,1.0),egui::Align2::RIGHT_BOTTOM,compact(stock.total),egui::FontId::proportional(9.0),style::PAPER);
    response
}
