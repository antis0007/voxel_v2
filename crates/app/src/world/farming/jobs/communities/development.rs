//! A bounded council turns observed shortages into ordinary paid work.
//! Surveys, recipes and construction never credit goods or completed labor.
use super::*;
use super::super::{B, Order, Task};
use crate::world::{block_data, farming, inventory::{all_recipes, CraftingRecipe}};
use std::{collections::HashMap, sync::LazyLock};

pub(super) const INTERVAL: u64 = 600;
pub(super) fn provisions(residents: usize) -> u32 {
    residents as u32 * 6 + if residents >= 4 { 12 } else { 0 }
}
const MAX_PROSPECTS: usize = 64;
const MAX_PROJECT_CELLS: usize = 1024;

// Policy may inspect admitted terrain only. The general farming lookup can
// sample an unedited chunk synchronously; councils must never invoke that path.
fn known(w: &World, at: IVec3) -> Option<B> {
    w.resident_block(at)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Deposit { at: [i32; 3], block: B, item: Item }

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Project {
    cells: Vec<Order>,
    home: Home,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct State {
    knowledge: u8,
    survey: u32,
    prospects: Vec<Deposit>,
    project: Option<Project>,
    pub(super) vacancies: Vec<Home>,
    pub(super) completed_homes: u32,
    pub(super) last_expansion: u64,
    pub(super) shortages: u16,
    plans: u64,
    #[serde(default)] pub(super) scout_routes: u32,
    #[serde(default)] coordinator: Option<u32>,
    #[serde(default)] pub(super) needs: Vec<ItemStack>,
    #[serde(default)] housing_cursor: u32,
}

impl State {
    pub(super) fn valid(&self) -> bool {
        self.knowledge <= 3 && self.prospects.len() <= MAX_PROSPECTS
            && self.coordinator.is_none_or(engine_worldgen::civilization::resident_id)
            && self.needs.len() <= 64 && self.needs.iter().all(|s| (1..=4096).contains(&s.count))
            && self.prospects.iter().all(|p| p.at.iter().all(|v| v.unsigned_abs() < 1 << 28)
                && super::super::mineable(p.block) && block_data::blast_drop(p.block) == Some(p.item))
            && self.vacancies.len() <= 4 && self.vacancies.iter().all(Home::valid)
            && self.project.as_ref().is_none_or(|p| !p.cells.is_empty()
                && p.cells.len() <= MAX_PROJECT_CELLS && p.home.valid()
                && p.cells.iter().all(|o| o.material().is_some() && o.at.iter().all(|v| v.unsigned_abs() < 1 << 28)))
    }

    pub(super) fn report(&self) -> serde_json::Value {
        serde_json::json!({"knowledge": self.knowledge, "surveys": self.survey,
            "resource_sites": self.prospects.len(), "plans": self.plans,
            "project_cells": self.project.as_ref().map_or(0, |p| p.cells.len()),
            "vacant_homes": self.vacancies.len(), "homes_built": self.completed_homes,
            "coordinator": self.coordinator, "scouting_journeys": self.scout_routes,
            "stock_targets": self.needs,
            "shortage_ticks": u64::from(self.shortages) * INTERVAL})
    }

    pub(super) fn reserve(&self, item: Item) -> u32 {
        self.needs.iter().find(|s| s.item == item).map_or(0,|s|s.count.div_ceil(2))
    }
}

static RECIPES: LazyLock<HashMap<Item, Vec<&'static CraftingRecipe>>> = LazyLock::new(|| {
    let mut out: HashMap<Item, Vec<&'static CraftingRecipe>> = HashMap::new();
    for recipe in all_recipes() {
        if recipe.output.count > 0 && !recipe.ingredients.is_empty() {
            out.entry(recipe.output.item).or_default().push(recipe);
        }
    }
    out
});

pub(super) fn discipline(task: Option<Task>) -> Option<usize> {
    match task? {
        Task::Mine(..) | Task::Produce(production::Task::Fell(_)) => Some(0),
        Task::Produce(production::Task::Harvest(_) | production::Task::Sow(_)) => Some(1),
        Task::Build(_) => Some(2),
        Task::Produce(production::Task::Craft(..) | production::Task::Smelt(..)) => Some(3),
        _ => None,
    }
}

fn push_demand(demand: &mut Vec<(Item, u32)>, item: Item, count: u32) {
    if let Some((_, n)) = demand.iter_mut().find(|(i, _)| *i == item) {
        *n = (*n).max(count.min(4096));
    } else if demand.len() < 64 {
        demand.push((item, count.min(4096)));
    }
}

/// Shared recipe data; a per-plan recursion guard rejects cycles and limits work.
fn supply_chain(w: &World, j: &mut Jobs, station: [i32; 3], worker: u32,
    demand: &mut Vec<(Item, u32)>, item: Item, amount: u32, visiting: &mut Vec<Item>, budget: &mut usize)
{
    push_demand(demand, item, amount);
    if *budget == 0 || visiting.len() >= 8 || visiting.contains(&item) { return; }
    *budget -= 1;
    let Some(pile) = j.stockpile else { return; };
    let slots = w.block_data.chest(pile.into());
    let held = production::count(&slots, item) + crew::cargo_item(j, item);
    if held >= amount { return; }
    let Some(options) = RECIPES.get(&item) else { return; };
    // Favor an available recipe, then fewer missing ingredients, then authored order.
    let Some(recipe) = options.iter().min_by_key(|r| r.ingredients.iter()
        .map(|&(i, n)| n.saturating_sub(production::count(&slots, i))).sum::<u32>()).copied() else { return; };
    let Some(p) = &mut j.production else { return; };
    if let Some(bill) = p.bills.iter_mut().find(|b| b.recipe == recipe.name) {
        bill.target = bill.target.max(amount.min(4096));
    } else {
        if p.bills.len() >= 64 { return; }
        p.bills.push(production::Bill { recipe: recipe.name.into(), target: amount.min(4096),
            station, worker, once: false, per_resident: 0 });
    }
    let batches = amount.saturating_sub(held).div_ceil(recipe.output.count);
    visiting.push(item);
    for &(input, count) in recipe.ingredients {
        supply_chain(w, j, station, worker, demand, input, count.saturating_mul(batches).min(4096), visiting,budget);
    }
    visiting.pop();
}

fn exposed(w: &World, at: IVec3) -> bool {
    [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].into_iter()
        .any(|d| known(w, at + d) == Some(B::Air)
            && known(w, at + d + IVec3::Y) == Some(B::Air))
}

fn survey(w: &mut World, city: &mut Community, pile: IVec3) {
    let workers = crew::ids(&w.farming.jobs).count();
    if workers == 0 { return; }
    let observer = crew::ids(&w.farming.jobs).nth((city.development.survey / 4) as usize % workers)
        .and_then(|id|crew::view(&w.farming.jobs,id)).and_then(|p|p.pos);
    let Some(observer) = observer else { return; };
    observe(w,city,pile,Vec3::from(observer));
}

pub(super) fn observe(w: &mut World, city: &mut Community, pile: IVec3, feet: Vec3) -> bool {
    let v = w.voxel_scale.meters();
    let observer = (feet / v).floor().as_ivec3();
    // Admit one small column at a time. A wide all-or-nothing request can exceed
    // the shared source pool and indefinitely evict its own earlier results.
    let mut cells = Vec::with_capacity(40);
    for _ in 0..4 {
        let n = city.development.survey;
        // Knowledge comes from the retained positions of actual residents.
        // A scout opens new ground by walking there, never by a global scan.
        let x = (n % 5) as f32 * 2.0 - 4.0;
        let z = ((n / 5) % 5) as f32 * 2.0 - 4.0;
        let base = observer + IVec3::new((x / v).round() as i32, 0, (z / v).round() as i32);
        let mut bounds = Vec::with_capacity(8);
        for dx in [-1, 1] { for dz in [-1, 1] { for y in [-5, 7] {
            bounds.push(base + IVec3::new(dx, y, dz));
        }}}
        if !super::super::residency::prepare_cells(w, bounds) { break; }
        city.development.survey = city.development.survey.wrapping_add(1);
        for y in -4..6 { cells.push(base + IVec3::Y * y); }
    }
    city.development.prospects.retain(|p| known(w, p.at.into()).is_none_or(|b| b == p.block));
    let observed = !cells.is_empty();
    for at in cells {
        if farming::base::overlaps(w, at, at) { continue; }
        let Some(block) = known(w, at) else { continue; };
        let Some(p) = w.farming.jobs.production.as_ref() else { continue; };
        if p.beds.len() < 64 && !p.beds.iter().any(|b| b.at == at.to_array()) {
            // Only an actually present plant/tree is adopted. Planting still spends seed.
            let root = block == B::Log
                && known(w, at - IVec3::Y).is_some_and(|b| matches!(b, B::Grass | B::Dirt | B::FungalSoil))
                && known(w, at + IVec3::Y) == Some(B::Log);
            let crop = if root { B::Sapling } else { block };
            let bed = production::Bed { at: at.to_array(), crop };
            if let Some((index, row)) = bed.row()
                && (root || block == row.crop) && w.farming.plots.len() < farming::MAX_PLOTS
                && !w.farming.plots.contains_key(&(at.x, at.y, at.z))
            {
                w.farming.plots.insert((at.x, at.y, at.z),
                    farming::Plot { crop: index, planted: w.farming.clock, rolled: 0, stage: row.mature() });
                let p = w.farming.jobs.production.as_mut().unwrap();
                if root { production::enable_forestry(p); }
                p.beds.push(bed);
                continue;
            }
        }
        // Keep excavation away from the original houses, crops and town square.
        let delta = (at - pile).as_vec3() * v;
        if delta.x.abs() < 25.0 && delta.z.abs() < 20.0 { continue; }
        if !super::super::mineable(block) || block == B::Log || !exposed(w, at) { continue; }
        let Some(item) = block_data::blast_drop(block) else { continue; };
        if city.development.prospects.len() < MAX_PROSPECTS
            && !city.development.prospects.iter().any(|p| p.at == at.to_array()) {
            city.development.prospects.push(Deposit { at: at.to_array(), block, item });
        }
    }
    observed
}

fn house(w: &World, at: IVec3) -> Option<Project> {
    let v = w.voxel_scale.meters();
    let width = (3.0 / v).ceil().max(3.0) as i32;
    let height = (2.2 / v).ceil().max(3.0) as i32;
    let mut cells = Vec::new();
    let door = width / 2;
    let door_width = (1.2 / v).ceil().max(1.0) as i32;
    let door_start = (width - door_width) / 2;
    for y in 0..=height + width / 2 { for z in -1..=width { for x in -1..=width {
        let inside = (0..width).contains(&x) && (0..width).contains(&z);
        let edge = x == 0 || x == width - 1 || z == 0 || z == width - 1;
        let doorway = z == 0 && (door_start..door_start + door_width).contains(&x)
            && y < (2.0 / v).ceil() as i32;
        let roof = height + x.min(width - 1 - x);
        let gable = inside && (z == 0 || z == width - 1) && y >= height && y < roof;
        if ((inside && edge && y < height) || gable || y == roof) && !doorway {
            let cell = at + IVec3::new(x, y, z);
            if known(w, cell) != Some(B::Air) || farming::base::overlaps(w, cell, cell) { return None; }
            let frame = y < height && (x == 0 || x == width - 1) && (z == 0 || z == width - 1);
            cells.push(Order::new(cell, if frame { B::Log } else { B::Planks }));
            if cells.len() > MAX_PROJECT_CELLS { return None; }
        }
        if inside && y < height && !edge && known(w, at + IVec3::new(x, y, z)) != Some(B::Air) { return None; }
        if inside && y == 0 && !known(w, at + IVec3::new(x, -1, z)).is_some_and(B::is_solid) { return None; }
    }}}
    // Paid exterior steps provide actual access for the roof builders. Their
    // width clears the ordinary resident body at every supported voxel scale.
    let stair_width = (1.2 / v).ceil().max(1.0) as i32;
    for step in 0..height { for x in -stair_width-1..=-2 { for y in 0..=step {
        let cell = at + IVec3::new(x,y,width+height-step);
        if known(w,cell) != Some(B::Air) || farming::base::overlaps(w,cell,cell)
            || (y == 0 && !known(w,cell-IVec3::Y).is_some_and(B::is_solid)) { return None; }
        cells.push(Order::new(cell,B::Planks));
    }}}
    for y in 0..height {
        let cell = at + IVec3::new(-1,y,width+1);
        if known(w,cell) != Some(B::Air) || farming::base::overlaps(w,cell,cell)
            || (y == 0 && !known(w,cell-IVec3::Y).is_some_and(B::is_solid)) { return None; }
        cells.push(Order::new(cell,B::Planks));
    }
    if cells.len() > MAX_PROJECT_CELLS { return None; }
    let entry = at + IVec3::new(door, 0, 0);
    let destination = at + IVec3::new(door, 0, width / 2);
    Some(Project { cells, home: Home { entry: entry.to_array(), destination: destination.to_array() } })
}

fn construction(w: &mut World, city: &mut Community, pile: IVec3, bread: u32, residents: usize) {
    if let Some(project) = &city.development.project {
        if project.cells.iter().all(|o| known(w, o.cell()) == Some(o.block())) {
            if let Some((&id, person)) = city.people.iter_mut().find(|(_,p)|
                p.died_at.is_none() && p.home.entry == p.home.destination) {
                person.home = project.home.clone();
                w.farming.jobs.homes.insert(id,project.home.clone());
            } else if city.development.vacancies.len() < 4 { city.development.vacancies.push(project.home.clone()); }
            city.development.completed_homes = city.development.completed_homes.saturating_add(1);
            city.development.housing_cursor = city.development.housing_cursor.wrapping_add(1);
            city.development.project = None;
        } else {
            for &order in &project.cells {
                if w.farming.jobs.build.len() >= 128 { break; }
                if known(w, order.cell()) == Some(B::Air) && !w.farming.jobs.build.contains(&order) {
                    w.farming.jobs.build.push(order);
                }
            }
            return;
        }
    }
    if city.development.knowledge == 0 || city.development.vacancies.len() >= 2
        || residents == 0 || bread < residents as u32 * 4 { return; }
    let v = w.voxel_scale.meters();
    let regional = w.generator_version >= engine_voxel::REGIONAL_CIVILIZATION_GENERATOR;
    let half_width = if regional { 96.0 } else { 60.0 };
    let side = if regional { 17 } else { 11 };
    let n = city.development.housing_cursor % (side * side);
    let x = (n % side) as f32 * 12.0 - half_width;
    let z = (n / side) as f32 * 12.0 - half_width;
    // Leave two connected avenues open for the district as housing fills in.
    let street = regional && (x % 48.0 == 0.0 || z % 48.0 == 0.0);
    if street || x*x+z*z > half_width*half_width || (x.abs() < 24.0 && z.abs() < 24.0) {
        city.development.housing_cursor = city.development.housing_cursor.wrapping_add(1);
        return;
    }
    let at = pile + IVec3::new((x / v).round() as i32, 0, (z / v).round() as i32);
    let size = (8.0 / v).ceil() as i32;
    let mut corners = Vec::new();
    // Fill intervening source pages too: fine voxels can span more than two.
    let stride = crate::world::CHUNK_SIZE as usize;
    for x in (-(3.0/v).ceil() as i32..=size+crate::world::CHUNK_SIZE).step_by(stride) {
      for y in (-1..=size+crate::world::CHUNK_SIZE).step_by(stride) {
       for z in (-1..=size+crate::world::CHUNK_SIZE).step_by(stride) {
        corners.push(at + IVec3::new(x, y, z));
    }}}
    if !super::super::residency::prepare_cells(w, corners) { return; }
    city.development.project = house(w, at);
    if city.development.project.is_some() {
        w.farming.jobs.territory=w.farming.jobs.territory.max(x.hypot(z)+12.0);
    }
    if city.development.project.is_none() {
        city.development.housing_cursor = city.development.housing_cursor.wrapping_add(1);
    }
}

pub(super) fn plan(w: &mut World, _site: u32, city: &mut Community) {
    city.development.plans = city.development.plans.saturating_add(1);
    let Some(pile) = w.farming.jobs.stockpile.map(IVec3::from) else { return; };
    if known(w, pile).is_none_or(|b| b.chest_tier().is_none()) { return; }
    let residents = crew::ids(&w.farming.jobs).count();
    if residents == 0 { return; }
    survey(w,city,pile);
    city.development.coordinator = crew::ids(&w.farming.jobs).filter(|&id|
        crew::view(&w.farming.jobs,id).is_some_and(|p| !p.mission && p.pos.is_some_and(|at|
            Vec3::from(at).distance_squared(Task::Store(pile).point(w.voxel_scale.meters())) <= 9.0)))
        .max_by_key(|&id| (city.people.get(&id).map_or(0,|p|p.experience.iter()
            .copied().fold(0u32,u32::saturating_add)),id));
    let stock = w.block_data.chest(pile);
    let bread = production::count(&stock, MEAL);
    city.development.shortages = if bread < residents as u32 {
        city.development.shortages.saturating_add(1)
    } else { city.development.shortages.saturating_sub(2) };
    for (&id, p) in &mut city.people {
        if p.enlisted || p.died_at.is_some() { continue; }
        let hungry = w.farming.jobs.production.as_ref().is_some_and(|s|
            w.farming.clock >= s.next_meal.get(&id).copied().unwrap_or(0)) && bread == 0;
        p.grievance = if hungry { p.grievance.saturating_add(1) } else { p.grievance.saturating_sub(2) };
    }
    if city.development.coordinator.is_none() { return; }
    if let Some(p) = &w.farming.jobs.production {
        let made = |name: &str| p.made.get(name).copied().unwrap_or(0);
        if made("Oak planks") >= 16 { city.development.knowledge = city.development.knowledge.max(1); }
        if made("Stone pickaxe") > 0 { city.development.knowledge = city.development.knowledge.max(2); }
        if made("Iron pickaxe") > 0 { city.development.knowledge = 3; }
    }
    construction(w, city, pile, bread, residents);
    let artisan = w.farming.jobs.production.as_ref().into_iter().flat_map(|p| p.workers.iter().copied())
        .filter(|&id| crew::view(&w.farming.jobs, id).is_some_and(|p| !p.mission) && w.travel.following != Some(id)
            && !w.travel.guard_posts.contains_key(&id) && !w.travel.politics.mobilized(id))
        .max_by_key(|&id| {
        let experience = city.people.get(&id).map_or(0, |p| p.experience[3]);
        (experience, engine_core::hash::hash_u32(id ^ 0x517c_c1b7) % 8)
    });
    let Some(artisan) = artisan else { return; };
    // Keep a provision surplus attainable: a founding threshold above the cook's
    // stock target would permanently prevent every autonomous departure.
    let mut demand = vec![(MEAL, provisions(residents)), (TIMBER, 96),
        (Item::Block(B::Cobblestone), 48), (Item::Block(B::Chest), 2), (Item::Block(B::Workbench), 1)];
    if city.development.knowledge >= 1 {
        push_demand(&mut demand,Item::Block(B::Smelter),1);
        let hearth = pile - IVec3::Z;
        let order = Order::new(hearth,B::Smelter);
        if order.ready(w) && !w.farming.jobs.build.contains(&order) && w.farming.jobs.build.len() < 128 {
            w.farming.jobs.build.push(order);
        }
        for name in ["Stone pickaxe", "Stone axe", "Ladder", "Torch (Coal)"] {
            if let Some(recipe) = all_recipes().iter().find(|r| r.name == name) {
                push_demand(&mut demand, recipe.output.item, if name == "Ladder" { 12 } else { 2 });
            }
        }
    }
    if city.development.knowledge >= 2 {
        for name in ["Iron pickaxe", "Iron axe", "Copper sword", "Stone bricks"] {
            if let Some(recipe) = all_recipes().iter().find(|r| r.name == name) {
                push_demand(&mut demand, recipe.output.item, 2);
            }
        }
    }
    if let Some(project) = &city.development.project {
        for order in &project.cells {
            if known(w, order.cell()) != Some(order.block()) {
                if let Some(item) = order.material() { push_demand(&mut demand, item, 128); }
            }
        }
    }
    // Move the executor only for policy calculation; no actor or stock is cloned.
    let mut jobs = std::mem::take(&mut w.farming.jobs);
    let mut budget = 128;
    for (item, target) in demand.clone() {
        supply_chain(w, &mut jobs, city.station, artisan, &mut demand, item, target, &mut Vec::new(),&mut budget);
    }
    let volume = crate::world::expedition::harvest_units(w.voxel_scale.meters());
    for metal in 6u8..=8 {
        let needed = demand.iter().find(|(item,_)| *item == Item::Resource(metal)).map_or(0,|(_,n)|*n);
        if needed == 0 { continue; }
        let batches = needed.max(4);
        for deposit in &city.development.prospects {
            if crate::world::expedition::ore_resource(deposit.block) == Some(usize::from(metal-6)) {
                push_demand(&mut demand,deposit.item,(batches*64).div_ceil(volume));
            }
            if crate::world::expedition::ore_resource(deposit.block) == Some(3) {
                push_demand(&mut demand,deposit.item,(batches*crate::world::expedition::SMELT_COAL).div_ceil(volume));
            }
        }
    }
    city.development.needs = demand.iter().map(|&(item,target)|ItemStack::new(item,target)).collect();
    jobs.mine.retain(|&(at, block)| known(w, at.into()).is_none_or(|b| b as u16 == block));
    for deposit in &city.development.prospects {
        if jobs.mine.len() >= 24 { break; }
        if demand.iter().any(|&(item, n)| item == deposit.item
            && production::count(&stock, item) + crew::cargo_item(&jobs, item) < n)
            && !jobs.mine.iter().any(|(at, _)| *at == deposit.at)
            && !farming::base::overlaps(w, deposit.at.into(), deposit.at.into()) {
            jobs.mine.push((deposit.at, deposit.block as u16));
        }
    }
    w.farming.jobs = jobs;
}
