//! Opaque voxels around the camera, one bit each, so a torch ray can stop
//! at a wall. The shader walk in `ray_hits_opaque` matches `blocked`.
use crate::mesh::GeneratedChunk;
use crate::world::{BlockId, CHUNK_SIZE};
use glam::{IVec3, Vec3};
use std::sync::{Arc, OnceLock, Weak};
use super::streaming::ResidentChunk;
use wgpu::util::DeviceExt;

/// Largest edge of the camera window, in voxels. The buffer is sized for it;
/// world_shader.wgsl `placed_torch_extinguished` uses the same word count.
pub const MAX_DIM: i32 = 192;
const WORDS: usize = (MAX_DIM as usize * MAX_DIM as usize * MAX_DIM as usize) / 32;
const HEADER: usize = 16;
/// One bit per 4-voxel block of the window, set when any voxel in it is
/// opaque. The shader walk crosses a clear block in one step.
const COARSE: i32 = 4;
const COARSE_WORDS: usize = ((MAX_DIM / COARSE) as usize).pow(3) / 32;
const COARSE_OFFSET: usize = HEADER + WORDS * 4;
const EXTINGUISHED_CAPACITY: usize = 262_144;
const EXTINGUISHED_OFFSET: usize = COARSE_OFFSET + COARSE_WORDS * 4;
/// Cells one ray may cross. A 22 m lantern at 0.25 m is 88 voxels, up to 152
/// cell crossings on a diagonal. world_shader.wgsl `ray_hits_opaque` has the
/// same limit and tests the same cells, skipping clear coarse blocks.
const STEPS: i32 = 160;

/// Window edge and origin snap, in voxels. Torch light reaches 18-22 m, so a
/// 96-voxel window (12 m either side at 0.25 m) ended shadows mid-room. Fine
/// scales take the large window: 24 m either side at 0.25 m. Its origin moves
/// in 8-voxel steps so walking does not upload 860 KiB per voxel crossed.
fn window(scale: f32) -> (i32, i32) {
    if scale < 0.75 { (MAX_DIM, 8) } else { (96, 1) }
}

fn word_count(dim: i32) -> usize {
    (dim as usize).pow(3) / 32
}

pub struct State {
    buffer: wgpu::Buffer,
    words: Box<[u32; WORDS]>,
    coarse: Box<[u32; COARSE_WORDS]>,
    origin: IVec3,
    dim: i32,
    sources: Vec<(IVec3, Weak<ResidentChunk>)>,
    ready: bool,
    extinguished: Vec<[i32; 3]>,
}

impl State {
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("torch blockage"),
            contents: &vec![0u8; EXTINGUISHED_OFFSET + 4 + EXTINGUISHED_CAPACITY * 12],
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            buffer,
            words: Box::new([0; WORDS]),
            coarse: Box::new([0; COARSE_WORDS]),
            origin: IVec3::ZERO,
            dim: 96,
            sources: Vec::new(),
            ready: false,
            extinguished: Vec::new(),
        }
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn upload_extinguished(&mut self, queue: &wgpu::Queue, mut cells: Vec<[i32; 3]>) {
        cells.truncate(EXTINGUISHED_CAPACITY);
        if self.extinguished == cells { return; }
        queue.write_buffer(&self.buffer, EXTINGUISHED_OFFSET as u64, bytemuck::bytes_of(&(cells.len() as u32)));
        if !cells.is_empty() {
            queue.write_buffer(&self.buffer, EXTINGUISHED_OFFSET as u64 + 4, bytemuck::cast_slice(&cells));
        }
        self.extinguished = cells;
    }

    /// `eye` is the camera in voxels. Skips the upload when the window and its
    /// voxel sources are unchanged. A short move fills only the new slabs, and
    /// a replaced chunk rewrites only its own cells: at 0.25 m the window holds
    /// some 340 chunks, and filling all of it for each one that changed was a
    /// visible hitch.
    pub fn upload(&mut self, queue: &wgpu::Queue, cache: &super::ChunkCache, eye: Vec3, scale: f32) {
        let (dim, snap) = window(scale);
        let origin = (eye.floor().as_ivec3() - IVec3::splat(dim / 2)).div_euclid(IVec3::splat(snap)) * snap;
        let max = origin + IVec3::splat(dim);
        if dim != self.dim {
            self.dim = dim;
            self.ready = false;
        }
        let used = word_count(dim);
        // Only local CPU source identity matters. A distant mesh upload
        // cannot change any of the bits used by these torch rays.
        let mut current = cache_sources(cache, origin, max);
        let same_sources = self.sources.iter().all(|(coord, source)| {
            current.next().is_some_and(|(next, resident)| {
                *coord == next && Weak::ptr_eq(source, &Arc::downgrade(resident))
            })
        }) && current.next().is_none();
        if self.ready && self.origin == origin && same_sources {
            return;
        }
        let delta = origin - self.origin;
        if self.ready && delta.abs().max_element() < dim {
            let words = &mut self.words[..used];
            if delta != IVec3::ZERO {
                shift_bits(words, dim, delta);
                refill_slabs(words, dim, self.origin, origin, cache);
            }
            // The held `Weak` keeps each old allocation alive, so an equal
            // pointer is the same chunk and not a reused address.
            let old: engine_core::fast_hash::FastMap<IVec3, *const ResidentChunk> =
                self.sources.iter().map(|(coord, source)| (*coord, source.as_ptr())).collect();
            let lut = opaque_lut();
            let mut dirty: Option<(usize, usize)> = None;
            let mut touch = |range: Option<(usize, usize)>| {
                if let Some((lo, hi)) = range {
                    dirty = Some(dirty.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
                }
            };
            for (coord, resident) in cache_sources(cache, origin, max) {
                if old.get(&coord) == Some(&Arc::as_ptr(resident)) {
                    continue;
                }
                touch(clear_chunk(words, dim, origin, coord));
                stamp(words, dim, origin, coord, resident, lut, origin, max);
            }
            for (coord, _) in &self.sources {
                if cache.cpu_chunks.get(coord).is_none() {
                    touch(clear_chunk(words, dim, origin, *coord));
                }
            }
            self.origin = origin;
            self.sources.clear();
            self.sources.extend(cache_sources(cache, origin, max)
                .map(|(coord, resident)| (coord, Arc::downgrade(resident))));
            if delta != IVec3::ZERO {
                self.write(queue);
            } else if let Some((lo, hi)) = dirty {
                queue.write_buffer(&self.buffer, (HEADER + lo * 4) as u64, bytemuck::cast_slice(&self.words[lo..hi]));
                self.write_coarse(queue);
            }
            return;
        }
        self.words[..used].fill(0);
        fill_cache(&mut self.words[..used], dim, origin, cache, origin, max);
        self.origin = origin;
        self.sources.clear();
        self.sources.extend(cache_sources(cache, origin, max)
            .map(|(coord, resident)| (coord, Arc::downgrade(resident))));
        self.ready = true;
        self.write(queue);
    }

    fn write(&mut self, queue: &wgpu::Queue) {
        let header = [self.origin.x, self.origin.y, self.origin.z, self.dim];
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&header));
        queue.write_buffer(&self.buffer, HEADER as u64, bytemuck::cast_slice(&self.words[..word_count(self.dim)]));
        self.write_coarse(queue);
    }

    fn write_coarse(&mut self, queue: &wgpu::Queue) {
        let used = word_count(self.dim / COARSE);
        coarsen(&self.words[..word_count(self.dim)], self.dim, &mut self.coarse[..used]);
        queue.write_buffer(&self.buffer, COARSE_OFFSET as u64, bytemuck::cast_slice(&self.coarse[..used]));
    }
}

/// Rebuilds the block bits from the voxel bits. Both index x, then z, then y.
fn coarsen(words: &[u32], dim: i32, coarse: &mut [u32]) {
    coarse.fill(0);
    let row = (dim / 32) as usize;
    let dim = dim as usize;
    let cdim = dim / COARSE as usize;
    for (w, &bits) in words.iter().enumerate() {
        if bits == 0 {
            continue;
        }
        let (z, y) = ((w / row) % dim, w / row / dim);
        let base = cdim * (z / COARSE as usize + cdim * (y / COARSE as usize)) + (w % row) * 8;
        for nibble in 0..8 {
            if bits >> (nibble * 4) & 0xf != 0 {
                let i = base + nibble;
                coarse[i >> 5] |= 1 << (i & 31);
            }
        }
    }
}

fn opaque_lut() -> &'static [bool; BlockId::COUNT] {
    static LUT: OnceLock<[bool; BlockId::COUNT]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut table = [false; BlockId::COUNT];
        for &id in engine_content::ALL_BLOCK_IDS {
            table[id as usize] = id.is_opaque();
        }
        table
    })
}

fn cache_sources(
    cache: &super::ChunkCache,
    min: IVec3,
    max: IVec3,
) -> impl Iterator<Item = (IVec3, &Arc<ResidentChunk>)> {
    let lo = min.div_euclid(IVec3::splat(CHUNK_SIZE));
    let hi = (max - IVec3::ONE).div_euclid(IVec3::splat(CHUNK_SIZE));
    (lo.y..=hi.y).flat_map(move |y| {
        (lo.z..=hi.z).flat_map(move |z| {
            (lo.x..=hi.x).filter_map(move |x| {
                let coord = IVec3::new(x, y, z);
                cache.cpu_chunks.get(&coord).map(|source| (coord, source))
            })
        })
    })
}

fn fill_cache(
    words: &mut [u32],
    dim: i32,
    origin: IVec3,
    cache: &super::ChunkCache,
    min: IVec3,
    max: IVec3,
) {
    let lut = opaque_lut();
    for (coord, resident) in cache_sources(cache, min, max) {
        stamp(words, dim, origin, coord, resident, lut, min, max);
    }
}

fn fill_chunks(
    words: &mut [u32],
    dim: i32,
    origin: IVec3,
    chunks: &[(IVec3, GeneratedChunk)],
    min: IVec3,
    max: IVec3,
) {
    let lut = opaque_lut();
    for (coord, chunk) in chunks {
        stamp(words, dim, origin, *coord, chunk, lut, min, max);
    }
}

fn stamp(
    words: &mut [u32],
    window: i32,
    origin: IVec3,
    coord: IVec3,
    chunk: &GeneratedChunk,
    lut: &[bool; BlockId::COUNT],
    min: IVec3,
    max: IVec3,
) {
    let min = min.max(origin);
    let max = max.min(origin + IVec3::splat(window));
    let base = coord * CHUNK_SIZE;
    let lo = min.max(base);
    let hi = max.min(base + IVec3::splat(CHUNK_SIZE));
    if lo.x >= hi.x || lo.y >= hi.y || lo.z >= hi.z {
        return;
    }
    let pad = engine_core::PADDED_CHUNK_SIZE as usize;
    let chunk_origin = chunk.origin();
    let dim = window as usize;
    let start_x = (lo.x - chunk_origin.x) as usize;
    let width = (hi.x - lo.x) as usize;
    let dx = (lo.x - origin.x) as usize;
    let shift = dx & 31;
    for y in lo.y..hi.y {
        let ly = (y - chunk_origin.y) as usize;
        let dy = (y - origin.y) as usize;
        for z in lo.z..hi.z {
            let lz = (z - chunk_origin.z) as usize;
            let dz = (z - origin.z) as usize;
            let row = (ly * pad + lz) * pad + start_x;
            let mut mask = 0u32;
            for (bit, &block) in chunk.blocks[row..row + width].iter().enumerate() {
                mask |= u32::from(lut[block as usize]) << bit;
            }
            let word = (dx + dim * (dz + dim * dy)) >> 5;
            words[word] |= mask << shift;
            if shift + width > 32 {
                words[word + 1] |= mask >> (32 - shift);
            }
        }
    }
}

/// Zeroes one chunk's cells inside the window. Returns the word range touched.
fn clear_chunk(words: &mut [u32], window: i32, origin: IVec3, coord: IVec3) -> Option<(usize, usize)> {
    let base = coord * CHUNK_SIZE;
    let lo = origin.max(base);
    let hi = (origin + IVec3::splat(window)).min(base + IVec3::splat(CHUNK_SIZE));
    if lo.x >= hi.x || lo.y >= hi.y || lo.z >= hi.z {
        return None;
    }
    let dim = window as usize;
    let index = |p: IVec3| {
        let d = (p - origin).as_uvec3();
        d.x as usize + dim * (d.z as usize + dim * d.y as usize)
    };
    let width = (hi.x - lo.x) as usize;
    let shift = (lo.x - origin.x) as usize & 31;
    let mask = if width >= 32 { u32::MAX } else { (1u32 << width) - 1 };
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            let word = index(IVec3::new(lo.x, y, z)) >> 5;
            words[word] &= !(mask << shift);
            if shift + width > 32 {
                words[word + 1] &= !(mask >> (32 - shift));
            }
        }
    }
    Some((index(lo) >> 5, (index(hi - IVec3::ONE) >> 5) + 1))
}

fn solid(words: &[u32], dim: i32, origin: IVec3, cell: IVec3) -> bool {
    let d = cell - origin;
    if d.min_element() < 0 || d.max_element() >= dim {
        return false;
    }
    let i = (d.x + dim * (d.z + dim * d.y)) as usize;
    words[i >> 5] & (1u32 << (i & 31)) != 0
}

/// Voxel walk from `from_v` toward `to_v`. The starting cell and the flame's
/// cell are not tested. `blocked` and the shader use the same axis priority.
fn blocked(words: &[u32], dim: i32, origin: IVec3, from_v: Vec3, to_v: Vec3) -> bool {
    let delta = to_v - from_v;
    let len = delta.length();
    if len < 0.5 {
        return false;
    }
    let rd = delta / len;
    let mut map = from_v.floor().as_ivec3();
    let end = to_v.floor().as_ivec3();
    if map == end {
        return false;
    }
    let (step_x, mut tmax_x, tdelta_x) = axis(rd.x, from_v.x, map.x);
    let (step_y, mut tmax_y, tdelta_y) = axis(rd.y, from_v.y, map.y);
    let (step_z, mut tmax_z, tdelta_z) = axis(rd.z, from_v.z, map.z);
    for i in 0..STEPS {
        if map == end {
            return false;
        }
        if i > 0 && solid(words, dim, origin, map) {
            return true;
        }
        if tmax_x <= tmax_y && tmax_x <= tmax_z {
            if tmax_x > len {
                return false;
            }
            map.x += step_x;
            tmax_x += tdelta_x;
        } else if tmax_y <= tmax_z {
            if tmax_y > len {
                return false;
            }
            map.y += step_y;
            tmax_y += tdelta_y;
        } else {
            if tmax_z > len {
                return false;
            }
            map.z += step_z;
            tmax_z += tdelta_z;
        }
    }
    false
}

fn axis(rd: f32, origin: f32, map: i32) -> (i32, f32, f32) {
    if rd > 1e-6 {
        let inv = 1.0 / rd;
        (1, (map as f32 + 1.0 - origin) * inv, inv)
    } else if rd < -1e-6 {
        let inv = 1.0 / rd;
        (-1, (map as f32 - origin) * inv, -inv)
    } else {
        (0, 1e30, 1e30)
    }
}

fn shift_bits(words: &mut [u32], dim: i32, delta: IVec3) {
    if delta.x != 0 {
        let row = (dim / 32) as usize;
        let rows = words.len() / row;
        for r in 0..rows {
            shift_row(&mut words[r * row..(r + 1) * row], delta.x);
        }
    }
    if delta.z != 0 {
        shift_z(words, dim, delta.z);
    }
    if delta.y != 0 {
        shift_y(words, dim, delta.y);
    }
}

fn shift_row(row: &mut [u32], delta_x: i32) {
    let bits = delta_x.unsigned_abs();
    let n = row.len();
    let word_shift = (bits / 32) as usize;
    let bit_shift = bits % 32;
    if word_shift >= n {
        row.fill(0);
        return;
    }
    if delta_x > 0 {
        if word_shift > 0 {
            row.copy_within(word_shift.., 0);
            row[n - word_shift..].fill(0);
        }
        if bit_shift == 0 {
            return;
        }
        let mut carry = 0u32;
        for w in row.iter_mut().rev() {
            let incoming = *w << (32 - bit_shift);
            *w = (*w >> bit_shift) | carry;
            carry = incoming;
        }
    } else {
        if word_shift > 0 {
            row.copy_within(0..n - word_shift, word_shift);
            row[..word_shift].fill(0);
        }
        if bit_shift == 0 {
            return;
        }
        let mut carry = 0u32;
        for w in row.iter_mut() {
            let outgoing = *w >> (32 - bit_shift);
            *w = (*w << bit_shift) | carry;
            carry = outgoing;
        }
    }
}

fn shift_z(words: &mut [u32], dim: i32, delta_z: i32) {
    let row = (dim / 32) as usize;
    let plane = row * dim as usize;
    let planes = words.len() / plane;
    let dz = delta_z.unsigned_abs() as usize;
    if dz >= dim as usize {
        words.fill(0);
        return;
    }
    for p in 0..planes {
        let plane_w = &mut words[p * plane..(p + 1) * plane];
        if delta_z > 0 {
            plane_w.copy_within(dz * row.., 0);
            plane_w[(dim as usize - dz) * row..].fill(0);
        } else {
            let keep = (dim as usize - dz) * row;
            plane_w.copy_within(0..keep, dz * row);
            plane_w[..dz * row].fill(0);
        }
    }
}

fn shift_y(words: &mut [u32], dim: i32, delta_y: i32) {
    let plane = (dim as usize * dim as usize) / 32;
    let dy = delta_y.unsigned_abs() as usize;
    if dy >= dim as usize {
        words.fill(0);
        return;
    }
    if delta_y > 0 {
        words.copy_within(dy * plane.., 0);
        words[(dim as usize - dy) * plane..].fill(0);
    } else {
        let keep = (dim as usize - dy) * plane;
        words.copy_within(0..keep, dy * plane);
        words[..dy * plane].fill(0);
    }
}

fn refill_slabs(words: &mut [u32], dim: i32, old: IVec3, new: IVec3, cache: &super::ChunkCache) {
    let d = new - old;
    let hi = new + IVec3::splat(dim);
    if d.x > 0 {
        fill_cache(words, dim, new, cache, new + IVec3::new(dim - d.x, 0, 0), hi);
    } else if d.x < 0 {
        fill_cache(words, dim, new, cache, new, new + IVec3::new(-d.x, dim, dim));
    }
    if d.y > 0 {
        fill_cache(words, dim, new, cache, new + IVec3::new(0, dim - d.y, 0), hi);
    } else if d.y < 0 {
        fill_cache(words, dim, new, cache, new, new + IVec3::new(dim, -d.y, dim));
    }
    if d.z > 0 {
        fill_cache(words, dim, new, cache, new + IVec3::new(0, 0, dim - d.z), hi);
    } else if d.z < 0 {
        fill_cache(words, dim, new, cache, new, new + IVec3::new(dim, dim, -d.z));
    }
}

fn refill_chunks(words: &mut [u32], dim: i32, old: IVec3, new: IVec3, chunks: &[(IVec3, GeneratedChunk)]) {
    let d = new - old;
    let hi = new + IVec3::splat(dim);
    if d.x > 0 {
        fill_chunks(words, dim, new, chunks, new + IVec3::new(dim - d.x, 0, 0), hi);
    } else if d.x < 0 {
        fill_chunks(words, dim, new, chunks, new, new + IVec3::new(-d.x, dim, dim));
    }
    if d.y > 0 {
        fill_chunks(words, dim, new, chunks, new + IVec3::new(0, dim - d.y, 0), hi);
    } else if d.y < 0 {
        fill_chunks(words, dim, new, chunks, new, new + IVec3::new(dim, -d.y, dim));
    }
    if d.z > 0 {
        fill_chunks(words, dim, new, chunks, new + IVec3::new(0, 0, dim - d.z), hi);
    } else if d.z < 0 {
        fill_chunks(words, dim, new, chunks, new, new + IVec3::new(dim, dim, -d.z));
    }
}

fn air_chunk(coord: IVec3) -> GeneratedChunk {
    let n = engine_core::PADDED_CHUNK_SIZE.pow(3) as usize;
    GeneratedChunk::new(coord, vec![BlockId::Air; n])
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIM: i32 = 96;
    const WORDS: usize = (DIM as usize).pow(3) / 32;

    fn wall_chunk() -> (IVec3, GeneratedChunk) {
        let mut chunk = air_chunk(IVec3::ZERO);
        for y in 0..8 {
            for z in 0..8 {
                chunk.set_world(IVec3::new(4, y, z), BlockId::Stone);
            }
        }
        (IVec3::ZERO, chunk)
    }

    #[test]
    fn wall_blocks_the_far_face_and_not_the_near_face() {
        let chunks = [wall_chunk()];
        let origin = IVec3::ZERO;
        let mut words = vec![0u32; WORDS];
        fill_chunks(&mut words, DIM, origin, &chunks, origin, origin + IVec3::splat(DIM));
        let torch = Vec3::new(2.5, 2.5, 2.5);
        let near = Vec3::new(4.0, 2.5, 2.5) + Vec3::NEG_X * 0.55;
        let far = Vec3::new(5.0, 2.5, 2.5) + Vec3::X * 0.55;
        assert!(!blocked(&words, DIM, origin, near, torch));
        assert!(blocked(&words, DIM, origin, far, torch));
        // Same room, beside the wall: the ray never enters the stone cell.
        let along = Vec3::new(2.5, 2.5, 6.5);
        assert!(!blocked(&words, DIM, origin, along, torch));
        let clear = [(IVec3::ZERO, air_chunk(IVec3::ZERO))];
        let mut air_words = vec![0u32; WORDS];
        fill_chunks(&mut air_words, DIM, origin, &clear, origin, origin + IVec3::splat(DIM));
        assert!(!blocked(&air_words, DIM, origin, far, torch));
    }

    #[test]
    fn a_short_move_matches_filling_again() {
        let mut chunk = air_chunk(IVec3::ZERO);
        chunk.set_world(IVec3::new(4, 2, 2), BlockId::Stone);
        chunk.set_world(IVec3::new(10, 6, 8), BlockId::Stone);
        chunk.set_world(IVec3::new(1, 1, 14), BlockId::Stone);
        let chunks = [(IVec3::ZERO, chunk)];
        let old = IVec3::new(-4, -4, -4);
        let new = IVec3::new(-3, -5, -2);
        let mut shifted = vec![0u32; WORDS];
        fill_chunks(&mut shifted, DIM, old, &chunks, old, old + IVec3::splat(DIM));
        shift_bits(&mut shifted, DIM, new - old);
        refill_chunks(&mut shifted, DIM, old, new, &chunks);
        let mut fresh = vec![0u32; WORDS];
        fill_chunks(&mut fresh, DIM, new, &chunks, new, new + IVec3::splat(DIM));
        assert_eq!(shifted, fresh);
    }
}
