//! Features that clip: one feature's blocks cutting through another's, or
//! set dressing buried in rock.
use super::feature::Kind;
use super::furnish::Fit;
use super::tests_geometry::cfg_at;
use super::*;
use crate::undercrown::plan::Plan;
use engine_core::VoxelScalePreset as Scale;
use std::collections::BTreeMap;

fn name(k: &Kind) -> String {
    let s = format!("{k:?}");
    let head = s.split([' ', '{', '(']).next().unwrap_or("").to_string();
    match k {
        Kind::Fit(f) => format!("Fit::{}", format!("{f:?}").split([' ', '{', '(']).next().unwrap_or("")),
        _ => head,
    }
}

/// Dressing that should stand free in the room, never in rock or in another
/// feature. Columns, ramps, pillars and pedestals are built into the room.
fn free(k: &Kind) -> bool {
    matches!(
        k,
        Kind::Mushrooms { .. }
            | Kind::Grove(_)
            | Kind::Lamps(_)
            | Kind::Pipes(_)
            | Kind::IcePipes { .. }
            | Kind::Fit(Fit::Lanterns { .. } | Fit::Brazier { .. } | Fit::Lamp { .. } | Fit::Pews { .. } | Fit::Tables { .. })
    )
}

/// Voxel centres in a box at scale `v`.
fn cells(lo: Vec3, hi: Vec3, v: f32, mut f: impl FnMut(Vec3)) {
    let a = (lo / v).floor().as_ivec3();
    let b = (hi / v).ceil().as_ivec3();
    for y in a.y..b.y {
        for z in a.z..b.z {
            for x in a.x..b.x {
                let m = (IVec3::new(x, y, z).as_vec3() + 0.5) * v;
                if m.cmpge(lo).all() && m.cmplt(hi).all() {
                    f(m);
                }
            }
        }
    }
}

/// Per kind pair, voxels where both features place a solid block; per free
/// kind, solid voxels where the room itself is not air.
fn clips(l: &Layout, v: f32, mech: Mech) -> (BTreeMap<(String, String), usize>, BTreeMap<String, usize>) {
    let (mut pairs, mut buried) = (BTreeMap::new(), BTreeMap::new());
    for (i, f) in l.features.iter().enumerate() {
        cells(f.lo, f.hi, v, |m| {
            let Some(b) = f.sample(m, v, mech).filter(|b| b.is_solid()) else {
                return;
            };
            let cell = l.index.at(m);
            for (j, g) in l.features.iter().enumerate() {
                if j > i && g.sample(m, v, mech).is_some_and(|b| b.is_solid()) {
                    *pairs.entry((name(&f.kind), name(&g.kind))).or_insert(0) += 1;
                }
            }
            if free(&f.kind) && l.ground(cell, m, v, mech, true) != Some(B::Air) {
                *buried.entry(format!("{} {b:?} in {:?}", name(&f.kind), l.ground(cell, m, v, mech, true))).or_insert(0) += 1;
            }
        });
    }
    (pairs, buried)
}

#[test]
fn dressing_stands_clear_of_rock_and_every_other_feature() {
    for seed in [424242u32, 7, 99, 1234] {
        for (scale, v) in [(Scale::OneMeter, 1.0), (Scale::HalfMeter, 0.5), (Scale::QuarterMeter, 0.25)] {
            let plan = Plan::for_config(&cfg_at(seed, scale));
            let l = plan.v2.as_deref().unwrap_or_else(|| unreachable!("v2 layout"));
            for mech in [Mech::default(), Mech::ALL] {
                let (pairs, buried) = clips(l, v, mech);
                let bad: Vec<_> = pairs.iter().filter(|((a, b), _)| [a, b].iter().any(|k| matches!(k.as_str(), "Grove" | "Lamps" | "Pipes"))).collect();
                assert!(bad.is_empty(), "seed {seed} at {v} m: dressing overlaps {bad:?}");
                // Chains and pipes end set into the rock. Mushrooms and lamps
                // keep off it, but for a torch a probe can step over, which
                // then shows through.
                let touching: usize = buried.iter().filter(|(k, _)| k.starts_with("Grove") || k.starts_with("Lamps GlowCap")).map(|(_, n)| n).sum();
                assert!(touching <= 4, "seed {seed} at {v} m: dressing in rock {buried:?}");
            }
            let shrooms: usize = l.features.iter().map(|f| if let Kind::Grove(g) = &f.kind { g.shrooms.len() } else { 0 }).sum();
            assert!(shrooms >= 3, "seed {seed}: only {shrooms} mushrooms fit");
        }
    }
}

#[test]
fn generator_51_keeps_its_grid_dressing() {
    let c = WorldSampleConfig { generator_version: super::settle::VERSION - 1, ..cfg_at(424242, Scale::HalfMeter) };
    let plan = Plan::for_config(&c);
    let l = plan.v2.as_deref().unwrap_or_else(|| unreachable!("v2 layout"));
    assert!(l.features.iter().any(|f| matches!(f.kind, Kind::Mushrooms { .. })));
    assert!(!l.features.iter().any(|f| matches!(f.kind, Kind::Grove(_) | Kind::Lamps(_) | Kind::Pipes(_))));
}

#[test]
#[ignore = "timing"]
fn time_settle() {
    for version in [51u32, 52] {
        let c = WorldSampleConfig { generator_version: version, ..cfg_at(424242, Scale::HalfMeter) };
        let t = std::time::Instant::now();
        for _ in 0..3 {
            let _ = super::layout::author(&c);
        }
        println!("generator {version}: {:?} per layout", t.elapsed() / 3);
    }
}
