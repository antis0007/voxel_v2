//! Runs the native gameplay loop without opening the renderer.
use super::*;
use anyhow::{Result, ensure};
use std::path::Path;

pub(crate) fn preview(w: &mut World, p: &mut crate::player::Player, ticks: usize) {
    let stop_at_trade = std::env::var("VOXEL_CITY_PREVIEW_TRADE").is_ok_and(|v| v == "1");
    let physical=std::env::var("VOXEL_CITY_COLONY").is_ok_and(|v|v=="1");
    let mut sim=crate::simulation::WorldSimulation::new();
    if physical {sim.reset_for_world(w.voxel_scale);}
    let mut day=crate::world::DayNightCycle::new(0.4,1200.0);
    let mut blocks=std::collections::HashSet::new();let mut chunks=std::collections::HashSet::new();
    let mut input = crate::player::Input::default();
    for _ in 0..ticks.min(1_000_000) {
        if physical&&!sim.prepare_step() {break;}
        crate::simulation::fixed::gameplay(w, p, &mut input, false, |_, _, _, _| {
            crate::simulation::fixed::Actions::default()
        });
        // The visual review keeps its requested daylight, but timber still
        // falls, settles and becomes real hauled pickups before it is built.
        if physical {crate::simulation::fixed::finish(&mut sim,w,p,&mut day,&mut blocks,&mut chunks,
            crate::simulation::Budget::UNBOUNDED,true,false);blocks.clear();chunks.clear();}
        w.adventure.dirty_voxels.clear();
        if stop_at_trade && w.farming.jobs.market.as_ref().is_some_and(|m| m.sold > 0) { break; }
        if w.farming
            .jobs
            .quarter
            .as_ref()
            .is_some_and(|s| s.frontier.as_ref().map_or(s.completed == 1,|f| f.phase == f.projects.len()))
        {
            break;
        }
    }
    println!(
        "[city-quarter] preview steps={} state={}",
        w.farming.clock,
        super::super::status(w)
    );
}

pub(crate) fn run(root: &Path) -> Result<()> {
    if super::frontier::enabled() {return super::frontier::check::run(root);}
    super::verify::geometry_checks()?;
    super::layout::checks::run(root)?;
    if std::env::var("VOXEL_CITY_LAYOUT_ONLY").is_ok_and(|v|v=="1") { return super::layout::checks::native_matrix(root); }
    std::fs::create_dir_all(root)?;
    super::verify::floor_work_position(root)?;
    super::crew_check::run(root)?;
    super::market_check::direct(root)?;
    let mut w = World::with_seed_in(
        crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter,
        424242,
        root,
    );
    let mut p = crate::player::Player::new();
    install(&mut w, &mut p).map_err(anyhow::Error::msg)?;
    let initial = stock(&w);
    println!(
        "[city-check] initial-stock={initial} chest={:?} priorities={:?}",
        w.block_data.chest(
            w.farming
                .jobs
                .stockpile
                .map(IVec3::from)
                .unwrap_or(IVec3::ZERO)
        ),
        w.farming.jobs.priorities()
    );
    super::layout::checks::native(&w, &mut p)?;
    let ground = IVec3::from(w.farming.jobs.quarter.as_ref().map_or([0; 3], |q| q.origin));
    for at in [
        IVec3::new(1, 1, 9),
        IVec3::new(1, 1, 8),
        IVec3::new(2, 1, 11),
        IVec3::new(4, 1, 5),
    ]
    .map(|v| v + ground)
    {
        println!(
            "[city-check] nav-sample at={at:?} stand={} floor={:?} body={:?} head={:?} known={:?} resident={}",
            super::super::stand(&w, at, 1.0),
            w.get(at - IVec3::Y),
            w.get(at),
            w.get(at + IVec3::Y),
            super::super::known(&w, at),
            w.chunks
                .contains_key(&crate::world::chunk_coord_from_world_block(at))
        );
    }
    let mut input = crate::player::Input::default();
    let mut changed = 0;
    let mut last_change = 0;
    let mut checkpointed = false;
    let mut placements = std::collections::BTreeMap::<u32, u32>::new();
    let mut last_placement = std::collections::BTreeMap::new();
    let mut max_carriers = 0;
    let mut payment_checkpoint = false;
    for step in 0..1_000_000 {
        crate::simulation::fixed::gameplay(&mut w, &mut p, &mut input, false, |_, _, _, _| {
            crate::simulation::fixed::Actions::default()
        });
        if w.farming.jobs.market.is_some() {
            ensure!(super::market_check::money(&w) == super::market_fixture::TIMBER, "Payment conservation at step {step}");
            if !payment_checkpoint && (0..3).any(|id| super::super::crew::view(&w.farming.jobs, id).is_some_and(|a| a.shopping.is_some())) {
                interrupted_checkpoint(&mut w, &mut p, root)?;
                payment_checkpoint = true;
            }
        }
        let edits = std::mem::take(&mut w.adventure.dirty_voxels).len();
        changed += edits;
        if edits > 0 {
            last_change = step;
        }
        let carriers = (0..3).filter(|&id| super::super::crew::view(&w.farming.jobs, id).is_some_and(|a| a.carry.is_some())).count();
        max_carriers = max_carriers.max(carriers);
        for id in 0..3 {
            if let Some(at) = super::super::crew::view(&w.farming.jobs, id).and_then(|a| a.last_placement).map(|v| v.0) {
                if last_placement.insert(id, at) != Some(at) { *placements.entry(id).or_default() += 1; }
            }
        }
        if step % 30 == 0 { ensure!(w.farming.jobs.valid(), "Invalid crew claims at step {step}"); }
        if !checkpointed && carriers >= 2 {
            interrupted_checkpoint(&mut w, &mut p, root)?;
            checkpointed = true;
        }
        if step - last_change > 24_000 {
            break;
        }
        if step == 35
            && let Some(o) = w.farming.jobs.build.first().copied()
        {
            let feet = w
                .farming
                .jobs
                .worker_pos
                .map(Vec3::from)
                .unwrap_or(Vec3::ZERO);
            let build = super::super::Task::Build(o);
            println!(
                "[city-check] first={o:?} material={:?} ready={} below={:?} path={} fetchpath={} at={:?}",
                o.material(),
                o.ready(&w),
                w.get(o.cell() - IVec3::Y),
                super::super::path(&w, feet, build).len(),
                super::super::path(
                    &w,
                    feet,
                    super::super::Task::Fetch(
                        IVec3::from(w.farming.jobs.stockpile.unwrap_or([0; 3])),
                        o
                    )
                )
                .len(),
                feet
            );
        }
        if step % 1200 == 0 {
            let city = w
                .farming
                .jobs
                .quarter
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Lost quarter"))?;
            println!(
                "[city-check] step={step} layer={} queue={} carry={:?} status={} worker={:?}",
                city.lots[0].layer,
                w.farming.jobs.build.len(),
                w.farming.jobs.carry,
                super::super::status(&w),
                w.farming.jobs.worker_pos
            );
        }
        if w.farming
            .jobs
            .quarter
            .as_ref()
            .is_some_and(|s| s.frontier.as_ref().map_or(s.completed == 1,|f| f.phase == f.projects.len()))
        {
            break;
        }
    }
    let city = w
        .farming
        .jobs
        .quarter
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Lost quarter"))?;
    ensure!(
        city.completed == 1,
        "Construction incomplete: {} orders={:?} path={:?} task={:?} carry={:?} stock={} helpers={:?} claims={:?}",
        super::super::status(&w),
        w.farming.jobs.build,
        w.farming.jobs.path,
        w.farming.jobs.task,
        w.farming.jobs.carry,
        stock(&w), w.farming.jobs.helpers, w.farming.jobs.claims
    );
    ensure!(checkpointed && max_carriers >= 2, "No simultaneous cargo checkpoint");
    ensure!((0..3).all(|id| placements.get(&id).is_some_and(|n| *n > 0)), "A crew member never built: {placements:?}");
    ensure!(placements.values().sum::<u32>() == initial, "Crew placement counts disagree with bill");
    super::market_check::complete(&w, payment_checkpoint)?;
    super::walk::check(&w, &mut p)?;
    super::roof_check::completed(&w, &mut p, root)?;
    let rows = traditions().map_err(anyhow::Error::msg)?;
    let l = &city.lots[0];
    let plan = geometry::plan(l, &rows[l.tradition]);
    ensure!(
        plan.iter().all(|(q, b)| w.get(geometry::at(l, *q)) == *b),
        "Missing building cells"
    );
    ensure!(stock(&w) == 0, "Unused material after exact bill");
    ensure!(
        initial == plan.len() as u32,
        "Material conservation: supplied={initial} built={}",
        plan.len()
    );
    let bytes = crate::world::farming::sidecar::encode(&w);
    let (loaded, _) = crate::world::farming::sidecar::decode(&bytes)?;
    ensure!(
        loaded
            .jobs
            .quarter
            .as_ref()
            .is_some_and(|s| s.frontier.as_ref().map_or(s.completed == 1,|f| f.phase == f.projects.len())),
        "Lost occupancy on reload"
    );
    std::fs::write(
        root.join("construction-check.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "layout":city.layout,
            "market_ledger":super::market_check::ledger(&w),"market_cover_blocks":if l.market_cover {geometry::market_cover().len()}else{0},
            "market_sales":w.farming.jobs.market.as_ref().map(|m|m.sold),"timber_produced":w.farming.jobs.market.as_ref().map(|m|m.harvested.count_ones()),"payment_checkpoint":payment_checkpoint,"payment_remaining":super::market_check::money(&w),
            "complete":true,"supplied_blocks":initial,"built_blocks":plan.len(),"stock_remaining":stock(&w),
            "voxel_changes":changed,"residents":city.residents,"style":rows[l.tradition].name,
        "complexity":l.complexity,"simulation_steps":w.farming.clock,"sidecar_round_trip":true,
        "interrupted_cargo_round_trip":checkpointed,"builders":3,"placements_by_worker":placements,"max_simultaneous_carriers":max_carriers
        }))?,
    )?;
    println!(
        "[city-check] PASS paid_blocks={} steps={}",
        plan.len(),
        w.farming.clock
    );
    Ok(())
}

fn stock(w: &World) -> u32 { super::market_check::remaining(w) }

fn interrupted_checkpoint(w: &mut World, p: &mut crate::player::Player, root: &Path) -> Result<()> {
    let total = stock(w);
    let carried = ron::to_string(&w.farming.jobs)?;
    let queued = w.farming.jobs.build.len();
    let path = root.join("interrupted-farming.vxf");
    std::fs::write(&path, crate::world::farming::sidecar::encode(w))?;
    let (loaded, _) = crate::world::farming::sidecar::decode(&std::fs::read(&path)?)?;
    w.farming = loaded;
    ensure!(
        ron::to_string(&w.farming.jobs)? == carried && stock(w) == total,
        "Cargo changed across checkpoint"
    );
    ensure!(
        w.farming.jobs.build.len() == queued,
        "Project changed across checkpoint"
    );
    if let Some(q) = w.farming.jobs.quarter.as_mut() {
        q.paused = true;
    }
    let mut input = crate::player::Input::default();
    for _ in 0..120 {
        crate::simulation::fixed::gameplay(w, p, &mut input, false, |_, _, _, _| {
            crate::simulation::fixed::Actions::default()
        });
    }
    ensure!(
        w.farming.jobs.build.len() == queued && stock(w) == total,
        "Paused project changed stock or orders"
    );
    if let Some(q) = w.farming.jobs.quarter.as_mut() {
        q.paused = false;
    }
    println!(
        "[city-check] PASS interrupted cargo/project file round trip and paused project conservation"
    );
    Ok(())
}
