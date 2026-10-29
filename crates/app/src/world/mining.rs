//! Survival break time, tool class and harvest tier from `assets/mining.ron`.
//!
//! The multiplayer edit path can call [`survival_break`] and [`harvest`] with
//! the same [`Tool`] the local loop resolves through [`tool_for`].
use super::inventory::Item;
use super::{BlockId, World};
use glam::IVec3;
use std::collections::HashSet;
use std::sync::LazyLock;

const MINING_RON: &str = include_str!("../../../../assets/mining.ron");
const MINING_HEADER: &str = "\
// Survival mining: break seconds = hardness (assets/materials.ron) x seconds_per_hardness / speed.
// A tool of the wrong class mines at speed x wrong_class_factor, never below the hand, and
// harvests like the hand. A block above the tool's tier takes under_tier_factor times longer
// and drops nothing. `kit` lists the item each workbench bore upgrade (expedition tool tier
// 1, 2) mines as when the held item has no row. Blocks without a rule are Dig, tier 0.
// `cargo test mining_ron_round_trips` checks the format.
";

/// The kind of block a tool is made for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ToolClass {
    /// Rock, ore, soil and placed masonry.
    Dig,
    /// Wood, leaves and plants.
    Cut,
}

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
}

impl Tool {
    pub fn hand() -> Self {
        Self { class: None, tier: 0, speed: TABLE.hand_speed }
    }

    pub fn of(item: Item) -> Option<Self> {
        TABLE.tools.iter().find(|row| row.item == item).map(|row| Self {
            class: Some(row.class),
            tier: row.tier,
            speed: row.speed,
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
    let (class, required_tier) = requirement(block);
    let right_class = tool.class == Some(class);
    let (speed, tier) = match tool.class {
        None => (TABLE.hand_speed, 0),
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
    /// Advances by `dt`; true on the step the cell should break.
    pub fn tick(&mut self, target: Option<(IVec3, BlockId)>, held: bool, needed: f32, dt: f32) -> bool {
        if !held || target.is_none() || target != self.target {
            self.target = if held { target } else { None };
            self.elapsed = 0.0;
        }
        self.needed = needed;
        if self.target.is_none() {
            return false;
        }
        self.elapsed += dt;
        if self.elapsed + 1e-6 >= needed {
            self.elapsed = 0.0;
            return true;
        }
        false
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
) -> bool {
    let block = world.get(p);
    if tool.is_some_and(|tool| !survival_break(block, tool).drops) {
        return super::effects::replace(world, p, block, BlockId::Air)
            .map(|commit| commit.publish(world, blocks, chunks))
            .is_ok();
    }
    let changed = super::expedition::harvest(world, p, blocks, chunks);
    if changed && tool.is_some() && world.get(p) != block {
        super::progression::on_harvest(world, block);
    }
    changed
}

/// Tooltip stat lines for an item with a tool row.
pub fn stat_lines(item: Item) -> Vec<String> {
    Tool::of(item).map_or_else(Vec::new, |tool| {
        vec![format!("Tier: {}", tool.tier), format!("Mining Speed: {:.1}×", tool.speed)]
    })
}

#[cfg(test)]
mod tests;
