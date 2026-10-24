use super::*;
use crate::sample_block;
use engine_core::VoxelScalePreset;
use std::collections::HashSet;

const FAMILIES: [Family; 4] = [Family::Foundry, Family::Toll, Family::Archive, Family::Hart];
const SCALES: [VoxelScalePreset; 3] =
    [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter];

fn cfg(version: u32, voxel_scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { generator_version: version, seed: 424242, world_type: WorldType::Hills, voxel_scale }
}

fn shrines(cfg: &WorldSampleConfig) -> Vec<(IVec3, Shrine)> {
    let mut out = Vec::new();
    for x in -10..=10 {
        for z in -10..=10 {
            if let Some(s) = site_shrine(cfg, x, z) {
                out.push((IVec3::new(x, z, 0), s));
            }
        }
    }
    for x in -16..=16 {
        for z in -16..=16 {
            if let Some(s) = cavern_shrine(cfg, x, z) {
                out.push((IVec3::new(x, z, 1), s));
            }
        }
    }
    out
}

fn first(all: &[(IVec3, Shrine)], family: Family) -> (IVec3, Shrine) {
    *all.iter().find(|(_, s)| s.family == family).unwrap_or_else(|| panic!("no {family:?} shrine"))
}

fn get(cfg: &WorldSampleConfig, p: IVec3) -> B {
    sample_block(cfg, p)
}

/// A synthetic complex: any family, seed and modifier, anywhere.
fn synthetic(family: Family, seed: u32, modifier: SiteModifier) -> Shrine {
    Shrine { family, center: Vec3::ZERO, turn: 0, voxel: 1.0, seed, modifier }
}

#[test]
fn boss_layouts_vary_between_sites() {
    for f in FAMILIES {
        let mut prints = HashSet::new();
        for seed in 0..8u32 {
            let s = synthetic(f, hash_u32(seed + 99), SiteModifier::None);
            let mut print = Vec::new();
            for u in (-12..12).step_by(2) {
                for w in (10..28).step_by(2) {
                    for y in [0, 3, 6] {
                        print.push(s.local_block(Vec3::new(u as f32 + 0.5, y as f32 + 0.5, w as f32 + 0.5)));
                    }
                }
            }
            prints.insert(format!("{print:?}"));
        }
        assert!(prints.len() >= 5, "{f:?} only {} distinct layouts", prints.len());
    }
}

#[test]
fn boss_shrines_exist_only_from_generator_32() {
    let old = cfg(31, VoxelScalePreset::OneMeter);
    assert!(shrines(&old).is_empty());
    let new = cfg(32, VoxelScalePreset::OneMeter);
    let all = shrines(&new);
    for f in FAMILIES {
        first(&all, f);
    }
    // The v31 sampler hooks stay silent over every v32 complex.
    for (_, s) in &all {
        for p in s.cells(BOUNDS).into_iter().step_by(37) {
            let m = (p.as_vec3() + Vec3::splat(0.5)) * old.voxel_meters();
            let (rx, rz) = ((m.x / 192.0).round() as i32, (m.z / 192.0).round() as i32);
            assert!(site_block(&old, rx, rz, m).is_none());
            assert!(cavern_block(&old, m).is_none());
        }
    }
}

#[test]
fn boss_complexes_fit_the_site_sampler_window() {
    let c = cfg(32, VoxelScalePreset::OneMeter);
    for (id, s) in shrines(&c).into_iter().filter(|(id, _)| id.z == 0) {
        let centre = Vec3::new(id.x as f32 * 192.0, 0.0, id.y as f32 * 192.0);
        for (u, w) in [(-16, -10), (16, -10), (-16, 29), (16, 29)] {
            let p = s.world_of(Vec3::new(u as f32, 0.0, w as f32)) - centre;
            assert!(p.x.abs() <= REACH && p.z.abs() <= REACH, "{id} corner {u},{w} at {p}");
        }
    }
}

#[test]
fn boss_encounter_points_sit_in_their_arenas() {
    let c = cfg(32, VoxelScalePreset::OneMeter);
    for (id, s) in shrines(&c).into_iter().filter(|(id, _)| id.z == 0).take(12) {
        let base = IVec3::new(id.x, id.y, 0);
        let found = super::super::encounters(&c, s.relic_point())
            .into_iter()
            .find(|(site, _, _)| *site == base)
            .expect("encounter");
        assert!((found.1 - s.relic_point()).length() < 1e-3, "{id}");
        assert_eq!(shrine(&c, base, found.1), Some(s));
        assert!(inside(CLEAR, s.local_of(s.relic_point() + Vec3::splat(0.01))));
    }
}

#[test]
fn every_boss_complex_is_sealed_readable_and_solvable_at_every_scale() {
    for scale in SCALES {
        let c = cfg(32, scale);
        let all = shrines(&c);
        for f in FAMILIES {
            let (id, s) = first(&all, f);
            let v = c.voxel_meters();
            let relic = (s.relic_point() / v + Vec3::splat(0.01)).floor().as_ivec3();
            assert_eq!(get(&c, relic), B::Air, "{f:?} {id} {scale:?} relic space");
            assert!(get(&c, relic - IVec3::Y).is_solid(), "{f:?} {scale:?} relic floor");
            let door = s.cells(DOOR);
            assert!(!door.is_empty() && door.iter().all(|p| get(&c, *p) == f.door_block()), "{f:?} door");
            for p in s.cells(WEAK_WALL) {
                assert_eq!(get(&c, p), B::WeatheredBricks, "{f:?} cracked fallback wall");
            }
            for t in s.treasures() {
                for p in s.cells(t.pedestal) {
                    assert_eq!(get(&c, p), B::VoidStone, "{f:?} {scale:?} pedestal {}", t.index);
                }
                for p in s.cells(t.cache) {
                    assert_eq!(get(&c, p), B::Air, "{f:?} {scale:?} cache {} empty", t.index);
                }
            }
            for p in s.cells([-1, 1, 1, 3, -6, -4]) {
                assert_eq!(get(&c, p), B::Air, "{f:?} approach");
            }
            let (keys, rule) = f.keys();
            for k in keys {
                let cells = s.cells(*k);
                assert!(!cells.is_empty());
                match rule {
                    Rule::Fill(_) => assert!(cells.iter().all(|p| get(&c, *p) == B::Air), "{f:?} socket"),
                    Rule::Break(b) => assert!(cells.iter().all(|p| get(&c, *p) == b), "{f:?} pin"),
                }
            }
            let supply: Vec<Box3> = match f {
                Family::Foundry => HOPPER_COAL.to_vec(),
                Family::Archive => WALL_CRYSTALS.to_vec(),
                Family::Toll | Family::Hart => Vec::new(),
            };
            for b in supply {
                assert!(s.cells(b).iter().all(|p| matches!(get(&c, *p), B::CoalOre | B::Crystal)));
            }
            if f == Family::Hart {
                for i in 1..=5 {
                    for p in s.cells([-5, -4, 1 + i, 3 + i, i - 4, i - 3]) {
                        assert_eq!(get(&c, p), B::Air, "ledge {i} headroom");
                    }
                }
            }
        }
    }
}

#[test]
fn boss_complex_cells_hold_at_negative_and_large_coordinates() {
    for scale in SCALES {
        let v = scale.meters();
        for (n, centre) in [Vec3::new(-100_000.0, -40.0, 300_000.0), Vec3::new(1.0e6, 120.0, -7.0e5)].into_iter().enumerate() {
            for turn in 0..4 {
                let s = Shrine { center: (centre / v).floor() * v, turn, voxel: v, ..synthetic(Family::Toll, 5 + n as u32, SiteModifier::None) };
                let door = s.cells(DOOR);
                let per_m = (1.0 / v).round() as usize;
                assert_eq!(door.len(), 2 * 3 * per_m * per_m * per_m, "{scale:?} turn {turn}");
                for p in door {
                    assert_eq!(s.sample((p.as_vec3() + Vec3::splat(0.5)) * v), Some(B::Planks));
                }
                let r = s.relic_point();
                assert_eq!(s.sample(r + Vec3::splat(0.01)), Some(B::Air));
            }
        }
    }
}

#[test]
fn boss_treasure_points_are_every_cache() {
    for scale in SCALES {
        let c = cfg(32, scale);
        let all = shrines(&c);
        for f in FAMILIES {
            let (id, s) = first(&all, f);
            let points: Vec<_> = treasure_points(&c, s.relic_point(), 40.0).into_iter().filter(|p| p.site == id).collect();
            assert_eq!(points.len(), 3, "{f:?} {scale:?}");
            for t in s.treasures() {
                let p = points.iter().find(|p| p.index == t.index).expect("index");
                assert_eq!(p.tier, t.index + 1);
                let cell = (p.pos / c.voxel_meters()).floor().as_ivec3();
                assert!(s.cells(t.cache).contains(&cell), "{f:?} {scale:?} cache {}", t.index);
            }
        }
        assert!(treasure_points(&cfg(31, scale), Vec3::ZERO, 2000.0).is_empty());
    }
}

/// Per-voxel cost over a fixed 64 x 24 x 64 m box around each family's
/// complex, sampled through the full generator. Run explicitly:
/// `cargo test --release -p engine_worldgen boss_access_probe -- --ignored --nocapture`
#[test]
#[ignore = "profiling probe; run explicitly"]
fn boss_access_probe() {
    let c = cfg(32, VoxelScalePreset::OneMeter);
    let all = shrines(&c);
    for f in FAMILIES {
        let (_, s) = first(&all, f);
        let o = s.center.floor().as_ivec3();
        let mut best = f64::MAX;
        let mut n = 0usize;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            n = 0;
            for x in -32..32 {
                for z in -32..32 {
                    for y in -4..20 {
                        n += get(&c, o + IVec3::new(x, y, z)).is_solid() as usize;
                    }
                }
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        println!("probe {f:?} {:.1} ms ({:.0} ns/voxel, solid {n})", best * 1e3, best * 1e9 / (64.0 * 64.0 * 24.0));
    }
}
