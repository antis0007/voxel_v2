//! Pan scoops: a water-covered gravel or sand bed is washed for heavy minerals.
//!
//! Placer grade follows where dense grains stop: gravel bedload, a bed resting
//! on bedrock, shallow water and grains the cellular flow already settled.
//! Each scoop removes the bed cell, so a deposit depletes without new state.
use super::fishing::roll;
use super::{World, water_along, weighted};
use crate::world::{BlockId, block_data, effects, factory};
use crate::world::environment::key;
use crate::world::inventory::{Item, ItemStack, resources};
use glam::{IVec3, Vec3};

pub const PAN: Item = Item::Tool(18);
const REACH_METRES: f32 = 5.0;
/// An arm in the water reaches a bed this far below the surface.
const SCOOP_DEPTH_METRES: f32 = 1.2;
/// Gold districts are 128 cells wide: upstream lodes feed whole reaches.
const DISTRICT_SHIFT: i32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Row {
    pub item: Item,
    pub count: u32,
    pub weight: u32,
    /// Placer rows scale with the bed's grade; others are ordinary bed grains.
    pub placer: bool,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Table {
    pub empty: u32,
    pub rows: Vec<Row>,
}

const PANNING_RON: &str = include_str!("../../../../../assets/panning.ron");

static TABLE: std::sync::LazyLock<Table> = std::sync::LazyLock::new(|| {
    ron::from_str(PANNING_RON).unwrap_or_else(|error| {
        eprintln!("[panning] assets/panning.ron: {error}");
        Table::default()
    })
});

/// The washable bed under a water column and its depth in cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bed {
    pub cell: IVec3,
    pub block: BlockId,
    pub water_cells: i32,
}

pub(crate) fn ordinary_sediment(block: BlockId) -> bool {
    matches!(block, BlockId::Gravel | BlockId::Sand)
}

fn is_sediment(block: BlockId) -> bool {
    ordinary_sediment(block) || block == BlockId::Paydirt
}

pub(crate) fn wet_bed(world: &World, cell: IVec3, block: BlockId) -> Option<Bed> {
    if !is_sediment(block) {
        return None;
    }
    let limit = (SCOOP_DEPTH_METRES / world.voxel_scale.meters()).ceil() as i32;
    let water_cells = (1..=limit)
        .take_while(|&dy| world.get(cell + IVec3::Y * dy) == BlockId::Water)
        .count() as i32;
    (water_cells > 0).then_some(Bed { cell, block, water_cells })
}

pub(crate) fn feed(world: &World, bed: Bed) -> Option<(u32, factory::placer::Feed)> {
    let volume = factory::placer::cell_volume(factory::step(world));
    if bed.block == BlockId::Paydirt {
        let grade = factory::placer::PAYDIRT_GRADE;
        return Some((grade, factory::placer::Feed::from_grade(grade, volume)));
    }
    if world.block_data.placed_sediment.contains(&block_data::key(bed.cell))
        || engine_worldgen::sample_block(&world.sample_config(), bed.cell) != bed.block
    {
        return None;
    }
    let grade = grade(source_grade(world, bed.cell), site(world, bed), world.voxel_scale.meters());
    Some((grade, factory::placer::Feed::from_grade(grade, volume)))
}

/// Regional source grade in per mille; squared so most reaches pay little.
pub fn district_grade(seed: u32, cell: IVec3) -> u32 {
    let g = roll(seed, IVec3::new(cell.x >> DISTRICT_SHIFT, 0, cell.z >> DISTRICT_SHIFT), 0, 0x601d) % 1000;
    g * g / 1000
}

pub fn bed_below(world: &World, water: IVec3) -> Option<Bed> {
    let mut cell = water;
    let mut water_cells = 0;
    let max_cells = (SCOOP_DEPTH_METRES / world.voxel_scale.meters()).ceil().max(1.0) as i32;
    while world.get(cell) == BlockId::Water {
        water_cells += 1;
        if water_cells > max_cells {
            return None;
        }
        cell -= IVec3::Y;
    }
    let block = world.get(cell);
    is_sediment(block).then_some(Bed { cell, block, water_cells })
}

/// Source grade in per mille. In placer worlds the material carries its grade,
/// so moving gravel or returning washed tailings cannot create gold.
pub fn source_grade(world: &World, cell: IVec3) -> u32 {
    let cfg = world.sample_config();
    if material_grade(&cfg) {
        return 0;
    }
    let metres = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    engine_worldgen::placer::grade(&cfg, metres.x, metres.z)
        .map_or_else(|| district_grade(world.seed, cell), |g| (g * 1000.0) as u32)
}

fn material_grade(cfg: &engine_voxel::WorldSampleConfig) -> bool {
    engine_worldgen::planetary::active(cfg) && cfg.generator_version >= engine_voxel::PLACER_PLANET_GENERATOR
}

/// Local conditions at a scooped bed that decide where dense grains stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub bed: Bed,
    pub below: BlockId,
    /// Grains the cellular flow already settled on this bed (mg).
    pub settled_mg: u64,
    /// Admitted hydraulic work pending at the water cell (mJ).
    pub current_mj: u64,
    /// Solid horizontal neighbours of the water cell above the bed, 0..4.
    pub shelter: u8,
    /// Shelter faces that are rock rather than loose bed.
    pub rock: u8,
}

fn loose(b: BlockId) -> bool {
    matches!(b, BlockId::Gravel | BlockId::Sand | BlockId::Paydirt | BlockId::Dirt)
}

pub fn site(world: &World, bed: Bed) -> Site {
    let water = bed.cell + IVec3::Y;
    let (mut shelter, mut rock) = (0, 0);
    for d in [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z] {
        let b = world.get(water + d);
        if b.is_solid() {
            shelter += 1;
            rock += u8::from(!loose(b));
        }
    }
    Site {
        bed,
        below: world.get(bed.cell - IVec3::Y),
        settled_mg: world.environment.sediment.get(&key(water)).map_or(0, |s| s.bed),
        current_mj: world.environment.hydraulic_work.get(&key(water)).copied().unwrap_or(0),
        shelter,
        rock,
    }
}

/// Placer grade in per mille: source grade times trap factors, saturating
/// smoothly so rich ground still ranks below richer ground.
pub fn grade(source: u32, site: Site, metres: f32) -> u32 {
    let bed = match site.bed.block {
        BlockId::Gravel => 2000,
        BlockId::Paydirt => 3000,
        _ => 1000,
    };
    // Gold sinks through loose bed until bedrock stops it.
    let bedrock = if site.below.is_solid() && !loose(site.below) { 3000 } else { 1000 };
    let shallow = if site.bed.water_cells as f32 * metres < 1.0 { 2000 } else { 1000 };
    let settled = if site.settled_mg > 0 { 2000 } else { 1000 };
    // Pockets and the lee of rock are slack water; live current winnows them.
    let shelter = 1000 + 500 * u64::from(site.shelter) + 250 * u64::from(site.rock);
    let current = if site.current_mj > 0 { 500 } else { 1000 };
    let raw = [bed, bedrock, shallow, settled, shelter, current]
        .into_iter()
        .fold(u64::from(source) * 1000, |g, f| g * f / 1000)
        / 1000;
    (1000 * raw / (raw + 1000)) as u32
}

/// The haul for one scoop; the same inputs always give the same row.
pub fn haul(seed: u32, bed: Bed, grade: u32, clock: u64) -> Option<Row> {
    let table = &*TABLE;
    let weight = |r: &Row| if r.placer { r.weight * grade / 100 } else { r.weight };
    let total = table.empty + table.rows.iter().map(weight).sum::<u32>();
    if total == 0 {
        return None;
    }
    let pick = roll(seed, bed.cell, clock, 0x9a17) % total;
    weighted(&table.rows, weight, pick.checked_sub(table.empty)?)
}

pub(super) fn use_pan(world: &mut World, eye_m: Vec3, forward: Vec3) {
    let Some(bed) = water_along(world, eye_m, forward, REACH_METRES).and_then(|w| bed_below(world, w)) else {
        world.expedition.say("No Wet Gravel in Reach");
        return;
    };
    let Some((g, feed)) = feed(world, bed) else {
        world.expedition.say("No Native Sediment");
        return;
    };
    let row = haul(world.seed, bed, g, world.farming.clock)
        .map(|row| ItemStack::new(row.item, row.count * bonus(row, g)));
    let gold = feed.pan_recovery();
    let mut inventory = world.inventory.clone();
    if resources::collect(&mut inventory, factory::GOLD_CONCENTRATE, gold) != gold
        || row.is_some_and(|stack| inventory.try_insert(stack).1 > 0)
    {
        world.expedition.say("Inventory Full");
        return;
    }
    // Existing water flows into the excavated cell through the fluid solver.
    if effects::replace(world, bed.cell, bed.block, BlockId::Air).is_err() {
        world.expedition.say("Bed Unavailable");
        return;
    }
    // The terrain commit owns any newly spilled or recovered world stacks.
    inventory.dropped = std::mem::take(&mut world.inventory.dropped);
    inventory.overflow = std::mem::take(&mut world.inventory.overflow);
    world.inventory = inventory;
    world.sync_from_inventory();
    let colour = format!("Colour {g} ‰{}", trend(world.travel.last_colour, g));
    world.travel.last_colour = Some(g);
    if gold > 0 {
        world.expedition.say(format!("Gold Concentrate {:.2} · {colour}", gold as f32 / 64.0));
    } else if let Some(stack) = row {
        world.expedition.say(format!("{} × {} · {colour}", stack.item.name(), stack.count));
    } else {
        world.expedition.say(format!("Pan Empty · {colour}"));
    }
}

/// Direction marker against the previous scoop, so colour can be followed upstream.
pub fn trend(last: Option<u32>, now: u32) -> &'static str {
    match last {
        Some(l) if now * 10 > l * 11 => " ▲",
        Some(l) if now * 10 < l * 9 => " ▼",
        Some(_) => " ·",
        None => "",
    }
}

/// Rich beds wash out several grains of a placer mineral at once.
pub fn bonus(row: Row, grade: u32) -> u32 {
    if !row.placer {
        return 1;
    }
    1 + u32::from(grade >= 700) + u32::from(grade >= 900)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panning_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&*TABLE, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = PANNING_RON.replace('\r', "");
        let body = file.find("\n(\n").map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end(), "rerun dump_panning_ron");
        assert!(TABLE.rows.iter().any(|r| r.placer));
    }

    #[test]
    #[ignore = "writes assets/panning.ron"]
    fn dump_panning_ron() {
        let header: String = PANNING_RON.lines().take_while(|l| l.starts_with("//")).map(|l| format!("{l}\n")).collect();
        let text = ron::ser::to_string_pretty(&*TABLE, ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/panning.ron");
        std::fs::write(path, format!("{header}{text}\n")).unwrap();
    }

    fn bed(block: BlockId, water_cells: i32) -> Bed {
        Bed { cell: IVec3::new(40, 10, -90), block, water_cells }
    }

    fn at(block: BlockId, water_cells: i32, below: BlockId) -> Site {
        Site { bed: bed(block, water_cells), below, settled_mg: 0, current_mj: 0, shelter: 0, rock: 0 }
    }

    #[test]
    fn placer_traps_raise_grade() {
        let g = |s: Site| grade(60, s, 0.5);
        let sand_deep = g(at(BlockId::Sand, 6, BlockId::Dirt));
        let gravel = g(at(BlockId::Gravel, 6, BlockId::Dirt));
        let paydirt = g(at(BlockId::Paydirt, 6, BlockId::Dirt));
        let on_rock = g(at(BlockId::Gravel, 6, BlockId::Stone));
        let shallow = g(at(BlockId::Gravel, 1, BlockId::Stone));
        assert!(sand_deep < gravel && gravel < paydirt && gravel < on_rock && on_rock < shallow,
            "{sand_deep} {gravel} {paydirt} {on_rock} {shallow}");
        let base = at(BlockId::Gravel, 1, BlockId::Stone);
        assert!(g(Site { settled_mg: 5, ..base }) > shallow);
        let pocket = g(Site { shelter: 2, ..base });
        assert!(pocket > shallow && g(Site { shelter: 2, rock: 2, ..base }) > pocket);
        assert!(g(Site { current_mj: 40, ..base }) < shallow, "live current winnows the bed");
    }

    #[test]
    fn trend_and_bonus_follow_grade() {
        assert_eq!(trend(None, 300), "");
        assert_eq!(trend(Some(300), 400), " ▲");
        assert_eq!(trend(Some(300), 200), " ▼");
        assert_eq!(trend(Some(300), 310), " ·");
        let gold = Row { item: Item::Resource(0), count: 1, weight: 1, placer: true };
        assert_eq!((bonus(gold, 100), bonus(gold, 750), bonus(gold, 950)), (1, 2, 3));
        assert_eq!(bonus(Row { placer: false, ..gold }, 950), 1);
    }

    #[test]
    fn grade_saturates_without_ties() {
        let rich = Site { settled_mg: 1, shelter: 4, rock: 4, ..at(BlockId::Paydirt, 1, BlockId::Stone) };
        let (a, b) = (grade(400, rich, 0.5), grade(800, rich, 0.5));
        assert!(a < b && b < 1000, "{a} {b}");
        assert_eq!(grade(0, rich, 0.5), 0);
    }

    #[test]
    fn rich_beds_pay_more_placer_rows() {
        let count = |g: u32| {
            (0..2000u64).filter(|&t| haul(9, bed(BlockId::Gravel, 1), g, t).is_some_and(|r| r.placer)).count()
        };
        assert_eq!(count(0), 0, "barren beds hold no placer minerals");
        assert!(count(1000) > count(100) * 3, "{} vs {}", count(1000), count(100));
        assert_eq!(haul(9, bed(BlockId::Gravel, 1), 400, 77), haul(9, bed(BlockId::Gravel, 1), 400, 77));
    }

    #[test]
    fn most_districts_are_poor() {
        let rich = (0..400).filter(|&i| district_grade(3, IVec3::new(i << DISTRICT_SHIFT, 0, 0)) > 500).count();
        assert!((40..160).contains(&rich), "{rich}");
    }
}
