//! The reaction pass of one block, on heat ticks after conduction. Every change comes from the material
//! table (phase changes, burning) or the compiled reactions (`assets/reactions.ron`), never from which
//! block a cell holds. A cell's material changes at most once per pass, in this order:
//! 1. the pair reaction along each of the block's twelve edges, in a fixed order: contact comes first, so
//!    lava touching water turns to basalt even though the water would also have boiled;
//! 2. its single-cell reaction;
//! 3. burning: at or above its ignition point a fuel is spent with chance 1 / `fuel` per heat tick;
//! 4. phase change: melt, boil or cool past the table's point, deterministic.
//!
//! Each edge of the lattice lies inside a block of exactly one of a heat tick's two phases, so a pair
//! rolls once per heat tick; every cell lies in a block of both, so single-cell rows and burning roll on
//! the first phase only. A phase change needs no roll and is checked on both.
//!
//! A reaction that leaves a cell's material alone (a burning log lighting the air beside it) does not use
//! up that cell's change.
use crate::cell::{Cell, VOLUME_FULL};
use crate::pinned::{Dice, SALT_BURN, SALT_PAIR, SALT_UNARY, burn_rate};
use engine_core::material::{MaterialId, MovementClass, SimMaterial};
use engine_core::reaction::SimRules;

/// The block's twelve edges as cell pairs (`idx` order: x is bit 0, z bit 1, y bit 2).
const EDGES: [(usize, usize); 12] =
    [(0, 1), (0, 2), (0, 4), (1, 3), (1, 5), (2, 3), (2, 6), (3, 7), (4, 5), (4, 6), (5, 7), (6, 7)];

/// Turn a cell into `into`, keeping its volume (empty space filled by a product becomes a full cell) and,
/// unless `temp` is set, its temperature. A product of empty space is empty space at ambient. Returns
/// whether the material changed.
fn become_(t: &[SimMaterial], cell: &mut Cell, into: MaterialId, temp: u16) -> bool {
    if into == MaterialId::NONE || into.0 == cell.material && cell.volume > 0 {
        if temp != 0 {
            cell.temperature = temp;
        }
        return false;
    }
    if t[into.0 as usize].class == MovementClass::Void {
        *cell = Cell::EMPTY;
        return true;
    }
    let volume = if cell.volume == 0 { VOLUME_FULL } else { cell.volume };
    let temperature = if temp != 0 { temp } else { cell.temperature };
    *cell = Cell { material: into.0, volume, temperature, ..*cell };
    true
}

/// The table's phase change for a cell, if its temperature has crossed one.
fn phase_change(row: &SimMaterial, temperature: u16) -> Option<MaterialId> {
    let past = |point: u16, into: MaterialId, above: bool| {
        point != 0 && into != MaterialId::NONE && if above { temperature >= point } else { temperature <= point }
    };
    if past(row.melt, row.melts_into, true) {
        Some(row.melts_into)
    } else if past(row.boil, row.boils_into, true) {
        Some(row.boils_into)
    } else if past(row.cool, row.cools_into, false) {
        Some(row.cools_into)
    } else {
        None
    }
}

/// Roll for a reaction whose condition holds. Returns whether it fires; a miss leaves the block pending.
fn roll(rate: u32, dice: &Dice, salt: u32, pending: &mut bool) -> bool {
    let fires = rate == u32::MAX || dice.roll(salt) < rate;
    *pending |= !fires;
    fires
}

/// Apply the reaction pass; `first_phase` is whether this is the heat tick's first Margolus phase. Returns
/// whether a reaction is waiting on the dice (the brick stays awake).
pub(crate) fn react(rules: &SimRules, cells: &mut [Cell; 8], dice: &Dice, first_phase: bool) -> bool {
    if !cells.iter().any(|c| rules.reactive(c.material)) {
        return false;
    }
    let t = rules.materials();
    let mut done = 0u8;
    let mut pending = false;
    for (e, &(a, b)) in EDGES.iter().enumerate() {
        if done & (1 << a | 1 << b) != 0 {
            continue;
        }
        let Some(r) = rules.reactions().pair(cells[a].material, cells[b].material) else { continue };
        let conditioned = if r.on_second() { cells[b].temperature } else { cells[a].temperature };
        if r.holds(conditioned) && roll(r.rate, dice, SALT_PAIR + e as u32, &mut pending) {
            let r = *r;
            done |= u8::from(become_(t, &mut cells[a], r.a_into, r.a_temp)) << a;
            done |= u8::from(become_(t, &mut cells[b], r.b_into, r.b_temp)) << b;
        }
    }
    for (i, cell) in cells.iter_mut().enumerate() {
        let Some(r) = rules.reactions().unary(cell.material).filter(|_| first_phase) else { continue };
        if done & (1 << i) == 0 && r.holds(cell.temperature) && roll(r.rate, dice, SALT_UNARY + i as u32, &mut pending) {
            done |= u8::from(become_(t, cell, r.a_into, r.a_temp)) << i;
        }
    }
    for (i, cell) in cells.iter_mut().enumerate() {
        let row = &t[cell.material as usize];
        let burning = first_phase && row.ignite != 0 && row.burns_into != MaterialId::NONE && cell.temperature >= row.ignite;
        if done & (1 << i) == 0 && burning && roll(burn_rate(row.fuel), dice, SALT_BURN + i as u32, &mut pending) {
            done |= u8::from(become_(t, cell, row.burns_into, 0)) << i;
        }
    }
    for (i, cell) in cells.iter_mut().enumerate() {
        if done & (1 << i) == 0
            && let Some(into) = phase_change(&t[cell.material as usize], cell.temperature)
        {
            become_(t, cell, into, 0);
        }
    }
    pending
}
