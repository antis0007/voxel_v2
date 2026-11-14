use super::*;
use crate::mesh::{GeneratedChunk, PADDED_CHUNK_SIZE};
use crate::world::CHUNK_SIZE;

fn chunk(coord: IVec3, fill: impl Fn(IVec3) -> BlockId) -> GeneratedChunk {
    let mut c = GeneratedChunk::new(coord, vec![BlockId::Air; (PADDED_CHUNK_SIZE as usize).pow(3)]);
    let base = coord * CHUNK_SIZE;
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let p = base + IVec3::new(x, y, z);
                c.set_world(p, fill(p));
            }
        }
    }
    c
}

#[test]
fn window_snaps_to_bricks_on_both_sides_of_zero() {
    assert_eq!(probe_origin(Vec3::new(0.2, 0.0, 7.9)), IVec3::new(-32, -32, -32));
    assert_eq!(probe_origin(Vec3::new(-0.5, -8.0, -8.1)), IVec3::new(-40, -40, -48));
    let far = probe_origin(Vec3::new(1_000_003.0, 5.0, -2_000_001.0));
    assert_eq!(far.x.rem_euclid(BRICK), 0);
    assert_eq!(far.z.rem_euclid(BRICK), 0);
    assert!(far.x <= 1_000_003 - PROBE_DIM / 2 + BRICK && far.x + PROBE_DIM / 2 > 1_000_003 - BRICK);
}

#[test]
fn stamp_separates_leaves_from_solid_and_absent_chunks_read_as_air() {
    for coord in [IVec3::ZERO, IVec3::new(-1, -1, -1), IVec3::new(31_250, 0, -62_500)] {
        let base = coord * CHUNK_SIZE;
        let c = chunk(coord, |p| match (p - base).y {
            0 => BlockId::Stone,
            1 => BlockId::Leaves,
            _ => BlockId::Air,
        });
        let mut w = Window::new(base - IVec3::splat(8), CHUNK_SIZE);
        w.stamp(coord, Some(&c));
        assert!(w.get(base + IVec3::new(3, 0, 5)) == Occupancy::Solid);
        assert!(w.get(base + IVec3::new(3, 1, 5)) == Occupancy::Leaf);
        assert!(w.get(base + IVec3::new(3, 2, 5)) == Occupancy::Air);
        // Outside the window and an absent chunk both read as air.
        assert!(w.get(base - IVec3::splat(9)) == Occupancy::Air);
        w.stamp(coord, None::<&GeneratedChunk>);
        assert!(w.get(base + IVec3::new(3, 0, 5)) == Occupancy::Air);
    }
}

#[test]
fn a_changed_chunk_rebakes_only_bricks_its_rays_reach() {
    let eye = Vec3::new(16.0, 16.0, 56.0);
    let origin = probe_origin(eye);
    let ground = chunk(IVec3::ZERO, |p| if p.y < 4 { BlockId::Stone } else { BlockId::Air });
    let mut s = Schedule::new(origin, CHUNK_SIZE);
    let all = s.pending.len();
    assert_eq!(all, 512);
    s.refresh(origin, |c| (c == IVec3::ZERO).then_some((&ground, 1)));
    s.take_batch(eye, usize::MAX);
    assert!(!s.refresh(origin, |c| (c == IVec3::ZERO).then_some((&ground, 1))));
    assert!(s.pending.is_empty(), "an unchanged revision must not rebake");

    let edited = chunk(IVec3::ZERO, |p| if p.y < 5 { BlockId::Stone } else { BlockId::Air });
    assert!(s.refresh(origin, |c| (c == IVec3::ZERO).then_some((&edited, 2))));
    assert!(!s.pending.is_empty() && s.pending.len() < all);
    for b in &s.pending {
        let lo = *b * BRICK;
        let near = (lo - IVec3::splat(REACH)).cmplt(IVec3::splat(CHUNK_SIZE)).all()
            && (lo + IVec3::splat(BRICK + REACH)).cmpgt(IVec3::ZERO).all();
        assert!(near, "brick {b} is beyond ray reach of the edit");
    }
}

#[test]
fn moving_one_brick_clears_one_slab_and_keeps_far_bricks_baked() {
    let eye = Vec3::new(-3.0, 10.0, 4.0);
    let mut s = Schedule::new(probe_origin(eye), CHUNK_SIZE);
    s.refresh(s.probe_origin, |_| None::<(&GeneratedChunk, u64)>);
    s.cleared.clear();
    s.take_batch(eye, usize::MAX);
    let moved = eye + Vec3::new(BRICK as f32, 0.0, 0.0);
    assert!(s.refresh(probe_origin(moved), |_| None::<(&GeneratedChunk, u64)>));
    let slab = (PROBE_DIM / BRICK).pow(2) as usize;
    assert_eq!(s.cleared.len(), slab);
    // The occupancy margin equals the ray reach, so kept bricks traced only
    // cells the old window already held: just the entering slab is rebaked.
    assert_eq!(s.pending.len(), slab);
    let batch = s.take_batch(moved, 4);
    assert_eq!(batch.len(), 4);
    for b in &batch {
        assert!(s.pending.iter().all(|p| {
            let d = |q: &IVec3| ((*q * BRICK).as_vec3() + Vec3::splat(4.0) - moved).length_squared();
            d(p) >= d(b)
        }));
    }
}

#[test]
fn a_moved_window_matches_a_fresh_stamp_cell_for_cell() {
    let terrain = |coord: IVec3| {
        chunk(coord, |p| {
            if p.y < (p.x * 3 + p.z * 5).rem_euclid(7) - 2 {
                BlockId::Stone
            } else if (p.x + p.z).rem_euclid(5) == 0 && p.y < 6 {
                BlockId::Leaves
            } else {
                BlockId::Air
            }
        })
    };
    let chunks: HashMap<IVec3, GeneratedChunk> = (-3..=3)
        .flat_map(|y| (-4..=4).flat_map(move |z| (-4..=4).map(move |x| IVec3::new(x, y, z))))
        .map(|c| (c, terrain(c)))
        .collect();
    let source = |c: IVec3| chunks.get(&c).map(|chunk| (chunk, 1));
    let mut eye = Vec3::new(-5.0, 3.0, 9.0);
    let mut s = Schedule::new(probe_origin(eye), CHUNK_SIZE);
    s.refresh(s.probe_origin, source);
    for step in [Vec3::new(8.0, 0.0, 0.0), Vec3::new(-8.0, 8.0, 16.0), Vec3::new(3.0, -9.0, -30.0), Vec3::new(-50.0, 0.0, 0.0)] {
        eye += step;
        s.refresh(probe_origin(eye), source);
        let mut fresh = Window::new(s.window.origin, CHUNK_SIZE);
        for c in window_chunks(fresh.origin, CHUNK_SIZE) {
            fresh.stamp(c, source(c).map(|e| e.0));
        }
        let lo = s.window.origin;
        for y in 0..BITS_DIM {
            for z in 0..BITS_DIM {
                for x in 0..BITS_DIM {
                    let cell = lo + IVec3::new(x, y, z);
                    assert!(s.window.get(cell) == fresh.get(cell), "cell {cell} after moving to {eye}");
                }
            }
        }
    }
}

#[test]
fn coarse_cells_are_solid_at_half_fill_and_porous_for_foliage() {
    for coord in [IVec3::ZERO, IVec3::new(-2, -1, 3), IVec3::new(-31_250, 2, 62_500)] {
        let base = coord * CHUNK_SIZE;
        // Cell 0 (y 0..4) has one stone layer, cell 1 one leaf layer, cell 2 a
        // stone in every fourth column of one layer.
        let c = chunk(coord, |p| {
            let l = p - base;
            match l.y {
                0 => BlockId::Stone,
                5 => BlockId::Leaves,
                8 if l.x % 4 == 0 && l.z % 4 == 0 => BlockId::Stone,
                _ => BlockId::Air,
            }
        });
        let coarse = Coarse::build(&c, COARSE_SCALE);
        assert!(coarse.class_at(IVec3::new(2, 0, 7)) == Occupancy::Solid);
        assert!(coarse.class_at(IVec3::new(2, 1, 7)) == Occupancy::Leaf);
        assert!(coarse.class_at(IVec3::new(2, 2, 7)) == Occupancy::Air);
        assert!(coarse.class_at(IVec3::new(7, 7, 0)) == Occupancy::Air);
    }
}

#[test]
fn a_coarse_window_tracks_moves_and_edits_in_coarse_cells() {
    let cells = CHUNK_SIZE / COARSE_SCALE;
    let ground = |coord: IVec3, top: i32| {
        Coarse::build(&chunk(coord, |p| if p.y < top { BlockId::Stone } else { BlockId::Air }), COARSE_SCALE)
    };
    let chunks: HashMap<IVec3, Coarse> = (-2..=1)
        .flat_map(|y| (-7..=7).flat_map(move |z| (-7..=7).map(move |x| IVec3::new(x, y, z))))
        .map(|c| (c, ground(c, 0)))
        .collect();
    let source = |c: IVec3| chunks.get(&c).map(|cells| (cells, 1));
    let mut eye = Vec3::new(-20.0, 5.0, 33.0) / COARSE_SCALE as f32;
    let mut s = Schedule::new(probe_origin(eye), cells);
    s.refresh(s.probe_origin, source);
    s.take_batch(eye, usize::MAX);
    for step in [Vec3::new(8.0, 0.0, 0.0), Vec3::new(-3.0, 8.0, -17.0)] {
        eye += step;
        s.refresh(probe_origin(eye), source);
        let mut fresh = Window::new(s.window.origin, cells);
        for c in window_chunks(fresh.origin, cells) {
            fresh.stamp(c, source(c).map(|e| e.0));
        }
        for i in 0..BITS_DIM.pow(3) {
            let cell = s.window.origin + IVec3::new(i % BITS_DIM, i / BITS_DIM / BITS_DIM, (i / BITS_DIM) % BITS_DIM);
            assert!(s.window.get(cell) == fresh.get(cell), "cell {cell} after moving to {eye}");
        }
    }
    // Ground tops out at voxel y = 0; one placed layer makes the y = 0 cell solid.
    assert!(s.window.get(IVec3::new(0, 0, 0)) == Occupancy::Air);
    s.take_batch(eye, usize::MAX);
    let raised = ground(IVec3::ZERO, 1);
    s.refresh(s.probe_origin, |c| if c == IVec3::ZERO { Some((&raised, 2)) } else { source(c) });
    assert!(s.window.get(IVec3::new(0, 0, 0)) == Occupancy::Solid);
    assert!(!s.pending.is_empty());
    for b in &s.pending {
        let lo = *b * BRICK;
        assert!((lo - IVec3::splat(REACH)).cmplt(IVec3::splat(cells)).all()
            && (lo + IVec3::splat(BRICK + REACH)).cmpgt(IVec3::ZERO).all(), "brick {b} is beyond coarse ray reach");
    }
}

#[test]
fn leaf_transmittance_matches_the_canopy_half_path_at_every_scale() {
    for voxel_m in [1.0f32, 0.5, 0.25] {
        let cells = (LEAF_HALF_PATH_M / voxel_m).round() as i32;
        assert!((leaf_transmittance(voxel_m).powi(cells) - 0.5).abs() < 1e-3);
    }
}
