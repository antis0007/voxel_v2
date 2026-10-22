//! Generator 25: an open combat nave, with cover at its edge and a low offering dais.
//! Pure sampling preserves old generator versions and all ordinary player edits.
use super::BlockId as B;
use glam::Vec3;

pub const FIRST_VERSION: u32 = 25;
pub fn block(p: Vec3, voxel: f32) -> Option<B> {
    let radius = p.x.hypot(p.z);
    if radius > 18.5 || p.y < -1.5 || p.y > 12.0 {
        return None;
    }
    let arch_height = 10.5 - (radius / 18.0).powi(2) * 2.0;
    // A readable central circuit, rather than a room-filling crystal slab.
    if p.y < 0.0 {
        return Some(
            if (radius - 11.5).abs() < voxel * 0.55 || (p.x.abs() < voxel * 0.55 && radius > 2.0) {
                B::Limestone
            } else if radius < 17.3 {
                B::StoneBricks
            } else {
                B::Basalt
            },
        );
    }
    let entrance = p.x.abs() < 2.8 && p.z > 13.0 && p.y < 5.0;
    if radius > 17.0 && p.y < arch_height && !entrance {
        let pilaster = ((p.z.abs() - 8.0).abs() < 1.0 || p.x.abs() < 1.0) && radius > 17.7;
        return Some(if pilaster { B::Limestone } else { B::Basalt });
    }
    if p.y >= arch_height && !entrance {
        // A small luminous oculus and sparse ribs focus the composition.
        if radius < 2.3 {
            return Some(if p.y > arch_height + voxel {
                B::Crystal
            } else {
                B::Air
            });
        }
        let rib = p.x.abs() < voxel || p.z.abs() < voxel || (p.x.abs() - p.z.abs()).abs() < voxel;
        return Some(if rib { B::Limestone } else { B::StoneBricks });
    }
    // Four buttresses supply genuine projectile cover, outside the charge circuit.
    for (x, z) in [(-10.5, -9.0), (10.5, -9.0), (-10.5, 9.0), (10.5, 9.0)] {
        if (p.x - x).abs() < 0.85 && (p.z - z).abs() < 0.85 && p.y < 6.0 {
            return Some(if p.y > 5.0 { B::Crystal } else { B::Limestone });
        }
        if (p.x - x).abs() < 1.5 && (p.z - z).abs() < 1.5 && p.y < voxel {
            return Some(B::Limestone);
        }
    }
    for (x, z) in [
        (-8.5_f32, -9.0),
        (8.5, -9.0),
        (-8.5, 9.0),
        (8.5, 9.0),
        (0.0, -14.0),
        (-14.0, 0.0),
        (14.0, 0.0),
    ] {
        if (p.x - x).abs() < 0.55 && (p.z - z).abs() < 0.55 {
            if p.y < 1.2 {
                return Some(B::StoneBricks);
            }
            if p.y < 1.2 + voxel {
                return Some(B::Torch);
            }
        }
    }
    // The rear shrine is small enough that the player can always see the stag's legs.
    if (p.x * p.x + (p.z + 12.5).powi(2)).sqrt() < 1.8 && p.y < voxel {
        return Some(B::Limestone);
    }
    // Carefully spaced peripheral geodes, away from dodge lanes and the reliquary.
    for (x, z, h) in [
        (-14.5, -3.0, 3.2),
        (14.5, 3.0, 2.8),
        (-7.5, -14.0, 2.2),
        (7.5, -14.0, 2.7),
    ] {
        let taper = (1.0 - p.y / h).max(0.0);
        if p.y < h && (p.x - x).abs() + (p.z - z).abs() < taper * 1.4 {
            return Some(B::Crystal);
        }
    }
    Some(B::Air)
}
