use super::*;
use engine_content::{BlockId as B, SIM_RULES};
use engine_core::hash::hash_u32;
use engine_sim::{VOLUME_FULL, block_rule, split_voxel, step};

#[test]
fn the_wgsl_validates() {
    for entry in [OFFLOAD_WGSL, crate::resident::RESIDENT_WGSL] {
        let source = program(entry);
        let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

#[test]
fn the_shader_offsets_match_the_packer() {
    for (name, value) in [("MAT_BASE", MAT_BASE), ("REACTIVE_BASE", REACTIVE_BASE), ("UNARY_BASE", UNARY_BASE), ("PAIR_BASE", PAIR_BASE), ("ROW_BASE", ROW_BASE)] {
        assert!(RULE_WGSL.contains(&format!("const {name}: u32 = {value}u;")), "{name} = {value} in block_rule.wgsl");
    }
}

#[test]
fn cells_and_material_rows_pack_field_by_field() {
    let c = Cell { material: 7, volume: 33, temperature: 61_234, pressure: -2, charge: 300 };
    assert_eq!(unpack_cell(pack_cell(c)), c);
    let words = pack_rules(&SIM_RULES);
    let water = &words[MAT_BASE + B::Water as usize * 8..][..8];
    let row = B::Water.sim();
    assert_eq!(water[0], row.density);
    assert_eq!(water[3] >> 16, row.boil as u32);
    assert_eq!((water[5] >> 16) & 0xFF, row.class as u32);
    assert_eq!(water[7] & 0xFF, row.boils_into.0 as u32);
    assert_eq!(words.len(), ROW_BASE + 4 * SIM_RULES.reactions().rows().len());
    let lava_water = (B::Lava as usize) * 256 + B::Water as usize;
    let entry = words[PAIR_BASE + lava_water / 4] >> (8 * (lava_water % 4)) & 0xFF;
    assert_ne!(entry, 0, "lava + water is in the packed pair index");
}

fn device() -> (wgpu::Device, wgpu::Queue) {
    let adapter = engine_gpu::test_adapter();
    pollster::block_on(adapter.request_device(&Default::default())).expect("device")
}

/// Materials that move, conduct and react on the real tables, hot and cold.
const PALETTE: [B; 12] = [B::Air, B::Stone, B::Sand, B::Water, B::Smoke, B::Lava, B::Ice, B::Steam, B::Log, B::Fire, B::Ash, B::Dirt];

fn random_cell(h: u32) -> Cell {
    let b = PALETTE[(h % PALETTE.len() as u32) as usize];
    if b == B::Air {
        return Cell::EMPTY;
    }
    let fluid = matches!(b, B::Water | B::Smoke | B::Lava | B::Steam);
    let volume = if fluid { 1 + (h >> 8) as u8 % VOLUME_FULL } else { VOLUME_FULL };
    // 250 K to 1450 K: across freezing, boiling, condensing and ignition points.
    Cell { volume, temperature: 2500 + (h >> 16) as u16 % 12_000, pressure: (h >> 3) as i16, charge: (h >> 9) as i16, ..Cell::filled(b as u8) }
}

/// Every block on its own: random cells, phase counters (heat and not, first and second phase) and
/// anchors, including negative ones. A mismatch names the block.
#[test]
#[ignore = "needs a GPU: just test-gpu"]
fn gpu_block_rule_matches_the_cpu_per_block() {
    let (device, queue) = device();
    let gpu = GpuBlockRule::new(device, queue, &SIM_RULES);
    let n = 200_000u32;
    let anchors: Vec<IVec3> = (0..n).map(|k| IVec3::new((hash_u32(k) % 512) as i32 - 256, (hash_u32(k ^ 77) % 64) as i32 - 32, (hash_u32(k ^ 99) % 512) as i32 - 256)).collect();
    let cells: Vec<[Cell; 8]> = (0..n).map(|k| std::array::from_fn(|i| random_cell(hash_u32(k * 8 + i as u32)))).collect();
    for phase_counter in [0u32, 1, 2, 3, 8, 9, 1_000_001] {
        let seed = hash_u32(phase_counter);
        let (after, outcomes) = gpu.run(seed, phase_counter, &anchors, &cells).expect("gpu run");
        for k in 0..n as usize {
            let mut cpu = cells[k];
            let out = block_rule(&SIM_RULES, &mut cpu, seed, phase_counter, anchors[k]);
            assert_eq!((after[k], outcomes[k]), (cpu, out), "block {k} at {} in phase {phase_counter}: input {:?}", anchors[k], cells[k]);
        }
    }
}

/// The resident world against the CPU reference: every tick the awake set, at the end every cell.
#[test]
#[ignore = "needs a GPU: just test-gpu"]
fn gpu_resident_equivalence() {
    let (device, queue) = device();
    // Gas rises and liquid falls one cell per phase with no floor: 40 ticks reach 80 cells either way.
    let window = Window { origin: IVec3::new(-2, -4, -2), size: glam::UVec3::new(4, 8, 4), capacity: 2000 };
    for seed in 0..64 {
        let mut cpu = equivalence_world(seed);
        let gpu = GpuWorld::new(device.clone(), queue.clone(), &SIM_RULES, window, &cpu).expect("upload");
        for tick in 0..40 {
            step(&mut cpu, &SIM_RULES, seed, tick);
            gpu.step(seed, tick);
            let snapshot = gpu.read_back().expect("read back");
            if snapshot.overflow != 0 {
                let mut outside = Vec::new();
                for chunk in cpu.chunks() {
                    let c = chunk - window.origin;
                    if c.cmplt(IVec3::ZERO).any() || c.as_uvec3().cmpge(window.size).any() {
                        cpu.for_each_cell(chunk, |p, cell| if cell != Cell::EMPTY { outside.push((chunk, p, cell.material)) });
                    }
                }
                panic!("seed {seed} tick {tick}: overflow {}; cpu cells outside the window: {:?}", snapshot.overflow, &outside[..outside.len().min(4)]);
            }
            assert_eq!(cpu.dirty_bricks(), snapshot.dirty, "seed {seed} tick {tick}: awake bricks");
        }
        let snapshot = gpu.read_back().expect("read back");
        let differs = equivalence_region().find(|&p| {
            let (chunk, local) = split_voxel(p);
            cpu.get(chunk, local) != snapshot.get(p)
        });
        assert_eq!(differs, None, "seed {seed}: first differing cell");
    }
}

fn equivalence_world(seed: u32) -> BrickStore {
    let mut s = BrickStore::new(600);
    for x in -10..10 {
        for y in -6..10 {
            for z in -10..10 {
                let h = hash_u32(seed ^ hash_u32((x + 64) as u32 | ((y + 64) as u32) << 8 | ((z + 64) as u32) << 16));
                if h.is_multiple_of(3) {
                    let (chunk, local) = split_voxel(IVec3::new(x, y, z));
                    s.set(chunk, local, random_cell(h >> 2)).expect("pool room");
                }
            }
        }
    }
    s
}

fn equivalence_region() -> impl Iterator<Item = IVec3> {
    (-24..24).flat_map(|x| (-100..100).flat_map(move |y| (-24..24).map(move |z| IVec3::new(x, y, z))))
}

/// A pool too small for the world reports overflow (bit 1) and the GPU neither hangs nor writes out of
/// bounds; a world that leaves its window reports bit 0.
#[test]
#[ignore = "needs a GPU: just test-gpu"]
fn gpu_resident_overflow_is_reported() {
    let (device, queue) = device();
    let world = equivalence_world(3);
    let tight = Window { origin: IVec3::new(-2, -4, -2), size: glam::UVec3::new(4, 8, 4), capacity: world.bricks_in_use() as u32 + 1 };
    let gpu = GpuWorld::new(device.clone(), queue.clone(), &SIM_RULES, tight, &world).expect("upload");
    for tick in 0..40 {
        gpu.step(3, tick);
    }
    assert_eq!(gpu.read_back().expect("read back").overflow & 2, 2, "the pool ran out");
    let small = Window { origin: IVec3::splat(-1), size: glam::UVec3::splat(2), capacity: 2000 };
    let gpu = GpuWorld::new(device.clone(), queue.clone(), &SIM_RULES, small, &world).expect("upload");
    for tick in 0..40 {
        gpu.step(3, tick);
    }
    assert_eq!(gpu.read_back().expect("read back").overflow & 1, 1, "matter fell out of the window");
    let bad = Window { size: glam::UVec3::ZERO, ..small };
    assert_eq!(GpuWorld::new(device, queue, &SIM_RULES, bad, &world).err(), Some(UploadError::BadWindow));
}

/// The acceptance test (docs/PLAN2.md A5): 64 seeds of random worlds, every tick compared byte for byte
/// with the CPU reference, cells and awake bricks both.
#[test]
#[ignore = "needs a GPU: just test-gpu"]
fn gpu_ca_equivalence() {
    let (device, queue) = device();
    let gpu = GpuBlockRule::new(device, queue, &SIM_RULES);
    let world = |seed: u32| {
        let mut s = BrickStore::new(600);
        for x in -10..10 {
            for y in -6..10 {
                for z in -10..10 {
                    let h = hash_u32(seed ^ hash_u32((x + 64) as u32 | ((y + 64) as u32) << 8 | ((z + 64) as u32) << 16));
                    if h.is_multiple_of(3) {
                        let (chunk, local) = split_voxel(IVec3::new(x, y, z));
                        s.set(chunk, local, random_cell(h >> 2)).expect("pool room");
                    }
                }
            }
        }
        s
    };
    let cells = |s: &BrickStore| -> Vec<Cell> {
        (-12..12)
            .flat_map(|x| (-8..24).flat_map(move |y| (-12..12).map(move |z| IVec3::new(x, y, z))))
            .map(|p| {
                let (chunk, local) = split_voxel(p);
                s.get(chunk, local)
            })
            .collect()
    };
    for seed in 0..64 {
        let (mut cpu, mut on_gpu) = (world(seed), world(seed));
        for tick in 0..40 {
            let a = step(&mut cpu, &SIM_RULES, seed, tick);
            let b = gpu.step(&mut on_gpu, seed, tick).expect("gpu step");
            assert_eq!(a.changed_blocks, b.changed_blocks, "seed {seed} tick {tick}: changed blocks");
            assert_eq!(cpu.dirty_bricks(), on_gpu.dirty_bricks(), "seed {seed} tick {tick}: awake bricks");
        }
        assert!(cells(&cpu) == cells(&on_gpu), "seed {seed}: worlds differ after 40 ticks");
    }
}

/// Timing, not a gate (the perf gate runs without a GPU): one tick over a 64³ region (262,144 cells) of
/// mixed materials with every brick awake, CPU reference against the offload GPU path. Prints medians.
#[test]
#[ignore = "needs a GPU: just test-gpu gpu_ca_timing -- --nocapture"]
fn gpu_ca_timing() {
    let (device, queue) = device();
    let gpu = GpuBlockRule::new(device, queue, &SIM_RULES);
    let region = || {
        let mut s = BrickStore::new(2000);
        for x in 0..64 {
            for y in 0..64 {
                for z in 0..64 {
                    let h = hash_u32(x | y << 8 | z << 16);
                    let (chunk, local) = split_voxel(IVec3::new(x as i32, y as i32, z as i32));
                    s.set(chunk, local, random_cell(h)).expect("pool room");
                }
            }
        }
        s
    };
    let wake_all = |s: &mut BrickStore| {
        for chunk in [IVec3::ZERO, IVec3::X, IVec3::Y, IVec3::Z, IVec3::new(1, 1, 0), IVec3::new(1, 0, 1), IVec3::new(0, 1, 1), IVec3::ONE] {
            for brick in 0..64 {
                s.wake(chunk, brick);
            }
        }
    };
    let median = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let (mut cpu_store, mut gpu_store) = (region(), region());
    let (mut cpu, mut on_gpu) = (Vec::new(), Vec::new());
    for tick in 0..9 {
        wake_all(&mut cpu_store);
        let t = std::time::Instant::now();
        step(&mut cpu_store, &SIM_RULES, 1, tick);
        cpu.push(t.elapsed().as_secs_f64() * 1e3);
        wake_all(&mut gpu_store);
        let t = std::time::Instant::now();
        gpu.step(&mut gpu_store, 1, tick).expect("gpu step");
        on_gpu.push(t.elapsed().as_secs_f64() * 1e3);
    }
    println!("[ca-timing] 262144 cells all awake: cpu {:.2} ms, gpu offload {:.2} ms (median of 9 ticks, heat every 4th)", median(cpu), median(on_gpu));
    // Where an offload phase spends its time.
    wake_all(&mut gpu_store);
    let bricks = gpu_store.take_all_dirty();
    let t = std::time::Instant::now();
    let blocks = engine_sim::gather(&gpu_store, &bricks, 0);
    let gathered = t.elapsed().as_secs_f64() * 1e3;
    let t = std::time::Instant::now();
    let (after, outcomes) = gpu.run(1, 0, &blocks.anchors, &blocks.cells).expect("gpu run");
    let ran = t.elapsed().as_secs_f64() * 1e3;
    let t = std::time::Instant::now();
    scatter(&mut gpu_store, &blocks, &after, &outcomes, &mut StepStats::default());
    let scattered = t.elapsed().as_secs_f64() * 1e3;
    println!("[ca-timing] one heat phase, {} blocks: gather {gathered:.2} ms, gpu run with transfers {ran:.2} ms, scatter {scattered:.2} ms", blocks.len());
}

/// Timing, not a gate: a resident tick with every brick awake, at the Phase A size (262,144 cells) and
/// the 1M-cell target, against the CPU reference. GPU time is per tick over 20 back-to-back ticks.
#[test]
#[ignore = "needs a GPU: just test-gpu gpu_resident_timing -- --nocapture"]
fn gpu_resident_timing() {
    let (device, queue) = device();
    for (sx, sy, sz) in [(64, 64, 64), (128, 64, 128)] {
        let mut store = BrickStore::new(4096);
        for x in 0..sx {
            for y in 0..sy {
                for z in 0..sz {
                    let (chunk, local) = split_voxel(IVec3::new(x, y, z));
                    store.set(chunk, local, random_cell(hash_u32((x | y << 8 | z << 16) as u32))).expect("pool room");
                }
            }
        }
        // A chunk of margin on every side: matter spreads and falls out of the filled region.
        let window = Window { origin: IVec3::new(-1, -2, -1), size: glam::UVec3::new(sx as u32 / 32 + 2, 6, sz as u32 / 32 + 2), capacity: 16_384 };
        let gpu = GpuWorld::new(device.clone(), queue.clone(), &SIM_RULES, window, &store).expect("upload");
        let ticks = 20u32;
        let mut cpu_ms = Vec::new();
        for tick in 0..4 {
            for chunk in store.chunks().collect::<Vec<_>>() {
                for brick in 0..64 {
                    store.wake(chunk, brick);
                }
            }
            let t = std::time::Instant::now();
            step(&mut store, &SIM_RULES, 1, tick);
            cpu_ms.push(t.elapsed().as_secs_f64() * 1e3);
        }
        gpu.wake_all();
        gpu.step(1, 0);
        gpu.finish().expect("warm up");
        let t = std::time::Instant::now();
        for tick in 1..=ticks {
            gpu.wake_all();
            gpu.step(1, tick);
        }
        gpu.finish().expect("finish");
        let gpu_ms = t.elapsed().as_secs_f64() * 1e3 / ticks as f64;
        cpu_ms.sort_by(f64::total_cmp);
        println!("[ca-timing] {} cells all awake: cpu {:.2} ms/tick (median of 4), gpu resident {:.3} ms/tick (mean of {ticks})", sx * sy * sz, cpu_ms[2], gpu_ms);
        let snapshot = gpu.read_back().expect("read back");
        assert_eq!(snapshot.overflow, 0, "{} bricks in use", snapshot.bricks_in_use());
    }
}

/// The M1 scenario on the resident GPU world: the dam is cleared on the CPU before upload, then both
/// kernels run the flood and must agree on awake bricks every eighth tick and on every cell at the end.
#[test]
#[ignore = "needs a GPU: just test-gpu gpu_dam_break"]
fn gpu_dam_break_matches_the_cpu() {
    let (device, queue) = device();
    let seed = 0xda4b;
    for (reservoir, ticks) in [(IVec3::new(12, 8, 6), 240), (engine_sim::DAM_BREAK_200K, 120)] {
        let d = engine_sim::DamBreak { reservoir, wall: B::Stone as u8, liquid: B::Water as u8, release_tick: 0 };
        let mut cpu = BrickStore::new(d.capacity());
        d.build(&mut cpu).expect("pool room");
        for action in d.actions() {
            action.apply(&mut cpu).expect("pool room");
        }
        let (lo, hi) = d.bounds();
        let (clo, _) = split_voxel(lo);
        let (chi, _) = split_voxel(hi - IVec3::ONE);
        let window = Window { origin: clo - IVec3::ONE, size: (chi - clo + IVec3::splat(3)).as_uvec3(), capacity: d.capacity() + 64 };
        let gpu = GpuWorld::new(device.clone(), queue.clone(), &SIM_RULES, window, &cpu).expect("upload");
        for tick in 0..ticks {
            step(&mut cpu, &SIM_RULES, seed, tick);
            gpu.step(seed, tick);
            if tick % 8 == 0 || tick + 1 == ticks {
                let snapshot = gpu.read_back().expect("read back");
                assert_eq!(snapshot.overflow, 0, "{reservoir} tick {tick}: overflow");
                assert_eq!(cpu.dirty_bricks(), snapshot.dirty, "{reservoir} tick {tick}: awake bricks");
            }
        }
        let snapshot = gpu.read_back().expect("read back");
        let differs = (lo.x..hi.x)
            .flat_map(|x| (lo.y..hi.y).flat_map(move |y| (lo.z..hi.z).map(move |z| IVec3::new(x, y, z))))
            .find(|&p| {
                let (chunk, local) = split_voxel(p);
                cpu.get(chunk, local) != snapshot.get(p)
            });
        assert_eq!(differs, None, "{reservoir}: first differing cell after {ticks} ticks");
    }
}
