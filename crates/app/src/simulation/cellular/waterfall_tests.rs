use super::super::*;
use crate::{fluid::VoxelScalePreset, world::WorldType};

const LO: IVec3 = IVec3::new(5, 101, 5);
const HI: IVec3 = IVec3::new(16, 116, 12);
/// First air column past the shelf; the stream falls here.
const LIP_X: i32 = 9;
const SHELF_TOP: i32 = 111;

/// A sealed stone box with a shelf under x < 9 and a reservoir on the shelf.
fn falls() -> World {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let mut cells = Vec::new();
    for y in LO.y - 1..=HI.y {
        for z in LO.z - 1..=HI.z {
            for x in LO.x - 1..=HI.x {
                let p = IVec3::new(x, y, z);
                let inside = p.cmpge(LO).all() && p.cmplt(HI).all();
                let shelf = x < LIP_X && y < SHELF_TOP;
                cells.push((p, if inside && !shelf { B::Air } else { B::Stone }));
            }
        }
    }
    world.set_many(&cells);
    for y in SHELF_TOP..SHELF_TOP + 4 {
        for z in LO.z..HI.z {
            for x in LO.x..LIP_X {
                world.set_water_volume(IVec3::new(x, y, z), 8);
            }
        }
    }
    world
}

fn sweep(world: &mut World, sources: &mut Residency) {
    let (mut visited, mut blocks, mut chunks) = (HashSet::new(), HashSet::new(), HashSet::new());
    world.activity.tick = world.activity.tick.wrapping_add(1);
    for y in LO.y..HI.y {
        for z in LO.z..HI.z {
            for x in LO.x..HI.x {
                let p = IVec3::new(x, y, z);
                if world.water_volume(p) > 0 && !visited.contains(&p) {
                    step(
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
}

fn exact(world: &World, p: IVec3) -> u64 {
    world.environment.exact_water(p, world.water_volume(p))
}

fn mass(world: &World) -> u64 {
    let mut total = 0;
    for y in LO.y..HI.y {
        for z in LO.z..HI.z {
            for x in LO.x..HI.x {
                total += exact(world, IVec3::new(x, y, z));
            }
        }
    }
    total
}

/// Dry cells between the highest and lowest wet cell of the falling column,
/// above the pool that forms on the floor.
fn gaps(world: &World, x: i32, z: i32) -> (usize, usize) {
    let wet: Vec<i32> = (LO.y..SHELF_TOP)
        .filter(|&y| exact(world, IVec3::new(x, y, z)) > 0)
        .collect();
    let (Some(&low), Some(&high)) = (wet.first(), wet.last()) else {
        return (0, 0);
    };
    let span = (high - low + 1) as usize;
    (span - wet.len(), span)
}

/// A steady spill must fall as an unbroken stream. Moving a whole packet two
/// cells per tick left every other cell of the fall dry, a dashed waterfall.
#[test]
fn spill_falls_as_an_unbroken_stream() {
    let mut world = falls();
    let before = mass(&world);
    let mut sources = Residency::default();
    let (mut dry, mut span) = (0, 0);
    for t in 0..40 {
        sweep(&mut world, &mut sources);
        if t >= 12 {
            for z in LO.z..HI.z {
                let (d, s) = gaps(&world, LIP_X, z);
                dry += d;
                span += s;
            }
        }
    }
    let column: Vec<u64> = (LO.y..SHELF_TOP)
        .map(|y| exact(&world, IVec3::new(LIP_X, y, 8)) * 100 / (world.environment.quantum() * 8))
        .collect();
    println!("[waterfall] {dry} dry of {span} stream cells; x={LIP_X} z=8 fill % bottom-up {column:?}");
    assert_eq!(mass(&world), before, "water mass must be conserved");
    assert!(span > 0, "no stream formed past the lip");
    assert!(dry * 20 <= span, "{dry} of {span} stream cells were dry");
}
