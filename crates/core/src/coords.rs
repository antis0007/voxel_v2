use glam::IVec3;

/// Cells per chunk edge.
pub const CHUNK_SIZE: i32 = 32;
const CHUNK_SHIFT: u32 = CHUNK_SIZE.trailing_zeros();
const _: () = assert!(CHUNK_SIZE > 0 && CHUNK_SIZE == 1 << CHUNK_SHIFT);
/// Chunk plus a one-voxel halo on every side, as meshing reads it.
pub const PADDED_CHUNK_SIZE: i32 = CHUNK_SIZE + 2;

/// Floor division: `-1 / 32 == -1`, unlike Rust's truncating `/`.
pub fn floor_div(a: i32, b: i32) -> i32 {
    let mut q = a / b;
    let r = a % b;
    if r != 0 && ((r > 0) != (b > 0)) {
        q -= 1;
    }
    q
}

/// Signed right shift gives floor division even west or below the origin.
#[inline]
pub fn chunk_axis_from_world_block(block: i32) -> i32 {
    block >> CHUNK_SHIFT
}

#[inline]
pub fn chunk_coord_from_world_block(p: IVec3) -> IVec3 {
    IVec3::new(
        chunk_axis_from_world_block(p.x),
        chunk_axis_from_world_block(p.y),
        chunk_axis_from_world_block(p.z),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_cells_round_toward_negative_infinity() {
        assert_eq!(floor_div(-1, 32), -1);
        assert_eq!(floor_div(-32, 32), -1);
        assert_eq!(floor_div(-33, 32), -2);
        assert_eq!(floor_div(31, 32), 0);
        assert_eq!(chunk_coord_from_world_block(IVec3::new(-1, 32, 63)), IVec3::new(-1, 1, 1));
    }
}
