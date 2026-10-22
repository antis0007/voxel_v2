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

/// The randomness of one block, folded once: a positional hash of the world seed, the phase counter
/// (`2 * tick + phase`) and the block's anchor cell. No state, no order dependence.
pub fn block_base(seed: u32, phase_counter: u32, anchor: IVec3) -> u32 {
    let mut h = hash_u32(seed ^ 0x9E37_79B9);
    for v in [phase_counter, anchor.x as u32, anchor.y as u32, anchor.z as u32] {
        h = hash_u32(h ^ v);
    }
    h
}

/// One decision inside a block: a single mix of the block's base and the decision's salt.
pub fn block_hash(base: u32, salt: u32) -> u32 {
    hash_u32(base ^ salt.wrapping_mul(0x9E37_79B9))
}

/// The dice of one block: rolled only for a decision that needs one, so a block that moves water (which
/// always moves) or has nothing to slide into costs no hashing. A roll depends on the inputs alone.
pub struct Dice {
    seed: u32,
    phase_counter: u32,
    anchor: IVec3,
    base: std::cell::Cell<Option<u32>>,
}

impl Dice {
    pub fn new(seed: u32, phase_counter: u32, anchor: IVec3) -> Self {
        Self { seed, phase_counter, anchor, base: std::cell::Cell::new(None) }
    }

    /// The block's base is folded on the first roll and kept; a block that never rolls pays nothing.
    pub fn roll(&self, salt: u32) -> u32 {
        let base = self.base.get().unwrap_or_else(|| {
            let b = block_base(self.seed, self.phase_counter, self.anchor);
            self.base.set(Some(b));
            b
        });
        block_hash(base, salt)
    }
}

/// A liquid or gas moves sideways this tick when its random byte is below its `flow`; 255 always
/// moves, without rolling. Pending (suppressed) moves keep a brick awake.
pub fn flow_gate(random: impl FnOnce() -> u32, flow: u8) -> bool {
    flow == u8::MAX || ((random() & 0xFF) as u8) < flow
}

/// Energy that crosses an edge per 0.1 K of difference and per 0.01 W/(m·K) of conductivity, per tick of
/// simulated time, before the cap that stops two cells passing each other. A tuning constant for how fast
/// heat spreads. A heat step carries `HEAT_TICK_INTERVAL` ticks' worth at once.
pub const HEAT_RATE: u32 = 4096;

/// Heat conducts every this many ticks, not every tick: temperature changes slowly next to matter
/// moving, so it needs a coarser step (0.32 s of simulated time at 1 m cells). Each heat step moves the
/// energy of the ticks it skipped, so the rate in simulated time is unchanged.
pub const HEAT_TICK_INTERVAL: u32 = 4;

/// Whether `tick` is one that conducts heat.
pub fn is_heat_tick(tick: u32) -> bool {
    tick.is_multiple_of(HEAT_TICK_INTERVAL)
}

/// Whether a phase counter (`2 * tick + phase`) belongs to a heat tick.
pub fn is_heat_phase(phase_counter: u32) -> bool {
    is_heat_tick(phase_counter / 2)
}

/// Temperature differences below this (0.1 K) do not conduct: the deadband that lets a thermal field
/// go idle instead of jittering by one step for ever.
pub const HEAT_DEADBAND: u32 = 2;

/// Chance per heat tick that a burning cell is spent, out of 2³²: one in `fuel`, so `fuel` is the
/// expected burn time in heat ticks. A fuel of 0 or 1 is spent at once.
pub fn burn_rate(fuel: u16) -> u32 {
    if fuel <= 1 { u32::MAX } else { u32::MAX / fuel as u32 }
}

/// Dice salts of the reaction pass, after the twelve conduction edges (16..40): burning per cell, the
/// single-cell row per cell, then the twelve pair edges.
pub const SALT_BURN: u32 = 48;
pub const SALT_UNARY: u32 = 56;
pub const SALT_PAIR: u32 = 64;

/// `floor(a × b / c)` and the remainder, exact; the quotient must fit `u32`, which the heat rule
/// guarantees because the energy it moves never exceeds half a cell's. The product needs 64 bits; the GPU port builds it from
/// 32-bit halves, and everything else in the heat rule stays in `u32`.
pub fn mul_div_rem(a: u32, b: u32, c: u32) -> (u32, u32) {
    let product = a as u64 * b as u64;
    debug_assert!(product / (c as u64) <= u32::MAX as u64, "quotient must fit u32");
    ((product / c as u64) as u32, (product % c as u64) as u32)
}

/// The high 32 bits of `a × b`: with a uniform `a`, the test `mulhi(a, c) < r` is true with probability
/// `r / c`, which is how a fractional temperature step is rounded without bias.
pub fn mulhi(a: u32, b: u32) -> u32 {
    ((a as u64 * b as u64) >> 32) as u32
}
