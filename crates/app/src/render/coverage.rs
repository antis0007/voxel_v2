//! Shared CPU/WGSL ownership of complete GPU-resident chunk columns.
use glam::Vec3;

pub(crate) const MAX_SIDE: i32 = 129;
pub(crate) const MAX_SPARSE_COLUMNS: usize = 512;
const SPARSE_SLOTS: usize = 1024;
const EMPTY_KEY: [i32; 2] = [i32::MIN; 2];

fn column_hash(x: i32, z: i32) -> usize {
    let mixed = (x as u32).wrapping_mul(0x9e37_79b1) ^ (z as u32).wrapping_mul(0x85eb_ca77);
    ((mixed ^ (mixed >> 16)) & 1023) as usize
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResidentCoverage {
    /// Origin chunk X/Z, square width, enabled (nonzero).
    pub header: [i32; 4],
    /// Row-major bits, packed into WGSL vec4<u32> lanes.
    pub words: [[u32; 4]; 131],
    /// Owned sparse count (including sentinel), sentinel-key ownership, reserved.
    pub sparse_header: [i32; 4],
    /// Two (chunk X,Z) keys per WGSL vec4<i32>; linear probing, half full at most.
    pub sparse_keys: [[i32; 4]; 512],
}

// SAFETY: repr(C), only inhabited integer arrays, no references/interior
// mutability, and every bit pattern is valid. The assertions also rule out
// padding. Manual impls avoid requiring an extra bytemuck array-size feature.
unsafe impl bytemuck::Zeroable for ResidentCoverage {}
unsafe impl bytemuck::Pod for ResidentCoverage {}
const _: [(); 10320] = [(); std::mem::size_of::<ResidentCoverage>()];
const _: [(); 16] = [(); std::mem::offset_of!(ResidentCoverage, words)];
const _: [(); 2112] = [(); std::mem::offset_of!(ResidentCoverage, sparse_header)];
const _: [(); 2128] = [(); std::mem::offset_of!(ResidentCoverage, sparse_keys)];
const _: [(); 10640] = [(); std::mem::size_of::<super::CameraUniform>()];
const _: [(); 320] = [(); std::mem::offset_of!(super::CameraUniform, resident_coverage)];

impl Default for ResidentCoverage {
    fn default() -> Self {
        Self {
            header: [0; 4],
            words: [[0; 4]; 131],
            sparse_header: [0; 4],
            sparse_keys: [[i32::MIN; 4]; 512],
        }
    }
}

impl ResidentCoverage {
    fn bit_index(&self, x: i32, z: i32) -> Option<(usize, usize, u32)> {
        let width = self.header[2];
        if !(1..=MAX_SIDE).contains(&width) {
            return None;
        }
        let x = i64::from(x) - i64::from(self.header[0]);
        let z = i64::from(z) - i64::from(self.header[1]);
        if x < 0 || z < 0 || x >= i64::from(width) || z >= i64::from(width) {
            return None;
        }
        let bit = (z * i64::from(width) + x) as usize;
        let word = bit / 32;
        Some((word / 4, word % 4, 1u32 << (bit % 32)))
    }

    /// May populate bits before enabling the completed map.
    pub fn set_column(&mut self, x: i32, z: i32) -> bool {
        if !(1..=MAX_SIDE).contains(&self.header[2]) {
            return false;
        }
        if let Some((group, lane, bit)) = self.bit_index(x, z) {
            self.words[group][lane] |= bit;
            return true;
        }
        let count = self.sparse_header[0];
        if !(0..=MAX_SPARSE_COLUMNS as i32).contains(&count) {
            return false;
        }
        if count == 0 {
            // Also permits a zeroed Pod to become a valid populated map.
            self.sparse_keys.fill([i32::MIN; 4]);
        }
        if [x, z] == EMPTY_KEY {
            if self.sparse_header[1] == 1 {
                return true;
            }
            if count == MAX_SPARSE_COLUMNS as i32 {
                return false;
            }
            self.sparse_header[1] = 1;
            self.sparse_header[0] += 1;
            return true;
        }
        let mut slot = column_hash(x, z);
        for _ in 0..SPARSE_SLOTS {
            let key = self.sparse_key(slot);
            if key == [x, z] {
                return true;
            }
            if key == EMPTY_KEY {
                if count == MAX_SPARSE_COLUMNS as i32 {
                    return false;
                }
                let lane = (slot & 1) * 2;
                self.sparse_keys[slot / 2][lane] = x;
                self.sparse_keys[slot / 2][lane + 1] = z;
                self.sparse_header[0] += 1;
                return true;
            }
            slot = (slot + 1) & (SPARSE_SLOTS - 1);
        }
        false
    }

    pub fn in_dense_window(&self, x: i32, z: i32) -> bool {
        self.bit_index(x, z).is_some()
    }

    fn sparse_key(&self, slot: usize) -> [i32; 2] {
        let lane = (slot & 1) * 2;
        [
            self.sparse_keys[slot / 2][lane],
            self.sparse_keys[slot / 2][lane + 1],
        ]
    }

    fn sparse_contains(&self, x: i32, z: i32) -> bool {
        if !(1..=MAX_SPARSE_COLUMNS as i32).contains(&self.sparse_header[0]) {
            return false;
        }
        if [x, z] == EMPTY_KEY {
            return self.sparse_header[1] == 1;
        }
        let mut slot = column_hash(x, z);
        for _ in 0..SPARSE_SLOTS {
            let key = self.sparse_key(slot);
            if key == EMPTY_KEY {
                return false;
            }
            if key == [x, z] {
                return true;
            }
            slot = (slot + 1) & (SPARSE_SLOTS - 1);
        }
        false
    }

    pub fn contains_column(&self, x: i32, z: i32) -> bool {
        if self.header[3] == 0 || !(1..=MAX_SIDE).contains(&self.header[2]) {
            return false;
        }
        if let Some((group, lane, bit)) = self.bit_index(x, z) {
            return self.words[group][lane] & bit != 0;
        }
        self.sparse_contains(x, z)
    }

    /// Test all columns touched by a world-metre AABB. Nonzero extents are
    /// half-open: a high edge exactly on a chunk boundary does not own the next
    /// column. Zero-width axes are point queries, matching the shader's floor.
    pub fn contains_bounds(&self, low: Vec3, high: Vec3, scale: f32) -> bool {
        let span = 32.0 * scale;
        if self.header[3] == 0
            || !(1..=MAX_SIDE).contains(&self.header[2])
            || !low.is_finite()
            || !high.is_finite()
            || !low.cmple(high).all()
            || !span.is_finite()
            || span <= 0.0
        {
            return false;
        }
        let cells = |a: f32, b: f32| -> Option<(i32, i32)> {
            let first = (f64::from(a) / f64::from(span)).floor();
            let last = if b > a {
                (f64::from(b) / f64::from(span)).ceil() - 1.0
            } else {
                first
            };
            if first < f64::from(i32::MIN)
                || first > f64::from(i32::MAX)
                || last < f64::from(i32::MIN)
                || last > f64::from(i32::MAX)
            {
                return None;
            }
            Some((first as i32, last as i32))
        };
        let Some((x0, x1)) = cells(low.x, high.x) else {
            return false;
        };
        let Some((z0, z1)) = cells(low.z, high.z) else {
            return false;
        };
        let max_owned = i64::from(self.header[2]).pow(2)
            + i64::from(self.sparse_header[0].clamp(0, MAX_SPARSE_COLUMNS as i32));
        let nx = i64::from(x1) - i64::from(x0) + 1;
        let nz = i64::from(z1) - i64::from(z0) + 1;
        // Test factors first so extreme signed coordinate spans cannot overflow.
        if nx <= 0 || nz <= 0 || nx > max_owned || nz > max_owned || nx * nz > max_owned {
            return false;
        }
        for z in z0..=z1 {
            for x in x0..=x1 {
                if !self.contains_column(x, z) {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_layout_and_last_bit_match_vec4_packing() {
        let mut coverage = ResidentCoverage {
            header: [-64, -64, 129, 1],
            ..Default::default()
        };
        assert!(coverage.set_column(-64, -64));
        assert!(coverage.set_column(64, 64));
        assert_eq!(coverage.words[0][0], 1);
        assert_eq!(coverage.words[130][0], 1);
        assert_eq!(bytemuck::bytes_of(&coverage).len(), 10320);
        assert!(coverage.contains_column(64, 64));
        assert!(coverage.set_column(65, 64));
        assert!(coverage.contains_column(65, 64));
        assert_eq!(coverage.sparse_header[0], 1);
    }

    #[test]
    fn negative_half_open_bounds_and_voxel_scales_agree() {
        let mut coverage = ResidentCoverage {
            header: [-1, -1, 2, 1],
            ..Default::default()
        };
        coverage.set_column(-1, -1);
        for scale in [1.0, 0.5, 0.25] {
            let span = 32.0 * scale;
            assert!(coverage.contains_bounds(
                Vec3::new(-span, 0.0, -span),
                Vec3::new(0.0, 1.0, 0.0),
                scale
            ));
            assert!(!coverage.contains_bounds(Vec3::ZERO, Vec3::ZERO, scale));
        }
        assert!(!coverage.contains_bounds(
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(0.1, 1.0, 0.1),
            1.0
        ));
    }

    #[test]
    fn complete_corner_is_owned_but_an_interior_hole_is_not() {
        let mut coverage = ResidentCoverage {
            header: [-1, -1, 3, 1],
            ..Default::default()
        };
        for z in -1..=1 {
            for x in -1..=1 {
                if (x, z) != (0, 0) {
                    coverage.set_column(x, z);
                }
            }
        }
        assert!(coverage.contains_bounds(
            Vec3::new(32.0, 0.0, 32.0),
            Vec3::new(64.0, 1.0, 64.0),
            1.0
        ));
        assert!(!coverage.contains_bounds(
            Vec3::new(-32.0, 0.0, -32.0),
            Vec3::new(64.0, 1.0, 64.0),
            1.0
        ));
    }

    #[test]
    fn invalid_maps_and_coordinates_are_conservative() {
        let mut coverage = ResidentCoverage::default();
        assert!(!coverage.set_column(0, 0));
        assert!(!coverage.contains_bounds(Vec3::ZERO, Vec3::ONE, 1.0));
        coverage.header = [i32::MIN, i32::MIN, 129, 1];
        assert!(coverage.set_column(i32::MAX, i32::MAX));
        assert!(coverage.contains_column(i32::MAX, i32::MAX));
        assert!(!coverage.contains_bounds(
            Vec3::splat(f32::INFINITY),
            Vec3::splat(f32::INFINITY),
            1.0
        ));
        assert!(!coverage.contains_bounds(Vec3::ZERO, Vec3::ONE, f32::NAN));
        coverage.header = [i32::MAX, i32::MAX, 1, 1];
        assert!(coverage.set_column(i32::MAX, i32::MAX));
        assert!(!coverage.contains_bounds(Vec3::splat(1e20), Vec3::splat(1e20), 1.0));
        coverage.header[2] = 130;
        assert!(!coverage.set_column(i32::MIN, i32::MIN));
    }

    #[test]
    fn serialized_gpu_bits_match_sparse_world_owners() {
        use std::collections::HashSet;
        // Negative origins and non-power-of-two widths exercise vec4 lane and
        // row crossings. The sparse set is the ownership oracle, independent
        // of either the packed lookup or AABB helper.
        for width in [1, 9, 129] {
            let mut map = ResidentCoverage {
                header: [-71, -67, width, 1],
                ..Default::default()
            };
            let mut owners = HashSet::new();
            for z in -67..-67 + width {
                for x in -71..-71 + width {
                    if (x * 17 + z * 31).rem_euclid(7) < 3 {
                        owners.insert((x, z));
                        assert!(map.set_column(x, z));
                    }
                }
            }
            let bytes = bytemuck::bytes_of(&map);
            for z in -68..=-67 + width {
                for x in -72..=-71 + width {
                    assert_eq!(map.contains_column(x, z), owners.contains(&(x, z)));
                    if x >= -71 && x < -71 + width && z >= -67 && z < -67 + width {
                        let linear = ((z + 67) * width + x + 71) as usize;
                        // WGSL uniform words begin after the 16-byte header;
                        // vec4 lanes contain consecutive native u32 words.
                        let offset = 16 + (linear / 32) * 4;
                        let word =
                            u32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap());
                        assert_eq!(word & (1 << (linear % 32)) != 0, owners.contains(&(x, z)));
                    }
                }
            }
        }
    }

    #[test]
    fn retained_owner_does_not_expand_desired_coverage_or_alias_grid_edge() {
        let mut desired = ResidentCoverage {
            header: [-64, -64, 129, 1],
            ..Default::default()
        };
        desired.set_column(0, 0);
        let mut drawn = desired;
        assert!(drawn.set_column(64, -64));
        let low = Vec3::new(2048.0, -100.0, -2048.0);
        let high = Vec3::new(2080.0, 100.0, -2016.0);
        assert!(drawn.contains_bounds(low, high, 1.0));
        assert!(!desired.contains_bounds(low, high, 1.0));
        // A high-exclusive edit ends exactly at the final grid edge. A point
        // on that edge belongs to the next column and must not wrap to row 0.
        assert!(!drawn.contains_column(65, -64));
        assert!(!drawn.contains_bounds(high, high, 1.0));
        assert!(drawn.set_column(65, -64));
        let edge_point = Vec3::new(high.x, high.y, low.z);
        assert!(drawn.contains_bounds(edge_point, edge_point, 1.0));
        assert!(drawn.contains_bounds(low, Vec3::new(2112.0, high.y, high.z), 1.0));
        assert!(drawn.contains_column(0, 0));
    }

    #[test]
    fn full_sparse_capacity_sentinel_and_serialized_keys_are_exact() {
        use std::collections::HashSet;
        let mut map = ResidentCoverage {
            header: [-64, -64, 129, 1],
            ..Default::default()
        };
        let mut owners = HashSet::from([(i32::MIN, i32::MIN)]);
        for i in 1..512 {
            owners.insert((1000 + i * 33, -2000 - i * 37));
        }
        let mut sorted: Vec<_> = owners.iter().copied().collect();
        sorted.sort_unstable();
        for &(x, z) in &sorted {
            assert!(map.set_column(x, z));
        }
        assert_eq!(map.sparse_header, [512, 1, 0, 0]);
        for &(x, z) in &sorted {
            assert!(map.contains_column(x, z));
            assert!(map.set_column(x, z), "duplicates remain valid at capacity");
        }
        assert!(!map.set_column(777777, -777777));
        assert!(!map.contains_column(i32::MIN, i32::MIN + 1));
        assert!(!map.contains_column(777777, -777777));
        assert!(
            map.set_column(0, 0),
            "dense capacity is independent of sparse pins"
        );
        let bytes = bytemuck::bytes_of(&map);
        let mut decoded = HashSet::from([(i32::MIN, i32::MIN)]);
        for entry in bytes[2128..].chunks_exact(8) {
            let x = i32::from_ne_bytes(entry[..4].try_into().unwrap());
            let z = i32::from_ne_bytes(entry[4..].try_into().unwrap());
            if [x, z] != EMPTY_KEY {
                assert!(decoded.insert((x, z)));
            }
        }
        assert_eq!(
            decoded, owners,
            "two vec2 keys per vec4 must survive serialization"
        );
    }

    #[test]
    fn colliding_sparse_keys_never_alias_or_fall_through_dense_holes() {
        let mut map = ResidentCoverage {
            header: [-64, -64, 129, 1],
            ..Default::default()
        };
        let bucket = column_hash(1000, -3000);
        let keys: Vec<_> = (1000..2_000_000)
            .filter(|&x| column_hash(x, -3000) == bucket)
            .take(512)
            .collect();
        assert_eq!(keys.len(), 512);
        for &x in &keys {
            assert!(map.set_column(x, -3000));
        }
        for &x in &keys {
            assert!(map.contains_column(x, -3000));
        }
        assert!(!map.contains_column(0, 0));
        assert!(!map.contains_column(keys[0], -3001));
        // If a caller changes the window, the dense bit is authoritative inside
        // it even when an obsolete sparse copy of that coordinate is present.
        map.header = [keys[0], -3000, 1, 1];
        assert!(!map.contains_column(keys[0], -3000));
    }

    #[test]
    fn retained_world_columns_survive_rebasing_at_all_voxel_scales() {
        let owners = [(-600, -620), (-599, -620), (1_000_000, -1_000_000)];
        for (cx, cz) in [(0, 0), (-600, -620), (1_000_000, -1_000_000)] {
            let mut map = ResidentCoverage {
                header: [cx - 64, cz - 64, 129, 1],
                ..Default::default()
            };
            for &(x, z) in &owners {
                assert!(map.set_column(x, z));
            }
            for scale in [1.0, 0.5, 0.25] {
                let span = 32.0 * scale;
                for &(x, z) in &owners {
                    let low = Vec3::new(x as f32 * span, 0.0, z as f32 * span);
                    assert!(map.contains_bounds(low, low + Vec3::new(span, 1.0, span), scale));
                }
                assert!(map.contains_bounds(
                    Vec3::new(-600.0 * span, 0.0, -620.0 * span),
                    Vec3::new(-598.0 * span, 1.0, -619.0 * span),
                    scale
                ));
                assert!(!map.contains_bounds(
                    Vec3::new(-600.0 * span, 0.0, -620.0 * span),
                    Vec3::new(-597.0 * span, 1.0, -619.0 * span),
                    scale
                ));
                assert!(!map.contains_bounds(Vec3::splat(-1e10), Vec3::splat(1e10), scale));
            }
        }
    }

    #[test]
    fn zeroed_empty_sparse_storage_is_initialized_before_insertion() {
        use bytemuck::Zeroable;
        let mut map = ResidentCoverage::zeroed();
        map.header = [100, 100, 1, 1];
        assert!(map.set_column(-300, 500));
        assert!(map.contains_column(-300, 500));
        assert!(!map.contains_column(0, 0));
    }
}

#[cfg(test)]
#[path = "water_ownership_contract.rs"]
mod water_ownership_contract;
