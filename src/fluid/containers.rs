//! Fluid containers: one kind, a capacity and an amount, with conservative
//! transfer. Pumps, pipes and tanks all move fluid through these rules, so mass
//! is never created or destroyed and a container never holds two kinds at once.
//!
//! Units are integers at `UNITS_PER_M3` per cubic metre, the same scale the
//! factory's water network already uses.

/// Units per cubic metre. Integer amounts keep transfers exact.
pub const UNITS_PER_M3: u32 = 512;

/// Fluids the factory can produce or hold. Order is the UI order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FluidKind {
    Water,
    Steam,
    Seawater,
    Brine,
    Lava,
    CrudeOil,
    Naphtha,
    Kerosene,
    Diesel,
    Gasoline,
    Creosote,
    BioEthanol,
    PlantOil,
    Latex,
    Wastewater,
    Ethylene,
    SulfuricAcid,
    Ammonia,
    Mercury,
    Hydrogen,
    Oxygen,
    LiquidNitrogen,
    LiquidOxygen,
    MoltenIron,
    MoltenCopper,
    MoltenSteel,
}

impl FluidKind {
    pub const ALL: [Self; 26] = [
        Self::Water,
        Self::Steam,
        Self::Seawater,
        Self::Brine,
        Self::Lava,
        Self::CrudeOil,
        Self::Naphtha,
        Self::Kerosene,
        Self::Diesel,
        Self::Gasoline,
        Self::Creosote,
        Self::BioEthanol,
        Self::PlantOil,
        Self::Latex,
        Self::Wastewater,
        Self::Ethylene,
        Self::SulfuricAcid,
        Self::Ammonia,
        Self::Mercury,
        Self::Hydrogen,
        Self::Oxygen,
        Self::LiquidNitrogen,
        Self::LiquidOxygen,
        Self::MoltenIron,
        Self::MoltenCopper,
        Self::MoltenSteel,
    ];

    /// Display name from the catalog.
    pub fn label(self) -> &'static str {
        crate::fluid::catalog::spec(self).name
    }
}

/// A bounded store of one fluid kind. Empty containers have no kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Container {
    kind: Option<FluidKind>,
    amount: u32,
    capacity: u32,
}

impl Container {
    pub fn new(capacity: u32) -> Self {
        Self {
            kind: None,
            amount: 0,
            capacity,
        }
    }

    /// A tank of `cubic_metres` capacity, as used by the factory's 16 m³ tank.
    pub fn with_volume_m3(cubic_metres: u32) -> Self {
        Self::new(cubic_metres * UNITS_PER_M3)
    }

    pub fn kind(&self) -> Option<FluidKind> {
        self.kind
    }

    pub fn amount(&self) -> u32 {
        self.amount
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn room(&self) -> u32 {
        self.capacity - self.amount
    }

    pub fn is_empty(&self) -> bool {
        self.amount == 0
    }

    pub fn is_full(&self) -> bool {
        self.amount >= self.capacity
    }

    /// Fill level in `[0, 1]`. A zero-capacity container reads as empty.
    pub fn fill_fraction(&self) -> f32 {
        if self.capacity == 0 {
            0.0
        } else {
            self.amount as f32 / self.capacity as f32
        }
    }

    /// Whether this container can accept `kind` right now.
    pub fn accepts(&self, kind: FluidKind) -> bool {
        self.kind.is_none_or(|held| held == kind) && !self.is_full()
    }

    /// Add up to `units` of `kind`. Returns the amount accepted, which is less
    /// than requested when the container fills up, and zero for a foreign kind.
    pub fn insert(&mut self, kind: FluidKind, units: u32) -> u32 {
        if self.kind.is_some_and(|held| held != kind) {
            return 0;
        }
        let accepted = units.min(self.room());
        if accepted > 0 {
            self.kind = Some(kind);
            self.amount += accepted;
        }
        accepted
    }

    /// Remove up to `units`. Returns the amount removed; the kind clears when
    /// the container empties.
    pub fn extract(&mut self, units: u32) -> u32 {
        let removed = units.min(self.amount);
        self.amount -= removed;
        if self.amount == 0 {
            self.kind = None;
        }
        removed
    }
}

/// Move up to `max_units` from `from` to `to`. Returns the amount moved, which
/// is bounded by the source's contents, the destination's room, and a matching
/// kind. Total units across both containers are conserved exactly.
pub fn transfer(from: &mut Container, to: &mut Container, max_units: u32) -> u32 {
    let Some(kind) = from.kind() else {
        return 0;
    };
    if !to.accepts(kind) {
        return 0;
    }
    let units = max_units.min(from.amount()).min(to.room());
    let taken = from.extract(units);
    let inserted = to.insert(kind, taken);
    debug_assert_eq!(taken, inserted, "transfer must conserve units");
    inserted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_container_is_empty_without_a_kind() {
        let c = Container::with_volume_m3(16);
        assert!(c.is_empty());
        assert_eq!(c.kind(), None);
        assert_eq!(c.capacity(), 16 * UNITS_PER_M3);
        assert_eq!(c.fill_fraction(), 0.0);
    }

    #[test]
    fn insert_clamps_to_room_and_sets_kind() {
        let mut c = Container::new(100);
        assert_eq!(c.insert(FluidKind::Water, 70), 70);
        assert_eq!(c.insert(FluidKind::Water, 70), 30);
        assert!(c.is_full());
        assert_eq!(c.kind(), Some(FluidKind::Water));
    }

    #[test]
    fn a_container_never_holds_two_kinds() {
        let mut c = Container::new(100);
        c.insert(FluidKind::Water, 10);
        assert_eq!(c.insert(FluidKind::Creosote, 10), 0);
        assert!(!c.accepts(FluidKind::Creosote));
        assert_eq!(c.amount(), 10);
    }

    #[test]
    fn emptying_clears_the_kind_so_another_fluid_can_enter() {
        let mut c = Container::new(100);
        c.insert(FluidKind::Water, 10);
        assert_eq!(c.extract(50), 10);
        assert_eq!(c.kind(), None);
        assert_eq!(c.insert(FluidKind::BioEthanol, 5), 5);
    }

    #[test]
    fn transfer_conserves_total_units() {
        let mut a = Container::new(100);
        let mut b = Container::new(100);
        a.insert(FluidKind::Water, 80);
        b.insert(FluidKind::Water, 30);
        let before = a.amount() + b.amount();
        let moved = transfer(&mut a, &mut b, 60);
        assert_eq!(moved, 60);
        assert_eq!(a.amount() + b.amount(), before);
        assert_eq!(b.amount(), 90);
    }

    #[test]
    fn transfer_stops_at_destination_room() {
        let mut a = Container::new(100);
        let mut b = Container::new(20);
        a.insert(FluidKind::Creosote, 100);
        assert_eq!(transfer(&mut a, &mut b, 100), 20);
        assert_eq!(a.amount(), 80);
        assert_eq!(b.amount(), 20);
    }

    #[test]
    fn transfer_refuses_a_mismatched_kind() {
        let mut a = Container::new(100);
        let mut b = Container::new(100);
        a.insert(FluidKind::Water, 50);
        b.insert(FluidKind::BioEthanol, 5);
        assert_eq!(transfer(&mut a, &mut b, 50), 0);
        assert_eq!(a.amount(), 50);
        assert_eq!(b.amount(), 5);
    }

    #[test]
    fn transfer_from_empty_moves_nothing() {
        let mut a = Container::new(100);
        let mut b = Container::new(100);
        assert_eq!(transfer(&mut a, &mut b, 10), 0);
        assert_eq!(b.kind(), None);
    }

    #[test]
    fn fill_fraction_is_linear_and_safe_at_zero_capacity() {
        let mut c = Container::new(200);
        c.insert(FluidKind::Water, 50);
        assert!((c.fill_fraction() - 0.25).abs() < 1e-6);
        assert_eq!(Container::new(0).fill_fraction(), 0.0);
    }
}
