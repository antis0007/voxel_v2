//! Disposable terrain survey and materialization share the same layout plan.
use super::*;
use crate::world::BlockId as B;

pub(in super::super) fn height([x,z]:Point)->Option<i32> {
    if x.abs()>X || z.abs()>Z {return None;}
    let ridge=(x+25).abs()+z.abs();
    Some(if ridge<=3 {2} else if ridge<=5 {1} else {0})
}

pub(in super::super) fn edits(plan:&Plan,base:IVec3)->Result<Vec<(IVec3,B)>,String> {
    let paving=plan.paving();
    let mut out=Vec::new();
    for z in -Z..=Z { for x in -X..=X {
        let h=height([x,z]).unwrap_or(0);
        for y in 0..=h {
            let block=if y<h {B::Stone} else if paving.contains(&[x,z]) {B::Cobblestone} else {B::Grass};
            out.push((base+IVec3::new(x,y,z),block));
        }
    }}
    let styles=traditions()?;
    let mut occupied=BTreeSet::new();
    for (i,parcel) in plan.parcels.iter().enumerate() {
        let lot=parcel.lot(base,i==0);
        for (cell,b) in geometry::plan(&lot,&styles[lot.tradition]) {
            let at=geometry::at(&lot,cell); occupied.insert([at.x-base.x,at.z-base.z]);
            if i>0 {out.push((at,b));}
        }
    }
    // A grove uses only unclaimed, level ground beyond street shoulders.
    for [x,z] in [[28,9],[38,12],[30,23],[41,24],[-43,-5],[-31,28]] {
        if (-3..=3).any(|dx| (-3..=3).any(|dz| paving.contains(&[x+dx,z+dz])
            || occupied.contains(&[x+dx,z+dz]) || height([x+dx,z+dz])!=Some(0))) {continue;}
        for y in 1..=4 {out.push((base+IVec3::new(x,y,z),B::Log));}
        for dy in 3..=5 {for dx in -2i32..=2 {for dz in -2i32..=2 {
            if dx.abs()+dz.abs()<4 {out.push((base+IVec3::new(x+dx,dy,z+dz),B::Leaves));}
        }}}
    }
    Ok(out)
}
