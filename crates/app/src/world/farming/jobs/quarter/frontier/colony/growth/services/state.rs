use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage { Survey, HubMaterials, HubBuilding, HomeMaterials, HomeBuilding, Complete }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReservedBudget {
    pub calls: u32,
    pub path_node_budget: u64,
    pub chunk_admission_budget: u64,
}

impl ReservedBudget {
    pub(super) fn reserve(&mut self) {
        self.calls=self.calls.saturating_add(1);
        self.path_node_budget=u64::from(self.calls)*1100;
        self.chunk_admission_budget=u64::from(self.calls);
    }
    pub(super) fn valid(&self) -> bool {
        self.path_node_budget==u64::from(self.calls)*1100 && self.chunk_admission_budget==u64::from(self.calls)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Services {
    pub base_project_count: usize,
    pub plan: Option<plan::Plan>,
    pub cursor: usize,
    pub retry_at: u64,
    pub attempts: u32,
    pub reserved_budget: ReservedBudget,
    pub stage: Stage,
    pub hub_project: Option<usize>,
    pub home_project: Option<usize>,
    pub targets: targets::Targets,
}

impl Services {
    pub(super) fn new(count: usize, targets: targets::Targets) -> Self {
        Self { base_project_count:count, plan:None, cursor:0, retry_at:0, attempts:0, reserved_budget:ReservedBudget::default(),
            stage:Stage::Survey, hub_project:None, home_project:None, targets }
    }
}

pub(super) fn eligible(g: &State, s: &Site, demand: u32) -> bool {
    g.version==3 && g.services.is_none() && repeated::valid(g,s) && !g.complete
        && g.project.is_none() && g.proposal.is_none() && demand>0
        && (6..=infill::PROJECT_LIMIT-2).contains(&s.projects.len())
        && (0..s.projects.len()).all(|i|terminal(s,i))
        && g.waiting=="No Space: Local Capacity Exhausted"
        && g.infill.as_ref().is_some_and(|p|p.schema==2 && p.surveyed && p.attempts>0
            && !p.searching() && p.pending().is_none())
}

pub(super) fn terminal(s: &Site, i: usize) -> bool {
    s.projects.get(i).zip(s.project_layers.get(i)).is_some_and(|(p,layer)|
        *layer>p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap_or(0)+1)
}

pub(super) fn upgrade(w: &World, s: &Site, g: &mut State, demand: u32) -> Result<(),String> {
    if !eligible(g,s,demand) {return Err("Settled Exhausted Growth 3 With Housing Demand Required".into());}
    if !w.farming.jobs.build.is_empty() || !w.farming.jobs.claims.is_empty()
        || (0..s.projects.len()).any(|i|!s.home_complete(w,i)) {
        return Err("Settled Project Reconciliation Required".into());
    }
    if w.farming.jobs.stockpile!=Some(g.stockpile)
        || !known(w,g.stockpile.into()).is_some_and(|b|b.chest_tier().is_some()) {
        return Err("Stockpile Unavailable".into());
    }
    let prod=w.farming.jobs.production.as_ref().ok_or("Production Unavailable")?;
    let services=Services::new(s.projects.len(),targets::Targets::capture(prod)?);
    g.services=Some(services);g.version=4;g.next_poll=0;
    g.waiting="Service Survey Pending".into();
    Ok(())
}

pub(super) fn admit(s: &mut Site, g: &mut State, service: &mut Services) -> Result<(),String> {
    let (expected,next)=match service.stage {
        Stage::HubMaterials=>(service.base_project_count,Stage::HubBuilding),
        Stage::HomeMaterials=>(service.base_project_count+1,Stage::HomeBuilding),
        _=>return Err("Material Stage Required".into()),
    };
    if s.projects.len()!=expected || g.project.is_some() {return Err("Project Ledger Changed".into());}
    let p=g.proposal.as_ref().ok_or("Proposal Missing")?;
    let layer=p.cells.iter().map(|o|o.cell().y).min().ok_or("Proposal Empty")?;
    if s.projects.len()>=infill::PROJECT_LIMIT || p.cells.len()>1024
        || s.projects.iter().map(|p|p.cells.len()).sum::<usize>()+p.cells.len()>4096 {
        return Err("Service Ledger Limit".into());
    }
    let p=g.proposal.take().ok_or("Proposal Missing")?;
    s.projects.push(p);s.project_layers.push(layer);g.project=Some(expected);
    if next==Stage::HubBuilding {service.hub_project=Some(expected);}
    else {service.home_project=Some(expected);}
    service.stage=next;
    Ok(())
}

pub(super) fn finished(s: &Site, g: &mut State, service: &mut Services) -> Result<(),String> {
    let expected=match service.stage {
        Stage::HubBuilding=>service.hub_project,
        Stage::HomeBuilding=>service.home_project,
        _=>return Err("Building Stage Required".into()),
    }.ok_or("Service Project Missing")?;
    if g.project!=Some(expected) || !terminal(s,expected) {return Err("Project Incomplete".into());}
    if service.stage==Stage::HubBuilding {
        g.proposal=Some(plan::home_project(service.plan.as_ref().ok_or("Service Plan Missing")?)?);
        service.stage=Stage::HomeMaterials;
    } else {service.stage=Stage::Complete;g.complete=true;}
    g.project=None;
    Ok(())
}
