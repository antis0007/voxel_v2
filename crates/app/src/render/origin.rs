use engine_core::{CHUNK_SIZE, floor_div};
use glam::{DVec3, IVec3, Vec3};
use std::collections::HashMap;

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

/// Per-draw placement read from vertex buffer slot 1.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawTransform {
    pub offset: [f32; 4],
    /// Unit quaternion `xyzw`; the identity until bent chunks need it (decision 0025).
    pub rotation: [f32; 4],
}

impl DrawTransform {
    pub const IDENTITY: DrawTransform = DrawTransform { offset: [0.0; 4], rotation: [0.0, 0.0, 0.0, 1.0] };

    /// Instance-step layout at slot 1, shared by the world and shadow pipelines.
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
///
/// A twin table in the same slot order keeps the shadow pass in absolute
/// coordinates until it moves to the origin frame (plan Task 4 deletes it).
#[derive(Default)]
pub struct DrawOffsets {
    transforms: Vec<DrawTransform>,
    twins: Vec<DrawTransform>,
    slots: HashMap<IVec3, u32>,
    buffer: Option<wgpu::Buffer>,
    twin_buffer: Option<wgpu::Buffer>,
    capacity: usize,
}

impl DrawOffsets {
    /// Clears the table and puts `absolute` in `ABSOLUTE_SLOT`.
    ///
    /// The twin's `ABSOLUTE_SLOT` is the zero offset: absolute vertices stay absolute.
    pub fn begin_frame(&mut self, absolute: DrawTransform) {
        self.transforms.clear();
        self.twins.clear();
        self.slots.clear();
        self.transforms.push(absolute);
        self.twins.push(DrawTransform::IDENTITY);
    }

    /// Returns `chunk`'s slot, computing its transform on first use this frame.
    ///
    /// A slot first taken here gets a zero twin; passes that read the twin
    /// take their slots with `slot_with_twin`.
    pub fn slot(&mut self, chunk: IVec3, transform: impl FnOnce() -> DrawTransform) -> u32 {
        self.slot_with_twin(chunk, transform, || DrawTransform::IDENTITY)
    }

    /// Returns `chunk`'s slot, computing both tables' transforms on first use.
    pub fn slot_with_twin(
        &mut self,
        chunk: IVec3,
        primary: impl FnOnce() -> DrawTransform,
        twin: impl FnOnce() -> DrawTransform,
    ) -> u32 {
        if let Some(&slot) = self.slots.get(&chunk) {
            return slot;
        }
        let slot = self.transforms.len() as u32;
        self.transforms.push(primary());
        self.twins.push(twin());
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
        if self.buffer.is_none() || self.twin_buffer.is_none() || self.transforms.len() > self.capacity {
            self.capacity = self.transforms.len().next_power_of_two().max(4096);
            let table = |label| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: (self.capacity * std::mem::size_of::<DrawTransform>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            };
            self.buffer = Some(table("draw offsets"));
            self.twin_buffer = Some(table("draw offsets absolute twin"));
        }
        if let Some(buffer) = &self.buffer {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&self.transforms));
        }
        if let Some(buffer) = &self.twin_buffer {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&self.twins));
        }
    }

    /// The uploaded table, bound at vertex slot 1.
    pub fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// The uploaded twin table, bound at vertex slot 1 by the shadow pass.
    pub fn twin_buffer(&self) -> Option<&wgpu::Buffer> {
        self.twin_buffer.as_ref()
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
    fn draw_offsets_twin_shares_slot_order() {
        let at = |x: f32| DrawTransform { offset: [x, 0.0, 0.0, 0.0], ..DrawTransform::IDENTITY };
        let mut d = DrawOffsets::default();
        d.begin_frame(at(-4096.0));
        assert_eq!(d.slot_with_twin(IVec3::X, || at(1.0), || at(32.0)), 1);
        assert_eq!(d.slot_with_twin(IVec3::X, || panic!("recomputed"), || panic!("recomputed")), 1);
        assert_eq!(d.slot(IVec3::NEG_X, || at(-1.0)), 2);
        assert_eq!(d.transforms, [at(-4096.0), at(1.0), at(-1.0)]);
        assert_eq!(d.twins, [DrawTransform::IDENTITY, at(32.0), DrawTransform::IDENTITY]);
    }

    #[test]
    fn eye_from_origin_keeps_sub_millimetre_precision() {
        let o = RenderOrigin { voxel: IVec3::new(897_024, 0, 0) };
        let eye = DVec3::new(900_031.999_7, 101.25, 0.5);
        let rel = o.eye_from_origin(eye);
        assert!((rel.x as f64 - 3_007.999_7).abs() < 1e-3);
    }
}
