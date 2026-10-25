//! Shared crafting admission for the screen and the committing operation.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftBlocker {
    Ingredients,
    Workbench,
    OutputSpace,
    Cursor,
}
impl CraftBlocker {
    pub fn message(self) -> &'static str {
        match self {
            Self::Ingredients => "The grid does not contain a complete recipe",
            Self::Workbench => "This recipe needs a workbench",
            Self::OutputSpace => "Make room for the complete output",
            Self::Cursor => "Put down the held stack before taking the output",
        }
    }
}

impl SurvivalInventory {
    pub fn grid_craft_batches(&self, to_pack: bool, bench: bool) -> Result<u32, CraftBlocker> {
        let (recipe, available) = self.matching_grid_recipe(bench).ok_or_else(|| {
            if !bench && self.matching_grid_recipe(true).is_some() {
                CraftBlocker::Workbench
            } else {
                CraftBlocker::Ingredients
            }
        })?;
        let output = recipe.output;
        let room = if to_pack {
            self.remaining_capacity_for(output.item)
        } else {
            match self.cursor {
                None => self.stack_limit(output.item),
                Some(cursor) if cursor.item == output.item => {
                    self.stack_limit(output.item).saturating_sub(cursor.count)
                }
                _ => return Err(CraftBlocker::Cursor),
            }
        };
        let batches = available.min(room / output.count.max(1));
        if batches == 0 { Err(CraftBlocker::OutputSpace) } else { Ok(if to_pack { batches } else { 1 }) }
    }

    pub fn armor_comparison(&self, item: Item) -> Option<(f32, f32)> {
        let Item::Armor(slot, _) = item else {
            return None;
        };
        let mut next = Self::empty();
        next.equipment = self.equipment;
        next.equipment[slot as usize] = Some(ItemStack::new(item, 1));
        Some((self.armor_reduction(), next.armor_reduction()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crafting_rejected_staging_does_not_move_any_stack() {
        let mut inv = SurvivalInventory::empty();
        inv.pack.fill(Some(ItemStack::new(Item::Block(BlockId::Stone), 64)));
        inv.hotbar.fill(Some(ItemStack::new(Item::Block(BlockId::Stone), 64)));
        inv.pack[0] = None;
        inv.craft_grid[0] = Some(ItemStack::new(Item::Block(BlockId::Dirt), 64));
        inv.craft_grid[1] = Some(ItemStack::new(Item::Block(BlockId::Sand), 64));
        let before = inv.clone();
        assert!(!inv.stage_recipe_into_grid(&all_recipes()[0], false));
        assert_eq!(inv.pack, before.pack);
        assert_eq!(inv.hotbar, before.hotbar);
        assert_eq!(inv.craft_grid, before.craft_grid);
    }
    #[test]
    fn crafting_admission_and_commit_agree_on_blocked_output() {
        let recipe = all_recipes().iter().find(|r| !r.needs_bench()).unwrap();
        let mut inv = SurvivalInventory::empty();
        for (i, &(item, count)) in recipe.ingredients.iter().enumerate() {
            inv.craft_grid[i] = Some(ItemStack::new(item, count));
        }
        inv.cursor = Some(ItemStack::new(Item::Weapon(5), 1));
        let before = inv.craft_grid;
        assert_eq!(inv.grid_craft_batches(false, false), Err(CraftBlocker::Cursor));
        assert!(inv.craft_from_grid(false, false).is_none());
        assert_eq!(inv.craft_grid, before);
    }
}
