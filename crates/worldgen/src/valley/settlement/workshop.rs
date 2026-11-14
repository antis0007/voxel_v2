//! A smith's hall, measured in metres. One sampler owns shell, voids and contents.
//! The two-metre centre aisle and perimeter service road stay unobstructed.
use super::*;

pub(super) fn sample(x: f32, y: f32, z: f32, v: f32) -> Option<B> {
    let (hw, hd) = (5.0_f32, 4.5_f32);
    let t = v.max(0.3);
    let eave = 0.75_f32.max(v);
    let roof = 5.4 + (hw - x.abs()) * 0.78;
    if x.abs() > hw + eave || z < -hd - 2.0 || z > hd + eave { return None; }
    if y < -6.0 || y > 11.0 { return None; }

    // Solid hearth/chimney on the rear right: the flue has a real hollow core.
    if (x - 3.5).abs() < 1.25 && (z - 2.8).abs() < 1.25 && y >= 0.0 && y < 10.8 {
        if y > 2.0 && (x - 3.5).abs() < 0.45 && (z - 2.8).abs() < 0.45 {
            return Some(B::Air);
        }
        return Some(if y > 10.0 { B::CarvedStone } else { B::StoneBricks });
    }
    // A lower lean-to porch establishes a second roof mass and sheltered work.
    if z < -hd {
        if x.abs() > hw { return None; }
        if y < v { return Some(if y < -v { B::StoneBricks } else { B::Cobblestone }); }
        let porch = 3.5 + (z + hd + 2.0) * 0.35;
        if y >= porch && y < porch + t { return Some(B::RoofTiles); }
        if x.abs() > hw - t && z < -hd - 2.0 + t && y >= v && y < porch {
            return Some(B::Log);
        }
        if y > 2.7 && y < porch && x.abs() > hw - 1.0
            && y > 2.7 + (hw - x.abs()) { return Some(B::Log); }
        return Some(B::Air);
    }
    if z.abs() <= hd + eave && y >= roof && y < roof + t {
        return Some(B::RoofTiles);
    }
    if x.abs() > hw || z > hd { return if y < v { None } else { Some(B::Air) }; }
    if y < v { return Some(if y < 0.0 { B::StoneBricks } else { B::Cobblestone }); }
    if y >= roof { return Some(B::Air); }
    let front = z < -hd + t;
    let back = z > hd - t;
    let side = x.abs() > hw - t;
    // Broad arch-like timber opening: high centre, lower supported shoulders.
    if front {
        let opening = if x.abs() < 1.7 { 4.2 } else if x.abs() < 3.5 { 3.4 } else { 0.0 };
        if y < opening { return Some(B::Air); }
        if y < opening + t || x.abs() > hw - t || x.abs() < t * 0.5 && y > 5.0 {
            return Some(B::Log);
        }
        // Two clerestory openings let daylight into the work bay.
        if (x.abs() - 2.3).abs() < 0.8 && y > 4.3 && y < 5.3 { return Some(B::Air); }
        return Some(B::Limestone);
    }
    if back || side {
        let along = if side { z } else { x };
        let post = (along.abs() - 3.5).abs() < t * 0.5 || along.abs() < t * 0.5;
        if !post && y > 2.2 && y < 4.4 && (along.abs() - 1.8).abs() < 1.0 {
            return Some(B::Air);
        }
        return Some(if y < 1.2 { B::StoneBricks } else if post || (y-5.0).abs() < t * 0.5 {
            B::Log
        } else { B::Limestone });
    }
    // Visible roof ties sit above all circulation, supported by side wall posts.
    if y > 5.0 && y < 5.0 + t && (z.abs() - 2.5).abs() < t * 0.5 { return Some(B::Log); }
    // Raised side work surfaces, bins and real processing/storage block types.
    if x < -3.5 && z > -1.5 && z < 2.5 {
        if y < v + 0.7 { return Some(B::Planks); }
        if y < v + 1.7 && (z + 0.5).abs() < v * 0.5 { return Some(B::Workbench); }
        if y > 3.0 && y < 3.0 + t { return Some(B::Planks); }
    }
    if (x-3.5).abs() < 0.7 && (z+0.5).abs() < 0.7 && y < v + 1.0 { return Some(B::Smelter); }
    if x > 2.5 && x < 4.5 && z < -2.0 && z > -3.5 && y < v + 0.8 {
        return Some(if x > 3.5 { B::IronOre } else { B::CoalOre });
    }
    if z > 2.5 && x < -1.5 && x > -3.0 && y < v + 1.0 { return Some(B::SalvageCrate); }
    // Supported lamps mark the two work areas without filling the hall with glow.
    if (x.abs()-4.0).abs() < v * 0.5 && (z+2.5).abs() < v * 0.5 {
        if y < 2.5 { return Some(B::StoneBricks); }
        if y < 2.5 + v { return Some(B::Torch); }
    }
    Some(B::Air)
}

pub(super) fn yard(x: f32, y: f32, z: f32, v: f32) -> Option<B> {
    yard_versioned(x, y, z, v, false)
}

/// `connected` thickens each tile row by the rise across one cell, so the
/// stepped treads of the store roof share faces at every voxel scale.
pub(super) fn yard_versioned(x: f32, y: f32, z: f32, v: f32, connected: bool) -> Option<B> {
    let t = v.max(0.3);
    let roof = 3.4 + (z + 4.0) * 0.18;
    if y < -v || y > roof + t { return None; }
    if y < v { return Some(B::Cobblestone); }
    let under = roof - if connected { 0.18 * v } else { 0.0 };
    if y >= under && y < roof + t { return Some(B::RoofTiles); }
    if x.abs() > 3.0-t && z.abs() > 4.0-t && y < roof { return Some(B::Log); }
    if z > 2.5 && y < v + 1.0 {
        return Some(if x < -0.5 { B::CoalOre } else if x > 0.5 { B::CopperOre } else { B::SalvageCrate });
    }
    // Quench trough: ordinary contained water, a solid bottom, open working side.
    if x > 0.5 && z < -1.0 && z > -3.5 && y < v + 1.0 {
        let rim = x > 2.5 || x < 0.5 + t || z < -3.5+t || z > -1.0-t;
        return Some(if rim { B::StoneBricks } else { B::Water });
    }
    if y < roof { Some(B::Air) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workshop_has_clear_entry_and_service_routes() {
        for v in [1.0, 0.5, 0.25] {
            for z in -6..=3 {
                for y in [v + 0.1, v + 1.0, v + 1.7] {
                    assert_eq!(sample(0.0,y,z as f32,v),Some(B::Air),"v={v} y={y} z={z}");
                }
            }
            // The end service road begins six metres from the workshop centre.
            assert_eq!(sample(6.1,1.8,0.0,v),None);
            assert_eq!(sample(0.0,0.0,0.0,v),Some(B::Cobblestone));
            assert_eq!(sample(0.0,-2.0,-5.5,v),Some(B::StoneBricks));
            assert_eq!(sample(5.5,-2.0,0.0,v),None);
            assert_eq!(yard(0.0,-4.0,0.0,v),None);
            assert_eq!(yard(2.0,v+0.2,-2.2,v),Some(B::Water));
            assert_eq!(yard(2.8,v+0.2,-2.2,v),Some(B::StoneBricks));
        }
    }
}

