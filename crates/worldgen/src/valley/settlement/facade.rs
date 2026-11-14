//! Generator 61 smith cottage: mirrored bays, complete reveals, quiet materials.
//! All dimensions are metres; the caller aligns the frame to voxel boundaries.
use super::*;

fn window(along: f32, y: f32, centres: [f32; 2], v: f32, beam: B) -> Option<B> {
    let t = v.max(0.25);
    let bottom = if v >= 1.0 { 2.0 } else { 1.5 };
    for centre in centres {
        let d = (along - centre).abs();
        let clear = if v >= 1.0 { 0.5 } else { 0.5_f32.max(v) };
        if d < clear && (bottom..3.0).contains(&y) { return Some(B::Air); }
        if d < clear + t {
            if (bottom-t..bottom).contains(&y) { return Some(B::DressedStone); }
            if (3.0..3.0+t).contains(&y) { return Some(beam); }
            if (bottom..3.0).contains(&y) { return Some(B::TimberPost); }
        }
    }
    None
}

pub(super) fn sample(x: f32, y: f32, z: f32, v: f32, x_is_world_x: bool) -> Option<B> {
    let t = v.max(0.25);
    let eave = if v >= 1.0 { 0.0 } else { 0.5 };
    if x.abs() >= 4.0 + eave || z.abs() >= 4.0 + eave || !(-6.0..9.0).contains(&y) { return None; }
    let bx = if x_is_world_x { B::TimberBeamX } else { B::TimberBeamZ };
    let bz = if x_is_world_x { B::TimberBeamZ } else { B::TimberBeamX };
    // A continuous tile stair: only the rising edge needs a concealed overlap.
    let roof_row = ((4.0 + (4.0 - x.abs()) * 0.5) / v).floor();
    let outer_row = ((4.0 + (4.0 - x.abs() - v) * 0.5) / v).floor();
    let row = (y / v).floor();
    if row == roof_row || row == roof_row - 1.0 && outer_row < roof_row {
        if row < roof_row && z.abs() >= 4.0 - t {
            return Some(if z.abs() < 4.0 && x.abs() < 4.0 {
                if (4.0-t..4.0).contains(&y) { bx } else { B::LimePlaster }
            } else { B::Air });
        }
        return Some(B::RoofTiles);
    }
    if x.abs() >= 4.0 || z.abs() >= 4.0 {
        // Recessed openings have a modest projecting outer sill and lintel.
        if v <= 0.5 && y < 3.0+t {
            let front_back = z.abs() >= 4.0 && z.abs() < 4.0+t && x.abs() < 4.0;
            let side = x.abs() >= 4.0 && x.abs() < 4.0+t && z.abs() < 4.0;
            if front_back {
                if z < 0.0 && x.abs() < 1.0+t && (3.0..3.0+t).contains(&y) { return Some(bx); }
                if let Some(b) = window(x,y,[-2.5,2.5],v,bx) { if b != B::Air { return Some(b); } }
            }
            if side {
                if let Some(b) = window(z,y,[-2.0,2.0],v,bz) { if b != B::Air { return Some(b); } }
            }
        }
        if z < -4.0 && x.abs() < 1.5 && y < v { return Some(B::DressedStone); }
        return if y >= v { Some(B::Air) } else { None };
    }
    if y < v { return Some(if y < 0.0 || x.abs() >= 4.0-t || z.abs() >= 4.0-t { B::DressedStone } else { B::Planks }); }
    if row >= roof_row { return Some(B::Air); }
    let end = z.abs() >= 4.0-t;
    let side = x.abs() >= 4.0-t;
    if end || side && y < 4.0 {
        if z < -4.0+t {
            if x.abs() < 1.0 && y < 3.0 { return Some(B::Air); }
            if x.abs() >= 1.0 && x.abs() < 1.0+t && y < 3.0 { return Some(B::TimberPost); }
            if x.abs() < 1.0+t && (3.0..3.0+t).contains(&y) { return Some(bx); }
        }
        let along = if end { x } else { z };
        let centres = if end { [-2.5,2.5] } else { [-2.0,2.0] };
        if let Some(b) = window(along,y,centres,v,if end { bx } else { bz }) { return Some(b); }
        let corner = end && side && y < 4.0-t;
        let plate = (4.0-t..4.0).contains(&y);
        return Some(if y < 1.0 { B::DressedStone }
            else if corner { B::TimberPost }
            else if plate { if end { bx } else { bz } }
            else { B::LimePlaster });
    }
    // Furniture sits against the back wall, leaving the door and central aisle clear.
    if y >= v && y < v+1.0 && z > 2.5 && z < 3.5 {
        if x < -1.5 && x > -3.0 { return Some(B::Workbench); }
        if x > 1.5 && x < 2.5 { return Some(B::Smelter); }
    }
    Some(B::Air)
}
