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
            Self::Ingredients => "Incomplete Recipe",
            Self::Workbench => "Workbench Required",
            Self::OutputSpace => "Output Full",
            Self::Cursor => "Cursor Occupied",
        }
    }
}

impl CraftingRecipe {
    /// Grid cells, items and counts that stage `batches` crafts, or `None`
    /// when the recipe does not fit the grid in use. Shapeless rows sit one
    /// unit per cell when they fit, else one stack per item.
    pub fn cells(&self, bench: bool, batches: u32) -> Option<Vec<(usize, Item, u32)>> {
        let usable: &[usize] = if bench { &[0, 1, 2, 3, 4, 5, 6, 7, 8] } else { &HAND_GRID };
        let mut cells = Vec::new();
        if let Some(shape) = self.shape() {
            for (i, item) in shape.cells.iter().enumerate() {
                if let Some(item) = *item {
                    cells.push((i, item, batches));
                }
            }
        } else {
            let units: u32 = self.ingredients.iter().map(|&(_, n)| n).sum();
            let per_unit = units as usize <= usable.len();
            let mut free = usable.iter().copied();
            for &(item, n) in self.ingredients {
                let (stacks, count) = if per_unit { (n, batches) } else { (1, n * batches) };
                for _ in 0..stacks {
                    cells.push((free.next()?, item, count));
                }
            }
        }
        cells.iter().all(|(i, _, _)| usable.contains(i)).then_some(cells)
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

    /// Batches a shaped recipe makes from the grid: the smallest stack under
    /// a pattern whose bounds match the occupied cells, mirrored or not.
    pub(super) fn shaped_batches(&self, recipe: &CraftingRecipe) -> Option<u32> {
        let shape = recipe.shape()?;
        let occupied = (0..CRAFT_SLOTS).filter(|&i| self.craft_grid[i].is_some());
        let (r0, c0) = occupied.clone().fold((3, 3), |(r, c), i| (r.min(i / 3), c.min(i % 3)));
        let (r1, c1) = occupied.fold((0, 0), |(r, c), i| (r.max(i / 3 + 1), c.max(i % 3 + 1)));
        if r1 < r0 || r1 - r0 != shape.height || c1 - c0 != shape.width {
            return None;
        }
        let fits = |mirror: bool| {
            (0..shape.height).all(|r| {
                (0..shape.width).all(|c| {
                    let pc = if mirror { shape.width - 1 - c } else { c };
                    self.craft_grid[(r0 + r) * 3 + c0 + c].map(|s| s.item) == shape.cells[r * 3 + pc]
                })
            })
        };
        if !fits(false) && !fits(true) {
            return None;
        }
        self.craft_grid.iter().flatten().map(|s| s.count).min()
    }

    /// Moves `batches` crafts of `recipe` from the inventory into an empty
    /// grid. Checks everything first, so a `false` leaves the grid untouched.
    pub(super) fn place_recipe(&mut self, recipe: &CraftingRecipe, bench: bool, batches: u32) -> bool {
        let Some(cells) = recipe.cells(bench, batches) else { return false };
        let fits = cells.iter().all(|&(_, item, n)| n <= self.stack_limit(item));
        let owned = recipe.ingredients.iter().all(|&(item, n)| self.count_item(item) >= n * batches);
        if !fits || !owned || self.craft_grid.iter().any(Option::is_some) {
            return false;
        }
        for (i, item, count) in cells {
            if !self.remove_item(item, count) {
                return false;
            }
            self.craft_grid[i] = Some(ItemStack { item, count });
        }
        true
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
        for &(item, count) in recipe.ingredients {
            let _ = inv.try_insert(ItemStack::new(item, count));
        }
        assert!(inv.stage_recipe_into_grid(recipe, false));
        inv.cursor = Some(ItemStack::new(Item::Weapon(5), 1));
        let before = inv.craft_grid;
        assert_eq!(inv.grid_craft_batches(false, false), Err(CraftBlocker::Cursor));
        assert!(inv.craft_from_grid(false, false).is_none());
        assert_eq!(inv.craft_grid, before);
    }

    fn recipe(name: &str) -> &'static CraftingRecipe {
        all_recipes().iter().find(|r| r.name == name).unwrap()
    }

    fn mirrored(shape: &Shape) -> [Option<Item>; CRAFT_SLOTS] {
        let mut cells = [None; CRAFT_SLOTS];
        for r in 0..shape.height {
            for c in 0..shape.width {
                cells[r * 3 + c] = shape.cells[r * 3 + shape.width - 1 - c];
            }
        }
        cells
    }

    #[test]
    fn patterns_are_trimmed_and_unambiguous() {
        let shaped: Vec<_> = all_recipes().iter().filter(|r| r.shaped()).collect();
        assert!(!shaped.is_empty());
        for (k, r) in shaped.iter().enumerate() {
            let s = r.shape().unwrap();
            assert!(s.width <= 3 && s.height <= 3, "{}: larger than 3 × 3", r.name);
            let row_used = |row: usize| (0..3).any(|c| s.cells[row * 3 + c].is_some());
            let col_used = |col: usize| (0..3).any(|row| s.cells[row * 3 + col].is_some());
            assert!(row_used(0) && row_used(s.height - 1), "{}: blank edge row", r.name);
            assert!(col_used(0) && col_used(s.width - 1), "{}: blank edge column", r.name);
            for other in &shaped[k + 1..] {
                let o = other.shape().unwrap();
                let same = (o.width, o.height) == (s.width, s.height)
                    && (o.cells == s.cells || o.cells == mirrored(&s));
                assert!(!same, "{} and {} share a pattern", r.name, other.name);
            }
        }
    }

    #[test]
    fn shaped_recipe_matches_anywhere_and_mirrored() {
        let torch = recipe("Torch (Coal)").shape().unwrap();
        let mut inv = SurvivalInventory::empty();
        inv.craft_grid[1] = torch.cells[0].map(|i| ItemStack::new(i, 3));
        inv.craft_grid[4] = torch.cells[3].map(|i| ItemStack::new(i, 2));
        let (found, batches) = inv.matching_grid_recipe(false).unwrap();
        assert_eq!(found.name, "Torch (Coal)");
        assert_eq!(batches, 2);
        inv.craft_grid.swap(1, 4);
        assert!(inv.matching_grid_recipe(false).is_none(), "upside down is another shape");

        let bricks = recipe("Blast bricks");
        let shape = bricks.shape().unwrap();
        let mut inv = SurvivalInventory::empty();
        for (i, item) in mirrored(&shape).iter().enumerate() {
            inv.craft_grid[i] = item.map(|i| ItemStack::new(i, 1));
        }
        assert_eq!(inv.matching_grid_recipe(true).map(|(r, _)| r.name), Some(bricks.name));
    }

    #[test]
    fn shaped_craft_takes_one_per_cell_and_stacked_cells_do_not_match() {
        let sword = recipe("Copper sword");
        let shape = sword.shape().unwrap();
        let (blade, grip) = (shape.cells[0].unwrap(), shape.cells[6].unwrap());
        let mut inv = SurvivalInventory::empty();
        inv.craft_grid[0] = Some(ItemStack::new(blade, 2));
        inv.craft_grid[3] = Some(ItemStack::new(grip, 1));
        assert!(inv.matching_grid_recipe(true).is_none_or(|(r, _)| r.name != sword.name));
        for i in [0, 3, 6] {
            inv.craft_grid[i] = shape.cells[i].map(|item| ItemStack::new(item, 2));
        }
        assert!(inv.matching_grid_recipe(false).is_none(), "three rows need the bench");
        assert_eq!(inv.matching_grid_recipe(true).map(|(r, b)| (r.name, b)), Some((sword.name, 2)));
        let made = inv.craft_from_grid(false, true).unwrap();
        assert_eq!(made.count, sword.output.count);
        assert!(inv.craft_grid.iter().flatten().all(|s| s.count == 1));
    }

    #[test]
    fn staging_again_adds_one_batch() {
        let bench = recipe("Artificer workbench");
        let mut inv = SurvivalInventory::empty();
        inv.pack[0] = Some(ItemStack::new(Item::Block(BlockId::Planks), 9));
        assert!(inv.stage_recipe_into_grid(bench, false));
        assert!(inv.stage_recipe_into_grid(bench, false));
        assert_eq!(inv.matching_grid_recipe(false).map(|(_, b)| b), Some(2));
        assert_eq!(inv.pack.iter().flatten().map(|s| s.count).sum::<u32>(), 1);
        // Not enough for a third: the grid keeps two.
        assert!(inv.stage_recipe_into_grid(bench, false));
        assert_eq!(inv.matching_grid_recipe(false).map(|(_, b)| b), Some(2));
    }

    #[test]
    fn shapeless_kit_stages_one_unit_per_cell() {
        let kit = all_recipes().iter().find(|r| r.output.item == Item::Block(BlockId::CoalGenerator)).unwrap();
        assert!(!kit.shaped());
        let mut inv = SurvivalInventory::empty();
        inv.pack[0] = Some(ItemStack::new(Item::Resource(6), 8));
        inv.pack[1] = Some(ItemStack::new(Item::Resource(8), 8));
        assert!(inv.stage_recipe_into_grid(kit, true));
        assert_eq!(inv.craft_grid.iter().flatten().count(), 4);
        assert_eq!(inv.matching_grid_recipe(true).map(|(r, _)| r.name), Some(kit.name));
    }
}
