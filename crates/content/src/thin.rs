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
    is_climbable(b) || matches!(b, B::Sapling | B::SnowLayer | B::Bamboo | B::BambooShoot | B::Sugarcane | B::SugarcaneShoot) || is_crop(b) || crate::flora::is_flora(b) || crate::shaped::is_shaped(b)
}

/// Planted grain and root crops and their shared sprout.
pub fn is_crop(b: B) -> bool {
    matches!(b, B::WheatCrop | B::CarrotCrop | B::BeetCrop | B::CropShoot)
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

/// World-voxel boxes for a thin block at `p` at 1 m voxels.
pub fn boxes(b: B, p: IVec3) -> Vec<(Vec3, Vec3)> {
    boxes_scaled(b, p, 1.0)
}

/// World-voxel boxes for a thin block at `p`; `metres` is the voxel size.
pub fn boxes_scaled(b: B, p: IVec3, metres: f32) -> Vec<(Vec3, Vec3)> {
    if let Some(parts) = crate::flora::parts(b, p) {
        let base = p.as_vec3();
        return parts.into_iter().map(|part| (base + part.lo, base + part.hi)).collect();
    }
    let authored: Vec<(Vec3, Vec3)> = if b == B::Bamboo {
        bamboo_parts(p, metres)
    } else if b == B::BambooShoot {
        vec![
            (Vec3::new(0.42, 0.0, 0.42), Vec3::new(0.58, 0.35, 0.58)),
            (Vec3::new(0.46, 0.35, 0.46), Vec3::new(0.54, 0.6, 0.54)),
        ]
    } else if matches!(b, B::Sugarcane | B::SugarcaneShoot) {
        let height = if b == B::Sugarcane { 1.0 } else { 0.38 };
        vec![
            (Vec3::new(0.44, 0.0, 0.44), Vec3::new(0.56, height, 0.56)),
            (Vec3::new(0.17, height * 0.48, 0.47), Vec3::new(0.46, height * 0.64, 0.54)),
            (Vec3::new(0.54, height * 0.72, 0.46), Vec3::new(0.84, height * 0.86, 0.53)),
        ]
    } else if is_crop(b) {
        crop_parts(b)
    } else if LADDERS.contains(&b) {
        let mut parts = vec![
            (Vec3::new(0.12, 0.0, 0.875), Vec3::new(0.24, 1.0, 1.0)),
            (Vec3::new(0.76, 0.0, 0.875), Vec3::new(0.88, 1.0, 1.0)),
        ];
        for y in [0.12, 0.37, 0.62, 0.87] {
            parts.push((Vec3::new(0.24, y, 0.89), Vec3::new(0.76, y + 0.07, 0.97)));
        }
        parts
    } else if VINES.contains(&b) {
        vine_parts(p)
    } else if b == B::Sapling {
        vec![
            (Vec3::new(0.45, 0.0, 0.45), Vec3::new(0.55, 0.5, 0.55)),
            (Vec3::new(0.25, 0.45, 0.44), Vec3::new(0.75, 0.85, 0.56)),
            (Vec3::new(0.44, 0.45, 0.25), Vec3::new(0.56, 0.85, 0.75)),
        ]
    } else if b == B::SnowLayer {
        vec![(Vec3::ZERO, Vec3::new(1.0, SNOW_DEPTH, 1.0))]
    } else {
        // Fences report every rail so callers can bound their box count.
        return crate::shaped::boxes(b, p, 0b1111);
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

/// Boxes inside the cell for a crop: stalks and ears, leaf tufts, or the shoot.
fn crop_parts(b: B) -> Vec<(Vec3, Vec3)> {
    let stalk = |x: f32, z: f32, h: f32| (Vec3::new(x, 0.0, z), Vec3::new(x + 0.07, h, z + 0.07));
    match b {
        B::WheatCrop => {
            let mut parts = Vec::with_capacity(8);
            for (x, z, h) in [(0.18, 0.2, 0.86), (0.62, 0.16, 0.92), (0.38, 0.55, 0.8), (0.74, 0.66, 0.88)] {
                parts.push(stalk(x, z, h));
                parts.push((Vec3::new(x - 0.02, h - 0.2, z - 0.02), Vec3::new(x + 0.09, h, z + 0.09)));
            }
            parts
        }
        B::CarrotCrop => vec![
            (Vec3::new(0.2, 0.0, 0.2), Vec3::new(0.8, 0.28, 0.8)),
            (Vec3::new(0.3, 0.28, 0.3), Vec3::new(0.7, 0.5, 0.7)),
            (Vec3::new(0.4, 0.5, 0.4), Vec3::new(0.6, 0.62, 0.6)),
        ],
        B::BeetCrop => vec![
            (Vec3::new(0.3, 0.0, 0.3), Vec3::new(0.7, 0.18, 0.7)),
            (Vec3::new(0.15, 0.18, 0.4), Vec3::new(0.85, 0.46, 0.6)),
            (Vec3::new(0.4, 0.18, 0.15), Vec3::new(0.6, 0.46, 0.85)),
        ],
        _ => vec![
            (Vec3::new(0.44, 0.0, 0.44), Vec3::new(0.56, 0.22, 0.56)),
            (Vec3::new(0.3, 0.2, 0.46), Vec3::new(0.7, 0.3, 0.54)),
        ],
    }
}

fn cell_hash(p: IVec3, salt: u32) -> u32 {
    engine_core::hash::hash_u32(
        (p.x as u32).wrapping_mul(0x9e37_79b9)
            ^ (p.y as u32).wrapping_mul(0x85eb_ca6b)
            ^ (p.z as u32).wrapping_mul(0xc2b2_ae35)
            ^ salt.wrapping_mul(0x27d4_eb2f),
    )
}

/// Vine strands and leaf tufts authored against a +Z wall.
// Strand x and the width at cell edges depend only on the column, so a
// stack of vine cells joins into continuous strands instead of combs.
fn vine_parts(p: IVec3) -> Vec<(Vec3, Vec3)> {
    let column = IVec3::new(p.x, 0, p.z);
    let unit = |h: u32, bits: u32| (h >> bits & 0xFF) as f32 / 255.0;
    let mut parts = Vec::with_capacity(20);
    for i in 0..4u32 {
        let c = cell_hash(column, i);
        let x = 0.06 + i as f32 * 0.235 + unit(c, 0) * 0.07;
        let w = 0.07 + unit(c, 8) * 0.04;
        let depth = 0.95 - unit(c, 16) * 0.02;
        let h = cell_hash(p, i);
        // One strand in four ends here; its tip is thinner.
        let tip = h % 4 == 0;
        let bottom = if tip { 0.25 + unit(h, 8) * 0.5 } else { 0.0 };
        let wobble = (unit(h, 16) - 0.5) * 0.08;
        let segments: [(f32, f32, f32); 3] = [(0.0, 0.33, 0.0), (0.33, 0.67, wobble), (0.67, 1.0, 0.0)];
        for (lo, hi, dx) in segments {
            if hi <= bottom {
                continue;
            }
            let lo = lo.max(bottom);
            // The top segment keeps full width so the strand above still meets it.
            let narrow = if tip && lo == bottom && hi < 1.0 { 0.6 } else { 1.0 };
            let x0 = (x + dx).clamp(0.0, 1.0 - w * narrow);
            parts.push((Vec3::new(x0, lo, depth), Vec3::new(x0 + w * narrow, hi, 1.0)));
        }
        // Leaf tufts stand off the strand so the vine has volume.
        for k in 0..2 {
            let l = cell_hash(p, 16 + i * 2 + k);
            if l % 3 == 0 {
                continue;
            }
            let y = bottom + unit(l, 0) * (0.9 - bottom).max(0.05);
            let half = 0.06 + unit(l, 8) * 0.05;
            let side = if l & 0x1_0000 == 0 { -half * 0.6 } else { half * 0.6 };
            let cx = (x + w * 0.5 + side).clamp(half, 1.0 - half);
            let y1 = (y + half * 1.4).min(1.0);
            parts.push((Vec3::new(cx - half, y, 0.88), Vec3::new(cx + half, y1, 0.97)));
        }
    }
    parts
}

/// Real culm diameter and node spacing, metres.
const CULM_METRES: f32 = 0.12;
const NODE_METRES: f32 = 0.4;

/// Bamboo cell: a cane cluster at coarse scales, one noded culm at fine ones.
fn bamboo_parts(p: IVec3, metres: f32) -> Vec<(Vec3, Vec3)> {
    let column = IVec3::new(p.x, 0, p.z);
    let unit = |h: u32, bits: u32| (h >> bits & 0xFF) as f32 / 255.0;
    let column_box = |cx: f32, cz: f32, w: f32, y0: f32, y1: f32| {
        let h = w * 0.5;
        (Vec3::new(cx - h, y0, cz - h), Vec3::new(cx + h, y1, cz + h))
    };
    let mut parts = Vec::new();
    if metres >= 0.75 {
        // Thinner than a cell: several canes share it, banded at mid height.
        for i in 0..3u32 {
            let c = cell_hash(column, 0xba00 + i);
            let cx = 0.2 + i as f32 * 0.3 + (unit(c, 0) - 0.5) * 0.12;
            let cz = 0.25 + unit(c, 8) * 0.5;
            parts.push(column_box(cx, cz, 0.1, 0.0, 1.0));
            let band = 0.4 + unit(c, 16) * 0.2;
            parts.push(column_box(cx, cz, 0.14, band, band + 0.06));
        }
        return parts;
    }
    let w = (CULM_METRES / metres).clamp(0.35, 1.0);
    parts.push(column_box(0.5, 0.5, w, 0.0, 1.0));
    let every = (NODE_METRES / metres).round().max(1.0) as i32;
    if p.y.rem_euclid(every) == 0 {
        let ring = (w + 0.12).min(1.0);
        parts.push(column_box(0.5, 0.5, ring, 0.0, 0.14));
        // Leaf sprigs branch off some nodes.
        let h = cell_hash(p, 0xba10);
        if h % 3 == 0 {
            let out = 0.5 + w * 0.5;
            let side = (h >> 8) % 4;
            let (lo, hi) = (Vec3::new(out, 0.1, 0.42), Vec3::new(1.0, 0.24, 0.58));
            let rot = [IVec3::Z, IVec3::NEG_Z, IVec3::X, IVec3::NEG_X][side as usize];
            if w < 0.9 {
                let (a, c) = (turn(lo, rot), turn(hi, rot));
                parts.push((a.min(c), a.max(c)));
            }
        }
    }
    parts
}

/// Rotates a point authored against a +Z wall onto the wall at `support`.
pub(crate) fn turn(v: Vec3, support: IVec3) -> Vec3 {
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

    #[test]
    fn bamboo_fits_its_cell_and_thickens_at_fine_scales() {
        let mut widths = Vec::new();
        for metres in [2.0, 1.0, 0.5, 0.25, 0.125, 0.0625] {
            for y in -9..9 {
                let p = IVec3::new(-3, y, 1 << 20);
                let parts = boxes_scaled(B::Bamboo, p, metres);
                assert!(!parts.is_empty());
                for (lo, hi) in &parts {
                    let (lo, hi) = (*lo - p.as_vec3(), *hi - p.as_vec3());
                    assert!(lo.cmpge(Vec3::splat(-1e-5)).all() && hi.cmple(Vec3::splat(1.0 + 1e-5)).all(), "{metres} m {lo:?}..{hi:?}");
                }
            }
            let (lo, hi) = boxes_scaled(B::Bamboo, IVec3::ZERO, metres)[0];
            widths.push(hi.x - lo.x);
        }
        // Past the cane cluster, one culm covers more of its cell as cells shrink.
        assert!(widths[2..].windows(2).all(|w| w[0] <= w[1]), "{widths:?}");
    }

    #[test]
    fn stacked_vines_join_into_strands() {
        // A strand that leaves the bottom of a cell must meet a strand at the
        // top of the cell below at the same x, or a vine column reads as combs.
        for (x, z) in [(0, 0), (-7, 13), (1 << 20, -(1 << 20))] {
            for y in -40..40 {
                let high = vine_parts(IVec3::new(x, y + 1, z));
                let low = vine_parts(IVec3::new(x, y, z));
                for (lo, hi) in high.iter().filter(|(lo, hi)| lo.y == 0.0 && hi.z == 1.0) {
                    let joined = low.iter().any(|(l2, h2)| {
                        h2.y == 1.0 && h2.z == 1.0 && (l2.x - lo.x).abs() < 1e-5 && (h2.x - hi.x).abs() < 1e-5
                    });
                    assert!(joined, "strand at {x},{},{z} x {} has no strand below", y + 1, lo.x);
                }
            }
        }
        assert!(!is_climbable(B::Sapling) && is_thin(B::Sapling));
    }
}
