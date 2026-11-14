//! Persistent industrial air, deposition and containment accounting.
use super::{BlockId as B, DayNightCycle, World};
use glam::IVec3;
use std::collections::BTreeMap;
mod air;
pub mod config;
pub(crate) mod geometry;
mod persistence;
pub mod presentation;
pub mod review;
pub mod water;

pub const SPECIES: usize = 6;
pub const FUEL: usize = 3;
pub const STEAM: usize = 4;
pub const OXYGEN: usize = 5;
pub const MAX_CELLS: usize = 8192;
pub const MAX_DEPOSITS: usize = 8192;
pub type Mass = [u64; SPECIES];
/// Native voxel origin and span in native voxels (one or one metre).
pub type Key = (i32, i32, i32, i32);
pub fn pos(k: Key) -> IVec3 {
    IVec3::new(k.0, k.1, k.2)
}
pub fn total(m: &Mass) -> u64 {
    m.iter().sum()
}

#[derive(Default)]
pub struct State {
    pub air: BTreeMap<Key, Mass>,
    pub deposits: BTreeMap<(i32, i32, i32), Mass>,
    pub tick: u64,
    pub target: u64,
    pub transformed: Mass,
    pub packed: Mass,
    pub imported: Mass,
    pub withdrawn: Mass,
    pub blocked: u64,
    pub wind: [f32; 2],
    pub(crate) topology: BTreeMap<(i32, i32, i32), bool>,
    pub(crate) faces: BTreeMap<(Key, usize), Vec<(Key, f32)>>,
    pub(crate) ground: BTreeMap<Key, Option<IVec3>>,
    condensate_cursor: Option<Key>,
}

impl State {
    pub fn concentration(&self, p: IVec3, scale: f32) -> [f32; SPECIES] {
        let metre = (1.0 / scale).round() as i32;
        let o = p.div_euclid(IVec3::splat(metre)) * metre;
        let key = if self.air.contains_key(&(p.x, p.y, p.z, 1)) {
            (p.x, p.y, p.z, 1)
        } else {
            (o.x, o.y, o.z, metre)
        };
        let volume = (key.3 as f32 * scale).powi(3);
        self.air
            .get(&key)
            .map_or([0.0; SPECIES], |m| m.map(|v| v as f32 / volume))
    }
    pub fn contamination(&self, p: IVec3, scale: f32) -> u64 {
        let size = (1.0 / scale).round() as i32;
        let o = p.div_euclid(IVec3::splat(size)) * size;
        self.deposits.get(&(o.x, o.y, o.z)).map_or(0, total)
    }
}

pub fn tick(
    world: &mut World,
    _player: &crate::player::Player,
    day: &DayNightCycle,
    budget: engine_core::Budget,
    blocks: &mut std::collections::HashSet<IVec3>,
    chunks: &mut std::collections::HashSet<IVec3>,
) {
    let mut state = std::mem::take(&mut world.industry);
    state.target = state.target.saturating_add(1);
    let size = super::factory::step(world);
    geometry::invalidate(&mut state, &world.changed_cells, size);
    if state.topology.len() > MAX_CELLS * 8 {
        state.topology.clear();
        state.faces.clear();
    }
    if state.faces.len() > MAX_CELLS * 6 {
        state.faces.clear();
    }
    if state.ground.len() > MAX_CELLS * 2 {
        state.ground.clear();
    }
    if state.target.saturating_sub(state.tick) >= 4 && budget.has_time() {
        // Sampled at the world origin: the air grid is shared by every peer.
        let wind = super::wind_at(day, Some(world.sample_config()), glam::DVec2::ZERO).clamp_length_max(2.0);
        state.wind = wind.to_array();
        super::factory::industry::route(world, &mut state);
        air::step(world, &mut state, day, 1.0 / 30.0);
        water::condense(world, &mut state, budget, blocks, chunks);
        state.tick = state.tick.saturating_add(4);
        presentation::exposure(world, &state, 4.0 / 120.0);
    }
    world.industry = state;
    review::observe(world);
}

pub(crate) fn accept(
    world: &World,
    state: &mut State,
    p: IVec3,
    mass: &mut [u32; SPECIES],
) -> bool {
    let key = geometry::owner(world, state, p);
    if !geometry::open(world, p) || (!state.air.contains_key(&key) && state.air.len() >= MAX_CELLS)
    {
        state.blocked = state.blocked.saturating_add(1);
        return false;
    }
    let stock = state.air.entry(key).or_default();
    for i in 0..SPECIES {
        stock[i] += u64::from(mass[i]);
        mass[i] = 0;
    }
    true
}

pub fn dirty_soil(world: &World, p: IVec3) -> bool {
    world.industry.contamination(p, world.voxel_scale.meters()) > 1000
}
