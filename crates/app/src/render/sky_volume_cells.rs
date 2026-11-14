//! Occupancy sources for `sky_volume` levels: a resident chunk read voxel by
//! voxel, or a cached downsample of it for the coarse level.
use super::{class_lut, Occupancy};
use crate::mesh::{GeneratedChunk, PADDED_CHUNK_SIZE};
use crate::world::CHUNK_SIZE;
use glam::IVec3;

/// Occupancy of one chunk in level cells, `local` in `0..chunk_cells`.
pub trait Cells {
    fn class_at(&self, local: IVec3) -> Occupancy;
}

impl Cells for GeneratedChunk {
    #[inline]
    fn class_at(&self, local: IVec3) -> Occupancy {
        let pad = PADDED_CHUNK_SIZE as usize;
        let p = (local + IVec3::ONE).as_uvec3();
        class_lut()[self.blocks[(p.y as usize * pad + p.z as usize) * pad + p.x as usize] as usize]
    }
}

/// A chunk at `scale` voxels per cell. A cell holding a full voxel layer's worth
/// of solid is solid, so one-voxel roofs and walls still block the sky; failing
/// that, one with a layer's worth of solid and leaves is porous foliage.
pub struct Coarse {
    cells: i32,
    classes: Vec<Occupancy>,
}

impl Coarse {
    pub fn build(chunk: &GeneratedChunk, scale: i32) -> Self {
        let cells = CHUNK_SIZE / scale;
        let volume = scale * scale * scale;
        let mut classes = Vec::with_capacity((cells * cells * cells) as usize);
        for cy in 0..cells {
            for cz in 0..cells {
                for cx in 0..cells {
                    let base = IVec3::new(cx, cy, cz) * scale;
                    let (mut solid, mut filled) = (0, 0);
                    for y in 0..scale {
                        for z in 0..scale {
                            for x in 0..scale {
                                match chunk.class_at(base + IVec3::new(x, y, z)) {
                                    Occupancy::Solid => {
                                        solid += 1;
                                        filled += 1;
                                    }
                                    Occupancy::Leaf => filled += 1,
                                    Occupancy::Air => {}
                                }
                            }
                        }
                    }
                    classes.push(if solid * scale >= volume {
                        Occupancy::Solid
                    } else if filled * scale >= volume {
                        Occupancy::Leaf
                    } else {
                        Occupancy::Air
                    });
                }
            }
        }
        Self { cells, classes }
    }
}

impl Cells for Coarse {
    #[inline]
    fn class_at(&self, local: IVec3) -> Occupancy {
        self.classes[((local.y * self.cells + local.z) * self.cells + local.x) as usize]
    }
}
