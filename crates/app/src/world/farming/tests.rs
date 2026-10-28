use super::*;
use crate::world::{ChunkData, WorldType, inventory::ItemStack};
use engine_core::VoxelScalePreset;
use std::sync::Arc;

const CELL: IVec3 = IVec3::new(16, 9, 16);

/// One resident chunk: dirt below a mycelium bed at y = 8, air above.
fn bed_world(scale: VoxelScalePreset) -> World {
    let mut w = World::with_seed(WorldType::Superflat, scale, 157);
    let mut blocks = Box::new([B::Air; 32 * 32 * 32]);
    for y in 0..9 {
        for z in 0..32 {
            for x in 0..32 {
                blocks[ChunkData::local_index(x, y, z)] = if y < 8 { B::Dirt } else { BED };
            }
        }
    }
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
        modified_bounds: None,
        structural_bounds: None,
        blocks,
        light: Box::new([0; 32 * 32 * 32]),
        torches: HashSet::new(),
    }));
    w
}

fn put(w: &mut World, p: IVec3, b: B, light: u8) {
    let mut c = w.chunks.get_mut(&IVec3::ZERO).expect("resident chunk");
    let c = Arc::make_mut(&mut c);
    c.blocks[ChunkData::local_index(p.x, p.y, p.z)] = b;
    c.light[ChunkData::local_index(p.x, p.y, p.z)] = light;
}

fn eye(w: &World, p: IVec3) -> Vec3 {
    (p.as_vec3() + Vec3::new(0.5, 1.5, 2.5)) * w.voxel_scale.meters()
}

fn sow(w: &mut World, p: IVec3, seed: B) {
    w.inventory.hotbar[0] = Some(ItemStack::new(Item::Block(seed), 4));
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let at = eye(w, p);
    assert!(interact(w, Some((p - IVec3::Y, p)), at, Vec3::NEG_Z, 0, &mut blocks, &mut chunks));
    assert!(w.farming.plots.contains_key(&key(p)), "no plot at {p:?}");
    assert_eq!(w.inventory.count_item(Item::Block(seed)), 3);
}

fn run(w: &mut World, ticks: u64) -> Vec<u8> {
    let mut stages = Vec::new();
    for _ in 0..ticks {
        tick(w, Budget::UNBOUNDED);
        if w.farming.clock.is_multiple_of(2400) {
            stages.push(w.farming.plots.get(&key(CELL)).map_or(u8::MAX, |p| p.stage));
        }
    }
    stages
}

fn total_growth(crop: &table::Crop, c: Conditions, rolls: u64) -> u32 {
    (0..256).map(|i| {
        let p = IVec3::new(i % 16, 9, i / 16);
        let mut plot = Plot::new(0, 0);
        stage::advance(&mut plot, crop, 157, p, rolls * u64::from(crop.roll_ticks), c, u32::MAX);
        u32::from(plot.stage)
    }).sum()
}

#[test]
fn farming_growth_follows_light_and_water() {
    let (red, _) = table::by_seed(B::RedCap).map(|(i, c)| (c, i)).expect("ember row");
    let (glow, _) = table::by_seed(B::GlowCap).map(|(i, c)| (c, i)).expect("luminous row");
    let sky = Conditions { light: 15, wet: false, soil: true };
    let wet = Conditions { wet: true, ..sky };
    let dark = Conditions { light: 0, ..sky };
    let dry = total_growth(red, sky, 12);
    assert!(dry > 0);
    assert!(total_growth(red, wet, 12) > dry + dry / 2, "water should speed growth");
    assert_eq!(total_growth(red, dark, 48), 0, "ember caps need light");
    assert_eq!(total_growth(glow, sky, 48), 0, "luminous caps need shade");
    assert!(total_growth(glow, dark, 12) > 0);
    assert_eq!(total_growth(red, Conditions { soil: false, ..wet }, 48), 0);
}

#[test]
fn farming_conditions_read_sky_torch_and_water() {
    let mut w = bed_world(VoxelScalePreset::OneMeter);
    let (_, red) = table::by_seed(B::RedCap).expect("row");
    assert_eq!(conditions(&w, CELL, red), Some(Conditions { light: 15, wet: false, soil: true }));
    put(&mut w, CELL + IVec3::Y * 3, B::Stone, 0);
    put(&mut w, CELL, B::Air, 10);
    put(&mut w, CELL + IVec3::new(3, 0, 0), B::Water, 0);
    assert_eq!(conditions(&w, CELL, red), Some(Conditions { light: 10, wet: true, soil: true }));
}

#[test]
fn farming_growth_is_identical_across_runs_and_swaps_at_maturity() {
    let runs: Vec<_> = (0..2).map(|_| {
        let mut w = bed_world(VoxelScalePreset::OneMeter);
        put(&mut w, CELL + IVec3::new(2, 0, 1), B::Water, 0);
        sow(&mut w, CELL, B::RedCap);
        assert_eq!(w.get(CELL), B::MushroomStem);
        let stages = run(&mut w, 3 * 57_600);
        (stages, w.get(CELL), w.farming.plots.get(&key(CELL)).copied())
    }).collect();
    assert_eq!(runs[0], runs[1]);
    let (stages, block, plot) = &runs[0];
    assert!(stages.windows(2).all(|s| s[0] <= s[1]), "stages never go back: {stages:?}");
    let plot = plot.expect("plot survives");
    assert_eq!(plot.stage, 3, "a watered, lit ember cap matures in three days: {stages:?}");
    assert_eq!(*block, B::RedCap);
}

#[test]
fn farming_catch_up_after_unload_equals_continuous_growth() {
    for (split, total) in [(10_000, 40_000), (1, 90_000), (30_000, 30_001)] {
        let mut a = bed_world(VoxelScalePreset::HalfMeter);
        let mut b = bed_world(VoxelScalePreset::HalfMeter);
        for w in [&mut a, &mut b] {
            put(w, CELL + IVec3::new(-3, 0, 2), B::Water, 0);
            sow(w, CELL, B::RedCap);
        }
        run(&mut a, total);
        run(&mut b, split);
        let chunk = b.chunks.get(&IVec3::ZERO).cloned().expect("resident");
        b.chunks.remove(&IVec3::ZERO);
        let frozen = b.farming.plots[&key(CELL)];
        run(&mut b, total - split - 1);
        assert_eq!(b.farming.plots[&key(CELL)], frozen, "an unloaded plot does not simulate");
        b.chunks.insert(IVec3::ZERO, chunk);
        run(&mut b, 1);
        assert_eq!(a.farming.plots[&key(CELL)], b.farming.plots[&key(CELL)], "split {split}");
        assert_eq!(a.get(CELL), b.get(CELL));
    }
}

#[test]
fn farming_harvest_yields_produce_and_seed() {
    let mut w = bed_world(VoxelScalePreset::OneMeter);
    let (index, red) = table::by_seed(B::RedCap).expect("row");
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    sow(&mut w, CELL, B::RedCap);
    assert_eq!(harvest(&mut w, CELL, &mut blocks, &mut chunks), Some(true));
    assert_eq!(w.get(CELL), B::Air);
    assert_eq!(w.inventory.count_item(Item::Block(B::RedCap)), 4, "an unripe plot returns its seed");
    sow(&mut w, CELL, B::RedCap);
    w.farming.plots.insert(key(CELL), Plot { crop: index, planted: 0, rolled: 30, stage: red.mature() });
    tick(&mut w, Budget::UNBOUNDED);
    assert_eq!(w.get(CELL), B::RedCap, "a mature plot shows the crop");
    let before = w.inventory.count_item(Item::Block(B::RedCap));
    let expected = harvest_count(w.seed, CELL, &w.farming.plots[&key(CELL)], red);
    assert!((red.yield_min..=red.yield_max).contains(&expected));
    assert!(super::super::expedition::harvest(&mut w, CELL, &mut blocks, &mut chunks));
    assert_eq!(w.inventory.count_item(Item::Block(B::RedCap)), before + expected);
    assert!(w.farming.plots.is_empty());
    assert_eq!(harvest(&mut w, CELL, &mut blocks, &mut chunks), None);
}

#[test]
fn farming_tills_with_a_dig_tool_and_prunes_broken_plots() {
    let mut w = bed_world(VoxelScalePreset::OneMeter);
    let ground = IVec3::new(10, 8, 10);
    put(&mut w, ground, B::Grass, 0);
    w.inventory.hotbar[1] = Some(ItemStack::new(Item::Tool(1), 1));
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let at = eye(&w, ground);
    assert!(interact(&mut w, Some((ground, ground + IVec3::Y)), at, Vec3::NEG_Z, 1, &mut blocks, &mut chunks));
    assert_eq!(w.get(ground), BED);
    sow(&mut w, CELL, B::GlowCap);
    put(&mut w, CELL, B::Air, 0);
    run(&mut w, 2);
    assert!(w.farming.plots.is_empty(), "a removed sprout drops its plot");
}

#[test]
fn farming_wild_seeds_scale_with_cell_volume() {
    let rate = |scale| {
        let w = World::with_seed(WorldType::Superflat, scale, 157);
        (0..4000).filter(|&i| wild_seed(&w, IVec3::new(i, 3, -i), B::RedCap).is_some()).count()
    };
    let whole = rate(VoxelScalePreset::OneMeter);
    assert!((1600..2400).contains(&whole), "{whole}");
    assert!(rate(VoxelScalePreset::QuarterMeter) < 100);
    let w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 157);
    assert_eq!(wild_seed(&w, IVec3::ZERO, B::Stone), None);
}

#[test]
fn farming_sidecar_round_trips_and_rejects_damage() {
    let mut w = bed_world(VoxelScalePreset::OneMeter);
    sow(&mut w, CELL, B::RedCap);
    sow(&mut w, CELL + IVec3::X * 2, B::GlowCap);
    run(&mut w, 5000);
    let bytes = sidecar::encode(&w);
    let (state, herd) = sidecar::decode(&bytes).expect("decodes");
    assert_eq!(state.clock, w.farming.clock);
    assert_eq!(state.plots, w.farming.plots);
    assert_eq!(herd, sidecar::herd_bytes(&w).as_slice());
    let mut bad = bytes.clone();
    bad[20] ^= 1;
    assert!(sidecar::decode(&bad).is_err());
    assert!(sidecar::decode(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn farming_clock_advances_once_per_adventure_step() {
    let mut w = bed_world(VoxelScalePreset::OneMeter);
    let mut player = crate::player::Player::new();
    player.pos = Vec3::new(16.5, 9.0, 20.5);
    for _ in 0..240 {
        crate::world::adventure::tick(&mut w, &mut player, crate::simulation::STEP_SECONDS, false, false, false);
    }
    assert_eq!(w.farming.clock, 240);
}
