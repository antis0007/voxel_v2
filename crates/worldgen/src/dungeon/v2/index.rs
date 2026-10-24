//! Coarse column grid of rooms and corridors, built once per complex.
use super::{Corr, Room, WALL};
use glam::Vec3;

/// Coarse column grid of room and corridor indices, so a voxel tests only the
/// pieces near it.
#[derive(Clone, Debug, Default)]
pub struct Index {
    x0: f32,
    z0: f32,
    nx: usize,
    nz: usize,
    cells: Vec<Vec<u16>>,
}

const INDEX_CELL: f32 = 8.0;
pub(super) const CORR_BIT: u16 = 0x8000;
/// How far a door's clear space reaches into a room, metres.
const THROAT: f32 = 10.0;

impl Index {
    pub(super) fn build(min: Vec3, max: Vec3, rooms: &[Room], corrs: &[Corr]) -> Self {
        let nx = (((max.x - min.x) / INDEX_CELL).ceil() as usize).max(1);
        let nz = (((max.z - min.z) / INDEX_CELL).ceil() as usize).max(1);
        let mut index = Self { x0: min.x, z0: min.z, nx, nz, cells: vec![Vec::new(); nx * nz] };
        let boxes = rooms
            .iter()
            .enumerate()
            .map(|(i, r)| (i as u16, r.x0 - WALL, r.x1 + WALL, r.z0 - WALL, r.z1 + WALL))
            .chain(corrs.iter().enumerate().map(|(i, c)| {
                // Doorways and throats reach `THROAT` metres past a corridor's ends.
                let (px, pz) = if c.along_x { (THROAT, 1.0) } else { (1.0, THROAT) };
                (i as u16 | CORR_BIT, c.x0 - px, c.x1 + px, c.z0 - pz, c.z1 + pz)
            }));
        for (id, x0, x1, z0, z1) in boxes {
            let (i0, i1) = (index.col(x0, index.x0, nx), index.col(x1, index.x0, nx));
            let (j0, j1) = (index.col(z0, index.z0, nz), index.col(z1, index.z0, nz));
            for j in j0..=j1 {
                for i in i0..=i1 {
                    index.cells[j * nx + i].push(id);
                }
            }
        }
        index
    }
    fn col(&self, v: f32, origin: f32, n: usize) -> usize {
        (((v - origin) / INDEX_CELL).floor().max(0.0) as usize).min(n - 1)
    }
    pub(super) fn at(&self, x: f32, z: f32) -> &[u16] {
        let i = self.col(x, self.x0, self.nx);
        let j = self.col(z, self.z0, self.nz);
        &self.cells[j * self.nx + i]
    }
}
