//! One-voxel flow steps mesh as ramps: no internal wall, both sides agree on
//! the shared corner heights, including across a chunk border at negative
//! coordinates. Deeper drops keep their wall.
use super::*;
use std::sync::atomic::AtomicBool;

fn empty(chunk: IVec3) -> GeneratedChunk {
    GeneratedChunk {
        voxel_meters: 1.0,
        generator_version: 0,
        flora_config: None,
        manyfold_shell: false,
        water_levels: Default::default(),
        water_deficit: Default::default(), water_motion: Default::default(),
        chunk,
        blocks: vec![BlockId::Air; (PADDED_CHUNK_SIZE as usize).pow(3)],
        sky_ceiling: vec![0; (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize],
        bed_height: vec![0; (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize],
        bed_frac: Vec::new(),
        trees: Vec::new(),
    }
}

/// A channel along x over `x0..x1`, z in -20..-12, sand banks; water up to row
/// `upper` where x < `step_x`, else `upper - drop`. Same world for any chunk.
fn channel(chunk: IVec3, x0: i32, x1: i32, step_x: i32, upper: i32, drop: i32) -> GeneratedChunk {
    let mut c = empty(chunk);
    for z in -22..-10 {
        for x in x0..x1 {
            let top = if x < step_x { upper } else { upper - drop };
            for y in 0..=upper + 1 {
                let bank = !(-20..-12).contains(&z);
                let block = if y == 0 || (bank && y <= upper) {
                    BlockId::Sand
                } else if !bank && y <= top {
                    BlockId::Water
                } else {
                    BlockId::Air
                };
                c.set_world(IVec3::new(x, y, z), block);
            }
        }
    }
    c
}

fn mesh_of(c: &GeneratedChunk) -> ChunkMesh {
    let never = AtomicBool::new(false);
    MeshPlan::for_chunk(c, &never, 0).unwrap().build_lod(c, 0)
}

fn water(mesh: &ChunkMesh) -> impl Iterator<Item = &Vertex3> {
    mesh.vertices.iter().filter(|v| v.surface[2] % 64.0 == 8.0)
}

/// Water wall vertices on the plane x = `plane`, strictly inside the channel.
fn walls_on(mesh: &ChunkMesh, plane: f32) -> usize {
    water(mesh)
        .filter(|v| v.normal[0].abs() > 0.9 && (v.pos[0] - plane).abs() < 1e-4)
        .filter(|v| v.pos[2] > -19.5 && v.pos[2] < -12.5)
        .count()
}

/// Top-surface heights at the corners on x = `plane`, keyed by z.
fn tops_on(mesh: &ChunkMesh, plane: f32) -> Vec<(i32, f32)> {
    let mut tops: Vec<(i32, f32)> = water(mesh)
        .filter(|v| v.normal[1] > 0.9 && (v.pos[0] - plane).abs() < 1e-4)
        .filter(|v| v.pos[2] > -19.5 && v.pos[2] < -12.5)
        .map(|v| (v.pos[2].round() as i32, v.pos[1]))
        .collect();
    tops.sort_by(|a, b| a.partial_cmp(b).unwrap());
    tops.dedup_by(|a, b| a.0 == b.0 && (a.1 - b.1).abs() < 1e-5);
    tops
}

#[test]
fn one_voxel_step_is_a_ramp_without_an_internal_wall() {
    let c = channel(IVec3::new(-1, 0, -1), -30, -2, -16, 6, 1);
    let mesh = mesh_of(&c);
    assert_eq!(walls_on(&mesh, -16.0), 0, "a one-voxel step must not emit a water wall");
    let tops = tops_on(&mesh, -16.0);
    assert!(!tops.is_empty());
    let level = 1.0 - WATER_SURFACE_INSET;
    let expected = 6.0 + (level + (level - 1.0)) * 0.5;
    for (z, y) in &tops {
        // One height per corner: the upper and lower quads meet exactly.
        assert_eq!(tops.iter().filter(|t| t.0 == *z).count(), 1, "split corner at z={z}: {tops:?}");
        assert!((y - expected).abs() < 1e-4, "corner z={z} at {y}, expected {expected}");
    }
}

#[test]
fn ramp_corners_agree_across_a_chunk_border() {
    // Step on the border x = 0 between chunks -1 and 0; each sees the other
    // side only through its one-voxel halo.
    let west = channel(IVec3::new(-1, 0, -1), -12, 12, 0, 6, 1);
    let east = channel(IVec3::new(0, 0, -1), -12, 12, 0, 6, 1);
    let (a, b) = (tops_on(&mesh_of(&west), 0.0), tops_on(&mesh_of(&east), 0.0));
    assert!(!a.is_empty() && a.len() == b.len(), "{a:?} vs {b:?}");
    for ((za, ya), (zb, yb)) in a.iter().zip(&b) {
        assert_eq!(za, zb);
        assert!((ya - yb).abs() < 1e-5, "z={za}: west {ya} east {yb}");
    }
    assert_eq!(walls_on(&mesh_of(&west), 0.0) + walls_on(&mesh_of(&east), 0.0), 0);
}

#[test]
fn two_voxel_drop_keeps_its_wall_and_flat_water_keeps_the_greedy_path() {
    let fall = channel(IVec3::new(-1, 0, -1), -30, -2, -16, 6, 2);
    assert!(walls_on(&mesh_of(&fall), -16.0) > 0, "a waterfall keeps its wall");
    let flat = channel(IVec3::new(-1, 0, -1), -30, -2, -16, 6, 0);
    let never = AtomicBool::new(false);
    let plan = super::solid::SolidPlan::new(&flat, &never).unwrap();
    assert!(!plan.uses_exact_water(), "flat water must not switch to the exact mesher");
}
