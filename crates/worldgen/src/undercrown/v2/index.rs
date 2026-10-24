//! A column grid over the layout: each 8 m xz cell lists the features,
//! spaces and passages whose boxes touch it, in authored order, so the
//! sampler tests a handful of candidates instead of every one and keeps
//! exactly the same first-hit order.
use glam::Vec3;

/// Cell edge, metres.
const SIZE: f32 = 8.0;
/// Slack round every box, so a point on a cell edge never misses and the
/// cavern formations, which keep 1.5 m off every feature, see them all.
const SLACK: f32 = 2.0;

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub features: Vec<u16>,
    pub spaces: Vec<u16>,
    pub tunnels: Vec<u16>,
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    x0: f32,
    z0: f32,
    nx: i32,
    nz: i32,
    cells: Vec<Cell>,
    empty: Cell,
}

impl Index {
    /// Index the three lists, each given as boxes in authored order.
    pub fn build(min: Vec3, max: Vec3, features: &[(Vec3, Vec3)], spaces: &[(Vec3, Vec3)], tunnels: &[(Vec3, Vec3)]) -> Self {
        let nx = ((max.x - min.x) / SIZE).ceil().max(1.0) as i32;
        let nz = ((max.z - min.z) / SIZE).ceil().max(1.0) as i32;
        let mut index = Self { x0: min.x, z0: min.z, nx, nz, cells: vec![Cell::default(); (nx * nz) as usize], empty: Cell::default() };
        for (list, boxes) in [(0, features), (1, spaces), (2, tunnels)] {
            for (i, &(lo, hi)) in boxes.iter().enumerate() {
                let (gx0, gz0) = index.grid(lo.x - SLACK, lo.z - SLACK);
                let (gx1, gz1) = index.grid(hi.x + SLACK, hi.z + SLACK);
                for gz in gz0..=gz1 {
                    for gx in gx0..=gx1 {
                        let cell = &mut index.cells[(gz * nx + gx) as usize];
                        let ids = match list {
                            0 => &mut cell.features,
                            1 => &mut cell.spaces,
                            _ => &mut cell.tunnels,
                        };
                        ids.push(i as u16);
                    }
                }
            }
        }
        index
    }

    fn grid(&self, x: f32, z: f32) -> (i32, i32) {
        let gx = ((x - self.x0) / SIZE).floor() as i32;
        let gz = ((z - self.z0) / SIZE).floor() as i32;
        (gx.clamp(0, self.nx - 1), gz.clamp(0, self.nz - 1))
    }

    /// Candidates at a point inside the layout box.
    pub fn at(&self, m: Vec3) -> &Cell {
        let gx = ((m.x - self.x0) / SIZE).floor() as i32;
        let gz = ((m.z - self.z0) / SIZE).floor() as i32;
        if gx < 0 || gz < 0 || gx >= self.nx || gz >= self.nz {
            return &self.empty;
        }
        &self.cells[(gz * self.nx + gx) as usize]
    }
}
