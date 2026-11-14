use super::*;
use crate::{fluid::VoxelScalePreset, world::WorldType};

#[test]
fn fernwake_stop_breach_moves_finite_water_without_flooding_crossing() {
    use crate::world::{ChunkData, WorldSampleConfig};
    use engine_worldgen::fernwake::waterworks;
    use std::sync::Arc;
    let cfg = WorldSampleConfig {
    planet: None, seed: 31, generator_version: waterworks::VERSION,
        world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter };
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    w.generator_version = waterworks::VERSION;
    let offset = IVec3::new(-300,104,-30);
    // Load only the four fixture chunks; no terrain jobs, saves or renderer.
    for cx in 0..2 { for cz in 0..2 {
        let c = IVec3::new(cx,3,cz);
        let mut blocks = Box::new([B::Stone; 32*32*32]);
        for y in 0..32 { for z in 0..32 { for x in 0..32 {
            let p = c*32 + IVec3::new(x,y,z);
            blocks[ChunkData::local_index(x,y,z)] = waterworks::sample(&cfg,
                (p-offset).as_vec3()+Vec3::splat(0.5)).unwrap_or(B::Stone);
        }}}
        w.chunks.insert(c, Arc::new(ChunkData { blocks, light: Box::new([0;32*32*32]),
            torches: HashSet::new(), modified_bounds:None, structural_bounds:None }));
    }}
    let cells: Vec<_> = (-2..3).flat_map(|y| (34..63).flat_map(move |z|
        (310..333).map(move |x| IVec3::new(x,y,z)+offset))).collect();
    let mass = |w: &World| cells.iter().map(|&p| w.water_microlitres(p)).sum::<u64>();
    let before = mass(&w);
    let receiver = IVec3::new(320,-2,54)+offset;
    assert_eq!(w.water_volume(receiver),0);
    for x in 320..322 { w.set(IVec3::new(x,-1,44)+offset,B::Air); }
    let mut sources = Residency::default();
    let mut blocks = HashSet::new(); let mut chunks = HashSet::new();
    for _ in 0..1024 {
        w.activity.tick += 1;
        let mut visited = HashSet::new();
        for &p in &cells {
            if w.get(p) == B::Water && !visited.contains(&p) {
                step(&mut sources,&mut w,p,(Vec3::ZERO,Vec3::ONE),&mut visited,&mut blocks,&mut chunks);
            }
        }
    }
    assert_eq!(mass(&w),before,"stop breach must not create or destroy water");
    assert!(w.water_volume(receiver)>0,"water must reach the receiving basin");
    assert!(w.water_fill(IVec3::new(317,2,37)+offset)<0.125,"service shelf remains submerged");
    for x in 310..333 { for z in 46..51 { for y in 0..4 {
        assert_eq!(w.water_volume(IVec3::new(x,y,z)+offset),0,"crossing flooded at {x},{y},{z}");
    }}}
    // Ordinary construction can close the same opening, without a puzzle flag.
    let mut pumped = 0;
    for x in 320..322 {
        let p = IVec3::new(x,-1,44)+offset;
        // A solid cannot silently delete fluid: drain this cell first, as a
        // player pump/bucket would, before checking the rebuilt obstruction.
        pumped += w.water_microlitres(p);
        assert!(w.set_water_microlitres(p,0));
        w.set(p,B::Planks);
        assert_eq!(w.get(p),B::Planks);
    }
    assert_eq!(mass(&w)+pumped,before,"rebuilding must account for every withdrawn microlitre");
}

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
    let before = world.water_microlitres(p);
    assert_eq!(tick(&mut world, p), 4);
    assert_eq!(world.water_volume(p), 0);
    let lip = p + IVec3::X;
    assert!(world.water_microlitres(outlet) > 0);
    assert_eq!(world.water_microlitres(lip) + world.water_microlitres(outlet), before);
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
    assert_eq!(world.water_volume(IVec3::new(6, 102, 8)), 8);
    assert_eq!(world.water_volume(IVec3::new(6, 106, 8)), 4);
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
fn current_does_not_displace_water_at_equal_head() {
    let mut world = chamber();
    let p = IVec3::new(6, 108, 8);
    let q = p + IVec3::X;
    world.set_water_volume(p, 4);
    world.set_water_volume(q, 4);
    world.set_water_velocity(p, Vec2::new(1.0, 0.0));
    assert_eq!(tick(&mut world, p), 0);
    assert_eq!(world.water_volume(p), 4);
    assert_eq!(world.water_volume(q), 4);
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
    let rendered_height = crate::mesh::free_water_height(5.0 / 8.0);
    assert!((surface.y - (p.y as f32 + rendered_height)).abs() < 1e-5);
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
    old.set(q,B::Air);
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
    pair.set(q,B::Air);
    let mut committed=0;
    let start = Instant::now();
    for i in 0..N {
        if i & 1 == 0 {
            committed+=usize::from(pair.apply_water_pair(p, 0, q, 8, 8));
        } else {
            committed+=usize::from(pair.apply_water_pair(q, 0, p, 8, 8));
        }
    }
    let pair_ns = start.elapsed().as_nanos() as f64 / N as f64;
    println!(
        "[water-bench] two-cell commit: set_water_volume={old_ns:.0} ns, apply_water_pair={pair_ns:.0} ns, speedup={:.2}x",
        old_ns / pair_ns.max(1.0)
    );
    assert_eq!(pair.water_volume(p), 8);
    assert_eq!(committed,N,"benchmark must measure committed transfers, not rejected solid targets");
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
        // Per-step publication as in `fixed::finish`; the journal would
        // otherwise grow for the whole run and its cost go unmeasured.
        world.publish_changes(&mut blocks, &mut chunks);
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
        sim.update(&mut world, &player, dt, &mut blocks, &mut chunks, crate::simulation::Budget::UNBOUNDED);
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
