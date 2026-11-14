//! Per-material heat factors for the sparse lattice, read from
//! `assets/materials.ron`, and the latent store of a phase change.
//!
//! The lattice runs far faster than real conduction through metre cells, so
//! factors are relative to a reference rock and clamped to a 16:1 range.
//! Gases and fire stand for convection, not conduction, and keep the
//! reference factors.
use super::sandbox::ThermalCell;
use crate::world::BlockId;
use engine_content::{ALL_BLOCK_IDS, MatterState};
use std::sync::LazyLock;

/// Exchange per face between two reference cells, per step.
const BASE: f32 = 0.04;
/// Volumetric heat capacity of the reference rock, J/(m³·K).
const REFERENCE_CAPACITY: f32 = 2.0e6;
/// Still air against a solid or liquid face: a thin film limits the
/// exchange, so frost ice in 20 °C air lasts about 45 s at 1 m cells.
const AIR_FILM: f32 = 0.05;
const KELVIN: f32 = 273.15;

#[derive(Clone, Copy, Debug)]
pub struct Heat {
    /// Heat capacity relative to the reference rock.
    pub capacity: f32,
    /// Conductance factor, square root of W/(m·K) so copper does not drown the scale.
    pub conductance: f32,
    pub gas: bool,
    /// Still air, as opposed to fire, smoke or steam.
    pub air: bool,
    pub melt_c: Option<f32>,
    pub boil_c: Option<f32>,
    pub cool_c: Option<f32>,
    pub ignite_c: Option<f32>,
    pub fuel: f32,
    /// Latent heat of fusion in kelvin of this material's own capacity.
    pub fusion_k: f32,
}

static TABLE: LazyLock<Vec<Heat>> = LazyLock::new(|| {
    let mut rows = vec![Heat::REFERENCE; ALL_BLOCK_IDS.iter().map(|b| *b as usize + 1).max().unwrap_or(0)];
    for &b in ALL_BLOCK_IDS {
        let def = b.material();
        let t = def.sim.thermal;
        let gas = matches!(def.physics.matter_state, MatterState::Vacuum | MatterState::Gas | MatterState::Plasma);
        let celsius = |k: Option<f32>| k.map(|k| k - KELVIN);
        rows[b as usize] = Heat {
            capacity: if gas { 1.0 } else { (def.physics.density * t.capacity / REFERENCE_CAPACITY).clamp(0.25, 4.0) },
            conductance: if gas { 1.0 } else { t.conductivity.sqrt().clamp(0.25, 4.0) },
            gas,
            air: def.physics.matter_state == MatterState::Vacuum,
            melt_c: celsius(t.melt),
            boil_c: celsius(t.boil),
            cool_c: celsius(t.cool),
            ignite_c: celsius(t.ignite),
            fuel: t.fuel,
            fusion_k: if t.capacity > 0.0 { t.fusion / t.capacity } else { 0.0 },
        };
    }
    rows
});

impl Heat {
    const REFERENCE: Self = Self {
        capacity: 1.0,
        conductance: 1.0,
        gas: false,
        air: false,
        melt_c: None,
        boil_c: None,
        cool_c: None,
        ignite_c: None,
        fuel: 0.0,
        fusion_k: 0.0,
    };
}

pub fn of(b: BlockId) -> Heat {
    TABLE.get(b as usize).copied().unwrap_or(Heat::REFERENCE)
}

/// Energy per kelvin of difference crossing the face between `a` and `b`:
/// the two halves in series. Capped so six faces never overshoot either cell.
pub fn conductance(a: Heat, b: Heat) -> f32 {
    let series = 2.0 * a.conductance * b.conductance / (a.conductance + b.conductance);
    let film = if (a.air && !b.gas) || (b.air && !a.gas) { AIR_FILM } else { 1.0 };
    (BASE * series * film).min(0.16 * a.capacity.min(b.capacity))
}

/// Holds `c` at `point` while its latent store (`fuel`) fills with heat moving
/// in the direction of the change, giving heat back if it reverses. Returns
/// true when `latent_k` has gone in and the phase change completes; the cell
/// then sits at `point` with an empty store.
pub fn latent(c: &mut ThermalCell, point: f32, rising: bool, latent_k: f32) -> bool {
    let sign = if rising { 1.0 } else { -1.0 };
    let store = c.fuel + sign * (c.temperature - point);
    if store <= 0.0 {
        if c.fuel > 0.0 {
            c.fuel = 0.0;
            c.temperature = point + sign * store;
        }
        return false;
    }
    if store < latent_k {
        c.fuel = store;
        c.temperature = point;
        return false;
    }
    c.fuel = 0.0;
    c.temperature = point;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_orders_materials_like_their_physics() {
        let (water, stone, log, copper, air) =
            (of(BlockId::Water), of(BlockId::Stone), of(BlockId::Log), of(BlockId::CopperBlock), of(BlockId::Air));
        assert!(water.capacity > stone.capacity && stone.capacity > log.capacity);
        assert!(copper.conductance > stone.conductance && stone.conductance > log.conductance);
        assert!(air.gas && !water.gas);
        assert_eq!(of(BlockId::Ice).melt_c, Some(0.0));
        assert!((of(BlockId::Water).fusion_k - 79.8).abs() < 0.1);
        assert_eq!(of(BlockId::Log).ignite_c.map(f32::round), Some(260.0));
    }

    #[test]
    fn exchange_cannot_overshoot_with_six_faces() {
        for &a in ALL_BLOCK_IDS {
            for &b in ALL_BLOCK_IDS {
                let g = conductance(of(a), of(b));
                assert!(6.0 * g / of(a).capacity <= 1.0, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn latent_store_fills_then_converts_and_refunds_on_reversal() {
        let mut c = ThermalCell { temperature: 3.0, fuel: 0.0, gas: 0 };
        assert!(!latent(&mut c, 0.0, true, 10.0));
        assert_eq!((c.temperature, c.fuel), (0.0, 3.0));
        c.temperature = -5.0;
        assert!(!latent(&mut c, 0.0, true, 10.0));
        assert_eq!((c.temperature, c.fuel), (-2.0, 0.0));
        c.temperature = 12.0;
        assert!(latent(&mut c, 0.0, true, 10.0));
        assert_eq!((c.temperature, c.fuel), (0.0, 0.0));
        let mut w = ThermalCell { temperature: -4.0, fuel: 0.0, gas: 0 };
        assert!(!latent(&mut w, 0.0, false, 80.0));
        assert_eq!((w.temperature, w.fuel), (0.0, 4.0));
    }
}
