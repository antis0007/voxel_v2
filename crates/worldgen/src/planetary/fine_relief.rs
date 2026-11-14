//! Generator 295 fine relief between the solved surface and the voxel.
//!
//! The history solve resolves ranges, valleys and drainage at a few hundred
//! metres. Generators from 280 on then add only a 7 m, half-metre ripple, so
//! ground between rivers reads as a flat sheet. This module adds the missing
//! decametre forms from the rock and cover the solve already knows:
//! rolling ground everywhere, knolls and tors on hard rock, kettle hollows on
//! formerly glaciated and thickly covered ground, skerries, stacks and coves
//! on rocky shores, and dune ridges behind sandy beaches.
//!
//! All wavelengths are body metres. Octaves finer than the sample footprint
//! are filtered so the horizon and the voxel columns agree.
use super::{SEA, noise, smooth};
use glam::DVec3;

/// Rock and cover at the sample point, from the solve.
pub(super) struct Ground {
    pub collision: f64,
    pub rift: f64,
    pub former_ice: f64,
    /// Mobile cover thickness, metres.
    pub sediment_m: f64,
    /// Solved surface slope, m/m.
    pub slope: f64,
    pub wet: f64,
}

impl Ground {
    /// Resistant bedrock near the surface, 0..1.
    fn hard(&self) -> f64 {
        (self.collision.clamp(0.0, 1.0) * 0.8 + self.rift.clamp(0.0, 1.0) * 0.5
            + self.former_ice.clamp(0.0, 1.0) * 0.4).min(1.0)
    }
    /// Thick loose cover, 0..1.
    fn soft(&self) -> f64 { smooth(4.0, 30.0, self.sediment_m) }
}

fn filtered(wavelength: f64, footprint: f64) -> f64 {
    1.0 - smooth(wavelength * 0.25, wavelength * 2.0, footprint)
}

/// Ridge lines of one noise octave, 0..1, sharpened by `power`.
fn ridge(seed: u32, p: DVec3, wavelength: f64, salt: u32, power: i32) -> f64 {
    (1.0 - (noise(seed, p / wavelength, salt) * 2.0 - 1.0).abs()).powi(power)
}

/// Height change on dry land, metres. The caller fades it at rivers and the sea.
pub(super) fn upland(seed: u32, p: DVec3, footprint: f64, g: &Ground) -> f64 {
    let f = |w: f64| filtered(w, footprint);
    if f(96.0) <= 0.0 { return 0.0; }
    let (hard, soft) = (g.hard(), g.soft());
    let steep = smooth(0.04, 0.35, g.slope);
    let roll = (noise(seed, p / 96.0, 295_001) - 0.5) * f(96.0)
        + (noise(seed, p / 41.0, 295_003) - 0.5) * 0.55 * f(41.0)
        + (noise(seed, p / 17.0, 295_007) - 0.5) * 0.30 * f(17.0);
    let roll = roll * (3.0 + 4.5 * steep + 3.0 * hard - 1.2 * soft);
    let knoll = ridge(seed, p, 58.0, 295_011, 3) * f(58.0)
        * (1.5 + 6.0 * hard + 3.0 * steep) * (1.0 - 0.7 * soft);
    // Tors stand in clusters, on hard rock only.
    let cluster = smooth(0.55, 0.80, noise(seed, p / 130.0, 295_013));
    let tor = ridge(seed, p, 21.0, 295_017, 3) * cluster * hard * 4.0 * f(21.0);
    let dell = smooth(0.72, 0.90, noise(seed, p / 74.0, 295_019)) * f(74.0)
        * (0.8 + 1.6 * g.former_ice.clamp(0.0, 1.0) + 0.6 * soft);
    roll + knoll + tor - dell
}

/// Height change in the shore band for solved height `h`, metres.
pub(super) fn shore(seed: u32, p: DVec3, footprint: f64, h: f64, g: &Ground) -> f64 {
    let (rock, dune, _) = shore_forms(seed, p, footprint, h, g);
    rock + dune
}

/// The shore band's rock forms, its dune ridges, and the dune weight alone
/// (0..1) for a generator that lays its own dune field (300).
pub(super) fn shore_forms(seed: u32, p: DVec3, footprint: f64, h: f64, g: &Ground) -> (f64, f64, f64) {
    let near = 1.0 - smooth(3.0, 14.0, (h - SEA).abs());
    let back = smooth(SEA + 1.2, SEA + 3.5, h) * (1.0 - smooth(SEA + 9.0, SEA + 16.0, h));
    if near <= 0.0 && back <= 0.0 { return (0.0, 0.0, 0.0); }
    let f = |w: f64| filtered(w, footprint);
    let sandy = smooth(3.0, 14.0, g.sediment_m);
    let rocky = (1.0 - sandy) * (0.45 + 0.55 * g.hard() + smooth(0.08, 0.40, g.slope)).min(1.0);
    let (mut rock, mut dunes, mut dune_weight) = (0.0, 0.0, 0.0);
    if near > 0.0 && rocky > 0.0 {
        // Stacks and skerries lift the shallows; coves let the sea in.
        let stack = ridge(seed, p, 34.0, 295_031, 4) * 5.5 * f(34.0)
            + ridge(seed, p, 13.0, 295_037, 4) * 2.5 * f(13.0);
        let cove = smooth(0.60, 0.85, noise(seed, p / 47.0, 295_041)) * 3.0 * f(47.0);
        rock = (stack - cove) * rocky * near;
    }
    if back > 0.0 && sandy > 0.0 {
        let dune = ridge(seed, p, 27.0, 295_043, 2) * 3.2 * f(27.0)
            + ridge(seed, p, 11.0, 295_047, 2) * 0.8 * f(11.0);
        dune_weight = back * sandy * (1.0 - 0.6 * g.wet.clamp(0.0, 1.0));
        dunes = dune * dune_weight;
    }
    (rock, dunes, dune_weight)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ground(collision: f64, sediment_m: f64, slope: f64) -> Ground {
        Ground { collision, rift: 0.0, former_ice: 0.0, sediment_m, slope, wet: 0.5 }
    }

    #[test]
    fn relief_is_bounded_and_filtered_out_for_coarse_samples() {
        let (plain, crag) = (ground(0.0, 12.0, 0.01), ground(1.0, 0.0, 0.4));
        let (mut low, mut high) = (f64::MAX, f64::MIN);
        for i in 0..4000 {
            let p = DVec3::new(30_600.0, f64::from(i) * 3.7, f64::from(i % 61) * 11.3);
            for g in [&plain, &crag] {
                let d = upland(7, p, 1.0, g);
                low = low.min(d);
                high = high.max(d);
                assert_eq!(upland(7, p, 400.0, g), 0.0);
                assert_eq!(shore(7, p, 400.0, SEA, g), 0.0);
            }
        }
        assert!(low > -12.0 && high < 28.0, "{low} {high}");
        assert!(high - low > 6.0, "relief too flat: {low} {high}");
    }

    #[test]
    fn shore_forms_stay_in_their_band() {
        let p = DVec3::new(30_600.0, 211.0, -87.0);
        assert_eq!(shore(7, p, 1.0, SEA + 40.0, &ground(1.0, 0.0, 0.3)), 0.0);
        assert_eq!(shore(7, p, 1.0, SEA - 40.0, &ground(1.0, 0.0, 0.3)), 0.0);
        // Dunes only raise sand; they never cut below the beach.
        for i in 0..500 {
            let q = p + DVec3::new(0.0, f64::from(i) * 2.3, f64::from(i) * 1.1);
            assert!(shore(7, q, 1.0, SEA + 5.0, &ground(0.0, 40.0, 0.02)) >= 0.0);
        }
    }
}
