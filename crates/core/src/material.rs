//! The cellular simulation's view of a material: ids and one packed row.
//!
//! `engine_content` builds the 256 rows from `assets/materials.ron`; the CPU
//! reference kernels and the GPU kernels both read this table and nothing
//! else, so a rule can only depend on properties, never on which block it is.
//! Every field is an integer (docs/adr/0008-ca-determinism.md).

/// Index into the 256-row table. `NONE` is "no such material" in a product field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialId(pub u8);

impl MaterialId {
    pub const NONE: Self = Self(u8::MAX);
}

/// How a cell moves, derived from `matter_state` and cohesion (never authored).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MovementClass {
    /// Empty space.
    Void = 0,
    /// Does not move. Includes cohesive powders such as dirt and clay.
    Solid = 1,
    /// Falls, then slides diagonally.
    Powder = 2,
    /// Falls, then spreads sideways at `flow` per 255.
    Liquid = 3,
    /// Rises and spreads.
    Gas = 4,
    /// Short-lived and luminous: fire.
    Plasma = 5,
}

/// One material row, 32 bytes (eight little-endian `u32` words on the GPU).
///
/// Temperatures are 0.1 K steps and 0 means "none". Product fields hold a
/// `MaterialId` and `MaterialId::NONE` means "none".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SimMaterial {
    /// 0.01 kg/m³ steps, so a gas lighter than 1 kg/m³ still orders against air.
    pub density: u32,
    /// Heat capacity of one volume level of this material, `density × capacity / 1024`: a full cell
    /// holds `VOLUME_FULL` times this many energy units per 0.1 K. Fits `u32` for 64 levels (validated).
    pub thermal_mass: u32,
    /// Thermal conductivity in 0.01 W/(m·K).
    pub conductivity: u16,
    /// Specific heat in J/(kg·K).
    pub capacity: u16,
    pub melt: u16,
    pub boil: u16,
    pub ignite: u16,
    /// Cools into `cools_into` below this temperature.
    pub cool: u16,
    pub fuel: u16,
    pub class: MovementClass,
    /// 0 flows freely, 255 never slides. Read only for powders.
    pub cohesion: u8,
    /// Sideways moves per 255 ticks for liquids and gases, from viscosity.
    pub flow: u8,
    /// Pressure response in 0.01 steps.
    pub pressure: u8,
    /// Decades of electrical conductivity above 1e-6 S/m: 0 insulates, 14 is copper.
    pub electric: u8,
    pub melts_into: MaterialId,
    pub boils_into: MaterialId,
    pub burns_into: MaterialId,
    pub cools_into: MaterialId,
    pub reserved: [u8; 1],
}

const _: () = assert!(size_of::<SimMaterial>() == 32 && align_of::<SimMaterial>() == 4);

/// Rows in the table; `MaterialId` is one byte.
pub const SIM_MATERIAL_ROWS: usize = 256;

impl SimMaterial {
    /// Unused rows: empty, inert, never changes.
    pub const INERT: Self = Self {
        density: 0,
        thermal_mass: 0,
        conductivity: 0,
        capacity: 0,
        melt: 0,
        boil: 0,
        ignite: 0,
        cool: 0,
        fuel: 0,
        class: MovementClass::Void,
        cohesion: 0,
        flow: 0,
        pressure: 0,
        electric: 0,
        melts_into: MaterialId::NONE,
        boils_into: MaterialId::NONE,
        burns_into: MaterialId::NONE,
        cools_into: MaterialId::NONE,
        reserved: [0; 1],
    };
}
