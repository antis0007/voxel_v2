use super::tests::uniform;
use super::*;
use crate::world::ChunkData;
use engine_core::VoxelScalePreset;
use std::sync::Arc;

fn add_chunk(world: &mut World, coord: IVec3, material: B) {
    world.chunks.insert(
        coord,
        Arc::new(ChunkData {
            modified_bounds: None,
            structural_bounds: None,
            blocks: Box::new([material; 32 * 32 * 32]),
            light: Box::new([0; 32 * 32 * 32]),
            torches: HashSet::new(),
        }),
    );
}

fn drain_events(world: &mut World) {
    world.environment.due.clear();
    world.environment.scheduled.clear();
}

fn stored_water(world: &World) -> u64 {
    world.environment.patches.values().map(|p| p.soil.water).sum()
}

#[test]
fn environment_full_patch_table_never_blocks_soil_edits() {
    let mut w = uniform(VoxelScalePreset::OneMeter, B::Dirt);
    add_chunk(&mut w, IVec3::X, B::Dirt);
    add_chunk(&mut w, IVec3::X * 2, B::Dirt);
    for i in 0..70_000 {
        // The service drains events in play; this isolates the patch table.
        if i % 1_000 == 0 {
            drain_events(&mut w);
        }
        let p = IVec3::new(i % 96, i / (96 * 32), (i / 96) % 32);
        w.set(p, B::Air);
        assert_eq!(w.get(p), B::Air, "dig {i} at {p}");
        w.set(p, B::Dirt);
        assert_eq!(w.get(p), B::Dirt, "place {i} at {p}");
        assert!(w.environment.patches.len() <= env::MAX_PATCHES);
    }
    assert_eq!(w.environment.patches.len(), env::MAX_PATCHES);
    assert_eq!(w.environment.ledger.initial_soil_water, stored_water(&w));

    let fresh = IVec3::new(7, 27, 7);
    let mut blocks = HashSet::new();
    let mut chunks = HashSet::new();
    assert!(!soil::process(&mut w, fresh, 1, &mut blocks, &mut chunks), "a full table must not repeat");
    assert!(!w.environment.patches.contains_key(&env::key(fresh)));
    drain_events(&mut w);
    w.set(fresh, B::Air);
    assert_eq!(w.get(fresh), B::Air);

    // Dug and refilled cells hold exactly their natural reserve: all redundant.
    let sample = env::key(IVec3::new(5, 3, 9));
    let kept = w.environment.patches[&sample].clone();
    drain_events(&mut w);
    for _ in 0..2_000 {
        prune::sweep(&mut w, IVec3::splat(10_000));
    }
    assert!(w.environment.patches.len() < prune::PRESSURE);
    assert_eq!(w.environment.ledger.initial_soil_water, stored_water(&w));
    assert!(!w.environment.patches.contains_key(&sample));
    assert!(ensure_patch(&mut w, env::position(sample)));
    let again = &w.environment.patches[&sample];
    assert_eq!((again.soil, again.species, again.material), (kept.soil, kept.species, kept.material));
    assert_eq!(w.environment.ledger.initial_soil_water, stored_water(&w));
}

#[test]
fn environment_pruning_keeps_non_default_patches_in_the_save() {
    let mut w = uniform(VoxelScalePreset::OneMeter, B::Stone);
    add_chunk(&mut w, IVec3::X * 3, B::Stone);
    let edited = IVec3::new(4, 8, 4);
    let near_residue = IVec3::new(6, 8, 4);
    let inert = IVec3::new(8, 8, 4);
    let pristine = IVec3::new(10, 8, 4);
    let far_residue = IVec3::new(110, 8, 4);
    w.set(edited, B::Dirt);
    w.set(pristine, B::Dirt);
    for p in [near_residue, inert, far_residue] {
        w.set(p, B::Air);
    }
    let base = w.environment.patches[&env::key(edited)].clone();
    let patch = |soil| env::Patch { soil, ..base.clone() };
    let residue = engine_sim::environment::Soil { nutrient: 160, flux_carry: 3, ..Default::default() };
    w.environment.patches.get_mut(&env::key(edited)).unwrap().soil.water = 1_234;
    w.environment.patches.get_mut(&env::key(edited)).unwrap().soil.nutrient = 50;
    w.environment.patches.insert(env::key(near_residue), patch(residue));
    w.environment.patches.insert(env::key(far_residue), patch(residue));
    w.environment.patches.insert(env::key(inert), patch(Default::default()));
    let natural = w.environment.fresh_patch(w.sample_config(), pristine, B::Dirt, false, true).unwrap();
    assert!(natural.soil.water > 0);
    w.environment.ledger.initial_soil_water += natural.soil.water;
    w.environment.patches.insert(env::key(pristine), natural.clone());
    // Off-chunk filler raises the table to sweep pressure and is never touched.
    for i in 0..prune::PRESSURE as i32 {
        let soil = engine_sim::environment::Soil { water: 7, ..Default::default() };
        w.environment.patches.insert((i, 5_000, 0), patch(soil));
    }
    drain_events(&mut w);

    let encode = |w: &World| {
        let mut bytes = Vec::new();
        w.environment.encode(&mut bytes);
        bytes
    };
    let decode = |bytes: &[u8]| {
        let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        State::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), VoxelScalePreset::OneMeter)
            .unwrap()
    };
    let before = decode(&encode(&w));
    let ledger = w.environment.ledger.initial_soil_water;
    for _ in 0..2 * (prune::PRESSURE / 64 + 4) {
        prune::sweep(&mut w, near_residue);
    }
    let bytes = encode(&w);
    let after = decode(&bytes);
    let mut again = Vec::new();
    after.encode(&mut again);
    assert_eq!(bytes, again);

    let dropped: Vec<_> = before.patches.keys().filter(|k| !after.patches.contains_key(k)).copied().collect();
    assert_eq!(dropped, vec![env::key(inert), env::key(pristine), env::key(far_residue)]);
    for (k, p) in &after.patches {
        let q = &before.patches[k];
        assert_eq!(
            (p.material, p.species, p.revision, p.last_tick, p.extent, p.soil),
            (q.material, q.species, q.revision, q.last_tick, q.extent, q.soil)
        );
    }
    assert_eq!(after.ledger.initial_soil_water, ledger - natural.soil.water);
    assert!(ensure_patch(&mut w, pristine));
    assert_eq!(w.environment.patches[&env::key(pristine)].soil, natural.soil);
    assert_eq!(w.environment.ledger.initial_soil_water, ledger);
}
