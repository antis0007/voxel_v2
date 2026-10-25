//! Generator 15 terrain character: continuous beach profiles, coast types, rare
//! "anomaly" landforms (karst spires, mesas, craters, canyons) and finer climate
//! ecotones. Everything here is a pure function of metre-space position and seed.
//! It owns no cache and never depends on chunk order or voxel size.
use super::{BlockId, WATER_LEVEL_METERS, hash2, smoothstep, value_noise2};

// ---------------------------------------------------------------- climate

/// Widens the climate extremes and roughens biome borders so neighbouring
/// biomes interleave in patches instead of meeting along smooth contours.
pub(super) fn climate(wx: f32, wz: f32, seed: u32, heat: f32, wet: f32) -> (f32, f32) {
    let patch_t = value_noise2(wx / 52.0, wz / 52.0, seed.wrapping_add(9601)) - 0.5;
    let patch_m = value_noise2(wx / 47.0, wz / 47.0, seed.wrapping_add(9603)) - 0.5;
    let fine_t = value_noise2(wx / 13.0, wz / 13.0, seed.wrapping_add(9607)) - 0.5;
    let fine_m = value_noise2(wx / 12.0, wz / 12.0, seed.wrapping_add(9611)) - 0.5;
    let heat = (heat - 0.5) * 1.12 + 0.5 + patch_t * 0.14 + fine_t * 0.05;
    let wet = (wet - 0.5) * 1.10 + 0.5 + patch_m * 0.14 + fine_m * 0.05;
    (heat.clamp(0.0, 1.0), wet.clamp(0.0, 1.0))
}

/// Generator 33: continent-scale climate. The 700 m fields alone give every
/// few hundred metres the same mix of biomes; these kilometre-scale belts and
/// regions let whole provinces run hot, cold, wet or dry. Faded out near the
/// origin so spawn stays temperate.
pub(super) fn regional_climate(wx: f32, wz: f32, seed: u32, heat: f32, wet: f32) -> (f32, f32) {
    let reach = smoothstep(350.0, 1600.0, (wx * wx + wz * wz).sqrt());
    if reach <= 0.0 {
        return (heat, wet);
    }
    // Wavy latitude bands about 7 km apart, phase 0 (temperate) at the origin.
    let lat = wz + (value_noise2(wx / 1900.0, wz / 1900.0, seed.wrapping_add(9701)) - 0.5) * 1800.0;
    let belt = (lat * std::f32::consts::TAU / 7200.0).sin();
    let region_t = value_noise2(wx / 2600.0, wz / 2600.0, seed.wrapping_add(9703)) - 0.5;
    let region_m = value_noise2(wx / 2200.0, wz / 2200.0, seed.wrapping_add(9707)) - 0.5;
    let heat = heat + (belt * 0.13 + region_t * 0.26) * reach;
    let wet = wet + region_m * 0.30 * reach;
    (heat.clamp(0.0, 1.0), wet.clamp(0.0, 1.0))
}

/// Desert ground is not all sand: dry clay pans, gravel plains (hamada) and
/// sandstone outcrops break up the dunes. Returns the surface and filler.
pub(super) fn desert_surface(wx: f32, wz: f32, seed: u32, slope: f32) -> (BlockId, BlockId) {
    let region = value_noise2(wx / 140.0, wz / 140.0, seed.wrapping_add(9901));
    let speck = value_noise2(wx / 31.0, wz / 31.0, seed.wrapping_add(9903));
    if slope > 0.18 || speck > 0.82 {
        (BlockId::Sandstone, BlockId::Sandstone)
    } else if region < 0.22 {
        (BlockId::Clay, BlockId::Sandstone)
    } else if region > 0.78 && speck > 0.45 {
        (BlockId::Gravel, BlockId::Stone)
    } else {
        (BlockId::Sand, BlockId::Sandstone)
    }
}

// ------------------------------------------------------------------ shore

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CoastKind {
    Rocky,
    Shingle,
    Sand,
    Mudflat,
}

#[derive(Clone, Copy)]
pub(super) struct Shore {
    pub kind: CoastKind,
    /// Beach-face height multiplier. Below 1 the beach is flatter and wider.
    pub gain: f32,
    /// Metres above sea level where beach material ends.
    pub limit: f32,
}

/// Coast character for one stretch of shoreline. It changes slowly along the
/// coast (hundreds of metres) so a bay is sandy, the next headland is rock.
pub(super) fn shore(wx: f32, wz: f32, seed: u32, temperature: f32, moisture: f32) -> Shore {
    let sx = wx + (value_noise2(wx / 90.0, wz / 90.0, seed.wrapping_add(9501)) - 0.5) * 70.0;
    let sz = wz + (value_noise2(wx / 90.0, wz / 90.0, seed.wrapping_add(9502)) - 0.5) * 70.0;
    // Value noise spans roughly 0.15..0.86 with a median near 0.5.
    let c = value_noise2(sx / 240.0, sz / 240.0, seed.wrapping_add(9503));
    // Cold coasts lean towards shingle, wet warm coasts towards mudflats.
    let cold = (0.32 - temperature).max(0.0) * 0.9;
    let rocky = 1.0 - smoothstep(0.17, 0.27, c);
    let shingle = 1.0 - smoothstep(0.25, 0.35, c - cold);
    let mud = smoothstep(0.76, 0.86, c) * smoothstep(0.42, 0.58, moisture);
    let length = value_noise2(wx / 70.0, wz / 70.0, seed.wrapping_add(9509));
    let sand_gain = 0.46 + value_noise2(wx / 130.0, wz / 130.0, seed.wrapping_add(9511)) * 0.2;
    let mut gain = sand_gain + shingle * 0.16;
    gain += (1.0 - gain) * rocky;
    gain += (0.36 - gain) * mud;
    let mut limit = 1.1 + length * 1.9;
    limit += (0.3 - limit) * rocky;
    limit += (1.0 - limit) * mud;
    let kind = if rocky > 0.5 {
        CoastKind::Rocky
    } else if mud > 0.5 {
        CoastKind::Mudflat
    } else if shingle > 0.5 {
        CoastKind::Shingle
    } else {
        CoastKind::Sand
    };
    Shore { kind, gain, limit }
}

/// Continuous, monotone reshaping of terrain height around sea level. The
/// shoreline stays where it is, the beach face gets flatter and wider, and
/// there is no vertical step anywhere (the old 1 m snap made a raised sand strip).
pub(super) fn reprofile(h: f32, shore: Shore, version: u32) -> f32 {
    let d = h - WATER_LEVEL_METERS;
    if version < 22 {
        if d >= 0.0 {
            let blend = smoothstep(shore.limit * 0.8, shore.limit * 2.4 + 1.0, d);
            return WATER_LEVEL_METERS + d * (shore.gain + (1.0 - shore.gain) * blend);
        }
        let blend = smoothstep(0.0, -4.0, d);
        return WATER_LEVEL_METERS + d * (0.8 + 0.2 * blend);
    }
    // Generator 21: a wider low-angle surf platform, a concave upper beach and
    // a deeper offshore shelf. The profile stays monotone, so the voxel surface
    // cannot create a wall at the waterline.
    if d >= 0.0 {
        let blend = smoothstep(shore.limit * 0.65, shore.limit * 2.8 + 1.4, d);
        let gain = shore.gain * 0.82;
        // Slightly concave-up above the swash zone: flat where waves run up,
        // steeper into the dune toe.
        let concave = 1.0 + 0.22 * smoothstep(0.0, shore.limit * 1.4, d);
        WATER_LEVEL_METERS + d * (gain + (1.0 - gain) * blend) * concave
    } else {
        let blend = if version >= 37 { 1.0 - smoothstep(-5.0, 0.0, d) } else { smoothstep(0.0, -5.0, d) };
        // Nearshore is flatter (wide shallow shelf); farther out it steepens.
        WATER_LEVEL_METERS + d * (0.48 + 0.52 * blend)
    }
}

/// Surface and filler for beach columns, or `None` away from the waterline.
pub(super) fn shore_surface(
    wx: f32,
    wz: f32,
    seed: u32,
    height: f32,
    slope: f32,
    shore: Shore,
    version: u32,
) -> Option<(BlockId, BlockId)> {
    let d = height - WATER_LEVEL_METERS;
    let patch_scale = if version >= 45 { 43.0 } else { 9.0 };
    let jitter = value_noise2(wx / patch_scale, wz / patch_scale, seed.wrapping_add(9521));
    let beach_limit = if version >= 49 {
        // Low wet strand, with grass-backed coves between exposed headlands.
        // A wide elevation interval formerly coated the whole coastal terrace.
        shore.limit * (0.55 + jitter * 0.65) + 0.25
    } else if version >= 45 {
        // Broad grass/soil tongues interrupt the upper sand band. Reuse the
        // same sediment patch for shingle and wrack instead of adding speckle.
        shore.limit + 0.5 + jitter * 2.4
    } else if version >= 22 {
        shore.limit + 2.2
    } else {
        shore.limit + 0.6
    };
    if !(-3.0..=beach_limit).contains(&d) {
        return None;
    }
    let underwater = d < -0.4;
    // Steep ground never holds sand: cliffs are bare rock whatever the coast type.
    // Generator 17 starts that rock a little sooner so headlands read as cliffs.
    let cliff = slope > if version >= 17 { 0.40 } else { 0.5 };
    let kind = if cliff { CoastKind::Rocky } else { shore.kind };
    if version >= 49 && !cliff && d > 0.9 + jitter * 1.3 {
        return Some((BlockId::Grass, BlockId::Dirt));
    }
    let mut placed = match kind {
        CoastKind::Rocky if !underwater => (
            if jitter > 0.55 {
                BlockId::Stone
            } else {
                BlockId::Gravel
            },
            BlockId::Stone,
        ),
        CoastKind::Shingle if !underwater => (
            if jitter > 0.38 {
                BlockId::Gravel
            } else {
                BlockId::Sand
            },
            BlockId::Gravel,
        ),
        CoastKind::Mudflat if d > -1.2 => (
            if jitter > 0.3 {
                BlockId::Mud
            } else {
                BlockId::Sand
            },
            BlockId::Clay,
        ),
        _ => (BlockId::Sand, BlockId::Sandstone),
    };
    if version >= 17 && !underwater {
        // Wrack line along the berm, and a gravel apron at the toe of a sea cliff.
        let berm = shore.limit * 0.55;
        if matches!(kind, CoastKind::Sand | CoastKind::Shingle)
            && (d - berm).abs() < 0.35
            && jitter > 0.62
        {
            placed.0 = BlockId::Gravel;
        }
        if kind == CoastKind::Rocky && d < if version >= 49 { 0.65 } else { 1.5 } {
            placed = (BlockId::Gravel, BlockId::Stone);
        }
    }
    Some(placed)
}

/// Generator 17 beach face. A low berm, the swale behind it, and short cusps.
/// Rocky coasts get a wave-cut notch instead. The shift stays under about a
/// metre so the waterline does not turn into a step.
pub(super) fn coastal_shoulders(h: f32, wx: f32, wz: f32, seed: u32) -> f32 {
        let d = h - WATER_LEVEL_METERS;
        // Broad coastal shoulders, not small bumps at the waterline. Keep
        // the wet strand fixed while raising broken grass-backed terraces.
        let shoulder = smoothstep(0.15, 1.5, d)
            * (1.0 - smoothstep(5.0, 9.0, d));
        let bank = value_noise2(wx / 38.0, wz / 38.0, seed.wrapping_add(9541));
        let rise = (1.2 + smoothstep(0.20, 0.80, bank) * 2.8) * shoulder;
        h + rise
}

pub(super) fn strand(h: f32, shore: Shore, wx: f32, wz: f32, seed: u32, version: u32) -> f32 {
    let d = h - WATER_LEVEL_METERS;
    match shore.kind {
        CoastKind::Rocky => {
            if !(0.2..6.0).contains(&d) {
                return h;
            }
            let notch = (-((d - 1.15) / 0.7).powi(2)).exp() * 0.5;
            h - notch
        }
        CoastKind::Sand | CoastKind::Shingle => {
            if version < 22 {
                if !(-1.2..shore.limit + 2.8).contains(&d) {
                    return h;
                }
                let fade = smoothstep(-1.2, 0.15, d)
                    * (1.0 - smoothstep(shore.limit + 0.3, shore.limit + 2.6, d));
                let cusp =
                    (value_noise2(wx / 14.0, wz / 14.0, seed.wrapping_add(9533)) - 0.5) * 0.65;
                let centre = shore.limit * 0.55;
                let berm = (-((d - centre) / 0.85).powi(2)).exp() * 0.85;
                let swale = (-((d - centre - 1.7) / 1.15).powi(2)).exp() * 0.5;
                return h + (cusp + berm - swale) * fade;
            }
            // Generator 21: offshore bars and troughs feed the surf; the
            // intertidal platform is flatter, then a low berm, swale and dune
            // toe rise behind it.
            if !(-4.2..shore.limit + 3.6).contains(&d) {
                return h;
            }
            let fade = smoothstep(-4.2, -1.8, d)
                * (1.0 - smoothstep(shore.limit + 0.6, shore.limit + 3.4, d));
            let section_scale = if version >= 45 { 67.0 } else { 17.0 };
            let section = value_noise2(wx / section_scale, wz / section_scale, seed.wrapping_add(9533));
            let cusp = (section - 0.5) * 0.42;
            let bars = if version >= 24 {
                // Bars run along the shore, not around it. Keyed to elevation alone,
                // the sine drew a ring of stripes at constant height on every coast.
                let along = value_noise2(wx / 9.0, wz / 9.0, seed.wrapping_add(9537)) - 0.5;
                along * 0.48 * (1.0 - smoothstep(-3.8, -0.2, d))
            } else {
                let bar_phase =
                    (d + value_noise2(wx / 38.0, wz / 38.0, seed.wrapping_add(9537)) * 1.1) * 2.15;
                bar_phase.sin() * 0.24 * (1.0 - smoothstep(-3.8, -0.2, d))
            };
            let platform = -0.10 * (1.0 - smoothstep(-0.7, 0.18, d));
            // Continuous beach sections vary together: low open strands give
            // way to raised dune toes, rather than three contour-wide rings.
            let shift = if version >= 45 { (section - 0.5) * 1.8 } else { 0.0 };
            let strength = if version >= 45 { smoothstep(0.24, 0.76, section) } else { 1.0 };
            let lower_berm = (-((d - shore.limit * 0.42 - shift) / 0.72).powi(2)).exp() * 0.58 * strength;
            let swale = (-((d - shore.limit * 0.95 - shift) / 1.15).powi(2)).exp() * 0.62 * strength;
            let dune_toe = (-((d - shore.limit * 1.65 - shift) / 1.65).powi(2)).exp() * 0.92 * strength;
            h + (cusp + bars + platform + lower_berm - swale + dune_toe) * fade
        }
        CoastKind::Mudflat => h,
    }
}

// -------------------------------------------------------------- anomalies

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Landform {
    Karst,
    Mesa,
    Crater,
    Canyon,
}

#[derive(Clone, Copy)]
pub(super) struct Anomaly {
    pub kind: Landform,
    /// 0..1 overall presence of any anomaly here.
    pub strength: f32,
    /// Weight of each landform, in `Landform` order. Sums to 1.
    pub weights: [f32; 4],
}

const LANDFORMS: [Landform; 4] = [
    Landform::Karst,
    Landform::Mesa,
    Landform::Crater,
    Landform::Canyon,
];

/// Roughly one in six land areas gets a wild landform. Region membership is a
/// smooth blend so no landform ever meets another across a hard seam.
pub(super) fn anomaly(wx: f32, wz: f32, seed: u32) -> Option<Anomaly> {
    let spawn = smoothstep(240.0, 420.0, wx.hypot(wz));
    if spawn <= 0.0 {
        return None;
    }
    let field = value_noise2(wx / 560.0, wz / 560.0, seed.wrapping_add(9701));
    let strength = smoothstep(0.76, 0.88, field) * spawn;
    if strength <= 0.0 {
        return None;
    }
    let k = value_noise2(wx / 1300.0 + 11.3, wz / 1300.0, seed.wrapping_add(9709));
    let mut weights = [0.0f32; 4];
    for (i, w) in weights.iter_mut().enumerate() {
        let centre = [0.26, 0.46, 0.64, 0.80][i];
        *w = (1.0 - (k - centre).abs() / 0.16).max(0.0);
    }
    let sum: f32 = weights.iter().sum();
    if sum <= 1e-4 {
        weights = [0.25; 4];
    } else {
        weights.iter_mut().for_each(|w| *w /= sum);
    }
    let mut best = 0;
    for i in 1..4 {
        if weights[i] > weights[best] {
            best = i;
        }
    }
    Some(Anomaly {
        kind: LANDFORMS[best],
        strength,
        weights,
    })
}

/// Height change in metres and extra slope for an anomaly at a land column.
pub(super) fn anomaly_shape(a: &Anomaly, wx: f32, wz: f32, seed: u32, h: f32, version: u32) -> (f32, f32) {
    let land = smoothstep(20.0, 34.0, h);
    if land <= 0.0 {
        return (0.0, 0.0);
    }
    let mut delta = 0.0;
    let mut slope = 0.0;
    if a.weights[0] > 0.01 {
        delta += a.weights[0] * karst(wx, wz, seed);
        slope += a.weights[0] * 0.30;
    }
    if a.weights[1] > 0.01 {
        delta += a.weights[1] * mesa(wx, wz, seed, h, version);
        slope += a.weights[1] * 0.22;
    }
    if a.weights[2] > 0.01 {
        delta += a.weights[2] * crater(wx, wz, seed);
        slope += a.weights[2] * 0.10;
    }
    if a.weights[3] > 0.01 {
        delta += a.weights[3] * canyon(wx, wz, seed, h);
        slope += a.weights[3] * 0.34;
    }
    (delta * a.strength * land, slope * a.strength * land)
}

/// Steep limestone towers rising out of sink-hole studded ground.
fn karst(wx: f32, wz: f32, seed: u32) -> f32 {
    let cells = value_noise2(wx / 26.0, wz / 26.0, seed.wrapping_add(9731));
    let tower = smoothstep(0.58, 0.70, cells);
    let height = 14.0 + value_noise2(wx / 70.0, wz / 70.0, seed.wrapping_add(9733)) * 34.0;
    let sink = (1.0 - smoothstep(0.18, 0.40, cells)) * 4.5;
    tower * height - sink
}

/// Stepped plateaus. Before generator 23 the riser was the last 23% of each
/// step, which made sheer 3-voxel walls that read as banding from the spawn
/// view. From 23 the riser spans the whole step as a smooth S-curve, so the
/// plateaus stay but the walls become gentle benches. Old worlds keep their
/// walls because their stored version is below 23.
fn mesa(wx: f32, wz: f32, seed: u32, h: f32, version: u32) -> f32 {
    let rise = smoothstep(
        0.46,
        0.60,
        value_noise2(wx / 190.0, wz / 190.0, seed.wrapping_add(9741)),
    ) * 30.0;
    let raised = h + rise;
    let t = raised / 8.0;
    let riser = if version >= 23 { (0.0, 1.0) } else { (0.72, 0.95) };
    let stepped = (t.floor() + smoothstep(riser.0, riser.1, t.fract())) * 8.0;
    stepped - h
}

/// A bowl with a raised rim around jittered centres on a 480 m grid.
fn crater(wx: f32, wz: f32, seed: u32) -> f32 {
    const CELL: f32 = 480.0;
    let gx = (wx / CELL).floor() as i32;
    let gz = (wz / CELL).floor() as i32;
    let mut delta = 0.0;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (ix, iz) = (gx + dx, gz + dz);
            if hash2(ix, iz, seed.wrapping_add(9751)) > 0.62 {
                continue;
            }
            let cx = (ix as f32 + 0.2 + hash2(ix, iz, seed.wrapping_add(9753)) * 0.6) * CELL;
            let cz = (iz as f32 + 0.2 + hash2(ix, iz, seed.wrapping_add(9757)) * 0.6) * CELL;
            let radius = 80.0 + hash2(ix, iz, seed.wrapping_add(9761)) * 80.0;
            // Warp the distance field so the rim is lobed and broken, not a compass circle.
            let warp = (value_noise2(wx / 55.0, wz / 55.0, seed.wrapping_add(9763)) - 0.5) * 0.5;
            let r = (wx - cx).hypot(wz - cz) / radius * (1.0 + warp);
            if r > 1.6 {
                continue;
            }
            let bowl = (1.0 - smoothstep(0.0, 0.62, r)) * 18.0;
            let breaks = 0.35 + 1.3 * value_noise2(wx / 38.0, wz / 38.0, seed.wrapping_add(9767));
            let rim = (-((r - 0.85) / 0.16).powi(2)).exp() * 24.0 * breaks;
            // Large craters rebound into a central peak.
            let peak = if radius > 120.0 {
                (-(r / 0.13).powi(2)).exp() * 15.0
            } else {
                0.0
            };
            delta += rim - bowl + peak;
        }
    }
    delta
}

/// Narrow meandering gorges cut deep into high ground.
fn canyon(wx: f32, wz: f32, seed: u32, h: f32) -> f32 {
    let bend = (value_noise2(wx / 130.0, wz / 130.0, seed.wrapping_add(9771)) - 0.5) * 90.0;
    let n = value_noise2(
        (wx + bend) / 210.0,
        (wz - bend) / 210.0,
        seed.wrapping_add(9773),
    );
    let ridge = 1.0 - ((n - 0.5) * 2.0).abs();
    -smoothstep(0.88, 0.98, ridge) * (h - 14.0).clamp(0.0, 28.0)
}

/// 0..1 noise used to dither anomaly borders (surface and biome).
pub(super) fn ragged_edge(wx: f32, wz: f32, seed: u32) -> f32 {
    value_noise2(wx / 23.0, wz / 23.0, seed.wrapping_add(9785))
}

/// Surface and filler that make an anomaly look like what it is.
pub(super) fn anomaly_surface(
    a: &Anomaly,
    wx: f32,
    wz: f32,
    seed: u32,
    height: f32,
    slope: f32,
) -> Option<(BlockId, BlockId)> {
    // Ragged edge: the surface threshold wanders with a 23 m noise so the
    // landform fades into its neighbour in patches, not along a clean contour.
    if a.strength < 0.10 + ragged_edge(wx, wz, seed) * 0.55 {
        return None;
    }
    let patch = value_noise2(wx / 14.0, wz / 14.0, seed.wrapping_add(9781));
    Some(match a.kind {
        Landform::Karst => {
            if slope > 0.5 || patch > 0.55 {
                (BlockId::Limestone, BlockId::Limestone)
            } else {
                return None;
            }
        }
        Landform::Mesa => {
            let band = ((height / 2.6).floor() as i32).rem_euclid(3);
            let surface = match band {
                0 => BlockId::Clay,
                1 => BlockId::Sandstone,
                _ if patch > 0.5 => BlockId::Clay,
                _ => BlockId::Sandstone,
            };
            (surface, BlockId::Sandstone)
        }
        Landform::Crater => {
            if slope > 0.35 {
                (BlockId::Basalt, BlockId::Basalt)
            } else if patch > 0.5 {
                (BlockId::Gravel, BlockId::Stone)
            } else {
                return None;
            }
        }
        Landform::Canyon => {
            if slope > 0.4 {
                (BlockId::Sandstone, BlockId::Sandstone)
            } else if patch > 0.45 {
                (BlockId::Gravel, BlockId::Stone)
            } else {
                return None;
            }
        }
    })
}

// ------------------------------------------------------- eroded morphology

/// Value noise in [0,1] on an integer lattice with quintic smoothing, plus its
/// analytic gradient in lattice units.
fn noise_d(x: f32, z: f32, seed: u32) -> (f32, f32, f32) {
    let (ix, iz) = (x.floor(), z.floor());
    let (fx, fz) = (x - ix, z - iz);
    let (ix, iz) = (ix as i32, iz as i32);
    let a = hash2(ix, iz, seed);
    let b = hash2(ix + 1, iz, seed);
    let c = hash2(ix, iz + 1, seed);
    let d = hash2(ix + 1, iz + 1, seed);
    let quint = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let dquint = |t: f32| 30.0 * t * t * (t - 1.0) * (t - 1.0);
    let (ux, uz) = (quint(fx), quint(fz));
    let k = a - b - c + d;
    let value = a + (b - a) * ux + (c - a) * uz + k * ux * uz;
    let dx = dquint(fx) * ((b - a) + k * uz);
    let dz = dquint(fz) * ((c - a) + k * ux);
    (value, dx, dz)
}

/// Erosion-like fractal noise in about [-1, 1]. Each octave is damped by the
/// slope already accumulated by the coarser ones, so valley floors stay smooth
/// while ridges and spurs keep their small-scale roughness, the same pattern
/// water leaves behind. `footprint` (metres) fades out octaves the caller
/// cannot resolve, which stops distant rings from aliasing into spikes.
fn eroded_fbm(x: f32, z: f32, seed: u32, wavelength: f32, octaves: u32, footprint: f32) -> f32 {
    let (mut px, mut pz) = (x / wavelength, z / wavelength);
    let (mut gx, mut gz) = (0.0f32, 0.0f32);
    let mut amp = 0.5;
    let mut wave = wavelength;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for octave in 0..octaves {
        let (v, dx, dz) = noise_d(px, pz, seed.wrapping_add(octave * 29));
        // Full strength by ~2.4 samples per wave: the horizon grid can show that
        // octave. The old window waited until 4 samples, so those ridges were
        // computed and then multiplied away. Finer octaves still fade before Nyquist.
        let resolved = smoothstep(footprint * 1.15, footprint * 2.4, wave);
        gx += dx * 2.0;
        gz += dz * 2.0;
        sum += amp * resolved * (v * 2.0 - 1.0) / (1.0 + (gx * gx + gz * gz) * 0.55);
        norm += amp;
        amp *= 0.5;
        wave /= 2.02;
        // Rotate each octave so grid axes never line up between scales.
        let (rx, rz) = (px * 0.8 - pz * 0.6, px * 0.6 + pz * 0.8);
        px = rx * 2.02 + 17.3;
        pz = rz * 2.02 - 9.1;
    }
    sum / norm.max(1e-4) * 2.0
}

/// Height change in metres applied on top of the tectonic bedrock at `h`:
/// eroded drainage relief, dendritic mountain spurs, and structural benches
/// where hard rock layers outcrop on hillsides. Lowlands barely move so coasts
/// and plains stay readable. `footprint` is the caller's sample spacing in metres.
pub(super) fn relief(wx: f32, wz: f32, seed: u32, h: f32, footprint: f32, version: u32) -> f32 {
    let upland = smoothstep(26.0, 60.0, h);
    if h < 16.0 {
        return 0.0;
    }
    let mountain = smoothstep(55.0, 125.0, h);
    // Uplifted plateaus. A narrow transition makes their rims real escarpments
    // (cuestas and sea cliffs) instead of one more gentle rise.
    // Generator 17 squeezes that rim and leaves a talus apron on the toe.
    let plateau_field = value_noise2(wx / 1500.0, wz / 1500.0, seed.wrapping_add(9841))
        + (value_noise2(wx / 330.0, wz / 330.0, seed.wrapping_add(9843)) - 0.5) * 0.08;
    let (rim_lo, rim_hi) = if version >= 37 {
        (0.59, 0.73)
    } else if version >= 17 {
        (0.648, 0.672)
    } else {
        (0.62, 0.68)
    };
    let plateau = smoothstep(rim_lo, rim_hi, plateau_field) * smoothstep(16.0, 26.0, h);
    let lift = if version >= 17 { 30.0 } else { 22.0 };
    let plateau_lift =
        plateau * (lift + 16.0 * value_noise2(wx / 640.0, wz / 640.0, seed.wrapping_add(9845)));
    let rolling = eroded_fbm(wx, wz, seed.wrapping_add(9801), 380.0, 6, footprint);
    let mut delta = rolling * (4.0 + 26.0 * mountain);
    if mountain > 0.0 {
        // Dendritic ridge network: crest lines branch like river basins in reverse.
        let spurs = eroded_fbm(
            wx + 311.0,
            wz - 177.0,
            seed.wrapping_add(9821),
            210.0,
            5,
            footprint,
        );
        let crest = 1.0 - spurs.abs().min(1.0);
        let crest_amp = if version >= 17 { 50.0 } else { 34.0 };
        delta += mountain * (crest * crest - 0.30) * crest_amp;
        if version >= 17 {
            // Cut saddles so a range is a line of peaks, not one long knife-edge.
            let saddle = value_noise2(wx / 96.0, wz / 96.0, seed.wrapping_add(9827));
            delta -= mountain * smoothstep(0.38, 0.66, saddle) * crest * 16.0;
        }
    }
    if version >= 17 {
        let toe =
            smoothstep(0.56, 0.63, plateau_field) * (1.0 - smoothstep(0.645, 0.71, plateau_field));
        delta += toe * 8.0 * smoothstep(18.0, 34.0, h);
    }
    // Structural benches: a tilted stack of hard and soft beds. On slopes the hard
    // beds stand as steps and the soft beds as gentle aprons.
    let lithology = value_noise2(wx / 430.0, wz / 430.0, seed.wrapping_add(9831));
    let hardness = smoothstep(0.42, 0.72, lithology);
    if hardness > 0.0 && footprint < 48.0 {
        let period = 6.5 + 8.0 * value_noise2(wx / 900.0, wz / 900.0, seed.wrapping_add(9833));
        let bed = (h + wx * 0.031 + wz * 0.022) / period;
        let f = bed - bed.floor();
        let step = smoothstep(0.55, 0.90, f);
        let fade = 1.0 - smoothstep(16.0, 48.0, footprint);
        delta += (step - f) * period * 0.6 * hardness * fade;
    }
    delta * upland + plateau_lift
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beach() -> Shore {
        Shore {
            kind: CoastKind::Sand,
            gain: 0.5,
            limit: 2.5,
        }
    }

    #[test]
    fn generator_22_beach_face_is_monotone_and_wider() {
        let shore = beach();
        let mut previous = f32::NEG_INFINITY;
        for step in -40..=60 {
            let h = WATER_LEVEL_METERS + step as f32 * 0.2;
            let p = reprofile(h, shore, 22);
            assert!(p.is_finite());
            assert!(p + 1e-5 >= previous, "beach profile reversed at {h}");
            previous = p;
        }
        assert!(
            reprofile(WATER_LEVEL_METERS + 2.0, shore, 22)
                < reprofile(WATER_LEVEL_METERS + 2.0, shore, 21),
            "generator 22 should flatten the beach face"
        );
    }

    #[test]
    fn generator_22_strand_stays_local() {
        let shore = beach();
        for step in -25..=80 {
            let h = WATER_LEVEL_METERS + step as f32 * 0.2;
            let shaped = strand(h, shore, 12.3, -4.5, 7, 22);
            assert!(shaped.is_finite());
            assert!(
                (shaped - h).abs() < 1.8,
                "beach ornament moved height by {} at {h}",
                shaped - h
            );
        }
    }

    #[test]
    fn painterly_strand_is_bounded_and_continuous_across_sections() {
        for x in [-200.0, -67.0, -0.01, 0.0, 67.0, 200.0] {
            for step in -30..=80 {
                let h = WATER_LEVEL_METERS + step as f32 * 0.1;
                let a = strand(h, beach(), x, 19.0, 7, 45);
                let b = strand(h, beach(), x + 0.01, 19.0, 7, 45);
                assert!((a - h).abs() < 1.8);
                assert!((a - b).abs() < 0.01);
            }
        }
    }
}
