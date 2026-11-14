//! Rank bounded candidate pages before admitting one paid-access proposal.
use super::*;
mod routing;
#[cfg(test)] mod tests;
const SHORTLIST: usize = 16;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Search {
    pub cursor: u32,
    pub routing: bool,
    pub shortlist: Vec<Candidate>,
    pub after: Option<Candidate>,
    pub routes: u32,
    pub paused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Candidate {
    pub frontage: u32,
    pub centre: u32,
    pub provenance: u32,
    pub index: u32,
}

fn decode(index: u32) -> (Point,bool,bool) {
    let width=(GRID*2+1) as u32;
    let shape=index/(width*width);
    ([(index/width%width) as i32-GRID,(index%width) as i32-GRID],shape>=2,shape%2==1)
}

fn templates(base: IVec3) -> Result<Vec<Project>,String> {
    [false,true].into_iter().flat_map(|turn|[false,true].map(move|right|(turn,right)))
        .map(|(turn,right)|scene::infill([0,1,0],turn,right,Vec::new(),base)).collect()
}

fn parcel(index: u32, plan: &InfillPlan, templates: &[Project]) -> Result<InfillParcel,String> {
    let (origin,turn,stairs_right)=decode(index);
    let template=&templates[usize::from(turn)*2+usize::from(stairs_right)];
    let (b,e)=shape(template,IVec3::from(plan.base)).ok_or("House Geometry Invalid")?;
    let [x,z]=origin;
    Ok(InfillParcel {origin,turn,stairs_right,footprint:[b[0]+x,b[1]+z,b[2]+x,b[3]+z],
        entrance:[e[0]+x,e[1]+z],access:Vec::new(),anchor:[0,0],
        provenance:provenance(plan.seed,origin,turn,stairs_right),project:None})
}

fn centre(p: &InfillParcel, plan: &InfillPlan) -> u32 {
    let pile=point(IVec3::from(plan.base),IVec3::from(plan.stockpile));
    let x=p.footprint[0]+p.footprint[2]-2*pile[0];
    let z=p.footprint[1]+p.footprint[3]-2*pile[1];
    (x*x+z*z) as u32
}

pub(super) fn valid(plan: &InfillPlan) -> bool {
    let Some(search)=&plan.search else {return true;};
    if plan.schema!=2 || !plan.surveyed || plan.pending().is_some() || search.cursor>CANDIDATES
        || search.shortlist.len()>SHORTLIST || search.routes>ROUTES
        || (search.paused && search.routes!=ROUTES)
        || search.routing!=(search.cursor==CANDIDATES) || (search.routing && search.shortlist.is_empty())
        || search.shortlist.iter().map(|p|p.index).collect::<BTreeSet<_>>().len()!=search.shortlist.len()
        || search.after.as_ref().is_some_and(|a|search.shortlist.iter().any(|p|p.index==a.index))
        || search.shortlist.windows(2).any(|p|p[0]>=p[1]) {return false;}
    let Ok(templates)=templates(IVec3::from(plan.base)) else {return false;};
    search.shortlist.iter().chain(search.after.iter()).all(|candidate| {
        candidate.index<CANDIDATES && candidate.frontage<=(GRID*4) as u32
            && parcel(candidate.index,plan,&templates).is_ok_and(|p|
                candidate.centre==centre(&p,plan) && candidate.provenance==p.provenance)
    }) && search.shortlist.iter().all(|candidate|candidate.index<search.cursor)
        && search.after.as_ref().is_none_or(|after|search.shortlist.iter().all(|p|p>after))
}

pub(crate) fn poll(w: &World, s: &Site, plan: &mut InfillPlan, scans: &mut Scans) -> Result<bool,String> {
    if plan.schema!=2 {return Err("Repeatable Plan Required".into());}
    if plan.pending().is_some() {return Ok(true);}
    if plan.parcels.len()>=PROJECT_LIMIT-6 {return Err("Project Ledger Full".into());}
    if plan.search.is_none() {
        plan.retry_at=plan.retry_at.min(w.farming.clock.saturating_add(RETRY));
        if plan.surveyed && w.farming.clock<plan.retry_at {return Ok(false);}
        plan.surveyed=true;
        plan.attempts=plan.attempts.saturating_add(1);
        plan.search=Some(Search::default());
    }
    if let Some(search)=plan.search.as_mut().filter(|search|search.paused) {
        plan.retry_at=plan.retry_at.min(w.farming.clock.saturating_add(RETRY));
        if w.farming.clock<plan.retry_at {return Ok(false);}
        search.paused=false;search.routes=0;
        plan.attempts=plan.attempts.saturating_add(1);
    }
    let survey=local::Survey::new(w,s,plan)?;
    let result=step(w,plan,scans,&survey);
    scans.cells=scans.cells.saturating_add(survey.reads.get());
    result
}

fn step(w: &World, plan: &mut InfillPlan, scans: &mut Scans, survey: &local::Survey<'_>) -> Result<bool,String> {
    if survey.anchors.is_empty() {
        plan.search=None;plan.retry_at=w.farming.clock.saturating_add(RETRY);
        return Err("Street Disconnected: No Paid Frontage".into());
    }
    let base=IVec3::from(plan.base);
    let templates=templates(base)?;
    let mut search=plan.search.clone().ok_or("Survey Cursor Missing")?;
    if search.routing {
        if search.shortlist.is_empty() {return Err("Survey Shortlist Missing".into());}
        let candidate=search.shortlist.remove(0);
        let mut proposal=parcel(candidate.index,plan,&templates)?;
        search.after=Some(candidate);
        let fits=survey.fits(&proposal) && plan.parcels.iter().all(|old|compatible(old,&proposal));
        let mut requests=0;
        let connected=fits && routing::route(w,plan,&mut proposal,survey,scans,ROUTES-search.routes,&mut requests)?;
        search.routes=search.routes.saturating_add(requests);
        if connected && plan.parcels.iter().all(|old|compatible(old,&proposal)) {
            plan.parcels.push(proposal);
            plan.search=None;
            return Ok(true);
        }
        if search.shortlist.is_empty() {search.cursor=0;search.routing=false;}
        if search.routes==ROUTES {search.paused=true;plan.retry_at=w.farming.clock.saturating_add(RETRY);}
        plan.search=Some(search);
        return Ok(false);
    }
    let pile=IVec3::from(plan.stockpile);
    let radius=(jobs::RANGE/w.voxel_scale.meters()).powi(2);
    let stop=(search.cursor+CANDIDATES_PER_POLL).min(CANDIDATES);
    while search.cursor<stop {
        let index=search.cursor;search.cursor+=1;
        scans.candidates=scans.candidates.saturating_add(1);
        let p=parcel(index,plan,&templates)?;
        if !bounded([p.footprint[0],p.footprint[1]]) || !bounded([p.footprint[2],p.footprint[3]]) {continue;}
        let template=&templates[usize::from(p.turn)*2+usize::from(p.stairs_right)];
        let delta=IVec3::new(p.origin[0],0,p.origin[1]);
        if template.cells.iter().any(|o|(o.cell()+delta-pile).as_vec3().length_squared()>radius)
            || plan.parcels.iter().any(|old|!compatible(old,&p)) || !survey.fits(&p) {continue;}
        let Some(frontage)=routing::starts(template,base).into_iter().flat_map(|start|
            survey.anchors.iter().map(move |(end,_)|
                (start[0]+p.origin[0]-end[0]).unsigned_abs()+(start[1]+p.origin[1]-end[1]).unsigned_abs())).min() else {continue;};
        let candidate=Candidate {frontage,centre:centre(&p,plan),provenance:p.provenance,index};
        if search.after.as_ref().is_some_and(|after|candidate<=*after) {continue;}
        let at=search.shortlist.binary_search(&candidate).unwrap_or_else(|i|i);
        if at<SHORTLIST {search.shortlist.insert(at,candidate);search.shortlist.truncate(SHORTLIST);}
    }
    if search.cursor==CANDIDATES {
        if search.shortlist.is_empty() {
            plan.retry_at=w.farming.clock.saturating_add(RETRY);
            plan.search=None;
            return Err(if search.after.is_some() {"Street Search Limit: No Connected Parcel"}
                else {"No Space: Local Capacity Exhausted"}.into());
        }
        search.routing=true;
    }
    plan.search=Some(search);
    Ok(false)
}
