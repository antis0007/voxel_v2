use super::*;

pub(super) fn observe(w: &World, run: &mut ProjectRun) {
    let Some(production)=w.farming.jobs.production.as_ref() else{return;};
    for (&id,before) in &run.residents_before {
        let Some(actor)=crew::view(&w.farming.jobs,id) else{continue;};
        let r=&actor.routine;
        if production.served.get(&id).copied().unwrap_or(0)>run.served_before.get(&id).copied().unwrap_or(0) {
            run.meal_observed_at.entry(id).or_insert(w.farming.clock);
        }
        if let Some(&meal)=run.meal_observed_at.get(&id)
            && meal>run.admitted_clock && r.rested_at>=meal && r.social_at>=r.rested_at
            && r.rests>before.rests && r.socials>before.socials {
            run.qualified_social.entry(id).or_insert((r.social_at,r.resumed));
        }
        if let Some(&(social,resumed))=run.qualified_social.get(&id)
            && w.farming.clock>social && r.resumed>resumed {run.continuing_cycles.insert(id);}
    }
}
