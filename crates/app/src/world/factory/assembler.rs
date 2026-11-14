//! Assembles grid recipes with factory resources and supported bulk blocks.
//! Resources move by item pipe or adjacent chest; supported bulk blocks
//! enter through chests. Finished items return to adjacent chests.
use super::{CAPACITY, Factory, Kind, Node, blocks::Material};
use crate::world::inventory::{CraftingRecipe, Item, all_recipes};

/// Seconds per craft at the base drive.
pub const CRAFT_TICKS: u32 = 8;
/// Draw while crafting: 5 kW at the base drive.
pub const POWER: u32 = 5;
pub const STOCK_LIMIT: u32 = 256;
pub const SINGLE_BATCH: u32 = 1;

pub fn automatable(recipe: &CraftingRecipe) -> bool {
    recipe
        .ingredients
        .iter()
        .all(|&(item, _)| match item {
            Item::Resource(_) => true,
            Item::Block(block) => Material::from_block(block).is_some(),
            _ => false,
        })
        && matches!(recipe.output.item, Item::Resource(_) | Item::Block(_) | Item::Food(_)
            | Item::Tool(_) | Item::Weapon(_) | Item::Armor(..) | Item::Bucket(0))
}

/// Bound for a saved `target` (recipe index + 1).
pub fn recipe_count() -> usize {
    all_recipes().len()
}

/// Recipes an assembler can be set to, as indices into `all_recipes`.
pub fn choices() -> Vec<usize> {
    all_recipes()
        .iter()
        .enumerate()
        .filter(|(_, r)| automatable(r))
        .map(|(i, _)| i)
        .collect()
}

pub fn recipe(n: &Node) -> Option<&'static CraftingRecipe> {
    n.target
        .checked_sub(1)
        .and_then(|i| all_recipes().get(i as usize))
        .filter(|r| automatable(r))
}

/// Resource units per craft for each ingredient.
fn needs(recipe: &CraftingRecipe) -> impl Iterator<Item = (usize, u32)> + '_ {
    recipe.ingredients.iter().filter_map(|&(item, n)| match item {
        Item::Resource(i) => Some((i as usize, n * 64)),
        _ => None,
    })
}

pub fn accepts(n: &Node, i: usize) -> bool {
    recipe(n).is_some_and(|r| needs(r).any(|(j, _)| j == i))
}

pub fn block_needs(recipe: &CraftingRecipe) -> impl Iterator<Item = (Material, u32)> + '_ {
    recipe.ingredients.iter().filter_map(|&(item, count)| match item {
        Item::Block(block) => Material::from_block(block).map(|material| (material, count)),
        _ => None,
    })
}

impl Factory {
    pub(super) fn run_assemblers(&mut self) {
        for n in self.nodes.values_mut() {
            if n.kind != Kind::Assembler || !n.enabled {
                continue;
            }
            let Some(recipe) = recipe(n) else {
                n.status = "No recipe";
                continue;
            };
            if !needs(recipe).all(|(i, k)| n.input[i] >= k)
                || !block_needs(recipe).all(|(material, count)| n.blocks.input[material.index()] >= count) {
                n.progress = 0;
                n.status = "Needs input";
                continue;
            }
            let out = recipe.output;
            let room = match out.item {
                Item::Resource(r) => n.output[r as usize] + out.count * 64 <= CAPACITY,
                _ => n.stock + out.count <= STOCK_LIMIT,
            };
            if !room {
                n.status = "Output blocked";
                continue;
            }
            let steps = (1 + n.upgrade).min(CRAFT_TICKS - n.progress);
            let cost = POWER * steps;
            if n.energy < cost {
                n.status = "Needs cable / power";
                continue;
            }
            n.energy -= cost;
            self.stats.power(0, cost);
            n.progress += steps;
            n.status = "Processing";
            if n.progress >= CRAFT_TICKS {
                for (i, k) in needs(recipe) {
                    n.input[i] -= k;
                }
                for (material, count) in block_needs(recipe) {
                    n.blocks.input[material.index()] -= count;
                }
                match out.item {
                    Item::Resource(r) => n.output[r as usize] += out.count * 64,
                    _ => n.stock += out.count,
                }
                n.progress = 0;
                if n.mode == SINGLE_BATCH {
                    n.enabled = false;
                    n.status = "Batch Complete";
                }
            }
        }
    }
}

/// The finished item held in the assembler's inventory output.
pub fn stock_item(n: &Node) -> Option<Item> {
    let item = recipe(n)?.output.item;
    (!matches!(item, Item::Resource(_))).then_some(item)
}

pub(crate) mod review_fixture;
pub(crate) mod window_review;
#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod batch_tests;
mod chests;
pub(super) use chests::{has_output, transfer as chests};

#[cfg(test)]
#[path = "assembler/chest_tests.rs"]
mod chest_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_assembler_builds_a_generator_kit_from_ingots() {
        let generator = all_recipes()
            .iter()
            .position(|r| r.output.item == Item::Block(Kind::Generator.block()))
            .unwrap();
        assert!(choices().contains(&generator));
        let mut f = Factory::default();
        let p = glam::IVec3::ZERO;
        let mut n = Node::new(Kind::Assembler);
        n.target = generator as u32 + 1;
        n.input[6] = 128;
        n.input[8] = 128;
        assert!(accepts(&n, 6) && !accepts(&n, 3));
        f.nodes.insert(p, n);
        for _ in 0..CRAFT_TICKS {
            f.nodes.get_mut(&p).unwrap().energy = 64;
            f.run_assemblers();
        }
        let n = &f.nodes[&p];
        assert_eq!(n.stock, 1);
        assert_eq!(n.input[6] + n.input[8], 0);
        assert_eq!(stock_item(n), Some(Item::Block(Kind::Generator.block())));
    }
}
