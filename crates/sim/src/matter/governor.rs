//! Narrows `R_fine` when the fine pass is cut short and widens it again after quiet ticks.
use crate::{FineRegion, FineStepStats};
use glam::IVec3;

/// Fine radii in metres, widest first: the governor steps down this list before any block radius.
pub const FINE_RADII: [u32; 3] = [24, 16, 12];
/// Ticks without a cut before the radius widens one level: two seconds at 60 Hz.
pub const RECOVER_TICKS: u32 = 120;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FineGovernor {
    level: u8,
    quiet: u32,
}

impl FineGovernor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn radius(&self) -> u32 {
        FINE_RADII[self.level as usize]
    }

    pub fn region(&self, center: IVec3) -> FineRegion {
        FineRegion { center, radius: self.radius() }
    }

    /// A cut or an overflowed awake cap narrows at once; `RECOVER_TICKS` quiet ticks widen one level.
    pub fn observe(&mut self, fine: &FineStepStats) {
        if fine.truncated || fine.deferred > 0 {
            self.level = (self.level + 1).min(FINE_RADII.len() as u8 - 1);
            self.quiet = 0;
        } else if self.level > 0 {
            self.quiet += 1;
            if self.quiet >= RECOVER_TICKS {
                self.level -= 1;
                self.quiet = 0;
            }
        }
    }
}
