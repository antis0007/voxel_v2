//! Verifies serial paid infill while every existing household stays active.
use super::*;
use super::growth::evidence;
use std::collections::BTreeSet;
mod report;
mod walk;
mod cycles;
mod geometry;
mod stability;
mod replay;
mod expansion;
mod obstruction;
const BOOTSTRAP: usize = 6;

fn site(w: &World) -> Result<&super::super::super::quarter::frontier::State> {
    w.farming.jobs.quarter.as_ref().and_then(|q| q.frontier.as_ref()).context("Missing colony")
}

#[derive(serde::Serialize)]
struct ProjectRun {
    index: usize,
    admitted: usize,
    admitted_clock: u64,
    served_before: BTreeMap<u32,u32>,
    meal_observed_at: BTreeMap<u32,u64>,
    qualified_social: BTreeMap<u32,(u64,u32)>,
    residents_before: BTreeMap<u32, Resident>,
    continuing_cycles: BTreeSet<u32>,
    cycle_completed_at: BTreeMap<u32,usize>,
    obstruction: Option<obstruction::Record>,
    cargo_reload: bool,
    funding_probe: bool,
    completed: Option<usize>,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    ensure!(storage::save_directory() == root.join("disposable-saves"), "Infill check requires disposable saves");
    vehicle::check_worker_reload(root)?;
    if std::env::var("VOXEL_INFILL_RELOAD_ONLY").is_ok_and(|v|v=="1") {return replay::run(root);}
    let mode = std::env::var("VOXEL_CITY_GROWTH").ok().and_then(|v|v.parse::<u8>().ok()).unwrap_or(0);
    ensure!((2..=3).contains(&mode), "Infill check requires growth version 2 or 3");
    let expected = std::env::var("VOXEL_INFILL_EXPECTED_HOMES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(if mode==3 {0}else{2});
    ensure!((mode==2 && (1..=2).contains(&expected)) || (mode==3 && expected<=10), "Expected infill count outside bounded scenario");
    std::fs::create_dir_all(root)?;
    let at = Instant::now(); let loaded = storage::load_review_checkpoint()?;
    let (mut w, mut player, mut day) = (loaded.world, loaded.player, loaded.day_night);
    let admitted_chunks = materialize::admit(&mut w)?;
    let load_ms = at.elapsed().as_secs_f64()*1000.0;
    ensure!(site(&w)?.projects.len() == BOOTSTRAP && site(&w)?.phase == BOOTSTRAP, "Expected six completed houses");
    let mut stable = stability::Homes::default();
    stable.observe(&w)?;
    let original_households = ron::to_string(&site(&w)?.households)?;
    let ids = w.farming.jobs.production.as_ref().context("Missing production")?.workers.clone();
    let old_ids: Vec<_> = site(&w)?.households.iter().flat_map(|h| h.members.iter().copied()).collect();
    ensure!(ids.len() == 64 && old_ids.len() == 24, "Expected 64 residents, 24 housed");
    let old_homes: BTreeMap<_,_> = old_ids.iter().filter_map(|id| w.travel.homes.get(id).map(|&p| (*id,p))).collect();
    let baseline: BTreeMap<_,_> = ids.iter().filter_map(|&id| crew::view(&w.farming.jobs,id).map(|a| (id,a.routine.clone()))).collect();
    let served_before = w.farming.jobs.production.as_ref().context("Missing meals")?.served.clone();
    report::sidecar(&mut w,root,"legacy-before-infill.vxf")?;
    let initial_geometry = geometry::census(&w);
    let balance = (food_balance(&w),timber_balance(&w));
    super::super::super::enable_housing_growth(&mut w).map_err(anyhow::Error::msg)?;
    let enabled = ron::to_string(&site(&w)?.growth)?;
    super::super::super::enable_housing_growth(&mut w).map_err(anyhow::Error::msg)?;
    ensure!(ron::to_string(&site(&w)?.growth)? == enabled, "Infill enable duplicated state");
    ensure!(site(&w)?.growth.as_ref().is_some_and(|g| g.version == mode), "Wrong growth schema");
    ensure!((food_balance(&w),timber_balance(&w)) == balance && geometry::census(&w) == initial_geometry, "Enabling growth changed resources or geometry");
    let at = Instant::now(); super::super::super::check_housing_growth_sites(&mut w)?;
    let plan_probe_ms = at.elapsed().as_secs_f64()*1000.0;
    ensure!((food_balance(&w),timber_balance(&w)) == balance, "Planning probes changed resources");
    ensure!(geometry::census(&w) == initial_geometry, "Planning probes changed building geometry");
    let before_food = food_balance(&w); let before_timber = timber_balance(&w);
    let before_components = evidence::timber(&w);
    let roots: Vec<_> = w.farming.jobs.production.as_ref().context("Missing forestry")?.beds.iter()
        .filter(|b| b.crop == B::Sapling).map(|b| IVec3::from(b.at)).collect();
    w.farming.jobs.quarter.as_mut().context("Missing city")?.paused = true;
    let mut simulation = WorldSimulation::new(); simulation.reset_for_world(w.voxel_scale);
    let mut input = Input::default(); let mut blocks = HashSet::new(); let mut chunks = HashSet::new();
    let mut pending = Some(materialize::Pending::new(&w,0,"infill-initial"));
    let mut materializations = Vec::new(); let mut reloads = Vec::new();
    let mut activity = evidence::Activity::default(); let mut projects: Vec<ProjectRun> = Vec::new();
    let mut game = Vec::new(); let mut finish = Vec::new(); let mut drop_ms = Vec::new(); let mut sample_ms = Vec::new();
    let mut stages = Vec::new(); let mut last_stage = String::new(); let mut grown = 0u32;
    let mut proposal_snapshots = BTreeSet::new(); let mut obstruction = None;
    let mut between_reload = false; let mut between_projects = BTreeSet::new();
    let mut exhaustion = expansion::Exhaustion::default(); let mut search_reloads = BTreeSet::new(); let mut final_reload = false; let mut finished_at = None;
    let mut timber_events = Vec::new(); let mut last_delta = 0i64;
    let mut waiting_plan: Option<(String,u32)> = None;
    let limit = std::env::var("VOXEL_INFILL_STEPS").ok().and_then(|v|v.parse::<usize>().ok()).unwrap_or(360000).clamp(12000,1200000);
    let started = Instant::now();
    for step in 0..limit {
        let growing: Vec<_> = roots.iter().copied().filter(|p|w.farming.plots.contains_key(&(p.x,p.y,p.z))).collect();
        ensure!(simulation.prepare_step(), "Infill simulation unavailable");
        let at = Instant::now(); evidence::settle(&mut w); drop_ms.push(at.elapsed().as_secs_f64()*1000.0);
        let at = Instant::now(); let mut observed = Ok(None);
        fixed::gameplay(&mut w,&mut player,&mut input,false,|hook,w,_,_| {
            if matches!(hook,fixed::Hook::BeforeAdventure) && let Some(stage) = &mut pending {
                observed = stage.observe(root,w,step);
            }
            fixed::Actions::default()
        });
        if let Some(value) = observed? { materializations.push(value); pending = None; activity.previous.clear(); }
        for at in growing { if w.get(at) == B::Log && !w.farming.plots.contains_key(&(at.x,at.y,at.z)) { grown += 1; } }
        game.push(at.elapsed().as_secs_f64()*1000.0);
        let at = Instant::now();
        fixed::finish(&mut simulation,&mut w,&player,&mut day,&mut blocks,&mut chunks,Budget::UNBOUNDED,true,true);
        finish.push(at.elapsed().as_secs_f64()*1000.0); blocks.clear(); chunks.clear(); w.adventure.dirty_voxels.clear();
        if step < 240 { ensure!(site(&w)?.projects.len() == BOOTSTRAP, "Paused district admitted a house"); }
        if step == 240 { w.farming.jobs.quarter.as_mut().context("Missing city")?.paused = false; }
        let s = site(&w)?; let g = s.growth.as_ref().context("Lost infill state")?;
        ensure!(s.projects.len() <= BOOTSTRAP+if expected==0 {10}else{expected}, "Unexpected extra infill project");
        exhaustion.observe(&w,step,game.last().copied().unwrap_or(0.0))?;
        if step%PULSE as usize == 0 { stable.observe(&w)?; }
        ensure!(old_homes.iter().all(|(id,pos)|w.travel.homes.get(id)==Some(pos)), "Original home assignment changed");
        let status = format!("{}:{:?}:{}",s.projects.len(),g.project,g.waiting);
        if status != last_stage {
            if stages.len() < 256 { stages.push(serde_json::json!({"step":step,"growth":g,"housed":s.households.iter().map(|h|h.members.len()).sum::<usize>()})); }
            last_stage = status;
        }
        let active = g.project;
        if active.is_none() && g.proposal.is_some() && g.waiting.starts_with("Materials") {
            let plan = ron::to_string(&g.proposal)?;
            if let Some((previous,queries)) = &waiting_plan && *previous == plan {
                ensure!(g.scans.path_searches == *queries, "Material waiting repeated street routing");
            }
            waiting_plan = Some((plan,g.scans.path_searches));
        } else { waiting_plan = None; }
        if g.proposal.is_some() && active.is_none() && proposal_snapshots.insert(s.projects.len()) {
            let name = format!("unfunded-{}.vxf",site(&w)?.projects.len());
            report::sidecar(&mut w,root,&name)?;
        }
        if mode==3 && pending.is_none() {
            let phase=site(&w)?.growth.as_ref().and_then(|g|g.infill.as_ref()).and_then(|p|p.search.as_ref())
                .filter(|q|q.cursor>0).map(|q|if q.routing {"shortlist"}else{"candidate-sweep"});
            if let Some(phase)=phase && !search_reloads.contains(phase) {
                let label=format!("infill-search-{phase}");
                report::reload(root,&mut w,&mut player,&mut day,&label,step,&mut reloads)?;
                search_reloads.insert(phase.to_string());simulation.reset_for_world(w.voxel_scale);
                pending=Some(materialize::Pending::new(&w,step+1,label));activity.previous.clear();
            }
        }
        if let Some(index) = active && !projects.iter().any(|p|p.index == index) {
            ensure!(index == BOOTSTRAP+projects.len(), "Project index skipped or reused");
            ensure!(projects.iter().all(|p|p.completed.is_some()), "Two projects funded concurrently");
            stable.observe(&w)?;
            ensure!(stable.len() == index, "Prior occupied home was not preserved before admission");
            let residents_before = site(&w)?.households.iter().flat_map(|h|h.members.iter().copied())
                .filter_map(|id|crew::view(&w.farming.jobs,id).map(|a|(id,a.routine.clone()))).collect();
            projects.push(ProjectRun { index, admitted:step, admitted_clock:w.farming.clock,
                served_before:w.farming.jobs.production.as_ref().context("Missing meals")?.served.clone(),
                meal_observed_at:BTreeMap::new(),qualified_social:BTreeMap::new(),
                residents_before, continuing_cycles:BTreeSet::new(), cycle_completed_at:BTreeMap::new(),
                obstruction:None, cargo_reload:false, funding_probe:false, completed:None });
            report::sidecar(&mut w,root,&format!("funded-{index}.vxf"))?;
        }
        for run in &mut projects {
            if run.completed.is_none() {
                cycles::observe(&w,run);
                for &id in &run.continuing_cycles {run.cycle_completed_at.entry(id).or_insert(step);}
            }
            if run.completed.is_none() && site(&w)?.home_complete(&w,run.index) { run.completed = Some(step); }
        }
        if let Some((cell,index)) = obstruction {
            let run=projects.iter_mut().find(|p|p.index==index).context("Lost obstructed project")?;
            if obstruction::poll(root,&mut w,run,cell,step,mode)? {obstruction=None;}
        }
        if pending.is_none() && let Some(run) = projects.iter_mut().find(|p|Some(p.index)==active) {
            if !run.funding_probe {
                let balances = (food_balance(&w),timber_balance(&w));
                super::super::super::check_housing_growth_funding(&mut w)?;
                ensure!((food_balance(&w),timber_balance(&w))==balances,"Funding probe changed resources");
                run.funding_probe = true;
            }
            if !run.cargo_reload && let Some(order) = w.farming.jobs.claims.iter().map(|c|c.order)
                .find(|o|w.farming.jobs.growth_funded_order(*o) && known(&w,o.cell())==Some(B::Air)) {
                w.set(order.cell(),B::Stone); obstruction=Some((order.cell(),run.index));
                run.obstruction=Some(obstruction::Record::new(step));
                report::sidecar(&mut w,root,&format!("carried-{}.vxf",run.index))?;
                let label = format!("infill-cargo-{}",run.index);
                report::reload(root,&mut w,&mut player,&mut day,&label,step,&mut reloads)?;
                run.cargo_reload = true; simulation.reset_for_world(w.voxel_scale);
                pending = Some(materialize::Pending::new(&w,step+1,label)); activity.previous.clear();
            }
        }
        if (mode==3 || expected==2) && pending.is_none() && !projects.is_empty()
            && projects.iter().all(|p|p.completed.is_some()) && !between_projects.contains(&projects.len())
            && site(&w)?.growth.as_ref().is_some_and(|g|g.project.is_none()) {
            let label=format!("infill-between-{}",projects.len());
            report::reload(root,&mut w,&mut player,&mut day,&label,step,&mut reloads)?;
            between_projects.insert(projects.len());between_reload=true;simulation.reset_for_world(w.voxel_scale);
            pending=Some(materialize::Pending::new(&w,step+1,label));activity.previous.clear();
        }
        if pending.is_none() && step%PULSE as usize == 0 {
            let at = Instant::now(); activity.sample(&w,&ids,step)?;
            ensure!(w.farming.jobs.valid(), "Invalid infill state at step {step}");
            sample_ms.push(at.elapsed().as_secs_f64()*1000.0);
        }
        if step%1200 == 0 {
            geometry::check(&w,&initial_geometry)?;
            let delta = i64::from(timber_balance(&w))-i64::from(before_timber+grown*16);
            if delta != last_delta && timber_events.len()<128 {
                timber_events.push(serde_json::json!({"step":step,"delta":delta,"grown":grown,"components":evidence::timber(&w)}));last_delta=delta;
            }
        }
        if step%12000 == 0 { evidence::write(root,&w,&activity)?; println!("[infill] step={step} {last_stage}"); }
        let done = if expected==0 {projects.len()>=3 && exhaustion.confirmed()} else {projects.len()==expected};
        if done && projects.iter().all(|p|p.completed.is_some()) && finished_at.is_none() {finished_at=Some(step);}
        if let Some(done) = finished_at && step>done+60000 && pending.is_none()
            && (mode!=3 || expansion::settled(&w)?) {
            if final_reload {break;}
            report::reload(root,&mut w,&mut player,&mut day,"infill-final",step,&mut reloads)?;
            final_reload=true;simulation.reset_for_world(w.voxel_scale);
            pending=Some(materialize::Pending::new(&w,step+1,"infill-final"));activity.previous.clear();
        }
    }
    activity.sample(&w,&ids,game.len())?;
    ensure!(w.farming.jobs.valid(), "Invalid final infill state");
    stable.observe(&w)?;
    let geometric_cells=geometry::check(&w,&initial_geometry)?;
    evidence::write(root,&w,&activity)?;
    let actual=if expected==0 {projects.len()}else{expected};
    expansion::finish(root,&w,mode,&exhaustion,&between_projects,&search_reloads,projects.len())?;
    report::finish(root,&w,&mut player,actual,&projects,&old_ids,&baseline,&served_before,&original_households,
        before_food,before_timber,grown,before_components,timber_events,&activity,proposal_snapshots.len(),between_reload,final_reload,pending.is_none(),
        obstruction.is_none(),geometric_cells,stable.len(),stages,materializations,reloads,load_ms,admitted_chunks,plan_probe_ms,started.elapsed().as_secs_f64(),game,finish,drop_ms,sample_ms)
}
