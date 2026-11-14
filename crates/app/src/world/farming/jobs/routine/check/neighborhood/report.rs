use super::*;
use crate::{player::Player,world::DayNightCycle};

pub(super) fn sidecar(w: &mut World, root: &Path, name: &str) -> Result<()> {
    let before=ron::to_string(&w.farming.jobs)?;
    ensure!(before.len()<=256*1024,"Saved city jobs exceed the 256 KiB sidecar budget");
    let bytes=crate::world::farming::sidecar::encode(w);std::fs::write(root.join(name),&bytes)?;
    let (restored,_)=crate::world::farming::sidecar::decode(&bytes)?;
    ensure!(ron::to_string(&restored.jobs)?==before,"Infill sidecar changed plan, commitments or routines");
    w.farming=restored;ensure!(crate::world::farming::sidecar::encode(w)==bytes,"Infill sidecar changed clocks or production");Ok(())
}

pub(super) fn reload(root: &Path,w: &mut World,p: &mut Player,day: &mut DayNightCycle,label: &str,step: usize,out: &mut Vec<serde_json::Value>) -> Result<()> {
    let before=ron::to_string(&site(w)?.growth)?;let at=Instant::now();
    super::super::reload::full(root,w,p,day)?;
    ensure!(ron::to_string(&site(w)?.growth)?==before,"Full reload changed infill provenance");
    std::fs::copy(root.join("pre-rest-checkpoint-residents.json"),root.join(format!("{label}-saved-residents.json")))?;
    out.push(serde_json::json!({"stage":label,"step":step,"ms":at.elapsed().as_secs_f64()*1000.0,"plan_and_resources_preserved":true}));Ok(())
}

pub(super) fn finish(root: &Path,w: &World,p: &mut Player,expected: usize,projects: &[ProjectRun],old_ids: &[u32],
    baseline: &BTreeMap<u32,Resident>,served_before: &BTreeMap<u32,u32>,old_households: &str,
    before_food: i64,before_timber: u32,grown: u32,before_components: serde_json::Value,timber_events: Vec<serde_json::Value>,
    activity: &evidence::Activity,proposal_snapshots: usize,between_reload: bool,final_reload: bool,materialized: bool,obstruction_cleared: bool,geometric_cells: usize,stable_homes: usize,
    stages: Vec<serde_json::Value>,materializations: Vec<serde_json::Value>,reloads: Vec<serde_json::Value>,load_ms: f64,admitted_chunks: usize,
    plan_probe_ms: f64,wall_seconds: f64,game: Vec<f64>,finish: Vec<f64>,drop_ms: Vec<f64>,sample_ms: Vec<f64>) -> Result<()> {
    let s=site(w)?;let g=s.growth.as_ref().context("Missing final infill state")?;
    let prod=w.farming.jobs.production.as_ref().context("Missing final production")?;
    let new_ids:Vec<_>=s.households.iter().filter(|h|h.project>=BOOTSTRAP).flat_map(|h|h.members.iter().copied()).collect();
    let housed=s.households.iter().map(|h|h.members.len()).sum::<usize>();
    let cycles=new_ids.iter().filter(|&&id|crew::view(&w.farming.jobs,id).is_some_and(|a|
        a.routine.rests>baseline[&id].rests&&a.routine.socials>baseline[&id].socials&&a.routine.resumed>baseline[&id].resumed
        &&prod.served.get(&id).copied().unwrap_or(0)>served_before.get(&id).copied().unwrap_or(0))).count();
    let old_retained=ron::to_string(&s.households.iter().filter(|h|h.project<BOOTSTRAP).collect::<Vec<_>>())?==old_households;
    let paid:usize=s.projects.iter().skip(BOOTSTRAP).flat_map(|p|&p.cells).filter(|o|known(w,o.cell())==Some(o.block())).count();
    let planned:usize=s.projects.iter().skip(BOOTSTRAP).map(|p|p.cells.len()).sum();
    let planks=s.projects.iter().skip(BOOTSTRAP).flat_map(|p|&p.cells).filter(|o|o.block()==B::Planks).count();
    let chests=s.projects.iter().skip(BOOTSTRAP).flat_map(|p|&p.cells).filter(|o|o.block()==B::Chest).count();
    let moved=baseline.keys().filter(|id|activity.moved.get(id).copied().unwrap_or(0.0)>=2.0).count();
    let food=food_balance(w)==before_food;let timber=timber_balance(w)==before_timber+grown*16;
    let completed=projects.len()==expected&&projects.iter().all(|p|p.completed.is_some()&&s.home_complete(w,p.index));
    let retained_cycles=projects.iter().enumerate().all(|(n,p)|p.residents_before.len()==old_ids.len()+n*4
        &&p.continuing_cycles.len()==p.residents_before.len());
    let settled=g.project.is_none()&&g.proposal.is_none()
        && if g.version==3 { !g.complete && (g.waiting.starts_with("No Space") || g.waiting=="Housing Supplied") } else {g.complete==(expected==2)};
    let mut routes=Vec::new();let mut route_errors=Vec::new();
    if completed {for run in projects {match walk::check(w,p,run.index) {
        Ok(route)=>routes.push(route),Err(error)=>route_errors.push(format!("Project {}: {error:#}",run.index)),
    }}}
    let passed=completed&&s.projects.len()==BOOTSTRAP+expected&&s.phase==s.projects.len()&&(g.version==2||g.version==3)
        &&housed==24+4*expected&&new_ids.len()==expected*4&&cycles==expected*4&&old_retained
        &&geometric_cells==paid&&stable_homes==BOOTSTRAP+expected&&route_errors.is_empty()
        &&planned==paid&&planned==planks+chests&&chests==expected&&retained_cycles&&settled
        &&projects.iter().all(|p|p.cargo_reload&&p.funding_probe)&&proposal_snapshots>=expected
        &&(expected==1||between_reload)&&final_reload&&materialized&&obstruction_cleared
        &&food&&timber&&activity.minimum_live==64&&moved==64&&activity.jumps.is_empty();
    let steps=game.len();
    let mut result=serde_json::json!({"passed":passed,"expected_homes":expected,"steps":steps,"simulated_seconds":steps as f64*crate::simulation::STEP_SECONDS as f64,
        "residents":64,"housed_before":24,"housed_after":housed,"unhoused_after":64-housed,"new_occupants":new_ids,"new_occupants_completed_cycles":cycles,
        "projects":projects,"completed":completed,"paid_cells":paid,"planned_cells":planned,"paid_planks":planks,"paid_chests":chests,
        "old_households_retained":old_retained,"existing_household_cycles_during_each_project":retained_cycles,"minimum_live":activity.minimum_live,
        "physically_moved":moved,"movement_discontinuities":activity.jumps,"distance_m":activity.moved,"growth":g});
    let details=serde_json::json!({"jobs_serialized_bytes":ron::to_string(&w.farming.jobs)?.len(),"food_conserved":food,"timber_conserved":timber,"grown_trees":grown,"unfunded_sidecar_snapshots":proposal_snapshots,
        "between_homes_full_reload":between_reload,"final_full_reload":final_reload,"materialization_stages":materializations,"reloads":reloads,
        "site_and_saved_plan_negative_checks":true,"funding_negative_checks":projects.iter().all(|p|p.funding_probe),"plan_probe_ms":plan_probe_ms,
        "obstruction_recovery":obstruction_cleared&&projects.iter().all(|p|p.cargo_reload),"paused_admission":true,"grounded_home_round_trips":routes,
        "stages":stages,"initial_load_ms":load_ms,"admitted_chunks":admitted_chunks,"wall_seconds":wall_seconds,
        "gameplay_ms":stats(game),"finish_ms":stats(finish),"drop_settle_ms":stats(drop_ms),"sample_ms":stats(sample_ms),
        "limits":if g.version==3 {"Repeatable demand-led infill inside unchanged 24 m service range, at most 16 ledger projects; same 64 workers and physical supply. No unrestricted city or multiplayer claim."} else {"At most two generated infill homes inside unchanged 24 m service range; same 64 workers, paid access and physical supply. No unrestricted city or multiplayer claim."}});
    if let (Some(out),Some(extra))=(result.as_object_mut(),details.as_object()){out.extend(extra.clone());}
    result["cycle_observation_contract"]=serde_json::json!({
        "mode":if g.version==3 {"Adaptive physical obstruction stress test"}else{"Legacy fixed obstruction"},
        "earliest_release_ticks":48000,"maximum_hold_ticks":if g.version==3 {144000}else{48000},
        "fixed_window_misses_preserved":true,"credits_activity_after_project_completion":false,
        "proves_unconstrained_scheduler_fairness":false});
    result["geometry_ledger"]=serde_json::json!({"new_paid_cells":geometric_cells,"stable_homes":stable_homes,"route_errors":route_errors});
    result["timber_ledger"]=serde_json::json!({"before":before_components,"after":evidence::timber(w),"expected":before_timber+grown*16,"grown":grown,"events":timber_events});
    std::fs::write(root.join("neighborhood-check.json"),serde_json::to_vec_pretty(&result)?)?;
    println!("[infill] passed={passed} houses={} housed={housed} paid={paid}/{planned} food={food} timber={timber}",projects.len());
    ensure!(passed,"Neighborhood acceptance incomplete");Ok(())
}
