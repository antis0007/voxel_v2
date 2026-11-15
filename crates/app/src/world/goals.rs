//! Next-goal guidance track: ordered goals from `assets/goals.ron` and the set already met.
//! Completed ids persist in `sidecar::FILE` beside the world, not in the save body.
pub mod sidecar;
pub mod table;
use super::inventory::{Item, SurvivalInventory, all_recipes};
use super::{BlockId, World, factory::Kind};
use std::sync::Arc;
pub use table::{Condition, Event, GOALS, Goal, Tier};

/// Seconds between evaluations; each one is O(goals).
const PERIOD: f32 = 0.25;
const MAX_PLACED: usize = 32;

#[derive(Clone, Debug, Default)]
pub struct State {
    /// Ids of met goals, in the order they were met.
    pub completed: Vec<String>,
    events: u32,
    placed: Vec<BlockId>,
    saw_night: bool,
    saw_day: bool,
    /// Set after the first evaluation, which counts the starting kit silently.
    baselined: bool,
    timer: f32,
    /// Goals met this session; the HUD flashes when it changes.
    met_count: u32,
    last_met: Option<String>,
    view: Arc<View>,
}

/// What the evaluator reads from the world.
pub struct Signals<'a> {
    pub inventory: &'a SurvivalInventory,
    pub machine: &'a dyn Fn(Kind) -> bool,
    /// Guardians defeated so far.
    pub guardians: usize,
    /// The world has a starter workshop to walk into.
    pub workshop: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ingredient {
    pub item: Item,
    pub held: u32,
    pub needed: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub tier: Option<Tier>,
    pub title: String,
    pub action: String,
    pub done: bool,
    pub optional: bool,
    pub recipe_item: Option<Item>,
}

/// HUD and Goals page snapshot, rebuilt at each evaluation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub current: Option<Row>,
    /// Recipe navigation is independent of a measured possession count.
    pub recipe_item: Option<Item>,
    /// The item a "hold N" goal asks for and how many are held.
    pub target: Option<Ingredient>,
    pub ingredients: Vec<Ingredient>,
    pub rows: Vec<Row>,
    pub met_count: u32,
    pub last_met: Option<String>,
}

impl State {
    pub fn note(&mut self, event: Event) {
        self.events |= event.bit();
    }

    /// Record a committed production milestone before a save can capture its output.
    pub(super) fn note_production(&mut self, event: Event) {
        if self.events & event.bit() != 0 {
            return;
        }
        self.note(event);
        for goal in GOALS.iter().filter(|goal| goal.condition == Condition::Event(event)) {
            if !self.is_done(&goal.id) {
                self.complete(goal, false);
            }
        }
    }

    fn complete(&mut self, goal: &Goal, silent: bool) {
        self.completed.push(goal.id.clone());
        if !silent {
            self.met_count = self.met_count.wrapping_add(1);
            self.last_met = Some(goal.title.clone());
        }
    }

    pub fn placed(&mut self, block: BlockId) {
        if block == BlockId::Air || self.placed.contains(&block) {
            return;
        }
        if self.placed.len() >= MAX_PLACED {
            self.placed.remove(0);
        }
        self.placed.push(block);
    }

    pub fn is_done(&self, id: &str) -> bool {
        self.completed.iter().any(|c| c == id)
    }

    pub fn view(&self) -> Arc<View> {
        self.view.clone()
    }

    /// Latches `NightSurvived` on the first dawn after a night was seen.
    /// A world that starts before dawn does not count its opening hours: the night must
    /// begin after a day was seen.
    pub fn observe_time(&mut self, night: bool) {
        if night {
            self.saw_night |= self.saw_day;
        } else {
            self.saw_day = true;
            if self.saw_night {
                self.note(Event::NightSurvived);
            }
        }
    }

    pub fn met(&self, condition: Condition, s: &Signals) -> bool {
        match condition {
            Condition::HasItem(item, count) => s.inventory.count_item(item) >= count,
            Condition::Placed(block) => self.placed.contains(&block),
            Condition::Machine(kind) => (s.machine)(kind),
            // A world with no workshop has nothing to enter, so the row is skipped.
            Condition::Event(Event::WorkshopEntered) => !s.workshop || self.events & Event::WorkshopEntered.bit() != 0,
            Condition::Event(event) => self.events & event.bit() != 0,
            Condition::GuardianDefeated => s.guardians > 0,
        }
    }

    /// Marks every unmet goal whose condition holds, then rebuilds the view.
    pub fn evaluate(&mut self, goals: &[Goal], s: &Signals) {
        // A fresh track credits what the starting kit already holds without a flash.
        let silent = !self.baselined && self.completed.is_empty();
        self.baselined = true;
        for goal in goals {
            if !self.is_done(&goal.id) && self.met(goal.condition, s) {
                self.complete(goal, silent);
            }
        }
        let view = self.build_view(goals, s.inventory);
        if *self.view != view {
            self.view = Arc::new(view);
        }
    }

    /// First unmet goal in table order.
    pub fn current<'a>(&self, goals: &'a [Goal]) -> Option<&'a Goal> {
        goals.iter().find(|g| !g.optional && !self.is_done(&g.id))
    }

    fn build_view(&self, goals: &[Goal], inventory: &SurvivalInventory) -> View {
        let row = |g: &Goal| Row {
            tier: Some(g.tier),
            title: g.title.clone(),
            action: g.action.clone(),
            done: self.is_done(&g.id),
            optional: g.optional,
            recipe_item: g.recipe_item(),
        };
        let current = self.current(goals);
        View {
            current: current.map(row),
            recipe_item: current.and_then(Goal::recipe_item),
            // Only a "hold N" goal has a count the inventory measures; a placed or built
            // block held in a pocket would read as done before it is.
            target: current.and_then(|g| match g.condition {
                Condition::HasItem(item, needed) => Some(Ingredient { item, held: inventory.count_item(item), needed }),
                _ => None,
            }),
            ingredients: current.map_or_else(Vec::new, |g| {
                let target = g.recipe.map(|item| Condition::HasItem(item, 1)).unwrap_or(g.condition);
                ingredients(target, inventory)
            }),
            rows: goals.iter().map(row).collect(),
            met_count: self.met_count,
            last_met: self.last_met.clone(),
        }
    }
}

/// Inputs for the goal's target item, scaled to the crafts it needs.
/// A target without a recipe lists itself.
pub fn ingredients(condition: Condition, inventory: &SurvivalInventory) -> Vec<Ingredient> {
    let Some((item, count)) = condition.target() else { return Vec::new() };
    let entry = |item, needed| Ingredient { item, held: inventory.count_item(item), needed };
    match all_recipes().iter().find(|r| r.output.item == item && r.output.count > 0) {
        Some(recipe) => {
            let crafts = count.div_ceil(recipe.output.count);
            recipe.ingredients.iter().map(|&(i, n)| entry(i, n.saturating_mul(crafts))).collect()
        }
        None => vec![entry(item, count)],
    }
}

/// Evaluates the track at most every `PERIOD` seconds.
pub fn update(world: &mut World, dt: f32, night: bool) {
    let goals = &mut world.goals;
    if world.adventure.health > 0.0 { goals.observe_time(night); } else { goals.saw_night = false; }
    goals.timer -= dt;
    if goals.timer > 0.0 {
        return;
    }
    goals.timer = PERIOD;
    let factory = &world.factory;
    let machine = |kind: Kind| factory.nodes.values().any(|n| n.kind == kind);
    let signals = Signals {
        inventory: &world.inventory,
        machine: &machine,
        guardians: world.adventure.records.values().filter(|n| **n & 256 != 0).count(),
        workshop: super::workshop::home(&world.sample_config()).is_some(),
    };
    world.goals.evaluate(&GOALS, &signals);
}

/// Latches `WorkshopEntered` once the player stands inside the starter workshop.
/// `feet_m` is the player's feet in metres.
pub fn visit(world: &mut World, feet_m: glam::Vec3) {
    if world.goals.events & Event::WorkshopEntered.bit() != 0 || world.goals.is_done("enter_workshop") { return; }
    let Some(home) = super::workshop::home(&world.sample_config()) else { return };
    // The bench stands at hall coordinates (-3.5, 2.5); the hall centre is where the walls meet.
    let d = feet_m - (home + glam::Vec3::new(HALL_CENTRE_FROM_BENCH.0, 0.0, HALL_CENTRE_FROM_BENCH.1));
    if d.x.abs() <= WORKSHOP_HALF_WIDTH_M && d.z.abs() <= WORKSHOP_HALF_DEPTH_M && (-2.0..=WORKSHOP_HEIGHT_M).contains(&d.y) {
        world.goals.note(Event::WorkshopEntered);
    }
}

// The starter hall is 18 m by 14.4 m; the bounds stay a little inside its walls.
const HALL_CENTRE_FROM_BENCH: (f32, f32) = (3.5, -2.5);
const WORKSHOP_HALF_WIDTH_M: f32 = 8.5;
const WORKSHOP_HALF_DEPTH_M: f32 = 6.5;
const WORKSHOP_HEIGHT_M: f32 = 4.0;
