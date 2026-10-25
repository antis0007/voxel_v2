use engine_core::{CHUNK_SIZE, floor_div};
use crate::fast_hash::FastMap;
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
    /// `bias` voxels are added on every axis after snapping; the band is
    /// measured from the unbiased origin, so a bias never forces a re-snap.
    pub fn follow(current: Option<RenderOrigin>, eye_voxels: DVec3, bias: i32) -> RenderOrigin {
        let bias = IVec3::splat(bias);
        if let Some(origin) = current {
            let rel = eye_voxels - (origin.voxel - bias).as_dvec3();
            let lo = -f64::from(ORIGIN_GRID_VOXELS);
            let hi = 2.0 * f64::from(ORIGIN_GRID_VOXELS);
            if [rel.x, rel.y, rel.z].iter().all(|&d| d >= lo && d < hi) {
                return origin;
            }
        }
        let snap = |v: f64| floor_div(v.floor() as i32, ORIGIN_GRID_VOXELS) * ORIGIN_GRID_VOXELS;
        RenderOrigin { voxel: IVec3::new(snap(eye_voxels.x), snap(eye_voxels.y), snap(eye_voxels.z)) + bias }
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

    /// Shader `eye_from_origin` in metres, computed in f64.
    ///
    /// The view matrix already translates by `-residual`, so it is left out
    /// here; otherwise origin-frame vertices would subtract it twice.
    pub fn eye_uniform(self, precise_eye_m: DVec3, scale: f64, residual: Vec3) -> Vec3 {
        (precise_eye_m - self.voxel.as_dvec3() * scale - residual.as_dvec3()).as_vec3()
    }
}

/// Largest accepted `VOXEL_RENDER_ORIGIN_BIAS` magnitude, in voxels.
const MAX_ORIGIN_BIAS: i32 = 1 << 24;

/// Parses a `VOXEL_RENDER_ORIGIN_BIAS` value in voxels.
fn parse_origin_bias(text: &str) -> Result<i32, String> {
    let bias: i32 = text.trim().parse().map_err(|_| format!("not an integer: {text:?}"))?;
    if bias % ORIGIN_GRID_VOXELS != 0 || bias.unsigned_abs() > MAX_ORIGIN_BIAS.unsigned_abs() {
        return Err(format!("{bias} is not a multiple of {ORIGIN_GRID_VOXELS} within +-{MAX_ORIGIN_BIAS}"));
    }
    Ok(bias)
}

/// Debug render-origin bias from `VOXEL_RENDER_ORIGIN_BIAS`, read once.
///
/// A nonzero bias moves the origin away from the eye at spawn, so captures
/// there exercise the origin-frame path with a nonzero origin.
pub fn origin_bias_voxels() -> i32 {
    static BIAS: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *BIAS.get_or_init(|| match std::env::var("VOXEL_RENDER_ORIGIN_BIAS") {
        Err(_) => 0,
        Ok(text) => match parse_origin_bias(&text) {
            Ok(bias) => {
                eprintln!("[render-origin] bias={bias} voxels");
                bias
            }
            Err(reason) => {
                eprintln!("[render-origin] VOXEL_RENDER_ORIGIN_BIAS ignored: {reason}");
                0
            }
        },
    })
}

/// Per-draw placement read from vertex buffer slot 1.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawTransform {
    pub offset: [f32; 4],
    /// Unit quaternion `xyzw`; identity until bent chunks (decision 0025).
    pub rotation: [f32; 4],
}

impl DrawTransform {
    pub const IDENTITY: DrawTransform = DrawTransform { offset: [0.0; 4], rotation: [0.0, 0.0, 0.0, 1.0] };

    /// Instance-step layout at slot 1 for world and shadow pipelines.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<DrawTransform>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &Self::ATTRIBUTES,
    };
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![5 => Float32x4, 6 => Float32x4];
}

const _: [(); 32] = [(); std::mem::size_of::<DrawTransform>()];

/// One frame's table of draw transforms, one slot per chunk.
///
/// The main pass and every shadow cascade take slots from the same table, so a
/// chunk drawn by several passes reads one transform. A draw selects its slot
/// through `first_instance`.
#[derive(Default)]
pub struct DrawOffsets {
    transforms: Vec<DrawTransform>,
    slots: FastMap<IVec3, u32>,
    buffer: Option<wgpu::Buffer>,
    capacity: usize,
}

impl DrawOffsets {
    /// Clears the table and puts `absolute` in `ABSOLUTE_SLOT`.
    pub fn begin_frame(&mut self, absolute: DrawTransform) {
        self.transforms.clear();
        self.slots.clear();
        self.transforms.push(absolute);
    }

    /// Returns `chunk`'s slot, computing its transform on first use this frame.
    pub fn slot(&mut self, chunk: IVec3, transform: impl FnOnce() -> DrawTransform) -> u32 {
        if let Some(&slot) = self.slots.get(&chunk) {
            return slot;
        }
        let slot = self.transforms.len() as u32;
        self.transforms.push(transform());
        self.slots.insert(chunk, slot);
        slot
    }

    /// Writes the table to the GPU, growing the buffer if needed.
    ///
    /// Call once per frame after every draw list has taken its slots and before
    /// the first pass binds the buffer: a pass that already bound it would keep
    /// the replaced buffer.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.transforms.is_empty() {
            return;
        }
        if self.buffer.is_none() || self.transforms.len() > self.capacity {
            self.capacity = self.transforms.len().next_power_of_two().max(4096);
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("draw offsets"),
                size: (self.capacity * std::mem::size_of::<DrawTransform>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = &self.buffer {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&self.transforms));
        }
    }

    /// The uploaded table, bound at vertex slot 1.
    pub fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }
}

#[cfg(test)]
#[path = "origin_gpu_tests.rs"]
mod origin_gpu_tests;

#[cfg(test)]
#[path = "origin_far_chunk_gpu_tests.rs"]
mod origin_far_chunk_gpu_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_snaps_with_floor_division() {
        let o = RenderOrigin::follow(None, DVec3::new(-0.5, 10.0, 900_032.25), 0);
        assert_eq!(o.voxel, IVec3::new(-4096, 0, 897_024));
    }

    #[test]
    fn origin_holds_inside_hysteresis_band() {
        let o = RenderOrigin::follow(None, DVec3::new(100.0, 0.0, 0.0), 0);
        for x in [-4095.0, 0.0, 4096.0, 8191.0] {
            assert_eq!(RenderOrigin::follow(Some(o), DVec3::new(x, 0.0, 0.0), 0), o);
        }
        assert_ne!(RenderOrigin::follow(Some(o), DVec3::new(8192.0, 0.0, 0.0), 0), o);
        assert_ne!(RenderOrigin::follow(Some(o), DVec3::new(-4097.0, 0.0, 0.0), 0), o);
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
    fn draw_offsets_dedupe_chunks_and_reserve_slot_zero() {
        let mut d = DrawOffsets::default();
        d.begin_frame(DrawTransform::IDENTITY);
        let a = d.slot(IVec3::new(1, 0, 0), || DrawTransform::IDENTITY);
        let b = d.slot(IVec3::new(-1, 0, 0), || DrawTransform::IDENTITY);
        assert_eq!((a, b), (1, 2));
        assert_eq!(d.slot(IVec3::new(1, 0, 0), || panic!("recomputed")), 1);
        d.begin_frame(DrawTransform::IDENTITY);
        assert_eq!(d.slot(IVec3::new(-1, 0, 0), || DrawTransform::IDENTITY), 1);
    }

    #[test]
    fn eye_uniform_keeps_sub_millimetre_precision() {
        let o = RenderOrigin { voxel: IVec3::new(897_024, 0, 0) };
        let eye = DVec3::new(900_031.999_7, 101.25, 0.5);
        let rel = o.eye_uniform(eye, 1.0, Vec3::ZERO);
        assert!((rel.x as f64 - 3_007.999_7).abs() < 1e-3);
    }

    #[test]
    fn origin_bias_offsets_snap_without_moving_the_band() {
        let biased = RenderOrigin::follow(None, DVec3::new(100.0, 50.0, -20.0), 4096);
        assert_eq!(biased.voxel, IVec3::new(4096, 4096, 0));
        let plain = RenderOrigin::follow(None, DVec3::new(100.0, 50.0, -20.0), 0);
        for x in [-4097.0, -4095.0, 0.0, 8191.0, 8192.0, 20_000.0] {
            let eye = DVec3::new(x, 50.0, -20.0);
            let b = RenderOrigin::follow(Some(biased), eye, 4096);
            assert_eq!(b.voxel - IVec3::splat(4096), RenderOrigin::follow(Some(plain), eye, 0).voxel, "x = {x}");
        }
    }

    #[test]
    fn origin_bias_accepts_grid_multiples_only() {
        assert_eq!(parse_origin_bias("4096"), Ok(4096));
        assert_eq!(parse_origin_bias(" -8192 "), Ok(-8192));
        assert_eq!(parse_origin_bias("0"), Ok(0));
        for bad in ["100", "abc", "", "4294967296", "-2147483648", "33554432"] {
            assert!(parse_origin_bias(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn eye_uniform_cancels_view_residual_far_from_spawn() {
        for (scale, eye) in [
            (1.0, DVec3::new(897_030.123_4, 101.3, -1_003_511.97)),
            (0.25, DVec3::new(897_030.123_4, 101.3, -1_003_511.97)),
        ] {
            // As render.rs computes it: the part of the eye the f32 view matrix holds.
            let residual = (eye - eye.as_vec3().as_dvec3()).as_vec3();
            assert!(residual.x.abs() > 1e-3 && residual.z.abs() > 1e-3, "{residual}");
            let origin = RenderOrigin::follow(None, eye / scale, 0);
            let e = origin.eye_uniform(eye, scale, residual);
            let chunk = origin.voxel / CHUNK_SIZE + IVec3::new(0, 3, 0);
            let in_chunk = Vec3::new(17.5, 7.25, 9.0);
            let local = Vec3::from_slice(&origin.chunk_offset(chunk)[..3]) + in_chunk;
            let drawn = local * scale as f32 - e - residual;
            let p_abs = (chunk * CHUNK_SIZE).as_dvec3() + in_chunk.as_dvec3();
            let truth = p_abs * scale - eye;
            let err = (drawn.as_dvec3() - truth).abs().max_element();
            assert!(err < 1e-4, "scale {scale}: drawn {drawn} truth {truth} err {err}");
        }
    }
}
