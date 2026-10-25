use engine_core::{CHUNK_SIZE, floor_div};
use glam::{DVec3, IVec3, Vec3};

/// Edge length of the grid the render origin snaps to.
pub const ORIGIN_GRID_VOXELS: i32 = 4096;

/// Draw-offset slot that holds `absolute_offset`.
pub const ABSOLUTE_SLOT: u32 = 0;

/// Integer voxel position that GPU positions are measured from.
///
/// Subtracting it in integers before the f32 cast keeps vertex coordinates
/// exact however far the camera is from the world origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOrigin {
    pub voxel: IVec3,
}

impl RenderOrigin {
    /// Holds `current` inside its band, else snaps to the eye's cell.
    ///
    /// The band spans one grid cell behind and two ahead of the origin on every
    /// axis, so an eye hovering at a cell edge does not re-snap every frame.
    pub fn follow(current: Option<RenderOrigin>, eye_voxels: DVec3) -> RenderOrigin {
        if let Some(origin) = current {
            let rel = eye_voxels - origin.voxel.as_dvec3();
            let lo = -f64::from(ORIGIN_GRID_VOXELS);
            let hi = 2.0 * f64::from(ORIGIN_GRID_VOXELS);
            if [rel.x, rel.y, rel.z].iter().all(|&d| d >= lo && d < hi) {
                return origin;
            }
        }
        let snap = |v: f64| floor_div(v.floor() as i32, ORIGIN_GRID_VOXELS) * ORIGIN_GRID_VOXELS;
        RenderOrigin { voxel: IVec3::new(snap(eye_voxels.x), snap(eye_voxels.y), snap(eye_voxels.z)) }
    }

    /// Origin-relative offset of a chunk's minimum corner, in voxels.
    pub fn chunk_offset(self, chunk: IVec3) -> [f32; 4] {
        let rel = chunk * CHUNK_SIZE - self.voxel;
        [rel.x as f32, rel.y as f32, rel.z as f32, 0.0]
    }

    /// Offset that maps absolute voxel coordinates to origin-relative ones.
    pub fn absolute_offset(self) -> [f32; 4] {
        [-self.voxel.x as f32, -self.voxel.y as f32, -self.voxel.z as f32, 0.0]
    }

    /// Eye position relative to the origin, narrowed to f32 after subtracting.
    pub fn eye_from_origin(self, eye_voxels: DVec3) -> Vec3 {
        (eye_voxels - self.voxel.as_dvec3()).as_vec3()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_snaps_with_floor_division() {
        let o = RenderOrigin::follow(None, DVec3::new(-0.5, 10.0, 900_032.25));
        assert_eq!(o.voxel, IVec3::new(-4096, 0, 897_024));
    }

    #[test]
    fn origin_holds_inside_hysteresis_band() {
        let o = RenderOrigin::follow(None, DVec3::new(100.0, 0.0, 0.0));
        for x in [-4095.0, 0.0, 4096.0, 8191.0] {
            assert_eq!(RenderOrigin::follow(Some(o), DVec3::new(x, 0.0, 0.0)), o);
        }
        assert_ne!(RenderOrigin::follow(Some(o), DVec3::new(8192.0, 0.0, 0.0)), o);
        assert_ne!(RenderOrigin::follow(Some(o), DVec3::new(-4097.0, 0.0, 0.0)), o);
    }

    #[test]
    fn chunk_offsets_are_exact_far_from_spawn() {
        let o = RenderOrigin { voxel: IVec3::new(897_024, 0, -1_003_520) };
        let chunk = IVec3::new(28_126, -2, -31_336);
        let off = o.chunk_offset(chunk);
        assert_eq!(off, [3_008.0, -64.0, 768.0, 0.0]);
        assert_eq!(o.absolute_offset(), [-897_024.0, 0.0, 1_003_520.0, 0.0]);
    }

    #[test]
    fn eye_from_origin_keeps_sub_millimetre_precision() {
        let o = RenderOrigin { voxel: IVec3::new(897_024, 0, 0) };
        let eye = DVec3::new(900_031.999_7, 101.25, 0.5);
        let rel = o.eye_from_origin(eye);
        assert!((rel.x as f64 - 3_007.999_7).abs() < 1e-3);
    }
}
