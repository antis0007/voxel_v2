use super::*;

#[derive(Default)]
pub(in super::super) struct Activity {
    pub previous:BTreeMap<u32,Vec3>,
    pub moved:BTreeMap<u32,f32>,
    pub minimum_live:usize,
    pub jumps:Vec<serde_json::Value>,
}
impl Activity {
    pub fn sample(&mut self,w:&World,ids:&[u32],step:usize)->Result<()> {
        ensure!(w.farming.jobs.production.as_ref().is_some_and(|p|p.workers==ids),"Production roster changed");
        ensure!(crew::saved_workers(w).len()==ids.len(),"Saved worker roster changed");
        let mut live=HashSet::new();let mut current=BTreeMap::new();
        for a in w.adventure.enemies.iter().filter(|a|a.health>0.0) {
            if let Some(id)=vehicle::villager_id(a) {
                ensure!(ids.contains(&id),"Unrequested physical resident spawned");
                ensure!(live.insert(id),"Duplicate physical resident {id}");
                ensure!(a.kind==vehicle::VILLAGER,"Resident kind changed");
                if let Some(last)=self.previous.get(&id) {
                    let distance=last.distance(a.pos);
                    if distance<3.0 {*self.moved.entry(id).or_default()+=distance;}
                    else if self.jumps.len()<128 {self.jumps.push(serde_json::json!({"id":id,"step":step,"from":last.to_array(),"to":a.pos.to_array(),"distance":distance}));}
                }
                current.insert(id,a.pos);
            }
        }
        self.previous=current;
        ensure!(live.len()==ids.len(),"A resident disappeared outside checkpoint rematerialization");
        self.minimum_live=if self.minimum_live==0 {live.len()} else {self.minimum_live.min(live.len())};Ok(())
    }
}
pub(in super::super) fn settle(w:&mut World) {
    let mut drops=std::mem::take(&mut w.inventory.dropped);
    for drop in &mut drops {
        let next=drop.pos-Vec3::Y*(crate::simulation::STEP_SECONDS*2.5).min(0.1);
        if w.get(((next-Vec3::Y*0.12)/w.voxel_scale.meters()).floor().as_ivec3()).is_empty(){drop.pos=next;}
    }
    w.inventory.dropped=drops;
}
pub(in super::super) fn write(root:&Path,w:&World,record:&Activity)->Result<()> {
    let rows:Vec<_>=crew::saved_workers(w).into_iter().filter_map(|(id,_)|crew::view(&w.farming.jobs,id).map(|a|
        serde_json::json!({"id":id,"feet":vehicle::villager_actor(w,id).map(|e|e.pos.to_array()),"routine":a.routine,
            "task":format!("{:?}",a.task),"status":a.status,"cargo":a.carry,"distance_m":record.moved.get(&id)}))).collect();
    std::fs::write(root.join("growth-residents.json"),serde_json::to_vec_pretty(&rows)?)?;
    std::fs::write(root.join("growth-jobs.ron"),ron::to_string(&w.farming.jobs)?)?;Ok(())
}

pub(in super::super) fn timber(w:&World)->serde_json::Value {
    use crate::world::inventory::Item;
    let j=&w.farming.jobs;let pile=j.stockpile.map(IVec3::from).unwrap_or(IVec3::ZERO);
    let stock=w.block_data.chest(pile);let base=pile-IVec3::Y;
    let published:HashSet<_>=w.sandbox.bodies.iter().filter(|b|b.felled_tree).flat_map(|b|b.occupied.iter().map(|(at,_)|*at)).collect();
    let mut overlap=0u32;let mut standing=0u32;
    for x in -24..=24 {for z in -24..=24 {for y in 0..=10 {let at=base+IVec3::new(x,y,z);if w.get(at)==B::Log {standing+=1;overlap+=u32::from(published.contains(&at));}}}}
    let body_logs=w.sandbox.bodies.iter().filter(|b|b.felled_tree)
        .map(|b|b.voxels.iter().filter(|(_,b)|*b==B::Log).count() as u32).sum::<u32>();
    let counts=|block| {
        let item=Item::Block(block);
        serde_json::json!({"stock":production::count(&stock,item),"cargo":crew::cargo_item(j,item),
            "drops":w.inventory.dropped.iter().filter(|d|d.stack.item==item).map(|d|d.stack.count).sum::<u32>(),
            "player":w.inventory.count_item(item),"paid":j.quarter.as_ref().and_then(|q|q.frontier.as_ref()).into_iter()
                .flat_map(|s|s.projects.iter()).flat_map(|p|p.cells.iter()).filter(|o|o.block()==block&&known(w,o.cell())==Some(block)).count()})
    };
    serde_json::json!({"total":timber_balance(w),"terrain_logs":standing-overlap,"raw_world_logs":standing,"published_body_logs":overlap,"body_logs":body_logs,
        "logs":counts(B::Log),"planks":counts(B::Planks),"chests":counts(B::Chest)})
}
