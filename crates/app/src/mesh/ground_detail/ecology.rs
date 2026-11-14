//! Column ecology for ground cover: field patches, tree root zones and canopy
//! shade. Pure functions of world position and generated trees, so chunks that
//! share a border agree.
use super::{GeneratedChunk, IVec3, hash};
use crate::mesh::TreeFootprint;

pub(super) const PATCH_METRES: f32 = 9.0;

pub(super) fn patch_field_metres(x: f32, z: f32) -> f32 {
    let (px, pz) = (x / PATCH_METRES, z / PATCH_METRES);
    let (gx, gz) = (px.floor() as i32, pz.floor() as i32);
    let (fx, fz) = (px - px.floor(), pz - pz.floor());
    let v = |i: i32, k: i32| (hash(IVec3::new(gx + i, 7919, gz + k)) & 65535) as f32 / 65535.0;
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let a = v(0, 0) + (v(1, 0) - v(0, 0)) * sx;
    let b = v(0, 1) + (v(1, 1) - v(0, 1)) * sx;
    a + (b - a) * sz
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Fields that steer what grows on one turf column, each in 0..=1.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Column {
    /// 9 m colonies: lush where high.
    pub meadow: f32,
    /// 23 m sward mosaic: tall grass pockets high, short open field low.
    pub sward: f32,
    /// ~3.9 m variation inside a colony.
    pub local: f32,
    /// Flower and clover colonies.
    pub colony: f32,
    /// Bare soil pockets of one to three metres.
    pub bare: f32,
    /// Straw-tipped dry patches.
    pub dry: f32,
    /// Canopy cover above this column.
    pub shade: f32,
    /// Share shaded by rainforest crowns.
    pub rainforest: f32,
    /// Trampled, root-heavy ring around a trunk.
    pub root: f32,
    /// Light-gap ring just outside a crown, where grass grows tallest.
    pub edge: f32,
}

impl Column {
    pub fn at(x: f32, z: f32, trees: &[TreeFootprint], ground_y: f32) -> Self {
        let mut c = Self {
            meadow: patch_field_metres(x, z),
            sward: patch_field_metres(x / 2.55 + 113.0, z / 2.55 - 71.0),
            local: patch_field_metres(x * 2.3 - 11.0, z * 2.3 + 7.0),
            colony: patch_field_metres(x + 37.0, z - 19.0),
            bare: smoothstep(0.76, 0.90, patch_field_metres(x * 1.7 + 59.0, z * 1.7 - 23.0)),
            dry: smoothstep(0.58, 0.80, patch_field_metres(x * 1.38 + 17.0, z * 1.38 + 5.0)),
            ..Self::default()
        };
        for t in trees {
            // A crown shades the floor it grows from, not a cliff top beside it.
            if ground_y > t.crown_low || ground_y < t.base_y - 3.0 {
                continue;
            }
            let d = ((x - t.x).powi(2) + (z - t.z).powi(2)).sqrt();
            let reach = t.reach.max(0.5);
            if d > reach * 1.35 {
                continue;
            }
            let n = d / reach;
            let shade = 1.0 - smoothstep(0.55, 1.02, n);
            c.shade = c.shade.max(shade);
            if t.rainforest {
                c.rainforest = c.rainforest.max(shade);
            }
            c.root = c.root.max(1.0 - smoothstep(0.5, 0.5 + reach.min(6.0) * 0.32, d));
            c.edge = c.edge.max(1.0 - ((n - 1.08) / 0.22).abs().min(1.0));
        }
        c.dry *= 1.0 - c.shade;
        c
    }
}

/// Trees whose crowns touch this chunk's columns.
pub(super) fn local_trees(chunk: &GeneratedChunk) -> Vec<TreeFootprint> {
    let s = chunk.voxel_meters;
    let lo = (chunk.chunk * 32).as_vec3() * s;
    let hi = lo + glam::Vec3::splat(32.0 * s);
    chunk
        .trees
        .iter()
        .copied()
        .filter(|t| {
            let r = t.reach * 1.35;
            t.x + r >= lo.x && t.x - r <= hi.x && t.z + r >= lo.z && t.z - r <= hi.z
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(x: f32, z: f32) -> TreeFootprint {
        TreeFootprint { x, z, base_y: 10.0, reach: 5.0, crown_low: 14.0, top: 20.0, rainforest: false }
    }

    #[test]
    fn trunks_make_a_root_zone_under_shade_and_a_lush_edge_outside_the_crown() {
        let t = [tree(0.0, 0.0)];
        let trunk = Column::at(0.6, 0.0, &t, 10.0);
        let crown = Column::at(3.0, 0.0, &t, 10.0);
        let edge = Column::at(5.4, 0.0, &t, 10.0);
        let open = Column::at(20.0, 0.0, &t, 10.0);
        assert!(trunk.root > 0.9 && trunk.shade > 0.95);
        assert!(crown.shade > 0.5 && crown.root < trunk.root);
        assert!(edge.edge > 0.6 && edge.shade < 0.2);
        assert!(open.shade == 0.0 && open.root == 0.0 && open.edge == 0.0);
    }

    #[test]
    fn a_crown_ignores_ground_on_another_terrace() {
        let t = [tree(0.0, 0.0)];
        assert_eq!(Column::at(2.0, 0.0, &t, 16.0).shade, 0.0);
        assert_eq!(Column::at(2.0, 0.0, &t, 4.0).shade, 0.0);
    }

    #[test]
    fn fields_are_bounded_and_continuous_across_sign_boundaries() {
        for x in -70..70 {
            for z in [-33, -32, -1, 0, 31, 32] {
                let a = Column::at(x as f32, z as f32, &[], 0.0);
                let b = Column::at(x as f32 + 0.25, z as f32, &[], 0.0);
                for (u, v) in [(a.meadow, b.meadow), (a.sward, b.sward), (a.local, b.local)] {
                    assert!((0.0..=1.0).contains(&u));
                    assert!((u - v).abs() <= 0.2);
                }
            }
        }
    }
}
