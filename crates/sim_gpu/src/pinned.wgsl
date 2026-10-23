// The cellular block rule on the GPU: a line-for-line port of engine_sim's `rules.rs`, `react.rs` and
// `pinned.rs`, in u32 arithmetic only, so every block comes out byte-identical to the CPU reference
// (docs/adr/0008-ca-determinism.md). An entry shader (offload.wgsl, resident.wgsl) declares `rules` at
// group 0 binding 1, fills the private `cells` and calls `apply_rule`; blocks of a phase are disjoint.
// Keep the order of every check and every dice salt exactly as the Rust; `ca_equivalence` compares bytes.

const MAT_BASE: u32 = 0u;
const REACTIVE_BASE: u32 = 2048u;
const UNARY_BASE: u32 = 2056u;
const PAIR_BASE: u32 = 2120u;
const ROW_BASE: u32 = 18504u;

const VOID: u32 = 0u;
const SOLID: u32 = 1u;
const POWDER: u32 = 2u;
const LIQUID: u32 = 3u;
const GAS: u32 = 4u;

const VOLUME_FULL: u32 = 64u;
const AMBIENT: u32 = 2932u;
const NONE: u32 = 255u;
const ALWAYS: u32 = 0xFFFFFFFFu;

const HEAT_RATE: u32 = 4096u;
const HEAT_TICK_INTERVAL: u32 = 4u;
const HEAT_DEADBAND: u32 = 2u;
const SALT_BURN: u32 = 48u;
const SALT_UNARY: u32 = 56u;
const SALT_PAIR: u32 = 64u;

struct Cell { m: u32, v: u32, t: u32, p: u32, q: u32 }

var<private> cells: array<Cell, 8>;
var<private> pending: bool;
var<private> dice_base: u32;

fn empty_cell() -> Cell {
    return Cell(0u, 0u, AMBIENT, 0u, 0u);
}

// ---- pinned.rs ----

fn hash_u32(x0: u32) -> u32 {
    var x = x0;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    return x ^ (x >> 16u);
}

fn block_base(seed: u32, phase_counter: u32, anchor: vec3<i32>) -> u32 {
    var h = hash_u32(seed ^ 0x9E3779B9u);
    h = hash_u32(h ^ phase_counter);
    h = hash_u32(h ^ bitcast<u32>(anchor.x));
    h = hash_u32(h ^ bitcast<u32>(anchor.y));
    h = hash_u32(h ^ bitcast<u32>(anchor.z));
    return h;
}

fn roll(salt: u32) -> u32 {
    return hash_u32(dice_base ^ (salt * 0x9E3779B9u));
}

fn is_heat_phase(phase_counter: u32) -> bool {
    return ((phase_counter / 2u) % HEAT_TICK_INTERVAL) == 0u;
}

// The 64-bit product of two u32, as (low, high).
fn mul64(a: u32, b: u32) -> vec2<u32> {
    let a0 = a & 0xFFFFu;
    let a1 = a >> 16u;
    let b0 = b & 0xFFFFu;
    let b1 = b >> 16u;
    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;
    let mid = (p00 >> 16u) + (p01 & 0xFFFFu) + (p10 & 0xFFFFu);
    let lo = (p00 & 0xFFFFu) | (mid << 16u);
    let hi = p11 + (p01 >> 16u) + (p10 >> 16u) + (mid >> 16u);
    return vec2<u32>(lo, hi);
}

fn mulhi(a: u32, b: u32) -> u32 {
    return mul64(a, b).y;
}

// floor(a * b / c) and the remainder; the quotient fits u32 (the Rust asserts it).
fn mul_div_rem(a: u32, b: u32, c: u32) -> vec2<u32> {
    let p = mul64(a, b);
    if (p.y == 0u) {
        return vec2<u32>(p.x / c, p.x % c);
    }
    var rem = 0u;
    var q = 0u;
    for (var i = 63i; i >= 0i; i = i - 1i) {
        var bit = 0u;
        if (i >= 32i) {
            bit = (p.y >> u32(i - 32i)) & 1u;
        } else {
            bit = (p.x >> u32(i)) & 1u;
        }
        let carry = rem >> 31u;
        rem = (rem << 1u) | bit;
        if (carry == 1u || rem >= c) {
            rem = rem - c;
            if (i < 32i) {
                q = q | (1u << u32(i));
            }
        }
    }
    return vec2<u32>(q, rem);
}

fn burn_rate(fuel: u32) -> u32 {
    if (fuel <= 1u) {
        return ALWAYS;
    }
    return ALWAYS / fuel;
}

// ---- the material table ----

fn mword(m: u32, k: u32) -> u32 {
    return rules[MAT_BASE + m * 8u + k];
}
fn density(m: u32) -> u32 { return mword(m, 0u); }
fn thermal_mass(m: u32) -> u32 { return mword(m, 1u); }
fn conductivity(m: u32) -> u32 { return mword(m, 2u) & 0xFFFFu; }
fn melt_point(m: u32) -> u32 { return mword(m, 3u) & 0xFFFFu; }
fn boil_point(m: u32) -> u32 { return mword(m, 3u) >> 16u; }
fn ignite_point(m: u32) -> u32 { return mword(m, 4u) & 0xFFFFu; }
fn cool_point(m: u32) -> u32 { return mword(m, 4u) >> 16u; }
fn fuel_of(m: u32) -> u32 { return mword(m, 5u) & 0xFFFFu; }
fn class_of(m: u32) -> u32 { return (mword(m, 5u) >> 16u) & 0xFFu; }
fn flow_of(m: u32) -> u32 { return mword(m, 6u) & 0xFFu; }
fn melts_into(m: u32) -> u32 { return mword(m, 6u) >> 24u; }
fn boils_into(m: u32) -> u32 { return mword(m, 7u) & 0xFFu; }
fn burns_into(m: u32) -> u32 { return (mword(m, 7u) >> 8u) & 0xFFu; }
fn cools_into(m: u32) -> u32 { return (mword(m, 7u) >> 16u) & 0xFFu; }

fn reactive(m: u32) -> bool {
    return ((rules[REACTIVE_BASE + (m >> 5u)] >> (m & 31u)) & 1u) != 0u;
}

fn byte_at(base: u32, index: u32) -> u32 {
    return (rules[base + (index >> 2u)] >> ((index & 3u) * 8u)) & 0xFFu;
}

