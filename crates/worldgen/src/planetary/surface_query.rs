//! A single immutable World/direction query. Nothing survives the calling node.
use super::{World, WorldSampleConfig, geo};
use glam::DVec3;
use std::cell::OnceCell;

pub(super) struct SurfaceQuery<'a> {
    world: &'a World,
    dir: DVec3,
    stencil: OnceCell<([usize; 4], [f64; 4])>,
    edifice: OnceCell<f64>,
    volcanic: OnceCell<f64>,
}

impl<'a> SurfaceQuery<'a> {
    pub(super) fn new(world: &'a World, dir: DVec3) -> Self {
        Self { world, dir, stencil: OnceCell::new(), edifice: OnceCell::new(),
            volcanic: OnceCell::new() }
    }
    pub(super) fn direction(&self) -> DVec3 { self.dir }
    pub(super) fn stencil(&self) -> ([usize; 4], [f64; 4]) {
        *self.stencil.get_or_init(|| self.world.mesh.stencil(self.dir))
    }
    fn sample(&self, field: &[f64]) -> f64 {
        let (ids, weights) = self.stencil();
        (0..4).map(|i| field[ids[i]] * weights[i]).sum()
    }
    pub(super) fn edifice_lift(&self) -> f64 {
        *self.edifice.get_or_init(|| {
            if self.world.edifice_lift.is_empty() { 0.0 } else { self.sample(&self.world.edifice_lift) }
        })
    }
    pub(super) fn volcanic_base(&self) -> f64 {
        *self.volcanic.get_or_init(|| {
            let bare = self.sample(&self.world.bare_height);
            if self.world.edifice_lift.is_empty() { return bare; }
            bare + self.world.volcanoes.edifice(self.dir).lift - self.edifice_lift()
        })
    }
    pub(super) fn volcano_shape(&self) -> crate::volcano::Shaped {
        // Craters query vent directions, not necessarily the caller's direction.
        self.world.volcanoes.shape(self.dir, self.volcanic_base(), |d| self.world.volcanic_base(d))
    }
    pub(super) fn ground_base(&self, cfg: &WorldSampleConfig, footprint: f32,
        volcanic: crate::volcano::Shaped) -> geo::Ground
    {
        self.world.ground_base_prepared(cfg, self, footprint, volcanic, None)
    }
    pub(super) fn history_strata(&self, cfg: &WorldSampleConfig) -> Option<super::drift::history::RockColumn> {
        self.world.history_strata_prepared(cfg, self)
    }
    pub(super) fn collision(&self) -> f64 { self.sample(&self.world.collision) }
    pub(super) fn sediment_cover(&self) -> f64 { self.sample(&self.world.sediment_cover) }
    pub(super) fn fault_damage(&self) -> f64 { self.sample(&self.world.fault_damage) }
}

#[cfg(test)]
pub(in crate::planetary) mod tests;
