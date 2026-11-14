use super::*;
use engine_core::VoxelScalePreset as S;

fn cfg(generator_version: u32) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version, seed: crate::test_world::seed(), world_type: WorldType::Hills, voxel_scale: S::OneMeter }
}

fn push_unique(out: &mut Vec<Trail>, t: &Trail) {
    if !out.iter().any(|o| o.ax == t.ax && o.az == t.az && o.bx == t.bx && o.bz == t.bz && o.path.len() == t.path.len()) {
        out.push(t.clone());
    }
}

/// Every road touching a square of regions `reach` wide around the origin, without repeats.
fn roads_near(c: &WorldSampleConfig, reach: i32) -> Vec<Trail> {
    let span = crate::sites::region_span(c);
    let mut out = Vec::new();
    for rz in -reach..=reach {
        for rx in -reach..=reach {
            for t in roads::near(c, rx as f32 * span, rz as f32 * span).trails.iter() {
                push_unique(&mut out, t);
            }
        }
    }
    out
}

/// Every road and spawn trail touching the regions around the origin.
fn trails(c: &WorldSampleConfig) -> Vec<Trail> {
    let mut out = plan(c).trails.clone();
    for t in roads_near(c, 3) {
        push_unique(&mut out, &t);
    }
    out
}

#[test]
fn a_polyline_trail_measures_its_arc_and_finds_its_nearest_point() {
    let t = Trail::polyline(vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], 1, 1.6);
    assert_eq!(t.length(), 20.0);
    assert_eq!(t.at(0.25), (5.0, 0.0));
    assert_eq!(t.at(0.75), (10.0, 5.0));
    let (d, along) = t.nearest(12.0, 7.0);
    assert!((d - 2.0).abs() < 1e-5 && (along - 0.85).abs() < 1e-5, "{d} {along}");
    let (tx, tz) = t.tangent(0.9);
    assert!(tx.abs() < 1e-5 && (tz - 1.0).abs() < 1e-5);
}

#[test]
fn surveyed_routes_hold_the_grade_and_turn_smoothly() {
    let c = cfg(TRAIL_SURVEY_VERSION);
    let all = trails(&c);
    let surveyed: Vec<_> = all.iter().filter(|t| !t.path.is_empty()).collect();
    let legs = all.iter().filter(|t| t.path.is_empty() && t.cx == (t.ax + t.bx) * 0.5 && t.cz == (t.az + t.bz) * 0.5).count();
    assert!(!surveyed.is_empty(), "fixture must need at least one survey; {} trails, {legs} straight detour legs", all.len());
    for t in &surveyed {
        let step = t.length() / (t.grade.len() - 1) as f32;
        for w in t.grade.windows(2) {
            assert!((w[1] - w[0]).abs() <= step * routes::MAX_GRADE + 0.02, "grade step {}", w[1] - w[0]);
        }
        for w in t.path.windows(3) {
            let a = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            let b = (w[2].0 - w[1].0, w[2].1 - w[1].1);
            let (la, lb) = (a.0.hypot(a.1), b.0.hypot(b.1));
            if la < 1e-3 || lb < 1e-3 {
                continue;
            }
            let turn = (a.0 * b.0 + a.1 * b.1) / (la * lb);
            assert!(turn > 0.5, "sharp turn {turn} at {:?}", w[1]);
        }
    }
}

/// Smallest angle, in degrees, between two trails that leave the same point.
fn tightest_spoke(all: &[Trail]) -> f32 {
    let ends = |t: &Trail| {
        let (a, b) = (t.at(0.0), t.at(1.0));
        let (a1, b1) = (t.at((6.0 / t.length()).min(0.5)), t.at(1.0 - (6.0 / t.length()).min(0.5)));
        [(a, (a1.0 - a.0, a1.1 - a.1)), (b, (b1.0 - b.0, b1.1 - b.1))]
    };
    let mut tightest = 180.0f32;
    for (i, p) in all.iter().enumerate() {
        for q in &all[i + 1..] {
            for (pa, pd) in ends(p) {
                for (qa, qd) in ends(q) {
                    if (pa.0 - qa.0).hypot(pa.1 - qa.1) > 0.5 {
                        continue;
                    }
                    let cos = (pd.0 * qd.0 + pd.1 * qd.1) / (pd.0.hypot(pd.1) * qd.0.hypot(qd.1)).max(1e-6);
                    tightest = tightest.min(cos.clamp(-1.0, 1.0).acos().to_degrees());
                }
            }
        }
    }
    tightest
}

#[test]
fn roads_never_leave_a_node_side_by_side() {
    let c = cfg(TRAIL_SURVEY_VERSION);
    let roads = roads_near(&c, 3);
    let old = roads_near(&cfg(TRAIL_SURVEY_VERSION - 1), 3);
    assert!(roads.len() > 6, "fixture holds {} roads", roads.len());
    let (angle, before) = (tightest_spoke(&roads), tightest_spoke(&old));
    assert!(angle > 25.0, "two roads leave one node {angle} degrees apart; {} roads, {before} before over {} roads", roads.len(), old.len());
    eprintln!("tightest spoke {before:.1} -> {angle:.1} degrees, roads {} -> {}", old.len(), roads.len());
}

#[test]
fn older_generators_keep_their_trails() {
    let old = cfg(TRAIL_SURVEY_VERSION - 1);
    assert!(trails(&old).iter().all(|t| t.path.is_empty()));
}

#[test]
#[ignore]
fn survey_cost() {
    for v in [TRAIL_SURVEY_VERSION - 1, TRAIL_SURVEY_VERSION, TRAIL_SURVEY_VERSION - 1, TRAIL_SURVEY_VERSION] {
        let c = WorldSampleConfig { seed: crate::test_world::seed() ^ 0x55, ..cfg(v) };
        let t = std::time::Instant::now();
        let n = roads_near(&c, 3).len() + plan(&c).trails.len();
        eprintln!("gen {v}: {n} trails in {:?}", t.elapsed());
    }
}

/// Review poses over surveyed trails on the test seed:
/// `cargo test -p engine_worldgen --lib survey_poses -- --ignored --nocapture`.
#[test]
#[ignore]
fn survey_poses() {
    let c = cfg(TRAIL_SURVEY_VERSION);
    eprintln!("seed {}", c.seed);
    for t in trails(&c).iter().filter(|t| !t.path.is_empty()) {
        let (x, z) = t.at(0.5);
        let h = routes::height_at(t, 0.5);
        eprintln!("surveyed len {:.0} m from ({:.0},{:.0}) to ({:.0},{:.0}) mid ({x:.0},{h:.0},{z:.0}) points {}", t.length(), t.ax, t.az, t.bx, t.bz, t.path.len());
    }
}

/// Hillshaded height map with the trail network drawn over it, one PNG per gate:
/// `cargo test --release -p engine_worldgen --lib trail_maps -- --ignored --nocapture`.
/// `TRAIL_MAP` = `cx,cz,size_m,step_m`; files land in `target/maps/`.
#[test]
#[ignore]
fn trail_maps() {
    let spec: Vec<f32> = std::env::var("TRAIL_MAP").unwrap_or_else(|_| "440,-200,1300,2".into())
        .split(',').filter_map(|v| v.trim().parse().ok()).collect();
    let (cx, cz, size, step) = (spec[0], spec[1], spec[2], spec[3]);
    let n = (size / step) as usize;
    for v in [TRAIL_SURVEY_VERSION - 1, TRAIL_SURVEY_VERSION] {
        let c = cfg(v);
        let mut all = plan(&c).trails.clone();
        let span = crate::sites::region_span(&c);
        let reach = (size / span / 2.0).ceil() as i32 + 1;
        for rz in -reach..=reach {
            for rx in -reach..=reach {
                for t in roads::near(&c, cx + rx as f32 * span, cz + rz as f32 * span).trails.iter() {
                    push_unique(&mut all, t);
                }
            }
        }
        let height = |x: f32, z: f32| c.world_y(sample_column(&c, c.blocks_for_height(x), c.blocks_for_height(z)).height + 1);
        let mut rgb = vec![0u8; n * n * 3];
        for j in 0..n {
            for i in 0..n {
                let (x, z) = (cx - size * 0.5 + i as f32 * step, cz - size * 0.5 + j as f32 * step);
                let h = height(x, z);
                let shade = (0.75 + (h - height(x + step, z + step)) * 0.25 / step).clamp(0.35, 1.25);
                let mut c3 = if h <= crate::WATER_LEVEL_METERS + 1.0 {
                    [0.18, 0.34, 0.52]
                } else {
                    let t = ((h - 12.0) / 90.0).clamp(0.0, 1.0);
                    [0.30 + 0.35 * t, 0.45 + 0.15 * t, 0.25 + 0.35 * t].map(|v| v * shade)
                };
                if let Some((d, _, t)) = routes::nearest(all.iter(), Vec3::new(x, 0.0, z)) {
                    if d <= t.half_width + step * 0.5 {
                        c3 = if t.path.is_empty() { [0.55, 0.36, 0.20] } else { [0.95, 0.80, 0.25] };
                    }
                }
                let k = (j * n + i) * 3;
                for (o, v) in c3.iter().enumerate() {
                    rgb[k + o] = (v.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }
        let dir = format!("{}/../../target/maps", env!("CARGO_MANIFEST_DIR"));
        std::fs::create_dir_all(&dir).expect("map dir");
        let out = format!("{dir}/trails-{}-v{v}.png", c.seed);
        std::fs::write(&out, crate::map_probe::png(n as u32, n as u32, &rgb)).expect("write map");
        eprintln!("{out}: {} trails, {} surveyed", all.len(), all.iter().filter(|t| !t.path.is_empty()).count());
    }
}
