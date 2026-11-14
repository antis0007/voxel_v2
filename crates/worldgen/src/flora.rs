//! Gate 210 wild flora: water plants, shore grasses, wild crop patches, desert plants,
//! small mangrove and fruit trees, and cave plants. Every placement is positional hashing.
use crate::{ColumnSample, WorldSampleConfig, WorldType, floor_div, hash2, hash3, landscape, sample_column, value_noise2};
use engine_content::BlockId as B;
use glam::IVec3;

/// Gate 210; live while `CURRENT_GENERATOR_VERSION` is at least this.
pub const VERSION: u32 = engine_voxel::WILD_FLORA_GENERATOR;
/// One small tree candidate per square of this size.
const TREE_CELL_METRES: f32 = 9.0;
const SEED: u32 = 0x00f1_02a5;

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION && matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged | WorldType::Planet)
}

/// Climate at a column: (temperature, moisture), both about 0..1.
fn climate(cfg: &WorldSampleConfig, x: i32, z: i32) -> (f32, f32) {
    let l = landscape::sample(cfg, x, z);
    (l.temperature, l.moisture)
}

/// Patch strength from metre-space noise, so patches keep their size at every voxel scale.
fn patch(cfg: &WorldSampleConfig, p: IVec3, metres: f32, salt: u32) -> f32 {
    value_noise2(cfg.world_x(p.x) / metres, cfg.world_z(p.z) / metres, cfg.seed ^ SEED ^ salt)
}

/// True for the one cell per `metres` square that hashes as its anchor.
fn anchor(cfg: &WorldSampleConfig, p: IVec3, metres: f32, salt: u32) -> bool {
    let span = cfg.blocks_for_span(metres).max(1);
    let (cx, cz) = (floor_div(p.x, span), floor_div(p.z, span));
    let seed = cfg.seed ^ SEED ^ salt;
    let ax = ((hash2(cx, cz, seed) * span as f32) as i32).min(span - 1);
    let az = ((hash2(cx, cz, seed ^ 31) * span as f32) as i32).min(span - 1);
    p.x.rem_euclid(span) == ax && p.z.rem_euclid(span) == az
}

/// The wild plant at `p` above the column's surface, or `Air`.
pub fn block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> B {
    if !active(cfg) || p.y <= column.height {
        return B::Air;
    }
    let water = cfg.water_level();
    let h = p.y - column.height;
    if column.height < water {
        let aquatic = aquatic(cfg, p, column, water);
        if aquatic != B::Air {
            return aquatic;
        }
    } else if living(cfg) {
        let plant = living_plant(cfg, p, column, water);
        if plant != B::Air {
            return plant;
        }
        // Cactus columns keep their per-voxel anchors, base cell included.
        if h <= cfg.blocks_for_span(3.0) && column.surface == B::Sand && anchor(cfg, p, 4.0, 0xca) {
            let base = IVec3::new(p.x, column.height + 1, p.z);
            let tall = 1 + (hash2(p.x, p.z, cfg.seed ^ 0xcac) * 3.0) as i32;
            if h <= tall.min(cfg.blocks_for_span(3.0)) && ground(cfg, base, column, water) == B::Cactus {
                return B::Cactus;
            }
        }
    } else if h == 1 {
        let ground = ground(cfg, p, column, water);
        if ground != B::Air {
            return ground;
        }
    } else if h <= cfg.blocks_for_span(3.0) && column.surface == B::Sand && anchor(cfg, p, 4.0, 0xca) {
        // Cactus columns: the base cell decides, upper cells follow it.
        let base = IVec3::new(p.x, column.height + 1, p.z);
        let tall = 1 + (hash2(p.x, p.z, cfg.seed ^ 0xcac) * 3.0) as i32;
        if h <= tall.min(cfg.blocks_for_span(3.0)) && ground(cfg, base, column, water) == B::Cactus {
            return B::Cactus;
        }
    }
    tree(cfg, p, water)
}

fn aquatic(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample, water: i32) -> B {
    let depth = water - column.height;
    let depth_m = depth as f32 * cfg.voxel_meters();
    let h = p.y - column.height;
    let bed = matches!(column.surface, B::Sand | B::Gravel | B::Clay | B::Mud | B::Dirt);
    if p.y == water + 1 {
        if depth_m <= 2.5 && patch(cfg, p, 14.0, 0x11) > 0.62 && hash2(p.x, p.z, cfg.seed ^ 0x1e) < 0.3
            && climate(cfg, p.x, p.z).1 > 0.45
        {
            return B::LilyPad;
        }
        return B::Air;
    }
    if p.y > water || !bed {
        return B::Air;
    }
    if depth_m >= 3.0 && anchor(cfg, p, 1.5, 0x4e) && patch(cfg, p, 22.0, 0x4f) > 0.5 {
        let length = cfg.blocks_for_span(1.5 + 4.0 * hash2(p.x, p.z, cfg.seed ^ 0x4e1)).max(1);
        if h <= length && p.y < water {
            return B::Kelp;
        }
    }
    if h != 1 {
        return B::Air;
    }
    let roll = hash3(p.x, p.y, p.z, cfg.seed ^ 0x5e);
    if depth == 1 && matches!(column.surface, B::Mud | B::Clay | B::Dirt | B::Sand) {
        let reeds = patch(cfg, p, 10.0, 0xca7);
        if roll < 0.35 * reeds && reeds > 0.5 {
            let (t, m) = climate(cfg, p.x, p.z);
            return if t > 0.6 && m > 0.65 && hash2(p.x, p.z, cfg.seed ^ 0x21) < 0.4 { B::RiceCrop } else { B::Cattail };
        }
        return B::Air;
    }
    if (0.5..4.0).contains(&depth_m) && roll < 0.4 * patch(cfg, p, 12.0, 0x5f) {
        return B::Seagrass;
    }
    B::Air
}

/// Generator 295 planets: climate ground plants, as metre-sized stamps at sub-metre voxels.
fn living(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Planet && crate::planetary::living_surface(cfg.generator_version)
}

/// The ground plant whose stamp holds `p`, or `Air`. One candidate per metre
/// square: the plant roots on the square's minimum corner column and fills a
/// box of cells above that column's surface.
fn living_plant(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample, water: i32) -> B {
    let metres = cfg.voxel_meters();
    let n = engine_content::stamp::cells_per_metre(metres);
    if p.y - column.height > 2 * n {
        return B::Air;
    }
    let (lx, lz) = (floor_div(p.x, n), floor_div(p.z, n));
    let roll = hash2(lx, lz, cfg.seed ^ SEED ^ 0x295);
    // Nothing grows more often than this, so most squares skip every sample.
    if roll >= 0.3 {
        return B::Air;
    }
    let (ax, az) = (lx * n, lz * n);
    let wide = engine_content::stamp::footprint(metres);
    if p.x - ax >= wide || p.z - az >= wide {
        return B::Air;
    }
    let base = if (p.x, p.z) == (ax, az) { column } else { sample_column(cfg, ax, az) };
    // The stamp stands on its root column; higher ground beside it cuts it short.
    if base.height < water || column.height > base.height || p.y <= base.height {
        return B::Air;
    }
    let root = IVec3::new(ax, base.height + 1, az);
    let plant = living_ground(cfg, root, base, water, roll);
    if plant == B::Air {
        return B::Air;
    }
    let tall = if n == 1 { 1 } else if engine_content::stamp::stampable(plant) {
        engine_content::stamp::height_cells(plant, root, metres)
    } else { 1 };
    // A one-cell plant stays on its root; only stamps spread over the square.
    if tall == 1 && (p.x, p.z) != (ax, az) {
        return B::Air;
    }
    if p.y - base.height <= tall { plant } else { B::Air }
}

/// `living_plant` over uniform ground: every square roots on `column`.
#[cfg(test)]
fn living_plant_on(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> B {
    let metres = cfg.voxel_meters();
    let n = engine_content::stamp::cells_per_metre(metres);
    let (lx, lz) = (floor_div(p.x, n), floor_div(p.z, n));
    let roll = hash2(lx, lz, cfg.seed ^ SEED ^ 0x295);
    let wide = engine_content::stamp::footprint(metres);
    if roll >= 0.3 || p.x - lx * n >= wide || p.z - lz * n >= wide || p.y <= column.height { return B::Air; }
    let root = IVec3::new(lx * n, column.height + 1, lz * n);
    let plant = living_ground(cfg, root, column, cfg.water_level(), roll);
    if plant == B::Air || !engine_content::stamp::stampable(plant) { return B::Air; }
    if p.y - column.height <= engine_content::stamp::height_cells(plant, root, metres) { plant } else { B::Air }
}

/// The plant rooted at `root` for its climate and soil; `roll` is the square's 0..1 draw.
fn living_ground(cfg: &WorldSampleConfig, root: IVec3, base: ColumnSample, water: i32, roll: f32) -> B {
    let above = (base.height - water) as f32 * cfg.voxel_meters();
    match base.surface {
        // Foredune grass reaches up the dune ridges behind the beach.
        B::Sand if above <= 9.0 => {
            if roll < 0.22 * patch(cfg, root, 9.0, 0x0a) { B::SeaOats } else { B::Air }
        }
        B::Sand | B::Sandstone | B::Gravel => {
            let (t, m) = climate(cfg, root.x, root.z);
            if t >= 0.62 && m <= 0.4 && roll < 0.04 { B::Agave } else { B::Air }
        }
        B::Mud | B::Clay if above <= 1.5 => {
            let reeds = patch(cfg, root, 10.0, 0xca7);
            if reeds > 0.45 && roll < 0.28 * reeds { B::Cattail } else { B::Air }
        }
        B::Grass | B::Dirt | B::SnowGrass | B::GroveMoss => {
            let (t, m) = climate(cfg, root.x, root.z);
            let shore = above <= 2.5;
            // Wild crops keep their dense patches and wide gaps.
            let strength = patch(cfg, root, 18.0, 0xc0);
            if strength >= 0.72 {
                if roll > 0.3 * (strength - 0.72) / 0.28 {
                    return B::Air;
                }
                let pick = hash2(floor_div(root.x, 16), floor_div(root.z, 16), cfg.seed ^ 0xc1);
                return wild_crop(t, m, shore, base.surface, pick);
            }
            let thicket = patch(cfg, root, 31.0, 0x7c);
            let meadow = patch(cfg, root, 23.0, 0x3e);
            let ripe = hash2(root.x, root.z, cfg.seed ^ 0xbe) < 0.35;
            let berry = if ripe { B::SweetBerries } else { B::BerryBush };
            if base.surface == B::SnowGrass || t < 0.35 {
                // Cold heath: berry scrub in drifts.
                if thicket > 0.55 && roll < 0.10 * (thicket - 0.55) / 0.45 { berry } else { B::Air }
            } else if base.surface == B::GroveMoss || (t > 0.62 && m > 0.66) {
                if meadow > 0.5 && roll < 0.08 * (meadow - 0.5) / 0.5 { B::Orchid } else { B::Air }
            } else if t > 0.62 && m < 0.4 {
                if roll < 0.025 { B::Agave } else { B::Air }
            } else if m < 0.42 {
                // Dry temperate grassland: lavender drifts.
                if meadow > 0.5 && roll < 0.18 * (meadow - 0.5) / 0.5 { B::Lavender } else { B::Air }
            } else if thicket > 0.6 && roll < 0.09 * (thicket - 0.6) / 0.4 {
                berry
            } else if meadow > 0.68 && roll < 0.05 {
                B::Lavender
            } else {
                B::Air
            }
        }
        _ => B::Air,
    }
}

/// Single-cell ground plants on the surface: shore grass, wild crop patches, desert plants.
fn ground(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample, water: i32) -> B {
    let mut roll = hash3(p.x, p.y, p.z, cfg.seed ^ 0x9a);
    // Generator 290 planets keep plants per square metre, not per voxel:
    // quarter-metre voxels grew sixteen times as many one-cell plants.
    if cfg.world_type == WorldType::Planet && crate::planetary::clean_riverbeds(cfg.generator_version) {
        let area = (cfg.voxel_meters() * cfg.voxel_meters()).min(1.0);
        if roll >= area { return B::Air; }
        roll /= area;
    }
    // Nothing below grows more often than this, so most cells skip the climate sample.
    if roll >= 0.3 {
        return B::Air;
    }
    let shore = column.height <= water + cfg.blocks_for_span(2.5);
    match column.surface {
        B::Sand if shore => {
            if roll < 0.12 * patch(cfg, p, 9.0, 0x0a) {
                return B::SeaOats;
            }
            B::Air
        }
        B::Sand | B::Sandstone | B::Gravel => {
            let (t, m) = climate(cfg, p.x, p.z);
            if t < 0.62 || m > 0.4 {
                return B::Air;
            }
            if column.surface == B::Sand && anchor(cfg, p, 4.0, 0xca) && roll < 0.25 {
                B::Cactus
            } else if roll < 0.02 {
                B::Agave
            } else {
                B::Air
            }
        }
        B::Grass | B::Dirt | B::SnowGrass | B::GroveMoss => {
            // Wild crops grow in patches, as in Farmer's Delight: dense spots, wide gaps.
            let strength = patch(cfg, p, 18.0, 0xc0);
            if strength < 0.72 || roll > 0.3 * (strength - 0.72) / 0.28 {
                return B::Air;
            }
            let (t, m) = climate(cfg, p.x, p.z);
            let pick = hash2(floor_div(p.x, 16), floor_div(p.z, 16), cfg.seed ^ 0xc1);
            wild_crop(t, m, shore, column.surface, pick)
        }
        _ => B::Air,
    }
}

/// The wild crop for a climate; `pick` chooses among those that fit.
fn wild_crop(t: f32, m: f32, shore: bool, surface: B, pick: f32) -> B {
    let choices: &[B] = if shore {
        &[B::Cabbage, B::BeetCrop]
    } else if surface == B::SnowGrass || t < 0.35 {
        &[B::Potato, B::SweetBerries, B::SweetBerries]
    } else if t < 0.62 {
        if m < 0.42 {
            &[B::Lavender, B::Onion, B::WheatCrop]
        } else {
            &[B::Onion, B::CarrotCrop, B::Pumpkin, B::SweetBerries]
        }
    } else if m > 0.66 {
        &[B::Orchid, B::Melon, B::Orchid]
    } else if m < 0.4 {
        &[B::Tomato, B::Grapes]
    } else {
        &[B::Grapes, B::Tomato, B::Melon]
    };
    choices[((pick * choices.len() as f32) as usize).min(choices.len() - 1)]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Mangrove,
    Fruit(B),
}

#[derive(Clone, Copy)]
pub(super) struct TreeSite {
    root: IVec3,
    kind: Kind,
}

#[inline]
fn tree_anchor(tx: i32, tz: i32, span: i32, radius: i32, seed: u32) -> Option<(i32, i32)> {
    if hash2(tx, tz, seed ^ 0x55) > 0.4 { return None; }
    let margin = radius + 1;
    let free = (span - 2 * margin).max(1) as f32;
    Some((
        tx * span + margin + ((hash2(tx, tz, seed) * free) as i32),
        tz * span + margin + ((hash2(tx, tz, seed ^ 31) * free) as i32),
    ))
}

/// Samples one procedural small-tree site.
pub(super) fn tree_site(cfg: &WorldSampleConfig, tx: i32, tz: i32) -> Option<TreeSite> {
    let span = cfg.blocks_for_span(TREE_CELL_METRES).max(4);
    let radius = cfg.blocks_for_span(2.5).max(1);
    let seed = cfg.seed ^ SEED ^ 0x7e;
    let (ax, az) = tree_anchor(tx, tz, span, radius, seed)?;
    let base = sample_column(cfg, ax, az);
    let kind = kind(cfg, ax, az, base, cfg.water_level(), hash2(tx, tz, seed ^ 0x99))?;
    Some(TreeSite { root: IVec3::new(ax, base.height, az), kind })
}

/// Cells of a small mangrove or fruit tree whose anchor lies near `p`.
fn tree(cfg: &WorldSampleConfig, p: IVec3, water: i32) -> B {
    let span = cfg.blocks_for_span(TREE_CELL_METRES).max(4);
    let radius = cfg.blocks_for_span(2.5).max(1);
    let (cx, cz) = (floor_div(p.x, span), floor_div(p.z, span));
    // Solved Hills/Ridged climate made repeated placement probes expensive.
    // Planet and historical samplers keep their cheaper direct lookup.
    let cache_sites = matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged) && crate::geo::active(cfg);
    let seed = cfg.seed ^ SEED ^ 0x7e;
    for tz in cz - 1..=cz + 1 {
        for tx in cx - 1..=cx + 1 {
            let Some((ax, az)) = tree_anchor(tx, tz, span, radius, seed) else { continue };
            let (dx, dz) = (p.x - ax, p.z - az);
            if dx.abs() > radius + 2 || dz.abs() > radius + 2 { continue; }
            let site = if cache_sites {
                crate::generation_cache::flora_tree(cfg, tx, tz)
            } else {
                let base = sample_column(cfg, ax, az);
                kind(cfg, ax, az, base, water, hash2(tx, tz, seed ^ 0x99))
                    .map(|kind| TreeSite { root: IVec3::new(ax, base.height, az), kind })
            };
            let Some(site) = site else { continue };
            let b = tree_cell(cfg, p, site.root, site.kind, water);
            if b != B::Air { return b; }
        }
    }
    B::Air
}

fn kind(cfg: &WorldSampleConfig, x: i32, z: i32, base: ColumnSample, water: i32, pick: f32) -> Option<Kind> {
    let near = (base.height - water).abs() <= cfg.blocks_for_span(1.0).max(1);
    let land = base.height > water;
    match base.surface {
        B::Mud | B::Clay | B::Sand if near => {
            let (t, m) = climate(cfg, x, z);
            (t > 0.6 && m > 0.55).then_some(Kind::Mangrove)
        }
        B::Grass | B::Dirt if land && pick < 0.5 => {
            let (t, m) = climate(cfg, x, z);
            Some(Kind::Fruit(if t > 0.66 { B::Fig } else if t < 0.45 || m > 0.6 { B::Cherry } else { B::Apple }))
                .filter(|_| (0.3..0.8).contains(&t))
        }
        _ => None,
    }
}

/// The block of a tree anchored on ground cell `root` at `p`, or `Air`.
fn tree_cell(cfg: &WorldSampleConfig, p: IVec3, root: IVec3, kind: Kind, water: i32) -> B {
    let mangrove = kind == Kind::Mangrove;
    // Mangroves stand on the waterline so their crowns stay level over the shallows.
    let ground = if mangrove { root.y.max(water) } else { root.y };
    let trunk = cfg.blocks_for_span(if mangrove { 4.0 } else { 3.0 }).max(2);
    let r = cfg.blocks_for_span(if mangrove { 2.5 } else { 2.0 }).max(1);
    let rh = cfg.blocks_for_span(1.5).max(1);
    let d = p - IVec3::new(root.x, ground, root.z);
    if d.x == 0 && d.z == 0 && (1..=trunk).contains(&d.y) {
        return B::Log;
    }
    // Fig trunks carry cocoa pods below the crown.
    if kind == Kind::Fruit(B::Fig) && d.y == 1 && (d.x.abs() + d.z.abs()) == 1
        && hash3(p.x, p.y, p.z, cfg.seed ^ 0xc0c0) > 0.5
    {
        return B::CocoaPod;
    }
    let centre = IVec3::new(0, trunk + 1, 0);
    let c = d - centre;
    let reach = (c.x * c.x + c.z * c.z) as f32 / ((r as f32 + 0.5).powi(2))
        + (c.y * c.y) as f32 / ((rh as f32 + 0.5).powi(2));
    let ragged = hash3(p.x, p.y, p.z, cfg.seed ^ 0x7e1);
    if reach <= 1.0 && !(reach > 0.7 && ragged < 0.3) {
        if let Kind::Fruit(fruit) = kind
            && c.y <= 0 && reach > 0.45 && ragged > 0.8
        {
            return fruit;
        }
        return B::Leaves;
    }
    // Propagules hang under the crown rim.
    if mangrove && c.y == -rh - 1 && reach_xz(c, r) <= 0.8 && ragged > 0.86 {
        return B::MangrovePropagule;
    }
    if mangrove && p.y > water && (1..=cfg.blocks_for_span(2.0).max(2)).contains(&d.y) {
        let out = d.x.abs().max(d.z.abs());
        let axis = d.x == 0 || d.z == 0;
        let lift = cfg.blocks_for_span(1.0).max(1);
        if axis && ((out == 1 && d.y >= lift) || (out == 2 && d.y < lift)) {
            return B::MangroveRoots;
        }
    }
    B::Air
}

fn reach_xz(c: IVec3, r: i32) -> f32 {
    (c.x * c.x + c.z * c.z) as f32 / ((r as f32 + 0.5).powi(2))
}

/// Cave plant for a carved air cell at `p`, or `Air`. `solid` tells whether a cell is rock.
pub fn cave(cfg: &WorldSampleConfig, p: IVec3, solid: impl Fn(IVec3) -> bool) -> B {
    if !active(cfg) {
        return B::Air;
    }
    let roll = hash3(p.x, p.y, p.z, cfg.seed ^ 0xca5e);
    if roll >= 0.1 {
        return B::Air;
    }
    let lush = patch(cfg, p, 24.0, 0x105);
    if lush < 0.45 {
        return B::Air;
    }
    if solid(p + IVec3::Y) {
        return if roll < 0.035 { B::GlowBerries } else if roll < 0.08 { B::CaveVine } else { B::SporeBlossom };
    }
    if solid(p - IVec3::Y) && roll < 0.04 {
        return B::Dripleaf;
    }
    B::Air
}

/// True when generated rock fills `q`; the rock test `cave` uses during generation.
pub fn rock_at(cfg: &WorldSampleConfig, q: IVec3) -> bool {
    let column = sample_column(cfg, q.x, q.z);
    q.y <= column.height && !crate::cave_carves(cfg, q, &column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;

    fn cfg(version: u32, seed: u32) -> WorldSampleConfig {
        WorldSampleConfig { planet: None, generator_version: version, seed, world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter }
    }

    #[test]
    fn flora_waits_for_its_gate() {
        let mut cfg = cfg(VERSION - 1, 11);
        assert!(!active(&cfg));
        for x in -40..40 {
            let c = sample_column(&cfg, x, -x * 3);
            assert_eq!(block(&cfg, IVec3::new(x, c.height + 1, -x * 3), c), B::Air);
        }
        cfg.generator_version = VERSION;
        assert!(active(&cfg));
    }

    #[test]
    fn living_plants_are_stamps_with_one_root_at_sub_metre_voxels() {
        use engine_content::stamp::{self, Role};
        for scale in [engine_core::VoxelScalePreset::HalfMeter, engine_core::VoxelScalePreset::QuarterMeter] {
            let cfg = WorldSampleConfig { voxel_scale: scale, ..cfg(engine_voxel::LIVING_SURFACE_GENERATOR, 9) };
            let metres = cfg.voxel_meters();
            let n = stamp::cells_per_metre(metres);
            // A flat meadow column; every lattice square samples the same ground.
            let column = ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone, height: cfg.water_level() + 40, surface: B::Grass, filler: B::Dirt,
                vegetation_density: 0.0, cave: Default::default() };
            let at = |q: IVec3| if q.y <= column.height { B::Dirt } else { living_plant_on(&cfg, q, column) };
            let (mut roots, mut cells) = (0, 0);
            for z in -160..160 { for x in -160..160 { for y in 1..=2 * n {
                let q = IVec3::new(x, column.height + y, z);
                let b = at(q);
                if b == B::Air || !stamp::stampable(b) { continue; }
                cells += 1;
                match stamp::role(b, q, metres, at) {
                    Role::Root(grow) => {
                        roots += 1;
                        assert_eq!(grow, n as f32);
                        assert_eq!((q.x.rem_euclid(n), q.z.rem_euclid(n), q.y), (0, 0, column.height + 1));
                    }
                    Role::Lone => panic!("lone stamp cell {b:?} at {q}"),
                    Role::Member => {}
                }
            }}}
            assert!(roots > 20, "{scale:?}: {roots} plants");
            assert!(cells >= roots * 2, "{scale:?}: {cells} cells for {roots} plants");
        }
    }

    #[test]
    fn wild_crops_cover_every_climate() {
        for (t, m) in [(0.2, 0.5), (0.5, 0.3), (0.5, 0.6), (0.8, 0.8), (0.8, 0.3), (0.8, 0.5)] {
            for pick in [0.0, 0.5, 0.99] {
                let b = wild_crop(t, m, false, B::Grass, pick);
                assert!(b != B::Air, "{t} {m}");
            }
        }
        assert!(matches!(wild_crop(0.5, 0.5, true, B::Grass, 0.1), B::Cabbage | B::BeetCrop));
    }

    #[test]
    fn trees_have_trunks_and_stay_near_their_anchor() {
        let cfg = cfg(VERSION, 3);
        for kind in [Kind::Mangrove, Kind::Fruit(B::Apple), Kind::Fruit(B::Fig)] {
            let root = IVec3::new(-(1 << 20), 20, 77);
            let mut logs = 0;
            let mut fruit = 0;
            for y in 21..40 {
                for z in 70..85 {
                    for x in root.x - 7..root.x + 8 {
                        match tree_cell(&cfg, IVec3::new(x, y, z), root, kind, 12) {
                            B::Log => logs += 1,
                            B::Apple | B::Fig => fruit += 1,
                            B::Air => {}
                            _ => assert!((x - root.x).abs() <= 4 && (z - root.z).abs() <= 4, "{kind:?} {x} {z}"),
                        }
                    }
                }
            }
            assert!(logs >= 2, "{kind:?}");
            if matches!(kind, Kind::Fruit(_)) {
                assert!(fruit > 0, "{kind:?} bears no fruit");
            }
            let near = |b| (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz)))
                .any(|(dx, dz)| tree_cell(&cfg, root + IVec3::new(dx, 1, dz), root, kind, 12) == b);
            if kind == Kind::Mangrove {
                assert!(near(B::MangroveRoots), "mangrove without roots");
            }
        }
    }
}
