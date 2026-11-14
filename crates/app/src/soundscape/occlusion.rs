//! How much solid ground or wall sits on the straight path from a source to the listener.
use engine_content::BlockId;
use glam::{IVec3, Vec3};

/// Spacing of the path samples, metres.
const STEP_M: f32 = 0.5;
/// Samples per path; a 64 m reach at half-metre spacing.
const MAX_SAMPLES: usize = 128;
/// Solid thickness that blocks 63% of the sound, metres.
const SOFT_M: f32 = 1.2;

/// Blocked share of the path, 0 clear to 1 sealed; the source's own cell is not counted.
pub fn between(get: impl Fn(IVec3) -> BlockId, listener_m: Vec3, source_m: Vec3, meters: f32) -> f32 {
    let delta = source_m - listener_m;
    let length = delta.length();
    if !length.is_finite() || length < STEP_M || meters <= 0.0 {
        return 0.0;
    }
    let cell = |p: Vec3| (p / meters).floor().as_ivec3();
    let (start, end) = (cell(listener_m), cell(source_m));
    let samples = ((length / STEP_M) as usize).min(MAX_SAMPLES);
    let mut solid = 0usize;
    let mut last = start;
    for k in 1..samples {
        let here = cell(listener_m + delta * (k as f32 / samples as f32));
        if here == last || here == end {
            continue;
        }
        last = here;
        solid += usize::from(get(here).is_opaque());
    }
    // A cell wider than a sample is counted once, so thickness is cells times their size.
    let thickness = solid as f32 * meters.max(STEP_M.min(meters));
    1.0 - (-thickness / SOFT_M).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(x: i32) -> impl Fn(IVec3) -> BlockId {
        move |p| if p.x == x { BlockId::Stone } else { BlockId::Air }
    }

    #[test]
    fn open_air_is_clear_and_a_wall_blocks_part() {
        let (from, to) = (Vec3::new(0.5, 1.5, 0.5), Vec3::new(10.5, 1.5, 0.5));
        assert_eq!(between(|_| BlockId::Air, from, to, 1.0), 0.0);
        let one = between(wall(5), from, to, 1.0);
        assert!((0.3..0.7).contains(&one), "one wall {one}");
    }

    #[test]
    fn thicker_walls_block_more_and_the_source_cell_is_free() {
        let (from, to) = (Vec3::new(0.5, 1.5, 0.5), Vec3::new(10.5, 1.5, 0.5));
        let thick = between(|p| if (4..8).contains(&p.x) { BlockId::Stone } else { BlockId::Air }, from, to, 1.0);
        assert!(thick > 0.9, "four cells {thick}");
        assert_eq!(between(wall(10), from, to, 1.0), 0.0, "a block being broken does not muffle itself");
    }

    #[test]
    fn degenerate_paths_are_clear() {
        let p = Vec3::splat(3.0);
        assert_eq!(between(|_| BlockId::Stone, p, p, 1.0), 0.0);
        assert_eq!(between(|_| BlockId::Stone, p, Vec3::splat(f32::NAN), 1.0), 0.0);
    }
}
