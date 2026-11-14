//! Ambient occlusion from generated tree crowns. Sky access ignores leaves (a
//! leaf roof would read as a cave), so without this a crown's underside and
//! the turf beneath it took full sky light and the tree sat pasted on the field.
use super::{GeneratedChunk, TreeFootprint, Vertex3};
use glam::Vec3;

/// Leaf path, in metres, that halves ambient light.
const HALF_PATH_M: f32 = 3.0;
/// Voxel crowns are porous: even a dense centre keeps this much sky.
const FLOOR: f32 = 0.30;

pub struct Canopy {
    trees: Vec<TreeFootprint>,
    scale: f32,
}

impl Canopy {
    pub fn new(chunk: &GeneratedChunk) -> Self {
        let s = chunk.voxel_meters;
        let lo = chunk.origin().as_vec3() * s;
        let hi = lo + Vec3::splat(super::PADDED_CHUNK_SIZE as f32 * s);
        let trees = chunk
            .trees
            .iter()
            .copied()
            .filter(|t| {
                t.x + t.reach >= lo.x
                    && t.x - t.reach <= hi.x
                    && t.z + t.reach >= lo.z
                    && t.z - t.reach <= hi.z
                    && t.top >= lo.y
                    && t.base_y <= hi.y
            })
            .collect();
        Self { trees, scale: s }
    }

    /// Ambient transmission at a voxel-space point, 1 in the open.
    pub fn visibility(&self, pos: Vec3) -> f32 {
        let q = pos * self.scale;
        let mut path = 0.0;
        for t in &self.trees {
            let (dx, dz) = (q.x - t.x, q.z - t.z);
            let u = (dx * dx + dz * dz) / (t.reach * t.reach).max(0.25);
            if u >= 1.0 {
                continue;
            }
            // Crown as an ellipsoid from the fork to the top; the leaf depth
            // above the point is its vertical chord above it.
            let ry = (t.top - t.crown_low) * 0.5;
            let half = ry * (1.0 - u).sqrt();
            let upper = t.crown_low + ry + half;
            path += (upper - q.y.max(upper - 2.0 * half)).clamp(0.0, 2.0 * half);
        }
        if path <= 0.0 {
            return 1.0;
        }
        FLOOR + (1.0 - FLOOR) * (-path * std::f32::consts::LN_2 / HALF_PATH_M).exp()
    }

    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }

    /// Scale a vertex's encoded AO, leaving the flag encodings outside 0..=1
    /// (water rims, backdrop and orbital tags) untouched.
    pub fn apply(&self, vertex: &mut Vertex3) {
        let encoded = vertex.ao_strength;
        if !(0.0..=1.0).contains(&encoded) {
            return;
        }
        let n = Vec3::from_array(vertex.normal);
        vertex.ao_strength = encoded * self.visibility(Vec3::from_array(vertex.pos) + n * 0.3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    fn canopy() -> Canopy {
        let mut chunk = GeneratedChunk::new(IVec3::ZERO, Vec::new());
        chunk.trees = vec![TreeFootprint { x: 16.0, z: 16.0, base_y: 4.0, reach: 5.0, crown_low: 9.0, top: 17.0, rainforest: false }];
        Canopy::new(&chunk)
    }

    #[test]
    fn shade_deepens_under_the_crown_and_vanishes_outside_and_above() {
        let c = canopy();
        let ground = c.visibility(Vec3::new(16.0, 4.0, 16.0));
        let rim = c.visibility(Vec3::new(20.5, 4.0, 16.0));
        let top = c.visibility(Vec3::new(16.0, 17.5, 16.0));
        let middle = c.visibility(Vec3::new(16.0, 13.0, 16.0));
        assert!((FLOOR..0.5).contains(&ground), "{ground}");
        assert!(rim > ground && rim < 1.0);
        assert_eq!(top, 1.0);
        assert!(middle > ground && middle < 1.0);
        assert_eq!(c.visibility(Vec3::new(30.0, 4.0, 16.0)), 1.0);
    }

    #[test]
    fn flag_encoded_ao_is_left_alone() {
        let c = canopy();
        for flag in [1.5, -0.5, -2.3, 4.2] {
            let mut v = Vertex3 { pos: [16.0, 4.0, 16.0], normal: [0.0, 1.0, 0.0], color: [0.0; 3], surface: [0.0; 4], ao_strength: flag };
            c.apply(&mut v);
            assert_eq!(v.ao_strength, flag);
        }
    }
}
