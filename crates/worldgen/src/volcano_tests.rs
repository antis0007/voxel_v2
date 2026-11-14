use super::*;
use engine_core::VoxelScalePreset;

const SEED: u32 = 7;

/// Every vent on the sphere, from a lattice that covers the whole surface.
fn all_vents() -> Vec<Vent> {
    let n = (atlas::RADIUS_M / VENT_CELL_M).ceil() as i64 + 1;
    let mut out = Vec::new();
    for x in -n..=n {
        for y in -n..=n {
            for z in -n..=n {
                out.extend(vent_in_cell(SEED, [x, y, z]));
            }
        }
    }
    out
}

#[test]
fn plates_both_collide_and_spread() {
    let (mut collide, mut spread) = (0, 0);
    for i in 0..4_000 {
        let a = f64::from(i) * 2.399_963;
        let y = 1.0 - 2.0 * (f64::from(i) + 0.5) / 4_000.0;
        let r = (1.0 - y * y).sqrt();
        let t = tectonics(SEED, DVec3::new(r * a.cos(), y, r * a.sin()));
        if t.boundary_m < 300.0 {
            if t.convergence > 0.2 { collide += 1 } else if t.convergence < -0.2 { spread += 1 }
        }
    }
    assert!(collide > 20 && spread > 20, "collide {collide} spread {spread}");
}

#[test]
fn every_volcano_kind_occurs_and_some_are_active() {
    let vents = all_vents();
    for kind in [Kind::Strato, Kind::Shield, Kind::Cinder, Kind::Caldera] {
        assert!(vents.iter().any(|v| v.kind == kind), "no {kind:?} among {}", vents.len());
    }
    let active = vents.iter().filter(|v| v.active).count();
    assert!(active > 0 && active < vents.len(), "active {active} of {}", vents.len());
}

#[test]
fn a_vent_is_found_from_any_nearby_point() {
    let v = all_vents()[0];
    let tangent = v.dir.cross(DVec3::Y).try_normalize().unwrap_or(DVec3::X);
    let beside = (v.dir + tangent * (v.radius_m * 0.5 / atlas::RADIUS_M)).normalize();
    assert!(vents_near(SEED, beside, 0.0).contains(&v));
}

#[test]
fn active_strato_craters_hold_flat_lava_and_dormant_ones_water() {
    let vents = all_vents();
    for (active, fluid) in [(true, BlockId::Lava), (false, BlockId::Water)] {
        let Some(v) = vents.iter().find(|v| v.kind == Kind::Strato && v.active == active) else { continue };
        let centre = shape(SEED, v.dir, 50.0, true, |_| 50.0);
        let (level, got) = centre.lake.expect("crater lake");
        assert_eq!(got, fluid);
        assert!(level > 50.0 + v.height_m * 0.5, "lake {level} on a {} m cone", v.height_m);
        // A point halfway out is high, steep basalt with no lake.
        let tangent = v.dir.cross(DVec3::Y).try_normalize().unwrap_or(DVec3::X);
        let flank = (v.dir + tangent * (v.radius_m * 0.4 / atlas::RADIUS_M)).normalize();
        let s = shape(SEED, flank, 50.0, true, |_| 50.0);
        assert!(s.lake.is_none() && s.metres > 80.0);
        assert_eq!(cover(&s, 0.5).map(|c| c.0), Some(BlockId::Basalt));
    }
}

#[test]
fn the_gate_adds_volcanic_ground_and_mossy_woods_and_older_planets_keep_theirs() {
    let cfg = |generator_version| crate::WorldSampleConfig {
        planet: None,
        generator_version,
        seed: SEED,
        world_type: crate::WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let (new, old) = (cfg(VERSION), cfg(VERSION - 1));
    let (mut volcanic, mut moss, mut changed) = (0, 0, 0);
    let step = atlas::HALF_M / 16.0;
    for face in 0..6 {
        for gz in -16..16 {
            for gx in -16..16 {
                let x = (face as f64 * atlas::PITCH_M + f64::from(gx) * step) as i32;
                let z = (f64::from(gz) * step) as i32;
                let c = crate::planet::column(&new, x, z);
                volcanic += u32::from(matches!(c.surface, BlockId::Basalt | BlockId::Ash | BlockId::Lava));
                moss += u32::from(c.surface == BlockId::GroveMoss);
                let was = crate::planet::column(&old, x, z);
                changed += u32::from((c.height, c.surface) != (was.height, was.surface));
            }
        }
    }
    assert!(volcanic > 10 && moss > 10, "volcanic {volcanic} moss {moss}");
    assert!(changed < 6 * 32 * 32 / 2, "gate rewrote {changed} columns");
}

/// `cargo test --release -p engine_worldgen column_cost -- --ignored --nocapture`
#[test]
#[ignore]
fn column_cost() {
    for generator_version in [VERSION - 1, VERSION - 1, VERSION, VERSION - 1, VERSION] {
        let cfg = crate::WorldSampleConfig { planet: None, generator_version, seed: SEED, world_type: crate::WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
        let start = std::time::Instant::now();
        let mut sum = 0i64;
        for z in 0..40 {
            for x in 0..40 {
                sum += i64::from(crate::planet::column(&cfg, x * 211 - 21_000, z * 211 - 21_000).height);
            }
        }
        println!("gen {generator_version}: {:.0} ns/column ({sum})", start.elapsed().as_nanos() as f64 / 1_600.0);
    }
}

#[test]
fn cones_rise_on_the_solved_spawn_face_too() {
    let cfg = |generator_version| crate::WorldSampleConfig {
        planet: None,
        generator_version,
        seed: SEED,
        world_type: crate::WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let edge = atlas::HALF_M - 2_000.0;
    let on_face = all_vents().into_iter().find_map(|v| {
        let p = atlas::from_body(v.dir * atlas::RADIUS_M);
        (p.x.abs() < edge && p.z.abs() < edge && v.kind != Kind::Caldera).then_some((v, p))
    });
    let (v, p) = on_face.expect("a vent on the spawn face");
    let (x, z) = (p.x as i32 + (v.crater_radius_m() * 1.5) as i32, p.z as i32);
    assert!(crate::basin::covers(&cfg(VERSION), x, z));
    let (new, old) = (crate::planet::column(&cfg(VERSION), x, z), crate::planet::column(&cfg(VERSION - 1), x, z));
    assert!(new.height > old.height + 10, "{:?} cone {} -> {}", v.kind, old.height, new.height);
}

#[test]
fn spawn_stays_clear_of_cones() {
    for seed in [1, 7, 42, 1234] {
        let cfg = |generator_version| crate::WorldSampleConfig { planet: None, generator_version, seed, world_type: crate::WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
        for (x, z) in [(0, 0), (300, -200), (-250, 400)] {
            assert_eq!(crate::planet::column(&cfg(VERSION), x, z).height, crate::planet::column(&cfg(VERSION - 1), x, z).height, "seed {seed}");
        }
    }
}
