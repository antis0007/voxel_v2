use engine_content::BlockId;
use engine_core::CHUNK_SIZE;
use glam::IVec3;
use std::collections::HashSet;

#[derive(Clone)]
pub struct ChunkData {
    /// Derived, conservative local bounds since procedural generation. Cold
    /// records use the full chunk; this metadata does not change the save ABI.
    pub modified_bounds: Option<(IVec3, IVec3)>,
    pub structural_bounds: Option<(IVec3, IVec3)>,
    pub blocks: Box<[BlockId; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]>,
    pub light: Box<[u8; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]>,
    pub torches: HashSet<IVec3>,
}

impl ChunkData {
    pub fn note_modified(&mut self, p: IVec3, old: BlockId, new: BlockId) {
        let p = p.rem_euclid(IVec3::splat(CHUNK_SIZE));
        self.modified_bounds = Some(self.modified_bounds.map_or((p, p + IVec3::ONE), |(lo, hi)|
            (lo.min(p), hi.max(p + IVec3::ONE))));
        if !matches!(old,BlockId::Air|BlockId::Water) || !matches!(new,BlockId::Air|BlockId::Water) {
            self.structural_bounds=Some(self.structural_bounds.map_or((p,p+IVec3::ONE),|(lo,hi)|
                (lo.min(p),hi.max(p+IVec3::ONE))));
        }
    }
    pub fn local_index(x: i32, y: i32, z: i32) -> usize {
        let lx = x.rem_euclid(CHUNK_SIZE) as usize;
        let ly = y.rem_euclid(CHUNK_SIZE) as usize;
        let lz = z.rem_euclid(CHUNK_SIZE) as usize;
        (ly * CHUNK_SIZE as usize * CHUNK_SIZE as usize) + (lz * CHUNK_SIZE as usize) + lx
    }
}

