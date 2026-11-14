//! Shared crafting availability and an all-or-nothing batch transaction.
use super::*;
use super::super::inventory::{CraftingRecipe, SlotAddr, SurvivalInventory};
use anyhow::{ensure, Result};
use std::collections::BTreeMap;

pub const MAX_BATCHES: u32 = 64;

pub fn at_bench(world: &World, eye_m: Vec3) -> bool {
    factory::at_bench(world, eye_m / world.voxel_scale.meters())
}

/// Built from the cached index and backpack, never by scanning storage slots.
pub struct Supplies(BTreeMap<(u32, u32), u64>);

impl Supplies {
    pub fn new(inventory: &SurvivalInventory, index: &Index) -> Self {
        let mut amounts: BTreeMap<_, _> = index.stock.iter().map(|s| (s.item.encode_u32(), s.available)).collect();
        let mut pack = BTreeMap::new();
        for (i, stack) in inventory.pack.iter().enumerate() {
            if inventory.is_protected(SlotAddr::Pack(i)) { continue; }
            if let Some(s) = stack { pack.entry(s.item.encode_u32()).or_insert((s.item, 0u64)).1 += u64::from(s.count); }
        }
        for (key, (item, n)) in pack {
            let usable = n.min(u64::from(inventory.count_item(item).saturating_sub(inventory.minimum(item))));
            *amounts.entry(key).or_default() += usable;
        }
        Self(amounts)
    }

    pub fn count(&self, item: Item) -> u64 { self.0.get(&item.encode_u32()).copied().unwrap_or(0) }

    /// Ingredient-limited count; output capacity is validated in the transaction.
    pub fn batches(&self, recipe: &CraftingRecipe, bench: bool) -> u32 {
        if recipe.needs_bench() && !bench { return 0; }
        requirements(recipe).values().filter(|(_, n)| *n > 0)
            .map(|(item,n)| self.count(*item) / n).min().unwrap_or(0).min(u64::from(MAX_BATCHES)) as u32
    }
}

fn requirements(recipe: &CraftingRecipe) -> BTreeMap<(u32,u32), (Item,u64)> {
    let mut result = BTreeMap::new();
    for &(item,n) in recipe.ingredients {
        result.entry(item.encode_u32()).or_insert((item,0u64)).1 += u64::from(n);
    }
    result
}

pub fn craft(world: &mut World, found: &[Container], eye_m: Vec3, id: usize, batches: u32) -> Result<u32> {
    ensure!((1..=MAX_BATCHES).contains(&batches), "Batch limit: 1–64");
    let recipe = super::super::inventory::all_recipes().get(id).ok_or_else(|| anyhow::anyhow!("Unknown recipe"))?;
    ensure!(!recipe.needs_bench() || at_bench(world, eye_m), "Workbench required");
    let output = recipe.output.count.checked_mul(batches).ok_or_else(|| anyhow::anyhow!("Output too large"))?;
    let (sources, mut slots) = load(world, found);
    let policies: Vec<_> = sources.iter().map(|s| policy::get(world, s.pos())).collect();
    let mut inv = world.inventory.clone();
    for (item, count) in requirements(recipe).into_values() {
        let n = u32::try_from(count * u64::from(batches)).map_err(|_| anyhow::anyhow!("Batch too large"))?;
        let from_pack = inv.take_from_pack(item, n.min(inv.count_item(item).saturating_sub(inv.minimum(item))));
        let taken = from_pack + policy::take(&mut slots, &policies, item, n - from_pack);
        ensure!(taken == n, "{}: {taken}/{n}", item.name());
    }
    ensure!(inv.try_insert(ItemStack { item: recipe.output.item, count: output }).1 == 0, "Inventory full");
    inv.crafted = inv.crafted.saturating_add(output);
    world.inventory = inv;
    store(world, &sources, &slots);
    Ok(output)
}
