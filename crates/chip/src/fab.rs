//! Where a packaged part came from. Only prepared blanks exist in this slice;
//! lithographic fabrication (masks, wafer lots, binning) is the next phase and
//! is named here so packages and saves already have a place for it.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provenance {
    /// A prepared blank die personalised by its top metal: the first route.
    Prepared,
    /// Not produced yet: a die from a wafer lot through a reusable mask set.
    Fabricated { mask_set: u64, lot: u32, die: u16 },
}

/// Mask layers a lithographic route will need per revision. Unused until
/// the fabrication phase; the prepared route patterns metal and vias only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaskLayer {
    Diffusion,
    Gate,
    Contact,
    Metal(u8),
    Via(u8),
}

/// Masks a revision's layout needs on a prepared blank: its metal layers and
/// the via layers between them.
pub fn prepared_masks(layers: u8) -> Vec<MaskLayer> {
    (0..layers)
        .map(MaskLayer::Metal)
        .chain((1..layers).map(MaskLayer::Via))
        .collect()
}
