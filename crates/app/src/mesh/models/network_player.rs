//! Shared remote-character presentation; metre-sized, joint-local authored colour blocks.
use super::Model;
use glam::{Quat,Vec3};
mod kit;
#[cfg(test)]
mod preview;

pub(super) fn review(m:&mut Model,player:&crate::player::Player,time:f32) {
    static REVIEW:std::sync::LazyLock<bool>=std::sync::LazyLock::new(|| {
        std::env::args().any(|a|a.starts_with("--benchmark-"))
            && std::env::var("VOXEL_REVIEW_AVATARS").is_ok_and(|s|s=="1")
    });
    if !*REVIEW {return;}
    let origin=player.pos*m.scale+player.flat_forward()*3.6;
    m.with_surface_kind(super::AUTHORED_SURFACE,|m| {
        for (index,offset) in [-1.65,-0.55,0.55,1.65].into_iter().enumerate() {
            draw(m,origin+player.right_dir()*offset,player.yaw+std::f32::consts::PI,
                if index==1 {0.25}else{0.0},time*5.0,
                if index==0 {0.0}else{0.8},index==3,0.0,index as u32);
        }
    });
}

pub(super) fn draw(m:&mut Model,origin:Vec3,yaw:f32,pitch:f32,phase:f32,stride:f32,flying:bool,attack:f32,id:u32) {
    let stride_n=stride.clamp(0.0,1.0);
    // Highest at mid-stance (legs vertical), lowest at full stride, as in a real gait.
    let origin=origin+Vec3::Y*(2.0*phase).cos()*0.035*stride_n;
    let lean=if flying {0.0}else{0.09*stride_n};
    let twist=(attack.clamp(0.0,1.0)*std::f32::consts::PI).sin()*0.28+phase.sin()*0.06*stride_n;
    let body=Quat::from_rotation_y(-yaw)*Quat::from_rotation_y(-twist)*Quat::from_rotation_x(-lean);
    let coat=match id%4 {0=>[0.14,0.29,0.34],1=>[0.34,0.19,0.12],
        2=>[0.20,0.29,0.15],_=>[0.28,0.15,0.29]};
    let dark=coat.map(|c|c*0.65);
    let light=coat.map(|c|c*1.18);
    let skin=match (id/4)%4 {0=>[0.56,0.34,0.21],1=>[0.31,0.17,0.105],2=>[0.67,0.46,0.31],_=>[0.42,0.25,0.15]};
    let leather=[0.16,0.09,0.055];
    let trouser=[0.11,0.14,0.18];
    let swing=phase.sin()*stride.clamp(0.0,1.0)*0.68;
    let mut part=|pivot:Vec3,rotation:Quat,offset:Vec3,size:Vec3,color:[f32;3]| {
        m.cuboid(origin+body*(pivot+rotation*offset),size,body*rotation,color,0.0);
    };
    let i=Quat::IDENTITY;
    // Readable shoulders, waist and tunic; attached panels are intentional clusters.
    part(Vec3::ZERO,i,Vec3::new(0.0,1.13,0.0),Vec3::new(0.46,0.58,0.26),coat);
    part(Vec3::ZERO,i,Vec3::new(0.0,0.88,0.0),Vec3::new(0.47,0.075,0.275),leather);
    part(Vec3::ZERO,i,Vec3::new(0.0,0.88,-0.145),Vec3::new(0.08,0.055,0.015),[0.62,0.39,0.14]);
    part(Vec3::ZERO,i,Vec3::new(0.0,1.12,-0.136),Vec3::new(0.035,0.42,0.014),dark);
    part(Vec3::ZERO,i,Vec3::new(-0.14,1.25,-0.14),Vec3::new(0.10,0.12,0.018),light);
    part(Vec3::ZERO,i,Vec3::new(0.17,0.88,0.13),Vec3::new(0.13,0.16,0.08),leather);
    part(Vec3::ZERO,i,Vec3::new(0.0,1.40,0.0),Vec3::new(0.24,0.07,0.28),dark);
    for side in [-1.0,1.0] {
        let hip=Vec3::new(side*0.125,0.84,0.0);
        let leg=Quat::from_rotation_x(if flying {0.18}else{side*swing});
        // The knee folds only while its leg swings forward, so the planted leg stays straight.
        let fold=if flying {0.35}else{(side*phase.cos()).max(0.0)*1.1*stride_n};
        let knee=hip+leg*Vec3::new(0.0,-0.36,0.0);
        let shin=leg*Quat::from_rotation_x(-fold);
        part(hip,leg,Vec3::new(0.0,-0.18,0.0),Vec3::new(0.21,0.37,0.24),trouser);
        part(knee,shin,Vec3::new(0.0,-0.17,0.0),Vec3::new(0.205,0.35,0.235),trouser);
        part(knee,shin,Vec3::new(0.0,-0.36,-0.025),Vec3::new(0.23,0.20,0.29),leather);
        part(knee,shin,Vec3::new(0.0,-0.22,-0.126),Vec3::new(0.17,0.055,0.015),[0.25,0.16,0.095]);
        let shoulder=Vec3::new(side*0.335,1.38,0.0);
        let arm=arm_pose(side,swing,flying,attack);
        part(shoulder,arm,Vec3::new(0.0,-0.25,0.0),Vec3::new(0.20,0.50,0.23),coat);
        part(shoulder,arm,Vec3::new(0.0,-0.43,-0.005),Vec3::new(0.205,0.065,0.24),dark);
        part(shoulder,arm,Vec3::new(0.0,-0.54,0.0),Vec3::new(0.18,0.16,0.20),skin);
        part(shoulder,arm,Vec3::new(side*0.101,-0.13,0.0),Vec3::new(0.012,0.18,0.14),light);
    }
    let neck=Vec3::new(0.0,1.43,0.0);
    // Undo the torso lean and twist so the gaze stays on the aim.
    let head=Quat::from_rotation_x(lean)*Quat::from_rotation_y(twist)*Quat::from_rotation_x(pitch.clamp(-1.1,1.1));
    part(neck,head,Vec3::new(0.0,0.20,0.0),Vec3::new(0.37,0.40,0.35),skin);
    part(neck,head,Vec3::new(0.0,0.39,0.0),Vec3::new(0.39,0.08,0.37),[0.10,0.065,0.045]);
    part(neck,head,Vec3::new(0.0,0.25,0.18),Vec3::new(0.38,0.25,0.025),[0.10,0.065,0.045]);
    for side in [-1.0,1.0] {
        part(neck,head,Vec3::new(side*0.095,0.23,-0.179),Vec3::new(0.075,0.045,0.012),[0.80,0.76,0.60]);
        part(neck,head,Vec3::new(side*0.083,0.23,-0.187),Vec3::new(0.028,0.045,0.009),[0.055,0.075,0.08]);
        part(neck,head,Vec3::new(side*0.10,0.29,-0.18),Vec3::new(0.09,0.025,0.012),[0.10,0.065,0.045]);
    }
    part(neck,head,Vec3::new(0.0,0.16,-0.185),Vec3::new(0.045,0.055,0.035),[0.48,0.27,0.16]);
    part(neck,head,Vec3::new(0.0,0.085,-0.181),Vec3::new(0.09,0.018,0.012),[0.25,0.12,0.085]);
    kit::draw(m,origin,body,head,id);
    cloth_pixels(m, origin, body, swing, flying, attack, coat, trouser);
}

fn arm_pose(side:f32, swing:f32, flying:bool, attack:f32)->Quat {
    let reach=if side>0.0 {(attack.clamp(0.0,1.0)*std::f32::consts::PI).sin()}else{0.0};
    Quat::from_rotation_z(side*if flying {0.32}else{0.035}-reach*0.25)
        *Quat::from_rotation_x((-side*swing*1.1+swing.abs()*0.25)*(1.0-reach)+reach*1.65)
}

// Deliberate connected cloth clusters on the same joints as their host limbs.
// Shallow front planes preserve the block silhouette instead of adding greebles.
fn cloth_pixels(m:&mut Model, origin:Vec3, body:Quat, swing:f32, flying:bool, attack:f32, coat:[f32;3], trouser:[f32;3]) {
    let mut panel=|pivot:Vec3, joint:Quat, top:Vec3, texel:f32, rows:&[&str], base:[f32;3]| {
        let q=body*joint;
        for (y,row) in rows.iter().enumerate() { for (x,key) in row.bytes().enumerate() {
            let factor=match key {b's'=>0.84,b'h'=>1.12,_=>continue};
            let c=base.map(|v|(v*factor).min(1.0));
            let a=top+Vec3::new(x as f32*texel,-(y as f32)*texel,0.0);
            let points=[a,a+Vec3::X*texel,a+Vec3::new(texel,-texel,0.0),a-Vec3::Y*texel]
                .map(|p|origin+body*pivot+q*p);
            m.triangle([points[0],points[1],points[2]],q*Vec3::NEG_Z,c,0.0);
            m.triangle([points[0],points[2],points[3]],q*Vec3::NEG_Z,c,0.0);
        }}
    };
    panel(Vec3::ZERO,Quat::IDENTITY,Vec3::new(-0.224,1.37,-0.151),0.028,&[
        "hhhh........hhhh", "..hh........hh..", "................", "ss............ss",
        "s..............s", "................", "................", "................",
        "s..............s", "ss............ss", "ss............ss", "s..............s",
        "................", "...hh......hh...", "..hhh......hhh..", "................",
    ],coat);
    for side in [-1.0,1.0] {
        let leg=Quat::from_rotation_x(if flying {0.18}else{side*swing});
        panel(Vec3::new(side*0.125,0.84,0.0),leg,Vec3::new(-0.09,-0.24,-0.121),0.03,&[
            "......", ".hhh..", "hhhh..", "......", "....ss", "...sss", "......", "s.....",
        ],trouser);
        let arm=arm_pose(side,swing,flying,attack);
        panel(Vec3::new(side*0.335,1.38,0.0),arm,Vec3::new(-0.08,-0.03,-0.116),0.04,&[
            "hhhh", "..hh", "....", "....", "s...", "ss..", "....", "..hh", "...h",
        ],coat);
    }
}
