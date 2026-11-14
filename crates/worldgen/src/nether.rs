//! Open-sky Nether regions derived from their parent planet, in world metres.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Biome {
    AshBarrens,
    FungalHollows,
    CinderRifts,
    BasaltScarps,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Self::AshBarrens => "Ash Barrens",
            Self::FungalHollows => "Fungal Hollows",
            Self::CinderRifts => "Cinder Rifts",
            Self::BasaltScarps => "Basalt Scarps",
        }
    }
}

fn direction(x: f32, z: f32) -> glam::DVec3 {
    engine_core::cube_sphere::atlas::to_body(glam::DVec3::new(f64::from(x), 0.0, f64::from(z))).normalize()
}

/// Shared body-space fields keep biome boundaries continuous across cube faces.
fn regions(cfg: &WorldSampleConfig, direction: glam::DVec3) -> (Biome, f64, f64, f64) {
    let sample = |salt| crate::planet::noise_at(direction * 96.0, cfg.seed ^ salt);
    let ash = sample(0x4153_4842);
    let fungal = sample(0x4655_4e47);
    let heat = sample(0x4349_4e44);
    let scarp = sample(0x5343_4152);
    let mut biome = Biome::AshBarrens;
    let mut highest = ash;
    for (score, candidate) in [
        (fungal, Biome::FungalHollows),
        (heat, Biome::CinderRifts),
        (scarp, Biome::BasaltScarps),
    ] {
        if score > highest {
            highest = score;
            biome = candidate;
        }
    }
    (biome, fungal, heat, scarp)
}

pub fn biome(cfg: &WorldSampleConfig, x_m: f32, z_m: f32) -> Option<Biome> {
    (cfg.world_type == WorldType::Nether && cfg.generator_version == engine_voxel::HELL_BIOME_GENERATOR)
        .then(|| regions(cfg, direction(x_m, z_m)).0)
}

#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub temperature: f32,
    pub moisture: f32,
    pub relief: f32,
}

pub fn profile(cfg: &WorldSampleConfig, x: i32, z: i32) -> Profile {
    let parent = WorldSampleConfig {
        world_type: WorldType::Hills,
        ..*cfg
    };
    let climate = sample_climate(&parent, x, z);
    Profile {
        temperature: 1.0 - climate.temperature,
        moisture: 1.0 - climate.moisture,
        relief: 1.0 - climate.continentalness,
    }
}

const CAMP: f32 = 112.0;

fn spared(cfg: &WorldSampleConfig, mx: f32, mz: f32) -> bool {
    if mx.hypot(mz) < 24.0 {
        return true;
    }
    if cfg.generator_version >= crate::hell_gate::VERSION {
        let gate = crate::hell_gate::center(cfg, glam::Vec3::new(mx, 0.0, mz));
        if (mx - gate.x).hypot(mz - gate.z) < 28.0 {
            return true;
        }
    }
    false
}

fn camp_at(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<(f32, f32)> {
    let h = hash_u32(cfg.seed ^ (ix as u32).wrapping_mul(0x9E37_79B9) ^ (iz as u32).wrapping_mul(0x85EB_CA6B));
    if h % 3 != 0 {
        return None;
    }
    let x = ix as f32 * CAMP + (h % 40) as f32 - 8.0;
    let z = iz as f32 * CAMP + ((h >> 8) % 40) as f32 - 8.0;
    (!spared(cfg, x, z)).then_some((x, z))
}

fn near_camp(cfg: &WorldSampleConfig, mx: f32, mz: f32) -> Option<(f32, f32)> {
    let ix = (mx / CAMP).round() as i32;
    let iz = (mz / CAMP).round() as i32;
    let mut best = None;
    let mut best_d = 6.0_f32;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let Some((x, z)) = camp_at(cfg, ix + dx, iz + dz) else { continue };
            let d = (mx - x).hypot(mz - z);
            if d < best_d {
                best = Some((x, z));
                best_d = d;
            }
        }
    }
    best
}

/// Metres added to the open ash field. Spawn and buried gates stay on the older field.
pub fn ground_delta(cfg: &WorldSampleConfig, mx: f32, mz: f32) -> f32 {
    if cfg.generator_version < crate::SEEDED_REALMS || spared(cfg, mx, mz) {
        return 0.0;
    }
    let angle = hash01(cfg.seed ^ 0x4E45_5448) * std::f32::consts::TAU;
    let along = mx * angle.cos() + mz * angle.sin();
    let across = -mx * angle.sin() + mz * angle.cos();
    let canal = across.abs() < 2.4 && along.abs() < 180.0;
    if canal {
        return -5.0;
    }
    let rib = ((along / 36.0).fract() - 0.5).abs() < 0.055 && across.abs() < 140.0;
    let gate = (along / 8.0).fract() < 0.22;
    let mut delta: f32 = if rib && !gate { 8.0 } else { 0.0 };
    if near_camp(cfg, mx, mz).is_some() {
        delta = delta.max(4.0);
    }
    delta
}

pub fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    if cfg.is_hell() { return hell_column(cfg, x, z); }
    let p = profile(cfg, x, z);
    let (mx, mz) = (cfg.world_x(x), cfg.world_z(z));
    let broad = fbm2(mx * 0.003, mz * 0.003, cfg.seed ^ 0x48454c4c);
    let ridges = ridge(fbm2(mx * 0.011, mz * 0.011, cfg.seed ^ 0xc13fa9a9));
    let base = 5.0 + 50.0 * broad + (12.0 + 36.0 * p.relief) * ridges;
    let height = cfg.clamp_height(cfg.blocks_for_height(base + ground_delta(cfg, mx, mz)));
    let canal = ground_delta(cfg, mx, mz) < 0.0;
    let surface = if canal {
        BlockId::Lava
    } else if p.temperature < 0.40 {
        BlockId::Ice
    } else if p.moisture > 0.60 {
        BlockId::FungalSoil
    } else if p.temperature > 0.56 {
        BlockId::Ash
    } else {
        BlockId::Basalt
    };
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone,
        height,
        surface,
        filler: BlockId::Basalt,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

/// Broad traversable shelves separated by branching lava cuts. This version
/// is exclusive to new Hell realms; historical Nether terrain is unchanged.
fn hell_column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    let (mx, mz) = (cfg.world_x(x), cfg.world_z(z));
    let direction = direction(mx, mz);
    if cfg.hell_mirror() { return mirror_column(cfg, direction); }
    let broad = crate::planet::noise_at(direction * 64.0, cfg.seed ^ 0x4845_4c4c) as f32;
    let warp = crate::planet::noise_at(direction * 160.0, cfg.seed ^ 0x4352_494d);
    let field = crate::planet::noise_at(direction * 220.0 + glam::DVec3::splat(warp * 0.4), cfg.seed ^ 0x5249_4654) as f32;
    let cut = (field - 0.5).abs();
    let mut shelf = ((24.0 + broad * 65.0) / 6.0).floor() * 6.0;
    let mut rim = ((cut - 0.025) / 0.065).clamp(0.0, 1.0);
    if cfg.hell_ecology() {
        let crags = crate::planet::noise_at(direction * 145.0, cfg.seed ^ 0x5350_4952) as f32;
        let rubble = crate::planet::noise_at(direction * 1200.0, cfg.seed ^ 0x4153_484c) as f32;
        shelf += ((crags - 0.53).max(0.0) * 175.0 / 3.0).floor() * 3.0 + (rubble - 0.5) * 4.0;
        // Keep a walkable bank above lava; steepen the upper walls.
        rim = 0.32 * rim + 0.68 * ((rim - 0.38) / 0.62).clamp(0.0, 1.0).powf(1.65);
    }
    let height_m = 10.0 + (shelf - 10.0) * rim;
    let surface = if cut < 0.025 { BlockId::Lava }
        else if cut < 0.065 { BlockId::Basalt }
        else if crate::planet::noise_at(direction * 700.0, cfg.seed ^ 0x4153_4800) > 0.72 { BlockId::Ash }
        else { BlockId::Hellstone };
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone, height: cfg.clamp_height(cfg.blocks_for_height(height_m)), surface,
        filler: BlockId::Hellstone, vegetation_density: 0.0, cave: Default::default() }
}

/// Invert the parent recipe at several scales on the sphere. Its ocean and
/// rainfall dials become lava extent and incision, while its continental
/// scale and relief determine the width and height of the surviving shelves.
fn mirror_column(cfg: &WorldSampleConfig, direction: glam::DVec3) -> ColumnSample {
    let parent = cfg.planet_settings().unwrap_or_default();
    let seed = cfg.seed;
    let field = |frequency, salt| crate::planet::noise_at(direction * frequency, seed ^ salt);
    let ocean = parent.unit(0);
    let continents = parent.unit(1);
    let relief = parent.unit(2);
    let erosion = parent.unit(3);
    let heat = parent.unit(4);
    let rain = parent.unit(5);

    let plate_frequency = 6.0 + 13.0 * (1.0 - continents);
    let plate = field(plate_frequency, 0x504c_4154);
    let district = field(plate_frequency * 2.6, 0x4449_5354);
    let warp = field(65.0, 0x5741_5250) - 0.5;
    let fracture = crate::planet::noise_at(
        direction * (90.0 + 145.0 * rain) + glam::DVec3::splat(warp * 0.72),
        seed ^ 0x5249_4654,
    );
    let cut = (fracture - 0.5).abs();
    let region = (cfg.generator_version == engine_voxel::HELL_BIOME_GENERATOR).then(|| regions(cfg, direction));
    let mut channel = 0.008 + 0.022 * rain;
    let mut basin_line = 0.16 + 0.38 * ocean;
    let mut lift = 0.0;
    if let Some((_, fungal, heat, scarp)) = region {
        channel = (channel + 0.13 * (heat - fungal)).clamp(0.004, 0.095);
        basin_line = (basin_line + 0.25 * (heat - fungal)).clamp(0.08, 0.68);
        lift = 125.0 * (scarp - 0.5).max(0.0) - 38.0 * (fungal - 0.5).max(0.0);
    }
    let basin = plate * 0.74 + district * 0.26;
    let shore = ((basin - basin_line) / 0.14).clamp(0.0, 1.0);
    let bank = ((cut - channel) / (0.052 + 0.07 * erosion)).clamp(0.0, 1.0);
    let spire = ((field(132.0, 0x5350_4952) - 0.52).max(0.0) * 2.0).powi(2);
    let shelf = 18.0 + district * 36.0 + plate * (25.0 + 83.0 * relief)
        + spire * (18.0 + 115.0 * relief) + lift;
    let terrace = 3.0 + 5.0 * (1.0 - erosion);
    let stepped = (shelf / terrace).floor() * terrace;
    let rubble = (field(970.0, 0x5255_424c) - 0.5) * 3.0;
    let lava = basin < basin_line || cut < channel;
    let height = if lava { 10.0 } else {
        10.0 + (stepped + rubble) * shore.min(bank).powf(1.15 + erosion)
    };

    // Existing voxel materials carry their real textures, drops and physics.
    // The seed shifts the palette boundary without changing the recipe dials.
    let palette = 0.60 * heat + 0.24 * rain + 0.16 * f64::from(hash01(seed ^ 0x5041_4c45));
    let accent = field(plate_frequency * 3.1, 0x434f_4c52) > 0.52;
    let surface = if lava { BlockId::Lava }
        else if shore.min(bank) < 0.25 { BlockId::Basalt }
        else if let Some((biome, ..)) = region {
            match biome {
                Biome::AshBarrens => if accent { BlockId::Ash } else { BlockId::Hellstone },
                Biome::FungalHollows => if accent { BlockId::FungalSoil } else { BlockId::Hellstone },
                Biome::CinderRifts => if accent { BlockId::Basalt } else { BlockId::Hellstone },
                Biome::BasaltScarps => {
                    if spire > 0.18 && field(320.0, 0x4745_4f44) > 0.72 { BlockId::Crystal }
                    else if accent { BlockId::Basalt } else { BlockId::Hellstone }
                }
            }
        }
        else if palette < 0.42 { if accent { BlockId::Ash } else { BlockId::Gravel } }
        else if palette > 0.64 { if accent { BlockId::Hellstone } else { BlockId::Sandstone } }
        else if accent { BlockId::Hellstone } else { BlockId::Clay };
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone,
        height: cfg.clamp_height(cfg.blocks_for_height(height as f32)),
        surface,
        filler: BlockId::Hellstone,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

fn ruin_ground(cfg: &WorldSampleConfig, surface: BlockId) -> bool {
    matches!(surface, BlockId::Hellstone | BlockId::Ash)
        || (cfg.hell_mirror() && matches!(surface, BlockId::Gravel | BlockId::Sandstone | BlockId::Clay))
        || (cfg.generator_version == engine_voxel::HELL_BIOME_GENERATOR && matches!(surface, BlockId::Basalt | BlockId::FungalSoil))
}

/// Soot-dark stalks and low, harvestable amber caps. One deterministic colony
/// per patch; the footprint is entirely inside that patch at every voxel scale.
fn hell_growth(cfg: &WorldSampleConfig, p: IVec3, c: ColumnSample) -> BlockId {
    let max_height = if cfg.generator_version == engine_voxel::HELL_BIOME_GENERATOR { 9.0 } else { 5.0 };
    if (p.y - c.height) as f32 * cfg.voxel_meters() > max_height || c.surface == BlockId::Lava { return BlockId::Air; }
    let scale = cfg.voxel_meters();
    let m = (p.as_vec3() + glam::Vec3::splat(0.5)) * scale;
    let gx = (m.x / 9.0).floor();
    let gz = (m.z / 9.0).floor();
    let h = hash_u32(cfg.seed ^ (gx as i32 as u32).wrapping_mul(0x9e37_79b9)
        ^ (gz as i32 as u32).wrapping_mul(0x85eb_ca6b) ^ 0x4655_4e47);
    let x = gx * 9.0 + 3.0 + (h & 7) as f32 * 0.38;
    let z = gz * 9.0 + 3.0 + ((h >> 8) & 7) as f32 * 0.38;
    let d = (m.x - x).hypot(m.z - z);
    if d > 2.45 { return BlockId::Air; }
    let region = biome(cfg, x, z);
    let colonies = if let Some(region) = region {
        match region {
            Biome::FungalHollows => 4,
            Biome::AshBarrens => 1,
            Biome::CinderRifts | Biome::BasaltScarps => 0,
        }
    } else if cfg.hell_mirror() {
        let parent = cfg.planet_settings().unwrap_or_default();
        // Forest-rich parents become fungal badlands.
        1 + (parent.unit(7) * 3.0).round() as u32
    } else { 2 };
    if h % 5 >= colonies { return BlockId::Air; }
    let fungal = region == Some(Biome::FungalHollows);
    if !fungal && d > 1.65 { return BlockId::Air; }
    let a = sample_column(cfg, (x / scale).floor() as i32, (z / scale).floor() as i32);
    if a.surface == BlockId::Lava || (a.height - c.height).abs() as f32 * scale > 1.25 { return BlockId::Air; }
    let dy = m.y - (a.height + 1) as f32 * scale;
    let height = 1.25 + ((h >> 20) & 7) as f32 * 0.27;
    if dy < 0.0 { return BlockId::Air; }
    if fungal {
        let trunk = height + 2.0;
        let crown = (d / 2.45).powi(2) + ((dy - trunk) / 1.15).powi(2);
        if crown < 1.0 {
            return if h & 0x10000 == 0 { BlockId::GlowCap } else { BlockId::UmberCap };
        }
        if d < 0.45_f32.max(scale * 0.72) && dy < trunk { return BlockId::MushroomStem; }
        if d < 1.8 && dy < scale.max(0.65) && h & 0x20000 == 0 { return BlockId::AmberCap; }
        return BlockId::Air;
    }
    let axis = if h & 0x1000 == 0 { glam::Vec2::X } else { glam::Vec2::Y };
    let side = if h & 0x2000 == 0 { -1.0 } else { 1.0 };
    let offset = glam::Vec2::new(m.x - x, m.z - z);
    let rise = (dy / height).clamp(0.0, 1.0);
    let stem = offset - axis * (side * rise * rise * 0.65);
    let taper = (0.86 - rise * 0.61).max(scale * 0.65);
    let stalk = dy < height && stem.length() < taper;
    let split = ((rise - 0.33) * 1.8).clamp(0.0, 1.0);
    let spur = dy > height * 0.28 && dy < height * 0.82
        && (stem + axis * side * split).length() < (0.48 - split * 0.22).max(scale * 0.55);
    if stalk || spur { return BlockId::UmberCap; }
    // Amber grows in small pockets at the root, never in bright flat caps.
    if h & 0xc000 == 0 && dy < scale.max(0.65) && d < 1.3 && offset.dot(axis) * side < 0.15 {
        return BlockId::AmberCap;
    }
    BlockId::Air
}

/// Vertical block run for one ruin column, independent of voxel Y.
#[derive(Clone, Copy)]
pub struct RuinColumn {
    clearance: f32,
    height: f32,
    body: BlockId,
    foundation: BlockId,
    cap: BlockId,
}

impl RuinColumn {
    fn block(self, above: i32, scale: f32) -> Option<BlockId> {
        let y = (above - 1) as f32 * scale;
        if y < self.clearance || y >= self.height || above as f32 * scale > 10.0 { return None; }
        Some(if y < 1.0 && self.foundation != self.body { self.foundation }
            else if y >= self.height - scale { self.cap }
            else { self.body })
    }
}

/// Resolves the footprint once per column. Each wall is founded on dry ground.
pub fn ruin_column(cfg: &WorldSampleConfig, x: i32, z: i32, c: ColumnSample) -> Option<RuinColumn> {
    type Key = (u32, u32, u32, u64, i32, i32);
    type Site = Option<(f32, f32, u32)>;
    thread_local! {
        static LAST_SITE: std::cell::Cell<Option<(Key, Site)>> = const { std::cell::Cell::new(None) };
    }
    ruin_column_with(cfg, x, z, c, |ix, iz| LAST_SITE.with(|cache| {
        let key = (cfg.seed, cfg.generator_version, cfg.voxel_meters().to_bits(), cfg.planet_fingerprint(), ix, iz);
        if let Some((old, site)) = cache.get() && old == key { return site; }
        let site = ruin_site(cfg, ix, iz);
        cache.set(Some((key, site)));
        site
    }))
}

pub(super) fn ruin_site(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<(f32, f32, u32)> {
    let (cx, cz) = camp_at(cfg, ix, iz)?;
    let style = hash_u32(cfg.seed ^ (ix as u32).rotate_left(13) ^ iz as u32);
    if cfg.generator_version != engine_voxel::HELL_SURVEYED_RUINS_GENERATOR && !cfg.hell_mirror() {
        return Some((cx, cz, style));
    }

    // Keep any adjustment inside the original camp cell. Test the aisle,
    // cache and main supports against the same terrain the voxel path uses.
    let scale = cfg.voxel_meters();
    let mut offsets = [(7.0, 0.0), (0.0, 7.0), (-7.0, 0.0), (0.0, -7.0)];
    offsets.rotate_left(((style >> 2) & 3) as usize);
    for (dx, dz) in std::iter::once((0.0, 0.0)).chain(offsets) {
        let (x, z) = (cx + dx, cz + dz);
        if spared(cfg, x, z) { continue; }
        let style = match biome(cfg, x, z) {
            Some(Biome::FungalHollows) => (style & !6) | 4,
            Some(Biome::CinderRifts) => (style & !6) | 2,
            Some(Biome::AshBarrens) => (style & !6) | 6,
            Some(Biome::BasaltScarps) => style & !6,
            None => style,
        };
        let ground = |mut u: f32, mut v: f32| {
            if style & 1 != 0 { std::mem::swap(&mut u, &mut v); }
            let c = sample_column(cfg, ((x + u) / scale).round() as i32, ((z + v) / scale).round() as i32);
            ruin_ground(cfg, c.surface).then_some(c.height)
        };
        let Some(mut last) = ground(0.0, -10.0) else { continue; };
        let mut walkable = true;
        for step in -9..=1 {
            let Some(y) = ground(0.0, step as f32) else { walkable = false; break; };
            if (y - last).abs() as f32 * scale > 1.0 { walkable = false; break; }
            last = y;
        }
        if !walkable || ground(-2.0, -3.0).is_none_or(|y| (y - last).abs() as f32 * scale > 2.0) {
            continue;
        }
        let reach = match style & 6 {
            4 | 6 if cfg.generator_version == engine_voxel::HELL_BIOME_GENERATOR => 5.5,
            _ if style & 2 == 0 => 6.0,
            _ => 4.0,
        };
        if [-reach, reach].into_iter().all(|u| [-reach, 0.0, reach].into_iter().all(|v| {
            ground(u, v).is_some_and(|y| (y - last).abs() as f32 * scale <= 3.0)
        })) {
            return Some((x, z, style));
        }
    }
    None
}

pub(super) fn ruin_column_with(
    cfg: &WorldSampleConfig, x: i32, z: i32, c: ColumnSample,
    site: impl FnOnce(i32, i32) -> Option<(f32, f32, u32)>,
) -> Option<RuinColumn> {
    if !ruin_ground(cfg, c.surface) { return None; }
    let v = cfg.voxel_meters();
    let m = glam::Vec3::new(x as f32 * v, 0.0, z as f32 * v);
    let solid = |height, body| RuinColumn { clearance: 0.0, height, body, foundation: body, cap: body };
    // The full ruin and the largest camp offset fit within half a grid cell.
    let ix = (m.x / CAMP).round() as i32;
    let iz = (m.z / CAMP).round() as i32;
    let (cx, cz, style) = site(ix, iz)?;
    let (mut x, mut z) = (m.x - cx, m.z - cz);
    if x.abs().max(z.abs()) > 8.0 { return None; }
    if style & 1 != 0 { std::mem::swap(&mut x, &mut z); }
    let (ax, az) = (x.abs(), z.abs());


    // The open approach leaves the supply cache accessible from the aisle.
    if x >= -2.0 - v * 0.5 && x < -2.0 + v * 0.5 && z >= -3.0 - v * 0.5 && z < -3.0 + v * 0.5 {
        return Some(solid(v, BlockId::SalvageCrate));
    }
    let regional_layout = cfg.generator_version == engine_voxel::HELL_BIOME_GENERATOR;
    if regional_layout && style & 6 == 4 {
        // A ruined cultivator shelter. The roof shades ground suitable for caps.
        if ax < 5.5 && az < 4.5 {
            let clearance = if ax > 4.0 || z > 3.0 { 0.0 } else { 3.0 };
            return Some(RuinColumn { clearance, ..solid(4.0, BlockId::WeatheredBricks) });
        }
    } else if regional_layout && style & 6 == 6 {
        if ax < 5.0 && az < 5.0 {
            let clearance = if ax > 3.8 || z > 3.8 { 0.0 } else { 3.0 };
            return Some(RuinColumn { clearance, ..solid(4.0, BlockId::Basalt) });
        }
    } else if style & 2 == 0 {
        // Broken ramparts, with corner towers and a wide breach on one side.
        let tower = (ax - 6.0).abs() < 1.25 && (az - 6.0).abs() < 1.25;
        let wall = (ax < 6.5 && (z - 6.0).abs() < 0.8)
            || ((ax - 6.0).abs() < 0.8 && az < 6.5 && z > -4.0);
        let tower_height = if x * z > 0.0 { 8.0 } else { 5.0 };
        if tower {
            return Some(RuinColumn { foundation: BlockId::Basalt, ..solid(tower_height, BlockId::WeatheredBricks) });
        }
        if wall {
            return Some(solid(3.0, BlockId::WeatheredBricks));
        }
    } else {
        // Two rows of eroded basalt pillars frame a walkable central aisle.
        let shaft = (ax - 4.0).abs() < 0.85 && ((az - 4.0).abs() < 0.85 || az < 0.85);
        let height = if az < 0.85 { 9.0 } else if x * z > 0.0 { 6.0 } else { 4.0 };
        if shaft {
            return Some(RuinColumn { cap: BlockId::Crystal, ..solid(height, BlockId::Basalt) });
        }
        if ax < 5.2 && az < 6.5 && ax > 2.8 {
            return Some(solid(v, BlockId::WeatheredBricks));
        }
    }
    None
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3, c: ColumnSample) -> BlockId {
    block_with(cfg, p, c, &CachedProfiles)
}

pub(super) fn block_with(cfg: &WorldSampleConfig, p: IVec3, c: ColumnSample, profiles: &impl Profiles) -> BlockId {
    let scale = cfg.voxel_meters();
    if p.y <= cfg.min_world_y() + cfg.blocks_for_span(1.0) {
        return BlockId::Bedrock;
    }
    if p.y > c.height {
        let above = p.y - c.height;
        if cfg.hell_ruins()
            && above as f32 * scale <= 10.0
            && let Some(block) = profiles.hell_ruin(cfg, p.x, p.z, c).and_then(|r| r.block(above, scale))
        {
            return block;
        }
        return if cfg.hell_ecology() { hell_growth(cfg, p, c) } else { BlockId::Air };
    }
    let m = p.as_vec3() * scale;
    let depth = (c.height - p.y) as f32 * scale;
    if depth < 1.0 && depth >= 0.0 {
        if cfg.generator_version != engine_voxel::HELL_BIOME_GENERATOR
            && let Some((x, z)) = near_camp(cfg, m.x, m.z) {
            if (m.x - x).hypot(m.z - z) < 0.8 {
                return BlockId::Crystal;
            }
        }
        return c.surface;
    }
    // Deep fissures and lava pockets; no ceiling blocks the sky.
    let cave = value_noise3(m.x * 0.055, m.y * 0.065, m.z * 0.055, cfg.seed ^ 0x66a94f31);
    let parent = if cfg.hell_mirror() { cfg.planet_settings().unwrap_or_default() } else { engine_voxel::PlanetSettings::default() };
    let cave_threshold = if cfg.hell_mirror() { 0.81 - parent.unit(8) as f32 * 0.30 } else { 0.66 };
    if depth > 5.0 && cave > cave_threshold {
        return if m.y < 8.0 {
            BlockId::Lava
        } else {
            BlockId::Air
        };
    }
    let ore = value_noise3(m.x * 0.19, m.y * 0.19, m.z * 0.19, cfg.seed ^ 0x9e37abcd);
    let crystal_threshold = if cfg.hell_mirror() { 0.88 - parent.unit(9) as f32 * 0.20 } else { 0.78 };
    if depth > 3.0 && ore > crystal_threshold {
        return BlockId::Crystal;
    }
    if depth > 2.0 && ore < 0.17 {
        return BlockId::IronOre;
    }
    if cfg.is_hell() {
        if (m.y / 6.0).floor() as i32 % 5 == 0 { BlockId::Basalt } else { BlockId::Hellstone }
    } else { BlockId::Basalt }
}

/// Camps near the player. Identity is the grid cell, so a clear survives reconnect.
pub fn encounters(cfg: &WorldSampleConfig, player: glam::Vec3) -> Vec<(IVec3, glam::Vec3, u32)> {
    if cfg.generator_version < crate::SEEDED_REALMS {
        return Vec::new();
    }
    let ix = (player.x / CAMP).round() as i32;
    let iz = (player.z / CAMP).round() as i32;
    let mut out = Vec::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            let candidate = if cfg.generator_version == engine_voxel::HELL_BIOME_GENERATOR {
                ruin_site(cfg, ix + dx, iz + dz).map(|(x, z, _)| (x, z))
            } else {
                camp_at(cfg, ix + dx, iz + dz)
            };
            let Some((x, z)) = candidate else { continue };
            let cell = ((glam::Vec3::new(x, 0.0, z)) / cfg.voxel_meters()).floor().as_ivec3();
            let ground = column(cfg, cell.x, cell.z);
            if cfg.hell_ecology() && !ground.surface.is_solid() { continue; }
            let feet = glam::Vec3::new(x, (ground.height as f32 + 1.0) * cfg.voxel_meters(), z);
            if (feet - player).length() > 80.0 {
                continue;
            }
            let gx = ix + dx;
            let gz = iz + dz;
            let roll = hash_u32(cfg.seed ^ gx as u32 ^ (gz as u32).wrapping_mul(17));
            let kind = match biome(cfg, x, z) {
                Some(Biome::FungalHollows) => 25, // Corpse Light
                Some(Biome::CinderRifts) => 12, // Kiln Crawler
                Some(Biome::BasaltScarps) => 21, // Pit Imp
                Some(Biome::AshBarrens) => [21, 25][roll as usize % 2],
                None if cfg.hell_ecology() => [12, 12, 21, 21, 25][roll as usize % 5],
                None => roll % 6,
            };
            out.push((IVec3::new(gx.wrapping_mul(4096).wrapping_add(gz), 500, 1), feet, kind));
        }
    }
    out
}

#[cfg(test)]
mod seeded_tests {
    use super::*;
    use engine_core::VoxelScalePreset;

    fn cfg(version: u32, seed: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            planet: None,
            generator_version: version,
            seed,
            world_type: WorldType::Nether,
            voxel_scale: VoxelScalePreset::OneMeter,
        }
    }

    #[test]
    fn the_spawn_disk_keeps_the_older_field() {
        let old = column(&cfg(crate::SEEDED_REALMS - 1, 7), 0, 0);
        let new = column(&cfg(crate::SEEDED_REALMS, 7), 0, 0);
        assert_eq!(old.height, new.height);
        assert_eq!(old.surface, new.surface);
    }

    #[test]
    fn a_seed_changes_ground_away_from_spawn() {
        let mut moved = false;
        for x in -40..40 {
            for z in 24..70 {
                let older = column(&cfg(crate::SEEDED_REALMS - 1, 5), x, z).height;
                let newer = column(&cfg(crate::SEEDED_REALMS, 5), x, z).height;
                if older != newer {
                    moved = true;
                }
            }
        }
        assert!(moved, "seeded nether matched the older field everywhere");
    }

    #[test]
    fn one_seed_repeats_the_same_camps() {
        let world = cfg(crate::SEEDED_REALMS, 19);
        let player = glam::Vec3::new(180.0, 30.0, 60.0);
        assert_eq!(encounters(&world, player), encounters(&world, player));
        assert!(encounters(&cfg(crate::SEEDED_REALMS - 1, 19), player).is_empty());
    }
}

/// Bound Keplerian orbit. Retrograde inclination and eccentricity disturb the
/// sky without changing energy or inventing camera-dependent trajectories.
pub fn satellite(seed: u32, index: u32, seconds: f64) -> glam::DVec3 {
    // Four authored satellites; arbitrary public inputs must not turn the
    // ellipse into a hyperbola or introduce NaNs into sky geometry.
    let index = index.min(3);
    let seconds = if seconds.is_finite() { seconds } else { 0.0 };
    let phase = (seed
        .wrapping_mul(1664525)
        .wrapping_add(index.wrapping_mul(1013904223)) as f64
        / u32::MAX as f64)
        * std::f64::consts::TAU;
    let radius = 1_592_750.0;
    let a = radius + 420_000.0 + index as f64 * 370_000.0;
    let eccentricity = 0.12 + index as f64 * 0.035;
    let mu = 9.81 * radius * radius;
    let mean = (phase + seconds * (mu / (a * a * a)).sqrt()).rem_euclid(std::f64::consts::TAU);
    let mut e = mean;
    for _ in 0..8 {
        e -= (e - eccentricity * e.sin() - mean) / (1.0 - eccentricity * e.cos());
    }
    let plane = glam::DVec3::new(
        a * (e.cos() - eccentricity),
        a * (1.0 - eccentricity * eccentricity).sqrt() * e.sin(),
        0.0,
    );
    let inclination = (115.0 + index as f64 * 17.0).to_radians();
    glam::DQuat::from_rotation_y(phase) * glam::DQuat::from_rotation_x(inclination) * plane
}
