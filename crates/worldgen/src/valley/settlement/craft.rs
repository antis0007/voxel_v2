//! Metre-space construction kit. Details widen to one cell at coarse scales.
//! All shapes produce ordinary editable blocks, including roof and window frames.
use super::*;

pub(super) fn house(role: u8, x: f32, y: f32, z: f32, w: f32, d: f32, v: f32) -> Option<B> {
    house_versioned(role, x, y, z, w, d, v, false)
}

pub(super) fn house_versioned(role: u8, x: f32, y: f32, z: f32, w: f32, d: f32, v: f32, connected: bool) -> Option<B> {
    let (hw, hd) = (w * 0.5, d * 0.5);
    let beam = v.max(0.25);
    let two = role == 1 || role == 4;
    let top = if two { 5.8 } else { 3.2 };
    let eave = v.max(0.65);
    if x.abs() > hw + eave || z.abs() > hd + eave { return None; }

    // A continuous gable with a thin shell, separate verge and capped ridge.
    // The old roof is explicitly cleared inside this envelope.
    let pitch = if two { 0.82 } else { 0.65 };
    let ridge = top + hw * pitch;
    let roof = top + (hw - x.abs()) * pitch;
    let under = roof - if connected { pitch * v } else { 0.0 };
    let shell = v.max(0.3);
    let chimney = x > hw - 1.8 && x < hw - 0.6 && z > 0.4 && z < 1.6;
    if chimney && y >= 0.0 && y < ridge + 0.8 {
        let cap = y >= ridge + 0.8 - v.max(0.3);
        return Some(if cap { B::Cobblestone } else { B::StoneBricks });
    }
    if y >= top - eave * pitch - if connected { pitch * v } else { 0.0 } {
        if y >= under && y < roof + shell {
            let verge = z.abs() > hd + eave - beam;
            let crest = x.abs() < beam * 0.5;
            // A metre-wide dark verge turns every step into a projecting tooth.
            // At coarse resolution keep the roof mass quiet; fine cells carry trim.
            return Some(if v <= 0.5 && (verge || crest) { B::Log } else { B::Planks });
        }
        // Fill the side-wall bearing up to the sampled roof underside.
        // A fixed wall top leaves air below rafters at different grid phases.
        if connected && y >= top && y < under
            && x.abs() <= hw && z.abs() <= hd
            && x.abs() >= hw - beam {
            return Some(B::Log);
        }
        if y < roof && y >= top && z.abs() >= hd - beam && z.abs() <= hd {
            let truss = x.abs() < beam * 0.5
                || v <= 0.5 && ((y - top).abs() < beam
                    || (y - (top + x.abs() * pitch)).abs() < beam * 0.5);
            return Some(if truss { B::Log } else { B::Limestone });
        }
        if y >= top { return Some(B::Air); }
        // Wall plate joins posts to rafters; overhang underside stays open.
        if x.abs() > hw || z.abs() > hd { return Some(B::Air); }
    }
    if x.abs() > hw || z.abs() > hd { return None; }
    let edge_x = x.abs() >= hw - beam;
    let edge_z = z.abs() >= hd - beam;
    if !(edge_x || edge_z) || y < v || y >= top { return None; }
    // Preserve the entire front working bay and the central entry aisle.
    let front = z < -hd + beam;
    if front && y < 2.6 && (role == 2 || x.abs() < v.max(1.0)) {
        return Some(B::Air);
    }
    let along = if edge_z { x } else { z };
    let span = if edge_z { hw } else { hd };
    let storey_y = if two && y >= 3.2 { y - 3.2 } else { y };
    let centre = span * 0.53;
    let pane_distance = (along.abs() - centre).abs();
    let window_half = 0.65f32.max(v * 0.5);
    let bottom = v.max(1.1);
    let upper = bottom + v.max(1.0);
    let window_bay = pane_distance <= window_half + beam;
    if window_bay && storey_y >= bottom && storey_y < upper + beam {
        let frame = pane_distance > window_half || storey_y < bottom + beam
            || storey_y >= upper;
        return Some(if frame { B::Log } else { B::Air });
    }
    let post = edge_x && edge_z || along.abs() < beam * 0.5;
    let rail = y >= top - beam || (two && (y - 3.0).abs() < beam * 0.5);
    Some(if y < 1.0 { B::StoneBricks } else if post || rail { B::Log } else { B::Limestone })
}

pub(super) fn paving(a: f32, b: f32, square: bool, service: bool) -> B {
    if square {
        // Large paving fields with a single border, rather than visual noise.
        return if a.abs() > 7.0 || b.abs() > 5.5 { B::Cobblestone } else { B::StoneBricks };
    }
    if service { return B::Gravel; }
    let spur = (a + 22.0).abs().min(a.abs()).min((a - 22.0).abs());
    let across = b.abs().min(spur);
    // A dirt centre worn by feet, gravel shoulders, occasional broad stones.
    let course = (a + b * 0.35).rem_euclid(5.0);
    if across < 0.8 && course < 1.3 { B::Cobblestone }
    else if across < 1.4 { B::Dirt }
    else { B::Gravel }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_bay_and_door_remain_open_at_supported_scales() {
        for v in [1.0, 0.5, 0.25] {
            for y in [1.2, 1.8, 2.4] {
                assert_eq!(house(2, 0.0, y, -3.5, 8.0, 7.0, v), Some(B::Air));
                assert_eq!(house(0, 0.0, y, -3.0, 7.0, 6.0, v), Some(B::Air));
            }
            // Roof has an overhang and a hollow loft, not a solid wedge.
            assert_eq!(house(2, 4.1, 3.2, 0.0, 8.0, 7.0, v), Some(B::Planks));
            assert_eq!(house(2, 0.0, 4.5, 0.0, 8.0, 7.0, v), Some(B::Air));
            assert_eq!(house(2, 6.0, 3.2, 0.0, 8.0, 7.0, v), None);
        }
    }
}

