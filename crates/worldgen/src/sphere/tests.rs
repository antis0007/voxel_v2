use engine_core::cube_sphere::Face;

use super::*;

const RADIUS_M: f64 = 6.0e6;

fn params(class: PlanetClass, seed: u32) -> PlanetParams {
    PlanetParams { class, seed, sea_level_m: 0.0, relief_m: 6000.0 }
}

fn grid(n: u32) -> SphereGrid {
    SphereGrid::new(n, RADIUS_M).expect("aligned size")
}

fn chord(g: &SphereGrid, i: usize, j: usize) -> f64 {
    (g.centre(i) - g.centre(j)).length()
}

#[test]
fn grid_rejects_unaligned_sizes() {
    assert!(SphereGrid::new(64, RADIUS_M).is_some());
    assert!(SphereGrid::new(32, RADIUS_M).is_none());
    assert!(SphereGrid::new(65, RADIUS_M).is_none());
    assert!(SphereGrid::new(0, RADIUS_M).is_none());
    assert!(SphereGrid::new(64, 0.0).is_none());
}

#[test]
fn adjacency_is_symmetric_with_four_distinct_neighbours() {
    let g = grid(64);
    let spacing = std::f64::consts::FRAC_PI_2 / 64.0;
    for i in 0..g.len() {
        let nb = g.neighbours(i);
        for (k, &j) in nb.iter().enumerate() {
            let j = j as usize;
            assert_ne!(j, i, "cell {i} is its own neighbour");
            assert!(nb[..k].iter().all(|&o| o as usize != j), "cell {i} repeats {j}");
            assert!(g.neighbours(j).contains(&(i as u32)), "{i} -> {j} is one-way");
            assert!(chord(&g, i, j) < 2.0 * spacing, "{i} -> {j} is not adjacent on the sphere");
        }
    }
    // Model R: the three corner cells around each cube corner are pairwise neighbours.
    // A corner cell is the only kind with two neighbours on other faces.
    let foreign = |j: usize| {
        g.neighbours(j).iter().filter(|&&k| g.face_of(k as usize) != g.face_of(j)).count()
    };
    let last = g.n() - 1;
    let mut corners = 0;
    for face in Face::ALL {
        for (u, v) in [(0, 0), (last, 0), (0, last), (last, last)] {
            let i = g.index(face, u, v);
            assert_eq!(foreign(i), 2, "{face:?} ({u}, {v})");
            let other_face_corners = g
                .neighbours(i)
                .iter()
                .filter(|&&j| g.face_of(j as usize) != face && foreign(j as usize) == 2)
                .count();
            assert_eq!(other_face_corners, 2, "{face:?} ({u}, {v})");
            corners += 1;
        }
    }
    assert_eq!(corners, 24, "eight cube corners, three cells each");
}

#[test]
fn areas_sum_to_sphere_and_shrink_at_corners() {
    let g = grid(64);
    let sphere = 4.0 * std::f64::consts::PI * RADIUS_M * RADIUS_M;
    let rel = (g.total_area_m2() - sphere).abs() / sphere;
    // Planar quads under the sphere miss about theta^2 / 12 of it; theta is 0.025 rad here.
    assert!(rel < 1e-3, "area off by {rel}");
    for face in Face::ALL {
        let corner = g.area_m2(g.index(face, 0, 0));
        let centre = g.area_m2(g.index(face, 32, 32));
        assert!(corner < centre * 0.95, "{face:?}: corner {corner} centre {centre}");
    }
}

#[test]
fn height_is_continuous_across_face_edges() {
    let g = grid(64);
    let p = params(PlanetClass::Wet, 7);
    let h = elevation(&g, &p);
    let (mut inner, mut cross, mut crossings) = (0.0f64, 0.0f64, 0);
    for i in 0..g.len() {
        for j in g.neighbours(i) {
            let j = j as usize;
            let slope = (h[i] - h[j]).abs() / chord(&g, i, j);
            if g.face_of(i) == g.face_of(j) {
                inner = inner.max(slope);
            } else {
                cross = cross.max(slope);
                crossings += 1;
            }
        }
    }
    assert_eq!(crossings, 6 * 4 * 64);
    assert!(cross <= inner * 1.5, "cross-edge slope {cross} vs interior {inner}");
}

fn assert_chains_terminate(g: &SphereGrid, d: &Drainage) {
    let mut terminal_area = 0.0;
    for i in 0..g.len() {
        let mut at = i;
        let mut steps = 0;
        while d.receiver[at] as usize != at {
            let next = d.receiver[at] as usize;
            assert!(g.neighbours(at).contains(&(next as u32)), "receiver of {at} is not adjacent");
            at = next;
            steps += 1;
            assert!(steps <= g.len(), "cycle from cell {i}");
        }
        if steps == 0 {
            assert!(d.ocean[i] || d.basins.binary_search(&(i as u32)).is_ok(), "stray terminal {i}");
            terminal_area += d.discharge_m2[i];
        }
    }
    let total = g.total_area_m2();
    assert!((terminal_area - total).abs() / total < 1e-9, "terminal area {terminal_area} of {total}");
    assert_eq!(d.order.len(), g.len());
}

#[test]
fn receiver_chains_terminate_and_conserve_area() {
    let g = grid(64);
    for class in [PlanetClass::Wet, PlanetClass::Icy, PlanetClass::Airless] {
        let d = solve(&g, &params(class, 7));
        assert_chains_terminate(&g, &d);
        for i in 0..g.len() {
            assert!(d.filled[i] >= d.height[i], "{class:?} cell {i} filled below ground");
        }
    }
}

#[test]
fn closed_sphere_without_ocean_keeps_the_lowest_cell() {
    let g = grid(64);
    let mut p = params(PlanetClass::Wet, 7);
    p.sea_level_m = -1.0e9;
    let d = solve(&g, &p);
    assert!(d.ocean.iter().all(|&o| !o));
    assert!(!d.basins.is_empty());
    assert_chains_terminate(&g, &d);
}

#[test]
fn a_river_crosses_a_face_edge() {
    let g = grid(64);
    let d = solve(&g, &params(PlanetClass::Wet, 7));
    let mean = g.total_area_m2() / g.len() as f64;
    let biggest = (0..g.len())
        .filter(|&i| g.face_of(i) != g.face_of(d.receiver[i] as usize) && !d.ocean[i])
        .map(|i| d.discharge_m2[i] / mean)
        .fold(0.0f64, f64::max);
    assert!(biggest >= 40.0, "largest cross-edge river drains {biggest} cells");
}

#[test]
fn classes_differ_in_base_level() {
    let g = grid(64);
    let wet = solve(&g, &params(PlanetClass::Wet, 7));
    let icy = solve(&g, &params(PlanetClass::Icy, 7));
    let airless = solve(&g, &params(PlanetClass::Airless, 7));
    assert!(wet.ocean.iter().any(|&o| o), "wet planet has no ocean");
    assert!(airless.ocean.iter().all(|&o| !o), "airless planet has an ocean");
    assert!(airless.lake.iter().all(|&l| !l), "airless basins are never filled");
    assert!(icy.basins.len() >= wet.basins.len());
    assert!(
        airless.basins.len() > wet.basins.len(),
        "airless {} basins, wet {}",
        airless.basins.len(),
        wet.basins.len()
    );
}

#[test]
fn solve_is_deterministic() {
    let g = grid(64);
    let p = params(PlanetClass::Icy, 11);
    let a = solve(&g, &p);
    let b = solve(&g, &p);
    assert_eq!(a.receiver, b.receiver);
    let bits = |d: &Drainage| d.discharge_m2.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a), bits(&b));
}

#[test]
fn solve_heights_rejects_mismatched_input() {
    let g = grid(64);
    let p = params(PlanetClass::Wet, 1);
    assert!(solve_heights(&g, &[0.0; 3], &p).is_none());
    let mut h = elevation(&g, &p);
    assert_eq!(solve_heights(&g, &h, &p), Some(solve(&g, &p)));
    h[0] = f64::NAN;
    assert!(solve_heights(&g, &h, &p).is_none());
}

#[test]
#[ignore = "timing probe; run with --ignored --nocapture"]
fn sphere_solve_timing_n128() {
    let start = std::time::Instant::now();
    let g = grid(128);
    let built = start.elapsed();
    let d = solve(&g, &params(PlanetClass::Wet, 7));
    let total = start.elapsed();
    assert_eq!(d.receiver.len(), 98_304);
    println!("n=128: grid {built:?}, grid + solve {total:?}, {} basins", d.basins.len());
}
