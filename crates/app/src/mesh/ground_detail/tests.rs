use super::*;
use crate::mesh::{PADDED_CHUNK_SIZE, TreeFootprint};

fn meadow(coord: IVec3, scale: f32, trees: Vec<TreeFootprint>) -> GeneratedChunk {
    let mut c = GeneratedChunk::new(coord, vec![B::Air; PADDED_CHUNK_SIZE.pow(3) as usize]);
    c.voxel_meters = scale;
    c.trees = trees;
    let origin = coord * 32;
    for z in -1..33 {
        for x in -1..33 {
            c.set_world(origin + IVec3::new(x, 4, z), B::Grass);
        }
    }
    c
}

fn plan(c: &GeneratedChunk) -> DetailPlan {
    DetailPlan::new(c, &AtomicBool::new(false)).unwrap()
}

fn clumps(p: &DetailPlan) -> impl Iterator<Item = &Detail> {
    p.items.iter().filter(|d| matches!(d.kind, Kind::Clump(b) if b != Blades::Litter))
}

#[test]
fn cover_water_and_fungal_substrate_choose_distinct_families() {
    assert_eq!(species(B::Grass, 0.0, 0.0, 0.5, 1), Species::Grass);
    assert_eq!(species(B::Grass, 0.0, 0.0, 0.2, 1), Species::Clover);
    assert_eq!(species(B::Grass, 0.0, 0.0, 0.8, 1), Species::Flower);
    assert_eq!(species(B::Grass, 0.25, 0.0, 0.5, 1), Species::Fern);
    assert_eq!(species(B::Grass, 0.0, 0.4, 0.5, 1), Species::Fern);
    assert_eq!(species(B::FungalSoil, 0.0, 0.0, 0.5, 1), Species::Mushroom);
}

#[test]
fn open_turf_mixes_thin_field_and_dense_pockets_within_budget() {
    let mut counts = Vec::new();
    let mut kinds = std::collections::HashSet::new();
    for cz in -2..2 {
        for cx in [-40_000, -3, 0, 2] {
            let p = plan(&meadow(IVec3::new(cx, 0, cz), 1.0, Vec::new()));
            counts.push(clumps(&p).count());
            kinds.extend(clumps(&p).map(|d| d.kind));
            let clump_indices: usize = clumps(&p).map(Detail::indices).sum();
            assert!(clump_indices <= MAX_CLUMP_INDICES);
            assert!(p.indices() <= MAX_INDICES + MAX_CLUMP_INDICES);
        }
    }
    let mean = counts.iter().sum::<usize>() / counts.len();
    let (lo, hi) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
    // The old single-tussock pass admitted about 62 grass items a chunk.
    assert!(mean > 250, "{counts:?}");
    assert!(hi > lo * 2, "pockets must contrast with open field: {counts:?}");
    assert!(kinds.len() >= 5, "{kinds:?}");
}

#[test]
fn cover_is_deterministic_and_neighbours_agree_on_shared_columns() {
    let a = plan(&meadow(IVec3::new(-1, 0, 0), 1.0, Vec::new()));
    let b = plan(&meadow(IVec3::new(-1, 0, 0), 1.0, Vec::new()));
    let key = |p: &DetailPlan| p.items.iter().map(|d| (d.hash, d.kind, d.p.to_array().map(f32::to_bits))).collect::<Vec<_>>();
    assert_eq!(key(&a), key(&b));
    // Items belong to their supporting column, so no chunk emits into its neighbour.
    for d in &a.items {
        assert!(d.p.x >= -32.0 && d.p.x <= 0.0 && d.p.z >= 0.0 && d.p.z <= 32.0, "{}", d.p);
    }
}


#[test]
fn a_tree_clears_a_root_zone_shades_its_floor_and_feeds_its_edge() {
    let tree = TreeFootprint { x: 0.0, z: 0.0, base_y: 5.0, reach: 6.0, crown_low: 9.0, top: 16.0, rainforest: false };
    let density = |x: f32, z: f32, trees: &[TreeFootprint]| clumps::density_at(&Column::at(x, z, trees, 5.0));
    for (x, z) in [(0.8, 0.3), (-1.0, 0.5), (0.2, -1.2)] {
        assert!(density(x, z, &[tree]) < 0.15 * density(x, z, &[]), "trunk ring at {x},{z}");
    }
    for (x, z) in [(3.0, 0.0), (0.0, -3.5), (-2.5, 2.5)] {
        assert!(density(x, z, &[tree]) < 0.5 * density(x, z, &[]), "crown shade at {x},{z}");
    }
    for (x, z) in [(6.5, 0.0), (0.0, -6.5), (-4.6, 4.6)] {
        assert!(density(x, z, &[tree]) > density(x, z, &[]) + 0.6, "edge at {x},{z}");
    }
    let shifted = TreeFootprint { x: 16.0, z: 16.0, ..tree };
    let wooded = plan(&meadow(IVec3::ZERO, 1.0, vec![shifted]));
    let open = plan(&meadow(IVec3::ZERO, 1.0, Vec::new()));
    let litter = wooded.items.iter().filter(|d| d.kind == Kind::Clump(Blades::Litter)).count();
    assert!(litter > 10 && open.items.iter().all(|d| d.kind != Kind::Clump(Blades::Litter)));
    assert!(
        clumps(&wooded)
            .filter(|d| (d.p.x - 16.0).hypot(d.p.z - 16.0) < 3.5)
            .all(|d| matches!(d.kind, Kind::Clump(Blades::Weed | Blades::Short | Blades::Dense)))
    );
}

#[test]
fn fine_worlds_keep_metre_density_of_clumps() {
    let one = clumps(&plan(&meadow(IVec3::ZERO, 1.0, Vec::new()))).count();
    // Sixteen quarter-metre chunks tile the same 32 m square.
    let mut quarter = 0;
    for z in 0..4 {
        for x in 0..4 {
            quarter += clumps(&plan(&meadow(IVec3::new(x, 0, z), 0.25, Vec::new()))).count();
        }
    }
    assert!(quarter * 2 > one && quarter < one * 2, "{one} vs {quarter}");
}

#[test]
fn lod1_thins_clumps_instead_of_clearing_them() {
    let c = meadow(IVec3::ZERO, 1.0, Vec::new());
    let p = plan(&c);
    let mut near = ChunkMesh::empty();
    let mut far = ChunkMesh::empty();
    p.append(&c, &mut near, 0);
    p.append(&c, &mut far, 1);
    let tufts = |m: &ChunkMesh| m.vertices.iter().filter(|v| v.surface[2] == TUFT_KIND).count();
    assert!(tufts(&far) > 0 && tufts(&far) * 2 < tufts(&near));
    assert!(near.indices.len() <= p.indices());
}

#[test]
#[ignore = "timing probe: cargo test --profile release-fast ground_cover_cost -- --ignored --nocapture"]
fn ground_cover_cost() {
    let trees: Vec<_> = (0..12)
        .map(|i| TreeFootprint {
            x: 96.0 + (i % 4) as f32 * 11.0 + 3.0,
            z: -64.0 + (i / 4) as f32 * 13.0 + 2.0,
            base_y: 5.0,
            reach: 5.5,
            crown_low: 9.0,
            top: 17.0,
            rainforest: false,
        })
        .collect();
    for (label, t) in [("open", Vec::new()), ("wooded", trees)] {
        let c = meadow(IVec3::new(3, 0, -2), 1.0, t);
        let start = std::time::Instant::now();
        let n = 50;
        let mut items = 0;
        for _ in 0..n {
            items += plan(&c).items.len();
        }
        let plan_ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let start = std::time::Instant::now();
        let mut indices = 0;
        for _ in 0..n {
            let m = crate::mesh::memory::MeshPlan::for_chunk(&c, &AtomicBool::new(false), 0).unwrap();
            indices += m.build_lod(&c, 0).indices.len();
        }
        let full_ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("{label}: detail plan {plan_ms:.3} ms, full plan+build {full_ms:.3} ms, items {}, tris {}", items / n, indices / n / 3);
    }
}
