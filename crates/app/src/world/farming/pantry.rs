//! Fernwake's receiving pantry buys, cooks and serves finite provisions.
use super::{World, freight, known};
use crate::world::{
    BlockId,
    inventory::{FoodKind, Item, all_recipes},
    travel::{
        DAY_TICKS,
        trades::{Stock, Trade},
    },
};
use std::sync::LazyLock;

pub(super) const FOOD_CAPACITY: u32 = 16;
pub(super) const BIOMASS_CAPACITY: u32 = 24;
pub(super) const PURSE_CAPACITY: u32 = 64;
pub(super) const RESIDENTS: u8 = 5;
const HOUSEHOLDS: [&str; 4] = ["Mara", "Orrin", "Sedge", "Pip"];
const SECOND: u64 = (1.0 / engine_core::time::PLAYER_STEP_SECONDS + 0.5) as u64;
const COOK_INTERVAL: u64 = 30 * SECOND;

const CAP: Item = Item::Block(BlockId::GlowCap);
pub(crate) const BIOMASS: Item = Item::Resource(13);
pub(crate) const FOOD: Item = Item::Food(FoodKind::Trailcake);
pub(crate) const PAYMENT: Item = Item::Resource(8);

struct MealBatch {
    caps: u32,
    biomass: u32,
    meals: u32,
}

static RECIPE: LazyLock<Option<MealBatch>> = LazyLock::new(|| {
    let batch = all_recipes().iter().find_map(|recipe| {
        if recipe.output.item != FOOD || recipe.ingredients.len() != 2 {
            return None;
        }
        let count = |ingredient| {
            recipe
                .ingredients
                .iter()
                .find(|&&(item, _)| item == ingredient)
                .map(|&(_, amount)| amount)
        };
        Some(MealBatch {
            caps: count(CAP)?,
            biomass: count(BIOMASS)?,
            meals: recipe.output.count,
        })
    });
    if batch.is_none() {
        eprintln!("[Fernwake pantry] Missing Glowcap and biomass trailcake recipe");
    }
    batch
});

#[derive(Clone, Debug)]
pub(crate) struct State {
    pub(super) meals: u32,
    pub(super) biomass: u32,
    pub(super) purse: u32,
    pub(super) day: u64,
    pub(super) demand: u8,
    pub(super) served: u8,
}

pub(crate) fn active(world: &World) -> bool {
    let cfg = world.sample_config();
    freight::active(&cfg) && super::super::environment::State::enabled(cfg.generator_version)
}

fn staffed(world: &World) -> bool {
    let clerk = freight::SELLER;
    world.travel.away.get(&clerk) != Some(&(world.farming.clock / DAY_TICKS))
        && world.travel.following != Some(clerk)
        && !world.travel.homes.contains_key(&clerk)
        && !world.travel.guard_posts.contains_key(&clerk)
        && !super::jobs::assigned(world, clerk)
}

pub(crate) fn work_goal(world: &World, id: u32) -> Option<glam::Vec3> {
    if id != freight::SELLER || !active(world) || !staffed(world) {
        return None;
    }
    let site = engine_worldgen::fernwake::freight::CLERK - glam::Vec3::Z * 2.0;
    Some(engine_worldgen::fernwake::freight::point(&world.sample_config(), site))
}

fn kitchen_issue(world: &World) -> Option<&'static str> {
    if !staffed(world) {
        return Some("Receiving Clerk is off kitchen duty");
    }
    match known(world, freight::bench_cell(&world.sample_config())) {
        Some(BlockId::Planks) => None,
        Some(_) => Some("Kitchen counter needs planks"),
        None => Some("Kitchen counter is out of view"),
    }
}

pub(super) fn service(world: &mut World) {
    let now = world.farming.clock;
    if now % SECOND != 0 || !active(world) {
        return;
    }
    let opening = world.farming.pantry.is_none();
    if opening && !super::resident(world, freight::bench_cell(&world.sample_config())) {
        return;
    }
    let day = now / DAY_TICKS;
    let residents = HOUSEHOLDS.len() as u8;
    let cook = now % COOK_INTERVAL == 0 && kitchen_issue(world).is_none();
    let farming = &mut world.farming;
    // Set aside one opening consignment. Later shipments draw from the nursery harvest.
    if opening && farming.freight.market == 0 && farming.freight.cargo == 0 {
        farming.nursery_stock = farming.nursery_stock.max(freight::LOCAL_RESERVE + freight::LOAD);
    }
    let pantry = farming.pantry.get_or_insert(State {
        meals: 2,
        biomass: 12,
        purse: 16,
        day,
        demand: residents,
        served: 0,
    });
    if pantry.day != day {
        pantry.day = day;
        pantry.demand = residents;
        pantry.served = 0;
    }
    if cook
        && let Some(batch) = RECIPE.as_ref()
        && pantry.meals < u32::from(pantry.demand)
        && farming.freight.market >= batch.caps
        && pantry.biomass >= batch.biomass
        && batch.meals <= FOOD_CAPACITY - pantry.meals
    {
        farming.freight.market -= batch.caps;
        pantry.biomass -= batch.biomass;
        pantry.meals += batch.meals;
    }
    let eaten = pantry.meals.min(u32::from(pantry.demand - pantry.served));
    pantry.meals -= eaten;
    pantry.served += eaten as u8;
}

pub(crate) fn uses(world: &World, stock: Stock, trade: &Trade) -> u16 {
    let (Some(pantry), [input]) = (&world.farming.pantry, trade.give.as_slice()) else {
        return 0;
    };
    let input_count = input.count.max(1);
    let output_count = trade.get.count.max(1);
    let uses = match stock {
        Stock::PantrySale if pantry.served == pantry.demand => {
            let retail = pantry.meals.saturating_sub(u32::from(pantry.demand));
            (retail / output_count).min((PURSE_CAPACITY - pantry.purse) / input_count)
        }
        Stock::PantryBiomass => {
            ((BIOMASS_CAPACITY - pantry.biomass) / input_count).min(pantry.purse / output_count)
        }
        Stock::PantryMeals => {
            ((FOOD_CAPACITY - pantry.meals) / input_count).min(pantry.purse / output_count)
        }
        _ => 0,
    };
    uses as u16
}

pub(crate) fn settle(world: &mut World, stock: Stock, trade: &Trade) {
    // travel::trade has already checked capacity, price and inputs.
    let pantry = world
        .farming
        .pantry
        .as_mut()
        .expect("admitted pantry trade");
    let input = trade.give[0].count;
    match stock {
        Stock::PantrySale => {
            pantry.meals -= trade.get.count;
            pantry.purse += input;
        }
        Stock::PantryBiomass => {
            pantry.biomass += input;
            pantry.purse -= trade.get.count;
        }
        Stock::PantryMeals => {
            pantry.meals += input;
            pantry.purse -= trade.get.count;
        }
        _ => unreachable!("admitted pantry row"),
    }
}

pub(crate) fn can_donate(world: &World, item: Item) -> bool {
    active(world)
        && world.farming.pantry.as_ref().is_some_and(|p| match item {
            FOOD => p.meals < FOOD_CAPACITY,
            BIOMASS => p.biomass < BIOMASS_CAPACITY,
            _ => false,
        })
        && world.inventory.count_item(item) != 0
}

pub(crate) fn donate(world: &mut World, item: Item) -> Result<(), &'static str> {
    if !can_donate(world, item) {
        return Err("Bring supplies to a pantry with room");
    }
    world.inventory.remove_item(item, 1);
    let p = world
        .farming
        .pantry
        .as_mut()
        .expect("admitted pantry donation");
    if item == FOOD {
        p.meals += 1;
    } else {
        p.biomass += 1;
    }
    world.sync_from_inventory();
    world.expedition.say("Delivery received by the Fernwake kitchen");
    Ok(())
}

pub(crate) struct Report {
    pub(crate) summary: String,
    pub(crate) missing: String,
    pub(crate) cause: Option<&'static str>,
}

pub(crate) fn report(world: &World) -> Option<Report> {
    if !active(world) {
        return None;
    }
    let f = &world.farming;
    let pantry = f.pantry.as_ref()?;
    let missing = (pantry.served..pantry.demand)
        .map(|i| {
            if usize::from(i) == HOUSEHOLDS.len() {
                return "Receiving Clerk"; // A fifth diner can remain in an older save.
            }
            HOUSEHOLDS[(pantry.day as usize + usize::from(i)) % HOUSEHOLDS.len()]
        })
        .collect::<Vec<_>>().join(", ");
    let cause = if missing.is_empty() {
        None
    } else if pantry.meals != 0 {
        Some("Meal delivery ready to serve")
    } else {
        kitchen_issue(world).or_else(|| match RECIPE.as_ref() {
            None => Some("Trailcake recipe unavailable"),
            Some(batch) if pantry.biomass < batch.biomass => Some("Kitchen needs biomass"),
            Some(batch) if f.freight.market < batch.caps => Some(freight::supply_issue(world)),
            Some(_) => Some("Waiting for the next bake"),
        })
    };
    Some(Report {
        summary: format!(
            "{}/{} fed · {} meals in reserve · {} glowcaps · {} biomass",
            pantry.served, pantry.demand, pantry.meals, f.freight.market, pantry.biomass
        ),
        missing,
        cause,
    })
}
