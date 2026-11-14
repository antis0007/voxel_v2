use super::{CAPACITY, Node, blocks::Material};

pub const RESOURCE: usize = 13;
pub const FIRST_MODE: u32 = 3;

pub struct Recipe {
    pub grass: u32,
    pub water: u32,
    pub ticks: u32,
    pub power: u32,
}

pub const RECIPES: [Recipe; 2] = [
    Recipe { grass: 2, water: 0, ticks: 16, power: 8 },
    Recipe { grass: 1, water: 128, ticks: 32, power: 12 },
];

pub fn recipe(mode: u32) -> Option<&'static Recipe> {
    RECIPES.get(mode.checked_sub(FIRST_MODE)? as usize)
}

pub(super) fn step(n: &mut Node, recipe: &Recipe) -> u32 {
    let grass = Material::Grass.index();
    if n.blocks.input[grass] < recipe.grass {
        n.progress = 0;
        n.status = "Needs input";
        return 0;
    }
    if n.output[RESOURCE] > CAPACITY - 64 {
        n.status = "Output blocked";
        return 0;
    }
    if n.water < recipe.water {
        n.status = "Needs water";
        return 0;
    }
    let steps = (1 + n.upgrade).min(recipe.ticks - n.progress);
    let cost = recipe.power * steps;
    if n.energy < cost {
        n.status = "Needs cable / power";
        return 0;
    }
    n.energy -= cost;
    n.progress += steps;
    super::industry::produce(n, steps);
    n.status = "Processing";
    if n.progress == recipe.ticks {
        n.blocks.input[grass] -= recipe.grass;
        n.water -= recipe.water;
        n.output[RESOURCE] += 64;
        n.progress = 0;
    }
    cost
}
