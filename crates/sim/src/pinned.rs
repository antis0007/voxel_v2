//! Everything that fixes what the cellular automaton computes, in one place.
//!
//! Changing anything here changes every world's evolution, so the canonical
//! scenario hash in `scenario_tests` fails until the new value is recorded
//! on purpose (docs/adr/0008-ca-determinism.md). The GPU kernels port exactly
//! these definitions, and only `u32` arithmetic, so CPU and GPU agree bit for bit.
use engine_core::hash::hash_u32;
use glam::IVec3;

/// Block anchors of the two Margolus phases. A tick runs both, in this order;
/// block `i` of a phase covers `anchor + {0,1}^3` for anchors `offset + 2k`.
pub const PHASE_OFFSETS: [IVec3; 2] = [IVec3::new(0, 0, 0), IVec3::new(1, 1, 1)];

/// The randomness of one block: a positional hash of the world seed, the
/// phase counter (`2 * tick + phase`), the block's anchor cell and a salt for
/// each decision inside the block. No state, no order dependence.
pub fn block_hash(seed: u32, phase_counter: u32, anchor: IVec3, salt: u32) -> u32 {
    let mut h = hash_u32(seed ^ 0x9E37_79B9);
    for v in [phase_counter, anchor.x as u32, anchor.y as u32, anchor.z as u32, salt] {
        h = hash_u32(h ^ v);
    }
    h
}

/// The dice of one block: rolled only for a decision that needs one, so a block that moves water (which
/// always moves) or has nothing to slide into costs no hashing. A roll depends on the inputs alone.
#[derive(Clone, Copy)]
pub struct Dice {
    pub seed: u32,
    pub phase_counter: u32,
    pub anchor: IVec3,
}

impl Dice {
    pub fn roll(&self, salt: u32) -> u32 {
        block_hash(self.seed, self.phase_counter, self.anchor, salt)
    }
}

/// A liquid or gas moves sideways this tick when its random byte is below its `flow`; 255 always
/// moves, without rolling. Pending (suppressed) moves keep a brick awake.
pub fn flow_gate(random: impl FnOnce() -> u32, flow: u8) -> bool {
    flow == u8::MAX || ((random() & 0xFF) as u8) < flow
}
