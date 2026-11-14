use super::*;

const EARLIEST: usize=48000;
const DEADLINE: usize=144000;

#[derive(serde::Serialize)]
pub(super) struct Record {
    placed: usize,
    earliest_release: usize,
    deadline: usize,
    fixed_window_missing: Option<Vec<u32>>,
    released_at: Option<usize>,
    held_ticks: usize,
    release_reason: Option<&'static str>,
}

impl Record {
    pub(super) fn new(step: usize) -> Self {
        Self {placed:step,earliest_release:step+EARLIEST,deadline:step+DEADLINE,
            fixed_window_missing:None,released_at:None,held_ticks:0,release_reason:None}
    }
}

pub(super) fn poll(root: &Path,w: &mut World,run: &mut ProjectRun,cell: IVec3,step: usize,mode: u8) -> Result<bool> {
    ensure!(known(w,cell)==Some(B::Stone),"Infill replaced the player obstruction");
    let missing:Vec<_>=run.residents_before.keys().filter(|id|!run.continuing_cycles.contains(id)).copied().collect();
    let record=run.obstruction.as_mut().context("Missing obstruction record")?;
    record.held_ticks=step-record.placed;
    if step<record.earliest_release {return Ok(false);}
    if record.fixed_window_missing.is_none() {record.fixed_window_missing=Some(missing.clone());}
    if mode==3 && !missing.is_empty() {
        if step>=record.deadline {
            record.release_reason=Some("Fixed deadline exceeded");
            let residents:BTreeMap<_,_>=missing.iter().filter_map(|&id|
                crew::view(&w.farming.jobs,id).map(|a|(id,a.routine.clone()))).collect();
            let report=serde_json::json!({"project":run,"missing":missing,"residents":residents,"blocked_cell":cell.to_array(),"step":step,"clock":w.farming.clock});
            std::fs::write(root.join("obstruction-cycle-timeout.json"),serde_json::to_vec_pretty(&report)?)?;
            anyhow::bail!("Existing residents missed the fixed 144000-tick obstruction deadline");
        }
        return Ok(false);
    }
    record.released_at=Some(step);
    record.release_reason=Some(if mode==3 {"All prior residents cycled"}else{"Legacy fixed window"});
    w.set(cell,B::Air);
    Ok(true)
}
