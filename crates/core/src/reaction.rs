//! Reactions the cellular kernels apply, compiled from `assets/reactions.ron`: packed rows and the
//! indexes that find them by material, so a kernel never names a block (docs/PLAN2.md A4).
use crate::material::{MaterialId, SIM_MATERIAL_ROWS, SimMaterial};

/// One compiled reaction, 16 bytes (four `u32` words on the GPU). On a heat tick, cell `a` (and, for a
/// pair, its neighbour `b`) turns into the products when the conditioned cell's temperature lies in
/// `a_above..=a_below` and the block's dice fall under `rate`.
///
/// Temperatures are 0.1 K; 0 in a product temperature keeps the cell's own, and `MaterialId::NONE` in a
/// product keeps the cell's material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Reaction {
    /// Chance per heat tick out of 2³²; `u32::MAX` always fires, without rolling.
    pub rate: u32,
    /// Lower bound on the conditioned cell's temperature; 0 is none.
    pub a_above: u16,
    /// Upper bound on the conditioned cell's temperature; `u16::MAX` is none.
    pub a_below: u16,
    pub a_temp: u16,
    pub b_temp: u16,
    pub a_into: MaterialId,
    pub b_into: MaterialId,
    /// Bit 0: the condition reads the second cell (set on the mirrored copy of a pair row).
    pub flags: u8,
    pub reserved: u8,
}

const _: () = assert!(size_of::<Reaction>() == 16);

impl Reaction {
    /// Fires every heat tick, unconditionally, and changes nothing: the base the rows are built on.
    pub const ALWAYS: Self = Self {
        rate: u32::MAX,
        a_above: 0,
        a_below: u16::MAX,
        a_temp: 0,
        b_temp: 0,
        a_into: MaterialId::NONE,
        b_into: MaterialId::NONE,
        flags: 0,
        reserved: 0,
    };

    /// A probability in `0..=1` as a threshold out of 2³², rounded down; 1 and above always fire.
    pub fn rate_from(probability: f64) -> u32 {
        if probability >= 1.0 { u32::MAX } else { (probability.max(0.0) * 4_294_967_296.0) as u32 }
    }

    /// Whether a roll of the block's dice fires this reaction.
    pub fn fires(&self, roll: u32) -> bool {
        self.rate == u32::MAX || roll < self.rate
    }

    /// Whether `temperature` (0.1 K) satisfies the condition.
    pub fn holds(&self, temperature: u16) -> bool {
        (self.a_above..=self.a_below).contains(&temperature)
    }

    /// Whether the condition reads the second cell of the pair.
    pub fn on_second(&self) -> bool {
        self.flags & 1 != 0
    }

    /// The same reaction seen from the other cell: roles and products swapped, condition kept on the
    /// cell it was written for.
    fn mirrored(self) -> Self {
        Self {
            a_into: self.b_into,
            b_into: self.a_into,
            a_temp: self.b_temp,
            b_temp: self.a_temp,
            flags: self.flags ^ 1,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionError {
    /// The reactants already have a row.
    Duplicate,
    /// 255 rows are in use; a row index must fit a byte.
    Full,
}

/// The compiled reactions: rows plus a pair index and a single-cell index. Each index entry is a row
/// number plus one, 0 meaning none. A pair is stored in both orders, so a kernel looks up its two
/// cells as they lie.
#[derive(Clone, Debug)]
pub struct ReactionTable {
    pair: Box<[[u8; SIM_MATERIAL_ROWS]; SIM_MATERIAL_ROWS]>,
    unary: [u8; SIM_MATERIAL_ROWS],
    rows: Vec<Reaction>,
}

impl Default for ReactionTable {
    fn default() -> Self {
        Self { pair: Box::new([[0; SIM_MATERIAL_ROWS]; SIM_MATERIAL_ROWS]), unary: [0; SIM_MATERIAL_ROWS], rows: Vec::new() }
    }
}

impl ReactionTable {
    fn push(&mut self, r: Reaction) -> Result<u8, ReactionError> {
        if self.rows.len() >= u8::MAX as usize {
            return Err(ReactionError::Full);
        }
        self.rows.push(r);
        Ok(self.rows.len() as u8)
    }

    /// `a` next to `b`, with the condition on `a`.
    pub fn add_pair(&mut self, a: MaterialId, b: MaterialId, r: Reaction) -> Result<(), ReactionError> {
        let (ia, ib) = (a.0 as usize, b.0 as usize);
        if self.pair[ia][ib] != 0 || self.pair[ib][ia] != 0 {
            return Err(ReactionError::Duplicate);
        }
        if a != b && self.rows.len() + 2 > u8::MAX as usize {
            return Err(ReactionError::Full);
        }
        self.pair[ia][ib] = self.push(r)?;
        if a != b {
            self.pair[ib][ia] = self.push(r.mirrored())?;
        }
        Ok(())
    }

    /// A reaction of one cell on its own.
    pub fn add_unary(&mut self, a: MaterialId, r: Reaction) -> Result<(), ReactionError> {
        if self.unary[a.0 as usize] != 0 {
            return Err(ReactionError::Duplicate);
        }
        self.unary[a.0 as usize] = self.push(r)?;
        Ok(())
    }

    pub fn pair(&self, a: u8, b: u8) -> Option<&Reaction> {
        self.row(self.pair[a as usize][b as usize])
    }

    pub fn unary(&self, a: u8) -> Option<&Reaction> {
        self.row(self.unary[a as usize])
    }

    fn row(&self, entry: u8) -> Option<&Reaction> {
        entry.checked_sub(1).map(|i| &self.rows[i as usize])
    }

    pub fn rows(&self) -> &[Reaction] {
        &self.rows
    }
}

/// Everything the cellular kernels read: the material table and the reactions, plus which materials can
/// change at all, so a block of inert matter skips the reaction pass.
#[derive(Clone, Debug)]
pub struct SimRules {
    materials: [SimMaterial; SIM_MATERIAL_ROWS],
    reactions: ReactionTable,
    reactive: [bool; SIM_MATERIAL_ROWS],
}

impl SimRules {
    pub fn new(materials: [SimMaterial; SIM_MATERIAL_ROWS], reactions: ReactionTable) -> Self {
        let mut reactive = [false; SIM_MATERIAL_ROWS];
        for (m, row) in materials.iter().enumerate() {
            let set = |threshold: u16, product: MaterialId| threshold != 0 && product != MaterialId::NONE;
            reactive[m] = set(row.melt, row.melts_into)
                || set(row.boil, row.boils_into)
                || set(row.cool, row.cools_into)
                || set(row.ignite, row.burns_into)
                || reactions.unary[m] != 0
                || reactions.pair[m].iter().any(|&e| e != 0);
        }
        Self { materials, reactions, reactive }
    }

    pub fn materials(&self) -> &[SimMaterial; SIM_MATERIAL_ROWS] {
        &self.materials
    }

    pub fn reactions(&self) -> &ReactionTable {
        &self.reactions
    }

    /// Whether a cell of this material can change state or react.
    pub fn reactive(&self, material: u8) -> bool {
        self.reactive[material as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAVA: MaterialId = MaterialId(6);
    const WATER: MaterialId = MaterialId(3);
    const BASALT: MaterialId = MaterialId(9);
    const STEAM: MaterialId = MaterialId(10);

    fn quench() -> Reaction {
        Reaction { a_into: BASALT, b_into: STEAM, b_temp: 3832, ..Reaction::ALWAYS }
    }

    #[test]
    fn a_pair_is_found_in_both_orders_with_the_roles_swapped() {
        let mut t = ReactionTable::default();
        t.add_pair(LAVA, WATER, Reaction { a_above: 12000, ..quench() }).unwrap();
        let forward = t.pair(LAVA.0, WATER.0).unwrap();
        assert_eq!((forward.a_into, forward.b_into, forward.on_second()), (BASALT, STEAM, false));
        let back = t.pair(WATER.0, LAVA.0).unwrap();
        assert_eq!((back.a_into, back.b_into, back.b_temp, back.a_temp), (STEAM, BASALT, 0, 3832));
        assert!(back.on_second(), "the condition stays on the lava, now the second cell");
        assert!(t.pair(LAVA.0, LAVA.0).is_none() && t.pair(WATER.0, BASALT.0).is_none());
    }

    #[test]
    fn a_second_row_for_the_same_reactants_is_refused() {
        let mut t = ReactionTable::default();
        t.add_pair(LAVA, WATER, quench()).unwrap();
        assert_eq!(t.add_pair(WATER, LAVA, quench()), Err(ReactionError::Duplicate));
        t.add_unary(LAVA, quench()).unwrap();
        assert_eq!(t.add_unary(LAVA, quench()), Err(ReactionError::Duplicate));
    }

    #[test]
    fn a_self_pair_is_stored_once() {
        let mut t = ReactionTable::default();
        t.add_pair(WATER, WATER, quench()).unwrap();
        assert_eq!(t.rows().len(), 1);
    }

    #[test]
    fn the_table_holds_at_most_255_rows() {
        let mut t = ReactionTable::default();
        for m in 0..255u8 {
            t.add_unary(MaterialId(m), quench()).unwrap();
        }
        assert_eq!(t.add_unary(MaterialId(255), quench()), Err(ReactionError::Full));
    }

    #[test]
    fn rates_round_down_and_one_always_fires() {
        assert_eq!(Reaction::rate_from(1.0), u32::MAX);
        assert_eq!(Reaction::rate_from(0.5), 1 << 31);
        assert_eq!(Reaction::rate_from(0.0), 0);
        assert!(Reaction::ALWAYS.fires(0xFFFF_FFFF));
        let half = Reaction { rate: Reaction::rate_from(0.5), ..Reaction::ALWAYS };
        assert!(half.fires(0x7FFF_FFFF) && !half.fires(0x8000_0000));
    }

    #[test]
    fn the_condition_bounds_are_inclusive_and_zero_or_max_is_open() {
        let r = Reaction { a_above: 3000, a_below: 4000, ..Reaction::ALWAYS };
        assert!(!r.holds(2999) && r.holds(3000) && r.holds(4000) && !r.holds(4001));
        assert!(Reaction::ALWAYS.holds(0) && Reaction::ALWAYS.holds(u16::MAX));
    }

    #[test]
    fn rules_mark_materials_that_can_change() {
        let mut materials = [SimMaterial::INERT; SIM_MATERIAL_ROWS];
        materials[WATER.0 as usize] = SimMaterial { boil: 3731, boils_into: STEAM, ..SimMaterial::INERT };
        materials[BASALT.0 as usize] = SimMaterial { boil: 3731, ..SimMaterial::INERT }; // no product: inert
        let mut reactions = ReactionTable::default();
        reactions.add_pair(LAVA, MaterialId(0), quench()).unwrap();
        let rules = SimRules::new(materials, reactions);
        assert!(rules.reactive(WATER.0) && rules.reactive(LAVA.0) && rules.reactive(0));
        assert!(!rules.reactive(BASALT.0) && !rules.reactive(STEAM.0));
    }
}
