//! A rock column in body coordinates. Water, talus, and exposed blocks
//! read the same contacts even when a canyon crosses a solve tile.
use engine_content::BlockId;
use glam::DVec3;

use super::{SEA, noise, smooth};

#[derive(Clone, Copy)]
pub(super) struct Column {
    datum: f64,
    stretch: f64,
    coal: f64,
    beds: f64,
    carbonate: f64,
}

const CONTACTS: [f64; 9] = [-60.0, -24.0, 5.0, 31.0, 57.0, 91.0, 137.0, 191.0, 263.0];
const STRENGTH: [f64; 10] = [1.26, 0.78, 1.66, 0.69, 1.90, 0.75, 1.70, 0.80, 1.46, 1.18];

impl Column {
    pub fn new(seed: u32, body: DVec3, collision: f64, carbonate: f64, rift: f64, offset: f64) -> Self {
        let broad_dip = (noise(seed, body / 4100.0, 279_011) - 0.5) * 156.0;
        let folded = (noise(seed, body / 1280.0, 279_023) - 0.5) * (10.0 + 44.0 * collision);
        Self {
            datum: SEA + 10.0 + broad_dip + folded + offset,
            stretch: 0.72 + 0.54 * noise(seed, body / 3600.0, 279_031),
            coal: 1.7 * smooth(0.44, 0.67, noise(seed, body / 1700.0, 279_039)),
            beds: (1.0 - 0.55 * smooth(0.30, 0.80, collision))
                * (1.0 - smooth(0.10, 0.27, rift)),
            carbonate,
        }
    }

    fn layer(self, height: f64) -> (usize, f64) {
        let depth = (height - self.datum) / self.stretch;
        (CONTACTS.partition_point(|&contact| depth >= contact), depth)
    }

    pub fn resistance(self, height: f64) -> f64 {
        let (layer, depth) = self.layer(height);
        let mut strength = STRENGTH[layer];
        if layer > 0 {
            let contact = CONTACTS[layer - 1];
            strength += 0.5 * (STRENGTH[layer - 1] - strength) * (1.0 - smooth(contact, contact + 2.5, depth));
        }
        if let Some(&contact) = CONTACTS.get(layer) {
            strength += 0.5 * (STRENGTH[layer + 1] - strength) * smooth(contact - 2.5, contact, depth);
        }
        1.0 + (strength - 1.0) * self.beds
    }

    pub fn block(self, height: f64) -> BlockId {
        let (layer, depth) = self.layer(height);
        if self.beds < 0.40 { return BlockId::Stone; }
        if self.carbonate < 0.58 && (depth - 50.0).abs() < self.coal {
            return BlockId::CoalOre;
        }
        match layer {
            2 | 4 | 6 | 8 if self.carbonate > 0.48 => BlockId::Limestone,
            1 | 3 | 5 | 7 | 9 => BlockId::Sandstone,
            _ => BlockId::Stone,
        }
    }
}
