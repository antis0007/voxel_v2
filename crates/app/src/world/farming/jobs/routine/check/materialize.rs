use super::*;

pub(super) struct Pending {
    started:usize,
    label:String,
    expected:BTreeMap<u32,Vec3>,
    first:BTreeMap<u32,(usize,Vec3,u32)>,
}
impl Pending {
    pub fn new(w:&World,started:usize,label:impl Into<String>)->Self {
        Self {started,label:label.into(),expected:crew::saved_workers(w).into_iter().collect(),first:BTreeMap::new()}
    }
    pub fn observe(&mut self,root:&Path,w:&World,step:usize)->Result<Option<serde_json::Value>> {
        let mut live=HashSet::new();
        for actor in &w.adventure.enemies {
            if actor.health<=0.0 {continue;}
            if let Some(id)=vehicle::villager_id(actor) {
                ensure!(self.expected.contains_key(&id),"Unexpected resident during rematerialization");
                ensure!(live.insert(id),"Duplicate live resident {id}");
                self.first.entry(id).or_insert((step,actor.pos,actor.kind));
            }
        }
        let ready=live.len()==self.expected.len();
        let timed_out=step.saturating_sub(self.started)>180;
        if !ready&&!timed_out {return Ok(None);}
        let rows:Vec<_>=self.expected.iter().map(|(&id,&saved)| {
            let first=self.first.get(&id);
            serde_json::json!({"id":id,"saved_position":saved.to_array(),
                "first_position":first.map(|(_,p,_)|p.to_array()),"first_step":first.map(|(s,_,_)|s),
                "kind":first.map(|(_,_,k)|k),"body_metres":first.map(|(_,_,k)|vehicle::resident_dimensions(*k).to_array()),"distance_from_saved":first.map(|(_,p,_)|p.distance(saved))})
        }).collect();
        let stable=self.expected.iter().all(|(id,saved)|self.first.get(id)
            .is_some_and(|(_,p,kind)|p.distance(*saved)<0.025&&*kind==vehicle::VILLAGER));
        let report=serde_json::json!({"stage":self.label,"started":self.started,"finished":step,
            "ticks":step.saturating_sub(self.started),"expected":self.expected.len(),"live":live.len(),
            "identity_pose_and_kind_preserved":stable,"pose_tolerance_m":0.025,"observation_before_adventure_movement":true,"residents":rows});
        std::fs::write(root.join(format!("materialization-{}.json",self.label)),serde_json::to_vec_pretty(&report)?)?;
        ensure!(ready,"Resident rematerialization exceeded180ticks");
        ensure!(stable,"Resident saved transform or kind changed during {} materialization",self.label);
        Ok(Some(report))
    }
}

pub(super) fn admit(w:&mut World)->Result<usize> {
    let v=w.voxel_scale.meters();let mut cells=Vec::new();
    cells.extend(crew::saved_workers(w).into_iter().map(|(_,p)|(p/v).floor().as_ivec3()));
    if let Some(p)=w.farming.jobs.stockpile {cells.push(p.into());}
    if let Some(p)=&w.farming.jobs.production {cells.extend(p.beds.iter().map(|b|IVec3::from(b.at)));}
    if let Some(s)=w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()) {
        cells.extend(s.projects.iter().flat_map(|p|p.cells.iter()).map(|o|o.cell()));
    }
    let mut chunks=std::collections::BTreeSet::new();
    for p in cells {for x in [-2,2] {for y in [-2,3] {for z in [-2,2] {
        chunks.insert(engine_core::chunk_coord_from_world_block(p+IVec3::new(x,y,z)).to_array());
    }}}}
    ensure!(chunks.len()<=64,"Daily working set exceeds64chunks");
    for &at in &chunks {ensure!(w.admit_edit_chunk(at.into()),"Daily working chunk unavailable");}
    Ok(chunks.len())
}
