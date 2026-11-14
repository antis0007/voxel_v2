//! Lazy physical columns for repeatable, bounded planning polls.
use super::*;
use jobs::known;
use std::{cell::{Cell, RefCell}, collections::BTreeMap};

pub(super) struct Survey<'a> {
    world: &'a World,
    base: IVec3,
    reserved: BTreeSet<Point>,
    columns: RefCell<BTreeMap<Point,bool>>,
    pub anchors: Vec<(Point,[usize;2])>,
    pub reads: Cell<u64>,
}

impl<'a> Survey<'a> {
    pub fn new(w: &'a World, s: &Site, plan: &InfillPlan) -> Result<Self,String> {
        let base=IVec3::from(plan.base);
        let j=&w.farming.jobs;
        if j.stockpile!=Some(plan.stockpile) {return Err("Stockpile Changed".into());}
        let prod=j.production.as_ref().ok_or("Production Unavailable")?;
        let mut reserved=BTreeSet::new();
        for &(x,_,z) in w.farming.plots.keys() {
            let p=[x-base.x,z-base.z]; if bounded(p) {reserved.insert(p);}
        }
        for b in &prod.beds {
            let margin=if b.crop==B::Sapling {(4.5/w.voxel_scale.meters()).ceil() as i32}else{0};
            for dx in -margin..=margin {for dz in -margin..=margin {
                let p=[b.at[0]-base.x+dx,b.at[2]-base.z+dz];
                if bounded(p) {reserved.insert(p);}
            }}
        }
        for p in s.timber.iter().chain(j.mine.iter().map(|(p,_)|p)) {
            reserved.insert([p[0]-base.x,p[2]-base.z]);
        }
        for o in s.projects.iter().flat_map(|p|&p.cells).chain(&j.build) {
            reserved.insert(point(base,o.cell()));
        }
        let mut anchors=Vec::new();
        let mut reads=0;
        for (i,p) in s.projects.iter().enumerate() {
            reads+=p.cells.len() as u64;
            if !s.home_complete(w,i) {continue;}
            for (k,o) in p.cells.iter().enumerate() {
                if o.block()!=B::Planks || o.cell().y!=base.y+1 || !bounded(point(base,o.cell())) {continue;}
                reads+=4;
                if known(w,o.cell())==Some(B::Planks)
                    && jobs::stand(w,o.cell()+IVec3::Y,w.voxel_scale.meters())
                    && (0..=2).all(|dy|!crate::world::farming::base::overlaps(w,o.cell()+IVec3::Y*dy,o.cell()+IVec3::Y*dy)) {
                    anchors.push((point(base,o.cell()),[i,k]));
                }
            }
        }
        Ok(Self {world:w,base,reserved,columns:RefCell::new(BTreeMap::new()),anchors,reads:Cell::new(reads)})
    }
    pub fn free(&self, p: Point) -> bool {
        if !bounded(p) || self.reserved.contains(&p) {return false;}
        if let Some(free)=self.columns.borrow().get(&p) {return *free;}
        let mut scans=Scans::default();
        let free=survey::column(self.world,self.base,p,&mut scans);
        self.reads.set(self.reads.get().saturating_add(scans.cells));
        self.columns.borrow_mut().insert(p,free);
        free
    }
    pub fn fits(&self, p: &InfillParcel) -> bool {
        (p.footprint[0]..=p.footprint[2]).all(|x|
            (p.footprint[1]..=p.footprint[3]).all(|z|self.free([x,z])))
    }
}

pub(super) fn revalidate(w: &World, s: &Site, plan: &InfillPlan, parcel: &InfillParcel,
    scans: &mut Scans) -> Result<(),String> {
    let survey=Survey::new(w,s,plan)?;
    let result=(|| {
        if !survey.anchors.iter().any(|a|a.1==parcel.anchor && parcel.access.last()==Some(&a.0)) {
            return Err("Street Disconnected".into());
        }
        if !survey.fits(parcel) {return Err("Site Obstructed or Reserved".into());}
        if parcel.access.iter().take(parcel.access.len().saturating_sub(1)).any(|p|!survey.free(*p)) {
            return Err("Access Obstructed or Reserved".into());
        }
        let project=parcel.build(IVec3::from(plan.base))?;
        let radius=(jobs::RANGE/w.voxel_scale.meters()).powi(2);
        if project.cells.iter().any(|o|(o.cell()-IVec3::from(plan.stockpile)).as_vec3().length_squared()>radius) {
            return Err("Stockpile Out of Range".into());
        }
        Ok(())
    })();
    scans.cells=scans.cells.saturating_add(survey.reads.get());
    result
}
