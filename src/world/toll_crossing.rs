//! Version 26 crossing. The gorge has a recovery floor and a second, lower route.
use super::BlockId as B;
use glam::Vec3;

pub fn block(p: Vec3, voxel: f32) -> Option<B> {
    if p.x.abs() > 18.0 || p.z.abs() > 16.0 || p.y < -6.0 || p.y > 7.0 {
        return None;
    }
    let bank = p.z.abs() >= 9.0;
    if p.y < -4.0 {
        return Some(B::Cobblestone);
    }
    if bank && p.y < 0.0 {
        return Some(B::StoneBricks);
    }
    // Broad upper bridge, with real piers. Nothing seals the lateral building space.
    if p.x.abs() < 4.5 && (-1.0..0.0).contains(&p.y) {
        return Some(B::StoneBricks);
    }
    if (p.x.abs() - 3.5).abs() < 0.7 && (p.z.abs() - 6.0).abs() < 0.8 && p.y < 0.0 {
        return Some(B::Cobblestone);
    }
    // Walkable maintenance ledge and stairs from both banks (half-metre rises).
    if (6.0..9.0).contains(&p.x) {
        let stair = (((p.z.abs() - 2.0).max(0.0) * 0.5) / voxel.max(0.5)).floor() * voxel.max(0.5);
        let top = (-3.0 + stair).min(0.0);
        if p.y < top && p.y >= top - 1.0 {
            return Some(B::Planks);
        }
    }
    // A ruined tollhouse on the far bank gives both fighters useful masonry cover.
    if (p.z + 12.0).abs() < 2.8 && p.x.abs() < 7.0 {
        let wall = p.x.abs() > 6.0 || (p.z + 12.0).abs() > 2.0;
        let door = p.x.abs() < 2.0 && p.z > -12.0;
        if wall && !door && p.y < 3.5 {
            return Some(B::StoneBricks);
        }
        if p.x.abs() > 3.0 && p.y > 3.5 && p.y < 4.5 {
            return Some(B::Planks);
        }
    }
    Some(B::Air)
}
