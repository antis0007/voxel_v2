//! The rule applied to one 2×2×2 Margolus block. It reads and writes only the
//! block's eight cells, so blocks of one phase are independent and may run in
//! any order or in parallel. Behaviour comes from the material table's class,
//! density, cohesion (folded into class) and flow, never from which block it is.
use crate::cell::{Cell, VOLUME_FULL};
use crate::react::react;
use crate::pinned::{Dice, HEAT_DEADBAND, HEAT_RATE, HEAT_TICK_INTERVAL, flow_gate, is_heat_phase, mul_div_rem, mulhi};
use engine_core::material::{MovementClass, SimMaterial};
use engine_core::reaction::SimRules;
use glam::IVec3;

/// Cell `(x, y, z)` of a block, each 0 or 1.
pub const fn idx(x: usize, y: usize, z: usize) -> usize {
    x + 2 * z + 4 * y
}

/// What a block did: `changed` cells, and `pending` moves that a random gate suppressed this
/// phase and that therefore must be retried (the brick must stay awake).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub changed: bool,
    pub pending: bool,
}

fn class(t: &[SimMaterial], c: &Cell) -> MovementClass {
    t[c.material as usize].class
}

/// Void, powder, liquid and gas cells take part; solids and plasma stay put.
fn movable(t: &[SimMaterial], c: &Cell) -> bool {
    matches!(class(t, c), MovementClass::Void | MovementClass::Powder | MovementClass::Liquid | MovementClass::Gas)
}

fn density(t: &[SimMaterial], c: &Cell) -> u32 {
    t[c.material as usize].density
}

fn spreads(t: &[SimMaterial], c: &Cell) -> bool {
    matches!(class(t, c), MovementClass::Liquid | MovementClass::Gas)
}

/// Move `amount` volume from `cells[from]` into `cells[to]` (same material or empty). The receiver's
/// temperature becomes the volume-weighted mean; a drained source becomes `EMPTY`.
fn transfer(cells: &mut [Cell; 8], from: usize, to: usize, amount: u8) {
    let src = cells[from];
    let dst = &mut cells[to];
    if dst.volume == 0 {
        *dst = Cell { volume: amount, pressure: 0, charge: 0, ..src };
    } else {
        let (a, b) = (amount as u32, dst.volume as u32);
        dst.temperature = ((dst.temperature as u32 * b + src.temperature as u32 * a) / (a + b)) as u16;
        dst.volume += amount;
    }
    cells[from].volume -= amount;
    if cells[from].volume == 0 {
        cells[from] = Cell::EMPTY;
    }
}

/// A column: denser material sinks below lighter, a liquid fills the same liquid beneath it.
fn fall(t: &[SimMaterial], cells: &mut [Cell; 8], lo: usize, up: usize) {
    let (l, u) = (cells[lo], cells[up]);
    if !movable(t, &l) || !movable(t, &u) {
        return;
    }
    if u.material == l.material {
        if class(t, &u) == MovementClass::Liquid {
            let amount = u.volume.min(VOLUME_FULL.saturating_sub(l.volume));
            if amount > 0 {
                transfer(cells, up, lo, amount);
            }
        }
    } else if density(t, &u) > density(t, &l) {
        cells.swap(up, lo);
    }
}

/// A loose powder resting on something slides to an empty neighbouring column, either direction by hash.
fn slide(t: &[SimMaterial], cells: &mut [Cell; 8], x: usize, z: usize, dice: &Dice) {
    let up = idx(x, 1, z);
    if class(t, &cells[up]) != MovementClass::Powder || cells[idx(x, 0, z)].volume == 0 {
        return;
    }
    let sideways = [(x ^ 1, z), (x, z ^ 1)];
    let random = dice.roll(1);
    for k in 0..2 {
        let (nx, nz) = sideways[(k + ((random >> (2 * (x + 2 * z))) & 1) as usize) % 2];
        let (n_lo, n_up) = (idx(nx, 0, nz), idx(nx, 1, nz));
        if cells[n_lo].volume == 0 && cells[n_up].volume == 0 {
            cells.swap(up, n_lo);
            return;
        }
    }
}

/// Two horizontal neighbours: equal liquid or gas levels out, and a liquid or gas spills half into empty
/// space. A liquid's `flow` gates it; a suppressed move is reported as pending.
fn level(t: &[SimMaterial], cells: &mut [Cell; 8], a: usize, b: usize, salt: u32, dice: &Dice, pending: &mut bool) {
    let (ca, cb) = (cells[a], cells[b]);
    let (from, to, amount, gate_flow) = if ca.material == cb.material && spreads(t, &ca) {
        let (hi, lo, diff) = if ca.volume >= cb.volume { (a, b, ca.volume - cb.volume) } else { (b, a, cb.volume - ca.volume) };
        (hi, lo, diff / 2, t[ca.material as usize].flow)
    } else if cb.volume == 0 && spreads(t, &ca) {
        (a, b, ca.volume / 2, t[ca.material as usize].flow)
    } else if ca.volume == 0 && spreads(t, &cb) {
        (b, a, cb.volume / 2, t[cb.material as usize].flow)
    } else {
        return;
    };
    if amount == 0 {
        return;
    }
    if flow_gate(|| dice.roll(salt), gate_flow) {
        transfer(cells, from, to, amount);
    } else {
        *pending = true;
    }
}

/// Heat capacity of a cell: its volume levels times the material's thermal mass per level. Zero for
/// empty space and for an unused material row.
fn heat_capacity(t: &[SimMaterial], c: &Cell) -> u32 {
    c.volume as u32 * t[c.material as usize].thermal_mass
}

/// Move `rem / c` of one 0.1 K step on top of `whole` steps: the fraction is rounded up with exactly that
/// probability, from `random`, so the expected energy is the exact energy.
fn steps(whole: u32, rem: u32, c: u32, random: u32) -> u32 {
    whole + u32::from(mulhi(random, c) < rem)
}

fn is_void(t: &[SimMaterial], c: &Cell) -> bool {
    class(t, c) == MovementClass::Void
}

/// Conduction along one edge. Energy `q = g × d` leaves the hotter cell and enters the colder, where `d`
/// is the difference and `g` is the conductivity-limited energy per step, never more than half of what
/// would equalise the pair. Each cell converts its share to whole steps plus a randomly rounded one.
///
/// Empty space is the environment: a sink at its stored temperature that does not warm up, so heat
/// leaving a hot object does not spread into an ever larger volume of air, and a world of air costs
/// nothing. Returns whether heat is still flowing here, which keeps the brick awake.
fn conduct(t: &[SimMaterial], cells: &mut [Cell; 8], a: usize, b: usize, salt: u32, dice: &Dice) -> bool {
    let (void_a, void_b) = (is_void(t, &cells[a]), is_void(t, &cells[b]));
    if void_a && void_b {
        return false;
    }
    let (ta, tb) = (cells[a].temperature as u32, cells[b].temperature as u32);
    let d = ta.abs_diff(tb);
    if d < HEAT_DEADBAND {
        return false;
    }
    let (ka, kb) = (t[cells[a].material as usize].conductivity as u32, t[cells[b].material as usize].conductivity as u32);
    let (ca, cb) = (heat_capacity(t, &cells[a]), heat_capacity(t, &cells[b]));
    if ka == 0 || kb == 0 || (!void_a && ca == 0) || (!void_b && cb == 0) {
        return false;
    }
    // Two cells of one material and volume (most solid edges): no division for the mean or the equal-share.
    let same = cells[a].material == cells[b].material && cells[a].volume == cells[b].volume;
    let k = if same { ka } else { mul_div_rem(2 * ka, kb, ka + kb).0 };
    if void_a || void_b {
        // Only the solid, liquid or gas cell changes; the environment keeps its temperature.
        let (cell, env_temp) = if void_a { (b, ta) } else { (a, tb) };
        let c = if void_a { cb } else { ca };
        let g = (k * HEAT_RATE * HEAT_TICK_INTERVAL).min(c / 2);
        let (whole, rem) = mul_div_rem(g, d, c);
        let moved = steps(whole, rem, c, dice.roll(salt)).min(u16::MAX as u32) as u16;
        let t_cell = &mut cells[cell].temperature;
        *t_cell = if *t_cell as u32 > env_temp { t_cell.saturating_sub(moved) } else { t_cell.saturating_add(moved) };
        return true;
    }
    let equalise = if same { ca / 2 } else { mul_div_rem(ca, cb, ca + cb).0 };
    let g = (k * HEAT_RATE * HEAT_TICK_INTERVAL).min(equalise / 2);
    let (hot, cold) = if ta > tb { (a, b) } else { (b, a) };
    let (c_hot, c_cold) = (heat_capacity(t, &cells[hot]), heat_capacity(t, &cells[cold]));
    let (whole_h, rem_h) = mul_div_rem(g, d, c_hot);
    let (whole_c, rem_c) = if same { (whole_h, rem_h) } else { mul_div_rem(g, d, c_cold) };
    let down = steps(whole_h, rem_h, c_hot, dice.roll(salt));
    let up = steps(whole_c, rem_c, c_cold, dice.roll(salt + 1));
    cells[hot].temperature = cells[hot].temperature.saturating_sub(down.min(u16::MAX as u32) as u16);
    cells[cold].temperature = cells[cold].temperature.saturating_add(up.min(u16::MAX as u32) as u16);
    true
}

/// Apply one phase's rule to the block anchored at `anchor`. `phase_counter` is `2 * tick + phase`.
pub fn block_rule(rules: &SimRules, cells: &mut [Cell; 8], seed: u32, phase_counter: u32, anchor: IVec3) -> Outcome {
    let t = rules.materials();
    let before = *cells;
    let mut pending = false;
    let columns = [(0, 0), (1, 0), (0, 1), (1, 1)];
    for (x, z) in columns {
        fall(t, cells, idx(x, 0, z), idx(x, 1, z));
    }
    let dice = Dice::new(seed, phase_counter, anchor);
    for (x, z) in columns {
        slide(t, cells, x, z, &dice);
    }
    // Edges of each 2×2 layer: x pairs, then z pairs.
    for y in 0..2 {
        for (salt, (a, b)) in [(0, 0), (0, 1), (1, 0), (1, 1)]
            .into_iter()
            .map(|(x, z)| if x == 0 { (idx(0, y, z), idx(1, y, z)) } else { (idx(z, y, 0), idx(z, y, 1)) })
            .enumerate()
        {
            level(t, cells, a, b, 2 + (y * 4 + salt) as u32, &dice, &mut pending);
        }
    }
    // Heat runs along the twelve edges of the cube, after matter has moved, on heat ticks only; a uniform block has none.
    if is_heat_phase(phase_counter) && cells.iter().any(|c| c.temperature != cells[0].temperature) {
        let mut salt = 16;
        for a in 0..8usize {
            for bit in [1usize, 2, 4] {
                if a & bit == 0 {
                    pending |= conduct(t, cells, a, a | bit, salt, &dice);
                    salt += 2;
                }
            }
        }
    }
    // Then phase changes and reactions, on the temperatures conduction left.
    if is_heat_phase(phase_counter) {
        pending |= react(rules, cells, &dice, phase_counter & 1 == 0);
    }
    Outcome { changed: *cells != before, pending }
}
