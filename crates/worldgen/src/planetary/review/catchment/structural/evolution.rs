//! Long-term uplift, detachment, sediment transport and slope failure.
use super::RockPackage;
use super::mesh::Mesh;
use crate::geo::grid::Grid;
use glam::DVec2;

pub(super) struct Landscape {
    pub grid: Grid,
    pub surface: Vec<f32>,
    pub cover: Vec<f32>,
    pub water: [f64; 3],
    pub sediment: [f64; 5],
    pub wind: [f64; 3],
    pub upland_rock_m3: f64,
    pub process_m3: [f64; 3],
}

const RAIN_EVENTS: [f64; 6] = [0.70, 0.55, 2.00, 1.05, 0.65, 1.05];
const COARSE_TECTONIC_YEARS: f64 = 30_000.0;
const WEATHERING_SEASONS: [f64; 6] = [1.55, 1.85, 0.40, 0.55, 0.75, 0.90];

pub(super) fn evolve(
    package: RockPackage,
    span: f64,
    initial: impl Fn(f64, f64) -> f64,
    noise: impl Fn(f64, f64, f64) -> f64,
    precipitation: impl Fn(f64, f64) -> f64,
    wind: Option<DVec2>,
    shortening: impl Fn(f64, f64) -> f64,
    upland_runoff: bool,
) -> Landscape {
    let mut previous: Option<Stage> = None;
    let margin = 512.0;
    let extent = span + 2.0 * margin;
    for (spacing, years, passes, history_years) in [
        (32.0, 180.0, 36, 486_000.0),
        (16.0, 73.5, 48, 324_000.0),
        (8.0, 32.0, 48, 270_000.0),
    ] {
        let grid = Grid {
            n: (extent / spacing) as usize,
            cell_m: spacing,
            min_m: [-margin; 2],
        };
        let mut stage = Stage::new(
            grid,
            package,
            span,
            &initial,
            &noise,
            &precipitation,
            wind,
            &shortening,
            previous.as_ref(),
        );
        for pass in 0..passes {
            let fraction = (f64::from(pass + 1) / 28.0).min(1.0);
            stage.advance(package, years, history_years / f64::from(passes), fraction, pass as usize, upland_runoff);
        }
        stage.sediment[4] = stage.soil_volume();
        previous = Some(stage);
    }
    previous.unwrap().finish()
}

struct Stage {
    grid: Grid,
    mesh: Mesh,
    initial: Vec<f32>,
    rock: Vec<f64>,
    soil: Vec<f64>,
    surface: Vec<f64>,
    fixed: Vec<bool>,
    outlet_drop: Vec<f64>,
    outlet_already_lowered: bool,
    uplift: Vec<f64>,
    precipitation: Vec<f64>,
    runoff: Vec<f64>,
    flux: Vec<f64>,
    fine_load: Vec<f64>,
    coarse_heading: Vec<DVec2>,
    water: [f64; 3],
    sediment: [f64; 5],
    wind: Option<(DVec2, Vec<usize>)>,
    wind_budget: [f64; 3],
    tectonic_lift: Vec<f64>,
    joints: Vec<f64>,
    scarp_datum: Vec<f64>,
    upland_rock_m3: f64,
    process_m3: [f64; 3],
}

impl Stage {
    fn new(
        grid: Grid,
        package: RockPackage,
        span: f64,
        initial: &impl Fn(f64, f64) -> f64,
        noise: &impl Fn(f64, f64, f64) -> f64,
        precipitation: &impl Fn(f64, f64) -> f64,
        wind: Option<DVec2>,
        shortening: &impl Fn(f64, f64) -> f64,
        previous: Option<&Stage>,
    ) -> Self {
        let len = grid.len();
        let mesh = Mesh::new(grid);
        let wind = wind.map(|direction| {
            let mut order: Vec<_> = (0..len).collect();
            order.sort_unstable_by(|&a, &b| {
                mesh.points[a]
                    .dot(direction)
                    .total_cmp(&mesh.points[b].dot(direction))
                    .then(a.cmp(&b))
            });
            (direction, order)
        });
        let mut s = Self {
            grid,
            mesh,
            initial: Vec::with_capacity(len),
            rock: Vec::with_capacity(len),
            soil: Vec::with_capacity(len),
            surface: vec![0.0; len],
            fixed: Vec::with_capacity(len),
            outlet_drop: Vec::with_capacity(len),
            outlet_already_lowered: previous.is_some(),
            uplift: Vec::with_capacity(len),
            precipitation: Vec::with_capacity(len),
            runoff: vec![0.0; len],
            flux: vec![0.0; len],
            fine_load: vec![0.0; len],
            coarse_heading: vec![DVec2::ZERO; len],
            water: [0.0; 3],
            sediment: previous.map_or([0.0; 5], |old| old.sediment),
            wind,
            wind_budget: previous.map_or([0.0; 3], |old| old.wind_budget),
            tectonic_lift: Vec::new(),
            joints: Vec::new(),
            scarp_datum: Vec::new(),
            upland_rock_m3: previous.map_or(0.0, |old| old.upland_rock_m3),
            process_m3: previous.map_or([0.0; 3], |old| old.process_m3),
        };
        for i in 0..len {
            let [x, z] = s.mesh.points[i].to_array();
            let boundary = grid.on_border(i);
            let h = initial(x, z);
            let (surface, cover) = if boundary {
                (h, 0.0)
            } else if let Some(old) = previous {
                // Carry the evolved landscape. Recover only the unresolved
                // detail of the same initial field, bounded at the old scale.
                let limit = 0.045 * old.grid.cell_m;
                let (ids, weights) = old.mesh.stencil(s.mesh.points[i]);
                let initial: f64 = ids.into_iter().zip(weights).map(|(j, w)| f64::from(old.initial[j]) * w).sum();
                let rock: f64 = ids.into_iter().zip(weights).map(|(j, w)| old.rock[j] * w).sum();
                let cover: f64 = ids.into_iter().zip(weights).map(|(j, w)| old.soil[j] * w).sum();
                (rock + cover + (h - initial).clamp(-limit, limit), cover)
            } else {
                (h, 0.42)
            };
            s.initial.push(h as f32);
            s.rock.push(surface - cover);
            s.soil.push(cover);
            s.fixed.push(boundary);
            let outer = grid.min_m[1] + grid.n as f64 * grid.cell_m;
            let mouth = if boundary && z >= outer - grid.cell_m {
                let width = if package.jointed() { 72.0 } else { 175.0 };
                (-((x - span * 0.5) / width).powi(2)).exp()
            } else {
                0.0
            };
            s.outlet_drop
                .push((h - package.outlet_m + 6.0).max(0.0) * mouth);
            let edge = ((x - grid.min_m[0])
                .min(z - grid.min_m[1])
                .min(outer - x)
                .min(outer - z)
                / 280.0)
                .min(1.0);
            let fault_distance = (x - RockPackage::fault_x(z)).abs();
            let highland = (fault_distance / 730.0).min(1.0);
            s.uplift
                .push(edge * (0.00055 + highland * 0.00115) * (1.0 + 0.45 * noise(x, z, 1300.0)));
            if s.wind.is_some() && previous.is_none() {
                let rate =
                    0.000035 * (0.20 + 0.80 * highland) * (0.20 + 0.80 * shortening(x, z));
                s.tectonic_lift.push(edge * rate * COARSE_TECTONIC_YEARS);
            }
            s.precipitation.push(precipitation(x, z));
            if package.jointed() {
                s.joints.push(package.joint_damage(x, z));
            }
            if package.bedrock_scarp {
                s.scarp_datum.push(package.datum(x, z));
            }
        }
        let start = s.soil_volume();
        if previous.is_some() {
            s.sediment[1] += start - s.sediment[4];
        } else {
            s.sediment[0] = start;
        }
        s
    }

    fn soil_volume(&self) -> f64 {
        self.soil
            .iter()
            .zip(&self.mesh.area)
            .map(|(soil, area)| soil * area)
            .sum()
    }

    fn advance(&mut self, package: RockPackage, dt: f64, history_years: f64, drop: f64, event: usize, upland_runoff: bool) {
        let drop = if self.outlet_already_lowered {
            1.0
        } else {
            drop
        };
        let mut rain = 0.0;
        let mut infiltrated = 0.0;
        let storm = self
            .wind
            .as_ref()
            .map_or(1.0, |_| RAIN_EVENTS[event % RAIN_EVENTS.len()]);
        for i in 0..self.grid.len() {
            if self.fixed[i] {
                self.rock[i] = f64::from(self.initial[i]) - drop * self.outlet_drop[i];
            } else {
                self.rock[i] +=
                    dt * self.uplift[i] + self.tectonic_lift.get(i).copied().unwrap_or(0.0);
            }
            let [x, z] = self.mesh.points[i].to_array();
            let rock = package.rock(x, self.rock[i], z);
            let cover = (self.soil[i] / 3.0).min(1.0);
            let rainfall = self.precipitation[i] * storm;
            let recharge_fraction = 0.10 + 0.26 * cover + 0.22 * rock.permeability;
            let recharge =
                (rainfall * recharge_fraction).min(self.precipitation[i] * recharge_fraction);
            if package.sediment_weathering && !self.fixed[i] {
                let damage = self.joints[i];
                let season = WEATHERING_SEASONS[event % WEATHERING_SEASONS.len()];
                // Soil is the regolith store carried between events and meshes.
                // Thick cover limits fresh production. Storms can expose new rock.
                let regolith = history_years * 0.000012 * season
                    * (0.15 + recharge).min(1.0)
                    * (0.35 + rock.erodibility + 3.8 * damage)
                    * (-self.soil[i] / 1.1).exp();
                // Surface carbonate solution follows available recharge and
                // open joints. The solute exits separately from loose soil.
                let solute = if rock.carbonate {
                    history_years * 0.000075 * recharge * (0.05 + 3.2 * damage)
                        * (-self.soil[i] / 1.35).exp()
                } else {
                    0.0
                };
                self.rock[i] -= regolith + solute;
                self.soil[i] += regolith;
                let area = self.mesh.area[i];
                self.sediment[2] += (regolith + solute) * area;
                self.sediment[3] += solute * area;
                self.process_m3[1] += regolith * area;
                self.process_m3[2] += solute * area;
            }
            self.surface[i] = self.rock[i] + self.soil[i];
            self.runoff[i] = rainfall - recharge;
            rain += rainfall * self.mesh.area[i];
            infiltrated += recharge * self.mesh.area[i];
        }
        let flow = self.mesh.route(&self.surface, &self.fixed, &self.runoff);
        self.water = [
            rain,
            infiltrated,
            flow.q
                .iter()
                .zip(&self.fixed)
                .filter_map(|(&q, &fixed)| fixed.then_some(q))
                .sum(),
        ];
        self.flux.fill(0.0);
        self.fine_load.fill(0.0);
        self.coarse_heading.fill(DVec2::ZERO);
        // Downstream-first implicit detachment uses the same links as water.
        for &i in &flow.order {
            let i = i as usize;
            if self.fixed[i] || flow.grade[i] == 0.0 {
                continue;
            }
            let [x, z] = self.mesh.points[i].to_array();
            let rock = package.rock(x, self.rock[i], z);
            let damage = self.joints.get(i).copied().unwrap_or(0.0);
            let power = flow.q[i].min(280_000.0).powf(0.42);
            let resistance = if self.wind.is_some() {
                0.26 + 0.80 * (1.0 - rock.erodibility.min(1.0))
            } else {
                0.41 + 0.65 * (1.0 - rock.erodibility.min(1.0))
            };
            let energy = power * flow.grade[i] * (1.0 + 0.48 * damage);
            let entrainment = ((energy - resistance) / (energy + 0.6)).clamp(0.0, 1.0);
            // Rain moving over an exposed hillslope can wear friable rock
            // before enough area has accumulated to cut a stream. It follows
            // the existing downhill routes and contributes the same sediment.
            let wash = if upland_runoff {
                let unchanneled = 1.0 - ((flow.q[i] - 6_000.0) / 38_000.0).clamp(0.0, 1.0);
                let gradient = if package.jointed() {
                    (flow.grade[i] / 0.16).clamp(0.0, 1.0).sqrt()
                } else {
                    ((flow.grade[i] - 0.015) / 0.16).clamp(0.0, 1.0)
                };
                let exposure = (-self.soil[i] / 0.95).exp();
                0.018 * dt * self.runoff[i] * unchanneled * gradient
                    * rock.erodibility.powi(2) * exposure * (1.0 + 4.8 * damage)
            } else {
                0.0
            };
            if entrainment == 0.0 && wash == 0.0 {
                continue;
            }
            let power = power * entrainment;
            let mut reciprocal_length = 0.0;
            let mut downstream_height = 0.0;
            for edge in flow.links[i].as_slice() {
                let j = edge.to as usize;
                let weight = f64::from(edge.share) / f64::from(edge.distance);
                reciprocal_length += weight;
                downstream_height += weight * (self.rock[j] + self.soil[j]);
            }
            let cover = self.soil[i];
            let fall = (self.surface[i] - downstream_height / reciprocal_length).max(0.0);
            let sediment_rate = 0.0038 * dt * power * reciprocal_length;
            let scoured = cover.min(fall * sediment_rate / (1.0 + sediment_rate));
            let detachment = if package.sediment_weathering { 0.00172 } else { 0.00185 };
            let k = detachment
                * dt
                * power
                * rock.erodibility
                * (-(cover - scoured) / 1.1).exp()
                * reciprocal_length;
            let channel_cut = ((fall - scoured).max(0.0) * k / (1.0 + k)).min(0.40 / reciprocal_length);
            let wash = wash.min((fall - scoured - channel_cut).max(0.0) * 0.32);
            let cut = channel_cut + wash;
            self.soil[i] -= scoured;
            self.rock[i] -= cut;
            let area = self.mesh.area[i];
            self.fine_load[i] = (0.34 * scoured + cut * rock.fines) * area;
            self.flux[i] = (scoured + cut) * area - self.fine_load[i];
            self.sediment[2] += cut * area;
            self.upland_rock_m3 += wash * area;
        }
        for &i in flow.order.iter().rev() {
            let i = i as usize;
            if self.fixed[i] {
                self.sediment[3] += self.flux[i] + self.fine_load[i];
                continue;
            }
            let mut slope = 0.0;
            let mut distance = 0.0;
            let mut normalization = 0.0;
            let edges = flow.links[i].as_slice();
            for edge in edges {
                let j = edge.to as usize;
                let d = f64::from(edge.distance);
                let w = f64::from(edge.share);
                slope +=
                    w * ((self.rock[i] + self.soil[i] - self.rock[j] - self.soil[j]) / d).max(0.0);
                distance += w * d;
                normalization += w;
            }
            let transport = if package.jointed() { 0.10 } else { 0.081 };
            let capacity = transport * flow.q[i] * slope * dt;
            if package.sediment_weathering && capacity > 0.0 && flow.q[i] > 22_000.0 {
                let area = self.mesh.area[i];
                // Only bedload delivered along incoming edges acts as tools.
                // Locally scoured material enters transport after this decision.
                let incoming = self.coarse_heading[i].length();
                if incoming > 0.0 {
                    let mobile_depth = incoming.min(capacity) / area;
                    let excess_depth = (self.flux[i] - capacity).max(0.0) / area;
                    let exposure = (-(self.soil[i] + 0.35 * excess_depth) / 0.75).exp();
                    let concentration = ((flow.q[i] - 22_000.0) / 82_000.0).min(1.0);
                    let [x, z] = self.mesh.points[i].to_array();
                    let rock = package.rock(x, self.rock[i], z);
                    let cut = (0.27 * mobile_depth * exposure * rock.erodibility
                        * concentration * (slope / 0.16).sqrt())
                        .min(0.16 * slope * distance)
                        .min(0.015 * dt);
                    self.rock[i] -= cut;
                    let volume = cut * area;
                    self.flux[i] += volume * (1.0 - rock.fines);
                    self.fine_load[i] += volume * rock.fines;
                    self.sediment[2] += volume;
                    self.process_m3[0] += volume;
                }
            }
            let settling = 1.0 - (-distance / 172.0).exp();
            let deposit =
                ((self.flux[i] - capacity).max(0.0) * settling).min(self.mesh.area[i] * 0.60);
            self.soil[i] += deposit / self.mesh.area[i];
            let transported = self.flux[i] - deposit;
            let fine = self.fine_load[i];
            if transported == 0.0 && fine == 0.0 { continue; }
            // Directional persistence for bedload on the existing downhill
            // faces. Incoming transport is diluted by fresh local sediment.
            let heading = self.coarse_heading[i] / self.flux[i].max(1e-12);
            let mut coarse_total = 0.0;
            let mut directions = [DVec2::ZERO; 8];
            let mut weights = [0.0; 8];
            for (slot, edge) in edges.iter().enumerate() {
                let j = edge.to as usize;
                let direction = (self.mesh.points[j] - self.mesh.points[i]) / f64::from(edge.distance);
                let aligned = (0.55 * heading.dot(direction)).clamp(-0.55, 0.55);
                let weight = f64::from(edge.share) * (1.0 + aligned);
                directions[slot] = direction;
                weights[slot] = weight;
                coarse_total += weight;
            }
            for (slot, edge) in edges.iter().enumerate() {
                let share = f64::from(edge.share) / normalization;
                let j = edge.to as usize;
                let coarse = transported * weights[slot] / coarse_total;
                self.flux[j] += coarse;
                self.coarse_heading[j] += directions[slot] * coarse;
                self.fine_load[j] += fine * share;
            }
        }
        self.slopes(package, dt);
        if event % 3 == 2 && self.wind.is_some() {
            self.wind_erosion(package, 3.0 * dt);
        }
    }

    fn slopes(&mut self, package: RockPackage, dt: f64) {
        self.flux.fill(0.0);
        let failure = 1.0 - (-dt / 110.0).exp();
        for i in 0..self.grid.len() {
            self.surface[i] = self.rock[i] + self.soil[i];
        }
        for i in 0..self.grid.len() {
            if self.fixed[i] {
                continue;
            }
            let [x, z] = self.mesh.points[i].to_array();
            let rock = package.rock(x, self.rock[i], z);
            let wall_support = if package.jointed() { 1.19 } else { 1.0 };
            let repose = (0.23 + 0.42 * rock.wall_grade.sqrt()) * wall_support;
            let weathered = if package.sediment_weathering {
                0.0
            } else {
                dt * (0.00022 + 0.00115 * rock.erodibility) * (-self.soil[i] / 1.35).exp()
            };
            self.rock[i] -= weathered;
            self.flux[i] += weathered;
            self.sediment[2] += weathered * self.mesh.area[i];
            let mut weights = [(0, 0.0); 8];
            let mut count = 0;
            let (mut maximum, mut drop, mut total) = (0.0_f64, 0.0_f64, 0.0);
            let mut face_grade = 0.0_f64;
            let mut unsupported = 0.0_f64;
            for edge in self.mesh.links[i].as_slice() {
                let j = edge.to as usize;
                let d = f64::from(edge.distance);
                let fall = self.surface[i] - self.surface[j];
                if fall <= 0.0 {
                    continue;
                }
                let excess = (fall - repose * d).max(0.0);
                maximum = maximum.max(excess);
                face_grade = face_grade.max(fall / d);
                if package.bedrock_scarp && fall > 0.55 * d {
                    let foot = self.surface[j] - self.soil[j]
                        + self.scarp_datum[i] - self.scarp_datum[j];
                    let crown = self.surface[i] - self.soil[i];
                    let span = package.weak_foot(self.scarp_datum[i], crown, foot).unwrap_or(0.0);
                    // Convert retreat across the cell to an equivalent
                    // lowering. Debris at the toe limits further retreat.
                    unsupported = unsupported.max(
                        span.min(fall) / d * (-self.soil[j] / 1.8).exp(),
                    );
                }
                drop = drop.max(fall);
                let weight = (0.13 * fall + excess) / d;
                weights[count] = (j, weight);
                count += 1;
                total += weight;
            }
            if count == 0 {
                continue;
            }
            let released = (0.55 * maximum * rock.erodibility * failure)
                .min(drop * 0.24)
                * (-self.soil[i] / 2.0).exp();
            // Backwear exposed, weak faces as a lateral retreat distance
            // projected onto the heightfield. Talus suppresses fresh attack,
            // so the scarp can keep retreating only where debris is removed.
            let released = if package.folded {
                let damage = self.joints[i];
                let exposed = (-self.soil[i] / 1.4).exp();
                let lateral = 0.065 * dt * rock.erodibility.powi(3) * (1.0 + 1.6 * damage);
                let backwear = lateral * (face_grade - 0.16).max(0.0) * exposed;
                let collapse = 0.045 * dt * unsupported * (0.40 + 2.1 * damage) * exposed;
                (released + backwear + collapse).min(drop * 0.32)
            } else {
                released
            };
            self.rock[i] -= released;
            self.sediment[2] += released * self.mesh.area[i];
            let creep =
                ((0.11 + 0.24 * rock.erodibility) * dt / self.grid.cell_m.powi(2)).min(0.20);
            let moved = (creep * drop + failure * maximum).min(self.soil[i] + released + weathered);
            self.flux[i] += released - moved;
            let volume = moved * self.mesh.area[i];
            for &(j, weight) in &weights[..count] {
                if !self.fixed[j] {
                    self.flux[j] += volume * weight / (total * self.mesh.area[j]);
                } else {
                    self.sediment[3] += volume * weight / total;
                }
            }
        }
        for (soil, change) in self.soil.iter_mut().zip(&self.flux) {
            *soil = (*soil + change).max(0.0);
        }
    }

    fn wind_erosion(&mut self, package: RockPackage, dt: f64) {
        let Some((wind, order)) = &self.wind else {
            return;
        };
        self.flux.fill(0.0);
        for i in 0..self.grid.len() {
            self.surface[i] = self.rock[i] + self.soil[i];
        }
        for &i in order {
            let incoming = self.flux[i];
            if self.fixed[i] {
                self.sediment[3] += incoming;
                self.wind_budget[2] += incoming;
                continue;
            }
            let area = self.mesh.area[i];
            let mut routes = [(0, 0.0); 8];
            let mut count = 0;
            let mut weight_sum = 0.0;
            let mut shelter = 0.0_f64;
            for edge in self.mesh.links[i].as_slice() {
                let j = edge.to as usize;
                let distance = f64::from(edge.distance);
                let alignment = (self.mesh.points[j] - self.mesh.points[i]).dot(*wind) / distance;
                if alignment > 0.01 {
                    let weight = alignment.powi(3);
                    routes[count] = (j, weight);
                    weight_sum += weight;
                    count += 1;
                } else if alignment < -0.01 {
                    let obstruction = (self.surface[j] - self.surface[i]) / distance * -alignment;
                    shelter = shelter.max(obstruction);
                }
            }
            if count == 0 {
                self.soil[i] += incoming / area;
                continue;
            }
            let dryness =
                1.0 - super::smooth_ramp(((self.precipitation[i] - 0.30) / 0.58).clamp(0.0, 1.0));
            let exposure = (1.0 - 2.8 * shelter).clamp(0.12, 1.0);
            let loose = self.soil[i] * (1.0 - (-0.0009 * dt * dryness * exposure).exp());
            let loose = loose.min(0.00022 * dt * dryness * exposure);
            let [x, z] = self.mesh.points[i].to_array();
            let susceptibility = package.rock(x, self.rock[i], z).erodibility;
            // Abrasion requires moving sand and exposed bedrock. Sheltered
            // ground and wet ground retain more of the incoming load.
            let abrasive = (incoming / area + loose) * (-self.soil[i] / 0.35).exp();
            let cut = (abrasive * 0.018 * susceptibility * dryness * exposure)
                .min(0.00009 * dt * dryness);
            let settling_factor = 1.0 + 3.5 * (1.0 - exposure) + 2.5 * (1.0 - dryness);
            let settling = 1.0 - (-self.grid.cell_m * settling_factor / 115.0).exp();
            let deposited = incoming * settling;
            self.rock[i] -= cut;
            self.soil[i] += deposited / area - loose;
            self.sediment[2] += cut * area;
            self.wind_budget[0] += cut * area;
            self.wind_budget[1] += loose * area;
            let transported = incoming - deposited + (cut + loose) * area;
            for &(j, weight) in &routes[..count] {
                self.flux[j] += transported * (weight / weight_sum);
            }
        }
    }

    fn finish(self) -> Landscape {
        Landscape {
            grid: self.grid,
            surface: self.mesh.regularize(|i| self.rock[i] + self.soil[i]),
            cover: self.mesh.regularize(|i| self.soil[i]),
            water: self.water,
            sediment: self.sediment,
            wind: self.wind_budget,
            upland_rock_m3: self.upland_rock_m3,
            process_m3: self.process_m3,
        }
    }
}
