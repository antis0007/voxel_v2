//! Survival break time, tool class and harvest tier from `assets/mining.ron`.
//!
//! The multiplayer edit path can call [`survival_break`] and [`harvest`] with
//! the same [`Tool`] the local loop resolves through [`tool_for`].
use super::inventory::Item;
use super::{BlockId, World};
use glam::{IVec3, Vec3};
use std::collections::HashSet;
use std::sync::LazyLock;

const MINING_RON: &str = include_str!("../../../../assets/mining.ron");

#[cfg(test)]
#[path = "../crystal_xp_review.rs"]
mod crystal_xp_review;
const MINING_HEADER: &str = "\
// Survival mining: break seconds = hardness (assets/materials.ron) x seconds_per_hardness / speed.
// A tool of the wrong class mines at speed x wrong_class_factor, never below the hand, and
// harvests like the hand. A block above the tool's tier takes under_tier_factor times longer
// and drops nothing. Dig fits rock, Cut fits timber, Scoop fits soil. `kit` lists the item each workbench bore upgrade (expedition tool tier
// 1, 2) mines as when the held item has no row. Blocks without a rule are Dig, tier 0.
// `cargo test mining_ron_round_trips` checks the format.
";

/// Seconds per tool strike while mining: the viewmodel chop clip's length.
pub fn strike_seconds() -> f32 { crate::player::viewmodel::clips::Name::Chop.clip().duration }
/// Fraction of each strike period at which the chop lands and the hit sounds.
pub fn strike_phase() -> f32 { crate::player::viewmodel::clips::Name::Chop.clip().impact_fraction() }

/// Phase within the current tool stroke, 0..1, for a break `elapsed` of `needed` seconds.
/// Strokes stretch from `strike_seconds` so the last one lands exactly when the cell breaks.
pub fn stroke_phase(elapsed: f32, needed: f32) -> f32 {
    let (seconds, phase) = (strike_seconds(), strike_phase());
    let strokes = (needed / seconds + 1.0 - phase).round().max(1.0);
    let period = needed / (strokes - 1.0 + phase);
    if !(period.is_finite() && period > 0.0) { return 0.0; }
    (elapsed / period).rem_euclid(1.0)
}

pub use engine_tools::capabilities::MiningClass as ToolClass;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRow {
    pub item: Item,
    pub class: ToolClass,
    pub tier: u8,
    pub speed: f32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockRule {
    pub class: ToolClass,
    pub tier: u8,
    pub blocks: Vec<BlockId>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Table {
    pub seconds_per_hardness: f32,
    pub min_seconds: f32,
    pub hand_speed: f32,
    pub wrong_class_factor: f32,
    pub under_tier_factor: f32,
    pub kit: Vec<Item>,
    pub tools: Vec<ToolRow>,
    pub blocks: Vec<BlockRule>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    ron::from_str(MINING_RON).unwrap_or_else(|error| panic!("assets/mining.ron: {error}"))
});

pub fn table() -> &'static Table {
    &TABLE
}

/// What breaks a block: the bare hand (`class: None`) or a tool row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tool {
    pub class: Option<ToolClass>,
    pub tier: u8,
    pub speed: f32,
    /// Hammer-class tools hold to charge and crack a crater instead of mining one cell.
    pub heavy: bool,
    sword: bool,
    fells_trees: bool,
}

impl Tool {
    pub fn hand() -> Self {
        Self { class: None, tier: 0, speed: TABLE.hand_speed, heavy: false, sword: false, fells_trees: false }
    }

    pub fn of(item: Item) -> Option<Self> {
        TABLE.tools.iter().find(|row| row.item == item).map(|row| Self {
            class: Some(row.class),
            tier: row.tier,
            speed: row.speed,
            heavy: false,
            sword: matches!(item, Item::Weapon(1..=4)),
            fells_trees: matches!(item, Item::Tool(12..=14)),
        })
    }
}

/// The held item's tool row, else the workbench bore kit, else the hand.
pub fn tool_for(held: Option<Item>, kit_tier: u32) -> Tool {
    held.and_then(Tool::of)
        .or_else(|| {
            let index = usize::try_from(kit_tier).ok()?.checked_sub(1)?;
            TABLE.kit.get(index).copied().and_then(Tool::of)
        })
        .unwrap_or_else(Tool::hand)
}

/// The class and minimum tier that harvest `block`.
pub fn requirement(block: BlockId) -> (ToolClass, u8) {
    TABLE
        .blocks
        .iter()
        .find(|rule| rule.blocks.contains(&block))
        .map_or((ToolClass::Dig, 0), |rule| (rule.class, rule.tier))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Break {
    pub seconds: f32,
    /// False when the block is above the tool's tier: it breaks with no drop.
    pub drops: bool,
    pub required_tier: u8,
}

/// Seconds a survival player holds the break button for one cell.
///
/// No voxel-scale input: one break action costs the same time at every scale.
pub fn survival_break(block: BlockId, tool: Tool) -> Break {
    if engine_content::thin::is_crop(block)
        || matches!(block, BlockId::Sapling | BlockId::Sugarcane | BlockId::SugarcaneShoot
            | BlockId::RedCap | BlockId::GlowCap | BlockId::AmberCap | BlockId::MushroomStem)
        || (tool.sword && matches!(block, BlockId::Bamboo | BlockId::BambooShoot))
    {
        return Break { seconds: 0.0, drops: true, required_tier: 0 };
    }
    let (class, required_tier) = requirement(block);
    let class = if matches!(block, BlockId::Bamboo | BlockId::BambooShoot) {
        ToolClass::Cut
    } else {
        class
    };
    let right_class = tool.class == Some(class);
    let (speed, tier) = match tool.class {
        // `Tool::hand` carries `hand_speed`; Haste may have scaled it.
        None => (tool.speed, 0),
        Some(_) if right_class => (tool.speed, tool.tier),
        Some(_) => ((tool.speed * TABLE.wrong_class_factor).max(TABLE.hand_speed), 0),
    };
    let drops = tier >= required_tier;
    let hardness = block.material().physics.hardness.max(0.0);
    let penalty = if drops { 1.0 } else { TABLE.under_tier_factor };
    let seconds = hardness * TABLE.seconds_per_hardness / speed.max(f32::EPSILON) * penalty;
    Break { seconds: seconds.max(TABLE.min_seconds), drops, required_tier }
}

/// Creative removes a cell on the click; survival follows [`survival_break`].
pub fn break_for(block: BlockId, tool: Tool, survival: bool) -> Break {
    if survival {
        survival_break(block, tool)
    } else {
        Break { seconds: 0.0, drops: true, required_tier: 0 }
    }
}

/// A held break on one cell; a new cell or a released button starts over.
#[derive(Clone, Copy, Debug, Default)]
pub struct Progress {
    target: Option<(IVec3, BlockId)>,
    elapsed: f32,
    needed: f32,
}

impl Progress {
    /// Advances by `dt`; stays ready until the target changes or input releases.
    /// Residency/admission may defer a completed break without losing its work.
    pub fn tick(&mut self, target: Option<(IVec3, BlockId)>, held: bool, needed: f32, dt: f32) -> bool {
        if !held || target.is_none() || target != self.target {
            self.target = if held { target } else { None };
            self.elapsed = 0.0;
        }
        self.needed = needed;
        if self.target.is_none() {
            return false;
        }
        self.elapsed = (self.elapsed + dt.max(0.0)).min(needed.max(0.0));
        if self.elapsed + 1e-6 >= needed {
            return true;
        }
        false
    }

    /// Seconds worked and seconds needed, while a break is in progress.
    pub fn timing(&self) -> Option<(f32, f32)> {
        (self.target.is_some() && self.needed > 0.0 && self.elapsed > 0.0).then_some((self.elapsed, self.needed))
    }

    /// Share of the current break done, while one is in progress.
    pub fn fraction(&self) -> Option<f32> {
        (self.target.is_some() && self.needed > 0.0 && self.elapsed > 0.0)
            .then(|| (self.elapsed / self.needed).clamp(0.0, 1.0))
    }
}

/// Survival harvest: an under-tier break removes the cell with no drop.
/// `tool` is `None` for creative edits, which always harvest.
pub fn harvest(
    world: &mut World,
    p: IVec3,
    tool: Option<Tool>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
    mut history: Option<&mut super::edit_history::EditHistory>,
    direction: Vec3,
) -> bool {
    let block = world.get(p);
    let collect = !world.adventure.creative;
    if let Some(changed) = super::farming::cut_stalk(world, p, collect, blocks, chunks, history.as_deref_mut()) {
        return changed;
    }
    if !harvest_cell(world, p, tool, blocks, chunks, direction) { return false; }
    if let Some(history) = history {
        history.record(world, p, block, world.get(p));
    }
    if let Some(tool) = tool.filter(|t| !t.heavy && t.class == Some(ToolClass::Dig) && t.tier >= 2)
        && crater::rock(block) && world.get(p) == BlockId::Air {
        crater::chip(world, p, block, tool, blocks, chunks);
    }
    true
}

fn harvest_cell(
    world: &mut World,
    p: IVec3,
    tool: Option<Tool>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
    direction: Vec3,
) -> bool {
    let block = world.get(p);
    let placed_crystal = block == BlockId::Crystal
        && world.block_data.placed_sediment.contains(&super::block_data::key(p));
    let changed = if tool.is_some_and(|tool| !survival_break(block, tool).drops) {
         super::effects::replace(world, p, block, super::farming::forage::vacated(block))
            .map(|commit| commit.publish(world, blocks, chunks))
            .is_ok()
    } else if let Some(tool) = tool.filter(|_| block == BlockId::Log) {
        super::expedition::harvest_log(world, p, tool.fells_trees.then_some(direction), blocks, chunks)
    } else {
        super::expedition::harvest(world, p, blocks, chunks)
    };
    if changed && world.get(p) != block && tool.is_some() {
        let drops = tool.is_some_and(|tool| survival_break(block, tool).drops);
        super::progression::on_harvest(world, if drops && !placed_crystal { block } else { BlockId::Air });
        world.stats.add(super::stats::Stat::Mined, 1);
    }
    changed
}

/// Resolve persistent tools without falling back to a legacy bore when broken.
/// Haste scales the result's speed.
pub fn tool_for_world(world: &World, held: Option<Item>) -> Tool {
    let mut tool = base_tool_for_world(world, held);
    tool.speed *= world.progression.effects.mining_factor();
    tool
}

fn base_tool_for_world(world:&World,held:Option<Item>)->Tool {
    if let Some(Item::Assembled{id,..})=held {
        let Some(tool)=world.adventure.tool_bank.get(id as usize) else{return Tool::hand()};
        if tool.is_broken() {return Tool::hand()}
        let Ok(stats)=tool.stats() else{return Tool::hand()};
        let Some(mining) = tool.capabilities().ok().and_then(|caps| caps.mining) else {return Tool::hand()};
        return Tool {
            class: Some(mining.class),
            tier: stats.mining_level,
            speed: stats.mining_speed,
            heavy: mining.charged,
            sword: mining.cuts_bamboo,
            fells_trees: mining.fells_trees,
        };
    }
    tool_for(held,world.expedition.tool_tier)
}

/// Tooltip stat lines for an item with a tool row.
pub fn stat_lines(item: Item) -> Vec<String> {
    Tool::of(item).map_or_else(Vec::new, |tool| {
        vec![format!("Tier: {}", tool.tier), format!("Mining Speed: {:.1}×", tool.speed)]
    })
}

pub mod charge;
pub mod crater;

/// Short-lived damage on exposed terrain; no save format change.
#[derive(Default)]
pub struct Fractures(Vec<(IVec3, BlockId, f32, f32)>);

impl Fractures {
    pub fn tick(&mut self, dt: f32) {
        self.0.retain_mut(|(_, _, _, life)| { *life -= dt; *life > 0.0 });
    }

    pub fn visible(&self) -> impl Iterator<Item = (IVec3, BlockId, f32)> + '_ {
        self.0.iter().map(|&(p, b, damage, life)| (p, b, damage * (life / 2.0).min(1.0)))
    }

    fn stress(&mut self, p: IVec3, b: BlockId, amount: f32) -> bool {
        if let Some((_, material, damage, life)) = self.0.iter_mut().find(|(cell, ..)| *cell == p) {
            if *material != b { *material = b; *damage = 0.0; }
            *damage = (*damage + amount).min(1.0);
            *life = 12.0;
            return *damage >= 1.0;
        }
        if self.0.len() >= 64
            && let Some((i, _)) = self.0.iter().enumerate().min_by(|a, b| a.1.3.total_cmp(&b.1.3)) {
            self.0.swap_remove(i);
        }
        self.0.push((p, b, amount.min(1.0), 12.0));
        amount >= 1.0
    }
}

#[cfg(test)]
mod tests;
