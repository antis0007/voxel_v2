use super::*;
use jobs::known;

pub(super) struct Survey {
    pub free: BTreeSet<Point>,
    pub anchors: Vec<(Point,[usize;2])>,
}

impl Survey {
    pub fn new(w: &World, s: &Site, plan: &InfillPlan, scans: &mut Scans) -> Result<Self,String> {
        let base = IVec3::from(plan.base);
        let j = &w.farming.jobs;
        if j.stockpile != Some(plan.stockpile) { return Err("Stockpile Changed".into()); }
        let prod = j.production.as_ref().ok_or("Production Unavailable")?;
        let mut reserved = BTreeSet::new();
        for &(x,_,z) in w.farming.plots.keys() { reserved.insert([x-base.x,z-base.z]); }
        for b in &prod.beds {
            let margin = if b.crop==B::Sapling { (4.5/w.voxel_scale.meters()).ceil() as i32 } else {0};
            for dx in -margin..=margin { for dz in -margin..=margin {
                reserved.insert([b.at[0]-base.x+dx,b.at[2]-base.z+dz]);
            }}
        }
        for p in s.timber.iter().chain(j.mine.iter().map(|(p,_)|p)) {
            reserved.insert([p[0]-base.x,p[2]-base.z]);
        }
        for o in s.projects.iter().flat_map(|p|&p.cells).chain(&j.build) {
            reserved.insert(point(base,o.cell()));
        }
        let mut free = BTreeSet::new();
        for x in -GRID..=GRID { for z in -GRID..=GRID {
            let p=[x,z];
            if reserved.contains(&p) {continue;}
            if column(w,base,p,scans) {free.insert(p);}
        }}
        let mut anchors = Vec::new();
        for (i,p) in s.projects.iter().enumerate() {
            if !s.home_complete(w,i) {continue;}
            for (k,o) in p.cells.iter().enumerate() {
                scans.cells=scans.cells.saturating_add(3);
                if o.block()==B::Planks && o.cell().y==base.y+1
                    && bounded(point(base,o.cell())) && known(w,o.cell())==Some(B::Planks)
                    && jobs::stand(w,o.cell()+IVec3::Y,w.voxel_scale.meters())
                    && (1..=3).all(|dy|!crate::world::farming::base::overlaps(w,o.cell()+IVec3::Y*(dy-1),o.cell()+IVec3::Y*(dy-1))) {
                    anchors.push((point(base,o.cell()),[i,k]));
                }
            }
        }
        Ok(Self {free,anchors})
    }
}

pub(super) fn column(w: &World, base: IVec3, p: Point, scans: &mut Scans) -> bool {
    scans.cells = scans.cells.saturating_add(1);
    let floor = at(base,p);
    if !known(w,floor-IVec3::Y).is_some_and(B::is_solid) {return false;}
    for dy in 0..=5 {
        scans.cells = scans.cells.saturating_add(1);
        let cell=floor+IVec3::Y*dy;
        if known(w,cell)!=Some(B::Air) || crate::world::farming::base::overlaps(w,cell,cell) {return false;}
    }
    true
}

pub(crate) fn revalidate(w: &World, s: &Site, plan: &InfillPlan, parcel: &InfillParcel,
    scans: &mut Scans) -> Result<(), String> {
    if plan.schema==2 {return local::revalidate(w,s,plan,parcel,scans);}
    let survey = Survey::new(w,s,plan,scans)?;
    if !survey.anchors.iter().any(|a|a.1==parcel.anchor && parcel.access.last()==Some(&a.0)) {
        return Err("Street Disconnected".into());
    }
    for x in parcel.footprint[0]..=parcel.footprint[2] { for z in parcel.footprint[1]..=parcel.footprint[3] {
        if !survey.free.contains(&[x,z]) {return Err("Site Obstructed or Reserved".into());}
    }}
    for p in parcel.access.iter().take(parcel.access.len().saturating_sub(1)) {
        if !survey.free.contains(p) {return Err("Access Obstructed or Reserved".into());}
    }
    let p=parcel.build(IVec3::from(plan.base))?;
    let range_squared=(jobs::RANGE/w.voxel_scale.meters()).powi(2);
    if p.cells.iter().any(|o|(o.cell()-IVec3::from(plan.stockpile)).as_vec3().length_squared()>range_squared) {
        return Err("Stockpile Out of Range".into());
    }
    Ok(())
}
