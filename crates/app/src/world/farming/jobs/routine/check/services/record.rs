use super::*;
#[derive(serde::Serialize)]
pub(super) struct Cycles {
    started:u64,
    baseline:BTreeMap<u32,Resident>,
    served:BTreeMap<u32,u32>,
    meals:BTreeMap<u32,u64>,
    social:BTreeMap<u32,(u64,u32)>,
    finished:BTreeSet<u32>,
}
impl Cycles {
    pub fn new(w:&World,ids:&[u32])->Self {
        Self{started:w.farming.clock,baseline:ids.iter().filter_map(|&id|crew::view(&w.farming.jobs,id).map(|a|(id,a.routine.clone()))).collect(),
            served:w.farming.jobs.production.as_ref().map(|p|p.served.clone()).unwrap_or_default(),
            meals:BTreeMap::new(),social:BTreeMap::new(),finished:BTreeSet::new()}
    }
    pub fn observe(&mut self,w:&World) {
        let Some(p)=&w.farming.jobs.production else{return;};
        for (&id,before) in &self.baseline {
            let Some(a)=crew::view(&w.farming.jobs,id) else{continue;};let r=&a.routine;
            if p.served.get(&id).copied().unwrap_or(0)>self.served.get(&id).copied().unwrap_or(0) {self.meals.entry(id).or_insert(w.farming.clock);}
            if let Some(&meal)=self.meals.get(&id)&&meal>self.started&&r.rested_at>=meal&&r.social_at>=r.rested_at
                &&r.rests>before.rests&&r.socials>before.socials {self.social.entry(id).or_insert((r.social_at,r.resumed));}
            if let Some(&(at,resumed))=self.social.get(&id)&&w.farming.clock>at&&r.resumed>resumed {self.finished.insert(id);}
        }
    }
    pub fn done(&self,ids:&[u32])->bool {ids.iter().all(|id|self.finished.contains(id))}
}
pub(super) fn sidecar(w:&mut World,root:&Path,label:&str)->Result<()> {
    let before=ron::to_string(&w.farming.jobs)?;
    ensure!(before.len()<=256*1024,"Service jobs exceed256KiB");
    let bytes=crate::world::farming::sidecar::encode(w);
    let (restored,_)=crate::world::farming::sidecar::decode(&bytes)?;
    ensure!(ron::to_string(&restored.jobs)?==before,"Service sidecar changed plans/cargo");
    w.farming=restored;std::fs::write(root.join(format!("{label}.vxf")),bytes)?;Ok(())
}
pub(super) fn reload(root:&Path,w:&mut World,p:&mut crate::player::Player,day:&mut crate::world::DayNightCycle,label:&str,step:usize,out:&mut Vec<serde_json::Value>)->Result<()> {
    let hub=frontier::hub_plan(&w.farming.jobs).map(|h|(h.stockpile,w.block_data.chest(h.stockpile)));
    let balance=ledger::timber(w);let at=Instant::now();
    super::super::reload::full(root,w,p,day)?;
    #[cfg(test)]
    if label=="courier-cargo" { jobs::supply::throughput_tests::capture(root,w)?; }
    ensure!(hub.as_ref().is_none_or(|(at,stock)|w.block_data.chest(*at)==*stock),"Service hub inventory changed across full checkpoint");
    ensure!(ledger::timber(w)==balance,"Service timber changed across full checkpoint");
    std::fs::copy(root.join("pre-rest-checkpoint-residents.json"),root.join(format!("{label}-saved-residents.json")))?;
    out.push(serde_json::json!({"label":label,"step":step,"ms":at.elapsed().as_secs_f64()*1000.0,"hub_inventory_exact":true}));Ok(())
}

pub(super) fn targets(w:&World)->serde_json::Value {
    let Some(p)=&w.farming.jobs.production else{return serde_json::Value::Null;};
    let bills:Vec<_>=p.bills.iter().filter(|b|matches!(b.recipe.as_str(),"Oak planks"|"Chest"))
        .map(|b|serde_json::json!({"recipe":b.recipe,"worker":b.worker,"station":b.station,"once":b.once,"target":b.target})).collect();
    serde_json::json!({"woodlot":production::woodlot_target(p),"bills":bills})
}

pub(super) fn home_access(w:&World,index:usize)->Result<serde_json::Value> {
    let s=site(w)?;let p=s.projects.get(index).context("Diagnostic project missing")?;
    let home=p.home.as_ref().context("Diagnostic home missing")?;
    let (a,b)=(IVec3::from(home.entry),IVec3::from(home.destination));let v=w.voxel_scale.meters();
    let mut q=jobs::navigation::Query::new(w,v);
    let corridor=jobs::clear_level_walk(&mut q,jobs::metres(a,v),jobs::metres(b,v),v);
    let cells:Vec<_>=(a.x-1..=b.x+1).flat_map(|x|(a.z.min(b.z)-1..=a.z.max(b.z)+1)
        .flat_map(move|z|(a.y-1..=a.y+2).map(move|y|IVec3::new(x,y,z))))
        .map(|at|serde_json::json!({"at":at.to_array(),"block":format!("{:?}",known(w,at))})).collect();
    Ok(serde_json::json!({"project":index,"entry":home.entry,"destination":home.destination,
        "entry_stand":jobs::stand(w,a,v),"destination_stand":jobs::stand(w,b,v),"corridor_clear":corridor,
        "paid_cells":p.cells.iter().filter(|o|known(w,o.cell())==Some(o.block())).count(),"planned_cells":p.cells.len(),"cells":cells}))
}
