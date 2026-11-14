//! Native edit-admission checks without a window or transport.
use super::*;
use anyhow::{Result, ensure};
use std::time::{Duration, Instant};

pub(crate) fn run(strict: bool) -> Result<()> {
    stored_reads()?;
    authority::review_pending()?;
    world::stored_samples::review()?;
    let mut failures = usize::from(!crate::simulation::review_backlog()?);
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for x in [-700_000, -32, 31, 700_000] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 7);
            let p = IVec3::new(x, (200.0 / scale.meters()) as i32, -32);
            w.set(p, BlockId::Dirt);
            w.activity = Default::default();
            for i in 0..world::activity::MAX_ACTIVE_CHUNKS {
                ensure!(w.activity.wake(IVec3::new(i as i32 * 32, 100_000, 0)), "fixture capacity");
            }
            let mut sim = crate::simulation::WorldSimulation::new();
            let ready = sim.prepare_tool_edit(&mut w, p);
            let retained = !ready && sim.tool_edit_waiting();
            if ready {
                ensure!(matches!(world::effects::replace(&mut w, p, BlockId::Dirt, BlockId::Air),
                    Err(world::effects::Failure::Deferred)), "pressure fixture did not refuse the commit");
            }
            println!("[edit-admission] activity scale={} cell={p:?} retained={retained}", scale.meters());
            failures += usize::from(!retained);
            w.activity = Default::default();
            ensure!(sim.prepare_tool_edit(&mut w, p), "edit did not resume after pressure cleared");
            let receipt = world::effects::replace(&mut w, p, BlockId::Dirt, BlockId::Air)
                .map_err(|e| anyhow::anyhow!("recovery refused: {e:?}"))?;
            receipt.publish(&w, &mut HashSet::new(), &mut HashSet::new());
            ensure!(w.get(p).is_empty(), "recovery did not commit");
        }
    }
    let mut times = Vec::new();
    for i in 0..20 {
        let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::HalfMeter, 7);
        let cfg = w.sample_config();
        let x = -700_000 + i * 32;
        let y = engine_worldgen::sample_column(&cfg, x, -32).height;
        let mut p = Player::new();
        p.pos = Vec3::new(x as f32 + 0.5, y as f32 + 1.0, -31.5);
        p.precise_pos = p.pos.as_dvec3();
        p.pitch = -1.5;
        p.flying = true;
        let hit = world::raycast_block(&w, p.eye(w.voxel_scale), p.forward_dir(), 12.0)
            .context("fixture ray missed")?.0;
        let before = w.get(hit);
        ensure!(!before.is_empty() && before != BlockId::Bedrock, "fixture not breakable: {before:?}");
        ensure!(!w.chunks.contains_key(&world::chunk_coord_from_world_block(hit)), "fixture is not cold");
        let mut history = world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS);
        let mut mining = world::mining::Progress::default();
        let mut cooldown = 0.0;
        let mut changes = BTreeMap::new();
        let mut blocks = HashSet::new();
        let mut chunks = HashSet::new();
        let tools = actions::Tools::default();
        let controls = Controls { mine: true, flying: true, pitch: -1.5, ..Default::default() };
        let start = Instant::now();
        let mut sim = crate::simulation::WorldSimulation::new();
        let mut pending = authority::Pending::default();
        for tick in 0..240 {
            let controls = Controls { mine: tick == 0, ..controls.clone() };
            authority::step(&mut w, &mut p, &controls, &tools, true, true,
                &mut cooldown, &mut mining, &mut history, STEP, &mut changes, &mut blocks, &mut chunks, &mut sim, &mut pending);
            if w.get(hit).is_empty() { break; }
            std::thread::sleep(Duration::from_millis(1));
        }
        let changed = w.get(hit).is_empty();
        println!("[edit-admission] host-cold cell={hit:?} changed={changed} elapsed_ms={:.3}", start.elapsed().as_secs_f64() * 1000.0);
        if changed {
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            ensure!(history.request(world::edit_history::Direction::Undo), "history request refused");
            let idle = Controls { mine: false, ..controls.clone() };
            authority::step(&mut w, &mut p, &idle, &tools, true, true,
                &mut cooldown, &mut mining, &mut history, STEP, &mut changes, &mut blocks, &mut chunks, &mut sim, &mut pending);
            ensure!(w.get(hit) == before, "cold edit undo did not restore the target");
            ensure!(history.request(world::edit_history::Direction::Redo), "redo request refused");
            authority::step(&mut w, &mut p, &idle, &tools, true, true,
                &mut cooldown, &mut mining, &mut history, STEP, &mut changes, &mut blocks, &mut chunks, &mut sim, &mut pending);
            ensure!(w.get(hit).is_empty(), "cold edit redo did not remove the target");
        }
        failures += usize::from(!changed);
    }
    times.sort_by(f64::total_cmp);
    if !times.is_empty() {
        let q = |p: f64| times[((times.len() as f64 * p).ceil() as usize).saturating_sub(1)];
        println!("[edit-admission] applied n={} p95_ms={:.3} p99_ms={:.3}", times.len(), q(0.95), q(0.99));
    }
    println!("[edit-admission] failed={failures}");
    ensure!(!strict || failures == 0, "{failures} edit-admission checks failed");
    Ok(())
}

fn stored_reads() -> Result<()> {
    let root = std::env::temp_dir().join(format!("axiom-edit-cache-review-{}", std::process::id()));
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    w.regions = world::regions::RegionStream::new_in(&root);
    let p = IVec3::new(-700_000, 200, -32);
    let coord = world::chunk_coord_from_world_block(p);
    w.set(p, BlockId::Dirt);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !w.regions.view.records.contains_key(&coord) {
        ensure!(Instant::now() < deadline, "disposable region write timed out");
        w.request_simulation_writeback();
        w.stream_regions(coord, 1);
        std::thread::sleep(Duration::from_millis(2));
    }
    w.chunks.remove(&coord);
    let cold = Instant::now();
    ensure!(w.get(p) == BlockId::Dirt, "cold saved read changed material");
    println!("[edit-admission] stored-read cold_ms={:.3}", cold.elapsed().as_secs_f64() * 1000.0);
    // Removing only this disposable fixture's backing path proves warm reads use decoded data.
    for entry in std::fs::read_dir(w.regions.view.root.as_ref())? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "vxr") {
            std::fs::rename(&path, path.with_extension("parked"))?;
        }
    }
    let mut times = Vec::new();
    for _ in 0..1000 {
        let start = Instant::now();
        for _ in 0..32 { ensure!(w.get(p) == BlockId::Dirt, "warm stored read touched disk"); }
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    println!("[edit-admission] stored-read warm_batches=1000 cells_per_batch=32 p95_ms={:.6} p99_ms={:.6}", times[949], times[989]);
    let version = w.regions.view.records.get(&coord).context("fixture record missing")?.version;
    w.set(p, BlockId::Stone);
    ensure!(w.get(p) == BlockId::Stone, "resident edit hidden by cached stored data");
    let deadline = Instant::now() + Duration::from_secs(5);
    while w.regions.view.records.get(&coord).is_none_or(|r| r.version == version) {
        ensure!(Instant::now() < deadline, "updated fixture write timed out");
        w.request_simulation_writeback();
        w.stream_regions(coord, 1);
        std::thread::sleep(Duration::from_millis(2));
    }
    w.chunks.remove(&coord);
    ensure!(w.get(p) == BlockId::Stone, "new stored version returned old material");
    println!("[edit-admission] stored-read resident_priority=true published_version=true");
    stored_action(&mut w, p, true)?;
    w.set(p, BlockId::Dirt);
    let version = w.regions.view.records.get(&coord).context("fixture record missing")?.version;
    let deadline = Instant::now() + Duration::from_secs(5);
    while w.regions.view.records.get(&coord).is_none_or(|r| r.version == version) {
        ensure!(Instant::now() < deadline, "survival fixture write timed out");
        w.request_simulation_writeback();
        w.stream_regions(coord, 1);
        std::thread::sleep(Duration::from_millis(2));
    }
    w.chunks.remove(&coord);
    stored_action(&mut w, p, false)?;
    Ok(())
}

fn stored_action(w: &mut World, cell: IVec3, creative: bool) -> Result<()> {
    ensure!(!w.chunks.contains_key(&world::chunk_coord_from_world_block(cell)), "stored action is not cold");
    if !creative { w.rules.mode = world::rules::GameMode::Survival; w.adventure.begin_journey(); }
    let mut p = Player::new();
    p.pos = cell.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
    p.precise_pos = p.pos.as_dvec3();
    let mut sim = crate::simulation::WorldSimulation::new();
    let mut pending = authority::Pending::default();
    let mut history = world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS);
    let mut mining = world::mining::Progress::default();
    let (mut cooldown, mut changes, mut blocks, mut chunks) = (0.0, BTreeMap::new(), HashSet::new(), HashSet::new());
    let start = Instant::now();
    let mut steps = 0;
    for tick in 0..1200 {
        let controls = Controls { mine: !creative || tick == 0, flying: creative, pitch: -1.5, ..Default::default() };
        authority::step(w, &mut p, &controls, &actions::Tools::default(), creative, true,
            &mut cooldown, &mut mining, &mut history, STEP, &mut changes, &mut blocks, &mut chunks, &mut sim, &mut pending);
        steps += 1;
        if tick == 0 && !creative { ensure!(w.get(cell) == BlockId::Dirt, "survival hold was bypassed"); }
        if w.get(cell).is_empty() { break; }
        std::thread::sleep(Duration::from_millis(1));
    }
    ensure!(w.get(cell).is_empty(), "cold stored action did not finish");
    if !creative {
        let needed = world::mining::survival_break(BlockId::Dirt, world::mining::Tool::hand()).seconds;
        ensure!(steps as f32 * STEP + 0.0001 >= needed, "survival hold completed early");
    }
    println!("[edit-admission] host-cold-stored creative={creative} changed=true steps={steps} elapsed_ms={:.3}", start.elapsed().as_secs_f64() * 1000.0);
    Ok(())
}

#[test]
fn native_edit_admission_regressions() { run(true).unwrap(); }
