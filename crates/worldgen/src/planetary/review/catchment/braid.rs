//! An offline mobile-bed reach. Downstream cellular routing follows
//! Murray and Paola's reduced model, with a sediment budget at open ends.
pub const SPACING_M: f64 = 4.0;
pub const ACROSS: usize = 128;
pub const DOWNSTREAM: usize = 450;
const GRADE: f64 = 0.0026;
const Q_IN: f64 = 72.0;
const BEDLOAD_IN: f64 = 0.42;

pub struct Reach {
    pub start_m: Vec<f32>,
    pub bed_m: Vec<f32>,
    pub flow_m3_s: Vec<f32>,
    pub sediment_residual_m3: f64,
    pub outlet_flow_m3_s: f64,
}

pub fn evolve(seed: u32, datum_m: f64) -> Reach {
    let len = ACROSS * DOWNSTREAM;
    let mut state = Bed {
        height: (0..len)
            .map(|i| {
                let x = (i % ACROSS) as f64 - (ACROSS - 1) as f64 * 0.5;
                let z = (i / ACROSS) as f64 * SPACING_M;
                let bank = 3.5 * ((x.abs() - 47.0) / 16.0).clamp(0.0, 1.0).powi(2);
                let jitter = f64::from(hash(seed ^ i as u32)) / f64::from(u32::MAX) - 0.5;
                datum_m - z * GRADE + bank + 0.084 * jitter + 0.36 * bed_roughness(seed, x, z)
            })
            .collect(),
        substrate: vec![0.0; len],
        water: vec![0.0; len],
        load: vec![0.0; len],
        change: vec![0.0; len],
    };
    let start = state.height.clone();
    for (i, h) in state.substrate.iter_mut().enumerate() {
        *h = start[i]
            - 1.24
            - 0.52
                * bed_roughness(
                    seed.wrapping_add(23),
                    (i % ACROSS) as f64,
                    (i / ACROSS) as f64 * SPACING_M,
                );
    }
    let mut exported = 0.0;
    const PASSES: usize = 2400;
    for _ in 0..PASSES {
        exported += state.route(true);
        for (h, dz) in state.height.iter_mut().zip(&state.change) {
            *h += dz;
        }
    }
    state.route(false);
    let moved = state
        .height
        .iter()
        .zip(&start)
        .map(|(a, b)| a - b)
        .sum::<f64>()
        * SPACING_M.powi(2);
    let outlet = state.water[len - ACROSS..].iter().sum();
    Reach {
        start_m: start.into_iter().map(|v| v as f32).collect(),
        bed_m: state.height.into_iter().map(|v| v as f32).collect(),
        flow_m3_s: state.water.into_iter().map(|v| v as f32).collect(),
        sediment_residual_m3: BEDLOAD_IN * PASSES as f64 - exported - moved,
        outlet_flow_m3_s: outlet,
    }
}

struct Bed {
    height: Vec<f64>,
    substrate: Vec<f64>,
    water: Vec<f64>,
    load: Vec<f64>,
    change: Vec<f64>,
}

impl Bed {
    fn route(&mut self, move_bed: bool) -> f64 {
        let cell_area = SPACING_M.powi(2);
        self.water.fill(0.0);
        self.load.fill(0.0);
        self.change.fill(0.0);
        let inlet: [f64; ACROSS] = std::array::from_fn(|x| {
            (-((x as f64 - (ACROSS - 1) as f64 * 0.5) / 7.8).powi(2)).exp()
        });
        let total: f64 = inlet.iter().sum();
        for (x, &weight) in inlet.iter().enumerate() {
            self.water[x] = Q_IN * weight / total;
            self.load[x] = BEDLOAD_IN * weight / total;
        }
        for z in 0..DOWNSTREAM - 1 {
            for x in 0..ACROSS {
                let i = z * ACROSS + x;
                let q = self.water[i];
                let incoming = self.load[i];
                let targets = x.saturating_sub(1)..=(x + 1).min(ACROSS - 1);
                let mut edges = [(0, 0.0, 0.0); 3];
                let mut count = 0;
                let mut downhill: f64 = 0.0;
                let depth_allowance = 0.018 * q.powf(0.46);
                for nx in targets {
                    let j = (z + 1) * ACROSS + nx;
                    let distance = SPACING_M
                        * if x == nx {
                            1.0
                        } else {
                            std::f64::consts::SQRT_2
                        };
                    let slope = (self.height[i] - self.height[j]) / distance;
                    downhill = downhill.max(slope + depth_allowance / SPACING_M);
                    edges[count] = (j, slope, 0.0);
                    count += 1;
                }
                let edges = &mut edges[..count];
                let mut sum = 0.0;
                for (_, slope, weight) in edges.iter_mut() {
                    *weight = if downhill > 0.0 {
                        (*slope + depth_allowance / SPACING_M).max(0.0).sqrt()
                    } else {
                        1.0 / (slope.abs() + 0.0006)
                    };
                    sum += *weight;
                }
                let mut capacity = [0.0; 3];
                let mut total_capacity = 0.0;
                for (k, &(j, slope, weight)) in edges.iter().enumerate() {
                    let share = q * weight / sum;
                    self.water[j] += share;
                    let power = share * (slope + 2.8 * GRADE).max(GRADE * 0.2);
                    capacity[k] = 215.0 * power.powf(2.35);
                    total_capacity += capacity[k];
                }
                if !move_bed {
                    continue;
                }
                // Limit the morphodynamic step while preserving sediment
                // that could not deposit or be entrained during that step.
                let limit = 0.011 * cell_area;
                let bedload = (self.height[i] - self.substrate[i]).max(0.0) * cell_area;
                let outgoing = total_capacity
                    .clamp((incoming - limit).max(0.0), incoming + limit.min(bedload));
                self.change[i] += (incoming - outgoing) / cell_area;
                for (k, &(j, _, weight)) in edges.iter().enumerate() {
                    let share = if total_capacity > 0.0 {
                        capacity[k] / total_capacity
                    } else {
                        weight / sum
                    };
                    self.load[j] += outgoing * share;
                }
                let mut remaining = (bedload + incoming - outgoing).max(0.0);
                for nx in x.saturating_sub(1)..=(x + 1).min(ACROSS - 1) {
                    let j = z * ACROSS + nx;
                    let fall = (self.height[i] - self.height[j]).max(0.0);
                    let slope = fall / SPACING_M;
                    let creep = 0.011 * cell_area * (slope - 0.16).max(0.0) * (q / 3.0).min(1.0);
                    let lateral = (creep + 0.085 * outgoing * slope.min(0.70)).min(remaining);
                    remaining -= lateral;
                    self.change[i] -= lateral / cell_area;
                    self.change[j] += lateral / cell_area;
                }
            }
        }
        self.load[self.load.len() - ACROSS..].iter().sum()
    }
}

fn bed_roughness(seed: u32, x: f64, z: f64) -> f64 {
    let a = x / 8.0 + z / 222.0;
    let b = z / 91.0 - x / 31.0;
    let ix = a.floor() as i32;
    let iz = b.floor() as i32;
    let u = a - a.floor();
    let v = b - b.floor();
    let sample = |i: i32, j: i32| {
        f64::from(hash(
            seed ^ (i as u32).wrapping_mul(0x9e3779b9) ^ (j as u32).wrapping_mul(0x85ebca6b),
        )) / f64::from(u32::MAX)
            - 0.5
    };
    (sample(ix, iz) * (1.0 - u) + sample(ix + 1, iz) * u) * (1.0 - v)
        + (sample(ix, iz + 1) * (1.0 - u) + sample(ix + 1, iz + 1) * u) * v
}

fn hash(mut n: u32) -> u32 {
    n = (n ^ (n >> 16)).wrapping_mul(0x7feb352d);
    n = (n ^ (n >> 15)).wrapping_mul(0x846ca68b);
    n ^ (n >> 16)
}
