use super::planet::{Planet, Progress, build};
use super::*;

const TROUGHS: PlanetParams = PlanetParams { cells: 128, ..PLANET_FACE };
const LANDFORMS: PlanetParams = PlanetParams { glacial_landforms: true, ..TROUGHS };

fn pair(seed: u32) -> (Planet, Planet) {
    (build(seed, &TROUGHS, &Progress::default()), build(seed, &LANDFORMS, &Progress::default()))
}

#[test]
fn historical_solves_record_no_ice_cover() {
    let (troughs, _) = pair(5);
    assert!(troughs.ice.iter().all(|&c| c == 0.0));
}

#[test]
fn landforms_quarry_cirques_and_leave_moraines() {
    let (troughs, landforms) = pair(5);
    let covered = landforms.ice.iter().filter(|&&c| c > 0.2).count();
    assert!(covered > 20, "glaciers on the cold ranges: {covered}");
    // Cirques and moraines move ground both ways against the trough-only solve.
    let (mut deeper, mut raised) = (0, 0);
    for i in 0..landforms.grid.len() {
        if landforms.ice[i] <= 0.0 {
            continue;
        }
        let d = landforms.height[i] - troughs.height[i];
        deeper += usize::from(d < -2.0);
        raised += usize::from(d > 1.0);
    }
    assert!(deeper > 0 && raised > 0, "cirques {deeper}, moraines {raised}");
}

#[test]
fn erratics_lie_on_former_ice_and_nowhere_else() {
    let (_, p) = pair(5);
    let g = &p.grid;
    let (mut on_ice, mut off_ice) = (0, 0);
    for i in (0..g.len()).step_by(3) {
        let [x, z] = g.at(i);
        for k in 0..16 {
            let (px, pz) = (x + (k % 4) as f64 * 9.0, z + (k / 4) as f64 * 9.0);
            if erratic::height(&p, 5, px, pz) > 0.0 {
                if g.bilinear(&p.ice, px, pz) > 0.0 { on_ice += 1 } else { off_ice += 1 }
            }
        }
    }
    assert!(on_ice > 0, "ice cover holds erratics");
    assert_eq!(off_ice, 0);
}

#[test]
fn snouts_leave_outwash_and_kettles() {
    let (troughs, p) = pair(5);
    // Below the ice, ground is raised by outwash and pitted by kettles.
    let (mut fan, mut pits) = (0, 0);
    for i in 0..p.grid.len() {
        if p.ice[i] > 0.0 {
            continue;
        }
        let d = p.height[i] - troughs.height[i];
        fan += usize::from(d > 0.5);
        pits += usize::from(d < -1.0);
    }
    assert!(fan > 0 && pits > 0, "outwash {fan}, kettles {pits}");
}

#[test]
fn pingos_are_cones_with_ice_cores_only_in_cold_ground() {
    let probe = |maat: f64| {
        (0..400).filter_map(|i| pingo::at(2, (i % 20) as f64 * 16.0, (i / 20) as f64 * 16.0, maat)).collect::<Vec<_>>()
    };
    assert!(probe(-3.0).is_empty(), "no cores in warm permafrost");
    let cold: Vec<_> = (0..64).flat_map(|s| {
        (0..64).filter_map(move |i| pingo::at(s, (i % 8) as f64 * 40.0, (i / 8) as f64 * 40.0, -12.0))
    }).collect();
    assert!(!cold.is_empty());
    assert!(cold.iter().all(|p| p.height > 0.0 && p.core < p.height));
    assert!(cold.iter().any(|p| p.core > 1.0));
}
