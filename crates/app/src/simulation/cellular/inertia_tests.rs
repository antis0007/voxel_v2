use super::super::*;
use crate::{fluid::VoxelScalePreset, world::WorldType};

const LO: IVec3 = IVec3::new(5, 101, 5);
const HI: IVec3 = IVec3::new(16, 115, 12);

/// A sealed 11 x 14 x 7 stone tank with a 6 x 8 x 7 water block against x = 5.
fn tank() -> World {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let mut cells = Vec::new();
    for y in LO.y - 1..=HI.y {
        for z in LO.z - 1..=HI.z {
            for x in LO.x - 1..=HI.x {
                let p = IVec3::new(x, y, z);
                let inside = p.cmpge(LO).all() && p.cmplt(HI).all();
                cells.push((p, if inside { B::Air } else { B::Stone }));
            }
        }
    }
    world.set_many(&cells);
    for y in 101..109 {
        for z in LO.z..HI.z {
            for x in 5..11 {
                world.set_water_volume(IVec3::new(x, y, z), 8);
            }
        }
    }
    world
}

/// One step of every wet cell; returns the update count `step` reports.
fn sweep(world: &mut World, sources: &mut Residency) -> usize {
    let mut moved = 0;
    let (mut visited, mut blocks, mut chunks) = (HashSet::new(), HashSet::new(), HashSet::new());
    world.activity.tick = world.activity.tick.wrapping_add(1);
    for y in LO.y..HI.y {
        for z in LO.z..HI.z {
            for x in LO.x..HI.x {
                let p = IVec3::new(x, y, z);
                if world.water_volume(p) > 0 && !visited.contains(&p) {
                    moved += step(
                        sources,
                        world,
                        p,
                        (Vec3::splat(-9.0), Vec3::splat(-8.0)),
                        &mut visited,
                        &mut blocks,
                        &mut chunks,
                    );
                }
            }
        }
    }
    moved
}

/// Exact water depth of one column in cells.
fn depth(world: &World, x: i32, z: i32) -> f32 {
    let full = (world.environment.quantum() * 8) as f32;
    (LO.y..HI.y)
        .map(|y| {
            let p = IVec3::new(x, y, z);
            world.environment.exact_water(p, world.water_volume(p)) as f32
        })
        .sum::<f32>()
        / full
}

/// Exact water in the tank, in microlitres.
fn mass(world: &World) -> u64 {
    let mut total = 0;
    for y in LO.y..HI.y {
        for z in LO.z..HI.z {
            for x in LO.x..HI.x {
                let p = IVec3::new(x, y, z);
                total += world.environment.exact_water(p, world.water_volume(p));
            }
        }
    }
    total
}

/// A released dam must reach the far wall, then settle flat and stop transferring.
#[test]
fn dam_break_reaches_the_far_wall_then_settles() {
    let mut world = tank();
    let before = mass(&world);
    let rest = before as f32
        / (world.environment.quantum() * 8) as f32
        / ((HI.x - LO.x) * (HI.z - LO.z)) as f32;
    let mut sources = Residency::default();
    let mut peak = 0.0f32;
    let mut trace = Vec::new();
    for t in 0..160 {
        sweep(&mut world, &mut sources);
        let far = depth(&world, HI.x - 1, 8);
        peak = peak.max(far);
        if t % 4 == 3 {
            trace.push((far * 10.0).round() / 10.0);
        }
    }
    println!("[inertia] far wall every 4 sweeps {trace:?}");
    let mut churn = 0;
    for t in 0..1200 {
        let updates = sweep(&mut world, &mut sources);
        if t >= 1000 {
            churn += updates;
        }
    }
    let columns: Vec<f32> = (LO.x..HI.x).map(|x| depth(&world, x, 8)).collect();
    let spread = columns.iter().cloned().fold(f32::MIN, f32::max)
        - columns.iter().cloned().fold(f32::MAX, f32::min);
    println!("[inertia] rest {rest:.2} cells, far-wall peak {peak:.2}, settled spread {spread:.3}, churn {churn}");
    assert_eq!(mass(&world), before, "water mass must be conserved");
    assert!(peak >= rest * 0.5, "far wall reached {peak:.2} cells against rest {rest:.2}");
    assert!(spread <= 0.01, "surface still {spread:.3} cells uneven after 1200 sweeps");
    assert_eq!(churn, 0, "a settled pool still moved water in its last 200 sweeps");
}
