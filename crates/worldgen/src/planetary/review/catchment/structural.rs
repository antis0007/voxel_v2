//! A tilted rock package and the channels that dissect it.
mod evolution;
pub(in crate::planetary) mod mesh;
use super::{Material, SPAN_M, Site, World, WorldSampleConfig, base_height, smooth_ramp};
use glam::{DVec2, DVec3};
use crate::BlockId;

#[derive(Clone, Copy)]
pub(super) struct RockPackage {
    outlet_m: f64,
    joint_seed: Option<u32>,
    folded: bool,
    bedrock_scarp: bool,
    pub sediment_weathering: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Rock {
    pub name: &'static str,
    pub erodibility: f64,
    pub wall_grade: f64,
    pub permeability: f64,
    pub fines: f64,
    pub carbonate: bool,
}

impl RockPackage {
    const CONTACTS_M: [f64; 5] = [17.0, 48.0, 73.0, 130.0, 148.0];

    pub fn new(outlet_m: f64) -> Self {
        Self { outlet_m, joint_seed: None, folded: false, bedrock_scarp: false, sediment_weathering: false }
    }

    pub fn with_joints(self, seed: u32) -> Self {
        Self { joint_seed: Some(seed), ..self }
    }

    pub fn jointed(self) -> bool { self.joint_seed.is_some() }

    pub fn with_folds(self, bedrock_scarp: bool) -> Self {
        Self { folded: true, bedrock_scarp, ..self }
    }

    fn fold_displacement(self, x: f64, z: f64) -> f64 {
        if !self.folded { return 0.0; }
        let shift = f64::from(self.joint_seed.unwrap_or_default() % 419) - 209.0;
        if self.bedrock_scarp {
            // A plunging monocline displaces every contact together. Strike
            // bends along the fold, and the downthrown limb widens downstream.
            let strike = 1730.0 + shift + 0.22 * (z - 2000.0)
                + 215.0 * (z / 940.0).sin() + 58.0 * (z / 410.0).sin();
            let limb = 0.5 * (1.0 + ((strike - x) / 275.0).tanh());
            let throw = 172.0 + 42.0 * ((z + shift) / 1470.0).cos();
            return throw * limb;
        }
        // Two finite fold limbs on opposing sides of the inherited valley.
        // Their dip, strike and relief differ, leaving a saddle between them.
        let limb = |cross: f64, along: f64, dip_width: f64, dip_slope: f64| {
            let width = if cross < 0.0 { dip_width } else { dip_slope };
            let normalized = (cross / width).powi(2) + along.powi(2);
            (1.0 - normalized).max(0.0).powi(2)
        };
        let west = x - 1210.0 - shift * 0.55;
        let north = z - 1550.0 - shift;
        let east = x - 2830.0 - shift * 0.75;
        let south = z - 2560.0 + shift;
        154.0 * limb(0.80 * west + 0.60 * north, (-0.60 * west + 0.80 * north) / 1280.0, 680.0, 260.0)
            + 116.0 * limb(-0.95 * east + 0.31 * south, (0.31 * east + 0.95 * south) / 1130.0, 640.0, 360.0)
    }

    fn joint_damage(self, x: f64, z: f64) -> f64 {
        let Some(seed) = self.joint_seed else { return 0.0 };
        let angle = f64::from(seed % 1021) * (std::f64::consts::TAU / 1021.0);
        let (sin, cos) = angle.sin_cos();
        let u = x * cos - z * sin;
        let v = x * sin + z * cos;
        let sample = |u, v, salt| {
            crate::planet::noise_at(DVec3::new(u, 3.0, v), seed.wrapping_add(salt))
        };
        let swarm = smooth_ramp(((sample(u / 185.0, v / 235.0, 7103) - 0.22) / 0.54).clamp(0.0, 1.0));
        let first = sample(u / 31.0, v / 105.0, 7109);
        let second = sample((u + 0.6 * v) / 86.0, (v - 0.7 * u) / 35.0, 7121);
        let connected = ((first.max(second) - 0.32) / 0.52).clamp(0.0, 1.0);
        swarm * smooth_ramp(connected)
    }

    fn datum(self, x: f64, z: f64) -> f64 {
        let fault = (x - Self::fault_x(z)) / 42.0;
        self.outlet_m + 0.036 * (x - 2000.0) - 0.018 * (z - 4000.0)
            + 23.0 * ((x + 0.67 * z) / 710.0).sin()
            + 12.0 * ((0.54 * x - z) / 430.0).sin()
            + 13.0 * fault.tanh()
            + self.fold_displacement(x, z)
    }

    pub fn contacts(self, x: f64, z: f64) -> [f64; 5] {
        let datum = self.datum(x, z);
        Self::CONTACTS_M.map(|contact| datum + contact)
    }

    fn fault_x(z: f64) -> f64 {
        let u = z - SPAN_M;
        SPAN_M * 0.5 + 0.18 * u + 245.0 * (u / 730.0).sin() + 72.0 * (u / 330.0).sin()
    }

    fn fractured(self, x: f64, z: f64) -> f64 {
        let main = ((x - Self::fault_x(z)) / 54.0).abs();
        let a = ((z - 1330.0 - 0.62 * (x - 1300.0) - 76.0 * (x / 510.0).sin()) / 60.0).abs();
        let b = ((z - 2650.0 + 0.89 * (x - 1800.0) + 97.0 * (x / 650.0).sin()) / 74.0).abs();
        let c = ((z - 3060.0 - 0.43 * (x - 2120.0) - 58.0 * (x / 420.0).sin()) / 49.0).abs();
        let (a_extent, b_extent, c_extent) = self.joint_seed.map_or((1.0, 1.0, 1.0), |seed| {
            let shift = f64::from(seed % 521) - 260.0;
            let segment = |centre: f64, length: f64| {
                smooth_ramp((1.0 - ((x - centre) / length).abs()).clamp(0.0, 1.0))
            };
            (segment(1470.0 + shift, 1200.0), segment(2100.0 - shift, 1640.0),
                segment(2640.0 + shift, 1120.0))
        });
        (-main.powi(2))
            .exp()
            .max(0.82 * a_extent * (-a.powi(2)).exp())
            .max(0.76 * b_extent * (-b.powi(2)).exp())
            .max(0.61 * c_extent * (-c.powi(2)).exp())
    }

    pub fn rock(self, x: f64, y: f64, z: f64) -> Rock {
        let units = [
            ("Basal siltstone", 0.95, 0.48, 0.24, 0.76),
            ("Lower sandstone", 0.27, 3.45, 0.72, 0.21),
            ("Recessive mudstone", 0.86, 0.68, 0.12, 0.82),
            ("Massive sandstone", 0.18, 4.80, 0.45, 0.17),
            ("Bedded bench", 0.70, 0.79, 0.34, 0.54),
            ("Caprock", 0.31, 2.95, 0.37, 0.19),
        ];
        let unit = self.unit(x, y, z);
        let (name, erodibility, wall_grade, permeability, fines) = units[unit];
        let fracture = self.fractured(x, z);
        let joint_loss = smooth_ramp(fracture);
        Rock {
            name,
            erodibility: erodibility + (1.20 - erodibility) * joint_loss,
            wall_grade: wall_grade + (0.60 - wall_grade) * joint_loss,
            permeability: permeability + (1.0 - permeability) * joint_loss,
            fines,
            carbonate: unit == 5,
        }
    }

    fn unit(self, x: f64, y: f64, z: f64) -> usize {
        let level = y - self.datum(x, z);
        Self::CONTACTS_M.partition_point(|&h| h <= level)
    }

    fn weak_foot(self, datum: f64, crown: f64, foot: f64) -> Option<f64> {
        // A sandstone face can span multiple cells. Detach its edge only
        // where the adjacent cut has exposed the recessive unit underneath.
        let (base, weak_thickness) = match Self::CONTACTS_M.partition_point(|&h| h <= crown - datum) {
            1 => (Self::CONTACTS_M[0], 17.0),
            3 => (Self::CONTACTS_M[2], 25.0),
            5 => (Self::CONTACTS_M[4], 18.0),
            _ => return None,
        };
        let exposed = ((datum + base - foot) / weak_thickness).clamp(0.0, 1.0);
        Some((crown - datum - base) * exposed.sqrt())
    }

    pub fn block(self, x: f64, y: f64, z: f64) -> BlockId {
        match self.unit(x, y, z) {
            1 | 3 => BlockId::Sandstone,
            5 => BlockId::Limestone,
            _ => BlockId::Stone,
        }
    }
}

pub(super) struct Terrain {
    landscape: evolution::Landscape,
}

impl Terrain {
    pub fn build(
        cfg: &WorldSampleConfig,
        world: &World,
        site: Site,
        material: Material,
        package: RockPackage,
    ) -> Self {
        let noise = |x: f64, z: f64, scale: f64| {
            let p = DVec3::new(0.90 * x - 0.43 * z, 93.0, 0.43 * x + 0.90 * z);
            crate::planet::noise_at(p / scale, cfg.seed.wrapping_add(6101 + scale as u32)) - 0.5
        };
        let initial = |x: f64, z: f64| {
            let (parent, _) = base_height(
                cfg,
                world,
                site,
                Material {
                    outlet_drop_m: 0.0,
                    ..material
                },
                x,
                z,
            );
            let (parent_weight, bedding_weight) = if package.jointed() { (0.74, 0.26) } else { (0.78, 0.22) };
            let displacement = package.fold_displacement(x, z);
            if package.bedrock_scarp {
                // Keep an intact depositional surface before lowering the
                // outlet. The solver, rather than the fold, draws tributaries.
                return 0.57 * parent + 0.43 * (package.datum(x, z) - displacement + 173.0)
                    + displacement + 18.0 * noise(x, z, 680.0)
                    + 5.0 * noise(x, z, 270.0);
            }
            parent_weight * (parent + displacement)
                + bedding_weight * (package.datum(x, z) + 177.0)
                + 37.0 * noise(x, z, 680.0)
                + 11.0 * noise(x, z, 270.0)
        };
        let precipitation = |x: f64, z: f64| {
            let chart = site.frame.chart(x, z);
            let dir = super::direction(cfg, chart.x, chart.y);
            0.18 + 1.2 * world.mesh.sample(&world.rain, dir)
        };
        let wind = material.weathering.then(|| {
            let origin = site.frame.chart(SPAN_M * 0.5, SPAN_M * 0.5);
            let dir = super::direction(cfg, origin.x, origin.y);
            let east = DVec3::Y.cross(dir).normalize_or_zero();
            let zonal = if (0.42..0.82).contains(&dir.y.abs()) { east } else { -east };
            let displaced = super::atlas::from_body((dir + zonal * 0.0002).normalize() * super::atlas::RADIUS_M);
            (site.frame.local(DVec2::new(displaced.x, displaced.z)) - site.frame.local(origin))
                .normalize_or(DVec2::X)
        });
        let shortening = |x: f64, z: f64| {
            let chart = site.frame.chart(x, z);
            world.mesh.sample(&world.collision, super::direction(cfg, chart.x, chart.y)).clamp(0.0, 1.0)
        };
        Self {
            landscape: evolution::evolve(package, SPAN_M, initial, noise, precipitation, wind, shortening, material.upland_runoff),
        }
    }

    pub fn height(&self, x: f64, z: f64) -> f64 {
        self.landscape.grid.bilinear(&self.landscape.surface, x, z)
    }

    pub fn cover(&self, x: f64, z: f64) -> f64 {
        self.landscape.grid.bilinear(&self.landscape.cover, x, z)
    }

    pub fn water_balance(&self) -> [f64; 3] {
        self.landscape.water
    }

    pub fn sediment_balance(&self) -> [f64; 5] {
        self.landscape.sediment
    }

    pub fn wind_balance(&self) -> [f64; 3] { self.landscape.wind }

    pub fn process_balance(&self) -> [f64; 3] { self.landscape.process_m3 }

    pub fn upland_rock_m3(&self) -> f64 { self.landscape.upland_rock_m3 }
}
