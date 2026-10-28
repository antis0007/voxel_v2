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
// The horizon samples exact terrain and trees out to 4.2 km, about 3,500 tiles,
// in several passes. At 1,024 each pass evicted the last one's tiles and rebuilt
// them at ~5-14 ms each. A tile is ~9 KB, so the cap bounds the cache near 75 MB.
const MAX_TILES: usize = 8192;

// Shared desert exclusion keeps distant vegetation consistent with terrain.
fn desert_climate(temperature: f32, moisture: f32) -> bool {
    temperature > 0.63 && moisture < 0.38
}

// Generator-10 savanna climate, also used by distant vegetation. Older hot,
// very dry desert rules take priority. Preserve these thresholds for old saves.
pub fn savanna_climate(temperature: f32, moisture: f32) -> bool {
    temperature > 0.56 && moisture < 0.49 && !desert_climate(temperature, moisture)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Generator 53 stretches the climate fields about threefold so a biome
/// region spans kilometres instead of a 700 m patchwork.
pub const CLIMATE_SCALE_GENERATOR: u32 = 53;

/// Raw temperature and rainfall fields, each 0..1 (about 0.14..0.86 in
/// practice), before the contrast, ecotone and regional stages in
/// `lowland_climate`. Pure function of metre position, seed and version.
pub(crate) fn climate(x: f32, z: f32, seed: u32, version: u32) -> (f32, f32) {
    if version >= CLIMATE_SCALE_GENERATOR {
        return climate_regional(x, z, seed);
    }
    let wx = x + (value_noise2(x / 420.0, z / 420.0, seed.wrapping_add(7)) - 0.5) * 180.0;
    let wz = z + (value_noise2(x / 390.0, z / 390.0, seed.wrapping_add(9)) - 0.5) * 180.0;
    let temp = value_noise2(wx / 700.0, wz / 700.0, seed.wrapping_add(11));
    let rain = value_noise2(wx / 520.0, wz / 520.0, seed.wrapping_add(23));
    (temp, rain)
}

/// Rotated, two-octave climate at kilometre scale. Each octave turns its own
/// way so the value-noise lattice never lines regions up into squares.
fn climate_regional(x: f32, z: f32, seed: u32) -> (f32, f32) {
    let turn = |x: f32, z: f32, (c, s): (f32, f32)| (x * c - z * s, x * s + z * c);
    let n = |(x, z): (f32, f32), w: f32, salt: u32| value_noise2(x / w, z / w, seed.wrapping_add(salt));
    let (a, b, c, d) = ((0.82, 0.57), (0.39, -0.92), (-0.66, 0.75), (0.95, 0.31));
    let warp_x = (n(turn(x, z, a), 1300.0, 7) - 0.5) * 560.0 + (n(turn(x, z, b), 410.0, 8) - 0.5) * 120.0;
    let warp_z = (n(turn(x, z, c), 1210.0, 9) - 0.5) * 560.0 + (n(turn(x, z, d), 380.0, 10) - 0.5) * 120.0;
    let (wx, wz) = (x + warp_x, z + warp_z);
    let temp = n(turn(wx, wz, b), 2100.0, 11) * 0.72 + n(turn(wx, wz, c), 820.0, 12) * 0.28;
    let rain = n(turn(wx, wz, d), 1600.0, 23) * 0.72 + n(turn(wx, wz, a), 640.0, 24) * 0.28;
    // Summing octaves narrows the spread; restore the single-octave range.
    let spread = |v: f32| ((v - 0.5) * 1.3 + 0.5).clamp(0.0, 1.0);
    (spread(temp), spread(rain))
}

/// Heat and wetness (0..1) below the 30 m lapse start.
pub fn lowland_climate(cfg: &WorldSampleConfig, x: f32, z: f32) -> (f32, f32) {
    let (mut heat, mut wet) = climate(x, z, cfg.seed, cfg.generator_version);
    if cfg.generator_version >= 5 {
        heat = ((heat - 0.5) * 1.55 + 0.5).clamp(0.0, 1.0);
        wet = ((wet - 0.5) * 1.4 + 0.5).clamp(0.0, 1.0);
    }
    if cfg.generator_version >= 15 {
        let soft = cfg.generator_version >= CLIMATE_SCALE_GENERATOR;
        (heat, wet) = super::terrain_variety::climate(x, z, cfg.seed, heat, wet, soft);
    }
    if cfg.generator_version >= 33 {
        (heat, wet) = super::terrain_variety::regional_climate(x, z, cfg.seed, heat, wet);
    }
    (heat, wet)
}

/// `lowland_climate` heat at `height` metres.
pub fn lapse_heat(cfg: &WorldSampleConfig, heat: f32, height: f32) -> f32 {
    let per_metre = if cfg.generator_version >= 7 { 0.0018 } else { 0.0035 };
    (heat - (height - 30.0).max(0.0) * per_metre).clamp(0.0, 1.0)
}

/// Cheap far-field ecology, using the same climate as editable terrain.
/// No erosion tiles are allocated to place visual vegetation.
pub fn visual_ecology(
    cfg: &WorldSampleConfig,
    x: f32,
    z: f32,
    height: f32,
) -> (f32, f32, f32) {
    let (heat, wet) = lowland_climate(cfg, x, z);
    let heat = lapse_heat(cfg, heat, height);
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

/// Generator 15 stretches plates, belts and shelves by this factor so continents are
/// several kilometres across instead of 500 m patchwork. All profile widths stretch with it.
pub fn tectonic_zoom(generator_version: u32) -> f32 {
    if generator_version >= 15 { 2.4 } else { 1.0 }
}

type TectonicKey = (u32, u32, u32, u32, bool);

thread_local! {
    // Tectonics depend only on XZ. Cave-column generation used to recompute the
    // full plate solve for every solid voxel; this keeps it to once per column
    // per worker while preserving exact results.
    static TECTONIC_CACHE: std::cell::RefCell<engine_core::fast_hash::FastMap<TectonicKey, TectonicSample>>
        = std::cell::RefCell::new(engine_core::fast_hash::FastMap::default());
    // Voxel loops walk y fastest, so the next query is nearly always the same
    // column. One remembered entry answers it without hashing at all.
    static TECTONIC_LAST: std::cell::Cell<Option<(TectonicKey, TectonicSample)>>
        = const { std::cell::Cell::new(None) };
}

/// Generator 53: continents dominate. Oceanic plates are rarer, and the plates
/// beside the origin are no longer forced oceanic, so spawn sits inland and the
/// sea is a coast some way off rather than a basin next door.
pub const LAND_GENERATOR: u32 = 53;

/// Plate sample at a generator version's zoom and continental bias.
pub fn sample_tectonics(wx: f32, wz: f32, seed: u32, version: u32) -> TectonicSample {
    sample_tectonics_with(wx, wz, seed, tectonic_zoom(version), version >= LAND_GENERATOR)
}

pub fn sample_tectonics_zoomed(wx: f32, wz: f32, seed: u32, zoom: f32) -> TectonicSample {
    sample_tectonics_with(wx, wz, seed, zoom, false)
}

fn sample_tectonics_with(wx: f32, wz: f32, seed: u32, zoom: f32, landward: bool) -> TectonicSample {
    let key = (wx.to_bits(), wz.to_bits(), seed, zoom.to_bits(), landward);
    if let Some((last, value)) = TECTONIC_LAST.get()
        && last == key {
            return value;
        }
    let value = match TECTONIC_CACHE.with(|cache| cache.borrow().get(&key).copied()) {
        Some(hit) => hit,
        None => {
            let value = sample_tectonics_uncached(wx, wz, seed, zoom, landward);
            TECTONIC_CACHE.with(|cache| {
                let mut cache = cache.borrow_mut();
                if cache.len() >= 32_768 {
                    cache.clear();
                }
                cache.insert(key, value);
            });
            value
        }
    };
    TECTONIC_LAST.set(Some((key, value)));
    value
}

fn sample_tectonics_uncached(wx: f32, wz: f32, seed: u32, zoom: f32, landward: bool) -> TectonicSample {
    let (wx, wz) = (wx / zoom, wz / zoom);
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
        let mut buoyancy = (macro_craton * 0.62 + plate_h * 0.38 - if landward { 0.32 } else { 0.44 }) * 2.2;
        if landward {
            if (ix.abs() + iz.abs()) <= 1 {
                buoyancy = buoyancy.max(0.42);
            }
        } else if ix == 0 && iz == 0 {
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
    let arc_volcanism = convergence
        * has_oceanic
        * (0.35 + 0.65 * on_overriding_plate)
        * arc_profile
        * (0.45 + 0.55 * cone_chain);
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
    // Zoomed (generator 15) worlds get broader, along-strike irregular fault blocks,
    // so a plate suture reads as a ragged scarp and not a ruled dashed line.
    let modern = zoom > 1.0;
    let fault_step = if modern {
        smoothstep(0.30, 0.70, fault_cell) - 0.5
    } else {
        smoothstep(0.42, 0.58, fault_cell) - 0.5
    };
    let fault_plane = (1.0 - ((fault_cell - 0.5).abs() * 14.0).min(1.0))
        * (boundary_proximity * 0.75 + transform_shear * 0.55).min(1.0);
    let fault_activity =
        (boundary_proximity * (convergence + divergence * 0.85) + transform_shear * 0.9).min(1.2);
    let along_strike = if modern {
        0.30 + 1.15
            * value_noise2(
                strike_coord * 0.7,
                dip_coord * 0.12,
                seed.wrapping_add(4281),
            )
    } else {
        1.0
    };
    let fault_throw = fault_step * fault_activity * 9.5 * along_strike
        + (strike_coord.sin() * transform_shear * 3.2);

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

/// Rotated, domain-warped Musgrave Ridged Multifractal for authentic alpine massifs.
/// Returns `(primary_spine, lateral_spur_chutes)`:
/// - `primary_spine`: sharp pyramidal horns and knife-edge arÃªtes where each finer octave
///   is weighted by the previous octave's crest, keeping valley floors broad and passable.
/// - `lateral_spur_chutes`: branching dendritic spurs separated by incised couloir chutes.
fn mountain_ridged_multifractal(
    x: f32,
    z: f32,
    seed: u32,
    tect: &TectonicSample,
) -> (f32, f32, f32) {
    // Domain warp aligned with mantle & tectonic strike so ridge lines curve naturally.
    let wx = x + (value_noise2(x / 210.0, z / 210.0, seed.wrapping_add(4301)) - 0.5) * 82.0;
    let wz = z + (value_noise2(x / 210.0, z / 210.0, seed.wrapping_add(4309)) - 0.5) * 82.0;

    // 4-octave rotated Musgrave ridged multifractal:
    let mut px = wx / 165.0;
    let mut pz = wz / 165.0;
    let mut weight = 1.0f32;
    let mut spine = 0.0f32;
    let mut amp = 0.52f32;
    let mut norm = 0.0f32;
    for oct in 0..4u32 {
        let n = value_noise2(px, pz, seed.wrapping_add(137 + oct * 31));
        // Value noise centers around 0.5 with effective span ~[0.14, 0.86]; normalize before folding
        // so ridge crests reach true 1.0 knife edges instead of flat-topped plateaus.
        let centered = ((n - 0.5) * 2.55).clamp(-1.0, 1.0);
        let ridge = 1.0 - centered.abs();
        let sharpened = ridge * ridge;
        let signal = sharpened * weight;
        spine += signal * amp;
        norm += amp;
        weight = (signal * 1.95).clamp(0.12, 1.0);
        // Rotate by 37 degrees (cos=0.8, sin=0.6) and scale by lacunarity 2.08:
        let rx = (px * 0.80 - pz * 0.60) * 2.08 + 13.7;
        let rz = (px * 0.60 + pz * 0.80) * 2.08 - 9.4;
        px = rx;
        pz = rz;
        amp *= 0.50;
    }
    let primary_spine = (spine / norm).clamp(0.0, 1.0);

    // Branching lateral spurs & incised couloirs oriented across the tectonic dip/strike:
    let strike_c = -wx * tect.boundary_normal[1] + wz * tect.boundary_normal[0];
    let dip_c = wx * tect.boundary_normal[0] + wz * tect.boundary_normal[1];
    let spur_warp = (value_noise2(wx / 58.0, wz / 58.0, seed.wrapping_add(4327)) - 0.5) * 2.6;
    let spur_phase = strike_c * 0.052 + (dip_c * 0.022).sin() * 1.4 + spur_warp;
    let spur_ridge = (1.0 - spur_phase.sin().abs()).powf(1.65);

    // Glacial cirque amphitheaters: concave bowls scooped out on upper flanks between spurs.
    let cirque_cell = value_noise2(wx / 76.0, wz / 76.0, seed.wrapping_add(4339));
    let cirque_bowl = smoothstep(0.58, 0.86, cirque_cell) * (1.0 - spur_ridge * 0.75);

    (primary_spine, spur_ridge, cirque_bowl)
}

// V9+ combines deterministic seeded plate tectonics (oceans, continental shelves,
// subduction trenches, volcanic island/cordilleran arcs, rift grabens, and
// fold-and-thrust mountain belts) with connected, walkable valleys.
fn compact_bedrock(x: f32, z: f32, seed: u32, ridged: bool, version: u32) -> f32 {
    let tect = sample_tectonics(x, z, seed, version);
    // Generator 15: mountain ranges are larger and rarer, so most of a continent is
    // plain, plateau and foothill and the relief is concentrated in real belts.
    let (range_zoom, mask_lo, mask_hi) = if version >= 15 {
        (2.2, if ridged { 0.38 } else { 0.48 }, 0.80)
    } else {
        (1.0, if ridged { 0.24 } else { 0.32 }, 0.70)
    };
    let ranges = value_noise2(
        x / (195.0 * range_zoom),
        z / (195.0 * range_zoom),
        seed.wrapping_add(97),
    );
    let massif_cluster = value_noise2(
        x / (360.0 * range_zoom),
        z / (360.0 * range_zoom),
        seed.wrapping_add(4349),
    );
    let range_envelope = (ranges * 0.68 + massif_cluster * 0.32).clamp(0.0, 1.0);
    let mountain_mask = smoothstep(mask_lo, mask_hi, range_envelope);
    let massif_scale = if version >= 15 { 1.8 } else { 1.0 };
    let (primary_spine, spur_ridge, cirque_bowl) =
        mountain_ridged_multifractal(x / massif_scale, z / massif_scale, seed, &tect);
    let hills = (fbm2(x / 65.0, z / 65.0, seed.wrapping_add(71)) - 0.5) * 16.0;

    // Hypsometric curve from abyssal oceanic crust (-18..+4m) across the continental
    // shelf (+8..+14m) to continental interiors (+18..+36m).
    let shelf_t = smoothstep(-0.48, 0.22, tect.continentalness);
    let abyssal_t = if version >= 37 {
        1.0 - smoothstep(-0.78, -0.12, tect.continentalness)
    } else {
        smoothstep(-0.12, -0.78, tect.continentalness)
    };
    let craton_t = smoothstep(0.05, 0.65, tect.continentalness);

    let ocean_floor = -4.5 - abyssal_t * 14.5 + hills * 0.28;
    let continental_base = 17.5 + craton_t * 11.5 + hills * (0.65 + 0.35 * craton_t);
    let mut h = ocean_floor * (1.0 - shelf_t) + continental_base * shelf_t;

    // 1. Orogenic fold-and-thrust mountain belts, pyramidal peaks, lateral spurs & cirques:
    // Asymmetric cuesta/thrust profile: steeper scarp toward the plate suture, longer dip slope behind.
    let fold_asymmetry =
        0.52 + 0.48 * (tect.fold_wave + 0.28 * (tect.fold_wave * 2.0).sin()).clamp(-1.0, 1.0);
    let arÃªte_profile = primary_spine.powf(1.28) * (0.68 + 0.32 * spur_ridge);
    let pyramidal_horn = smoothstep(0.55, 0.92, primary_spine) * smoothstep(0.45, 0.88, spur_ridge);
    let massif_drive = (tect.orogeny * 1.25 + mountain_mask * craton_t * 0.88).clamp(0.0, 1.65);

    // Piedmont foothill apron rises smoothly into steep alpine massifs and horns:
    let piedmont_apron = massif_drive * (0.35 + 0.65 * fold_asymmetry) * 18.0 * shelf_t;
    // Generator 15 belts are wider and rarer, so the peaks inside them stand taller.
    let cordillera_scale = match (ridged, version >= 15) {
        (true, true) => 185.0,
        (false, true) => 150.0,
        (true, false) => 132.0,
        (false, false) => 106.0,
    };
    let mut orogenic_uplift = piedmont_apron
        + massif_drive
            * (0.58 + 0.42 * fold_asymmetry)
            * (arÃªte_profile * cordillera_scale
                + pyramidal_horn * if version >= 15 { 48.0 } else { 34.0 });

    // Glacial cirque & U-shaped valley scouring: scoops concave amphitheaters out of
    // mid-to-upper flanks while leaving knife-edge arÃªtes and horns standing tall above them.
    let flank_zone =
        smoothstep(32.0, 78.0, orogenic_uplift) * (1.0 - smoothstep(0.76, 0.95, primary_spine));
    let glacial_scour = cirque_bowl * flank_zone * 22.0;
    orogenic_uplift = (orogenic_uplift - glacial_scour).max(0.0);
    h += orogenic_uplift;

    // 2. Volcanic arcs & stratovolcano island chains (can breach sea level from oceanic plates!):
    let volcano_cone =
        primary_spine.powf(1.45) * (0.65 + 0.35 * spur_ridge) * (0.55 + 0.45 * mountain_mask);
    let volcanic_uplift = tect.volcanism * (28.0 + volcano_cone * 82.0);
    // High volcanic stratocones feature a sunken summit caldera crater rimmed by basalt cliffs.
    let caldera = smoothstep(0.74, 0.92, tect.volcanism * primary_spine) * 19.0;
    h += volcanic_uplift - caldera;

    // 3. Oceanic subduction trench depression:
    h -= tect.trench * 18.5;

    // 4. Divergent continental rift graben & mid-ocean spreading ridge:
    let horst_shoulders =
        (-((tect.boundary_dist - 64.0) / 34.0).powi(2)).exp() * (-tect.boundary_stress).max(0.0);
    if version >= 37 {
        // A coast must not change elevation abruptly at a plate threshold.
        let land = smoothstep(-0.15, 0.05, tect.continentalness);
        h += horst_shoulders * (14.0 + 1.5 * land)
            - tect.rift_graben * (6.0 + 13.5 * land);
    } else if tect.continentalness > -0.05 {
        // Sunken rift valley (graben) often deep enough to host rift lakes or sea inlets,
        // bordered by uplifted horst fault-block escarpments.
        h -= tect.rift_graben * 19.5;
        h += horst_shoulders * 15.5;
    } else {
        // Mid-ocean spreading ridge rising from the abyssal floor with a central axial cleft.
        h += horst_shoulders * 14.0 - tect.rift_graben * 6.0;
    }

    // 5. Fault-block scarps & strike-slip offset relief:
    h += tect.fault_throw * (0.45 + 0.55 * shelf_t);
    h
}

fn base_height(wx: f32, wz: f32, seed: u32, kind: u8, version: u32, footprint: f32) -> f32 {
    let (sx, sz) = if version >= 3 {
        (
            wx + (value_noise2(wx / 230.0, wz / 230.0, seed.wrapping_add(9001)) - 0.5) * 140.0,
            wz + (value_noise2(wx / 270.0, wz / 270.0, seed.wrapping_add(9003)) - 0.5) * 140.0,
        )
    } else {
        (wx, wz)
    };
    let mut h = if version >= 9 {
        compact_bedrock(wx, wz, seed, kind == WorldType::Ridged as u8, version)
    } else {
        bedrock(sx, sz, seed, kind == WorldType::Ridged as u8)
    };
    if version >= 3 {
        let mountain = smoothstep(42.0, 108.0, h);
        let fold = (sx * 0.018 + (sz * 0.006).sin() * 2.0).sin().abs();
        h += mountain * fold.powf(1.65) * if version >= 9 { 16.0 } else { 28.0 };
        let (heat, wet) = climate(wx, wz, seed, version);
        let mesa = smoothstep(0.52, 0.75, heat) * (1.0 - smoothstep(0.32, 0.5, wet));
        let terrace = (h / 9.0).floor() * 9.0 + smoothstep(0.25, 0.85, (h / 9.0).fract()) * 9.0;
        h += (terrace - h) * mesa * 0.85;
    }
    if version >= 15 {
        h += super::terrain_variety::relief(wx, wz, seed, h, footprint, version);
    }
    h
}

// Visual far-field samples do not need a 48-step erosion simulation per region.
// Editable chunks and the inner backdrop rings retain the exact generator.
pub fn preview_surface(cfg: &WorldSampleConfig, x: i32, z: i32) -> (f32, BlockId) {
    preview_surface_lod(cfg, x, z, 8.0)
}

/// Unquantized far-field surface height in metres (top of the surface voxel),
/// from the same filtered evaluator as `preview_surface_lod`. The whole-voxel
/// column height steps by one voxel, which turns gentle slopes into bands when
/// a normal is taken over a few tens of metres; this keeps the slope continuous.
pub fn preview_height_lod(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> f32 {
    if cfg.world_type == WorldType::Superflat || cfg.generator_version < 2 {
        return super::distant_surface(cfg, x, z).0 * cfg.voxel_meters();
    }
    preview_landscape_lod(cfg, x, z, footprint).height + cfg.voxel_meters()
}

/// Visual far-field sample. `footprint` is the horizon ring spacing in metres: terrain
/// detail finer than that is faded out instead of aliasing into spikes.
pub fn preview_surface_lod(
    cfg: &WorldSampleConfig,
    x: i32,
    z: i32,
    footprint: f32,
) -> (f32, BlockId) {
    if cfg.world_type == WorldType::Superflat || cfg.generator_version < 2 {
        return super::distant_surface(cfg, x, z);
    }
    let column = preview_column_lod(cfg, x, z, footprint);
    (column.height as f32 + 1.0, column.surface)
}

/// Canonical horizon policy beyond the exact 64 m rings. Keep the existing
/// fixed 8 m macro evaluator (stable shared knots/cache policy), but do not
/// sample metre-scale residuals into hundred-metre triangles. All canonical
/// far rings use this SAME policy, including refinement scouts.
pub fn preview_surface_canonical(cfg: &WorldSampleConfig, x: i32, z: i32) -> (f32, BlockId) {
    if cfg.generator_version < 38 || cfg.world_type == WorldType::Superflat {
        return preview_surface_lod(cfg, x, z, 8.0);
    }
    let s = preview_landscape_with_footprints(cfg, x, z, 8.0, 64.0);
    let column = column_from_landscape(cfg, x, z, s);
    (column.height as f32 + 1.0, column.surface)
}

pub(super) fn preview_column_lod(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> ColumnSample {
    if cfg.world_type == WorldType::Superflat || cfg.generator_version < 2 {
        return super::sample_column(cfg, x, z);
    }
    column_from_landscape(cfg, x, z, preview_landscape_lod(cfg, x, z, footprint))
}

pub(super) fn preview_landscape_lod(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> Landscape {
    preview_landscape_with_footprints(cfg, x, z, footprint, footprint)
}

fn preview_landscape_with_footprints(
    cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32, detail_footprint: f32,
) -> Landscape {
    let wx = cfg.world_x(x);
    let wz = cfg.world_z(z);
    let (h, slope, temperature, moisture) = preview_shaped(cfg, wx, wz, footprint, detail_footprint);
    classify_landscape(cfg, wx, wz, h, slope, temperature, moisture, 0.25)
}

/// Far-field ground top before drainage carving. It builds no erosion tiles,
/// so drainage planning can sample far from the chunk being generated.
pub(super) fn pre_drainage_preview_top(cfg: &WorldSampleConfig, wx: f32, wz: f32) -> f32 {
    preview_shaped(cfg, wx, wz, 8.0, 8.0).0 + cfg.voxel_meters()
}

fn preview_shaped(
    cfg: &WorldSampleConfig, wx: f32, wz: f32, footprint: f32, detail_footprint: f32,
) -> (f32, f32, f32, f32) {
    if super::geo::active(cfg) {
        let g = super::geo::ground(cfg, wx, wz, footprint.max(detail_footprint));
        return (g.top_m - cfg.voxel_meters(), g.slope, g.temperature, g.moisture);
    }
    let probe = footprint.clamp(8.0, 96.0);
    let height_at = |px: f32, pz: f32| {
        base_height(
            px,
            pz,
            cfg.seed,
            cfg.world_type as u8,
            cfg.generator_version,
            footprint,
        )
    };
    let h0 = height_at(wx, wz);
    let hx = height_at(wx + probe, wz);
    let hz = height_at(wx, wz + probe);
    let slope0 = (((hx - h0) / probe).powi(2) + ((hz - h0) / probe).powi(2)).sqrt();
    apply_macro_shaping(cfg, wx, wz, h0, slope0, 0.25, detail_footprint)
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
            let h = base_height(wx, wz, seed, kind, version, SPACING);
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
                // Thermal creep & alpine frost-wedging move unstable scree down steep headwalls,
                // building natural talus aprons in couloirs and at the base of cliffs.
                if step % 3 == 0 {
                    let j = *neighbors
                        .iter()
                        .min_by(|&&a, &&b| height[a].total_cmp(&height[b]))
                        .unwrap();
                    let frost_boost = if height[i] > 68.0 { 1.35 } else { 1.0 };
                    let angle_of_repose = if height[i] > 82.0 {
                        SPACING * 0.62
                    } else {
                        SPACING * 0.70
                    };
                    let talus = ((height[i] - height[j] - angle_of_repose) * 0.085 * frost_boost)
                        .clamp(0.0, 0.30);
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
            if cache.entries.len() >= MAX_TILES
                && let Some(i) = cache
                    .recent
                    .iter()
                    .position(|k| cache.entries.get(k).is_some_and(|c| c.get().is_some()))
                {
                    let old = cache.recent.remove(i).unwrap();
                    cache.entries.remove(&old);
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
        if hot.len() >= 64 {
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
    if super::geo::active(cfg) {
        let g = super::geo::ground(cfg, wx, wz, cfg.voxel_meters());
        return classify_landscape(cfg, wx, wz, g.top_m - cfg.voxel_meters(), g.slope, g.temperature, g.moisture, g.sediment);
    }
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
    let (h, slope, temperature, moisture) = apply_macro_shaping(cfg, wx, wz, h, slope, sediment, cfg.voxel_meters());
    classify_landscape(cfg, wx, wz, h, slope, temperature, moisture, sediment)
}

fn classify_landscape(
    cfg: &WorldSampleConfig, wx: f32, wz: f32, h: f32, slope: f32,
    temperature: f32, moisture: f32, sediment: f32,
) -> Landscape {
    // The planet solve carves its own rivers and lakes into `h`.
    let geo = super::geo::active(cfg);
    let drainage = if geo { Default::default() } else { super::drainage::sample(cfg, wx, wz) };
    let h = if geo {
        h
    } else if cfg.generator_version >= super::drainage::RIVER_FIT_GENERATOR {
        // Fitted channels give the ground top directly: a carve ceiling and a
        // bank floor that keeps water from standing above dry ground.
        // Lakes join the same way; ceilings combine by minimum, floors by maximum.
        // A lake's banks override the dry swales it absorbs (wet rivers never
        // reach a lake), so only its own ceiling caps its floor.
        let raw = h + cfg.voxel_meters();
        let lake = super::lakes::sample(cfg, wx, wz);
        let ceiling = drainage.bed_m.min(lake.ceiling_m);
        let river_fill = drainage.levee_m.map_or(f32::MIN, |l| l.min(raw + super::drainage::LEVEE_CAP_M)).min(ceiling);
        let lake_fill = lake.floor_m.map_or(f32::MIN, |l| l.min(raw + lake.floor_cap_m)).min(lake.ceiling_m);
        raw.min(ceiling).max(river_fill.max(lake_fill)) - cfg.voxel_meters()
    } else {
        h + (drainage.bed_m.min(h) - h) * drainage.bank
    };
    let moisture = (moisture + drainage.bank * 0.24).min(1.0);
    let sediment = (sediment + drainage.bank * 0.35).min(1.0);
    let alpine = if cfg.generator_version >= 9 {
        h > 124.0 || (h > 98.0 && temperature < 0.24) || (h > 84.0 && slope > 0.78)
    } else if cfg.generator_version >= 7 {
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
    let v15 = cfg.generator_version >= 15;
    let coastal = if v15 {
        shoreline_biome(cfg, wx, wz, h, slope, temperature, moisture)
    } else if h < 14.8 && slope < 0.42 {
        Some(Biome::Coast)
    } else {
        None
    };
    let weird = if v15 && h > 20.0 && !alpine {
        super::terrain_variety::anomaly(wx, wz, cfg.seed)
    } else {
        None
    };
    let biome = if cfg.generator_version >= 3 && wx.hypot(wz) < 110.0 {
        if wx > 30.0 {
            Biome::Forest
        } else {
            Biome::Meadow
        }
    } else if h < 10.5 {
        Biome::Ocean
    } else if let Some(shore_biome) = coastal {
        shore_biome
    } else if weird.is_some_and(|a| {
        a.strength * (a.weights[1] + a.weights[3])
            > 0.25 + super::terrain_variety::ragged_edge(wx, wz, cfg.seed) * 0.4
    }) {
        // Mesas and canyons read as red-rock country whatever the climate says.
        Biome::Badlands
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
    } else if cfg.generator_version >= 10
        && moisture > 0.78
        && (16.5..24.0).contains(&h)
        && slope < 0.14
    {
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

/// Generator 15 shoreline biome. Beach width follows the local coast character,
/// rocky coasts get no beach biome at all, and wet flats become marsh.
fn shoreline_biome(
    cfg: &WorldSampleConfig,
    wx: f32,
    wz: f32,
    h: f32,
    slope: f32,
    temperature: f32,
    moisture: f32,
) -> Option<Biome> {
    use super::terrain_variety::{CoastKind, shore};
    let d = h - super::WATER_LEVEL_METERS;
    if !(-1.5..6.0).contains(&d) || slope > 0.5 {
        return None;
    }
    let s = shore(wx, wz, cfg.seed, temperature, moisture);
    if s.kind == CoastKind::Rocky {
        return None;
    }
    if d < s.limit.max(0.5) {
        return Some(if s.kind == CoastKind::Mudflat {
            Biome::Marsh
        } else {
            Biome::Coast
        });
    }
    // Low, wet, flat ground just behind the beach.
    if d < 4.5 && slope < 0.14 && moisture > 0.58 {
        let ragged = value_noise2(wx / 38.0, wz / 38.0, cfg.seed.wrapping_add(9531));
        if ragged > 0.42 {
            return Some(Biome::Marsh);
        }
    }
    None
}

fn apply_macro_shaping(
    cfg: &WorldSampleConfig,
    wx: f32,
    wz: f32,
    mut h: f32,
    mut slope: f32,
    sediment: f32,
    footprint: f32,
) -> (f32, f32, f32, f32) {
    let (mut temperature, mut rain) = climate(wx, wz, cfg.seed, cfg.generator_version);
    if cfg.generator_version >= 5 {
        // Preserve coherent weather regions while making the dry/cold extremes
        // reachable; interpolated climate noise otherwise favours green forest.
        temperature = ((temperature - 0.5) * 1.55 + 0.5).clamp(0.0, 1.0);
        rain = ((rain - 0.5) * 1.4 + 0.5).clamp(0.0, 1.0);
    }
    if cfg.generator_version >= 15 {
        let soft = cfg.generator_version >= CLIMATE_SCALE_GENERATOR;
        (temperature, rain) = super::terrain_variety::climate(wx, wz, cfg.seed, temperature, rain, soft);
    }
    if cfg.generator_version >= 33 {
        (temperature, rain) =
            super::terrain_variety::regional_climate(wx, wz, cfg.seed, temperature, rain);
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
    if cfg.generator_version < 38 {
        h += (value_noise2(wx / 6.0, wz / 6.0, cfg.seed.wrapping_add(83)) - 0.5)
        * (0.65 + slope.min(1.0) * 1.3)
        * shore_calm;
    }
    if (3..37).contains(&cfg.generator_version) {
        // A real inland watershed, not a platform placed after world generation.
        // The central clearing is dry, gently graded and joins 320 m of dry land.
        let distance = wx.hypot(wz);
        let v7 = cfg.generator_version >= 7;
        // Generator 30 stretches or squeezes the falloff beyond the 110 m core by
        // a slow noise field, so the home landmass gets bays and headlands instead
        // of a circular rim of sand. Inside 110 m nothing changes.
        let falloff_distance = if cfg.generator_version >= 30 && distance > 110.0 {
            let lobe = fbm2(wx / 190.0, wz / 190.0, cfg.seed.wrapping_add(9061));
            let reach = (1.0 + (lobe - 0.5) * 2.0 * 1.1).clamp(0.45, 2.0);
            110.0 + (distance - 110.0) / reach
        } else {
            distance
        };
        let inland = 1.0
            - smoothstep(
                if v7 { 110.0 } else { 160.0 },
                if v7 { 380.0 } else { 520.0 },
                falloff_distance,
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
    if (5..38).contains(&cfg.generator_version) {
        // Historical residual landforms below the erosion grid's 8 m sampling interval.
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
        // Rotated secondary arÃªte rib & incised couloir chute detail for high mountains:
        let rx = wx * 0.80 - wz * 0.60;
        let rz = wx * 0.60 + wz * 0.80;
        let a_rot = value_noise2(rx / 18.0, rz / 18.0, cfg.seed.wrapping_add(9233));
        let chute_furrow = (1.0 - ((a_rot - 0.5) * 2.4).clamp(-1.0, 1.0).abs()).powi(2);
        let alpine_relief = smoothstep(52.0, 118.0, h);
        // Structural rock benches along tilted mountain strata:
        let bench_phase = ((h + wx * 0.04 - wz * 0.03) / 10.5).fract();
        let couloir_scale = if cfg.generator_version >= 17 {
            6.6
        } else {
            4.2
        };
        let bench_scale = if cfg.generator_version >= 17 {
            4.4
        } else {
            2.6
        };
        let rock_bench =
            (smoothstep(0.18, 0.82, bench_phase) - bench_phase) * bench_scale * alpine_relief;
        let couloir_cut =
            (1.0 - chute_furrow) * couloir_scale * alpine_relief * smoothstep(0.25, 0.78, slope);

        let broken = (rib - 0.35) * 9.0 + (b_eff - 0.5) * 4.0 + (c_eff - 0.5) * 1.3 + rock_bench
            - couloir_cut;
        let hummock = (a - 0.5) * 3.0 + (b_eff - 0.5) * 1.5;
        let mut dunes = (wx * 0.085 + wz * 0.032 + a * 4.0).sin() * 2.2;
        if cfg.generator_version >= 15 {
            // Long swept megadunes carry the small ripples, like a real sand sea.
            let phase = (wx * 0.021 - wz * 0.012 + a * 3.0) / std::f32::consts::TAU;
            let saw = phase - phase.floor();
            dunes += (saw.powf(1.7) - 0.4) * 9.0;
        }
        let arid = smoothstep(0.55, 0.8, temperature) * (1.0 - smoothstep(0.2, 0.4, moisture));
        let residual = (hummock * (1.0 - arid) + dunes * arid) * (1.0 - exposed) + broken * exposed;
        let strength = smoothstep(20.0, 65.0, wx.hypot(wz)) * smoothstep(12.2, 19.0, h);
        let mountain_scale = if cfg.generator_version >= 9 {
            0.48 + 0.38 * alpine_relief
        } else {
            1.0
        };
        h += residual * strength * mountain_scale;
        slope +=
            ((b_eff - 0.5).abs() * 0.25 + chute_furrow * alpine_relief * 0.14) * exposed * strength;
    }
    if cfg.generator_version >= 23 {
        // Generator 23 blends the far-field shore treatment in across 100..120 m
        // instead of switching it on at 110 m, which left a step in the ground.
        let gate = smoothstep(100.0, 120.0, wx.hypot(wz));
        if cfg.generator_version >= 15 && gate > 0.0 {
            let h0 = h;
            let mut h1 = h;
            if let Some(a) = super::terrain_variety::anomaly(wx, wz, cfg.seed) {
                let (dh, ds) = super::terrain_variety::anomaly_shape(&a, wx, wz, cfg.seed, h, cfg.generator_version);
                h1 += dh;
                slope += ds * gate;
            }
            let shore = super::terrain_variety::shore(wx, wz, cfg.seed, temperature, moisture);
            h1 = super::terrain_variety::reprofile(h1, shore, cfg.generator_version);
            h1 = super::terrain_variety::strand(h1, shore, wx, wz, cfg.seed, cfg.generator_version);
            h = h0 + (h1 - h0) * gate;
        }
    } else if cfg.generator_version >= 15 && wx.hypot(wz) > 110.0 {
        let tv = super::terrain_variety::anomaly(wx, wz, cfg.seed);
        if let Some(a) = tv {
            let (dh, ds) = super::terrain_variety::anomaly_shape(&a, wx, wz, cfg.seed, h, cfg.generator_version);
            h += dh;
            slope += ds;
        }
        // Continuous beach profile. Heights keep their shoreline position but the
        // beach face gets flatter and wider, with no step for the voxel grid to expose.
        let shore = super::terrain_variety::shore(wx, wz, cfg.seed, temperature, moisture);
        h = super::terrain_variety::reprofile(h, shore, cfg.generator_version);
        if cfg.generator_version >= 17 {
            h = super::terrain_variety::strand(h, shore, wx, wz, cfg.seed, cfg.generator_version);
        }
    } else if cfg.generator_version < 15 && channel < 0.48 && h > 11.15 && h < 12.25 {
        // Prevent shoreline heights from lingering in the 11.2..12.15m rounding band
        // where 1m voxel quantization creates isolated 1-block water lines on beaches.
        // (Generator 14 and older only: the snap is a 1.2 m step.)
        h = if h >= 11.42 {
            12.28 + (h - 11.42) * 0.65
        } else {
            11.05 - (11.42 - h) * 0.40
        };
    }
    if cfg.generator_version >= 37 {
        let distance = wx.hypot(wz);
        // A broad, irregular foothill basin. The old 28..55 m blend could
        // join a 26 m clearing directly to a 150 m mountain wall.
        let warp = (value_noise2(wx / 240.0, wz / 240.0, cfg.seed.wrapping_add(9061)) - 0.5) * 0.3;
        let reach = distance * (1.0 + warp);
        let meadow = 26.0 + (value_noise2(wx / 100.0, wz / 100.0,
            cfg.seed.wrapping_add(9011)) - 0.5) * 4.0;
        let blend = smoothstep(115.0, 580.0, reach);
        h = meadow + (h - meadow) * blend;
        slope = slope * blend + 0.04 * (1.0 - blend);
        let spawn = 1.0 - smoothstep(8.0, 26.0, distance);
        h += (26.0 - h) * spawn;
    }
    if cfg.generator_version >= 39 && cfg.world_type != WorldType::Superflat {
        let (relief, landform_slope) = super::landforms::relief(cfg, wx, wz, h);
        h += relief;
        slope = slope.hypot(landform_slope);
    }
    if cfg.generator_version >= 38 && cfg.world_type != WorldType::Superflat {
        let (relief, detail_slope) = super::terrain_detail::sample(
            cfg, wx, wz, h, slope, temperature, moisture, sediment, footprint,
        );
        h += relief;
        slope = slope.hypot(detail_slope);
    }
    if cfg.generator_version >= 49 && cfg.world_type != WorldType::Superflat {
        // Evaluate against the final natural coast, after the spawn basin and
        // landforms. Earlier shoreline relief was erased by those later stages.
        // Authored river/village grading below still has final authority.
        h = super::terrain_variety::coastal_shoulders(h, wx, wz, cfg.seed);
    }
    if cfg.generator_version >= 14 && cfg.world_type != WorldType::Superflat {
        h = super::valley::carve(cfg, wx, wz, h);
    }
    if cfg.generator_version >= super::amberroot::VERSION && cfg.world_type != WorldType::Superflat {
        h = super::amberroot::carve(cfg, wx, wz, h);
    }
    (h, slope, temperature, moisture)
}

/// Generator 30: how far the true seabed lies above (+) or below (-) the
/// whole-voxel column height, in voxels. Water depth adds it back so a gentle
/// shelf grades smoothly instead of stepping one colour ring per voxel.
pub(super) fn bed_fraction(cfg: &WorldSampleConfig, x: i32, z: i32) -> f32 {
    if cfg.world_type == WorldType::Superflat || cfg.generator_version < 30 {
        return 0.0;
    }
    let voxels = sample(cfg, x, z).height * cfg.inv_voxel_meters();
    (voxels - voxels.round()).clamp(-0.5, 0.5)
}

pub(super) fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    if cfg.world_type == WorldType::Superflat {
        return ColumnSample {
            height: cfg.blocks_for_height(16.0),
            surface: BlockId::Grass,
            filler: BlockId::Dirt,
            vegetation_density: 0.0,
            cave: super::CaveColumn::default(),
        };
    }
    column_from_landscape(cfg, x, z, sample(cfg, x, z))
}

/// One material/ecology policy for resident columns and filtered horizon samples.
/// The evaluator may filter relief, but never invent a second biome palette.
fn column_from_landscape(cfg: &WorldSampleConfig, x: i32, z: i32, s: Landscape) -> ColumnSample {
    let (mut surface, mut filler, mut vegetation) = match s.biome {
        Biome::Ocean | Biome::Coast => (BlockId::Sand, BlockId::Sandstone, 0.0),
        Biome::Marsh => (BlockId::Mud, BlockId::Clay, 0.25),
        Biome::Desert if cfg.generator_version >= 15 => {
            let (top, under) = super::terrain_variety::desert_surface(
                cfg.world_x(x),
                cfg.world_z(z),
                cfg.seed,
                s.slope,
            );
            (top, under, 0.0)
        }
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
            Biome::Alpine
                | Biome::Tundra
                | Biome::Taiga
                | Biome::Forest
                | Biome::Meadow
                | Biome::Heathland
        ) {
            if cfg.generator_version >= 7 {
                let cold = 1.0 - smoothstep(0.08, 0.52, s.temperature);
                // The planet solve's temperature already includes altitude.
                let snowline = if super::geo::active(cfg) {
                    1.0 - smoothstep(0.17, 0.33, s.temperature)
                } else if cfg.generator_version >= 9 {
                    smoothstep(104.0, 158.0, s.height)
                } else {
                    smoothstep(138.0, 205.0, s.height)
                };
                let sheltered = 1.0 - smoothstep(0.18, 1.05, s.slope);
                let cold_weight = if cfg.generator_version >= 9 {
                    0.20
                } else {
                    0.50
                };
                let snow = cold * cold_weight + snowline * 0.58 + patch * 0.32 + sheltered * 0.12
                    - s.slope * 0.18;
                surface = if snow > 0.64 && s.slope < 0.74 {
                    if cfg.generator_version >= 9
                        && s.height > 134.0
                        && s.slope < 0.34
                        && s.temperature < 0.22
                        && patch > 0.60
                    {
                        BlockId::Ice
                    } else {
                        BlockId::SnowGrass
                    }
                } else if s.slope > 0.54 {
                    if s.sediment > 0.58 && s.slope < 0.74 {
                        BlockId::Gravel
                    } else {
                        BlockId::Stone
                    }
                } else if s.height > 72.0 && s.sediment > 0.62 && patch > 0.52 {
                    // Alpine talus scree aprons at the foot of cirque headwalls & couloirs.
                    BlockId::Gravel
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
            if cfg.generator_version >= 15 {
                // Generator 15 places beaches in the shore pass below.
            } else if s.height < 16.0 && s.slope < 0.36 {
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
            } else if s.moisture > 0.84
                && s.height >= 17.5
                && s.height < 23.0
                && s.slope < 0.14
                && patch > 0.68
            {
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
                    if patch < 0.25 && s.slope < 0.35 {
                        surface = BlockId::Dirt;
                    }
                }
                Biome::Rainforest => {
                    vegetation *= 0.65 + grove * 0.45;
                    if patch > 0.72 && s.slope < 0.25 {
                        surface = BlockId::Mud;
                    }
                }
                Biome::Heathland => {
                    if patch > 0.66 && s.slope > 0.22 {
                        surface = BlockId::Gravel;
                    }
                }
                Biome::Fen => {
                    // Grass islands support trees; soft clay/mud fills hollows.
                    if patch < 0.54 {
                        surface = BlockId::Mud;
                        vegetation = 0.0;
                    } else {
                        vegetation *= smoothstep(0.54, 0.82, patch);
                    }
                }
                _ => {}
            }
        }
        if cfg.generator_version >= 9 && wx.hypot(wz) > 110.0 {
            let tect =
                sample_tectonics(wx, wz, cfg.seed, cfg.generator_version);
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
                // Active volcanic arc cones & basaltic rift shields (preserving snowcaps on high flat rims).
                if !matches!(surface, BlockId::SnowGrass | BlockId::Ice) || s.slope > 0.48 {
                    surface = if patch > 0.74 {
                        BlockId::Gravel
                    } else {
                        BlockId::Basalt
                    };
                }
                filler = BlockId::Basalt;
                vegetation *= 0.12;
            } else if (s.slope > 0.68 || (tect.fault_plane > 0.62 && s.slope > 0.40))
                && s.height > 15.0
            {
                // Steep cliff faces & fault scarps expose the tilted 3D stratigraphic bed.
                let bed = sample_stratigraphic_bed(
                    wx,
                    s.height,
                    wz,
                    cfg.seed,
                    &tect,
                    cfg.generator_version,
                );
                surface = if tect.fault_plane > 0.72 && patch > 0.55 {
                    BlockId::Gravel
                } else {
                    bed
                };
                filler = bed;
            }
            // Subalpine timberline tapers tree density between 86m and 122m elevation.
            let timberline = 1.0 - smoothstep(86.0, 122.0, s.height);
            vegetation *= timberline;
            if matches!(
                surface,
                BlockId::Stone
                    | BlockId::Basalt
                    | BlockId::Limestone
                    | BlockId::Sandstone
                    | BlockId::Ice
            ) {
                vegetation = 0.0;
            } else if surface == BlockId::Gravel {
                vegetation *= 0.15;
            }
        }
        if cfg.generator_version >= 15 && wx.hypot(wz) > 110.0 {
            use super::terrain_variety as tv;
            if let Some(a) = tv::anomaly(wx, wz, cfg.seed).filter(|_| s.height > 20.0)
                && let Some((top, under)) =
                    tv::anomaly_surface(&a, wx, wz, cfg.seed, s.height, s.slope)
                {
                    if !matches!(surface, BlockId::SnowGrass | BlockId::Ice) {
                        surface = top;
                        filler = under;
                    }
                    vegetation *= match a.kind {
                        tv::Landform::Karst => 0.5,
                        tv::Landform::Crater => 0.3,
                        tv::Landform::Mesa | tv::Landform::Canyon => 0.0,
                    };
                }
            let shore = tv::shore(wx, wz, cfg.seed, s.temperature, s.moisture);
            if let Some((top, under)) = tv::shore_surface(
                wx,
                wz,
                cfg.seed,
                s.height,
                s.slope,
                shore,
                cfg.generator_version,
            ) {
                surface = top;
                filler = under;
                vegetation = 0.0;
            }
        }
        // Exposed rock has rock beneath it. A thin stone cap over dirt made
        // steep slopes a distracting brown/grey checkerboard at every step.
        if matches!(surface, BlockId::Stone | BlockId::Gravel) {
            filler = BlockId::Stone;
        } else if matches!(
            surface,
            BlockId::Basalt | BlockId::Limestone | BlockId::Sandstone
        ) {
            filler = surface;
        } else if surface == BlockId::Ice {
            filler = BlockId::Ice;
        }
    }
    if cfg.generator_version >= 14 {
        let wx = cfg.world_x(x);
        let wz = cfg.world_z(z);
        if super::valley::in_channel(cfg, wx, wz) {
            surface = BlockId::Sand;
            filler = BlockId::Sand;
            vegetation = 0.0;
        } else if super::valley::built_up(cfg, wx, wz) {
            vegetation = 0.0;
        }
    }
    if cfg.generator_version >= super::amberroot::VERSION
        && let Some((s, f, v)) = super::amberroot::column(
            cfg,
            cfg.world_x(x),
            cfg.world_z(z),
            (surface, filler),
            vegetation,
        )
    {
        surface = s;
        filler = f;
        vegetation = v;
    }
    if cfg.generator_version >= 37 {
        // Leave a usable first factory meadow beside the route junction.
        let yard = ((cfg.world_x(x) + 38.0) / 24.0).hypot((cfg.world_z(z) - 18.0) / 19.0);
        vegetation *= smoothstep(1.0, 1.7, yard);
    }
    if cfg.generator_version >= engine_core::environment::ENVIRONMENT_GENERATOR {
        let drainage = super::drainage::sample(cfg, cfg.world_x(x), cfg.world_z(z));
        if drainage.bank > 0.5 {
            surface = if s.slope > 0.2 { BlockId::Gravel } else { BlockId::Mud };
            filler = if s.sediment > 0.65 { BlockId::Clay } else { BlockId::Dirt };
            vegetation *= if drainage.water_m.is_some() { 0.0 } else { 0.6 };
        }
    }
    if cfg.generator_version >= super::lakes::LAKES_GENERATOR {
        let lake = super::lakes::sample(cfg, cfg.world_x(x), cfg.world_z(z));
        match lake.kind {
            Some(kind) if lake.edge_m < 3.0 => {
                (surface, filler) = super::lakes::shore_blocks(kind);
                vegetation = 0.0;
            }
            // An oasis keeps a green ring in the dry ground around it.
            Some(super::lakes::LakeKind::Oasis) if lake.edge_m < 14.0 => {
                surface = BlockId::Grass;
                vegetation = 1.2;
            }
            _ => {}
        }
    }
    if cfg.generator_version >= 47 {
        // Wider occupied stands alternate with open glades. Keep all upstream
        // water, route, yard and site exclusions: zero density stays zero.
        let wx = cfg.world_x(x);
        let wz = cfg.world_z(z);
        let stand = value_noise2(wx / 83.0, wz / 83.0, cfg.seed.wrapping_add(45131));
        let wooded = matches!(s.biome, Biome::Forest | Biome::Taiga | Biome::Rainforest);
        let envelope = smoothstep(0.28, 0.64, stand);
        vegetation *= if wooded { 0.10 + 1.48 * envelope } else { 0.10 + 0.90 * envelope };
        // Coherent exposed forest soil under closed stands, rather than a
        // uniform lawn. It is actual editable surface material, not a decal.
        if wooded && surface == BlockId::Grass && vegetation > 0.60 && stand > 0.60
            && value_noise2(wx / 23.0, wz / 23.0, cfg.seed.wrapping_add(47131)) > 0.58 {
            surface = BlockId::Dirt;
        }
    } else if cfg.generator_version >= 45 {
        // One stand envelope for resident and horizon columns. Existing site,
        // rock and water exclusions remain multiplicative, never overridden.
        let stand = value_noise2(cfg.world_x(x) / 83.0, cfg.world_z(z) / 83.0,
            cfg.seed.wrapping_add(45131));
        vegetation *= 0.18 + 0.82 * smoothstep(0.25, 0.62, stand);
    }
    let height = cfg.clamp_height(cfg.blocks_for_height(s.height));
    ColumnSample {
        height,
        surface,
        filler,
        vegetation_density: vegetation,
        cave: super::cave_column(cfg, x, z, height),
    }
}

/// Generator 15 bedrock. A mountain is mostly one rock; thick, wavy beds of a
/// second rock show only now and then, and basalt appears only in volcanic
/// provinces. The old 9.5-11 m cycle made every cliff a stack of zebra stripes.
/// The parts of `bedrock_v15` that depend on the column only. Voxel loops walk
/// y fastest, so one remembered column serves nearly every query and saves
/// about ten noise evaluations per solid voxel.
#[derive(Clone, Copy)]
struct BedrockColumn {
    warp: f32,
    heat: f32,
    region: f32,
    period: f32,
}

thread_local! {
    static BEDROCK_LAST: std::cell::Cell<Option<((u32, u32, u32, bool), BedrockColumn)>>
        = const { std::cell::Cell::new(None) };
}

fn bedrock_column(wx: f32, wz: f32, seed: u32, version: u32) -> BedrockColumn {
    let scaled = version >= CLIMATE_SCALE_GENERATOR;
    let key = (wx.to_bits(), wz.to_bits(), seed, scaled);
    if let Some((last, column)) = BEDROCK_LAST.get()
        && last == key {
            return column;
        }
    // Low-frequency warp bends beds into long gentle waves instead of flat stripes.
    let warp = (value_noise2(wx / 170.0, wz / 170.0, seed.wrapping_add(9947)) - 0.5) * 16.0;
    let region = value_noise2(wx / 1100.0, wz / 1100.0, seed.wrapping_add(9951));
    // Warm lowland-climate regions are sedimentary sandstone country; elsewhere a
    // mountain is grey stone. The far-terrain shader tints rock by the same heat
    // field, so near and distant slopes agree.
    let (base_heat, _) = climate(wx, wz, seed, version);
    let (heat, _) = super::terrain_variety::climate(
        wx,
        wz,
        seed,
        ((base_heat - 0.5) * 1.55 + 0.5).clamp(0.0, 1.0),
        0.5,
        scaled,
    );
    let period = 34.0 + 22.0 * value_noise2(wx / 1600.0, wz / 1600.0, seed.wrapping_add(9955));
    let column = BedrockColumn {
        warp,
        heat,
        region,
        period,
    };
    BEDROCK_LAST.set(Some((key, column)));
    column
}

fn bedrock_v15(wx: f32, wy: f32, wz: f32, seed: u32, tect: &TectonicSample, version: u32) -> BlockId {
    let dip_coord = wx * tect.boundary_normal[0] + wz * tect.boundary_normal[1];
    let tilt = (dip_coord * 0.012).sin() * (6.0 + tect.boundary_proximity * 10.0);
    let fold = tect.fold_wave * (4.0 + tect.orogeny * 12.0);
    let BedrockColumn {
        warp,
        heat,
        region,
        period,
    } = bedrock_column(wx, wz, seed, version);
    let strat_y = wy + tilt + fold + warp + tect.fault_throw;
    if tect.volcanism > 0.46 || tect.rift_graben > 0.5 {
        let dike = (dip_coord * 0.07 + wy * 0.03 + warp * 0.1).sin().abs();
        if dike < 0.07 + tect.volcanism * 0.10 {
            return BlockId::Basalt;
        }
    }
    let dominant = if tect.volcanism > 0.40 {
        BlockId::Basalt
    } else if heat > 0.60 && region < 0.72 {
        BlockId::Sandstone
    } else if region > 0.78 && tect.continentalness < 0.45 {
        BlockId::Limestone
    } else {
        BlockId::Stone
    };
    let bed = (strat_y / period).floor();
    let sheet = value_noise2(
        bed * 3.17 + 0.5,
        (wx * 0.6 + wz * 0.8) / 650.0,
        seed.wrapping_add(9953),
    );
    if sheet > 0.80 && dominant != BlockId::Basalt {
        if dominant == BlockId::Stone {
            if region < 0.5 {
                BlockId::Sandstone
            } else {
                BlockId::Limestone
            }
        } else {
            BlockId::Stone
        }
    } else {
        dominant
    }
}

fn sample_stratigraphic_bed(
    wx: f32,
    wy: f32,
    wz: f32,
    seed: u32,
    tect: &TectonicSample,
    version: u32,
) -> BlockId {
    if version >= 15 {
        return bedrock_v15(wx, wy, wz, seed, tect, version);
    }
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

/// Generator 15 interior rock: thin patchy ore lenses inside thick, calm bedrock.
fn stratum_v15(
    cfg: &WorldSampleConfig,
    tect: &TectonicSample,
    meters: glam::Vec3,
    below: f32,
    fleck: f32,
) -> BlockId {
    let seed = cfg.seed;
    // Flat coal and ironstone lenses, rare enough that a cliff shows a few, not a barcode.
    let lens = super::value_noise3(
        meters.x * 0.035,
        meters.y * 0.16,
        meters.z * 0.035,
        seed.wrapping_add(9961),
    );
    if lens > 0.80 && fleck > 0.30 {
        return if tect.crust_age > 0.54 || meters.y < 6.0 {
            BlockId::IronOre
        } else {
            BlockId::CoalOre
        };
    }
    let seam = super::value_noise3(
        meters.x * 0.09 + tect.fold_wave * 0.4,
        meters.y * 0.15,
        meters.z * 0.09,
        seed.wrapping_add(1201),
    );
    if seam > 0.68 - (tect.orogeny * 0.05 + tect.volcanism * 0.06) && fleck > 0.25 {
        return if meters.y > 0.0 {
            BlockId::CopperOre
        } else {
            BlockId::TinOre
        };
    }
    if below > 78.0 {
        return BlockId::Basalt;
    }
    if below > 54.0 && fleck > 0.45 {
        return BlockId::Ice;
    }
    if below > 30.0 && fleck > 0.62 {
        return BlockId::FungalSoil;
    }
    bedrock_v15(meters.x, meters.y, meters.z, seed, tect, cfg.generator_version)
}

pub(super) fn geological_stratum(
    cfg: &WorldSampleConfig,
    p: glam::IVec3,
    column_height: i32,
    depth: i32,
) -> BlockId {
    if cfg.generator_version < 34 {
        return geological_stratum_legacy(cfg, p, column_height, depth, None);
    }
    // Generator 34 swaps the scattered ores for placed ore bodies.
    let meters = p.as_vec3() * cfg.voxel_meters();
    let below = depth as f32 * cfg.voxel_meters();
    let tect = sample_tectonics(meters.x, meters.z, cfg.seed, cfg.generator_version);
    let rock = geological_stratum_legacy(cfg, p, column_height, depth, Some(tect));
    let host = if super::ore_geology::is_scattered_ore(rock) {
        bedrock_v15(meters.x, meters.y, meters.z, cfg.seed, &tect, cfg.generator_version)
    } else {
        rock
    };
    super::ore_geology::deposit(cfg.seed, &tect, meters, below, host).unwrap_or(host)
}

/// `tect` lets generator 34 share the tectonic sample it already took; `None`
/// keeps every older version sampling exactly as before.
fn geological_stratum_legacy(
    cfg: &WorldSampleConfig,
    p: glam::IVec3,
    column_height: i32,
    depth: i32,
    tect: Option<TectonicSample>,
) -> BlockId {
    let meters = p.as_vec3() * cfg.voxel_meters();
    let below = depth as f32 * cfg.voxel_meters();
    // Generator 15+ rarely reads `vein` (only in volcanic pipes and fault
    // planes), so it is evaluated on demand instead of for every solid voxel.
    let vein_cache = std::cell::OnceCell::new();
    let vein = || {
        *vein_cache.get_or_init(|| {
            super::value_noise3(
                meters.x * 0.24,
                meters.y * 0.24,
                meters.z * 0.24,
                cfg.seed.wrapping_add(883),
            )
        })
    };
    let fleck = super::hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(887));

    if cfg.generator_version >= 9 {
        let tect = tect.unwrap_or_else(|| {
            sample_tectonics(meters.x, meters.z, cfg.seed, cfg.generator_version)
        });
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
            let pipe_nx = value_noise2(
                meters.x / 34.0,
                meters.z / 34.0,
                cfg.seed.wrapping_add(4513),
            );
            let pipe_nz = value_noise2(
                meters.x / 31.0,
                meters.z / 31.0,
                cfg.seed.wrapping_add(4523),
            );
            let pipe_strength = pipe_nx * pipe_nz * (tect.volcanism + tect.rift_graben * 0.65);
            // Safely encased deep magma conduit core + surrounding basalt wall & porphyry ore halo:
            if pipe_strength > 0.46
                && below > 9.0
                && meters.y < (column_height as f32 * cfg.voxel_meters() - 8.0)
            {
                if pipe_strength > 0.56 && below > 14.0 {
                    return BlockId::Lava;
                }
                return BlockId::Basalt;
            }
            if pipe_strength > 0.34 {
                if fleck > 0.58 && vein() > 0.56 {
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
            if vein() > 0.60 && fleck > 0.35 {
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

        if cfg.generator_version >= 15 {
            return stratum_v15(cfg, &tect, meters, below, fleck);
        }
        // 4. 3D Tilted, Folded, and Fault-Offset Sedimentary & Metamorphic Stratigraphy:
        let dip_tilt = (dip_coord * 0.045).sin() * (4.0 + tect.boundary_proximity * 6.5);
        let fold_disp = tect.fold_wave * (5.0 + tect.orogeny * 13.5);
        let strat_y =
            meters.y + dip_tilt + fold_disp + tect.fault_throw * 1.15 + (vein() - 0.5) * 2.2;
        let bed_phase = (strat_y / 9.5).rem_euclid(1.0);

        // Bedded Carboniferous coal seams & ancient cratonic banded ironstone formations:
        if (0.47..0.54).contains(&bed_phase) && vein() > 0.62 && fleck > 0.34 {
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
        if (0.20..0.33).contains(&bed_phase)
            && strat_y > -6.0
            && tect.crust_age > 0.38
            && vein() < 0.52
        {
            return BlockId::Sandstone;
        }
        if (0.72..0.79).contains(&bed_phase) && below < 22.0 && vein() < 0.42 {
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
        if meters.y > -12.0 && meters.y < 6.0 && vein() < 0.36 {
            return BlockId::Limestone;
        }
    }
    if vein() > 0.72 && fleck > 0.38 {
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
    use engine_core::VoxelScalePreset;

    #[test]
    fn seeded_tectonics_creates_oceans_coasts_mountain_belts_and_geological_strata() {
        let world =
            crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 17);
        let cfg = world.sample_config();

        let mut ocean_columns = 0;
        let mut coast_columns = 0;
        let mut land_columns = 0;
        let mut alpine_columns = 0;
        let mut max_mountain_height = 0.0_f32;
        let mut volcanic_or_orogenic = 0;
        let mut strata_blocks = std::collections::HashSet::new();
        let mut mountain_surfaces = std::collections::HashSet::new();

        // Generator 53 keeps spawn inland, so the sea sits kilometres away:
        // the grid covers about 8 km rather than 2 km.
        for gz in -10..=10 {
            for gx in -10..=10 {
                let x = gx * 384;
                let z = gz * 384;
                let s = sample(&cfg, x, z);
                let tect = sample_tectonics(x as f32, z as f32, cfg.seed, cfg.generator_version);
                if tect.orogeny > 0.22
                    || tect.volcanism > 0.25
                    || tect.trench > 0.20
                    || tect.rift_graben > 0.20
                {
                    volcanic_or_orogenic += 1;
                }
                max_mountain_height = max_mountain_height.max(s.height);
                if s.biome == Biome::Alpine {
                    alpine_columns += 1;
                }
                match s.biome {
                    Biome::Ocean => ocean_columns += 1,
                    Biome::Coast => coast_columns += 1,
                    _ => land_columns += 1,
                }
                let col = column(&cfg, x, z);
                if s.height > 75.0 {
                    mountain_surfaces.insert(col.surface);
                }
                for d in [4, 10, 18, 28, 42] {
                    let b = geological_stratum(
                        &cfg,
                        glam::IVec3::new(x, col.height - d, z),
                        col.height,
                        d,
                    );
                    strata_blocks.insert(b);
                }
            }
        }

        assert!(
            ocean_columns >= 12,
            "expected true oceans across tectonic grid, got {ocean_columns}"
        );
        assert!(
            land_columns >= 40,
            "expected continents across tectonic grid, got {land_columns}"
        );
        assert!(
            volcanic_or_orogenic >= 10,
            "expected active tectonic boundaries, got {volcanic_or_orogenic}"
        );
        assert!(
            max_mountain_height > 115.0,
            "expected tectonic cordilleras to reach true alpine heights (>115m), got {max_mountain_height}"
        );
        assert!(
            alpine_columns > 0
                || mountain_surfaces.contains(&BlockId::SnowGrass)
                || mountain_surfaces.contains(&BlockId::Stone),
            "expected alpine mountain zonation across high peaks"
        );
        assert!(
            strata_blocks.contains(&BlockId::Limestone),
            "expected marine Limestone strata"
        );
        assert!(
            strata_blocks.contains(&BlockId::Basalt),
            "expected Basalt lithosphere/dikes"
        );
        assert!(
            strata_blocks.contains(&BlockId::Sandstone) || coast_columns > 0,
            "expected Sandstone strata or coastal shelves"
        );
    }

    #[test]
    fn generator_15_shores_are_continuous_and_world_has_variety() {
        use super::super::terrain_variety as tv;
        let world =
            crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 17);
        let base = world.sample_config();
        let measure = |version: u32| {
            let mut cfg = base;
            cfg.generator_version = version;
            let (mut shore_cols, mut steps, mut width_sum, mut width_n) = (0, 0, 0.0f32, 0);
            let mut kinds = std::collections::HashMap::new();
            let mut biomes = std::collections::HashSet::new();
            let mut weird = 0;
            for gz in -60..=60 {
                for gx in -60..=60 {
                    let (x, z) = (gx * 29, gz * 29);
                    let s = sample(&cfg, x, z);
                    biomes.insert(format!("{:?}", s.biome));
                    if version >= 15
                        && tv::anomaly(x as f32, z as f32, cfg.seed)
                            .is_some_and(|a| a.strength > 0.5)
                    {
                        weird += 1;
                    }
                    if (s.height - 12.0).abs() > 2.5 || (x as f32).hypot(z as f32) < 110.0 {
                        continue;
                    }
                    shore_cols += 1;
                    let h = |x, z| column(&cfg, x, z).height;
                    let h0 = h(x, z);
                    steps += i32::from((h(x + 1, z) - h0).abs().max((h(x, z + 1) - h0).abs()) >= 2);
                    // March uphill along +x and -x until 4 m above sea: beach width.
                    for dir in [-1, 1] {
                        let mut run = None;
                        for k in 1..=120 {
                            let hh = column(&cfg, x + dir * k, z).height;
                            if (hh as f32) > 15.0 {
                                run = Some(k);
                                break;
                            }
                        }
                        if let Some(k) = run {
                            width_sum += k as f32;
                            width_n += 1;
                        }
                    }
                    let sh = tv::shore(x as f32, z as f32, cfg.seed, s.temperature, s.moisture);
                    *kinds.entry(format!("{:?}", sh.kind)).or_insert(0) += 1;
                }
            }
            eprintln!(
                "v{version}: shore_cols={shore_cols} step_cols={steps} avg_rise_run={:.1}m biomes={} {:?} weird={weird} kinds={:?}",
                width_sum / width_n.max(1) as f32,
                biomes.len(),
                {
                    let mut b: Vec<_> = biomes.iter().cloned().collect();
                    b.sort();
                    b
                },
                kinds
            );
            (shore_cols, steps)
        };
        let (_, old_steps) = measure(14);
        let (cols, new_steps) = measure(15);
        assert!(cols > 20, "need shoreline samples, got {cols}");
        assert!(
            new_steps <= old_steps,
            "v15 must not add shoreline steps: {new_steps} vs {old_steps}"
        );
    }
}
