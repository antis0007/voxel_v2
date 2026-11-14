//! Distinguish persisted roster entries from living, moving physical actors.
use super::*;
use crate::world::adventure::wildlife::vehicle;

#[derive(Default)]
pub(super) struct Record {
    previous:BTreeMap<u32,Vec3>,
    distance:BTreeMap<u32,f32>,
    samples:usize,
    minimum:Option<usize>,
    maximum:usize,
    discontinuities:usize,
}
fn live(w:&World)->Vec<(u32,Vec3)> {
    w.adventure.enemies.iter().filter(|e|e.health>0.0).filter_map(|e| {
        let id=vehicle::villager_id(e)?;
        jobs::crew::view(&w.farming.jobs,id).map(|_|(id,e.pos))
    }).collect()
}
impl Record {
    pub(super) fn sample(&mut self,w:&World)->Result<()> {
        let actors=live(w);
        let current:BTreeMap<_,_>=actors.iter().copied().collect();
        ensure!(actors.len()==current.len(),"Duplicate physical resident identities");
        self.samples+=1;
        self.minimum=Some(self.minimum.map_or(actors.len(),|n|n.min(actors.len())));
        self.maximum=self.maximum.max(actors.len());
        for (id,feet) in &current {
            if let Some(last)=self.previous.get(id) {
                let distance=feet.distance(*last);
                // A 0.25 s sample cannot count a spawn/teleport as walking.
                if distance<=3.0 {*self.distance.entry(*id).or_default()+=distance;}
                else {self.discontinuities+=1;}
            }
        }
        self.previous=current;
        Ok(())
    }
    pub(super) fn report(&self,w:&World)->serde_json::Value {
        let final_ids:Vec<_>=live(w).into_iter().map(|(id,_)|id).collect();
        let moving:Vec<_>=self.distance.iter().filter(|(_,m)|**m>=1.0).map(|(id,_)|*id).collect();
        serde_json::json!({"sample_interval_ticks":30,"samples":self.samples,
            "minimum_active_count":self.minimum,"maximum_active_count":self.maximum,
            "final_active_count":final_ids.len(),"final_active_ids":final_ids,
            "moving_resident_count":moving.len(),"moving_resident_ids":moving,
            "sampled_distance_m":self.distance,"excluded_discontinuities":self.discontinuities})
    }
}
