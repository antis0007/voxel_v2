use super::*;

// The original nested trees are the differential oracle, including allocation sizes.
pub(super) fn group(frame: Frame, missing: &BTreeSet<[i32;3]>) -> Result<PreparedGroups> {
    let terrain=frame.chunk.as_ref().map(|c|c.coord);
    if let Some(coord)=terrain {ensure!(missing.contains(&coord),"unexpected terrain frame chunk");}
    let mut groups=BTreeMap::<[i32;3],BTreeMap<[i32;3],BlockId>>::new();
    if let Some(chunk)=frame.chunk {
        let base=IVec3::from_array(chunk.coord)*32;
        let edits=groups.entry(chunk.coord).or_default();let mut index=0;
        for (count,block) in chunk.runs {for _ in 0..count {
            let p=base+IVec3::new(index%32,index/1024,(index/32)%32);
            edits.insert(p.to_array(),BlockId::from_u32(block));index+=1;
        }}
    }
    for (p,block) in frame.cells {
        let coord=world::chunk_coord_from_world_block(IVec3::from_array(p)).to_array();
        ensure!(!missing.contains(&coord)||terrain==Some(coord),"terrain edit precedes its chunk");
        groups.entry(coord).or_default().insert(p,BlockId::from_u32(block));
    }
    let groups=groups.into_iter().map(|(coord,edits)|Group {
        coord:IVec3::from_array(coord),edits:edits.into_iter().map(|(p,b)|(IVec3::from_array(p),b)).collect(),
        next:0,staged:None,
    }).collect();
    Ok((terrain,groups))
}
