use super::*;
use crate::world::WorldType;
use engine_core::VoxelScalePreset;
use std::collections::{HashSet, VecDeque};

fn world(coords: &[IVec3]) -> World {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    for &coord in coords {
        world.chunks.insert(coord, Arc::new(ChunkData {
            modified_bounds: None,
            structural_bounds: None,
            blocks: Box::new([BlockId::Air; 32768]),
            light: Box::new([0xB0; 32768]),
            torches: HashSet::new(),
        }));
    }
    world
}

// Previous cell-wise flood is retained as a differential correctness oracle.
fn reference(world: &mut World, start: IVec3, level: u8) {
    fn write(world: &mut World, pos: IVec3, level: u8) -> bool {
        let coord = chunk_coord_from_world_block(pos);
        let Some(mut data) = world.chunks.get_mut(&coord) else { return false; };
        let data = Arc::make_mut(&mut data);
        let index = ChunkData::local_index(pos.x, pos.y, pos.z);
        if data.blocks[index].is_opaque() || level <= data.light[index] & 15 { return false; }
        data.light[index] = (data.light[index] & 0xF0) | level;
        world.regions.mark_dirty(coord);
        true
    }
    let mut queue = VecDeque::new();
    if write(world, start, level) { queue.push_back((start, level)); }
    while let Some((p, level)) = queue.pop_front() {
        if level <= 1 { continue; }
        for d in [IVec3::X, -IVec3::X, IVec3::Y, -IVec3::Y, IVec3::Z, -IVec3::Z] {
            let p = p + d;
            let Some(data) = world.chunks.get(&chunk_coord_from_world_block(p)) else { continue; };
            let index = ChunkData::local_index(p.x, p.y, p.z);
            if !data.blocks[index].is_opaque() && level - 1 > data.light[index] & 15
                && write(world, p, level - 1) { queue.push_back((p, level - 1)); }
        }
    }
}

fn assert_same(actual: &World, expected: &World) {
    assert_eq!(actual.edited_resident_bytes(), expected.edited_resident_bytes());
    assert_eq!(actual.chunks.len(), expected.chunks.len());
    for (coord, data) in actual.chunks.iter() {
        let reference = &expected.chunks[coord];
        assert_eq!(data.blocks, reference.blocks);
        assert_eq!(data.light, reference.light, "chunk {coord:?}");
        assert_eq!(data.torches, reference.torches);
        assert_eq!(actual.regions.source_version(*coord).is_some(), expected.regions.source_version(*coord).is_some());
        assert!(data.light.iter().all(|level| level & 0xF0 == 0xB0));
    }
}

fn fixture(coords: &[IVec3]) -> (World, World) {
    let mut actual = world(coords);
    for &coord in coords {
        let mut data = actual.chunks.get_mut(&coord).unwrap();
        for y in 0..32 { for z in 0..32 { for x in 0..32 {
            if (x * 7 + y * 11 + z * 3) % 31 == 0 {
                Arc::make_mut(&mut data).blocks[ChunkData::local_index(x,y,z)] = BlockId::Stone;
            }
        } } }
    }
    let mut expected = world(&[]);
    for (&coord, data) in actual.chunks.iter() { expected.chunks.insert(coord, Arc::clone(data)); }
    (actual, expected)
}

#[test]
fn chunk_flood_matches_cell_flood_at_faces_corners_holes_and_negative_coordinates() {
    let coords: Vec<_> = (-2..=1).flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| IVec3::new(x,y,z))))
        .filter(|c| *c != IVec3::new(-1,0,0)).collect();
    let (mut actual, mut expected) = fixture(&coords);
    for p in [IVec3::new(-33,0,0), IVec3::new(-32,-1,31), IVec3::new(31,31,31),
        IVec3::new(0,0,0), IVec3::new(-48,16,16), IVec3::new(16,16,16),
        IVec3::new(32,32,32), IVec3::new(-1,-1,-1), IVec3::new(63,31,31)] {
        for w in [&mut actual, &mut expected] {
            if let Some(mut data) = w.chunks.get_mut(&chunk_coord_from_world_block(p)) {
                Arc::make_mut(&mut data).blocks[ChunkData::local_index(p.x,p.y,p.z)] = BlockId::Air;
            }
        }
        for level in [0, 1, 2, 7, 14, 15, 15] {
            propagate_light(&mut actual,p,level);
            reference(&mut expected,p,level);
            assert_same(&actual,&expected);
        }
    }
}

#[test]
fn chunk_flood_preserves_snapshots_and_unchanged_chunk_ownership() {
    let coord = IVec3::new(-1,2,-1);
    let p = coord*CHUNK_SIZE+IVec3::splat(16);
    let mut actual = world(&[coord, coord+IVec3::X]);
    let old = Arc::clone(&actual.chunks[&coord]);
    let neighbor = Arc::clone(&actual.chunks[&(coord+IVec3::X)]);
    let bytes = actual.edited_resident_bytes();
    actual.regions.mark_dirty(coord);
    let before_version = actual.regions.source_version(coord);
    propagate_light(&mut actual,p,14);
    assert_ne!(actual.regions.source_version(coord), before_version);
    assert_eq!(actual.edited_resident_bytes(),bytes);
    assert!(old.light.iter().all(|level| *level == 0xB0));
    assert!(!Arc::ptr_eq(&old,&actual.chunks[&coord]));
    assert!(Arc::ptr_eq(&neighbor,&actual.chunks[&(coord+IVec3::X)]));
    assert!(!actual.regions.source_version(coord+IVec3::X).is_some());
    let version = actual.regions.source_version(coord);
    let current = Arc::clone(&actual.chunks[&coord]);
    propagate_light(&mut actual,p,14);
    assert_eq!(actual.regions.source_version(coord), version);
    assert!(Arc::ptr_eq(&current,&actual.chunks[&coord]));
}

#[test]
fn opaque_and_zero_level_seeds_do_not_clone_or_dirty_chunks() {
    let coord = IVec3::new(-2,3,1);
    let p = coord*CHUNK_SIZE+IVec3::splat(16);
    let mut actual = world(&[coord]);
    {
        let mut data = actual.chunks.get_mut(&coord).unwrap();
        Arc::make_mut(&mut data).blocks[ChunkData::local_index(p.x,p.y,p.z)] = BlockId::Stone;
    }
    let before = Arc::clone(&actual.chunks[&coord]);
    propagate_light(&mut actual,p,15);
    propagate_light(&mut actual,p+IVec3::X,0);
    assert!(Arc::ptr_eq(&before,&actual.chunks[&coord]));
    assert_eq!(actual.regions.source_version(coord),None);
    assert!(before.light.iter().all(|level| *level == 0xB0));
}

#[test]
fn chunk_flood_relights_wall_and_lamp_edits_like_cell_flood() {
    let coords: Vec<_> = (-1..=1).flat_map(|x| (-1..=1).map(move |z| IVec3::new(x,0,z))).collect();
    let (mut actual, mut expected) = fixture(&coords);
    let lamps = [IVec3::new(-1,16,-1),IVec3::new(31,16,31),IVec3::new(3,16,1)];
    for world in [&mut actual,&mut expected] {
        for &p in &lamps {
            let mut data=world.chunks.get_mut(&chunk_coord_from_world_block(p)).unwrap();
            let data=Arc::make_mut(&mut data);
            data.blocks[ChunkData::local_index(p.x,p.y,p.z)] = BlockId::Torch;
            data.torches.insert(p);
        }
    }
    for (i, p) in lamps.into_iter().chain((0..24).map(|i| IVec3::new(i-12,16,i%4))).enumerate() {
        for world in [&mut actual,&mut expected] {
            let mut data=world.chunks.get_mut(&chunk_coord_from_world_block(p)).unwrap();
            let data=Arc::make_mut(&mut data);
            let block=if i%3==0 {BlockId::Torch} else if i%3==1 {BlockId::Stone} else {BlockId::Air};
            data.blocks[ChunkData::local_index(p.x,p.y,p.z)] = block;
            if block.is_light_source() {data.torches.insert(p);} else {data.torches.remove(&p);}
        }
        relight_bounds(&mut actual,p,p);
        let reach=IVec3::splat(REACH);
        zero_block_light(&mut expected,p-reach,p+reach);
        let scan_lo=p-reach*2; let scan_hi=p+reach*2;
        let sources: Vec<_>=expected.chunks.values().flat_map(|data|data.torches.iter().copied())
            .filter(|cell|cell.cmpge(scan_lo).all()&&cell.cmple(scan_hi).all()).collect();
        for p in sources {reference(&mut expected,p,14);}
        assert_same(&actual,&expected);
    }
}

#[test]
#[ignore = "wall-clock chunk flood comparison; run without concurrent builds"]
fn block_light_chunk_cost() {
    use std::time::Instant;
    let coords: Vec<_>=(-1..=1).flat_map(|x|(-1..=1).flat_map(move |y|(-1..=1).map(move |z|IVec3::new(x,y,z)))).collect();
    for (label,p) in [("center",IVec3::splat(16)),("face",IVec3::new(31,16,16)),("corner",IVec3::splat(31)),("boundary_maze",IVec3::new(31,16,31))] {
        let (mut actual,mut expected)=fixture(&coords);
        if label == "boundary_maze" {
            for w in [&mut actual, &mut expected] {
                for y in 10..=22 { for k in 25..=38 {
                    for (q,open) in [(IVec3::new(33,y,k), y==16&&k==28), (IVec3::new(k,y,33),y==18&&k==30)] {
                        let mut data = w.chunks.get_mut(&chunk_coord_from_world_block(q)).unwrap();
                        Arc::make_mut(&mut data).blocks[ChunkData::local_index(q.x,q.y,q.z)] = if open {BlockId::Air} else {BlockId::Stone};
                    }
                } }
            }
        }
        for w in [&mut actual, &mut expected] {
            let mut data = w.chunks.get_mut(&chunk_coord_from_world_block(p)).unwrap();
            Arc::make_mut(&mut data).blocks[ChunkData::local_index(p.x,p.y,p.z)] = BlockId::Air;
        }
        let mut old=Vec::new();let mut new=Vec::new();
        for i in 0..41 {
            for w in [&mut actual,&mut expected] {zero_block_light(w,p-IVec3::splat(REACH),p+IVec3::splat(REACH));}
            let mut run=|candidate:bool| {
                let start=Instant::now();
                if candidate {propagate_light(&mut actual,p,14);} else {reference(&mut expected,p,14);}
                let us=start.elapsed().as_secs_f64()*1e6;
                if i>0 {if candidate {new.push(us);} else {old.push(us);}}
            };
            if i%2==0 {run(false);run(true);} else {run(true);run(false);}
            assert_same(&actual,&expected);
        }
        old.sort_by(f64::total_cmp);new.sort_by(f64::total_cmp);
        eprintln!("[block-light-cost] case={label} reference_p50_us={:.2} reference_p95_us={:.2} chunk_p50_us={:.2} chunk_p95_us={:.2} ratio={:.3}",old[20],old[38],new[20],new[38],new[20]/old[20]);
    }
}
