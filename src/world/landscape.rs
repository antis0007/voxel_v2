//! Versioned metre-space climate, overlapped erosion regions and biome strata.
//! Region borders blend the same four deterministic tiles on both sides. Results
//! never depend on chunk size, worker order, or voxel resolution.
use super::{
    BlockId, ColumnSample, WorldSampleConfig, WorldType, fbm2, hash2, smoothstep, value_noise2,
};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

const SPACING: f32 = 8.0;
const REGION: f32 = 128.0;
const EDGE: usize = 33;
const COUNT: usize = EDGE * EDGE;
const STEPS: usize = 48;

// Shared desert exclusion keeps distant vegetation consistent with terrain.
fn desert_climate(temperature: f32, moisture: f32) -> bool {
    temperature > 0.63 && moisture < 0.38
}

// Generator-10 savanna climate, also used by distant vegetation. Older hot,
// very dry desert rules take priority. Preserve these thresholds for old saves.
pub(crate) fn savanna_climate(temperature: f32, moisture: f32) -> bool {
    temperature > 0.56 && moisture < 0.49 && !desert_climate(temperature, moisture)
}

#[derive(Clone, Copy, Debug)]
pub enum Biome {
    Ocean,
    Coast,
    Marsh,
    Meadow,
    Forest,
    Taiga,
    Desert,
    Badlands,
    Tundra,
    Alpine,
    Savanna,
    Rainforest,
    Heathland,
    Fen,
}
impl Biome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ocean => "Ocean",
            Self::Coast => "Coast",
            Self::Marsh => "Wetlands",
            Self::Meadow => "Meadow",
            Self::Forest => "Temperate forest",
            Self::Taiga => "Taiga",
            Self::Desert => "Desert",
            Self::Badlands => "Badlands",
            Self::Tundra => "Tundra",
            Self::Alpine => "Alpine peaks",
            Self::Savanna => "Savanna",
            Self::Rainforest => "Rainforest",
            Self::Heathland => "Highland heath",
            Self::Fen => "Lowland fen",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Landscape {
    pub height: f32,
    pub slope: f32,
    pub moisture: f32,
    pub temperature: f32,
    pub sediment: f32,
    pub biome: Biome,
}

struct Tile {
    height: Vec<f32>,
    sediment: Vec<f32>,
}
type TileKey = (u32, u8, i32, i32, u32);
#[derive(Default)]
struct TileCache {
    entries: HashMap<TileKey, Arc<OnceLock<Arc<Tile>>>>,
    recent: VecDeque<TileKey>,
}
static TILES: OnceLock<Mutex<TileCache>> = OnceLock::new();

fn climate(x: f32, z: f32, seed: u32) -> (f32, f32) {
    let wx = x + (value_noise2(x / 420.0, z / 420.0, seed.wrapping_add(7)) - 0.5) * 180.0;
    let wz = z + (value_noise2(x / 390.0, z / 390.0, seed.wrapping_add(9)) - 0.5) * 180.0;
    let temp = value_noise2(wx / 700.0, wz / 700.0, seed.wrapping_add(11));
    let rain = value_noise2(wx / 520.0, wz / 520.0, seed.wrapping_add(23));
    (temp, rain)
}

/// Cheap far-field ecology, using the same climate as editable terrain.
/// No erosion tiles are allocated to place visual vegetation.
pub(crate) fn visual_ecology(cfg: &WorldSampleConfig, x: f32, z: f32, height: f32) -> (f32, f32, f32) {
    let (mut heat, mut wet) = climate(x, z, cfg.seed);
    if cfg.generator_version >= 5 {
        heat = ((heat - 0.5) * 1.55 + 0.5).clamp(0.0, 1.0);
        wet = ((wet - 0.5) * 1.4 + 0.5).clamp(0.0, 1.0);
    }
    heat = (heat - (height - 30.0).max(0.0) * if cfg.generator_version >= 7 { 0.0018 } else { 0.0035 }).clamp(0.0, 1.0);
    let grove = value_noise2(x / 270.0, z / 270.0, cfg.seed.wrapping_add(3127)) * 0.72
        + value_noise2(x / 87.0, z / 87.0, cfg.seed.wrapping_add(3157)) * 0.28;
    (heat, wet, grove)
}

fn bedrock(x: f32, z: f32, seed: u32, ridged: bool) -> f32 {
    let continental = value_noise2(x / 900.0, z / 900.0, seed.wrapping_add(37));
    let land = smoothstep(0.30, 0.60, continental);
    let ranges = value_noise2(x / 340.0, z / 340.0, seed.wrapping_add(97));
    let mountain = smoothstep(if ridged { 0.36 } else { 0.55 }, 0.88, ranges);
    let ridge = 1.0 - (fbm2(x / 170.0, z / 170.0, seed.wrapping_add(137)) * 2.0 - 1.0).abs();
    let hills = (fbm2(x / 100.0, z / 100.0, seed.wrapping_add(71)) - 0.5) * 20.0;
    -10.0 + land * (38.0 + hills + mountain * ridge * ridge * 125.0)
}

#[derive(Clone, Copy, Debug)]
pub struct TectonicSample {
    /// Crustal buoyancy: < 0.0 is dense basaltic oceanic crust (abyssal plains, trenches, shelves);
    /// > 0.0 is buoyant granitic/sedimentary continental crust.
    pub continentalness: f32,
    /// Relative normal convergence (> 0: collision/subduction) or divergence (< 0: rift valley / mid-ocean ridge).
    pub boundary_stress: f32,
    /// Transform / strike-slip shear intensity along the plate boundary [0, 1].
    pub transform_shear: f32,
    /// Proximity to the active plate suture [0, 1] (1.0 right on the plate boundary).
    pub boundary_proximity: f32,
    /// Perpendicular distance (meters) to the nearest plate boundary bisector.
    pub boundary_dist: f32,
    /// Unit normal vector pointing across the plate boundary (direction of maximum tectonic compression/extension).
    pub boundary_normal: [f32; 2],
    /// Fold-and-thrust compressional wave [-1, 1] (anticlines > 0, synclines < 0).
    pub fold_wave: f32,
    /// Orogenic mountain uplift intensity [0, 1] from continent-continent collision or cordilleran compression.
    pub orogeny: f32,
    /// Volcanic arc / rift magmatism intensity [0, 1] (drives basalt flows, dikes, calderas & hydrothermal ores).
    pub volcanism: f32,
    /// Oceanic subduction trench depression [0, 1].
    pub trench: f32,
    /// Continental rift graben / mid-ocean axial valley depression [0, 1].
    pub rift_graben: f32,
    /// Discrete fault-block vertical throw (meters) across local secondary fault planes.
    pub fault_throw: f32,
    /// Proximity [0, 1] to a local secondary fault plane (exposes fault breccia & hydrothermal veins).
    pub fault_plane: f32,
    /// Crustal age / craton stability [0, 1] (older shields preserve deeper sedimentary & banded-iron records).
    pub crust_age: f32,
}

pub fn sample_tectonics(wx: f32, wz: f32, seed: u32) -> TectonicSample {
    const PLATE_SCALE: f32 = 540.0;
    // Mantle convection plumes warp plate sutures into organic, curving arcs.
    let warp_x = (value_noise2(wx / 420.0, wz / 420.0, seed.wrapping_add(4101)) - 0.5) * 185.0
        + (value_noise2(wx / 155.0, wz / 155.0, seed.wrapping_add(4109)) - 0.5) * 52.0;
    let warp_z = (value_noise2(wx / 420.0, wz / 420.0, seed.wrapping_add(4117)) - 0.5) * 185.0
        + (value_noise2(wx / 155.0, wz / 155.0, seed.wrapping_add(4127)) - 0.5) * 52.0;
    let px = wx + warp_x;
    let pz = wz + warp_z;

    let gx = (px / PLATE_SCALE).floor() as i32;
    let gz = (pz / PLATE_SCALE).floor() as i32;

    let plate_props = |ix: i32, iz: i32| -> (f32, f32, f32, f32, f32, f32) {
        let jx = 0.16 + hash2(ix, iz, seed.wrapping_add(4133)) * 0.68;
        let jz = 0.16 + hash2(ix, iz, seed.wrapping_add(4139)) * 0.68;
        let cx = (ix as f32 + jx) * PLATE_SCALE;
        let cz = (iz as f32 + jz) * PLATE_SCALE;

        // Supercontinent cycle + plate hash determines continental (> 0) vs oceanic (< 0) lithosphere.
        let macro_craton = value_noise2(cx / 1350.0, cz / 1350.0, seed.wrapping_add(37));
        let plate_h = hash2(ix, iz, seed.wrapping_add(4153));
        let mut buoyancy = (macro_craton * 0.62 + plate_h * 0.38 - 0.44) * 2.2;
        if ix == 0 && iz == 0 {
            // The origin plate is a coastal continental craton so the starter watershed
            // is anchored on land while neighboring plates can form deep oceans.
            buoyancy = buoyancy.max(0.42);
        } else if (ix.abs() + iz.abs()) == 1 && plate_h < 0.58 {
            // Ensure at least some immediate neighbor plates are oceanic so oceans are
            // reachable within a few hundred meters of the starter basin.
            buoyancy = -0.35 - (0.58 - plate_h) * 0.95;
        }

        let angle = hash2(ix, iz, seed.wrapping_add(4177)) * std::f32::consts::TAU;
        let speed = 0.45 + hash2(ix, iz, seed.wrapping_add(4189)) * 0.55;
        let vx = angle.cos() * speed;
        let vz = angle.sin() * speed;
        let age = hash2(ix, iz, seed.wrapping_add(4201));
        (cx, cz, buoyancy, vx, vz, age)
    };

    let mut d1_sq = f32::MAX;
    let mut d2_sq = f32::MAX;
    let mut p1 = (0.0f32, 0.0f32, 0.3f32, 1.0f32, 0.0f32, 0.5f32);
    let mut p2 = p1;

    for dz in -1..=1 {
        for dx in -1..=1 {
            let cand = plate_props(gx + dx, gz + dz);
            let dx_m = px - cand.0;
            let dz_m = pz - cand.1;
            let d_sq = dx_m * dx_m + dz_m * dz_m;
            if d_sq < d1_sq {
                d2_sq = d1_sq;
                p2 = p1;
                d1_sq = d_sq;
                p1 = cand;
            } else if d_sq < d2_sq {
                d2_sq = d_sq;
                p2 = cand;
            }
        }
    }

    let c12_x = p2.0 - p1.0;
    let c12_z = p2.1 - p1.1;
    let c12_len = (c12_x * c12_x + c12_z * c12_z).sqrt().max(1.0);
    let nx = c12_x / c12_len;
    let nz = c12_z / c12_len;

    // Exact perpendicular distance to the Voronoi bisector between plate 1 and plate 2.
    let boundary_dist = ((d2_sq - d1_sq) / (2.0 * c12_len)).max(0.0);
    let boundary_proximity = (-boundary_dist / 135.0).exp();

    // Relative velocity of plate 1 toward plate 2 projected onto the boundary normal & tangent.
    let dvx = p1.3 - p2.3;
    let dvz = p1.4 - p2.4;
    let boundary_stress = (dvx * nx + dvz * nz).clamp(-1.35, 1.35);
    let transform_shear = (dvx * (-nz) + dvz * nx).abs().clamp(0.0, 1.2) * boundary_proximity;

    let convergence = boundary_stress.max(0.0);
    let divergence = (-boundary_stress).max(0.0);

    // Smoothly transition crustal buoyancy across passive & active margins to form
    // natural continental shelves and slopes between continents and ocean basins.
    let margin_blend = smoothstep(0.0, 105.0, boundary_dist);
    let boundary_buoyancy = 0.5 * (p1.2 + p2.2);
    let mut continentalness = boundary_buoyancy + (p1.2 - boundary_buoyancy) * margin_blend;
    // Intra-plate coastal embayments, peninsulas, and marginal seas.
    let shelf_variation = (fbm2(wx / 310.0, wz / 310.0, seed.wrapping_add(4219)) - 0.5) * 0.52;
    continentalness = (continentalness + shelf_variation).clamp(-1.2, 1.2);

    // Compressional fold wave (anticlines & synclines) aligned parallel to the plate suture.
    let fold_warp = (fbm2(wx / 92.0, wz / 92.0, seed.wrapping_add(4231)) - 0.5) * 3.4;
    let fold_phase = boundary_dist * 0.082 + fold_warp;
    let fold_wave = fold_phase.cos();

    let both_continental = smoothstep(-0.15, 0.25, p1.2) * smoothstep(-0.15, 0.25, p2.2);
    let has_oceanic = 1.0 - both_continental;
    let on_subducting_plate = smoothstep(0.05, -0.30, p1.2) * smoothstep(-0.35, 0.25, p2.2 - p1.2);
    let on_overriding_plate = smoothstep(-0.25, 0.15, p1.2) * smoothstep(-0.10, 0.35, p1.2 - p2.2);

    // 1. Orogeny: Continent-continent collision belts + cordilleran thrust ranges.
    let suture_collar = (-((boundary_dist - 28.0) / 98.0).powi(2)).exp();
    let orogeny = (convergence * suture_collar * (0.45 + 0.65 * both_continental)).clamp(0.0, 1.0);

    // 2. Subduction Trench: Deep oceanic trench on the subducting oceanic slab near the suture.
    let trench_profile = (-((boundary_dist - 18.0) / 42.0).powi(2)).exp();
    let trench = (convergence * has_oceanic * on_subducting_plate * trench_profile).clamp(0.0, 1.0);

    // 3. Volcanic Arc & Rift Magmatism: Peaks 40..115m behind the trench on the overriding plate,
    // plus fissure volcanism along divergent rifts.
    let arc_profile = (-((boundary_dist - 68.0) / 54.0).powi(2)).exp();
    let cone_chain = smoothstep(
        0.34,
        0.78,
        value_noise2(wx / 68.0, wz / 68.0, seed.wrapping_add(4243)),
    );
    let arc_volcanism = convergence * has_oceanic * (0.35 + 0.65 * on_overriding_plate) * arc_profile * (0.45 + 0.55 * cone_chain);
    let rift_profile = (-(boundary_dist / 52.0).powi(2)).exp();
    let rift_volcanism = divergence * rift_profile * (0.35 + 0.65 * cone_chain);
    let volcanism = (arc_volcanism + rift_volcanism * 0.75).clamp(0.0, 1.0);

    // 4. Divergent Rift Graben (sunken axial valley flanked by uplifted horst shoulders).
    let rift_graben = (divergence * (-(boundary_dist / 46.0).powi(2)).exp()).clamp(0.0, 1.0);

    // 5. Secondary Fault Network: Discrete fault-block throw and fault-plane breccia zones.
    let strike_coord = (wx * (-nz) + wz * nx) / 88.0
        + (value_noise2(wx / 110.0, wz / 110.0, seed.wrapping_add(4259)) - 0.5) * 0.9;
    let dip_coord = (wx * nx + wz * nz) / 74.0
        + (value_noise2(wx / 95.0, wz / 95.0, seed.wrapping_add(4271)) - 0.5) * 0.8;
    let fault_cell = dip_coord.fract().rem_euclid(1.0);
    let fault_step = smoothstep(0.42, 0.58, fault_cell) - 0.5;
    let fault_plane = (1.0 - ((fault_cell - 0.5).abs() * 14.0).min(1.0))
        * (boundary_proximity * 0.75 + transform_shear * 0.55).min(1.0);
    let fault_activity = (boundary_proximity * (convergence + divergence * 0.85) + transform_shear * 0.9).min(1.2);
    let fault_throw = fault_step * fault_activity * 9.5 + (strike_coord.sin() * transform_shear * 3.2);

    TectonicSample {
        continentalness,
        boundary_stress,
        transform_shear,
        boundary_proximity,
        boundary_dist,
        boundary_normal: [nx, nz],
        fold_wave,
        orogeny,
        volcanism,
        trench,
        rift_graben,
        fault_throw,
        fault_plane,
        crust_age: p1.5 * margin_blend + 0.5 * (p1.5 + p2.5) * (1.0 - margin_blend),
    }
}

// V9+ combines deterministic seeded plate tectonics (oceans, continental shelves,
// subduction trenches, volcanic island/cordilleran arcs, rift grabens, and
// fold-and-thrust mountain belts) with connected, walkable valleys.
fn compact_bedrock(x: f32, z: f32, seed: u32, ridged: bool) -> f32 {
    let tect = sample_tectonics(x, z, seed);
    let ranges = value_noise2(x / 180.0, z / 180.0, seed.wrapping_add(97));
    let mountain_mask = smoothstep(if ridged { 0.26 } else { 0.34 }, 0.74, ranges);
    let ridge = 1.0 - (fbm2(x / 95.0, z / 95.0, seed.wrapping_add(137)) * 2.0 - 1.0).abs();
    let hills = (fbm2(x / 65.0, z / 65.0, seed.wrapping_add(71)) - 0.5) * 16.0;

    // Hypsometric curve from abyssal oceanic crust (-18..+4m) across the continental
    // shelf (+8..+14m) to continental interiors (+18..+36m).
    let shelf_t = smoothstep(-0.48, 0.22, tect.continentalness);
    let abyssal_t = smoothstep(-0.12, -0.78, tect.continentalness);
    let craton_t = smoothstep(0.05, 0.65, tect.continentalness);

    let ocean_floor = -4.5 - abyssal_t * 14.5 + hills * 0.28;
    let continental_base = 17.5 + craton_t * 11.5 + hills * (0.65 + 0.35 * craton_t);
    let mut h = ocean_floor * (1.0 - shelf_t) + continental_base * shelf_t;

    // 1. Orogenic fold-and-thrust mountain belts & cratonic ranges:
    let fold_ridges = (0.55 + 0.45 * tect.fold_wave) * ridge.powf(1.35);
    let orogenic_uplift = (tect.orogeny * 1.15 + mountain_mask * craton_t * 0.72)
        * fold_ridges
        * if ridged { 84.0 } else { 64.0 };
    h += orogenic_uplift;

    // 2. Volcanic arcs & seamount island chains (can breach sea level from oceanic plates!):
    let volcano_cone = ridge.powf(1.6) * (0.6 + 0.4 * mountain_mask);
    let volcanic_uplift = tect.volcanism * (24.0 + volcano_cone * 52.0);
    // High volcanic peaks occasionally feature a sunken summit caldera crater.
    let caldera = smoothstep(0.76, 0.94, tect.volcanism * ridge) * 14.0;
    h += volcanic_uplift - caldera;

    // 3. Oceanic subduction trench depression:
    h -= tect.trench * 18.5;

    // 4. Divergent continental rift graben & mid-ocean spreading ridge:
    let horst_shoulders = (-((tect.boundary_dist - 64.0) / 34.0).powi(2)).exp()
        * (-tect.boundary_stress).max(0.0);
    if tect.continentalness > -0.05 {
        // Sunken rift valley (graben) often deep enough to host rift lakes or sea inlets,
        // bordered by uplifted horst fault-block escarpments.
        h -= tect.rift_graben * 19.5;
        h += horst_shoulders * 13.5;
    } else {
        // Mid-ocean spreading ridge rising from the abyssal floor with a central axial cleft.
        h += horst_shoulders * 14.0 - tect.rift_graben * 6.0;
    }

    // 5. Fault-block scarps & strike-slip offset relief:
    h += tect.fault_throw * (0.45 + 0.55 * shelf_t);
    h
}

fn base_height(wx: f32, wz: f32, seed: u32, kind: u8, version: u32) -> f32 {
    let (sx, sz) = if version >= 3 {
        (
            wx + (value_noise2(wx / 230.0, wz / 230.0, seed.wrapping_add(9001)) - 0.5) * 140.0,
            wz + (value_noise2(wx / 270.0, wz / 270.0, seed.wrapping_add(9003)) - 0.5) * 140.0,
        )
    } else {
        (wx, wz)
    };
    let mut h = if version >= 9 {
        compact_bedrock(wx, wz, seed, kind == WorldType::Ridged as u8)
    } else {
        bedrock(sx, sz, seed, kind == WorldType::Ridged as u8)
    };
    if version >= 3 {
        let mountain = smoothstep(40.0, 95.0, h);
        let fold = (sx * 0.018 + (sz * 0.006).sin() * 2.0).sin().abs();
        h += mountain * fold.powf(1.8) * if version >= 9 { 9.0 } else { 28.0 };
        let (heat, wet) = climate(wx, wz, seed);
        let mesa = smoothstep(0.52, 0.75, heat) * (1.0 - smoothstep(0.32, 0.5, wet));
        let terrace = (h / 9.0).floor() * 9.0 + smoothstep(0.25, 0.85, (h / 9.0).fract()) * 9.0;
        h += (terrace - h) * mesa * 0.85;
    }
    h
}

// Visual far-field samples do not need a 48-step erosion simulation per region.
// Editable chunks and the inner backdrop rings retain the exact generator.
pub(crate) fn preview_surface(cfg: &WorldSampleConfig, x: i32, z: i32) -> (f32, BlockId) {
    if cfg.world_type == WorldType::Superflat || cfg.generator_version < 2 {
        return super::distant_surface(cfg, x, z);
    }
    let wx = cfg.world_x(x);
    let wz = cfg.world_z(z);
    let h0 = base_height(
        wx,
        wz,
        cfg.seed,
        cfg.world_type as u8,
        cfg.generator_version,
    );
    let hx = base_height(
        wx + 8.0,
        wz,
        cfg.seed,
        cfg.world_type as u8,
        cfg.generator_version,
    );
    let hz = base_height(
        wx,
        wz + 8.0,
        cfg.seed,
        cfg.world_type as u8,
        cfg.generator_version,
    );
    let slope0 = (((hx - h0) / 8.0).powi(2) + ((hz - h0) / 8.0).powi(2)).sqrt();
    let (h, slope, temperature, moisture) = apply_macro_shaping(cfg, wx, wz, h0, slope0, 0.25);
    let tect = if cfg.generator_version >= 9 {
        Some(sample_tectonics(wx, wz, cfg.seed))
    } else {
        None
    };
    let block = if h < 1.5 {
        BlockId::Basalt
    } else if h < 14.8 && slope < 0.42 {
        BlockId::Sand
    } else if tect.is_some_and(|t| t.volcanism > 0.56 && (h > 36.0 || slope > 0.42)) {
        BlockId::Basalt
    } else if h > 158.0 || slope > 0.86 {
        if temperature < 0.35 && h > 148.0 && slope < 0.65 {
            BlockId::SnowGrass
        } else {
            BlockId::Stone
        }
    } else if temperature > 0.63 && moisture < 0.38 {
        BlockId::Sand
    } else {
        BlockId::Grass
    };
    (
        cfg.clamp_height(cfg.blocks_for_height(h)) as f32 + 1.0,
        block,
    )
}

fn build_tile(key: TileKey) -> Tile {
    let (seed, kind, tx, tz, version) = key;
    let mut height = vec![0.0; COUNT];
    let mut water = vec![0.0; COUNT];
    let mut sediment = vec![0.0; COUNT];
    let mut deposit = vec![0.0; COUNT];
    let mut delta_h = vec![0.0; COUNT];
    let mut delta_w = vec![0.0; COUNT];
    let mut delta_s = vec![0.0; COUNT];
    for z in 0..EDGE {
        for x in 0..EDGE {
            let wx = tx as f32 * REGION + x as f32 * SPACING - REGION;
            let wz = tz as f32 * REGION + z as f32 * SPACING - REGION;
            let h = base_height(wx, wz, seed, kind, version);
            height[z * EDGE + x] = h;
        }
    }
    // Shallow-water transport: bounded downhill flux carries sediment; erosion
    // and deposition exchange exactly the same amount with the terrain.
    for step in 0..STEPS {
        delta_h.fill(0.0);
        delta_w.fill(0.0);
        delta_s.fill(0.0);
        for i in 0..COUNT {
            water[i] += 0.12;
        }
        for z in 1..EDGE - 1 {
            for x in 1..EDGE - 1 {
                let i = z * EDGE + x;
                let neighbors = [i - 1, i + 1, i - EDGE, i + EDGE];
                let mut drops = [0.0; 4];
                let mut sum = 0.0f32;
                for (n, &j) in neighbors.iter().enumerate() {
                    drops[n] = (height[i] + water[i] - height[j] - water[j]).max(0.0);
                    sum += drops[n];
                }
                if sum < 0.0001 {
                    continue;
                }
                let runoff = water[i].min(sum * 0.20);
                let capacity = (sum / (4.0 * SPACING)).min(3.0) * runoff * 1.8;
                let exchange = ((capacity - sediment[i]) * 0.22).clamp(-sediment[i], 0.16);
                delta_h[i] -= exchange;
                sediment[i] += exchange;
                if exchange < 0.0 {
                    deposit[i] -= exchange;
                }
                for (n, &j) in neighbors.iter().enumerate() {
                    let flow = runoff * drops[n] / sum;
                    let carried = sediment[i] * flow / water[i].max(0.001);
                    delta_w[i] -= flow;
                    delta_w[j] += flow;
                    delta_s[i] -= carried;
                    delta_s[j] += carried;
                }
                // Thermal creep moves unstable scree down slopes above ~35 degrees.
                if step % 3 == 0 {
                    let j = *neighbors
                        .iter()
                        .min_by(|&&a, &&b| height[a].total_cmp(&height[b]))
                        .unwrap();
                    let talus = ((height[i] - height[j] - SPACING * 0.7) * 0.08).clamp(0.0, 0.24);
                    delta_h[i] -= talus;
                    delta_h[j] += talus;
                    deposit[j] += talus;
                }
            }
        }
        for i in 0..COUNT {
            height[i] += delta_h[i];
            water[i] = ((water[i] + delta_w[i]).max(0.0) * 0.94).min(8.0);
            sediment[i] = (sediment[i] + delta_s[i]).max(0.0);
        }
    }
    for i in 0..COUNT {
        height[i] += sediment[i];
        deposit[i] += sediment[i];
    }
    Tile {
        height,
        sediment: deposit,
    }
}

thread_local! {
    // Column queries reuse four overlapping tiles. A tiny worker-local cache
    // removes thousands of shared locks from each generated voxel chunk.
    static HOT_TILES:std::cell::RefCell<VecDeque<(TileKey,Arc<Tile>)>>=const {std::cell::RefCell::new(VecDeque::new())};
}
fn tile(key: TileKey) -> Arc<Tile> {
    if let Some(hit) = HOT_TILES.with(|hot| {
        let mut hot = hot.borrow_mut();
        let i = hot.iter().position(|(k, _)| *k == key)?;
        let entry = hot.remove(i).unwrap();
        let result = Arc::clone(&entry.1);
        hot.push_back(entry);
        Some(result)
    }) {
        return hit;
    }
    let cache = TILES.get_or_init(|| Mutex::new(TileCache::default()));
    let cell = {
        let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cell) = cache.entries.get(&key).cloned() {
            if let Some(i) = cache.recent.iter().position(|k| *k == key) {
                cache.recent.remove(i);
            }
            cache.recent.push_back(key);
            cell
        } else {
            // Never evict an in-flight initialization: neighboring workers join
            // the same erosion calculation instead of rebuilding it in parallel.
            if cache.entries.len() >= 256 {
                if let Some(i) = cache
                    .recent
                    .iter()
                    .position(|k| cache.entries.get(k).is_some_and(|c| c.get().is_some()))
                {
                    let old = cache.recent.remove(i).unwrap();
                    cache.entries.remove(&old);
                }
            }
            let cell = Arc::new(OnceLock::new());
            cache.entries.insert(key, Arc::clone(&cell));
            cache.recent.push_back(key);
            cell
        }
    };
    let result = Arc::clone(cell.get_or_init(|| Arc::new(build_tile(key))));
    HOT_TILES.with(|hot| {
        let mut hot = hot.borrow_mut();
        if hot.len() >= 8 {
            hot.pop_front();
        }
        hot.push_back((key, Arc::clone(&result)));
    });
    result
}

fn bilinear(field: &[f32], x: f32, z: f32) -> f32 {
    let x = x.clamp(0.0, (EDGE - 1) as f32 - 0.001);
    let z = z.clamp(0.0, (EDGE - 1) as f32 - 0.001);
    let ix = x.floor() as usize;
    let iz = z.floor() as usize;
    let a = field[iz * EDGE + ix] * (1.0 - x.fract()) + field[iz * EDGE + ix + 1] * x.fract();
    let b = field[(iz + 1) * EDGE + ix] * (1.0 - x.fract())
        + field[(iz + 1) * EDGE + ix + 1] * x.fract();
    a * (1.0 - z.fract()) + b * z.fract()
}

pub fn sample(cfg: &WorldSampleConfig, x: i32, z: i32) -> Landscape {
    let wx = cfg.world_x(x);
    let wz = cfg.world_z(z);
    let tx = (wx / REGION).floor() as i32;
    let tz = (wz / REGION).floor() as i32;
    let fx = smoothstep(0.0, 1.0, (wx / REGION).fract().rem_euclid(1.0));
    let fz = smoothstep(0.0, 1.0, (wz / REGION).fract().rem_euclid(1.0));
    let mut h = 0.0;
    let mut sediment = 0.0;
    let mut slope = 0.0;
    for dz in 0..=1 {
        for dx in 0..=1 {
            let t = tile((
                cfg.seed,
                cfg.world_type as u8,
                tx + dx,
                tz + dz,
                cfg.generator_version,
            ));
            let u = (wx - (tx + dx) as f32 * REGION + REGION) / SPACING;
            let v = (wz - (tz + dz) as f32 * REGION + REGION) / SPACING;
            let weight = if dx == 0 { 1.0 - fx } else { fx } * if dz == 0 { 1.0 - fz } else { fz };
            h += bilinear(&t.height, u, v) * weight;
            sediment += bilinear(&t.sediment, u, v) * weight;
            let sx = (bilinear(&t.height, u + 0.5, v) - bilinear(&t.height, u - 0.5, v)) / SPACING;
            let sz = (bilinear(&t.height, u, v + 0.5) - bilinear(&t.height, u, v - 0.5)) / SPACING;
            slope += (sx * sx + sz * sz).sqrt() * weight;
        }
    }
    let (h, slope, temperature, moisture) = apply_macro_shaping(cfg, wx, wz, h, slope, sediment);
    let alpine = if cfg.generator_version >= 7 {
        h > 190.0 || (h > 158.0 && temperature < 0.20)
    } else {
        h > if cfg.generator_version >= 5 {
            105.0
        } else {
            85.0
        } || slope
            > if cfg.generator_version >= 5 {
                1.25
            } else {
                0.85
            }
    };
    let biome = if cfg.generator_version >= 3 && wx.hypot(wz) < 110.0 {
        if wx > 30.0 {
            Biome::Forest
        } else {
            Biome::Meadow
        }
    } else if h < 10.5 {
        Biome::Ocean
    } else if h < 14.8 && slope < 0.42 {
        Biome::Coast
    } else if alpine {
        // Keep alpine rock and snow on the high summits. Slope is handled as
        // exposed rock below; using it as a biome override erased forests from
        // every steep hillside.
        Biome::Alpine
    } else if temperature < 0.24 {
        if moisture > 0.42 {
            Biome::Taiga
        } else {
            Biome::Tundra
        }
    } else if desert_climate(temperature, moisture) {
        if slope > 0.20 {
            Biome::Badlands
        } else {
            Biome::Desert
        }
    } else if cfg.generator_version >= 10 && moisture > 0.78 && h >= 16.5 && h < 24.0 && slope < 0.14 {
        Biome::Fen
    } else if cfg.generator_version >= 10 && temperature > 0.56 && moisture > 0.68 {
        Biome::Rainforest
    } else if cfg.generator_version >= 10 && savanna_climate(temperature, moisture) {
        Biome::Savanna
    } else if cfg.generator_version >= 10 && temperature < 0.44 && moisture < 0.58 && h > 32.0 {
        Biome::Heathland
    } else if moisture > 0.55 {
        Biome::Forest
    } else {
        Biome::Meadow
    };
    Landscape {
        height: h,
        slope,
        temperature,
        moisture,
        sediment,
        biome,
    }
}

fn apply_macro_shaping(
    cfg: &WorldSampleConfig,
    wx: f32,
    wz: f32,
    mut h: f32,
    mut slope: f32,
    sediment: f32,
) -> (f32, f32, f32, f32) {
    let (mut temperature, mut rain) = climate(wx, wz, cfg.seed);
    if cfg.generator_version >= 5 {
        // Preserve coherent weather regions while making the dry/cold extremes
        // reachable; interpolated climate noise otherwise favours green forest.
        temperature = ((temperature - 0.5) * 1.55 + 0.5).clamp(0.0, 1.0);
        rain = ((rain - 0.5) * 1.4 + 0.5).clamp(0.0, 1.0);
    }
    // Generator 7 uses a gentler normalized lapse rate. The previous rate drove
    // the climate field to zero on ordinary highlands, making a single snowy
    // alpine biome swallow most of the visible mountain range.
    let lapse_rate = if cfg.generator_version >= 7 {
        0.0018
    } else {
        0.0035
    };
    let temperature = (temperature - (h - 30.0).max(0.0) * lapse_rate).clamp(0.0, 1.0);
    let moisture = (rain + sediment.min(2.0) * 0.06).clamp(0.0, 1.0);
    let channel_noise = value_noise2(wx / 160.0, wz / 160.0, cfg.seed.wrapping_add(109));
    let channel = 1.0 - smoothstep(0.012, 0.065, (channel_noise - 0.5).abs());
    let lowland = 1.0 - smoothstep(22.0, 58.0, h);
    h -= channel * lowland * (h - 9.5).max(0.0) * smoothstep(0.2, 0.7, moisture);
    // Damp high-frequency 6m surface chatter right across the shoreline band
    // (10.5..15.5m) so beaches do not develop isolated 1-voxel water slits/puddles.
    let shore_calm = smoothstep(0.85, 3.6, (h - 12.0).abs());
    h += (value_noise2(wx / 6.0, wz / 6.0, cfg.seed.wrapping_add(83)) - 0.5)
        * (0.65 + slope.min(1.0) * 1.3)
        * shore_calm;
    if cfg.generator_version >= 3 {
        // A real inland watershed, not a platform placed after world generation.
        // The central clearing is dry, gently graded and joins 320 m of dry land.
        let distance = wx.hypot(wz);
        let v7 = cfg.generator_version >= 7;
        let inland = 1.0
            - smoothstep(
                if v7 { 110.0 } else { 160.0 },
                if v7 { 380.0 } else { 520.0 },
                distance,
            );
        let floor =
            27.0 + (value_noise2(wx / 90.0, wz / 90.0, cfg.seed.wrapping_add(9011)) - 0.5) * 5.0;
        h += (floor - h).max(0.0) * inland;
        let clearing = 1.0
            - smoothstep(
                if v7 { 28.0 } else { 95.0 },
                if cfg.generator_version >= 9 {
                    55.0
                } else if v7 {
                    100.0
                } else {
                    175.0
                },
                distance,
            );
        let basin = if v7 {
            26.0 + (fbm2(wx / 58.0, wz / 58.0, cfg.seed.wrapping_add(9023)) - 0.5) * 18.0
                + (value_noise2(wx / 16.0, wz / 16.0, cfg.seed.wrapping_add(9041)) - 0.5) * 2.5
        } else {
            28.0 + (fbm2(wx / 70.0, wz / 70.0, cfg.seed.wrapping_add(9023)) - 0.5) * 9.0
        };
        h = h * (1.0 - clearing) + basin * clearing;
        let spawn_pad = 1.0 - smoothstep(6.0, if v7 { 14.0 } else { 20.0 }, distance);
        let spawn_height = if v7 {
            26.0 + (value_noise2(wx / 22.0, wz / 22.0, cfg.seed.wrapping_add(9053)) - 0.5) * 1.2
        } else {
            28.0
        };
        h = h * (1.0 - spawn_pad) + spawn_height * spawn_pad;
        slope = slope * (1.0 - clearing) + 0.08 * clearing * (1.0 - spawn_pad);
    }
    if cfg.generator_version >= 5 {
        // Residual landforms below the erosion grid's 8 m sampling interval.
        // Soft soil has hummocks, dry ground has wind-aligned dunes, exposed
        // uplands have broken ribs and gullies rather than uniformly noisy hills.
        let a = value_noise2(wx / 23.0, wz / 23.0, cfg.seed.wrapping_add(9203));
        let b = value_noise2(wx / 7.0, wz / 7.0, cfg.seed.wrapping_add(9209));
        let c = value_noise2(wx / 2.7, wz / 2.7, cfg.seed.wrapping_add(9221));
        let shore_detail = smoothstep(1.1, 3.8, (h - 12.0).abs());
        let b_eff = 0.5 + (b - 0.5) * shore_detail;
        let c_eff = 0.5 + (c - 0.5) * shore_detail;
        let exposed = smoothstep(0.12, 0.75, slope).max(smoothstep(48.0, 105.0, h));
        let rib = (1.0 - (a * 2.0 - 1.0).abs()).powi(3);
        let broken = (rib - 0.35) * 9.0 + (b_eff - 0.5) * 4.0 + (c_eff - 0.5) * 1.3;
        let hummock = (a - 0.5) * 3.0 + (b_eff - 0.5) * 1.5;
        let dunes = (wx * 0.085 + wz * 0.032 + a * 4.0).sin() * 2.2;
        let arid = smoothstep(0.55, 0.8, temperature) * (1.0 - smoothstep(0.2, 0.4, moisture));
        let residual = (hummock * (1.0 - arid) + dunes * arid) * (1.0 - exposed) + broken * exposed;
        let strength = smoothstep(20.0, 65.0, wx.hypot(wz)) * smoothstep(12.2, 19.0, h);
        h += residual
            * strength
            * if cfg.generator_version >= 9 {
                0.45
            } else {
                1.0
            };
        slope += (b_eff - 0.5).abs() * exposed * 0.25 * strength;
    }
    // Prevent shoreline heights from lingering in the 11.2..12.15m rounding band
    // where 1m voxel quantization creates isolated 1-block water lines on beaches.
    if channel < 0.48 && h > 11.15 && h < 12.25 {
        h = if h >= 11.42 {
            12.28 + (h - 11.42) * 0.65
        } else {
            11.05 - (11.42 - h) * 0.40
        };
    }
    (h, slope, temperature, moisture)
}

pub(super) fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    if cfg.world_type == WorldType::Superflat {
        return ColumnSample {
            height: cfg.blocks_for_height(16.0),
            surface: BlockId::Grass,
            filler: BlockId::Dirt,
            vegetation_density: 0.0,
        };
    }
    let s = sample(cfg, x, z);
    let (mut surface, mut filler, mut vegetation) = match s.biome {
        Biome::Ocean | Biome::Coast => (BlockId::Sand, BlockId::Sandstone, 0.0),
        Biome::Marsh => (BlockId::Mud, BlockId::Clay, 0.25),
        Biome::Desert => (BlockId::Sand, BlockId::Sandstone, 0.0),
        Biome::Badlands => (BlockId::Clay, BlockId::Sandstone, 0.0),
        Biome::Alpine => (
            if s.temperature < 0.35 {
                BlockId::SnowGrass
            } else {
                BlockId::Stone
            },
            BlockId::Stone,
            0.0,
        ),
        Biome::Tundra => (
            if cfg.generator_version >= 7 {
                BlockId::Grass
            } else {
                BlockId::SnowGrass
            },
            BlockId::Dirt,
            0.05,
        ),
        Biome::Taiga => (
            if cfg.generator_version >= 7 {
                BlockId::Grass
            } else {
                BlockId::SnowGrass
            },
            BlockId::Dirt,
            0.8,
        ),
        Biome::Forest => (BlockId::Grass, BlockId::Dirt, 1.0),
        Biome::Meadow => (BlockId::Grass, BlockId::Dirt, 0.12),
        Biome::Savanna => (BlockId::Grass, BlockId::Dirt, 0.16),
        Biome::Rainforest => (BlockId::Grass, BlockId::Dirt, 1.12),
        Biome::Heathland => (BlockId::Grass, BlockId::Gravel, 0.055),
        Biome::Fen => (BlockId::Grass, BlockId::Clay, 0.28),
    };
    let exposed_slope = if cfg.generator_version >= 7 {
        0.86
    } else {
        0.65
    };
    if s.slope > exposed_slope && s.height > 15.0 {
        surface = if s.sediment > 0.7 {
            BlockId::Gravel
        } else {
            BlockId::Stone
        };
    }
    if cfg.generator_version >= 5 {
        let wx = cfg.world_x(x);
        let wz = cfg.world_z(z);
        let patch = value_noise2(wx / 19.0, wz / 19.0, cfg.seed.wrapping_add(9341));
        let grove = value_noise2(wx / 61.0, wz / 61.0, cfg.seed.wrapping_add(9349));
        // Snow collects in sheltered pockets, not on every stair-step top.
        // Snow coverage follows the continuous climate field on both sides of
        // the biome threshold. Gating it to cold-biome labels made a hard seam
        // where temperature crossed 0.24, regardless of the local snow patch.
        if matches!(
            s.biome,
            Biome::Alpine | Biome::Tundra | Biome::Taiga | Biome::Forest | Biome::Meadow | Biome::Heathland
        ) {
            if cfg.generator_version >= 7 {
                let cold = 1.0 - smoothstep(0.08, 0.52, s.temperature);
                let snowline = smoothstep(138.0, 205.0, s.height);
                let sheltered = 1.0 - smoothstep(0.18, 1.10, s.slope);
                let cold_weight = if cfg.generator_version >= 9 {
                    0.16
                } else {
                    0.50
                };
                let snow = cold * cold_weight + snowline * 0.55 + patch * 0.35 + sheltered * 0.10
                    - s.slope * 0.16;
                surface = if snow > 0.67 {
                    BlockId::SnowGrass
                } else if s.slope > 0.55 {
                    BlockId::Stone
                } else if matches!(surface, BlockId::SnowGrass) {
                    BlockId::Grass
                } else {
                    surface
                };
            } else {
                let snow = (0.34 - s.temperature) * 3.0 + patch * 0.62 - s.slope * 0.38;
                surface = if snow > 0.38 {
                    BlockId::SnowGrass
                } else if s.slope > 0.5 {
                    BlockId::Stone
                } else if matches!(surface, BlockId::SnowGrass) {
                    BlockId::Grass
                } else {
                    surface
                };
            }
        }
        if matches!(s.biome, Biome::Forest | Biome::Meadow) {
            vegetation *= 0.25 + smoothstep(0.25, 0.7, grove) * 0.95;
            if s.slope > 0.35 && patch > 0.69 {
                surface = BlockId::Gravel;
            }
            // Coastal transition: gentle slopes near sea level blend sand and
            // coastal shingle into turf instead of dark mud stripes.
            if s.height < 16.0 && s.slope < 0.36 {
                let dune = value_noise2(wx / 11.0, wz / 11.0, cfg.seed.wrapping_add(9371));
                if s.height < 14.8 + dune * 1.1 {
                    surface = if dune > 0.76 && s.height > 14.2 {
                        BlockId::Gravel
                    } else {
                        BlockId::Sand
                    };
                    filler = BlockId::Sandstone;
                    vegetation = 0.0;
                }
            } else if s.moisture > 0.84 && s.height >= 17.5 && s.height < 23.0 && s.slope < 0.14 && patch > 0.68 {
                surface = BlockId::Mud;
                filler = BlockId::Clay;
            }
        }
        if matches!(s.biome, Biome::Badlands) {
            surface = if patch > 0.60 {
                BlockId::Sandstone
            } else {
                BlockId::Clay
            };
        }
        if cfg.generator_version >= 10 {
            match s.biome {
                Biome::Savanna => {
                    vegetation *= 0.35 + smoothstep(0.35, 0.72, grove);
                    if patch < 0.25 && s.slope < 0.35 { surface = BlockId::Dirt; }
                }
                Biome::Rainforest => {
                    vegetation *= 0.65 + grove * 0.45;
                    if patch > 0.72 && s.slope < 0.25 { surface = BlockId::Mud; }
                }
                Biome::Heathland => {
                    if patch > 0.66 && s.slope > 0.22 { surface = BlockId::Gravel; }
                }
                Biome::Fen => {
                    // Grass islands support trees; soft clay/mud fills hollows.
                    if patch < 0.54 { surface = BlockId::Mud; vegetation = 0.0; }
                    else { vegetation *= smoothstep(0.54, 0.82, patch); }
                }
                _ => {}
            }
        }
        if cfg.generator_version >= 9 && wx.hypot(wz) > 110.0 {
            let tect = sample_tectonics(wx, wz, cfg.seed);
            if matches!(s.biome, Biome::Ocean) {
                if s.height < 1.0 || tect.trench > 0.35 {
                    // Abyssal basaltic ocean floor & subduction trench ooze.
                    surface = if patch > 0.34 {
                        BlockId::Basalt
                    } else {
                        BlockId::Clay
                    };
                    filler = BlockId::Basalt;
                } else if s.height < 6.0 {
                    // Outer continental shelf carbonate platform & shingle.
                    surface = if patch > 0.68 {
                        BlockId::Limestone
                    } else if patch < 0.22 {
                        BlockId::Gravel
                    } else {
                        BlockId::Sand
                    };
                    filler = BlockId::Limestone;
                }
            } else if tect.volcanism > 0.54 && (s.slope > 0.34 || s.height > 42.0) {
                // Active volcanic arc cones & basaltic rift shields.
                surface = if patch > 0.74 {
                    BlockId::Gravel
                } else {
                    BlockId::Basalt
                };
                filler = BlockId::Basalt;
                vegetation *= 0.12;
            } else if (s.slope > 0.68 || (tect.fault_plane > 0.62 && s.slope > 0.40)) && s.height > 15.0 {
                // Steep cliff faces & fault scarps expose the tilted 3D stratigraphic bed.
                let bed = sample_stratigraphic_bed(wx, s.height, wz, cfg.seed, &tect);
                surface = if tect.fault_plane > 0.72 && patch > 0.55 {
                    BlockId::Gravel
                } else {
                    bed
                };
                filler = bed;
            }
        }
        // Exposed rock has rock beneath it. A thin stone cap over dirt made
        // steep slopes a distracting brown/grey checkerboard at every step.
        if matches!(surface, BlockId::Stone | BlockId::Gravel) {
            filler = BlockId::Stone;
        } else if matches!(surface, BlockId::Basalt | BlockId::Limestone | BlockId::Sandstone) {
            filler = surface;
        }
    }
    ColumnSample {
        height: cfg.clamp_height(cfg.blocks_for_height(s.height)),
        surface,
        filler,
        vegetation_density: vegetation,
    }
}

fn sample_stratigraphic_bed(
    wx: f32,
    wy: f32,
    wz: f32,
    seed: u32,
    tect: &TectonicSample,
) -> BlockId {
    // Paleostratigraphic coordinate tilted along plate dip, folded in orogenic belts,
    // and displaced vertically across fault planes.
    let dip_coord = wx * tect.boundary_normal[0] + wz * tect.boundary_normal[1];
    let dip_tilt = (dip_coord * 0.045).sin() * (4.0 + tect.boundary_proximity * 6.5);
    let fold_disp = tect.fold_wave * (5.0 + tect.orogeny * 13.5);
    let warp = (value_noise2(wx / 54.0, wz / 54.0, seed.wrapping_add(4409)) - 0.5) * 4.5;
    let strat_y = wy + dip_tilt + fold_disp + tect.fault_throw * 1.15 + warp;

    // Igneous basaltic sill/dike intrusions in volcanic & rift provinces.
    if tect.volcanism > 0.46 || tect.rift_graben > 0.48 {
        let dike = (dip_coord * 0.14 + wy * 0.08 + warp * 0.4).sin().abs();
        if dike < 0.16 + tect.volcanism * 0.18 {
            return BlockId::Basalt;
        }
    }

    let cycle = (strat_y / 11.0).rem_euclid(1.0);
    if cycle < 0.22 {
        if tect.continentalness < 0.18 || strat_y < 18.0 {
            BlockId::Limestone
        } else {
            BlockId::Sandstone
        }
    } else if cycle < 0.36 && tect.crust_age > 0.45 {
        BlockId::Sandstone
    } else if cycle > 0.84 && tect.orogeny > 0.32 {
        BlockId::Basalt
    } else {
        BlockId::Stone
    }
}

pub(super) fn geological_stratum(
    cfg: &WorldSampleConfig,
    p: glam::IVec3,
    column_height: i32,
    depth: i32,
) -> BlockId {
    let meters = p.as_vec3() * cfg.voxel_meters();
    let below = depth as f32 * cfg.voxel_meters();
    let vein = super::value_noise3(
        meters.x * 0.24,
        meters.y * 0.24,
        meters.z * 0.24,
        cfg.seed.wrapping_add(883),
    );
    let fleck = super::hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(887));

    if cfg.generator_version >= 9 {
        let tect = sample_tectonics(meters.x, meters.z, cfg.seed);
        let dip_coord = meters.x * tect.boundary_normal[0] + meters.z * tect.boundary_normal[1];
        let strike_coord = -meters.x * tect.boundary_normal[1] + meters.z * tect.boundary_normal[0];

        // 1. Subducting oceanic slab & deep basaltic lithosphere:
        let moho_depth = if tect.continentalness < -0.15 {
            14.0
        } else {
            68.0 - tect.volcanism * 28.0 - tect.rift_graben * 24.0
        };
        if below > moho_depth {
            return BlockId::Basalt;
        }

        // 2. Active volcanic feeder pipes & basaltic dikes:
        if tect.volcanism > 0.38 || tect.rift_graben > 0.45 {
            let pipe_nx = value_noise2(meters.x / 34.0, meters.z / 34.0, cfg.seed.wrapping_add(4513));
            let pipe_nz = value_noise2(meters.x / 31.0, meters.z / 31.0, cfg.seed.wrapping_add(4523));
            let pipe_strength = pipe_nx * pipe_nz * (tect.volcanism + tect.rift_graben * 0.65);
            // Safely encased deep magma conduit core + surrounding basalt wall & porphyry ore halo:
            if pipe_strength > 0.46 && below > 9.0 && meters.y < (column_height as f32 * cfg.voxel_meters() - 8.0) {
                if pipe_strength > 0.56 && below > 14.0 {
                    return BlockId::Lava;
                }
                return BlockId::Basalt;
            }
            if pipe_strength > 0.34 {
                if fleck > 0.58 && vein > 0.56 {
                    return if meters.y > 0.0 {
                        BlockId::CopperOre
                    } else {
                        BlockId::IronOre
                    };
                }
                return BlockId::Basalt;
            }
        }

        // 3. Fault-plane breccia & hydrothermal mineralization along tectonic sutures:
        if tect.fault_plane > 0.68 {
            if vein > 0.60 && fleck > 0.35 {
                return if meters.y > -4.0 {
                    BlockId::CopperOre
                } else {
                    BlockId::TinOre
                };
            }
            if fleck > 0.48 {
                return BlockId::Gravel;
            }
        }

        // 4. 3D Tilted, Folded, and Fault-Offset Sedimentary & Metamorphic Stratigraphy:
        let dip_tilt = (dip_coord * 0.045).sin() * (4.0 + tect.boundary_proximity * 6.5);
        let fold_disp = tect.fold_wave * (5.0 + tect.orogeny * 13.5);
        let strat_y = meters.y + dip_tilt + fold_disp + tect.fault_throw * 1.15 + (vein - 0.5) * 2.2;
        let bed_phase = (strat_y / 9.5).rem_euclid(1.0);

        // Bedded Carboniferous coal seams & ancient cratonic banded ironstone formations:
        if (0.47..0.54).contains(&bed_phase) && vein > 0.62 && fleck > 0.34 {
            return if tect.crust_age > 0.54 || strat_y < 6.0 {
                BlockId::IronOre
            } else {
                BlockId::CoalOre
            };
        }

        // Orogenic & subduction porphyry copper/tin seams:
        let seam = super::value_noise3(
            meters.x * 0.09 + tect.fold_wave * 0.4,
            strat_y * 0.15,
            meters.z * 0.09 + strike_coord * 0.01,
            cfg.seed.wrapping_add(1201),
        );
        let ore_threshold = 0.68 - (tect.orogeny * 0.05 + tect.volcanism * 0.06);
        if seam > ore_threshold && fleck > 0.25 {
            return if meters.y > 0.0 {
                BlockId::CopperOre
            } else {
                BlockId::TinOre
            };
        }

        // Paleo-marine limestone carbonate platforms & tilted sandstone/shale cyclothems:
        if bed_phase < 0.20 && (tect.continentalness < 0.24 || (-16.0..18.0).contains(&strat_y)) {
            return BlockId::Limestone;
        }
        if (0.20..0.33).contains(&bed_phase) && strat_y > -6.0 && tect.crust_age > 0.38 && vein < 0.52 {
            return BlockId::Sandstone;
        }
        if (0.72..0.79).contains(&bed_phase) && below < 22.0 && vein < 0.42 {
            return BlockId::Clay;
        }
    }

    if cfg.generator_version >= 3 {
        let seam = super::value_noise3(
            meters.x * 0.09,
            meters.y * 0.16,
            meters.z * 0.09,
            cfg.seed.wrapping_add(1201),
        );
        if seam > 0.67 && fleck > 0.25 {
            return if meters.y > 0.0 {
                BlockId::CopperOre
            } else {
                BlockId::TinOre
            };
        }
        if meters.y > -12.0 && meters.y < 6.0 && vein < 0.36 {
            return BlockId::Limestone;
        }
    }
    if vein > 0.72 && fleck > 0.38 {
        return if meters.y < 8.0 {
            BlockId::IronOre
        } else {
            BlockId::CoalOre
        };
    }
    if cfg.generator_version >= 13 {
        if below > 78.0 {
            return BlockId::Basalt;
        }
        if below > 54.0 && fleck > 0.45 {
            return BlockId::Ice;
        }
        if below > 30.0 && fleck > 0.62 {
            return BlockId::FungalSoil;
        }
    } else if meters.y < -24.0 {
        return BlockId::Basalt;
    }
    BlockId::Stone
}

#[cfg(test)]
mod tectonics_and_oceans {
    use super::*;
    use crate::fluid::VoxelScalePreset;

    #[test]
    fn seeded_tectonics_creates_oceans_coasts_mountain_belts_and_geological_strata() {
        let world = crate::world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 17);
        let cfg = world.sample_config();

        let mut ocean_columns = 0;
        let mut coast_columns = 0;
        let mut land_columns = 0;
        let mut volcanic_or_orogenic = 0;
        let mut strata_blocks = std::collections::HashSet::new();

        for gz in -8..=8 {
            for gx in -8..=8 {
                let x = gx * 120;
                let z = gz * 120;
                let s = sample(&cfg, x, z);
                let tect = sample_tectonics(x as f32, z as f32, cfg.seed);
                if tect.orogeny > 0.22 || tect.volcanism > 0.25 || tect.trench > 0.20 || tect.rift_graben > 0.20 {
                    volcanic_or_orogenic += 1;
                }
                match s.biome {
                    Biome::Ocean => ocean_columns += 1,
                    Biome::Coast => coast_columns += 1,
                    _ => land_columns += 1,
                }
                let col = column(&cfg, x, z);
                for d in [4, 10, 18, 28, 42] {
                    let b = geological_stratum(&cfg, glam::IVec3::new(x, col.height - d, z), col.height, d);
                    strata_blocks.insert(b);
                }
            }
        }

        assert!(ocean_columns >= 12, "expected true oceans across tectonic grid, got {ocean_columns}");
        assert!(land_columns >= 40, "expected continents across tectonic grid, got {land_columns}");
        assert!(volcanic_or_orogenic >= 10, "expected active tectonic boundaries, got {volcanic_or_orogenic}");
        assert!(strata_blocks.contains(&BlockId::Limestone), "expected marine Limestone strata");
        assert!(strata_blocks.contains(&BlockId::Basalt), "expected Basalt lithosphere/dikes");
        assert!(strata_blocks.contains(&BlockId::Sandstone) || coast_columns > 0, "expected Sandstone strata or coastal shelves");
    }
}
