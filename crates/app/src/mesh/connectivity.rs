//! Face-to-face links through a chunk's open cells, for cave culling.
//!
//! Faces are numbered `axis * 2 + positive`: -X, +X, -Y, +Y, -Z, +Z.
use super::{CHUNK_PAD, GeneratedChunk, PADDED_CHUNK_SIZE};
use crate::world::{BlockId, CHUNK_SIZE};
use engine_content::RenderKind;
use std::sync::atomic::{AtomicBool, Ordering};

const N: usize = CHUNK_SIZE as usize;
const FACE_BITS: u64 = (1 << 36) - 1;
const SPILL_BIT: u64 = 1 << 36;

/// Which pairs of chunk faces open space links, plus a spill flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkConnectivity(u64);

impl ChunkConnectivity {
    pub const ALL_OPEN: Self = Self(FACE_BITS);
    pub const SEALED: Self = Self(0);

    /// Links built from components, each given as the set of faces it touches.
    #[cfg(test)]
    pub fn from_components(components: impl IntoIterator<Item = u8>) -> Self {
        let mut links = Self::SEALED;
        for touched in components {
            links.link_component(touched);
        }
        links
    }

    fn link_component(&mut self, touched: u8) {
        let touched = u64::from(touched & 0x3f);
        let mut remaining = touched;
        while remaining != 0 {
            let face = remaining.trailing_zeros();
            self.0 |= touched << (face * 6);
            remaining &= remaining - 1;
        }
    }

    pub fn connects(self, a: usize, b: usize) -> bool {
        a < 6 && b < 6 && self.0 & (1 << (a * 6 + b)) != 0
    }

    /// Faces reachable from any face in `entered` (bit per face).
    pub fn exits_from(self, entered: u8) -> u8 {
        if self.0 & FACE_BITS == FACE_BITS && entered & 0x3f != 0 {
            return 0x3f;
        }
        let mut remaining = entered & 0x3f;
        let mut exits = 0;
        while remaining != 0 {
            let face = remaining.trailing_zeros();
            exits |= ((self.0 >> (face * 6)) & 0x3f) as u8;
            remaining &= remaining - 1;
        }
        exits
    }

    /// Geometry from `spills_out` blocks can reach a neighbour chunk.
    pub fn spills(self) -> bool {
        self.0 & SPILL_BIT != 0
    }

    fn with_spill(self, spills: bool) -> Self {
        if spills { Self(self.0 | SPILL_BIT) } else { self }
    }

    /// Connectivity of a chunk filled with one block.
    pub fn uniform(block: BlockId) -> Self {
        if occludes(block) { Self::SEALED } else { Self::ALL_OPEN.with_spill(spills_out(block)) }
    }

    /// Flood-fills the chunk's own 32 cubed cells; `None` if cancelled or malformed.
    pub fn compute(chunk: &GeneratedChunk, cancelled: &AtomicBool) -> Option<Self> {
        if chunk.blocks.len() != (PADDED_CHUNK_SIZE as usize).pow(3) {
            return None;
        }
        let mut solid = vec![0u32; N * N];
        let mut solid_cells = 0;
        let mut spills = false;
        for y in 0..N {
            for z in 0..N {
                let start =
                    GeneratedChunk::local_index(CHUNK_PAD, y as i32 + CHUNK_PAD, z as i32 + CHUNK_PAD);
                let mut bits = 0u32;
                for (x, &b) in chunk.blocks[start..start + N].iter().enumerate() {
                    if occludes(b) {
                        bits |= 1 << x;
                    } else if !spills {
                        spills = spills_out(b);
                    }
                }
                solid[y * N + z] = bits;
                solid_cells += bits.count_ones() as usize;
            }
        }
        let links = if solid_cells == 0 {
            Self::ALL_OPEN
        } else if solid_cells == N * N * N {
            Self::SEALED
        } else {
            flood(&mut solid, cancelled)?
        };
        Some(links.with_spill(spills))
    }
}

/// A full cube the mesher draws and culls neighbours against; cutout leaves,
/// liquids, torches, hinged fittings and models stay open.
pub fn occludes(b: BlockId) -> bool {
    b.is_opaque() && matches!(b.material().render.kind, RenderKind::Opaque) && !b.is_torch()
}

/// Torches, models and non-occluding cutouts (hinged fittings, fire) may draw
/// geometry past their own cell.
fn spills_out(b: BlockId) -> bool {
    let render = &b.material().render;
    b.is_torch() || b.is_model() || (matches!(render.kind, RenderKind::Cutout) && !render.occludes_light)
}

fn flood(seen: &mut [u32], cancelled: &AtomicBool) -> Option<ChunkConnectivity> {
    let mut stack: Vec<u16> = Vec::with_capacity(N * N);
    let mut links = ChunkConnectivity::SEALED;
    for row in 0..N * N {
        while seen[row] != u32::MAX {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let x = (!seen[row]).trailing_zeros() as usize;
            seen[row] |= 1 << x;
            stack.push((row * N + x) as u16);
            let mut touched = 0u8;
            while let Some(cell) = stack.pop() {
                let cell = cell as usize;
                let (row, x) = (cell / N, cell % N);
                let (y, z) = (row / N, row % N);
                let edge = |v: usize, low: u8| if v == 0 { low } else if v == N - 1 { low << 1 } else { 0 };
                touched |= edge(x, 1) | edge(y, 4) | edge(z, 16);
                // This component already connects every face. Unvisited cells
                // cannot add any links, and spills were collected in the scan.
                if touched == 0x3f {
                    return Some(ChunkConnectivity::ALL_OPEN);
                }
                let mut visit = |row: usize, x: usize| {
                    if seen[row] & (1 << x) == 0 {
                        seen[row] |= 1 << x;
                        stack.push((row * N + x) as u16);
                    }
                };
                if x > 0 { visit(row, x - 1); }
                if x + 1 < N { visit(row, x + 1); }
                if z > 0 { visit(row - 1, x); }
                if z + 1 < N { visit(row + 1, x); }
                if y > 0 { visit(row - N, x); }
                if y + 1 < N { visit(row + N, x); }
            }
            links.link_component(touched);
        }
    }
    Some(links)
}

#[cfg(test)]
#[path = "connectivity_tests.rs"]
mod tests;
