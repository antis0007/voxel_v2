//! Claim rewards for Undercrown plinths, from `assets/undercrown_rewards.ron`.
use crate::world::inventory::{DroppedStack, Item, ItemStack};
use crate::world::World;
use engine_worldgen::undercrown::v2::props;
use engine_worldgen::undercrown::{ObjectId, Plan, PropKind, State, flags};
use glam::Vec3;
use std::sync::LazyLock;

const REWARDS_RON: &str = include_str!("../../../../../assets/undercrown_rewards.ron");

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Row {
    pub district: u8,
    pub room: u16,
    pub label: String,
    /// Prop index within the room; 1 is the room's original plinth,
    /// generator 302 strongboxes and reading stands use 2 and up.
    #[serde(default = "first_prop", skip_serializing_if = "is_first_prop")]
    pub prop: u8,
    #[serde(default)]
    pub items: Vec<(Item, u32)>,
    /// Spell added to the found-spell log on claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spell: Option<u32>,
    /// Tier of a found wand minted on claim, seeded by the prop's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wand: Option<u32>,
}

fn first_prop() -> u8 {
    1
}

fn is_first_prop(prop: &u8) -> bool {
    *prop == 1
}

static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| {
    ron::from_str(REWARDS_RON).unwrap_or_else(|error| panic!("assets/undercrown_rewards.ron: {error}"))
});

fn row(id: ObjectId) -> Option<&'static Row> {
    ROWS.iter().find(|r| r.district == id.district() && r.room == id.room() && r.prop == id.prop())
}

pub fn for_prop(id: ObjectId) -> &'static [(Item, u32)] {
    row(id).map_or(&[], |r| r.items.as_slice())
}

/// What a claim found, for the message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Granted {
    pub spell: bool,
    pub wand: bool,
}

/// Height of a shown reward over the prop point, clear of the plinth crystal.
const DISPLAY_LIFT: f32 = 0.9;

/// Rewards shown above unclaimed plinths, from generator 292 where plinths
/// have a cap to stand them on. Each plinth shows its first reward.
pub fn plinth_displays(plan: &Plan, state: &State) -> Vec<(Vec3, Item)> {
    if plan.v2.as_ref().is_none_or(|layout| layout.version < props::VERSION) {
        return Vec::new();
    }
    plan.props
        .iter()
        .filter(|p| p.kind == PropKind::Plinth && state.get(p.id) & flags::CLAIMED == 0)
        .filter_map(|p| for_prop(p.id).first().map(|&(item, _)| (p.pos + Vec3::Y * DISPLAY_LIFT, item)))
        .collect()
}

/// Reserve enough fallback drops before claiming, so a full pack and crowded
/// floor cannot permanently consume a puzzle reward without delivering it.
pub fn can_grant(world: &World, id: ObjectId) -> bool {
    let wand = row(id).is_some_and(|r| r.wand.is_some());
    world.inventory.dropped.len().saturating_add(for_prop(id).len() + usize::from(wand)) <= 1024
}

fn give(world: &mut World, item: Item, count: u32, at: Vec3) {
    let (inserted, remainder) = world.inventory.try_insert(ItemStack::new(item, count));
    if inserted > 0 {
        world.inventory.notify_pickup(item, inserted);
    }
    if remainder > 0 && world.inventory.dropped.len() < 1024 {
        world.inventory.dropped.push(DroppedStack { pos: at, stack: ItemStack::new(item, remainder) });
    }
}

/// Seed of a found wand: positional, from the prop's save identity, so one
/// world always finds the same wand in the same box.
fn wand_seed(id: ObjectId) -> u32 {
    engine_core::hash::hash_u32(0x5eed_0000 ^ (u32::from(id.district()) << 18) ^ (u32::from(id.room()) << 8) ^ u32::from(id.prop()))
}

/// Each reward goes to the pack, or drops at `at` when the pack is full.
pub fn grant(world: &mut World, id: ObjectId, at: Vec3) -> Granted {
    let mut granted = Granted::default();
    for &(item, count) in for_prop(id) {
        give(world, item, count, at);
    }
    let Some(row) = row(id) else {
        return granted;
    };
    if let Some(spell) = row.spell {
        granted.spell = world.adventure.spell_log.add(spell);
    }
    if let Some(tier) = row.wand {
        let record = crate::world::adventure::wand_items::generate(wand_seed(id), tier);
        if let Some(item) = world.adventure.mint_wand(record) {
            give(world, item, 1, at);
            granted.wand = true;
        }
    }
    granted
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_worldgen::{WorldSampleConfig, WorldType};
    use engine_core::VoxelScalePreset;

    #[test]
    fn undercrown_rewards_ron_round_trips_and_covers_every_plinth() {
        let back: Vec<Row> = ron::from_str(&ron::to_string(&*ROWS).unwrap()).unwrap();
        assert_eq!(back.len(), ROWS.len());
        let cfg = WorldSampleConfig { planet: None, generator_version: 32, seed: 424242, world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter };
        let plan = Plan::for_config(&cfg);
        for prop in plan.props.iter().filter(|p| p.kind == PropKind::Plinth) {
            let row = ROWS.iter().find(|r| r.district == prop.id.district() && r.room == prop.id.room());
            let row = row.unwrap_or_else(|| panic!("no reward row for {}", prop.label));
            assert_eq!(row.label, prop.label);
            assert!(!row.items.is_empty());
        }
    }

    #[test]
    fn every_generator_302_find_has_a_row_that_gives_something() {
        let plan = plan_at(engine_worldgen::undercrown::v2::life::VERSION);
        let finds: Vec<_> = plan.props.iter().filter(|p| matches!(p.kind, PropKind::Cache | PropKind::Lectern)).collect();
        assert!(!finds.is_empty());
        for prop in finds {
            let row = row(prop.id).unwrap_or_else(|| panic!("no reward row for {}", prop.label));
            assert_eq!(row.label, prop.label);
            assert!(!row.items.is_empty() || row.spell.is_some() || row.wand.is_some(), "{} gives nothing", prop.label);
            if prop.kind == PropKind::Lectern {
                assert!(row.spell.is_some(), "{} teaches no spell", prop.label);
            }
        }
    }

    #[test]
    fn wand_seed_is_stable_per_prop() {
        let a = ObjectId::new(2, 3, 2).unwrap();
        let b = ObjectId::new(2, 4, 2).unwrap();
        assert_eq!(wand_seed(a), wand_seed(a));
        assert_ne!(wand_seed(a), wand_seed(b));
    }

    fn plan_at(version: u32) -> Plan {
        Plan::for_config(&WorldSampleConfig { planet: None, generator_version: version, seed: 424242, world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter })
    }

    #[test]
    fn unclaimed_plinths_show_their_reward() {
        let plan = plan_at(props::VERSION);
        let shown = plinth_displays(&plan, &State::default());
        let plinths = plan.props.iter().filter(|p| p.kind == PropKind::Plinth).count();
        assert!(plinths > 0);
        assert_eq!(shown.len(), plinths);
    }

    #[test]
    fn a_claimed_plinth_shows_nothing() {
        let plan = plan_at(props::VERSION);
        let mut state = State::default();
        let first = plan.props.iter().find(|p| p.kind == PropKind::Plinth).unwrap();
        let before = plinth_displays(&plan, &state).len();
        state.set(first.id, flags::CLAIMED);
        assert_eq!(plinth_displays(&plan, &state).len(), before - 1);
    }

    #[test]
    fn older_generators_show_no_rewards() {
        assert!(plinth_displays(&plan_at(props::VERSION - 1), &State::default()).is_empty());
    }
}
