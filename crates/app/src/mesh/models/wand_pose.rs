//! Data-driven attachments and layered held-equipment poses.
use super::wand::WandView;
use glam::{Mat3, Quat, Vec3};
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Clone, Copy, Deserialize)]
struct Key {
    offset: [f32;3],
    axis: [f32;3],
    roll: f32,
}

/// Finger joint for the RON hand meshes in `wand_asset`.
#[derive(Clone, Copy, Deserialize)]
pub struct Bone {
    pub pivot: [f32;3],
    pub axis: [f32;3],
    pub angle: f32,
}

#[derive(Deserialize)]
pub struct Rig {
    pub scale: f32,
    pub fov_degrees: f32,
    pub left_handed: bool,
    rest: Key, aim: Key, inspect: Key, interact: Key, sprint: Key,
    windup_offset: [f32;3], release_offset: [f32;3],
    elbow: [f32;3],
    pub bones: [Bone;5],
}

pub static RIG: LazyLock<Rig> = LazyLock::new(|| {
    // Review runs may frame from a file so placement iterates without a rebuild.
    let review = std::env::args().any(|a| a.starts_with("--benchmark-"))
        .then(|| std::env::var("VOXEL_VIEWMODEL_RIG").ok()).flatten()
        .and_then(|path| std::fs::read_to_string(&path)
            .inspect_err(|error| eprintln!("[viewmodel] rig {path}: {error}")).ok())
        .and_then(|text| ron::from_str(&text)
            .inspect_err(|error| eprintln!("[viewmodel] rig parse: {error}")).ok());
    review.unwrap_or_else(|| ron::from_str(include_str!("../../../../../assets/runtime/viewmodels/presentation.ron"))
        .unwrap_or_else(|error| panic!("invalid held-equipment rig: {error}")))
});

pub fn smooth(t: f32) -> f32 {
    let t=t.clamp(0.0,1.0);
    t*t*(3.0-2.0*t)
}

pub struct Pose {
    pub grip: Vec3,
    pub rotation: Quat,
    pub cast: f32,
    pub cast_left: f32,
    pub time: f32,
    pub fingers: f32,
}

fn camera(v: &WandView, local: Vec3) -> Vec3 {
    v.right*local.x + v.up*local.y + v.forward*local.z
}

/// Shoulder-side end of the sleeve; it follows part of the hand's lag so the
/// whole forearm swings instead of pivoting at a camera-fixed point.
pub fn elbow(v:&WandView) -> Vec3 {
    let sway=v.motion.sway.value*0.6;
    v.eye+camera(v,Vec3::from_array(RIG.elbow)+Vec3::new(sway.x,sway.y,0.0))
}

fn review(v: &WandView) -> (f32,f32,f32,f32,f32,f32) {
    static OVERRIDE: LazyLock<Option<(Option<f32>,String)>> = LazyLock::new(|| {
        if !std::env::args().any(|a| a.starts_with("--benchmark-")) { return None; }
        Some((std::env::var("VOXEL_VIEWMODEL_TIME").ok().and_then(|t| t.parse().ok()),
            std::env::var("VOXEL_VIEWMODEL_POSE").unwrap_or_default()))
    });
    let (mut time,mut cast,mut aim,mut inspect,mut interact,mut speed)=
        (v.time,v.cast,v.motion.aim,v.motion.inspect,v.motion.interact,v.motion.speed);
    if let Some((fixed,mode)) = OVERRIDE.as_ref() {
        time=fixed.filter(|t| t.is_finite()).unwrap_or(time);
        match mode.as_str() {
            "aim" => aim=1.0,
            "inspect" => inspect=1.0,
            "interact" => interact=1.0,
            "sprint" => speed=8.5,
            "windup" => cast=0.82,
            "release" => cast=0.60,
            "cycle" => {
                let phase=time.rem_euclid(5.0);
                cast=if phase<0.4 { 1.0-phase/0.4 } else { 0.0 };
                inspect=smooth((phase-1.3)/0.5)*(1.0-smooth((phase-3.1)/0.5));
                aim=smooth((phase-3.7)/0.3)*(1.0-smooth((phase-4.5)/0.3));
            }
            mode => { cast=mode.parse::<f32>().ok().filter(|x| x.is_finite()).unwrap_or(cast); }
        }
    }
    (time,cast.clamp(0.0,1.0),aim,inspect,interact,speed)
}

pub fn pose(v: &WandView) -> Pose {
    let (time,left,aim,inspect,interact,pace)=review(v);
    let mut offset=Vec3::from_array(RIG.rest.offset);
    let mut axis=Vec3::from_array(RIG.rest.axis);
    let mut roll=RIG.rest.roll;
    // Lowered sprint carry between walk (5.5 m/s) and sprint (8.5 m/s); a cast
    // raises the hand again before the release.
    let sprint=((pace-6.0)/2.0).clamp(0.0,1.0)*(1.0-(left*4.0).min(1.0));
    for (key,weight) in [(RIG.sprint,sprint),(RIG.aim,aim),(RIG.inspect,inspect),(RIG.interact,interact)] {
        let t=smooth(weight);
        offset=offset.lerp(Vec3::from_array(key.offset),t);
        axis=axis.lerp(Vec3::from_array(key.axis),t);
        roll += (key.roll-roll)*t;
    }
    let progress=1.0-left;
    let prepare=smooth(progress/0.24)*(1.0-smooth((progress-0.24)/0.16));
    let release=smooth((progress-0.24)/0.16)*(1.0-smooth((progress-0.40)/0.60));
    offset += Vec3::from_array(RIG.windup_offset)*prepare
        + Vec3::from_array(RIG.release_offset)*release;
    let speed=(v.motion.speed/6.0).clamp(0.0,1.5)*(1.0-inspect);
    offset.x += v.motion.sway.value.x + v.motion.gait.sin()*0.009*speed;
    // Dip at mid-sway, matching held items (`held_item::frame`).
    offset.y += v.motion.sway.value.y - (v.motion.gait*2.0).cos()*0.006*speed
        + (time*1.3).sin()*0.0015 - v.boost*0.035 - v.motion.hit*0.025;
    offset.z -= v.boost*0.035 + v.motion.hit*0.030;
    // Holster from the wrist; changing equipment never changes gameplay timing.
    let equip=if v.motion.tier==0 { 1.0 } else { v.motion.equip };
    offset.y -= (1.0-smooth(equip))*0.35;
    roll += -prepare*0.12 + release*0.10 + v.motion.fidget*0.08 + v.motion.hit*0.15;
    axis.z += release*0.12;
    // Bank about the view axis with the lag spring and the stride, so turns,
    // strafes and steps lean the kit instead of only sliding it.
    let bank=Quat::from_axis_angle(v.forward,
        -v.motion.sway.value.x*2.3 + v.motion.gait.sin()*0.025*speed);
    let axis=(bank*camera(v,axis)).normalize_or_zero();
    let right=bank*v.right;
    let side=(right-axis*right.dot(axis)).normalize_or_zero();
    let rotation=Quat::from_mat3(&Mat3::from_cols(side,axis,side.cross(axis)))
        * Quat::from_rotation_y(roll);
    let grip=v.eye+camera(v,offset);
    Pose { grip,rotation,cast:release,cast_left:left,time,
        fingers:interact*0.60+inspect*0.16+v.motion.fidget*0.12-prepare*0.06 }
}

/// Preserve the depth projection while allowing independent held-equipment FOV.
pub fn present(v: &WandView, position: Vec3) -> Vec3 {
    let q=position-v.eye;
    let focal=36.0f32.to_radians().tan()/(RIG.fov_degrees.clamp(40.0,110.0)*0.5).to_radians().tan();
    let handed=if RIG.left_handed {-1.0} else {1.0};
    position+v.right*q.dot(v.right)*(focal*handed-1.0)+v.up*q.dot(v.up)*(focal-1.0)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> WandView {
        WandView { eye:Vec3::ZERO,forward:Vec3::NEG_Z,right:Vec3::X,up:Vec3::Y,time:0.0,cast:0.0,
            spell:[0.3,0.8,0.7],boost:0.0,
            motion:Default::default() }
    }

    #[test]
    fn strafe_lag_banks_the_kit_toward_the_motion() {
        let still=pose(&view());
        let mut v=view();
        // Strafing right leaves the spring trailing left.
        v.motion.sway.value.x = -0.03;
        let strafe=pose(&v);
        let lean=|p:&Pose| (p.rotation*Vec3::Y).dot(Vec3::X);
        assert!(lean(&strafe)>lean(&still)+0.03, "kit must bank clockwise into a right strafe");
        assert!(strafe.grip.x<still.grip.x);
    }

    #[test]
    fn sprint_lowers_the_carry_and_a_cast_restores_it() {
        let rest=pose(&view());
        let mut v=view();
        v.motion.speed=8.5;
        let sprint=pose(&v);
        assert!(sprint.grip.y<rest.grip.y-0.02, "sprint carry must drop the hand");
        v.cast=1.0;
        let casting=pose(&v);
        assert!((casting.grip.y-sprint.grip.y).abs()>0.02, "a cast leaves the sprint carry");
    }
}
