//! Saved-state rejection cases execute in the native acceptance binary.
use super::*;
use anyhow::{Context, ensure};

pub(super) fn check(original: &Site, proposal: &Project) -> anyhow::Result<()> {
    let mut s = original.clone();
    s.growth=Some(State::new([0,1,0]));
    s.growth.as_mut().context("Missing growth")?.proposal=Some(proposal.clone());
    ensure!(s.valid(),"Valid growth proposal rejected");
    let home=proposal.home.as_ref().context("Missing candidate home")?;
    for bad in [[1<<28,home.entry[1],home.entry[2]],
        [home.entry[0],home.entry[1]+1,home.entry[2]],
        [home.entry[0]+1,home.entry[1],home.entry[2]],
        [home.entry[0]+50,home.entry[1],home.entry[2]+50]] {
        let g=s.growth.as_mut().context("Missing growth")?;
        g.proposal.as_mut().and_then(|p|p.home.as_mut()).context("Missing proposal home")?.entry=bad;
        rejected(&s)?;
    }
    s.growth.as_mut().context("Missing growth")?.proposal=Some(proposal.clone());
    let saved=ron::to_string(&s)?;
    let decoded:Site=ron::from_str(&saved)?;
    ensure!(decoded.valid(),"Valid proposal reload rejected");
    s.projects.push(proposal.clone());
    let first=proposal.cells.iter().map(|o|o.cell().y).min().context("Empty proposal")?;
    let terminal=proposal.cells.iter().map(|o|proposal.work_layer(*o)).max().context("Empty proposal")?+2;
    s.project_layers.push(first);
    let g=s.growth.as_mut().context("Missing growth")?;
    g.proposal=None;g.project=Some(6);g.complete=true;
    rejected(&s)?;
    s.growth.as_mut().context("Missing growth")?.complete=false;
    ensure!(s.valid(),"Funded unfinished project rejected");
    s.project_layers[6]=terminal;s.phase=7;
    ensure!(s.valid(),"Completion poll lag rejected");
    s.growth.as_mut().context("Missing growth")?.complete=true;
    ensure!(s.valid(),"Terminal completion rejected");
    s.project_layers.clear();rejected(&s)?;
    let mut g=State::new([0,1,0]);g.next_poll=u64::MAX;
    ensure!(!g.due(100) && g.next_poll==100+4*jobs::PULSE && g.due(100+4*jobs::PULSE),
        "Persisted retry deadline did not recover");
    let mut legacy=original.clone();legacy.growth=None;
    let encoded=ron::to_string(&legacy)?;
    ensure!(!encoded.contains("growth:"),"Legacy save gained growth state");
    let decoded:Site=ron::from_str(&encoded)?;
    ensure!(decoded.valid() && decoded.growth.is_none(),"Legacy save no longer loads");
    println!("[city-growth] saved-state-check passed home=4 premature-complete=1 parallel-ledger=1 retry=1 legacy=1");
    Ok(())
}

fn rejected(s: &Site) -> anyhow::Result<()> {
    let encoded=ron::to_string(s)?;
    let decoded:Site=ron::from_str(&encoded)?;
    ensure!(!decoded.valid(),"Malformed growth save accepted");
    Ok(())
}
