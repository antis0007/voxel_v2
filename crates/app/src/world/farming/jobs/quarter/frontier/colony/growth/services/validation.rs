use super::*;

pub(in super::super) fn valid(g: &State, s: &Site) -> bool {
    let Some(service)=&g.services else {return false;};
    let Some(prefix)=&g.infill else {return false;};
    let count=service.base_project_count;
    if g.version!=4 || g.bootstrap!=6 || g.waiting.len()>160 || g.stockpile!=prefix.stockpile
        || prefix.schema!=2 || !prefix.valid_prefix(s,count) || !prefix.surveyed || prefix.attempts==0
        || prefix.pending().is_some() || prefix.searching() || !service.targets.valid()
        || !(6..=infill::PROJECT_LIMIT-2).contains(&count) || s.project_layers.len()!=s.projects.len()
        || (0..count).any(|i|!state::terminal(s,i) || s.projects[i].cells.len()>jobs::construction::LIMIT)
        || g.chest_target.is_some_and(|n|!(1..=4096).contains(&n))
        || service.cursor>plan::candidate_count() || service.attempts>ATTEMPT_LIMIT
        || !service.reserved_budget.valid() || service.reserved_budget.calls<service.attempts
        || g.complete!=(service.stage==Stage::Complete) {return false;}
    if service.stage==Stage::Survey {
        return service.plan.is_none() && g.project.is_none() && g.proposal.is_none()
            && service.hub_project.is_none() && service.home_project.is_none() && s.projects.len()==count
            && (service.cursor==plan::candidate_count())==(service.retry_at>0)
            && (service.cursor==0 || service.attempts>0);
    }
    let Some(plan)=&service.plan else {return false;};
    if service.retry_at!=0 || service.attempts==0 || service.cursor!=plan.candidate
        || plan.primary!=g.stockpile || !plan::valid(plan,s,count) {return false;}
    let (Ok(hub),Ok(home))=(plan::hub_project(plan),plan::home_project(plan)) else {return false;};
    if !bounded(&hub) || !bounded(&home)
        || s.projects.iter().take(count).map(|p|p.cells.len()).sum::<usize>()+hub.cells.len()+home.cells.len()>4096 {
        return false;
    }
    let saved=|i,p:&Project|s.projects.get(i).is_some_and(|old|infill::same(old,p));
    let proposed=|p:&Project|g.proposal.as_ref().is_some_and(|old|infill::same(old,p));
    match service.stage {
        Stage::Survey=>false,
        Stage::HubMaterials=>s.projects.len()==count && service.hub_project.is_none()
            && service.home_project.is_none() && g.project.is_none() && proposed(&hub),
        Stage::HubBuilding=>s.projects.len()==count+1 && service.hub_project==Some(count)
            && service.home_project.is_none() && g.project==Some(count) && g.proposal.is_none() && saved(count,&hub),
        Stage::HomeMaterials=>s.projects.len()==count+1 && service.hub_project==Some(count)
            && service.home_project.is_none() && g.project.is_none() && proposed(&home)
            && saved(count,&hub) && state::terminal(s,count),
        Stage::HomeBuilding|Stage::Complete=>s.projects.len()==count+2 && service.hub_project==Some(count)
            && service.home_project==Some(count+1) && g.proposal.is_none() && saved(count,&hub) && saved(count+1,&home)
            && state::terminal(s,count) && if service.stage==Stage::Complete {
                g.project.is_none() && state::terminal(s,count+1)
            } else {g.project==Some(count+1)},
    }
}

fn bounded(p: &Project) -> bool {
    let mut cells=std::collections::BTreeSet::new();
    let mut layers=std::collections::BTreeMap::new();
    !p.cells.is_empty() && p.cells.len()<=1024 && p.cells.iter().all(|o| {
        *layers.entry(p.work_layer(*o)).or_insert(0usize)+=1;
        o.material().is_some() && o.at.iter().all(|n|n.unsigned_abs()<1<<27) && cells.insert(o.at)
    }) && layers.values().all(|n|*n<=jobs::construction::LIMIT)
}
