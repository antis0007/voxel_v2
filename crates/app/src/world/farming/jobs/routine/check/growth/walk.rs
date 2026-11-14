use super::*;

pub(super) fn check(w:&World,p:&mut crate::player::Player,entry:[i32;3],destination:[i32;3])->Result<f32> {
    let base=IVec3::from(w.farming.jobs.quarter.as_ref().context("Missing city")?.origin).as_vec3();
    let entry=IVec3::from(entry).as_vec3()+Vec3::new(0.5,0.0,0.5);
    let interior=IVec3::from(destination).as_vec3()+Vec3::new(0.5,0.0,0.5);
    p.pos=base+Vec3::new(10.5,2.0,14.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;
    let points=[Vec3::new(p.pos.x,entry.y,entry.z),entry,interior];
    let mut travelled=0.0;
    for target in points {
        let mut arrived=false;
        for _ in 0..2400 {
            let delta=(target-p.pos)*Vec3::new(1.0,0.0,1.0);
            if delta.length()<0.23 {arrived=true;break;}
            p.yaw=delta.x.atan2(-delta.z);p.pitch=0.0;
            let before=p.pos;let mut input=Input::default();input.forward=true;
            crate::player::update_player(p,&mut input,w,crate::simulation::STEP_SECONDS);
            travelled+=p.pos.distance(before);
        }
        ensure!(arrived,"New home grounded route blocked: feet={:?}, target={target:?}",p.pos);
    }
    ensure!(!p.flying&&(p.pos.y-interior.y).abs()<0.15,"New home interior is not grounded");Ok(travelled)
}
