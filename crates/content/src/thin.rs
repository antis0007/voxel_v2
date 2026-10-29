//! Thin cutout blocks drawn as boxes inside their cell: ladders, vines, saplings.
use crate::BlockId as B;
use glam::{IVec3, Vec3};

const LADDERS: [B; 4] = [B::Ladder, B::LadderSouth, B::LadderWest, B::LadderEast];
const VINES: [B; 4] = [B::Vines, B::VinesSouth, B::VinesWest, B::VinesEast];
/// Supporting wall per orientation index, matching the wall torches.
const SUPPORT: [IVec3; 4] = [IVec3::Z, IVec3::NEG_Z, IVec3::X, IVec3::NEG_X];
/// Snow layer height as a share of its cell.
const SNOW_DEPTH: f32 = 0.1875;

fn family(b: B) -> Option<&'static [B; 4]> {
    if LADDERS.contains(&b) {
        Some(&LADDERS)
    } else if VINES.contains(&b) {
        Some(&VINES)
    } else {
        None
    }
}

/// Ladders and vines: the body climbs while overlapping one.
pub fn is_climbable(b: B) -> bool {
    family(b).is_some()
}

pub fn is_thin(b: B) -> bool {
    is_climbable(b) || b == B::Sapling || b == B::SnowLayer
}

/// Offset from the cell to the wall a ladder or vine hangs on.
pub fn support_offset(b: B) -> Option<IVec3> {
    let family = family(b)?;
    family.iter().position(|&f| f == b).map(|i| SUPPORT[i])
}

/// The member of `b`'s family hung on the face whose outward normal is `normal`.
pub fn attach(b: B, normal: IVec3) -> Option<B> {
    let family = family(b)?;
    SUPPORT.iter().position(|&s| s == -normal).map(|i| family[i])
}

/// The item a ladder or vine of any orientation drops.
pub fn canonical(b: B) -> Option<B> {
    family(b).map(|f| f[0])
}

/// World-voxel boxes for a thin block at `p`; empty for other blocks.
pub fn boxes(b: B, p: IVec3) -> Vec<(Vec3, Vec3)> {
    let authored: Vec<(Vec3, Vec3)> = if LADDERS.contains(&b) {
        let mut parts = vec![
            (Vec3::new(0.12, 0.0, 0.875), Vec3::new(0.24, 1.0, 1.0)),
            (Vec3::new(0.76, 0.0, 0.875), Vec3::new(0.88, 1.0, 1.0)),
        ];
        for y in [0.12, 0.37, 0.62, 0.87] {
            parts.push((Vec3::new(0.24, y, 0.89), Vec3::new(0.76, y + 0.07, 0.97)));
        }
        parts
    } else if VINES.contains(&b) {
        // Strand lengths vary by cell so a wall of vines does not tile.
        let h = engine_core::hash::hash_u32(
            (p.x as u32).wrapping_mul(0x9e37_79b9)
                ^ (p.y as u32).wrapping_mul(0x85eb_ca6b)
                ^ (p.z as u32).wrapping_mul(0xc2b2_ae35),
        );
        (0..5)
            .map(|i| {
                let x = 0.04 + i as f32 * 0.19;
                let drop = ((h >> (i * 4)) & 0xF) as f32 / 15.0 * 0.6;
                (Vec3::new(x, drop, 0.94), Vec3::new(x + 0.15, 1.0, 1.0))
            })
            .collect()
    } else if b == B::Sapling {
        vec![
            (Vec3::new(0.45, 0.0, 0.45), Vec3::new(0.55, 0.5, 0.55)),
            (Vec3::new(0.25, 0.45, 0.44), Vec3::new(0.75, 0.85, 0.56)),
            (Vec3::new(0.44, 0.45, 0.25), Vec3::new(0.56, 0.85, 0.75)),
        ]
    } else if b == B::SnowLayer {
        vec![(Vec3::ZERO, Vec3::new(1.0, SNOW_DEPTH, 1.0))]
    } else {
        return Vec::new();
    };
    let support = support_offset(b).unwrap_or(IVec3::Z);
    let base = p.as_vec3();
    authored
        .into_iter()
        .map(|(lo, hi)| {
            let (a, c) = (turn(lo, support), turn(hi, support));
            (base + a.min(c), base + a.max(c))
        })
        .collect()
}

/// Rotates a point authored against a +Z wall onto the wall at `support`.
fn turn(v: Vec3, support: IVec3) -> Vec3 {
    match (support.x, support.z) {
        (0, -1) => Vec3::new(1.0 - v.x, v.y, 1.0 - v.z),
        (1, _) => Vec3::new(v.z, v.y, v.x),
        (-1, _) => Vec3::new(1.0 - v.z, v.y, v.x),
        _ => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_orientation_hangs_on_its_wall() {
        for b in LADDERS.iter().chain(VINES.iter()).copied() {
            let wall = support_offset(b).expect("support");
            assert_eq!(attach(b, -wall), Some(b));
            assert_eq!(canonical(b).and_then(|c| attach(c, -wall)), Some(b));
            let boxes = boxes(b, IVec3::ZERO);
            assert!(!boxes.is_empty());
            // Every box touches the supporting face and stays inside the cell.
            let face = (wall.as_vec3() * 0.5 + Vec3::splat(0.5)).dot(wall.as_vec3().abs());
            for (lo, hi) in boxes {
                assert!(lo.cmpge(Vec3::ZERO).all() && hi.cmple(Vec3::ONE).all(), "{b:?}");
                let axis = wall.as_vec3().abs();
                let near = if face > 0.5 { hi.dot(axis) } else { lo.dot(axis) };
                assert!((near - face).abs() < 0.04, "{b:?} box {lo:?}..{hi:?} is off the wall");
            }
        }
        assert_eq!(attach(B::Ladder, IVec3::Y), None);
        assert!(!is_climbable(B::Sapling) && is_thin(B::Sapling));
    }
}
