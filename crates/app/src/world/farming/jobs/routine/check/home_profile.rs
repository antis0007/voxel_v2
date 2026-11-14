//! Read-only warm queries are measured outside fixed gameplay calls.
use super::*;
use std::hint::black_box;

pub(super) fn run(root:&Path,w:&World)->Result<()> {
    if !std::env::var("VOXEL_CITY_HOME_PROFILE").is_ok_and(|v|v=="1") {return Ok(());}
    let j=&w.farming.jobs;
    let before=ron::to_string(j)?;
    let s=j.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Home profile needs colony")?;
    ensure!(s.phase==s.projects.len(),"Home profile needs completed geometry");
    let ids=crew::ids(j).collect::<Vec<_>>();
    ensure!(!ids.is_empty()&&ids.len()<=crew::CAPACITY,"Home profile roster out of bounds");
    let signature=ids.iter().map(|&id|(id,home(w,j,id).map(|(a,b,m)|(a.to_array(),b.to_array(),m)))).collect::<Vec<_>>();
    let homes=s.households.iter().map(|h|h.project).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    ensure!(!homes.is_empty(),"Home profile needs physical homes");
    let home_queries=measure(ids.len()*32,ids.len(),|n|usize::from(black_box(home(black_box(w),black_box(j),black_box(ids[n%ids.len()]))).is_some()));
    let original_guard=measure(ids.len()*32,ids.len(),|_|{
        let s=black_box(s);
        usize::from(black_box(s.growth.as_ref().is_none_or(|g|!g.households_live(s))&&s.phase!=s.projects.len()))
    });
    let reordered_guard=measure(ids.len()*32,ids.len(),|_|{
        let s=black_box(s);
        usize::from(black_box(s.phase!=s.projects.len()&&s.growth.as_ref().is_none_or(|g|!g.households_live(s))))
    });
    let completion_queries=measure(homes.len()*32,homes.len(),|n|
        usize::from(black_box(black_box(s).home_complete(black_box(w),black_box(homes[n%homes.len()])))));
    let bootstrap_queries=s.growth.as_ref().map(|g|measure(ids.len()*32,ids.len(),|_|
        usize::from(black_box(black_box(g).households_live(black_box(s))))));
    let bootstrap=s.growth.as_ref().map_or(0,|g|g.bootstrap);
    let shape_cells=s.projects.iter().take(bootstrap).map(|p|p.cells.len()).sum::<usize>();
    let shape_chest_rows=s.projects.iter().take(bootstrap).map(|p|
        p.cells.iter().filter(|o|o.block()==B::Chest).count()*p.cells.len()).sum::<usize>();
    let equivalence=guard_equivalence()?;
    ensure!(ron::to_string(j)?==before,"Read-only profiling changed jobs");
    let report=serde_json::json!({"roster":ids.len(),"housed":signature.iter().filter(|(_,h)|h.is_some()).count(),
        "completed_projects":s.projects.len(),"unique_households":homes.len(),"home_signature":signature,
        "actual_home_queries":home_queries,"original_guard_query":original_guard,"reordered_guard_query":reordered_guard,
        "home_complete_queries":completion_queries,"households_live_queries":bootstrap_queries,
        "bootstrap_shape":{"projects":bootstrap,"cells":shape_cells,"additional_chest_height_rows":shape_chest_rows,
            "scope":"Full-pass structural row counts, not observed runtime call totals"},
        "guard_equivalence":equivalence,"saved_jobs_unchanged":true,
        "scope":"Warm repeated native read-only queries on the final completed snapshot, outside gameplay timing. No per-worker runtime attribution or active-construction cost claim."});
    std::fs::write(root.join("home-query-profile.json"),serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

fn measure(calls:usize,warmup:usize,mut query:impl FnMut(usize)->usize)->serde_json::Value {
    for n in 0..warmup {black_box(query(n));}
    let mut samples=Vec::new();let mut digest=0usize;
    for _ in 0..9 {
        let at=Instant::now();
        for n in 0..calls {digest=digest.wrapping_add(black_box(query(n)));}
        samples.push(at.elapsed().as_secs_f64()*1e9/calls as f64);
    }
    black_box(digest);samples.sort_by(f64::total_cmp);
    let at=|q:f64|samples[((samples.len()-1) as f64*q) as usize];
    serde_json::json!({"batches":samples.len(),"calls_per_batch":calls,"measured_calls":calls*samples.len(),
        "warmup_calls":warmup,"ns_per_query":{"mean":samples.iter().sum::<f64>()/samples.len() as f64,
            "min":at(0.0),"p50":at(0.5),"p95":at(0.95),"max":at(1.0)},"digest":digest})
}

fn guard_equivalence()->Result<serde_json::Value> {
    let mut cases=Vec::new();
    for complete in [false,true] {for live in [false,true] {
        let mut before_calls=0;let before={before_calls+=1;!live}&&!complete;
        let mut after_calls=0;let after=!complete&&{after_calls+=1;!live};
        ensure!(before==after,"Home guard changed admission");
        ensure!(after_calls==usize::from(!complete),"Completed phase evaluated bootstrap predicate");
        cases.push(serde_json::json!({"complete":complete,"bootstrap_live":live,"blocked":before,
            "original_predicate_calls":before_calls,"reordered_predicate_calls":after_calls}));
    }}
    Ok(serde_json::json!({"passed":true,"cases":cases,"scope":"Boolean contract; physical interruption gates remain in the existing native daily checks"}))
}