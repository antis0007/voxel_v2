//! The rule applied to one 2×2×2 Margolus block. It reads and writes only the
//! block's eight cells, so blocks of one phase are independent and may run in
//! any order or in parallel. Behaviour comes from the material table's class,
//! density, cohesion (folded into class) and flow, never from which block it is.
use crate::cell::{Cell, VOLUME_FULL};
use crate::pinned::{Dice, flow_gate};
use engine_core::material::{MovementClass, SimMaterial};
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
            let amount = u.volume.min(VOLUME_FULL - l.volume);
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
        let (nx, nz) = sideways[(k + (random >> (2 * (x + 2 * z))) as usize) % 2];
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

/// Apply one phase's rule to the block anchored at `anchor`. `phase_counter` is `2 * tick + phase`.
pub fn block_rule(t: &[SimMaterial], cells: &mut [Cell; 8], seed: u32, phase_counter: u32, anchor: IVec3) -> Outcome {
    let before = *cells;
    let mut pending = false;
    let columns = [(0, 0), (1, 0), (0, 1), (1, 1)];
    for (x, z) in columns {
        fall(t, cells, idx(x, 0, z), idx(x, 1, z));
    }
    let dice = Dice { seed, phase_counter, anchor };
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
    Outcome { changed: *cells != before, pending }
}
