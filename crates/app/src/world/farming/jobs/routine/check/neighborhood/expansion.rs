//! Verifies bounded survey polls and persistent no-space retries.
use super::*;

#[derive(Default)]
pub(super) struct Exhaustion {
    previous: Option<(u32,u32,u64,u64,u64,bool)>,
    pub episodes: Vec<serde_json::Value>,
    last_exhausted_attempt: Option<u32>,
    max_candidates: u32,
    max_routes: u32,
    max_node_budget: u64,
    max_cells: u64,
    quiet_ticks: usize,
    planning_ticks_ms: Vec<f64>,
}
impl Exhaustion {
    pub fn confirmed(&self)->bool {self.episodes.len()>=2}
    pub fn observe(&mut self,w:&World,step:usize,gameplay_ms:f64)->Result<()> {
        let g=site(w)?.growth.as_ref().context("Missing growth")?;
        if g.version!=3 {return Ok(());}
        let p=g.infill.as_ref().context("Missing repeatable plan")?;
        if let Some((c,r,n,cells,retry,quiet))=self.previous {
            let dc=g.scans.candidates.saturating_sub(c);
            let dr=g.scans.path_searches.saturating_sub(r);
            let dn=g.scans.path_node_budget.saturating_sub(n);
            let ds=g.scans.cells.saturating_sub(cells);
            if dc>0 || dr>0 || ds>0 {self.planning_ticks_ms.push(gameplay_ms);}
            self.max_candidates=self.max_candidates.max(dc);self.max_routes=self.max_routes.max(dr);
            self.max_node_budget=self.max_node_budget.max(dn);self.max_cells=self.max_cells.max(ds);
            ensure!(dc<=64 && dr<=2 && dn<=40002,"Repeatable planner exceeded per-poll search budget");
            if quiet && w.farming.clock<retry {
                ensure!(dc==0 && dr==0 && dn==0 && ds==0,"No-space cooldown repeated a survey");
                self.quiet_ticks+=1;
            }
        }
        let exhausted=g.project.is_none() && g.proposal.is_none() && !p.searching() && g.waiting.starts_with("No Space");
        if exhausted && self.last_exhausted_attempt!=Some(p.attempts) {
            self.episodes.push(serde_json::json!({"step":step,"clock":w.farming.clock,"attempt":p.attempts,
                "retry_at":p.retry_at,"projects":site(w)?.projects.len(),"status":g.waiting}));
            self.last_exhausted_attempt=Some(p.attempts);
        }
        self.previous=Some((g.scans.candidates,g.scans.path_searches,g.scans.path_node_budget,g.scans.cells,p.retry_at,exhausted));
        Ok(())
    }
}

pub(super) fn finish(root:&Path,w:&World,mode:u8,e:&Exhaustion,between:&BTreeSet<usize>,search_reloads:&BTreeSet<String>,count:usize)->Result<()> {
    if mode!=3 {return Ok(());}
    let g=site(w)?.growth.as_ref().context("Missing repeatable growth")?;
    let result=serde_json::json!({"passed":e.confirmed()&&count>=3&&between.len()==count&&e.quiet_ticks>0&&search_reloads.len()==2,
        "search_full_reload_phases":search_reloads,"completed_homes":count,"between_project_full_reloads":between,"exhaustion_episodes":e.episodes,
        "quiet_cooldown_ticks":e.quiet_ticks,"maximum_candidates_per_poll":e.max_candidates,
        "maximum_routes_per_poll":e.max_routes,"maximum_reserved_route_nodes_per_poll":e.max_node_budget,
        "gameplay_ticks_with_planning_ms":stats(e.planning_ticks_ms.clone()),"maximum_counted_cells_per_poll":e.max_cells,"cumulative_work":g.scans,
        "route_node_measurement":"Reserved cap, not observed expansions","unchanged_service_radius_m":24});
    std::fs::write(root.join("repeatable-growth-budget.json"),serde_json::to_vec_pretty(&result)?)?;
    ensure!(result["passed"].as_bool()==Some(true),"Repeatable growth, exhaustion or between-project reload gate incomplete");
    Ok(())
}

pub(super) fn settled(w:&World)->Result<bool> {
    let g=site(w)?.growth.as_ref().context("Missing repeatable growth")?;
    let p=g.infill.as_ref().context("Missing repeatable plan")?;
    Ok(g.project.is_none() && g.proposal.is_none() && !p.searching()
        && g.waiting.starts_with("No Space") && p.retry_at>w.farming.clock.saturating_add(180))
}
