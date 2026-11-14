//! Match the live single-player driver's loose-drop gravity in headless runs.
//! This runs in main.rs outside fixed::gameplay, so the headless driver must
//! explicitly include it. Without it, canopy drops remain suspended forever.
use super::*;

pub(super) fn settle(w:&mut World) {
    let mut drops=std::mem::take(&mut w.inventory.dropped);
    let scale=w.voxel_scale.meters();
    for drop in &mut drops {
        let next=drop.pos-Vec3::Y*(crate::simulation::STEP_SECONDS*2.5).min(0.1);
        if w.get(((next-Vec3::Y*0.12)/scale).floor().as_ivec3()).is_empty() {drop.pos=next;}
    }
    w.inventory.dropped=drops;
}
