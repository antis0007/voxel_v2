//! Cellular-simulation properties of a material and their packed form.
//!
//! Rows are authored in real units (kelvin, W/(m·K), J/(kg·K), S/m) and packed
//! into the integer `SimMaterial` row at load. Packing uses only IEEE basic
//! operations (multiply, divide, compare, round), so the table is identical on
//! every machine. The values are tuned to match `simulation/thermal.rs` (ice
//! melts at 35 °C because the ambient is 20 °C); the science profile
//! (docs/PLAN2.md Phase D) replaces them with measured constants.
use super::{ALL_BLOCK_IDS, BlockId, MATERIAL_TABLE, MaterialDef, MatterState};
use engine_core::material::{MaterialId, MovementClass, SIM_MATERIAL_ROWS, SimMaterial};
use std::sync::LazyLock;

/// Heat, fuel and state-change properties. Missing fields take the generic solid's.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ThermalTraits {
    /// W/(m·K).
    pub conductivity: f32,
    /// J/(kg·K).
    pub capacity: f32,
    /// Heating above this (K) turns the cell into `melts_into`.
    pub melt: Option<f32>,
    /// Heating above this (K) turns the cell into `boils_into`.
    pub boil: Option<f32>,
    /// Fuel ignites above this (K).
    pub ignite: Option<f32>,
    /// Cooling below this (K) turns the cell into `cools_into`.
    pub cool: Option<f32>,
    /// Burn time in simulation units; 0 does not burn.
    pub fuel: f32,
}

/// Missing fields take the generic solid's, and a row equal to it is not written.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SimTraits {
    /// 0..1: how strongly a powder holds a slope. At 0.5 and above it does not slide.
    pub cohesion: f32,
    pub thermal: ThermalTraits,
    /// S/m.
    pub electric_conductivity: f32,
    pub melts_into: Option<BlockId>,
    pub boils_into: Option<BlockId>,
    /// What is left when the fuel is spent.
    pub burns_into: Option<BlockId>,
    pub cools_into: Option<BlockId>,
}

impl Default for ThermalTraits {
    fn default() -> Self {
        Self { conductivity: 1.0, capacity: 800.0, melt: None, boil: None, ignite: None, cool: None, fuel: 0.0 }
    }
}

impl SimTraits {
    /// True for the generic inert solid; `materials.ron` omits those rows' `sim`.
    pub(crate) fn is_generic(&self) -> bool {
        *self == Self::default()
    }
}

impl Default for SimTraits {
    /// A generic inert solid, so a new block needs a `sim` row only if it differs.
    fn default() -> Self {
        Self {
            cohesion: 1.0,
            thermal: ThermalTraits::default(),
            electric_conductivity: 0.0,
            melts_into: None,
            boils_into: None,
            burns_into: None,
            cools_into: None,
        }
    }
}

/// Powders at or above this cohesion behave as solids (dirt, clay, mud).
pub const COHESION_STABLE: f32 = 0.5;

impl From<BlockId> for MaterialId {
    fn from(block: BlockId) -> Self {
        // `ALL_BLOCK_IDS.len() < 255` is asserted below, so this never truncates.
        Self(block as u8)
    }
}

const _: () = assert!(ALL_BLOCK_IDS.len() < MaterialId::NONE.0 as usize);

fn product(block: Option<BlockId>) -> MaterialId {
    block.map_or(MaterialId::NONE, MaterialId::from)
}

/// Tenths of a kelvin; 0 means "none".
fn deci_kelvin(kelvin: Option<f32>) -> u16 {
    kelvin.map_or(0, |k| (k * 10.0).round() as u16)
}

/// Whole decades of conductivity above 1e-6 S/m, by repeated multiplication.
fn electric_decades(siemens_per_metre: f32) -> u8 {
    let (mut level, mut threshold) = (0u8, 1e-6_f32);
    while siemens_per_metre >= threshold && level < 15 {
        level += 1;
        threshold *= 10.0;
    }
    level
}

fn movement_class(def: &MaterialDef) -> MovementClass {
    match def.physics.matter_state {
        MatterState::Vacuum => MovementClass::Void,
        MatterState::Solid => MovementClass::Solid,
        MatterState::Powder if def.sim.cohesion >= COHESION_STABLE => MovementClass::Solid,
        MatterState::Powder => MovementClass::Powder,
        MatterState::Liquid => MovementClass::Liquid,
        MatterState::Gas => MovementClass::Gas,
        MatterState::Plasma => MovementClass::Plasma,
    }
}

/// Pack one validated row. Out-of-range values were rejected by `validate`.
pub(crate) fn pack(def: &MaterialDef) -> SimMaterial {
    let class = movement_class(def);
    let fluid = matches!(class, MovementClass::Liquid | MovementClass::Gas);
    let t = &def.sim.thermal;
    SimMaterial {
        class,
        cohesion: (def.sim.cohesion * 255.0).round() as u8,
        density: def.physics.density.round() as u16,
        flow: if fluid { (255.0 / def.physics.fluid.viscosity.max(1.0)) as u8 } else { 0 },
        pressure: (def.physics.fluid.pressure_response * 100.0).round() as u8,
        electric: electric_decades(def.sim.electric_conductivity),
        melts_into: product(def.sim.melts_into),
        conductivity: (t.conductivity * 100.0).round() as u16,
        capacity: t.capacity.round() as u16,
        melt: deci_kelvin(t.melt),
        boil: deci_kelvin(t.boil),
        ignite: deci_kelvin(t.ignite),
        cool: deci_kelvin(t.cool),
        fuel: t.fuel.round() as u16,
        boils_into: product(def.sim.boils_into),
        burns_into: product(def.sim.burns_into),
        cools_into: product(def.sim.cools_into),
        ..SimMaterial::INERT
    }
}

/// The table the cellular kernels read, indexed by `MaterialId`. Rows past the
/// last `BlockId` are `SimMaterial::INERT`.
pub static SIM_TABLE: LazyLock<[SimMaterial; SIM_MATERIAL_ROWS]> = LazyLock::new(|| {
    let mut table = [SimMaterial::INERT; SIM_MATERIAL_ROWS];
    for def in MATERIAL_TABLE.iter() {
        table[def.id as usize] = pack(def);
    }
    table
});

impl BlockId {
    pub fn sim(self) -> &'static SimMaterial {
        &SIM_TABLE[self as usize]
    }
}
