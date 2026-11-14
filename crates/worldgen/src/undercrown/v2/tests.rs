use super::*;
use super::tests_grid::Grid;
use crate::WorldType;
use crate::undercrown::plan::Plan;
use engine_core::VoxelScalePreset;
use std::collections::HashSet;

fn cfg(seed: u32) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version: super::tests_grid::generator(), seed, world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter }
}

fn plan(seed: u32) -> (WorldSampleConfig, Plan) {
    let c = cfg(seed);
    let plan = Plan::for_config(&c);
    (c, plan)
}

fn start(plan: &Plan) -> IVec3 {
    (plan.anchor + Vec3::new(-4.0, 0.0, -6.0)).floor().as_ivec3()
}

fn reachable_rooms(c: &WorldSampleConfig, plan: &Plan, mech: Mech, from: IVec3) -> Vec<&'static str> {
    let layout = plan.v2.as_ref().unwrap();
    let grid = Grid::new(c, layout, mech);
    grid.rooms(plan, &grid.reach_list(from))
}

pub(super) const ALWAYS: [&str; 22] = [
    "Ruined well", "First landing", "Counterweight hall", "Broken stair", "Service corridor",
    "Door behind the first camp", "Pilgrims' kitchen", "Chapel growing through the prison",
    "Lantern orchard mouth", "Seed bank", "Crown Chasm", "Bridge village", "Bellkeeper's crown",
    "Stone giant's pocket", "Giant's listening stair", "Hall of patient bells", "Choir antechamber",
    "Freight cradle", "Observatory stair", "Clifftop observatory", "Chain lift", "Furnace mouth",
];
pub(super) const BOROUGH: [&str; 7] = [
    "Street beneath the air bell", "Flooded courthouse", "Reservoir control", "Ferry landing",
    "Maintenance duct", "Bathhouse of reverse flow", "Aqueduct inside a house",
];

#[test]
fn the_first_descent_reaches_everything_but_the_borough_and_the_archive() {
    for seed in [424242u32, 7, 99, 1, 2, 3, 1234, 555] {
        let (c, plan) = plan(seed);
        let got = reachable_rooms(&c, &plan, Mech::default(), start(&plan));
        for name in ALWAYS {
            assert!(got.contains(&name), "seed {seed}: {name} unreachable with no mechanisms; reached {got:?}");
        }
        for name in BOROUGH.iter().chain(&["Glass Archive"]) {
            assert!(!got.contains(name), "seed {seed}: {name} reachable before its gate");
        }
    }
}

#[test]
fn the_winch_opens_the_borough_and_lens_plus_a_crossing_opens_the_archive() {
    let (c, plan) = plan(424242);
    let s = start(&plan);
    let got = reachable_rooms(&c, &plan, Mech::WINCH, s);
    for name in BOROUGH {
        assert!(got.contains(&name), "{name} unreachable with the winch wound");
    }
    assert!(!reachable_rooms(&c, &plan, Mech::LENS, s).contains(&"Glass Archive"), "lava must stop the archive");
    assert!(!reachable_rooms(&c, &plan, Mech::COOLING, s).contains(&"Glass Archive"), "the door must stop the archive");
    assert!(reachable_rooms(&c, &plan, Mech::LENS.with(Mech::COOLING), s).contains(&"Glass Archive"));
    assert!(reachable_rooms(&c, &plan, Mech::LENS.with(Mech::FREIGHT), s).contains(&"Glass Archive"));
}

#[test]
fn the_chasm_is_one_way_until_the_bell_restores_the_stair() {
    let (c, plan) = plan(424242);
    let chasm = plan.rooms.iter().find(|r| r.name == "Crown Chasm").unwrap();
    let floor = Vec3::new((chasm.x0 + chasm.x1) * 0.5, chasm.floor + 1.0, (chasm.z0 + chasm.z1) * 0.5).floor().as_ivec3();
    let before = reachable_rooms(&c, &plan, Mech::default(), floor);
    assert!(!before.contains(&"First landing"), "the landing is reachable from the chasm without the bell");
    let after = reachable_rooms(&c, &plan, Mech::BELL, floor);
    assert!(after.contains(&"First landing"), "the restored stair does not reach the landing");
    let repaired = reachable_rooms(&c, &plan, Mech::STAIR, floor);
    assert!(repaired.contains(&"First landing"), "the repaired broken stair does not climb back");
}

#[test]
fn every_prop_can_be_reached_from_a_standing_spot() {
    let (c, plan) = plan(424242);
    let layout = plan.v2.as_ref().unwrap();
    let grid = Grid::new(&c, layout, Mech::ALL);
    let seen = grid.reach(start(&plan));
    for prop in &plan.props {
        let near = seen.iter().any(|p| {
            let eye = p.as_vec3() + Vec3::new(0.5, 1.6, 0.5);
            (eye - prop.pos).length() < 3.2
        });
        assert!(near, "{} at {:?} has no reachable spot within reach", prop.label, prop.pos);
    }
}

#[test]
fn mechanism_edits_are_bounded_and_idempotent() {
    let (c, plan) = plan(424242);
    let layout = plan.v2.as_ref().unwrap();
    for mech in [Mech::STAIR, Mech::BRIDGE, Mech::BELL, Mech::WINCH, Mech::SLUICE, Mech::COOLING, Mech::LENS, Mech::FREIGHT] {
        let edits = layout.edits(&c, Mech::default(), mech);
        assert!(!edits.is_empty(), "{mech:?} changes nothing");
        assert!(edits.len() < 6000, "{mech:?} rewrites {} cells", edits.len());
        assert!(layout.edits(&c, mech, mech).is_empty());
        for (p, b) in &edits {
            // Cells the new state leaves to nature get the natural block.
            let want = layout.sample(&c, p.as_vec3() + Vec3::splat(0.5), mech).unwrap_or_else(|| crate::sample_block(&c, *p));
            assert_eq!(want, *b, "{mech:?} at {p:?}");
        }
    }
    let drained = layout.edits(&c, Mech::default(), Mech::SLUICE);
    assert!(drained.iter().all(|(_, b)| *b == B::Air), "the sluice only drains");
}

#[test]
fn ids_match_the_generator_19_plan_and_the_archive_is_new() {
    let c = cfg(crate::test_world::seed());
    let old = Plan::for_config(&WorldSampleConfig { generator_version: 31, ..c });
    let new = Plan::for_config(&c);
    let ids = |p: &Plan| p.rooms.iter().map(|r| (r.id.raw(), r.name)).collect::<HashSet<_>>();
    let (a, b) = (ids(&old), ids(&new));
    assert!(a.is_subset(&b), "generator 32 drops rooms: {:?}", a.difference(&b).collect::<Vec<_>>());
    assert_eq!(b.len(), a.len() + 1);
    let props = |p: &Plan| p.props.iter().map(|q| (q.id.raw(), format!("{:?}", q.kind))).collect::<HashSet<_>>();
    assert!(props(&old).is_subset(&props(&new)));
    assert!(old.v2.is_none() && new.v2.is_some());
}

#[test]
fn the_archive_is_sealed_in_tombstone() {
    let (c, plan) = plan(424242);
    let layout = plan.v2.as_ref().unwrap();
    let archive = plan.rooms.iter().find(|r| r.name == "Glass Archive").unwrap();
    let z = (archive.z0 + archive.z1) * 0.5;
    for y in [archive.floor + 1.0, archive.floor + 4.0] {
        assert_eq!(layout.sample(&c, Vec3::new(archive.x0 - 0.5, y, z), Mech::default()), Some(B::TombStone));
        assert_eq!(layout.sample(&c, Vec3::new(archive.x1 + 0.5, y, z), Mech::default()), Some(B::TombStone));
    }
}

/// One character per block for the dumps.
fn glyph(b: B) -> char {
    match b {
        B::Air => '.',
        B::Water => '~',
        B::Lava => 'L',
        B::Planks | B::Log => '=',
        B::GlowCap | B::Crystal | B::StarGlass => '*',
        b if b.is_torch() => 't',
        b if b.is_solid() => '#',
        _ => '?',
    }
}

/// The dumps' plan: `UC_SEED` (default 424242), `UC_HALF=1` for 0.5 m voxels.
fn dump_plan_of() -> (WorldSampleConfig, Plan) {
    let seed = std::env::var("UC_SEED").map_or(424242, |s| s.parse().unwrap());
    let scale = if std::env::var("UC_HALF").is_ok() { VoxelScalePreset::HalfMeter } else { VoxelScalePreset::OneMeter };
    super::tests_grid::plan_scaled(seed, scale)
}

/// `UC_SLICE=axis(x|z),at,a0,a1,y0,y1,mech` in metres from the anchor, one
/// character per voxel; reached cells (from the landing, with that state)
/// print as `o`.
#[test]
#[ignore = "debug dump"]
fn dump_slice() {
    let a: Vec<String> = std::env::var("UC_SLICE").unwrap().split(',').map(String::from).collect();
    let f = |i: usize| a[i].parse::<f32>().unwrap();
    let (c, plan) = dump_plan_of();
    let layout = plan.v2.as_ref().unwrap();
    let grid = Grid::new(&c, layout, Mech(f(6) as u16));
    let seen = grid.reach(super::tests_grid::start_cell(&plan, &grid));
    println!("anchor {:?}", plan.anchor);
    let v = grid.v;
    let mut y = f(5);
    while y >= f(4) {
        let mut line = format!("{y:>6.1} ");
        let mut k = f(2);
        while k < f(3) {
            let off = if a[0] == "z" { Vec3::new(k, y, f(1)) } else { Vec3::new(f(1), y, k) };
            let p = grid.cell(plan.anchor + off);
            line.push(if seen.contains(&p) { 'o' } else { glyph(grid.get(p)) });
            k += v;
        }
        println!("{line}");
        y -= v;
    }
}

/// `UC_PLAN=height,x0,x1,z0,z1,mech`: top-down rows of the layer at
/// `height` metres over the anchor, one character per voxel.
#[test]
#[ignore = "debug dump"]
fn dump_plan() {
    let a: Vec<f32> = std::env::var("UC_PLAN").unwrap().split(',').map(|s| s.parse().unwrap()).collect();
    let (c, plan) = dump_plan_of();
    let layout = plan.v2.as_ref().unwrap();
    let grid = Grid::new(&c, layout, Mech(a[5] as u16));
    let seen = grid.reach(super::tests_grid::start_cell(&plan, &grid));
    let v = grid.v;
    let mut z = a[3];
    while z < a[4] {
        let mut line = format!("{z:>6.1} ");
        let mut x = a[1];
        while x < a[2] {
            let p = grid.cell(plan.anchor + Vec3::new(x, a[0], z));
            line.push(if seen.contains(&p) { 'o' } else { glyph(grid.get(p)) });
            x += v;
        }
        println!("{line}");
        z += v;
    }
}
