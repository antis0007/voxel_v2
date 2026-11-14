//! Exact recipe-book transactions without an ambiguous intermediate grid.
use super::*;

pub const MAX_SELECTED_BATCHES: u32 = 64;

/// Guards selected-recipe commands against differing local datapacks.
#[cfg(feature = "multiplayer")]
pub fn selected_recipe_key(id: usize) -> Option<u64> {
    let recipe = all_recipes().get(id)?;
    let mut hash = 0xcbf29ce484222325u64;
    let mut word = |n: u32| { for byte in n.to_le_bytes() { hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3); } };
    let (kind, value) = recipe.output.item.encode_u32();
    for n in [kind, value, recipe.output.count, u32::from(RECIPES.bench.contains(recipe)), recipe.pattern.len() as u32] { word(n); }
    for row in recipe.pattern { word(row.chars().count() as u32); }
    word(recipe.ingredients.len() as u32);
    for &(item, count) in recipe.ingredients {
        let (kind, value) = item.encode_u32();
        for n in [kind, value, count] { word(n); }
    }
    Some(hash)
}

impl SurvivalInventory {
    /// Crafts the selected catalog entry, committing only complete batches.
    pub fn craft_selected_recipe(&mut self, id: usize, to_pack: bool, bench: bool) -> Result<ItemStack, CraftBlocker> {
        #[cfg(feature = "multiplayer")]
        let _net_record = crate::multiplayer::inventory::record(crate::multiplayer::inventory::Command::CraftRecipe(id, selected_recipe_key(id).unwrap_or(0), to_pack));
        let recipe = all_recipes().get(id).ok_or(CraftBlocker::Ingredients)?;
        self.craft_selected(recipe, to_pack, bench)
    }

    fn craft_selected(&mut self, recipe: &CraftingRecipe, to_pack: bool, bench: bool) -> Result<ItemStack, CraftBlocker> {
        let _units = recipe.ingredients.iter().try_fold(0u32, |total, &(_, n)| total.checked_add(n)).ok_or(CraftBlocker::Ingredients)?;
        if recipe.needs_bench() && !bench { return Err(CraftBlocker::Workbench); }
        if recipe.output.count == 0 || recipe.cells(bench, 1).is_none() { return Err(CraftBlocker::Ingredients); }
        let mut requirements = Vec::<(Item, u32)>::new();
        for &(item, count) in recipe.ingredients {
            if count == 0 { return Err(CraftBlocker::Ingredients); }
            if let Some((_, total)) = requirements.iter_mut().find(|(other, _)| *other == item) {
                *total = total.checked_add(count).ok_or(CraftBlocker::Ingredients)?;
            } else { requirements.push((item, count)); }
        }
        // Bound work and prevent outputs from extending their own initial supply.
        let available = requirements.iter().map(|&(item, count)| self.count_item(item) / count).min().unwrap_or(0);
        let limit = available.min(if to_pack { MAX_SELECTED_BATCHES } else { 1 });
        let mut next = self.clone();
        let mut produced = 0u32;
        let mut blocker = CraftBlocker::Ingredients;
        for _ in 0..limit {
            let mut batch = next.clone();
            if !requirements.iter().all(|&(item, count)| batch.remove_item(item, count)) { break; }
            let output = recipe.output;
            if to_pack {
                if batch.try_insert_pack_first(output).1 != 0 {
                    blocker = CraftBlocker::OutputSpace;
                    break;
                }
            } else {
                let held = match batch.cursor {
                    None => 0,
                    Some(stack) if stack.item == output.item => stack.count,
                    _ => { blocker = CraftBlocker::Cursor; break; }
                };
                if output.count > batch.stack_limit(output.item).saturating_sub(held) {
                    blocker = CraftBlocker::OutputSpace;
                    break;
                }
                batch.cursor = Some(ItemStack::new(output.item, held + output.count));
            }
            let Some(total) = produced.checked_add(output.count) else { break; };
            produced = total;
            batch.crafted = batch.crafted.saturating_add(output.count);
            next = batch;
        }
        if produced == 0 { return Err(blocker); }
        *self = next;
        Ok(ItemStack::new(recipe.output.item, produced))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_crafting_duplicate_self_input_does_not_extend_initial_supply() {
        let recipe = CraftingRecipe { name: "Test", output: ItemStack::new(Item::Resource(4), 2),
            pattern: &[], key: &[], ingredients: &[(Item::Resource(4), 1), (Item::Resource(4), 1)], description: "" };
        let mut inv = SurvivalInventory::empty();
        inv.pack[0] = Some(ItemStack::new(Item::Resource(4), 4));
        assert_eq!(inv.craft_selected(&recipe, true, true), Ok(ItemStack::new(Item::Resource(4), 4)));
        assert_eq!(inv.count_item(Item::Resource(4)), 4);
        assert_eq!(inv.crafted, 4);
    }
    #[test]
    fn selected_crafting_oversized_datapack_input_is_rejected_without_overflow() {
        let recipe = CraftingRecipe { name: "Test", output: ItemStack::new(Item::Resource(4), 1),
            pattern: &[], key: &[], ingredients: &[(Item::Resource(4), u32::MAX), (Item::Resource(6), 1)], description: "" };
        let mut inv = SurvivalInventory::empty();
        let before = inv.clone();
        assert_eq!(inv.craft_selected(&recipe, true, true), Err(CraftBlocker::Ingredients));
        assert_eq!(inv, before);
    }
}
