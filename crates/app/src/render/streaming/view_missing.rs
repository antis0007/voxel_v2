//! Benchmark evidence for streaming order: how many chunks the camera can see
//! have no resident mesh yet.

use glam::{Mat4, Vec3};

use super::ChunkCache;
use super::visibility::{chunk_bounds_sphere, chunk_radial_distance_sq, extract_planes};

impl ChunkCache {
    /// Visible chunks inside the frustum (far plane ignored, as in
    /// `build_draw_list`) with no resident GPU mesh: `(all, within 8 chunks)`.
    /// One pass over `visible`; the window calls it only while benchmarking.
    pub fn missing_in_view(&self, view_proj: Mat4) -> (usize, usize) {
        const NEAR_CHUNKS: i32 = 8;
        let planes = extract_planes(view_proj);
        let (mut all, mut near) = (0, 0);
        for key in &self.visible {
            if self.gpu_meshes.contains_key(&key.coord) {
                continue;
            }
            let (center, radius): (Vec3, f32) = chunk_bounds_sphere(key.coord);
            if planes[..5].iter().any(|plane| plane.truncate().dot(center) + plane.w < -radius) {
                continue;
            }
            all += 1;
            if chunk_radial_distance_sq(self.center_chunk, key.coord) <= NEAR_CHUNKS * NEAR_CHUNKS {
                near += 1;
            }
        }
        (all, near)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::streaming::{ChunkGpuMesh, ChunkKey};
    use crate::world::CHUNK_SIZE;
    use glam::IVec3;

    fn key(x: i32, y: i32, z: i32) -> ChunkKey {
        ChunkKey { coord: IVec3::new(x, y, z), lod: 0 }
    }

    #[test]
    fn missing_in_view_counts_frustum_chunks_without_mesh() {
        let mut cache = ChunkCache::new(IVec3::new(-2, 0, -2));
        cache.visible = vec![key(-2, 0, -6), key(-2, 0, 2), key(-2, 0, -40), key(-2, 0, -5)];
        cache.gpu_meshes.insert(IVec3::new(-2, 0, -5), ChunkGpuMesh::test_mesh(6, 0, true, 64));
        // Camera in chunk (-2, 0, -2) looking down -Z: (-2,0,2) is behind it.
        let eye = Vec3::new(-1.5, 0.5, -1.5) * CHUNK_SIZE as f32;
        let view = glam::camera::rh::view::look_at_mat4(eye, eye - Vec3::Z, Vec3::Y);
        let proj = glam::camera::rh::proj::directx::perspective(1.2, 1.0, 0.05, 1.0e6);
        assert_eq!(cache.missing_in_view(proj * view), (2, 1));
    }
}
