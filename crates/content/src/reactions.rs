//! `assets/reactions.ron`: what happens when materials meet, as data. Rows are validated and compiled
//! into `engine_core::reaction::ReactionTable`; phase changes and burning come from the material table and
//! need no row. Errors cite `assets/reactions.ron:LINE`.
use crate::BlockId;
use engine_core::material::{MaterialId, SIM_MATERIAL_ROWS, SimMaterial};
use engine_core::reaction::{Reaction, ReactionError, ReactionTable};
use serde::{Deserialize, Serialize};

const FILE: &str = "assets/reactions.ron";
pub(crate) const REACTIONS_RON: &str = include_str!("../../../assets/reactions.ron");

/// One side of a reaction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Reactant {
    /// One material; `Air` is empty space.
    Block(BlockId),
    /// Every material with an ignition point and something it burns into. Allowed as `a` only.
    Burnable,
}

/// A temperature bound on `a`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Threshold {
    /// Kelvin.
    K(f32),
    /// The material's own ignition point.
    Ignition,
}

/// One authored reaction. On a heat tick, when `a`'s temperature is inside the bounds, it fires with
/// chance `rate`: `a` becomes `a_into` and `b` (its neighbour, if any) becomes `b_into`. A missing product
/// keeps the material; a missing temperature keeps the cell's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReactionRow {
    /// What the row is for, in a sentence.
    pub why: String,
    pub a: Reactant,
    #[serde(default)]
    pub b: Option<Reactant>,
    #[serde(default)]
    pub a_above: Option<Threshold>,
    #[serde(default)]
    pub a_below: Option<Threshold>,
    /// Chance per heat tick, in (0, 1].
    pub rate: f64,
    #[serde(default)]
    pub a_into: Option<BlockId>,
    #[serde(default)]
    pub b_into: Option<BlockId>,
    /// Kelvin.
    #[serde(default)]
    pub a_temp: Option<f32>,
    #[serde(default)]
    pub b_temp: Option<f32>,
}

/// 1-based line where row `index` starts: the pretty printer opens each row on a line of its own.
fn line_of_row(text: &str, index: usize) -> usize {
    text.lines().enumerate().filter(|(_, l)| l.trim_end() == "    (").nth(index).map_or(1, |(i, _)| i + 1)
}

/// Kelvin to 0.1 K, in f64 so an authored 383.15 lands on 3832 without depending on f32 spacing.
fn deci_kelvin(kelvin: f32) -> Option<u16> {
    let k = kelvin as f64;
    (k > 0.0 && k <= 6553.5).then(|| (k * 10.0).round() as u16)
}

fn id(block: Option<BlockId>) -> MaterialId {
    block.map_or(MaterialId::NONE, |b| MaterialId(b as u8))
}

/// The materials a reactant stands for.
fn expand(reactant: Reactant, table: &[SimMaterial; SIM_MATERIAL_ROWS]) -> Vec<u8> {
    match reactant {
        Reactant::Block(b) => vec![b as u8],
        Reactant::Burnable => (0..crate::ALL_BLOCK_IDS.len() as u8)
            .filter(|&m| table[m as usize].ignite != 0 && table[m as usize].burns_into != MaterialId::NONE)
            .collect(),
    }
}

/// Validate and compile every row; all problems are reported, each with its line.
pub(crate) fn compile(rows: &[ReactionRow], text: &str, table: &[SimMaterial; SIM_MATERIAL_ROWS]) -> Result<ReactionTable, Vec<String>> {
    let mut out = ReactionTable::default();
    let mut errors = Vec::new();
    for (index, r) in rows.iter().enumerate() {
        let line = line_of_row(text, index);
        let mut fail = |message: String| errors.push(format!("{FILE}:{line}: {message}"));
        if !(r.rate > 0.0 && r.rate <= 1.0) {
            fail(format!("rate must be in (0, 1], not {}", r.rate));
        }
        if r.b.is_none() && (r.b_into.is_some() || r.b_temp.is_some()) {
            fail("a single-cell row has no b_into or b_temp".into());
        }
        if r.b == Some(Reactant::Burnable) {
            fail("Burnable is only allowed as a".into());
        }
        if r.rate > 0.0 && Reaction::rate_from(r.rate) == 0 {
            fail(format!("rate {} rounds to a chance of zero per heat tick", r.rate));
        }
        if r.a_into.is_none() && r.b_into.is_none() {
            fail("the row changes no material, so its condition would never clear".into());
        }
        if let (Reactant::Block(a), Some(Reactant::Block(b))) = (r.a, r.b)
            && a == b
        {
            fail(format!("a self-pair ({a:?} + {a:?}) has no first cell to hold the condition"));
        }
        if r.b.is_none() && matches!(r.a, Reactant::Block(a) if table[a as usize].class == engine_core::material::MovementClass::Void) {
            fail("empty space does nothing on its own; give it a partner".into());
        }
        let temp = |name: &str, kelvin: Option<f32>, fail: &mut dyn FnMut(String)| match kelvin {
            None => 0,
            Some(k) => deci_kelvin(k).unwrap_or_else(|| {
                fail(format!("{name} must be in (0, 6553.5] K, not {k}"));
                0
            }),
        };
        let (a_temp, b_temp) = (temp("a_temp", r.a_temp, &mut fail), temp("b_temp", r.b_temp, &mut fail));
        let rate = Reaction::rate_from(r.rate);
        for a in expand(r.a, table) {
            let bound = |t: Option<Threshold>, open: u16, fail: &mut dyn FnMut(String)| match t {
                None => open,
                Some(Threshold::K(k)) => deci_kelvin(k).unwrap_or_else(|| {
                    fail(format!("a threshold must be in (0, 6553.5] K, not {k}"));
                    open
                }),
                Some(Threshold::Ignition) if table[a as usize].ignite != 0 => table[a as usize].ignite,
                Some(Threshold::Ignition) => {
                    fail(format!("Ignition needs a material with an ignition point; {:?} has none", BlockId::from_u32(a as u32)));
                    open
                }
            };
            let (a_above, a_below) = (bound(r.a_above, 0, &mut fail), bound(r.a_below, u16::MAX, &mut fail));
            if a_above > a_below {
                fail("a_above is above a_below".into());
            }
            let reaction =
                Reaction { rate, a_above, a_below, a_temp, b_temp, a_into: id(r.a_into), b_into: id(r.b_into), ..Reaction::ALWAYS };
            let name = |m: u8| format!("{:?}", BlockId::from_u32(m as u32));
            let added = match r.b {
                None => out.add_unary(MaterialId(a), reaction).map_err(|e| (e, name(a))),
                Some(b) => expand(b, table)
                    .into_iter()
                    .try_for_each(|b| out.add_pair(MaterialId(a), MaterialId(b), reaction).map_err(|e| (e, format!("{} + {}", name(a), name(b))))),
            };
            match added {
                Ok(()) => {}
                Err((ReactionError::Duplicate, who)) => fail(format!("{who} already has a row")),
                Err((ReactionError::Full, _)) => fail("more than 255 compiled rows".into()),
            }
        }
    }
    // A `Burnable` row checks each material it stands for; report each message once.
    let mut seen = std::collections::HashSet::new();
    errors.retain(|e| seen.insert(e.clone()));
    if errors.is_empty() { Ok(out) } else { Err(errors) }
}

#[cfg(test)]
#[path = "reactions_tests.rs"]
mod tests;
