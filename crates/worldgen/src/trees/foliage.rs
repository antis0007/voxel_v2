//! Generator 130 crown detail: clumped leaf lobes and leaf relief bands that
//! switch on only where the voxel grid can resolve them. The physical crown is
//! the same at every scale; finer grids reveal more of its surface.
use crate::value_noise3;
use glam::Vec3;

/// Dormant gate; `docs/worldgen/GENERATOR_GATES.md` row 130.
pub const VERSION: u32 = 130;

/// Broad threshold band as in generator 52 (`0.84 + edge * 0.22`), but from
/// continuous noise: hashed metre cubes left flat axis-aligned facets on fine grids.
const CORE: f32 = 0.84;
const BROAD: (f32, f32) = (1.6, 0.22);
/// (wavelength, amplitude) in metres: signed displacement of the lobe surface,
/// so a band carves pockets as deep as the tufts it raises. Absolute metres
/// keep large lobes as rough as small ones.
const BANDS: [(f32, f32); 2] = [(1.1, 0.30), (0.7, 0.30)];
/// Farthest a relief band moves the surface, metres.
pub(super) const OUTSET: f32 = 0.60;

/// Per-tree leaf relief, sampled in tree-local metres so planet-scale anchors
/// never reach the noise lattice as large f32 values.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Relief {
    pub seed: u32,
}

/// Gain of a band of `wavelength` metres on a `scale` metre grid: zero below
/// 1.5 voxels per wavelength, full from 2.8.
fn resolved(wavelength: f32, scale: f32) -> f32 {
    let t = ((wavelength / scale - 1.5) / 1.3).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn noise(q: Vec3, wavelength: f32, seed: u32) -> f32 {
    let p = q / wavelength;
    value_noise3(p.x, p.y, p.z, seed)
}

impl Relief {
    /// True when `field` (squared normalised distance to the nearest lobe,
    /// whose smallest radius is `size` metres) is leaf at `q`.
    pub fn leaf(self, q: Vec3, scale: f32, field: f32, size: f32) -> bool {
        let s = field.sqrt();
        if (s - CORE.sqrt()) * size < -OUTSET {
            return true;
        }
        if (s - (CORE + BROAD.1).sqrt()) * size >= OUTSET {
            return false;
        }
        let surface = (CORE + BROAD.1 * noise(q, BROAD.0, self.seed)).sqrt();
        let mut lift = 0.0;
        for (i, (wavelength, amplitude)) in BANDS.into_iter().enumerate() {
            let gain = resolved(wavelength, scale);
            if gain > 0.0 {
                let n = noise(q, wavelength, self.seed.wrapping_add(7919 * (i as u32 + 1)));
                lift += amplitude * gain * (2.0 * n - 1.0);
            }
        }
        (s - surface) * size < lift
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_never_exceed_the_outset() {
        assert!(BANDS.iter().map(|b| b.1).sum::<f32>() <= OUTSET);
    }

    #[test]
    fn only_resolvable_bands_switch_on() {
        assert_eq!(resolved(0.7, 1.0), 0.0);
        assert_eq!(resolved(1.1, 1.0), 0.0);
        assert_eq!(resolved(0.7, 0.5), 0.0);
        assert!(resolved(1.1, 0.5) > 0.3);
        assert_eq!(resolved(0.7, 0.25), 1.0);
        assert_eq!(resolved(1.1, 0.25), 1.0);
    }

    #[test]
    fn one_metre_relief_is_the_broad_band_alone() {
        let relief = Relief { seed: 99 };
        for i in 0..400 {
            let q = Vec3::new(i as f32 * 0.37, (i % 17) as f32 * 0.61, (i % 23) as f32 * 0.29);
            let field = 0.6 + (i % 50) as f32 * 0.016;
            let broad = CORE + BROAD.1 * noise(q, BROAD.0, 99);
            assert_eq!(relief.leaf(q, 1.0, field, 2.0), field < broad, "q={q}");
        }
    }
}
