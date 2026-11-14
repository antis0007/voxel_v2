//! Editable lift habitats use ordinary chunks, cylinder motion and finite gas.
//! A habitat registered since the wing landed adds a generated station wing
//! (`engine_worldgen::station`) past the drum's far endcap.
use crate::{player::Player, world::{World, BlockId as B}};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
mod geometry;
mod pressure;
mod airlock;
pub mod residents;
pub mod persistence;
#[cfg(test)]
mod tests;
pub use geometry::{acceleration, allows_cell, protects, register, Grid};
pub use airlock::{interact, prompt};

/// Local metre coordinates of the cabin-side and station-side hatch panels.
pub const HATCH_X: [i32; 2] = [4, 10];
pub const THROAT_X_M: f32 = 3.0;
pub(crate) const MIN_BREATHING_PRESSURE_PA: f64 = 30_000.0;

#[derive(Deserialize)]
pub struct Config {
    radius_m: i32, length_m: i32, shell_m: i32,
    pressure_pa: f64, tank_volume_m3: f64, tank_pressure_pa: f64,
    pub minimum_breach_radius_m: f32, pub hull_cost_per_m3: f32,
}
pub fn config() -> &'static Config {
    static CONFIG: std::sync::LazyLock<Config> = std::sync::LazyLock::new(|| {
        ron::from_str(include_str!("../../../../../assets/station_habitat.ron"))
            .unwrap_or(Config { radius_m: 6, length_m: 16, shell_m: 2,
                pressure_pa: 101325.0, tank_volume_m3: 12.0, tank_pressure_pa: 400000.0,
                minimum_breach_radius_m: 6.0, hull_cost_per_m3: 100.0 })
    });
    &CONFIG
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub habitats: Vec<Habitat>,
    #[serde(default)]
    pub cargo: super::cargo::State,
    #[serde(skip)]
    pub exposure_s: f32,
    #[serde(skip)]
    steps: u32,
    #[serde(skip)]
    building: Option<(IVec3, Grid)>,
}

#[derive(Serialize, Deserialize)]
pub struct Habitat {
    pub origin: IVec3,
    pub berth: Vec3,
    pub cell: glam::IVec2,
    pub build_cursor: usize,
    pub ready: bool,
    pub gas: Vec<[f64; 2]>,
    pub tank: [f64; 2],
    pub cabin: [f64; 2],
    pub escaped: [f64; 2],
    /// -1 idle; 0 cycling toward cabin; 1 cycling toward habitat.
    pub cycle: i8,
    pub doors_open: [bool; 2],
    /// Gas grid bounds; habitats saved before the wing keep the drum box.
    #[serde(default)]
    pub grid: Grid,
    /// Seed of the generated wing; `None` for a drum-only habitat.
    #[serde(default)]
    pub wing: Option<u32>,
    /// Planned wing inhabitants, filled once; dead ones are removed.
    #[serde(default)]
    pub residents: Vec<residents::Resident>,
    #[serde(default)]
    pub populated: bool,
    /// Wing under construction on a habitat that already held air.
    #[serde(default)]
    pub retrofit: bool,
    #[serde(skip)]
    volumes: Vec<f64>,
    #[serde(skip)]
    faces: Vec<[f64; 3]>,
    #[serde(skip)]
    dirty: BTreeSet<usize>,
}

pub fn note_edit(state: &mut State, p: IVec3, scale: f32) {
    let m = ((p.as_vec3() + Vec3::splat(0.5)) * scale).floor().as_ivec3();
    for h in &mut state.habitats {
        for q in [m, m - IVec3::X, m - IVec3::Y, m - IVec3::Z] {
            if let Some(i) = h.index(q - h.origin) { h.dirty.insert(i); }
        }
    }
}

pub fn readings(world: &World, player: &Player) -> Option<(f64, f64)> {
    if let Some(r) = world.orbital.express.ride.filter(|r| !r.deck) {
        let crown = (r.berth_m + glam::DVec3::Y * r.profile.length_m).as_vec3();
        if let Some(h) = world.orbital.habitats.habitats.iter().find(|h| h.origin.as_vec3().distance(crown) < 1.0) {
            return Some((engine_sim::atmosphere::pressure(h.cabin, cabin_volume()), h.cabin[0] / cabin_volume()));
        }
    }
    readings_at(world, player.precise_eye(world.voxel_scale) * f64::from(world.voxel_scale.meters()))
}

pub(crate) fn readings_at(world: &World, position_m: glam::DVec3) -> Option<(f64, f64)> {
    let mut near_habitat = false;
    for h in &world.orbital.habitats.habitats {
        let local = (position_m - h.origin.as_dvec3()).as_vec3();
        if !h.ready || local.length() > 200.0 { continue; }
        near_habitat = true;
        let Some(i) = h.index(local.floor().as_ivec3()) else { continue; };
        let volume = h.volumes.get(i).copied().unwrap_or(1.0).max(0.001);
        let gas = h.gas.get(i).copied().unwrap_or([0.0; 2]);
        return Some((engine_sim::atmosphere::pressure(gas, volume), gas[0] / volume));
    }
    near_habitat.then_some((0.0, 0.0))
}

pub fn tick(world: &mut World, player: &Player) {
    if world.ca_view.is_some() { return; }
    let mut state = std::mem::take(&mut world.orbital.habitats);
    for h in &mut state.habitats { geometry::retrofit(world.seed, h); }
    // Keep the bounds registered while ordinary voxel edits build the module.
    for index in 0..state.habitats.len() {
        world.orbital.habitats.building = Some((state.habitats[index].origin, state.habitats[index].grid));
        geometry::build(world, &mut state.habitats[index]);
    }
    state.steps = state.steps.wrapping_add(1);
    for h in &mut state.habitats {
        world.orbital.habitats.building = Some((h.origin, h.grid));
        if !h.ready { continue; }
        pressure::refresh(world, h);
        if state.steps.is_multiple_of(12) {
            let docked = world.orbital.express.ride.is_some_and(|r| r.ascending && r.docked()
                && (r.cabin_m().as_vec3() - h.origin.as_vec3()).length() < 1.0);
            pressure::step(h, 0.1, docked);
            airlock::step(world, h, player);
            residents::populate(h);
            residents::advance(world, h, player);
        }
    }
    world.orbital.habitats = state;
    if let Some((p, oxygen)) = readings(world, player) {
        if p < MIN_BREATHING_PRESSURE_PA || oxygen < 12_000.0 {
            world.orbital.habitats.exposure_s += crate::simulation::STEP_SECONDS;
            if world.orbital.habitats.exposure_s >= 1.0 {
                world.orbital.habitats.exposure_s -= 1.0;
                if !world.rules.mode.creative_tools() {
                    world.adventure.environment_damage(6.0, crate::world::adventure::death::Cause::Suffocation);
                }
            }
        } else { world.orbital.habitats.exposure_s = 0.0; }
    } else { world.orbital.habitats.exposure_s = 0.0; }
}

pub fn cabin_volume() -> f64 {
    let width = super::express::HALF_WIDTH_M as f64 * 2.0;
    width * width * super::express::HEIGHT_M as f64
}

pub fn ordinary_spell_allowed(world: &World, cell: IVec3) -> bool {
    !protects(world, cell) || world.get(cell) != B::SteelBlock
}
