//! Seed shifts the hazards inside the fixed precinct. Routes stay where they are.
use crate::{hash_u32, WorldSampleConfig, SEEDED_REALMS};

#[derive(Clone, Copy)]
pub struct Plan {
    pub lava_south: f32,
    pub lava_north: f32,
    pub water_a0: f32,
    pub water_a1: f32,
    pub water_b0: f32,
    pub water_b1: f32,
    pub ring: f32,
    pub cistern: f32,
    pub pillars: [(f32, f32); 4],
}

pub fn classic() -> Plan {
    Plan {
        lava_south: 20.0,
        lava_north: 40.0,
        water_a0: 22.0,
        water_a1: 24.5,
        water_b0: 35.5,
        water_b1: 38.0,
        ring: 6.0,
        cistern: 32.0,
        pillars: [(-4.0, 66.0), (4.0, 66.0), (-4.0, 74.0), (4.0, 74.0)],
    }
}

pub fn of(cfg: &WorldSampleConfig) -> Plan {
    if cfg.generator_version < SEEDED_REALMS {
        return classic();
    }
    let h = hash_u32(cfg.seed ^ 0xC411_5EED);
    let n = |shift: u32, span: u32| (h.rotate_left(shift) % span) as f32;
    let mut pillars = classic().pillars;
    for (i, pillar) in pillars.iter_mut().enumerate() {
        let j = n(i as u32 * 5, 5) - 2.0;
        let k = n(i as u32 * 5 + 3, 5) - 2.0;
        pillar.0 += j * 0.45;
        pillar.1 += k * 0.45;
    }
    Plan {
        lava_south: 18.0 + n(1, 3),
        lava_north: 40.0 + n(4, 3),
        water_a0: 21.2 + n(8, 3) * 0.4,
        water_a1: 24.2 + n(10, 3) * 0.35,
        water_b0: 34.6 + n(12, 3) * 0.4,
        water_b1: 37.6 + n(14, 3) * 0.35,
        ring: 5.2 + n(16, 4) * 0.35,
        cistern: 31.0 + n(20, 4) * 0.8,
        pillars,
    }
}
