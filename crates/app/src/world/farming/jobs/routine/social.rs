use super::*;

pub(super) fn goal(w:&World,j:&mut State,id:u32,entry:IVec3,destination:IVec3,members:&[u32])->Option<IVec3> {
    let now=w.farming.clock;
    j.meetings.retain(|m| !(m.done[0]&&m.done[1]) && now.saturating_sub(m.started)<=TIMEOUT);
    let index=members.iter().position(|&member|member==id)?;
    let first=index/2*2;
    let ids=[*members.get(first)?,*members.get(first+1)?];
    let side=usize::from(id==ids[1]);
    let mut found=j.meetings.iter().position(|m|m.ids==ids);
    if found.is_none() {
        if j.meetings.len()>=crew::CAPACITY/2 || j.meetings.iter().any(|m|m.ids.iter().any(|other|ids.contains(other))) {return None;}
        let inward=(destination-entry)/2;
        let across=IVec3::new(inward.z,0,-inward.x);
        let offset=if first==0 {-2} else {1};
        let points=[entry+across*offset,entry+across*(offset+1)];
        if !points.iter().all(|&p|stand(w,p,w.voxel_scale.meters())) {return None;}
        j.meetings.push(Meeting{ids,points:points.map(|p|p.to_array()),started:now,last:now,dwell:0,done:[false;2]});
        found=Some(j.meetings.len()-1);
    }
    let at=found?;
    let points=j.meetings[at].points;
    let other=crew::view(j,ids[1-side]);
    let participating=other.is_some_and(|a|a.routine.phase==Phase::Social)
        && home(w,j,ids[1-side]).is_some();
    let physical=participating && ids.iter().zip(points).all(|(&member,point)|
        crew::physical(w,j,member).is_some_and(|at|
            arrived(at,metres(point.into(),w.voxel_scale.meters()),w.voxel_scale.meters())))
        && crate::world::trace::solid(w,metres(points[0].into(),w.voxel_scale.meters())+Vec3::Y*0.7,
            metres(points[1].into(),w.voxel_scale.meters())+Vec3::Y*0.7).is_none();
    let m=&mut j.meetings[at];
    m.observe(now,physical,PULSE,SOCIAL_TICKS);
    if m.dwell>=SOCIAL_TICKS && !m.done[side] {
        let first_completion = !m.done[0] && !m.done[1];
        m.done[side]=true;
        j.routine.socials=j.routine.socials.saturating_add(1);
        j.routine.social_need=0;
        j.routine.social_partner=Some(ids[1-side]);
        j.routine.social_at=now;
        j.routine.enter(Phase::Work,now); j.routine.due=now.saturating_add(WORK_TICKS+u64::from(id%8)*PULSE);
        if first_completion { super::super::groups::met(j,w.seed,ids,now); }
    }
    Some(points[side].into())
}

