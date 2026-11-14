//! A subgrid delta fan. Branches split around bars and drain one shared
//! depositional platform. The finished height field is sampled in metres.
use super::{SEA, tectonics::smooth};
use engine_core::hash::hash_u32;
use glam::DVec2;

const DX: f64 = 2.0;
const INLAND: f64 = 62.0;
const HALF: f64 = 192.0;
const WIDTH: usize = 193;

pub(super) struct Islands {
    cells: Box<[[f32; 4]]>,
    length: usize,
    pub share: f64,
}

#[derive(Clone, Copy)]
pub(super) struct Surface {
    pub level: f64,
    pub weight: f64,
    pub mud: f64,
    pub channel: f64,
}

#[derive(Clone, Copy)]
struct Channel {
    start: DVec2,
    end: DVec2,
    width: f64,
    bend: f64,
}

impl Channel {
    fn center(self, x: f64) -> f64 {
        let t = ((x - self.start.x) / (self.end.x - self.start.x)).clamp(0.0, 1.0);
        self.start.y + (self.end.y - self.start.y) * t + self.bend * 4.0 * t * (1.0 - t)
    }

    fn influence(self, p: DVec2) -> f64 {
        if p.x < self.start.x - 8.0 || p.x > self.end.x + 18.0 {
            return 0.0;
        }
        let span = self.end.x - self.start.x;
        let t = ((p.x - self.start.x) / span).clamp(0.0, 1.0);
        let slope = (self.end.y - self.start.y + 4.0 * self.bend * (1.0 - 2.0 * t)) / span;
        let side = (p.y - self.center(p.x)).abs() / (self.width * (1.0 + slope * slope).sqrt());
        (1.0 - smooth(0.75, 1.32, side))
            * smooth(self.start.x - 8.0, self.start.x + 3.0, p.x)
            * (1.0 - smooth(self.end.x, self.end.x + 18.0, p.x))
    }
}

impl Islands {
    pub fn build(
        outlet: DVec2,
        inlet_y: f64,
        width: f64,
        retained: f64,
        drift: f64,
        salt: u32,
        shelf: impl Fn(DVec2) -> Option<(f64, f64)>,
    ) -> Option<Self> {
        if retained < 62_000.0 {
            return None;
        }
        let unit = |key| f64::from(hash_u32(salt ^ key)) / f64::from(u32::MAX);
        let toe = (outlet.x + 108.0).clamp(162.0, 265.0);
        let reach_y = outlet.y.clamp(-78.0, 78.0);
        let fan_width = (0.45 * toe + 14.0 + 14.0 * drift.abs()).clamp(89.0, 134.0);
        let fork = DVec2::new(-12.0 + 16.0 * unit(12), inlet_y * 0.70);
        let trunk = Channel {
            start: DVec2::new(-45.0, inlet_y),
            end: fork,
            width: width.clamp(11.0, 36.0) * 0.84,
            bend: (unit(19) - 0.5) * 9.0,
        };
        let mut routes = Vec::with_capacity(18);
        let mut mouths = Vec::with_capacity(4);
        routes.push(trunk);
        for (side, key) in [(-1.0, 31), (1.0, 71)] {
            let axis = DVec2::new(
                toe * (0.35 + 0.12 * unit(key)),
                reach_y * 0.42 + side * fan_width * (0.29 + 0.07 * unit(key ^ 5)),
            );
            let arm_width = (width * (0.45 + 0.12 * unit(key ^ 16))).clamp(8.0, 17.0);
            routes.push(Channel {
                start: fork,
                end: axis,
                width: arm_width,
                bend: -side * (4.0 + 8.0 * unit(key ^ 17)),
            });
            for (wing, branch) in [(0.22, key ^ 27), (0.80, key ^ 48)] {
                let end = DVec2::new(
                    toe + (unit(branch) - 0.35) * 30.0,
                    reach_y + side * fan_width * (wing + 0.09 * unit(branch ^ 18)),
                );
                let main = Channel {
                    start: axis,
                    end,
                    width: (arm_width * if wing < 0.5 { 0.68 } else { 0.49 }).max(4.6),
                    bend: side * (unit(branch ^ 3) - 0.42) * 22.0,
                };
                routes.push(main);
                mouths.push(main);
                // A shallow secondary route divides and rejoins its parent.
                // This cuts a small bar island within the shared fan.
                let x = axis.x + (end.x - axis.x) * (0.12 + unit(branch ^ 25) * 0.12);
                let stop = axis.x + (end.x - axis.x) * (0.76 + unit(branch ^ 31) * 0.10);
                routes.push(Channel {
                    start: DVec2::new(x, main.center(x)),
                    end: DVec2::new(stop, main.center(stop)),
                    width: 2.4 + 1.3 * unit(branch ^ 45),
                    bend: side
                        * if wing < 0.5 { -1.0 } else { 1.0 }
                        * (main.width + 12.0 + 6.0 * unit(branch ^ 12)),
                });
            }
        }
        mouths.sort_by(|a, b| a.end.y.total_cmp(&b.end.y));
        // Small interdistributary creeks give the interior of the fan
        // narrow passages as well as the large outward-flowing channels.
        for (i, pair) in mouths.windows(2).enumerate() {
            let start_x = pair[0].start.x.max(pair[1].start.x);
            let key = 140 + i as u32 * 19;
            for fraction in [0.31 + 0.13 * unit(key), 0.60 + 0.09 * unit(key ^ 5)] {
                let x0 = start_x + (toe - start_x) * fraction;
                let x1 = x0 + 17.0 + 10.0 * unit(key ^ 11);
                let (a, b) = if i == 1 {
                    (pair[1], pair[0])
                } else {
                    (pair[0], pair[1])
                };
                routes.push(Channel {
                    start: DVec2::new(x0, a.center(x0)),
                    end: DVec2::new(x1, b.center(x1)),
                    width: 2.0 + 1.05 * unit(key ^ 15),
                    bend: (unit(key ^ 22) - 0.5) * 17.0,
                });
            }
        }
        let length = ((toe + INLAND + 53.0) / DX).ceil() as usize;
        let mut cells = Vec::with_capacity(length * WIDTH);
        let mut added = 0.0;
        for row in 0..WIDTH {
            for col in 0..length {
                let p = DVec2::new(col as f64 * DX - INLAND, row as f64 * DX - HALF);
                let Some((bed, mud)) = shelf(p) else {
                    cells.push([(SEA - 2.2) as f32, 0.0, 0.5, 0.0]);
                    continue;
                };
                let t = (p.x / toe).clamp(0.0, 1.2);
                let middle = reach_y * t + inlet_y * (1.0 - t.min(1.0));
                let spread = 27.0 + fan_width * smooth(-0.06, 0.65, t);
                let lateral = (p.y - middle) / spread;
                let lip = 0.93
                    + 0.095 * (lateral * 8.3 + 3.0 * unit(81)).sin()
                    + 0.052 * (lateral * 16.8 - 2.4 * drift).cos();
                let fan = smooth(-32.0, 35.0, p.x)
                    * (1.0 - smooth(lip - 0.19, lip + 0.13, t))
                    * (1.0 - smooth(0.61, 1.10, lateral.abs()));
                let shelf_fit =
                    smooth(SEA - 13.0, SEA - 8.0, bed) * (1.0 - smooth(SEA + 4.5, SEA + 8.5, bed));
                let channel = routes.iter().map(|c| c.influence(p)).fold(0.0f64, f64::max);
                let fill = (fan * 1.64).min(1.0) * shelf_fit;
                let ridge = (p.y * 0.15 + t * 8.0).sin() * (p.x * 0.092 - lateral).cos();
                let platform = SEA - 2.2 + fan * (4.88 + 0.27 * ridge);
                let mouth = smooth(0.0, 0.19, fan + 1.0 - smooth(0.69, 1.03, t));
                let cut = channel * smooth(-46.0, -10.0, p.x) * mouth;
                let level = platform + (SEA - 2.4 - platform) * cut;
                let weight = fill.max(cut * shelf_fit);
                added += (level - bed).max(0.0) * weight * DX * DX;
                let fine = (mud * 0.66 + (1.0 - cut) * 0.31).clamp(0.0, 1.0);
                cells.push([
                    level as f32,
                    weight as f32,
                    fine as f32,
                    (cut * shelf_fit) as f32,
                ]);
            }
        }
        let strength = (retained * 0.52 / added.max(1.0)).min(1.0);
        if strength < 0.48 {
            return None;
        }
        for c in &mut cells {
            // A limited sediment supply reduces fill. It cannot close the
            // surveyed gaps between deposits or block an established branch.
            c[1] = (c[1] * strength as f32).max(c[3]);
        }
        Some(Self {
            cells: cells.into_boxed_slice(),
            length,
            share: (added * strength / retained).clamp(0.0, 0.52),
        })
    }

    pub fn sample(&self, p: DVec2) -> Option<Surface> {
        let x = (p.x + INLAND) / DX;
        let y = (p.y + HALF) / DX;
        if x < 0.0 || y < 0.0 || x >= (self.length - 1) as f64 || y >= (WIDTH - 1) as f64 {
            return None;
        }
        let (ix, iy) = (x as usize, y as usize);
        let i = iy * self.length + ix;
        let u = x - ix as f64;
        let v = y - iy as f64;
        let ids = [i, i + 1, i + self.length, i + self.length + 1];
        let factors = [(1.0 - u) * (1.0 - v), u * (1.0 - v), (1.0 - u) * v, u * v];
        let blend = |field| {
            ids.into_iter()
                .zip(factors)
                .map(|(id, factor)| f64::from(self.cells[id][field]) * factor)
                .sum::<f64>()
        };
        let weight = blend(1);
        (weight > 0.0).then(|| Surface {
            level: blend(0),
            weight,
            mud: blend(2),
            channel: blend(3),
        })
    }

    pub fn checksum(&self, mut hash: u64) -> u64 {
        for &value in self.cells.iter().flatten() {
            hash = (hash ^ u64::from(value.to_bits())).wrapping_mul(0x100000001b3);
        }
        (hash ^ self.share.to_bits()).wrapping_mul(0x100000001b3)
    }
}
