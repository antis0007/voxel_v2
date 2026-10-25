//! Generator 47 workshops: a dark roof over the timber hall, with a supported
//! cross-gable entrance on the existing paved apron. No prop scatter or decals.
use super::{B, Building, Vec3};

pub(super) fn dress(mut building: Building) -> Building {
    if let Some(hall) = building.wings.first_mut() {
        hall.pal.roof = B::Basalt;
        building.workshop_entry = true;
    }
    building
}

pub(super) fn sample(building: &Building, q: Vec3, v: f32, th: f32, version: u32) -> Option<B> {
    let base = building.sample_circulation(q, v, th);
    let Some(hall) = building.wings.first() else {
        return base;
    };
    let Some(door) = hall.doors.iter().find(|door| door.side == 1) else {
        return base;
    };
    // The hall's existing walls, foundations, fixtures, floor and roof retain
    // priority. This also avoids punching the canopy through its parent roof.
    if base.is_some_and(|block| block != B::Air) {
        // Pale masonry carries the frame; timber panels and dark roof retain
        // their own values. Restrict to solid perimeter infill, above the floor.
        let edge = (q.x - hall.x0).abs() < v.max(0.7)
            || (q.x - hall.x0 - hall.width()).abs() < v.max(0.7)
            || (q.z - hall.z0).abs() < v.max(0.7)
            || (q.z - hall.z0 - hall.depth()).abs() < v.max(0.7);
        if version >= 48 && base == Some(B::Planks) && edge && (1.0..2.5).contains(&q.y) {
            return Some(B::Limestone);
        }
        // Gable tie and king post connect to the existing wall frame. Recolor
        // only the solid gable infill, never its doors, glass or roof.
        if base == Some(B::Planks) && q.y >= hall.top() {
            let x = q.x - hall.x0;
            let z = q.z - hall.z0 - hall.depth() * 0.5;
            let end = x < v.max(0.6) || x > hall.width() - v.max(0.6);
            if end && (q.y < hall.top() + v.max(0.5) || z.abs() < v.max(0.5) * 0.5) {
                return Some(B::Log);
            }
        }
        return base;
    }
    let x = q.x - hall.x0 - door.u;
    let z = q.z - hall.z0 - hall.depth();
    // The existing workshop apron extends 2 m from this wall; keep its survey,
    // terrain grading and path endpoint unchanged. Door approach stays central.
    if x.abs() > 3.2 || !(-0.6..=2.0).contains(&z) || q.y < v {
        return base;
    }
    let post = v.max(0.55);
    let front = z >= 2.0 - post;
    if front && (x.abs() - 2.6).abs() < post * 0.5 && q.y < 5.0 {
        return Some(if q.y < 1.8 {
            B::WeatheredBricks
        } else {
            B::Log
        });
    }
    // The first low pitch consumed the entire gable at 1 m resolution. This
    // steeper cross-gable leaves two visible infill courses under its ridge.
    let roof = 5.0 + (3.2 - x.abs());
    let thickness = (v * 1.7).max(0.55);
    if q.y <= roof && q.y > roof - thickness {
        // Timber fascia makes the entrance pitch legible against the main roof.
        return Some(if front && q.y < roof - v * 0.75 {
            B::Log
        } else {
            B::Basalt
        });
    }
    // A continuous lintel, carried by the front posts, leaves at least 3 m of
    // centre headroom above the 1 m floor. The small gable closes above it.
    if front && x.abs() <= 2.6 + post * 0.5 && (4.0..4.0 + post).contains(&q.y) {
        return Some(B::Log);
    }
    if front && q.y >= 4.0 + post && q.y < roof - thickness {
        return Some(if x.abs() < post * 0.5 {
            B::Log
        } else {
            B::Planks
        });
    }
    base
}
