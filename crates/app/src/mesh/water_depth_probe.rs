//! Resident water depth must be continuous across chunk borders, and coarse
//! (LOD > 0) chunks must carry real depth rather than the shader's 10 m default.
use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::{World, WorldType};
use std::sync::atomic::AtomicBool;

// The border tests need ocean at chunks x -12..-9. Generator 53 shrank seas and
// moved this coast; a historical generator never changes, so pin one.
fn config() -> crate::world::WorldSampleConfig {
    let mut world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 0xA341_316C);
    world.generator_version = 52;
    world.sample_config()
}

#[test]
fn submerged_volume_optics_and_tessellation_are_invariant_but_free_surfaces_change() {
    let never = AtomicBool::new(false);
    let mesh_of = |source: &GeneratedChunk| MeshPlan::for_chunk(source, &never).unwrap().build_lod(source, 0);
    let bytes = |mesh: &ChunkMesh| bytemuck::cast_slice::<Vertex3, u8>(&mesh.vertices).to_vec();
    for scale in [1.0, 0.5, 0.25] {
        let mut source = GeneratedChunk {
            voxel_meters: scale, water_levels: Default::default(), water_deficit: Default::default(), chunk: IVec3::new(-1, 0, -1),
            blocks: vec![BlockId::Air; (PADDED_CHUNK_SIZE as usize).pow(3)],
            sky_ceiling: vec![0; (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize],
            bed_height: vec![0; (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize],
            bed_frac: Vec::new(),
        };
        for z in -28..-4 { for x in -28..-4 { for y in 0..=6 {
            source.set_world(IVec3::new(x,y,z), if y==0 {BlockId::Sand} else {BlockId::Water});
        }}}
        let baseline = mesh_of(&source);
        let baseline_bytes = bytes(&baseline);
        let p = IVec3::new(-16,3,-16);
        for level in [1,7,8] {
            let mut altered=source.clone();altered.water_levels.insert(p,level);
            let mesh=mesh_of(&altered);
            assert_eq!(bytes(&mesh),baseline_bytes,"all vertex geometry/optics/tessellation must match: scale={scale} level={level}");
            assert_eq!(mesh.indices,baseline.indices,"index order must also match");
            assert_eq!(altered.water_levels.get(&p),Some(&level),"rendering must preserve authoritative volumes");
        }
        let p=IVec3::new(-16,6,-16);
        let mut low=source.clone();low.water_levels.insert(p,1);
        let mut high=source.clone();high.water_levels.insert(p,7);
        assert_ne!(bytes(&mesh_of(&low)),bytes(&mesh_of(&high)),"free fractional geometry must remain a dependency at scale={scale}");
    }
}

#[test]
fn depth_matches_on_both_sides_of_a_chunk_border() {
    let cfg = config();
    let never = AtomicBool::new(false);
    let top = cfg.water_level() as f32 + 1.0 - WATER_SURFACE_INSET;
    for cx in -12..-9 {
        let a = crate::world::generate_padded(&cfg, IVec3::new(cx, 0, 0), &never).unwrap();
        let b = crate::world::generate_padded(&cfg, IVec3::new(cx + 1, 0, 0), &never).unwrap();
        let x = ((cx + 1) * CHUNK_SIZE) as f32;
        for z in [0.0f32, 8.0, 16.0, 24.0] {
            let p = Vec3::new(x, top, z);
            let (da, _) = a.water_optics(p, Vec3::Y);
            let (db, _) = b.water_optics(p, Vec3::Y);
            assert!((da - db).abs() < 1e-3, "x={x} z={z}: {da} vs {db}");
        }
    }
}

#[test]
fn coarse_ocean_water_gets_real_depth() {
    let cfg = config();
    let never = AtomicBool::new(false);
    let chunk = crate::world::generate_padded(&cfg, IVec3::new(-11, 0, 0), &never).unwrap();
    let mut mesh = coarsened_mesh(&chunk, 1);
    memory::encode_water(&chunk, &mut mesh);
    let depths: Vec<f32> = mesh
        .vertices
        .iter()
        .filter(|v| v.surface[2] % 64.0 == 8.0 && v.normal[1] > 0.5)
        .map(|v| (v.surface[3] - 8.0).floor() * 0.25)
        .collect();
    assert!(!depths.is_empty(), "no coarse water tops in an ocean chunk");
    assert!(
        depths.iter().any(|&d| (d - 10.0).abs() > 0.5),
        "every coarse water vertex kept the 10 m default depth"
    );
}

#[test]
fn shared_border_vertices_agree_on_packed_water_data() {
    use std::collections::HashMap;
    let cfg = config();
    let never = AtomicBool::new(false);
    let mesh_of = |cx: i32| {
        let chunk = crate::world::generate_padded(&cfg, IVec3::new(cx, 0, 0), &never).unwrap();
        MeshPlan::for_chunk(&chunk, &never)
            .unwrap()
            .build_lod(&chunk, 0)
    };
    let key = |p: [f32; 3]| (p[0].to_bits(), p[1].to_bits(), p[2].to_bits());
    let tops = |mesh: &ChunkMesh| -> HashMap<_, _> {
        mesh.vertices
            .iter()
            .filter(|v| v.surface[2] % 64.0 == 8.0 && v.normal[1] > 0.5)
            .map(|v| {
                (
                    key(v.pos),
                    (v.surface[3].floor(), v.color.map(f32::to_bits)),
                )
            })
            .collect()
    };
    let mut shared = 0;
    for cx in -12..-9 {
        let (a, b) = (tops(&mesh_of(cx)), tops(&mesh_of(cx + 1)));
        for (k, va) in &a {
            if let Some(vb) = b.get(k) {
                assert_eq!(
                    va,
                    vb,
                    "border vertex {k:?} differs between chunks {cx} and {}",
                    cx + 1
                );
                shared += 1;
            }
        }
    }
    assert!(
        shared >= 30,
        "expected many shared border vertices, found {shared}"
    );
}

#[test]
fn all_lods_preserve_free_water_level_area_and_optics() {
    use std::collections::HashMap;
    let never = AtomicBool::new(false);
    for scale in [1.0, 0.5, 0.25] {
        for chunk_y in [-1, 0] {
            for level in [8u8, 4] {
                let coord = IVec3::new(-2, chunk_y, 3);
                let bed = chunk_y * CHUNK_SIZE + 8;
                let row = chunk_y * CHUNK_SIZE + 12;
                let mut chunk = GeneratedChunk {
                    voxel_meters: scale,
                    water_levels: HashMap::new(),
                    water_deficit: HashMap::new(),
                    chunk: coord,
                    blocks: vec![BlockId::Air; (PADDED_CHUNK_SIZE as usize).pow(3)],
                    sky_ceiling: vec![bed; (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize],
                    bed_height: vec![bed; (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize],
                    bed_frac: Vec::new(),
                };
                let origin = chunk.origin();
                for y in 0..PADDED_CHUNK_SIZE {
                    for z in 0..PADDED_CHUNK_SIZE {
                        for x in 0..PADDED_CHUNK_SIZE {
                            let p = origin + IVec3::new(x, y, z);
                            let b = if p.y <= bed {
                                BlockId::Stone
                            } else if p.y <= row {
                                BlockId::Water
                            } else {
                                BlockId::Air
                            };
                            chunk.set_world(p, b);
                            if p.y == row && level != 8 {
                                chunk.water_levels.insert(p, level);
                            }
                        }
                    }
                }
                let fine = MeshPlan::for_chunk(&chunk, &never)
                    .unwrap()
                    .build_lod(&chunk, 0);
                let key = |p: [f32; 3]| p.map(f32::to_bits);
                let is_top = |v: &Vertex3| v.surface[2] % 64.0 == 8.0 && v.normal[1] > 0.5;
                let fine_tops: HashMap<_, _> = fine
                    .vertices
                    .iter()
                    .filter(|v| is_top(v))
                    .map(|v| {
                        (
                            key(v.pos),
                            (v.surface[3].to_bits(), v.color.map(f32::to_bits)),
                        )
                    })
                    .collect();
                assert!(!fine_tops.is_empty());
                let expected = row as f32 + f32::from(level) / 8.0 * (1.0 - WATER_SURFACE_INSET);
                for lod in 1..=3 {
                    let mut coarse = coarsened_mesh(&chunk, lod);
                    memory::encode_water(&chunk, &mut coarse);
                    let mut shared = 0;
                    for v in coarse.vertices.iter().filter(|v| is_top(v)) {
                        assert!(
                            (v.pos[1] - expected).abs() < 2e-6,
                            "lod={lod} scale={scale} level={level} y={} expected={expected}",
                            v.pos[1]
                        );
                        if let Some(fine) = fine_tops.get(&key(v.pos)) {
                            assert_eq!(
                                &(v.surface[3].to_bits(), v.color.map(f32::to_bits)),
                                fine,
                                "shared water optics differ at {:?}, lod={lod}",
                                v.pos
                            );
                            shared += 1;
                        }
                    }
                    assert!(shared >= 4, "no shared source corners, lod={lod}");
                    let area: f32 = coarse
                        .indices
                        .chunks_exact(3)
                        .filter_map(|tri| {
                            let a = &coarse.vertices[tri[0] as usize];
                            if !is_top(a) {
                                return None;
                            }
                            let b = Vec3::from_array(coarse.vertices[tri[1] as usize].pos);
                            let c = Vec3::from_array(coarse.vertices[tri[2] as usize].pos);
                            Some(
                                (b - Vec3::from_array(a.pos))
                                    .cross(c - Vec3::from_array(a.pos))
                                    .y
                                    .abs()
                                    * 0.5,
                            )
                        })
                        .sum();
                    assert!(
                        (area - 1024.0).abs() < 0.01,
                        "missing or duplicate water area: {area}, lod={lod}"
                    );
                }
            }
        }
    }
}

#[test]
fn exact_water_budget_covers_fragmented_partial_surfaces() {
    let mut chunk = GeneratedChunk {
        voxel_meters: 1.0,
        water_levels: Default::default(),
        water_deficit: Default::default(),
        chunk: IVec3::ZERO,
        blocks: vec![BlockId::Air; (PADDED_CHUNK_SIZE as usize).pow(3)],
        sky_ceiling: Vec::new(),
        bed_height: Vec::new(),
        bed_frac: Vec::new(),
    };
    for y in 0..PADDED_CHUNK_SIZE {
        for z in 0..PADDED_CHUNK_SIZE {
            for x in 0..PADDED_CHUNK_SIZE {
                let p = chunk.origin() + IVec3::new(x, y, z);
                if (p.x + p.y + p.z).rem_euclid(2) == 0 {
                    chunk.set_world(p, BlockId::Water);
                    chunk.water_levels.insert(p, 4);
                }
            }
        }
    }
    let admitted = exact_water_mesh_bytes_upper_bound(&chunk);
    let mut mesh = ChunkMesh::empty();
    append_water_mesh(&mut mesh, &chunk);
    let capacity = mesh.vertices.capacity() * std::mem::size_of::<Vertex3>()
        + mesh.indices.capacity() * std::mem::size_of::<u32>();
    assert!(!mesh.indices.is_empty());
    assert!(
        capacity <= admitted,
        "water capacities {capacity} exceed admission {admitted}"
    );
    assert!(
        admitted > memory::PLANNING_BYTES,
        "fragmented water needs an explicit output reservation"
    );
}
