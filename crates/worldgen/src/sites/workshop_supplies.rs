//! A broken mine crossing with a sealed sump. The bridge and the drain use
//! ordinary blocks, so player edits and water use the existing save path.
use super::{B, Vec3, WorldSampleConfig};

pub(super) fn enabled(cfg: &WorldSampleConfig) -> bool {
    crate::features::enabled(cfg, crate::features::Feature::WorkshopSupplies)
        && cfg.generator_version >= engine_voxel::WORKSHOP_SUPPLY_GENERATOR
}

pub(super) fn sample(q: Vec3, v: f32) -> Option<B> {
    let x = q.x;
    let depth = -q.z;
    if !(2.6..13.0).contains(&x) || !(8.0..21.0).contains(&depth) {
        return None;
    }
    // Match the centre of the decline opening at 11 metres. y=0 is the
    // walking surface, with the stone deck immediately below it.
    let deck = (-11.0_f32 * 0.45 / v).floor() * v + v;
    let y = q.y - deck;
    if !(-6.0..4.0).contains(&y) {
        return None;
    }

    if depth >= 14.0 {
        if depth < 15.0 && (6.0..7.0).contains(&x) && (-2.0..-2.0 + v).contains(&y) {
            // A low outlet drains into the sump without exposing a drop in the floor.
            return Some(B::Planks);
        }
        if (15.0..20.0).contains(&depth) && (5.0..9.0).contains(&x) && (-5.0..-1.0).contains(&y) {
            return Some(B::Air);
        }
        if depth < 15.0 || ((4.0..10.0).contains(&x) && y < 0.0) {
            return Some(B::StoneBricks);
        }
        return None;
    }

    let entry = (10.0..12.0).contains(&depth);
    if y >= 3.0 || y < -2.0 || x >= 12.0 || depth < 9.0 {
        return Some(B::StoneBricks);
    }
    if x < 3.6 && !entry && y >= 0.0 {
        return Some(if (9.0..10.0).contains(&depth) || (12.0..13.0).contains(&depth) {
            B::Log
        } else {
            B::StoneBricks
        });
    }
    if y < 0.0 {
        if (5.0..9.0).contains(&x) {
            // Stair ledges leave a way out of the channel from either side.
            if !(6.0..8.0).contains(&x) && y < -1.0 {
                return Some(B::DressedStone);
            }
            // The dry top row leaves room to place a bridge at deck level.
            return Some(if y < -v { B::Water } else { B::Air });
        }
        return Some(B::DressedStone);
    }
    if y < v {
        if (11.0..11.0 + v).contains(&x) && (11.0..11.0 + v).contains(&depth) {
            return Some(B::SurveyCache);
        }
        if ((4.0..4.0 + v).contains(&x) || (10.0..10.0 + v).contains(&x))
            && (13.0..13.0 + v).contains(&depth)
        {
            return Some(B::Torch);
        }
    }
    Some(B::Air)
}
