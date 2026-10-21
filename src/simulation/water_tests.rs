use super::*;
use crate::{fluid::VoxelScalePreset, world::WorldType};

fn chamber() -> World {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let mut cells = Vec::new();
    for y in 100..116 {
        for z in 4..13 {
            for x in 4..17 {
                cells.push((IVec3::new(x, y, z), B::Stone));
            }
        }
    }
    world.set_many(&cells);
    world
}
fn tick(world: &mut World, p: IVec3) -> usize {
    step(
        &mut Residency::default(),
        world,
        p,
        (Vec3::ZERO, Vec3::ONE),
        &mut HashSet::new(),
        &mut HashSet::new(),
        &mut HashSet::new(),
    )
}
fn mass(world: &World) -> u32 {
    let mut total = 0;
    for y in 100..116 {
        for z in 4..13 {
            for x in 4..17 {
                total += world.water_volume(IVec3::new(x, y, z)) as u32;
            }
        }
    }
    total
}
#[test]
fn last_water_quantum_drains_down_a_ledge_without_mass_loss() {
    let mut world = chamber();
    let p = IVec3::new(6, 104, 8);
    let outlet = p + IVec3::X - IVec3::Y;
    world.set_many(&[(p + IVec3::X, B::Air), (outlet, B::Air)]);
    world.set_water_volume(p, 1);
    assert_eq!(tick(&mut world, p), 2);
    assert_eq!(world.water_volume(p), 0);
    assert_eq!(world.water_volume(outlet), 1);
    assert_eq!(mass(&world), 1);
}
#[test]
fn connected_water_pushes_up_an_outlet_below_its_head() {
    let mut world = chamber();
    for x in 6..=10 {
        world.set_water_volume(IVec3::new(x, 102, 8), 8);
    }
    for y in 103..=106 {
        world.set_water_volume(IVec3::new(6, y, 8), 8);
    }
    let outlet = IVec3::new(10, 103, 8);
    world.set_many(&[(outlet, B::Air), (IVec3::new(6, 107, 8), B::Air)]);
    let before = mass(&world);
    assert_eq!(tick(&mut world, IVec3::new(6, 102, 8)), 2);
    // The hydraulic search is staggered, so a found connection moves four
    // units rather than two: enough to register downstream without starving
    // the outlet between expensive searches.
    assert_eq!(world.water_volume(outlet), 4);
    assert_eq!(mass(&world), before);
}
#[test]
fn paired_commit_is_one_revision_and_conserves_mass() {
    let mut world = chamber();
    let from = IVec3::new(6, 108, 8);
    let to = IVec3::new(7, 108, 8);
    world.set_water_volume(from, 8);
    world.set_water_volume(to, 2);
    let before = mass(&world);
    let revision = world.revision();
    world.apply_water_pair(from, 6, to, 4, 2);
    assert_eq!(world.water_volume(from), 6);
    assert_eq!(world.water_volume(to), 4);
    assert_eq!(world.revision(), revision + 1);
    assert_eq!(mass(&world), before);
}

#[test]
fn momentum_carries_equal_water_into_the_next_cell() {
    let mut world = chamber();
    let p = IVec3::new(6, 108, 8);
    let q = p + IVec3::X;
    world.set_water_volume(p, 4);
    world.set_water_volume(q, 4);
    world.set_water_velocity(p, Vec2::new(1.0, 0.0));
    assert_eq!(tick(&mut world, p), 2);
    assert_eq!(world.water_volume(p), 3);
    assert_eq!(world.water_volume(q), 5);
    assert!(world.water_velocity(q).x > 0.0);
}

#[test]
fn surface_uses_partial_volume_and_rejects_roofed_water() {
    let mut world = chamber();
    let p = IVec3::new(8, 108, 8);
    world.set_many(&[(p + IVec3::Y, B::Air)]);
    world.set_water_volume(p, 5);
    let surface = world
        .water_surface_at((p.as_vec3() + Vec3::splat(0.5)) * 1.0)
        .expect("exposed partial cell has a surface");
    assert!((surface.y - (p.y as f32 + 5.0 / 8.0)).abs() < 1e-5);
    // A stone roof is not a splash surface.
    world.set(p + IVec3::Y, B::Stone);
    assert!(
        world
            .water_surface_at((p.as_vec3() + Vec3::splat(0.5)) * 1.0)
            .is_none()
    );
}

#[test]
#[ignore = "transaction microbenchmark; run explicitly in release"]
fn water_pair_commit_microbenchmark() {
    use std::time::Instant;
    const N: usize = 50_000;
    let p = IVec3::new(6, 108, 8);
    let q = IVec3::new(7, 108, 8);
    let mut old = chamber();
    old.set_water_volume(p, 8);
    let start = Instant::now();
    for i in 0..N {
        if i & 1 == 0 {
            old.set_water_volume(p, 0);
            old.set_water_volume(q, 8);
        } else {
            old.set_water_volume(q, 0);
            old.set_water_volume(p, 8);
        }
    }
    let old_ns = start.elapsed().as_nanos() as f64 / N as f64;

    let mut pair = chamber();
    pair.set_water_volume(p, 8);
    let start = Instant::now();
    for i in 0..N {
        if i & 1 == 0 {
            pair.apply_water_pair(p, 0, q, 8, 8);
        } else {
            pair.apply_water_pair(q, 0, p, 8, 8);
        }
    }
    let pair_ns = start.elapsed().as_nanos() as f64 / N as f64;
    println!(
        "[water-bench] two-cell commit: set_water_volume={old_ns:.0} ns, apply_water_pair={pair_ns:.0} ns, speedup={:.2}x",
        old_ns / pair_ns.max(1.0)
    );
    assert_eq!(pair.water_volume(p), 8);
}

#[test]
fn pressure_does_not_cross_a_solid_wall() {
    let mut world = chamber();
    for y in 102..=106 {
        world.set_water_volume(IVec3::new(6, y, 8), 8);
    }
    let outlet = IVec3::new(8, 103, 8);
    world.set_many(&[(outlet, B::Air), (IVec3::new(6, 107, 8), B::Air)]);
    let before = mass(&world);
    assert_eq!(tick(&mut world, IVec3::new(6, 102, 8)), 0);
    assert_eq!(world.water_volume(outlet), 0);
    assert_eq!(mass(&world), before);
}

/// `cargo test --release dam_break_throughput -- --ignored --nocapture`
#[test]
#[ignore]
fn dam_break_throughput() {
    let mut world = chamber();
    // Hollow the chamber and drop a 6x8x7 block of water against one wall.
    let mut air = Vec::new();
    for y in 101..115 {
        for z in 5..12 {
            for x in 5..16 {
                air.push((IVec3::new(x, y, z), B::Air));
            }
        }
    }
    world.set_many(&air);
    for y in 101..109 {
        for z in 5..12 {
            for x in 5..11 {
                world.set_water_volume(IVec3::new(x, y, z), 8);
            }
        }
    }
    let before = mass(&world);
    let mut sources = Residency::default();
    let (mut visited, mut blocks, mut chunks) = (HashSet::new(), HashSet::new(), HashSet::new());
    let start = std::time::Instant::now();
    let (mut steps, mut updates) = (0usize, 0usize);
    for t in 0..400u32 {
        world.activity.tick = t as _;
        visited.clear();
        for y in 101..115 {
            for z in 5..12 {
                for x in 5..16 {
                    let p = IVec3::new(x, y, z);
                    if world.water_volume(p) > 0 && !visited.contains(&p) {
                        updates += step(
                            &mut sources,
                            &mut world,
                            p,
                            (Vec3::splat(-9.0), Vec3::splat(-8.0)),
                            &mut visited,
                            &mut blocks,
                            &mut chunks,
                        );
                        steps += 1;
                    }
                }
            }
        }
        blocks.clear();
        chunks.clear();
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "{steps} cell steps, {updates} updates in {:.1} ms: {:.0} ns/step",
        elapsed * 1e3,
        elapsed * 1e9 / steps as f64
    );
    assert_eq!(mass(&world), before, "water mass must be conserved");
}

/// One sweep over every wet cell in the chamber, the same loop the 30 Hz world
/// tick runs once per batch. Returns the number of sweeps taken.
fn sweep(world: &mut World, sources: &mut Residency) {
    let (mut visited, mut blocks, mut chunks) = (HashSet::new(), HashSet::new(), HashSet::new());
    world.activity.tick = world.activity.tick.wrapping_add(1);
    for y in 101..115 {
        for z in 5..12 {
            for x in 5..16 {
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

/// Regression guard for "water moves too slowly". A column of water on a flat
/// floor must spread across a 6-cell-wide chamber within a bounded number of
/// sweeps, and must conserve mass while it does.
#[test]
fn flow_front_reaches_the_far_side_within_bounded_sweeps() {
    let mut world = chamber();
    let mut air = Vec::new();
    for y in 101..115 {
        for z in 5..12 {
            for x in 5..16 {
                air.push((IVec3::new(x, y, z), B::Air));
            }
        }
    }
    world.set_many(&air);
    for z in 5..12 {
        for x in 5..7 {
            world.set_water_volume(IVec3::new(x, 101, z), 8);
        }
    }
    let before = mass(&world);
    let mut sources = Residency::default();
    let mut sweeps = 0;
    while sweeps < 60 {
        sweep(&mut world, &mut sources);
        sweeps += 1;
        if (11..16).any(|x| world.water_volume(IVec3::new(x, 101, 8)) > 0) {
            break;
        }
    }
    println!("[water-front] reached x=11 after {sweeps} sweeps");
    assert!(
        (11..16).any(|x| world.water_volume(IVec3::new(x, 101, 8)) > 0),
        "front did not reach x=11 within 60 sweeps"
    );
    assert!(sweeps <= 30, "front took {sweeps} sweeps to cover 6 cells");
    assert_eq!(mass(&world), before, "water mass must be conserved");
}

/// Front speed through the real scheduler: `WorldSimulation::update` at 60 Hz,
/// the same call the game makes each frame. The column is released at t=0 and
/// the test records when the front first reaches each x. One cell per 80 ms
/// tick is the ceiling. Before the wake filter and the whole-chunk snapshot,
/// 2.8k inert edit neighbours queued ahead of the front and it reached x=15
/// only after 1.8 s.
#[test]
fn scheduler_front_keeps_pace_without_backlog_stall() {
    use crate::{fluid::VoxelScalePreset as Scale, player::Player};
    let mut world = chamber();
    let mut air = Vec::new();
    for y in 101..115 {
        for z in 5..12 {
            for x in 5..16 {
                air.push((IVec3::new(x, y, z), B::Air));
            }
        }
    }
    world.set_many(&air);
    for z in 5..12 {
        for x in 5..7 {
            world.set_water_volume(IVec3::new(x, 101, z), 8);
        }
    }
    let mut sim = super::super::WorldSimulation::new();
    sim.reset_for_world(Scale::OneMeter);
    let mut player = Player::new();
    player.pos = Vec3::new(-20.0, 100.0, 8.0);
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let dt = 1.0 / 60.0;
    let mut reached = [None::<f32>; 16];
    let mut t = 0.0f32;
    for _frame in 0..(6.0 / dt) as usize {
        sim.update(&mut world, &player, dt, &mut blocks, &mut chunks);
        t += dt;
        for x in 6..16 {
            if reached[x].is_none() && world.water_volume(IVec3::new(x as i32, 101, 8)) > 0 {
                reached[x] = Some(t);
            }
        }
    }
    for x in 6..16 {
        match reached[x] {
            Some(s) => println!("[scheduler-front] x={x} reached at {s:.3} s"),
            None => println!("[scheduler-front] x={x} not reached"),
        }
    }
    let far = reached[15].expect("front never reached x=15 within 6 s");
    assert!(far <= 1.2, "front reached x=15 at {far:.3} s; backlog is stalling it");
}
