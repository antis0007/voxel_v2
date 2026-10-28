//! Square cell grids over the planet face.

pub const SQRT2: f64 = std::f64::consts::SQRT_2;

/// Eight neighbour offsets with their distance in cells.
pub const NEIGHBOURS: [(i32, i32, f64); 8] = [
    (-1, 0, 1.0),
    (1, 0, 1.0),
    (0, -1, 1.0),
    (0, 1, 1.0),
    (-1, -1, SQRT2),
    (1, -1, SQRT2),
    (-1, 1, SQRT2),
    (1, 1, SQRT2),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub n: usize,
    pub cell_m: f64,
    /// World position of the corner of cell (0, 0).
    pub min_m: [f64; 2],
}

impl Grid {
    pub fn len(&self) -> usize {
        self.n * self.n
    }

    /// Centre of cell `i` in world metres.
    pub fn at(&self, i: usize) -> [f64; 2] {
        let (x, z) = (i % self.n, i / self.n);
        [
            self.min_m[0] + (x as f64 + 0.5) * self.cell_m,
            self.min_m[1] + (z as f64 + 0.5) * self.cell_m,
        ]
    }

    /// Neighbours of `i` inside the grid with their distance in metres.
    pub fn neighbours(&self, i: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let (x, z) = ((i % self.n) as i32, (i / self.n) as i32);
        let n = self.n as i32;
        NEIGHBOURS.iter().filter_map(move |&(dx, dz, d)| {
            let (a, b) = (x + dx, z + dz);
            (a >= 0 && b >= 0 && a < n && b < n).then(|| ((b * n + a) as usize, d * self.cell_m))
        })
    }

    pub fn on_border(&self, i: usize) -> bool {
        let (x, z) = (i % self.n, i / self.n);
        x == 0 || z == 0 || x + 1 == self.n || z + 1 == self.n
    }

    /// Cell coordinates of a world point, clamped so a 2x2 stencil stays inside.
    pub fn cell(&self, wx: f64, wz: f64) -> (usize, usize, f64, f64) {
        let top = (self.n - 1) as f64 - 1e-9;
        let u = ((wx - self.min_m[0]) / self.cell_m - 0.5).clamp(0.0, top);
        let v = ((wz - self.min_m[1]) / self.cell_m - 0.5).clamp(0.0, top);
        let (ux, vz) = (u.floor(), v.floor());
        (ux as usize, vz as usize, u - ux, v - vz)
    }

    pub fn bilinear(&self, f: &[f32], wx: f64, wz: f64) -> f64 {
        let (x, z, tx, tz) = self.cell(wx, wz);
        let i = z * self.n + x;
        let a = f[i] as f64;
        let b = f[i + 1] as f64;
        let c = f[i + self.n] as f64;
        let d = f[i + self.n + 1] as f64;
        let top = a + (b - a) * tx;
        let bottom = c + (d - c) * tx;
        top + (bottom - top) * tz
    }

    pub fn contains(&self, wx: f64, wz: f64) -> bool {
        let span = self.n as f64 * self.cell_m;
        wx >= self.min_m[0] && wz >= self.min_m[1] && wx < self.min_m[0] + span && wz < self.min_m[1] + span
    }
}
