use super::*;
use crate::world::inventory::ItemStack;

const STEP: f32 = 1.0 / 120.0;

fn bronze_bore() -> Tool {
    Tool::of(Item::Tool(1)).expect("bronze bore row")
}

fn copper_sword() -> Tool {
    Tool::of(Item::Weapon(1)).expect("copper sword row")
}

#[test]
fn break_time_scales_with_hardness_and_tool_speed() {
    let hand = Tool::hand();
    let stone = survival_break(BlockId::Stone, hand).seconds;
    let dirt = survival_break(BlockId::Dirt, hand).seconds;
    let hardness = |b: BlockId| b.material().physics.hardness;
    let ratio = hardness(BlockId::Stone) / hardness(BlockId::Dirt);
    assert!((stone / dirt - ratio).abs() < 1e-4, "stone {stone} s, dirt {dirt} s");
    assert!(
        (stone - hardness(BlockId::Stone) * table().seconds_per_hardness).abs() < 1e-4,
        "hand speed is the unit"
    );
    let bore = bronze_bore();
    let bored = survival_break(BlockId::Stone, bore).seconds;
    assert!((stone / bored - bore.speed).abs() < 1e-3, "hand {stone} s, bore {bored} s");
}

#[test]
fn wrong_tool_class_is_slower() {
    let (bore, sword) = (bronze_bore(), copper_sword());
    assert!(survival_break(BlockId::Stone, sword).seconds > survival_break(BlockId::Stone, bore).seconds);
    assert!(survival_break(BlockId::Log, bore).seconds > survival_break(BlockId::Log, sword).seconds);
    assert!(survival_break(BlockId::Stone, sword).drops, "the wrong class still harvests tier 0");
}

#[test]
fn under_tier_takes_longer_and_drops_nothing() {
    let hand = survival_break(BlockId::IronOre, Tool::hand());
    assert!(!hand.drops);
    assert_eq!(hand.required_tier, 2);
    let unpenalised = BlockId::IronOre.material().physics.hardness * table().seconds_per_hardness;
    assert!((hand.seconds - unpenalised * table().under_tier_factor).abs() < 1e-3);
    assert!(!survival_break(BlockId::IronOre, copper_sword()).drops, "a blade is the wrong class");
    let bored = survival_break(BlockId::IronOre, bronze_bore());
    assert!(bored.drops && bored.seconds * 4.0 < hand.seconds);
}

#[test]
fn creative_breaks_instantly() {
    for block in [BlockId::Stone, BlockId::IronOre, BlockId::Basalt, BlockId::Log] {
        let creative = break_for(block, Tool::hand(), false);
        assert_eq!(creative.seconds, 0.0);
        assert!(creative.drops);
        assert!(break_for(block, Tool::hand(), true).seconds > 0.0);
    }
}

#[test]
fn progress_restarts_on_a_new_cell_or_release() {
    let needed = 0.5;
    let a = Some((IVec3::ZERO, BlockId::Stone));
    let b = Some((IVec3::X, BlockId::Stone));
    let mut progress = Progress::default();
    for _ in 0..30 {
        assert!(!progress.tick(a, true, needed, STEP));
    }
    assert!(progress.fraction().is_some_and(|f| f > 0.4));
    assert!(!progress.tick(b, true, needed, STEP));
    assert!(progress.fraction().is_some_and(|f| f < 0.1));
    assert!(!progress.tick(b, false, needed, STEP));
    assert_eq!(progress.fraction(), None);
    let ticks = (1..1000).find(|_| progress.tick(a, true, needed, STEP)).unwrap_or(0);
    assert_eq!(ticks, 60);
}

#[test]
fn held_row_wins_then_bore_kit_then_hand() {
    assert_eq!(tool_for(None, 0), Tool::hand());
    assert_eq!(tool_for(Some(Item::Block(BlockId::Stone)), 0), Tool::hand());
    assert_eq!(tool_for(Some(Item::Block(BlockId::Stone)), 1), bronze_bore());
    assert_eq!(tool_for(Some(Item::Weapon(1)), 2), copper_sword());
    assert_eq!(tool_for(None, 9), Tool::hand());
}

#[test]
fn table_rows_are_unique_and_kit_items_have_rows() {
    let t = table();
    for (i, row) in t.tools.iter().enumerate() {
        assert!(t.tools[..i].iter().all(|r| r.item != row.item), "{:?} listed twice", row.item);
        assert!(row.speed >= t.hand_speed, "{:?} is slower than the hand", row.item);
    }
    let mut seen = HashSet::new();
    for block in t.blocks.iter().flat_map(|rule| &rule.blocks) {
        assert!(seen.insert(*block), "{block:?} has two rules");
    }
    assert!(t.kit.iter().all(|&item| Tool::of(item).is_some()));
}

#[test]
fn tool_tooltips_carry_tier_and_speed() {
    assert_eq!(stat_lines(Item::Tool(1)), ["Tier: 2", "Mining Speed: 4.0×"]);
    assert!(stat_lines(Item::Block(BlockId::Stone)).is_empty());
}

#[test]
fn under_tier_harvest_removes_the_cell_without_a_drop() {
    let mut world = World::with_seed(
        crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter,
        1234,
    );
    let cell = IVec3::new(0, 100, 0);
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let iron = Item::Resource(2);
    for (tool, drops) in [(Tool::hand(), false), (bronze_bore(), true)] {
        world.set(cell, BlockId::IronOre);
        assert_eq!(world.get(cell), BlockId::IronOre);
        let before = (world.expedition.resources[2], world.inventory.count_item(iron));
        assert!(harvest(&mut world, cell, Some(tool), &mut blocks, &mut chunks));
        assert_eq!(world.get(cell), BlockId::Air);
        let after = (world.expedition.resources[2], world.inventory.count_item(iron));
        assert_eq!(after != before, drops, "{tool:?}: {before:?} -> {after:?}");
    }
    world.inventory.hotbar[0] = Some(ItemStack::new(Item::Tool(1), 1));
    assert_eq!(tool_for(world.inventory.hotbar[0].map(|s| s.item), 0), bronze_bore());
}

/// Serialising the loaded table reproduces the file byte for byte.
#[test]
fn mining_ron_round_trips() {
    let text = ron::ser::to_string_pretty(table(), ron::ser::PrettyConfig::new())
        .expect("serialise mining table")
        .replace('\r', "");
    let file = MINING_RON.replace('\r', "");
    let body = file.strip_prefix(&MINING_HEADER.replace('\r', "")).unwrap_or(&file);
    assert_eq!(
        text.trim_end(),
        body.trim_end(),
        "run `cargo test dump_mining_ron -- --ignored` to rewrite assets/mining.ron"
    );
}

/// Rewrites assets/mining.ron from the loaded table (keeps the header).
#[test]
#[ignore = "writes assets/mining.ron"]
fn dump_mining_ron() {
    let text = ron::ser::to_string_pretty(table(), ron::ser::PrettyConfig::new())
        .expect("serialise mining table");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mining.ron");
    std::fs::write(path, format!("{MINING_HEADER}{text}\n")).expect("write assets/mining.ron");
}
