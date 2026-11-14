//! Delta aggregation with bounded topology checks. Unchanged frames do no inventory work.
use super::*;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

type ItemKey = (u32, u32);
type Contribution = BTreeMap<ItemKey, (Item, u32)>;

#[derive(Clone, Default)]
pub struct View {
    pub index: Arc<Index>,
    pub freed: usize,
    pub rebuilds: u64,
    next: f64,
    revision: u64,
    anchor: Option<IVec3>,
    contents: HashMap<Source, (ops::Inventory, Contribution)>,
    totals: BTreeMap<ItemKey, (Item, u64)>,
}

impl View {
    pub fn invalidate(&mut self) { self.next = 0.0; self.anchor = None; }

    pub fn refresh(&mut self, world: &World, anchor: IVec3, now: f64) {
        let revision = world.block_data.storage_revision;
        if self.anchor == Some(anchor) && revision == self.revision && now < self.next { return; }
        let centre = (anchor.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
        let found = discover(world, centre, RADIUS_M, Some(anchor));
        let members: std::collections::HashSet<_> = found.iter().map(|c|c.source).collect();
        let removed: Vec<_> = self.contents.keys().copied().filter(|s|!members.contains(s)).collect();
        let mut changed = found != self.index.containers || revision != self.revision;
        for source in removed {
            if let Some((_, old)) = self.contents.remove(&source) { self.subtract(&old); changed=true; }
        }
        for c in &found {
            let slots=slots_of(world,c.source);
            if self.contents.get(&c.source).is_some_and(|(s,_)| *s == slots) { continue; }
            changed=true;
            if let Some((_, old)) = self.contents.remove(&c.source) { self.subtract(&old); }
            let mut contribution=Contribution::new();
            for s in slots.iter().flatten() {
                let row=contribution.entry(s.item.encode_u32()).or_insert((s.item,0));
                row.1 += s.count;
            }
            for (&key,&(item,n)) in &contribution {
                self.totals.entry(key).or_insert((item,0)).1 += u64::from(n);
            }
            self.contents.insert(c.source,(slots,contribution));
        }
        if changed {
            let mut stock: Vec<_> = self.totals.iter().filter(|(_,(_,n))|*n>0).map(|(&key,&(item,total))| {
                let sources: Vec<_> = found.iter().enumerate().filter_map(|(i,c)|self.contents[&c.source].1.get(&key).map(|(_,n)|(i,*n))).collect();
                let available = sources.iter().map(|&(i,n)| u64::from(n.saturating_sub(policy::get(world, found[i].pos).reserve))).sum();
                Stock{item,total,available,sources}
            }).collect();
            stock.sort_by_key(|s|sorting::sort_key(s.item));
            let free_slots=self.contents.values().map(|(s,_)|s.iter().filter(|s|s.is_none()).count()).sum();
            self.index=Arc::new(Index{containers:found.clone(),stock,free_slots});
            self.freed=consolidate_preview(world,&found);
            self.rebuilds+=1;
        }
        self.anchor=Some(anchor);
        self.revision=revision;
        self.next=now+0.25;
    }

    fn subtract(&mut self, contribution:&Contribution) {
        for (key,(_,n)) in contribution {
            if let Some(row)=self.totals.get_mut(key) { row.1-=u64::from(*n); if row.1==0 {self.totals.remove(key);} }
        }
    }
}
