use super::{hash, leaf, part, post, unit, Part, DARK_LEAF, LEAF, WOOD};
use glam::{IVec3, Vec3};

fn limb(out: &mut Vec<Part>, a: Vec3, b: Vec3, width: f32) {
    leaf(out, a, b, width, 0.0, WOOD);
}

fn blade(out: &mut Vec<Part>, root: Vec3, direction: Vec3, shade: f32) {
    let side = Vec3::Y.cross(direction).normalize_or(Vec3::X);
    let tip = root + direction;
    let node = root + direction * 0.30;
    let color = LEAF.map(|c| c * shade);
    leaf(out, root, tip, 0.17, 0.025, color);
    for sign in [-1.0, 1.0] {
        leaf(out, node, root + direction * 0.70 + side * (0.095 * sign),
            0.095, 0.016, color.map(|c| c * 0.93));
    }
}

fn bunch(out: &mut Vec<Part>, p: IVec3, node: Vec3, salt: u32) {
    let h = hash(p, salt);
    let top = node - Vec3::Y * (0.065 + unit(h, 8) * 0.025);
    limb(out, node, top - Vec3::Y * 0.025, 0.016);
    // Staggered front and back berries form a bunch with an exposed tapered tip.
    for (i, (x, y, z)) in [(-0.065, 0.0, 0.0), (0.014, -0.005, 0.015),
        (0.073, -0.032, -0.035), (-0.025, -0.075, 0.030),
        (0.048, -0.095, 0.020), (0.003, -0.145, 0.020),
        (-0.026, -0.040, -0.060), (0.019, -0.106, -0.052)].into_iter().enumerate() {
        let pick = hash(p, salt + 1 + i as u32);
        let r = 0.031 + unit(pick, 8) * 0.008;
        let centre = top + Vec3::new(x, y, z);
        let pigment = 0.83 + unit(pick, 16) * 0.32;
        let color = [0.30, 0.16, 0.39].map(|c| c * pigment);
        out.push(part((centre - Vec3::splat(r)).to_array(),
            (centre + Vec3::splat(r)).to_array(), Some(color)));
    }
}

pub(super) fn build(out: &mut Vec<Part>, p: IVec3, ripe: bool) {
    let h = hash(p, 0x67a9);
    let root = Vec3::new(0.45 + unit(h, 0) * 0.10, 0.0, 0.47 + unit(h, 8) * 0.06);
    let shoulder = root + Vec3::new((unit(h, 16) - 0.5) * 0.08, 0.60, 0.0);
    out.push(post(root.x, root.z, 0.052, 0.0, 0.24, Some(WOOD)));
    limb(out, root + Vec3::Y * 0.20, shoulder, 0.044);
    let angle = (unit(h, 24) - 0.5) * 0.70;
    let axis = Vec3::new(angle.cos(), 0.0, angle.sin());
    for (i, sign) in [-1.0, 1.0].into_iter().enumerate() {
        let pick = hash(p, 0x68a0 + i as u32);
        let arm = axis * sign;
        let end = shoulder + arm * (0.28 + unit(pick, 8) * 0.045)
            + Vec3::Y * (0.12 + unit(pick, 16) * 0.10);
        limb(out, shoulder - Vec3::Y * 0.04, end, 0.027);
        for (along, side) in [(0.20, -1.0), (0.58, 1.0), (0.92, -1.0)] {
            let node = shoulder.lerp(end, along);
            let cross = Vec3::Y.cross(arm);
            let direction = arm * 0.075 + cross * (side * 0.14)
                + Vec3::Y * (0.09 + 0.035 * unit(pick, 24));
            blade(out, node, direction, 0.82 + 0.16 * along + 0.10 * unit(pick, 0));
        }
        if ripe {
            let node = shoulder.lerp(end, 0.64) + Vec3::Y.cross(arm) * (sign * 0.060);
            bunch(out, p, node, 0x6900 + i as u32 * 16);
        }
        let tendril = end + arm * 0.035 - Vec3::Y * 0.065;
        leaf(out, end, tendril, 0.012, 0.0, DARK_LEAF);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockId as B;

    #[test]
    fn grapes_preserve_bare_vines_and_have_small_separate_fruit() {
        for x in -8..8 {
            let p = IVec3::new(x, 100, -3);
            let bare = crate::flora::parts(B::TrellisVine, p).unwrap();
            let ripe = crate::flora::parts(B::Grapes, p).unwrap();
            assert!(bare.iter().all(|part| ripe.contains(part)));
            let fruit: Vec<_> = ripe.iter().filter(|part| {
                part.leaf.is_none() && part.color.is_some_and(|c| c[2] > c[0])
            }).collect();
            assert_eq!(fruit.len(), 16);
            assert!(fruit.iter().all(|part| (part.hi - part.lo).max_element() < 0.08));
            let triangles: usize = ripe.iter().map(|part| if part.leaf.is_some() { 8 } else { 12 }).sum();
            assert!(triangles <= 420, "{triangles}");
            assert!(ripe.iter().all(|part| part.leaf.is_some() ||
                (part.hi - part.lo).min_element() < 0.08));
        }
    }

    #[test]
    fn cell_hash_changes_vines_but_repeats_exactly() {
        let a = crate::flora::parts(B::Grapes, IVec3::ZERO);
        assert_eq!(a, crate::flora::parts(B::Grapes, IVec3::ZERO));
        assert_ne!(a, crate::flora::parts(B::Grapes, IVec3::X));
    }
}
