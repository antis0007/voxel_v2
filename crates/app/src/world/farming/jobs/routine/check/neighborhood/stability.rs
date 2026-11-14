use super::*;

#[derive(Default)]
pub(super) struct Homes { records:BTreeMap<usize,(String,String,BTreeMap<u32,Vec3>)> }
impl Homes {
    pub fn len(&self)->usize {self.records.len()}
    pub fn observe(&mut self,w:&World)->Result<()> {
        let s=site(w)?;
        for (&i,(project,household,homes)) in &self.records {
            ensure!(s.projects.get(i).is_some_and(|p|ron::to_string(p).is_ok_and(|p|&p==project)),"Completed project {i} changed");
            ensure!(s.households.iter().find(|h|h.project==i).is_some_and(|h|ron::to_string(h).is_ok_and(|h|&h==household)),"Completed household {i} changed");
            ensure!(homes.iter().all(|(id,pos)|w.travel.homes.get(id)==Some(pos)),"Completed home {i} was reassigned");
        }
        for h in &s.households {
            if !self.records.contains_key(&h.project)&&s.home_complete(w,h.project) {
                let homes=h.members.iter().map(|id|w.travel.homes.get(id).copied().map(|p|(*id,p)).context("Missing occupied home assignment")).collect::<Result<_>>()?;
                self.records.insert(h.project,(ron::to_string(&s.projects[h.project])?,ron::to_string(h)?,homes));
            }
        }
        Ok(())
    }
}
