//! Cosmetic voxel erosion in the particle's own frame; no world-grid discard.
use super::*;

impl Model {
    pub fn eroding_particle(
        &mut self, center: Vec3, size: f32, rotation: Quat, color: [f32; 3],
        glow: f32, alpha: f32, erosion: f32, seed: u32, detail: f32,
    ) {
        if erosion >= 1.0 || size <= 0.0 || alpha <= 0.0 { return; }
        if erosion <= 0.0 {
            self.glow_voxel(center, Vec3::splat(size), rotation, color, glow, alpha);
            return;
        }
        if detail < 0.5 {
            let coarse_alpha = alpha * (1.0 - erosion);
            if coarse_alpha >= 0.06 {
                self.glow_voxel(center, Vec3::splat(size), rotation, color, glow, coarse_alpha);
            }
            return;
        }
        if alpha < 0.06 { return; }
        // A permutation gives every cell a distinct death threshold. The same
        // seed and normalized lifetime always produce the same volume.
        let mut ranks = [0u32, 1, 2, 3, 4, 5, 6, 7];
        let mut random = seed;
        for i in (1..8).rev() {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            ranks.swap(i, ((random >> 16) as usize) % (i + 1));
        }
        let present = ranks.map(|rank| erosion < (rank + 1) as f32 / 8.0);
        let old_alpha = self.alpha;
        let old_dissolve = self.dissolve;
        self.alpha = alpha;
        let faces = [
            (IVec3::X, Vec3::Y, Vec3::Z), (IVec3::NEG_X, Vec3::Z, Vec3::Y),
            (IVec3::Y, Vec3::Z, Vec3::X), (IVec3::NEG_Y, Vec3::X, Vec3::Z),
            (IVec3::Z, Vec3::X, Vec3::Y), (IVec3::NEG_Z, Vec3::Y, Vec3::X),
        ];
        for cell in 0..8 {
            if !present[cell] { continue; }
            let grid = IVec3::new((cell & 1) as i32, ((cell >> 1) & 1) as i32, ((cell >> 2) & 1) as i32);
            let local_center = (grid.as_vec3() - Vec3::splat(0.5)) * (size * 0.5);
            let until_removal = (ranks[cell] + 1) as f32 / 8.0 - erosion;
            // The shader brightens only cells at the erosion front.
            self.dissolve = (1.0 - until_removal * 8.0).clamp(0.0, 1.0);
            for (normal, u, v) in faces {
                let neighbor = grid + normal;
                if neighbor.cmpge(IVec3::ZERO).all() && neighbor.cmple(IVec3::ONE).all() {
                    let index = (neighbor.x + neighbor.y * 2 + neighbor.z * 4) as usize;
                    if present[index] { continue; }
                }
                // Publish actual cut faces. Empty cells reveal depth and do
                // not leave hollow billboard sheets or invisible solid faces.
                let face = local_center + normal.as_vec3() * (size * 0.25);
                let u = u * (size * 0.25);
                let v = v * (size * 0.25);
                let corners = [face - u - v, face + u - v, face + u + v, face - u + v]
                    .map(|p| center + rotation * p);
                let normal = rotation * normal.as_vec3();
                self.triangle([corners[0], corners[1], corners[2]], normal, color, glow.max(0.1));
                self.triangle([corners[0], corners[2], corners[3]], normal, color, glow.max(0.1));
            }
        }
        self.alpha = old_alpha;
        self.dissolve = old_dissolve;
    }
}
