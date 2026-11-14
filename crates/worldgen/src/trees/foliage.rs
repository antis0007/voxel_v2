//! Generator 130 leaf surfaces shared by every tree crown: continuous broad
//! breakup plus relief bands that switch on only where the voxel grid can
//! resolve them. The physical crown is the same at every scale; finer grids
//! reveal more of its surface. Skeleton crowns also split lobes into clumps.
use super::{TreeProfile, WorldSampleConfig, hash2};
use crate::value_noise3;
use glam::Vec3;

/// Original authoring gate; also released in tree-only generator 127.
pub const VERSION: u32 = 130;

pub(super) fn enabled(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version == engine_voxel::TREE_DETAIL_GENERATOR || cfg.generator_version >= VERSION
}

/// Continuous noise for the broad threshold band: hashed metre cubes left flat
/// axis-aligned facets on fine grids.
const BROAD_WAVELENGTH: f32 = 1.6;
/// (wavelength, amplitude) in metres: signed displacement of the leaf surface,
/// so a band carves pockets as deep as the tufts it raises. Absolute metres
/// keep large lobes as rough as small ones.
const BANDS: [(f32, f32); 2] = [(1.1, 0.30), (0.7, 0.30)];
/// Farthest a relief band moves the surface on any grid, metres.
pub(super) const OUTSET: f32 = 0.60;

/// How one kind of foliage meets the air.
#[derive(Clone, Copy)]
pub(super) struct Surface {
    /// Ellipsoid field below which every voxel is leaf.
    pub core: f32,
    /// Field span the broad band adds on top of `core`.
    pub span: f32,
    /// Fraction of band amplitude: needle sprays and fronds are thinner than leaf clusters.
    pub depth: f32,
}

/// Skeleton lobes, generator 52: `0.84 + edge * 0.22`.
pub(super) const CLUSTER: Surface = Surface { core: 0.84, span: 0.22, depth: 1.0 };
/// Open-crown, umbrella and weeping lobes: `0.83 + edge * 0.24`.
pub(super) const OPEN: Surface = Surface { core: 0.83, span: 0.24, depth: 1.0 };
/// Conifer pads, whorls and tiers.
pub(super) const NEEDLES: Surface = Surface { core: 1.0, span: 0.0, depth: 0.6 };
/// Cycad fronds.
pub(super) const FRONDS: Surface = Surface { core: 1.0, span: 0.0, depth: 0.35 };

/// Per-tree leaf relief, sampled in tree-local metres so planet-scale anchors
/// never reach the noise lattice as large f32 values.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Relief {
    seed: u32,
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

/// Farthest the resolvable bands move a full-depth surface on a `scale` grid.
pub(super) fn reach(scale: f32) -> f32 {
    BANDS.iter().map(|&(wavelength, amplitude)| amplitude * resolved(wavelength, scale)).sum()
}

/// Bound padding for crowns sampled through `Relief`: zero before generator 130
/// and on grids too coarse for any band.
pub(super) fn pad(cfg: &WorldSampleConfig) -> f32 {
    if enabled(cfg) { reach(cfg.voxel_meters()) } else { 0.0 }
}

impl Relief {
    /// This tree's relief from generator 130 on.
    pub fn of(cfg: &WorldSampleConfig, tree: &TreeProfile) -> Option<Self> {
        if !enabled(cfg) {
            return None;
        }
        let cell = (hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(130_011)) * 65535.0) as u32;
        Some(Self { seed: cfg.seed.wrapping_add(130_007) ^ cell.wrapping_mul(2_654_435_761) })
    }

    /// Signed surface displacement at `q`. Pockets stop one voxel short of the
    /// middle of a part `half` metres thick, so thin strands and tips never
    /// break into floating pieces.
    fn lift(self, q: Vec3, scale: f32, depth: f32, half: f32) -> f32 {
        let mut lift = 0.0;
        for (i, (wavelength, amplitude)) in BANDS.into_iter().enumerate() {
            let gain = resolved(wavelength, scale);
            if gain > 0.0 {
                let n = noise(q, wavelength, self.seed.wrapping_add(7919 * (i as u32 + 1)));
                lift += amplitude * depth * gain * (2.0 * n - 1.0);
            }
        }
        lift.max(-(half - scale).max(0.0))
    }

    /// True when `field` (squared normalised distance to the nearest lobe,
    /// whose smallest radius is `size` metres) is leaf at `q`.
    pub fn leaf(self, q: Vec3, scale: f32, field: f32, size: f32, surface: Surface) -> bool {
        let s = field.sqrt();
        let max = reach(scale) * surface.depth;
        if (s - surface.core.sqrt()) * size < -max {
            return true;
        }
        if (s - (surface.core + surface.span).sqrt()) * size >= max {
            return false;
        }
        let broad = if surface.span > 0.0 { noise(q, BROAD_WAVELENGTH, self.seed) } else { 0.0 };
        let edge = (surface.core + surface.span * broad).sqrt();
        (s - edge) * size < self.lift(q, scale, surface.depth, size)
    }

    /// True when `distance` (signed metres outside a smooth envelope around a
    /// part `half` metres thick) is leaf at `q`.
    pub fn shell(self, q: Vec3, scale: f32, distance: f32, half: f32, surface: Surface) -> bool {
        let max = reach(scale) * surface.depth;
        if distance < -max {
            return true;
        }
        if distance >= max {
            return false;
        }
        distance < self.lift(q, scale, surface.depth, half)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_never_exceed_the_outset() {
        assert!(BANDS.iter().map(|b| b.1).sum::<f32>() <= OUTSET);
        assert!(reach(0.25) <= OUTSET);
        assert_eq!(reach(1.0), 0.0);
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
            let broad = CLUSTER.core + CLUSTER.span * noise(q, BROAD_WAVELENGTH, 99);
            assert_eq!(relief.leaf(q, 1.0, field, 2.0, CLUSTER), field < broad, "q={q}");
            let d = (i % 40) as f32 * 0.05 - 1.0;
            assert_eq!(relief.shell(q, 1.0, d, 2.0, NEEDLES), d < 0.0, "d={d}");
        }
    }

    #[test]
    fn thin_foliage_moves_less_and_keeps_a_voxel() {
        let relief = Relief { seed: 7 };
        for i in 0..400 {
            let q = Vec3::new(i as f32 * 0.13, (i % 11) as f32 * 0.29, (i % 7) as f32 * 0.41);
            assert!(relief.lift(q, 0.25, FRONDS.depth, 2.0).abs() <= reach(0.25) * FRONDS.depth + 1e-6);
            assert!(relief.lift(q, 0.25, 1.0, 0.35) >= -0.10 - 1e-6);
            assert!(relief.lift(q, 0.25, 1.0, 0.2) >= 0.0);
        }
    }
}
