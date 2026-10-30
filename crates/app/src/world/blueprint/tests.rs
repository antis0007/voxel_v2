use super::format::{self, MAGIC};
use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::{World, WorldType, chunk_coord_from_world_block};
use engine_core::Budget;
use std::collections::HashSet;
use std::sync::Arc;

const PALETTE: [BlockId; 8] = [
    BlockId::Air,
    BlockId::Stone,
    BlockId::Planks,
    BlockId::Cobblestone,
    BlockId::Sandstone,
    BlockId::Air,
    BlockId::StoneBricks,
    BlockId::Bookcase,
];

fn world() -> World {
    World::new(WorldType::Superflat, VoxelScalePreset::OneMeter)
}

fn pattern(p: IVec3) -> BlockId {
    let h = engine_core::hash::hash_u32((p.x as u32).wrapping_mul(73) ^ (p.y as u32).wrapping_mul(9151) ^ (p.z as u32).wrapping_mul(30_011));
    PALETTE[(h % PALETTE.len() as u32) as usize]
}

fn admit(w: &mut World, p: IVec3) -> bool {
    w.resident_edit(chunk_coord_from_world_block(p)).is_ok()
}

fn build(w: &mut World, region: Region) {
    for y in region.min.y..=region.max.y {
        for z in region.min.z..=region.max.z {
            for x in region.min.x..=region.max.x {
                let p = IVec3::new(x, y, z);
                assert!(admit(w, p));
                w.set(p, pattern(p - region.min));
            }
        }
    }
}

fn copy(w: &World, region: Region) -> Blueprint {
    let mut capture = Capture::new(w, region).expect("region fits");
    assert!(capture.step(w, Budget::UNBOUNDED, usize::MAX));
    capture.finish()
}

fn run(w: &mut World, job: &mut Job, max_cells: usize) -> usize {
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let mut prepare = admit;
    let mut sink = Sink { prepare: &mut prepare, blocks: &mut blocks, chunks: &mut chunks };
    let mut steps = 1;
    while !job.step(w, &mut sink, Budget::UNBOUNDED, max_cells) {
        steps += 1;
        assert!(steps < 1_000_000, "paste never finished");
    }
    steps
}

fn assert_matches(w: &World, origin: IVec3, bp: &Blueprint) {
    for i in 0..bp.volume() {
        let local = bp.local(i);
        assert_eq!(w.get(origin + local), bp.cells()[i], "cell {local:?} at origin {origin:?}");
    }
}

#[test]
fn copy_then_paste_reproduces_cells_across_chunk_borders() {
    let mut w = world();
    let source = Region::from_corners(IVec3::new(-20, 4, -37), IVec3::new(19, 9, 3));
    build(&mut w, source);
    let bp = Arc::new(copy(&w, source));
    assert_eq!(bp.size(), IVec3::new(40, 6, 41));
    assert!(bp.solid_count() > 0 && bp.solid_count() < bp.volume());
    for origin in [IVec3::new(-70, 20, -45), IVec3::new(1_000_010, 5, -1_000_020), IVec3::new(-2_000_017, 30, 2_000_000)] {
        assert_ne!(chunk_coord_from_world_block(origin), chunk_coord_from_world_block(origin + bp.size() - 1));
        let mut job = Job::paste(&w, bp.clone(), origin, false);
        run(&mut w, &mut job, usize::MAX);
        assert_eq!(job.skipped, 0);
        assert_matches(&w, origin, &bp);
        assert_eq!(copy(&w, Region::from_corners(origin, origin + bp.size() - 1)), *bp);
    }
}

#[test]
fn skipping_air_keeps_what_stands_under_the_blueprint() {
    let mut w = world();
    let origin = IVec3::new(-3, 6, -3);
    let mut bp = Blueprint::filled(IVec3::new(2, 1, 1), BlockId::Air).expect("size");
    bp.set(IVec3::new(1, 0, 0), BlockId::Planks);
    assert!(admit(&mut w, origin));
    w.set(origin, BlockId::Stone);
    let mut job = Job::paste(&w, Arc::new(bp), origin, true);
    run(&mut w, &mut job, usize::MAX);
    assert_eq!(w.get(origin), BlockId::Stone);
    assert_eq!(w.get(origin + IVec3::X), BlockId::Planks);
}

#[test]
fn rotating_four_times_and_mirroring_twice_are_identity() {
    let size = IVec3::new(5, 3, 7);
    let cells: Vec<_> = (0..105).map(|i| match i % 9 {
        0 => BlockId::TorchWallNorth,
        1 => BlockId::TorchWallWest,
        2 => BlockId::TorchWallSouth,
        3 => BlockId::TorchWallEast,
        4 => BlockId::Basalt,
        5 => BlockId::Torch,
        6 => BlockId::WeatheredBricks,
        _ => pattern(IVec3::splat(i)),
    }).collect();
    let bp = Blueprint::new(size, cells).expect("size");
    let once = bp.rotated();
    assert_eq!(once.size(), IVec3::new(7, 3, 5));
    assert_ne!(once, bp);
    assert_eq!(once.rotated().rotated().rotated(), bp);
    assert_ne!(bp.mirrored(), bp);
    assert_eq!(bp.mirrored().mirrored(), bp);
    // A north wall torch leans on the cell at +z; a quarter turn moves that support to -x.
    assert_eq!(rotate_block(BlockId::TorchWallNorth), BlockId::TorchWallEast);
    assert_eq!(mirror_block(BlockId::TorchWallWest), BlockId::TorchWallEast);
    for &b in engine_content::ALL_BLOCK_IDS {
        assert_eq!(rotate_block(rotate_block(rotate_block(rotate_block(b)))), b, "{b:?}");
        assert_eq!(mirror_block(mirror_block(b)), b, "{b:?}");
    }
}

#[test]
fn axis_variants_pair_by_name() {
    let named = |n: &str| engine_content::ALL_BLOCK_IDS.iter().copied().find(|b| format!("{b:?}") == n);
    for (a, b) in [("TimberBeamX", "TimberBeamZ"), ("OakDoorUpper", "OakDoorZUpper"), ("OakTrapdoorTopOpen", "OakTrapdoorZTopOpen")] {
        if let (Some(a), Some(b)) = (named(a), named(b)) {
            assert_eq!((rotate_block(a), rotate_block(b)), (b, a));
        }
    }
    assert_eq!(rotate_block(BlockId::Stone), BlockId::Stone);
    assert_eq!(rotate_block(BlockId::Torch), BlockId::Torch);
}

#[test]
fn rotation_moves_cells_with_their_support() {
    let mut bp = Blueprint::filled(IVec3::new(3, 1, 2), BlockId::Air).expect("size");
    bp.set(IVec3::new(0, 0, 1), BlockId::Stone);
    bp.set(IVec3::new(0, 0, 0), BlockId::TorchWallNorth);
    let r = bp.rotated();
    let torch = (0..r.volume()).map(|i| r.local(i)).find(|&p| r.get(p).is_torch()).expect("torch kept");
    let offset = r.get(torch).torch_support_offset().expect("wall torch");
    assert_eq!(r.get(torch + offset), BlockId::Stone);
}

#[test]
fn undo_restores_the_world_after_paste_and_cut() {
    use crate::world::edit_history::{Direction, EditHistory, LOCAL_CELLS};
    let mut w = world();
    let area = Region::from_corners(IVec3::new(30, -2, -6), IVec3::new(36, 3, 1));
    let before = copy(&w, area);
    let bp = Arc::new(Blueprint::new(area.size(), (0..area.volume()).map(|i| pattern(IVec3::splat(i as i32))).collect()).expect("size"));
    let mut history = EditHistory::new(LOCAL_CELLS);
    let mut job = Job::paste(&w, bp.clone(), area.min, false);
    run(&mut w, &mut job, usize::MAX);
    assert_matches(&w, area.min, &bp);
    assert!(history.push(&w, job.into_record()));

    let cut: Vec<_> = (0..bp.volume())
        .filter(|&i| bp.cells()[i] != BlockId::Air)
        .map(|i| Change { p: area.min + bp.local(i), old: bp.cells()[i], new: BlockId::Air })
        .collect();
    let mut job = Job::changes(&w, cut);
    run(&mut w, &mut job, usize::MAX);
    assert_eq!(copy(&w, area).solid_count(), 0);
    assert!(history.push(&w, job.into_record()));

    let reverse = |w: &mut World, history: &mut EditHistory, direction| {
        history.start(w, direction).expect("history entry");
        let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
        let mut prepare = admit;
        let mut sink = Sink { prepare: &mut prepare, blocks: &mut blocks, chunks: &mut chunks };
        while history.step(w, &mut sink, Budget::UNBOUNDED, 97).is_none() {}
    };
    for expected in [&*bp, &before] {
        reverse(&mut w, &mut history, Direction::Undo);
        assert_eq!(copy(&w, area), *expected);
    }
    assert_eq!(history.start(&w, Direction::Undo), Err("Undo History Empty"));
    reverse(&mut w, &mut history, Direction::Redo);
    assert_eq!(copy(&w, area), *bp);
}

#[test]
fn batched_paste_equals_immediate_paste() {
    let bp = Arc::new(Blueprint::new(IVec3::new(33, 4, 9), (0..33 * 4 * 9).map(|i| pattern(IVec3::new(i, 1, 2))).collect()).expect("size"));
    let origin = IVec3::new(-17, 3, 60);
    let area = Region::from_corners(origin, origin + bp.size() - 1);
    let (mut a, mut b) = (world(), world());
    let mut immediate = Job::paste(&a, bp.clone(), origin, false);
    assert_eq!(run(&mut a, &mut immediate, usize::MAX), 1);
    let mut batched = Job::paste(&b, bp.clone(), origin, false);
    assert!(run(&mut b, &mut batched, 7) > 100);
    assert_eq!(copy(&a, area), copy(&b, area));
    assert_eq!(immediate.record(), batched.record());
}

#[test]
fn file_format_round_trips_and_is_compact() {
    let mut bp = Blueprint::filled(IVec3::splat(64), BlockId::Air).expect("size");
    for y in 0..6 {
        for x in 10..20 {
            bp.set(IVec3::new(x, y, 30), if y == 5 { BlockId::Basalt } else { BlockId::StoneBricks });
            bp.set(IVec3::new(x, y, 31), BlockId::TorchWallSouth);
        }
    }
    let bytes = format::encode(&bp);
    assert!(bytes.len() < 1024, "{} bytes for a sparse 64^3", bytes.len());
    let decoded = format::decode(&bytes).expect("decodes");
    assert_eq!(decoded.blueprint, bp);
    assert_eq!(decoded.unknown, 0);
    assert_eq!(format::peek_size(&bytes), Some(IVec3::splat(64)));

    let noisy = Blueprint::new(IVec3::new(17, 5, 3), (0..255).map(|i| pattern(IVec3::splat(i))).collect()).expect("size");
    assert_eq!(format::decode(&format::encode(&noisy)).expect("decodes").blueprint, noisy);
}

#[test]
fn corrupt_and_oversize_files_are_rejected() {
    let bp = Blueprint::new(IVec3::new(4, 2, 3), (0..24).map(|i| pattern(IVec3::splat(i))).collect()).expect("size");
    let good = format::encode(&bp);
    for i in 0..good.len() {
        let mut bad = good.clone();
        bad[i] ^= 0x41;
        assert!(format::decode(&bad).is_err(), "flipped byte {i} accepted");
    }
    for len in 0..good.len() {
        assert!(format::decode(&good[..len]).is_err(), "truncated to {len} accepted");
    }
    let mut long = good.clone();
    long.resize(format::MAX_FILE_BYTES + 1, 0);
    assert!(format::decode(&long).is_err());

    // A valid checksum over an absurd size or a run past the volume.
    let reseal = |mut body: Vec<u8>| {
        let sum = format::fnv1a64(&body);
        body.extend_from_slice(&sum.to_le_bytes());
        body
    };
    let mut huge = good[..good.len() - 8].to_vec();
    huge[10..16].copy_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    assert!(format::decode(&reseal(huge)).is_err());
    let mut header = MAGIC.to_vec();
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&[2, 0, 2, 0, 2, 0, 1, 0, 3]);
    header.extend_from_slice(b"Air");
    header.extend_from_slice(&1u32.to_le_bytes());
    let mut overrun = header.clone();
    overrun.extend_from_slice(&[0, 9]);
    assert!(format::decode(&reseal(overrun)).is_err());
    let mut exact = header;
    exact.extend_from_slice(&[0, 8]);
    assert_eq!(format::decode(&reseal(exact)).expect("eight air cells").blueprint.solid_count(), 0);
}

#[test]
fn unknown_materials_load_as_air() {
    let bp = Blueprint::filled(IVec3::ONE, BlockId::Stone).expect("size");
    let mut bytes = format::encode(&bp);
    let at = bytes.windows(5).position(|w| w == b"Stone").expect("palette name");
    bytes[at..at + 5].copy_from_slice(b"Stomp");
    let body = bytes.len() - 8;
    let sum = format::fnv1a64(&bytes[..body]);
    bytes[body..].copy_from_slice(&sum.to_le_bytes());
    let decoded = format::decode(&bytes).expect("decodes");
    assert_eq!((decoded.unknown, decoded.blueprint.solid_count()), (1, 0));
}

#[test]
fn library_names_files_and_round_trips() {
    for bad in ["", "   ", "a/b", "..", "x.y", "CON", "com1", "lpt9", &"n".repeat(library::MAX_NAME + 1)] {
        assert_eq!(library::clean_name(bad), None, "{bad:?}");
    }
    assert_eq!(library::clean_name("  Tower_2 west "), Some("Tower_2 west".into()));
    assert_eq!(library::clean_name("Console"), Some("Console".into()));

    let dir = std::env::temp_dir().join(format!("voxel-blueprints-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(library::list(&dir).expect("missing folder").is_empty());
    let bp = Blueprint::new(IVec3::new(3, 2, 1), vec![BlockId::Stone, BlockId::Air, BlockId::Planks, BlockId::Air, BlockId::Torch, BlockId::Air]).expect("size");
    library::save(&dir, "Gate", &bp).expect("saves");
    library::save(&dir, "arch", &bp).expect("saves");
    let names: Vec<_> = library::list(&dir).expect("lists").into_iter().map(|e| (e.name, e.size)).collect();
    assert_eq!(names, vec![("arch".into(), Some(bp.size())), ("Gate".into(), Some(bp.size()))]);
    assert!(library::rename(&dir, "Gate", "arch").is_err());
    library::rename(&dir, "Gate", "Gatehouse").expect("renames");
    assert_eq!(library::load(&dir, "Gatehouse").expect("loads").blueprint, bp);
    library::delete(&dir, "arch").expect("deletes");
    assert!(library::load(&dir, "arch").is_err());
    std::fs::write(dir.join("junk.vxbp"), b"VXBLUEPRxx").expect("writes");
    assert!(library::load(&dir, "junk").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
