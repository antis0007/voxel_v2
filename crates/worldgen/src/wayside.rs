//! Generator 320: wayside finds. Small things a walker meets every few dozen
//! metres on a Planet world: a stone cairn with a survey cache tucked into its
//! base, an abandoned camp (ash hearth, stone ring, log seats and a salvage
//! crate), a ring of standing stones that sometimes guards an offering, and a
//! hollow fallen log with a cultivator cache in its open end.
//!
//! Each find is surveyed once in metres and lives entirely inside its own
//! 48 m cell, so a voxel reads one cached profile and chunk seams cannot split
//! a find. Caches are axis-aligned one-metre boxes snapped to the voxel grid:
//! the container pass pays loot by connected volume, so a find pays the same
//! at every voxel scale. Every find stands on dry, level, open ground and
//! keeps clear of trails, rivers, caves, trees, settlements, the freight
//! market, ruins, dungeons and surveyor sites.
use crate::{
    BlockId as B, CachedProfiles, ColumnSample, Profiles, WorldSampleConfig, WorldType, cave_carves,
    civilization, dungeon, hash2, planet_freight, planetary, ruins, sample_column, sites,
    steady_water_column, surface, surveyor, value_noise2,
};
use engine_core::fast_hash::FastMap;
use glam::{IVec3, Vec2, Vec3};
use std::cell::RefCell;
use std::f32::consts::{PI, TAU};

pub const WAYSIDE_GENERATOR: u32 = 320;

/// One find at most per cell, metres.
const CELL_M: f32 = 48.0;
/// A find's centre stays this far inside its cell; every footprint is smaller.
const MARGIN_M: f32 = 8.0;
/// Tallest find above its floor, plus the most relief a footprint may span.
const REACH_M: f32 = 4.6;
/// Share of cells that roll a find before the ground is checked.
const ADMIT: f32 = 0.55;
/// A cairn points at a site door between these distances, metres.
const WAYMARK_MIN_M: f32 = 20.0;
const WAYMARK_MAX_M: f32 = 170.0;
/// No settlement centre within this distance of a find, metres.
const SETTLEMENT_CLEAR_M: f32 = 80.0;

pub(crate) fn enabled(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Planet && crate::features::legacy_preview(cfg.generator_version, WAYSIDE_GENERATOR)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Cairn,
    Camp,
    Stones,
    Stash,
}

#[derive(Clone, Copy, Debug)]
struct Find {
    kind: Kind,
    /// On a voxel centre, so a narrow cairn top keeps a column of voxels.
    centre: Vec2,
    dir: Vec2,
    /// Top of the lowest column under the footprint, metres; higher ground
    /// simply buries the foot of the find.
    floor: f32,
    /// Cairn and menhir height, or log length, metres.
    size: f32,
    /// Cairn base radius or log radius, metres.
    radius: f32,
    count: u32,
    /// Min corner of the one-metre cache box, metres, when the find has one.
    cache: Option<Vec3>,
    /// A cairn whose capstone arm points along `dir` at a site door.
    points: bool,
    seed: u32,
}

fn footprint(kind: Kind, size: f32) -> f32 {
    match kind {
        Kind::Cairn => 1.9,
        Kind::Camp => 3.2,
        Kind::Stones => 3.8,
        Kind::Stash => size * 0.5 + 0.8,
    }
}

fn ground_allows(kind: Kind, c: &ColumnSample) -> bool {
    let veg = c.vegetation_density;
    match kind {
        Kind::Cairn => veg < 0.8
            && matches!(c.surface, B::Grass | B::Dirt | B::GroveMoss | B::Gravel | B::Stone | B::Sand | B::SnowGrass),
        Kind::Camp => (0.08..0.75).contains(&veg) && matches!(c.surface, B::Grass | B::Dirt | B::GroveMoss | B::Gravel),
        Kind::Stones => veg < 0.5
            && matches!(c.surface, B::Grass | B::Dirt | B::GroveMoss | B::Gravel | B::Stone | B::SnowGrass),
        Kind::Stash => veg > 0.3 && matches!(c.surface, B::Grass | B::Dirt | B::GroveMoss | B::Mud),
    }
}

/// Top of the solid column under a point, metres, or `None` when the point
/// is wet, low, undercut by a cave or on a trail.
fn dry_top(cfg: &WorldSampleConfig, at: Vec2) -> Option<(f32, ColumnSample)> {
    let (x, z) = (cfg.blocks_for_height(at.x), cfg.blocks_for_height(at.y));
    let c = sample_column(cfg, x, z);
    let top = cfg.world_y(c.height + 1);
    let wet = steady_water_column(cfg, x, z).is_some()
        || CachedProfiles.river_top(cfg, x, z).is_some_and(|water| water > top - cfg.voxel_meters());
    if wet
        || c.height < cfg.water_level() + cfg.blocks_for_span(1.0)
        || cave_carves(cfg, IVec3::new(x, c.height, z), &c)
        || surface::trail_clears(cfg, Vec3::new(at.x, top, at.y))
        || planet_freight::clears(cfg, Vec3::new(at.x, top, at.y))
    {
        return None;
    }
    Some((top, c))
}

fn snap_centre(cfg: &WorldSampleConfig, at: Vec2) -> Vec2 {
    let v = cfg.voxel_meters();
    ((at / v).floor() + Vec2::splat(0.5)) * v
}

/// The one-metre cache box with its lowest corner column as the floor, so
/// no corner floats; higher corners stay buried in the ground.
fn cache_box(cfg: &WorldSampleConfig, centre: Vec2) -> Option<Vec3> {
    let v = cfg.voxel_meters();
    let min = ((centre - Vec2::splat(0.5)) / v).floor() * v;
    let mut floor = f32::INFINITY;
    for corner in [Vec2::ZERO, Vec2::new(0.9, 0.0), Vec2::new(0.0, 0.9), Vec2::splat(0.9)] {
        floor = floor.min(dry_top(cfg, min + corner + Vec2::splat(0.05))?.0);
    }
    Some(Vec3::new(min.x, floor, min.y))
}

fn survey(cfg: &WorldSampleConfig, cx: i32, cz: i32) -> Option<Find> {
    let seed = cfg.seed.wrapping_add(320_011);
    if hash2(cx, cz, seed) > ADMIT {
        return None;
    }
    let span = CELL_M - 2.0 * MARGIN_M;
    let centre = snap_centre(cfg, Vec2::new(
        cx as f32 * CELL_M + MARGIN_M + hash2(cx, cz, seed ^ 5) * span,
        cz as f32 * CELL_M + MARGIN_M + hash2(cx, cz, seed ^ 7) * span,
    ));
    // The ground picks the kind: what fits here, by weight. A cairn is
    // preferred where it can mark the way to a nearby site.
    let (centre_top, column) = dry_top(cfg, centre)?;
    let door = nearest_door(cfg, centre);
    let kind = if door.is_some() && ground_allows(Kind::Cairn, &column) && hash2(cx, cz, seed ^ 29) < 0.4 {
        Kind::Cairn
    } else {
        let options = [(Kind::Cairn, 0.14), (Kind::Camp, 0.32), (Kind::Stash, 0.30), (Kind::Stones, 0.24)];
        let total: f32 = options.iter().filter(|(k, _)| ground_allows(*k, &column)).map(|(_, w)| w).sum();
        let mut pick = hash2(cx, cz, seed ^ 3) * total;
        let mut chosen = None;
        for (k, w) in options {
            if !ground_allows(k, &column) {
                continue;
            }
            if pick < w {
                chosen = Some(k);
                break;
            }
            pick -= w;
        }
        let kind = chosen?;
        // Bare ground offers only cairns and rings; keep those rare so open
        // fell reads as sparse and a stone ring stays a landmark.
        let keep = match kind {
            Kind::Stones => 0.25,
            Kind::Cairn => 0.6,
            Kind::Camp | Kind::Stash => 1.0,
        };
        if hash2(cx, cz, seed ^ 31) >= keep {
            return None;
        }
        kind
    };
    let mut dir = Vec2::from_angle(hash2(cx, cz, seed ^ 11) * TAU);
    let v = cfg.voxel_meters();
    let (size, radius, count) = match kind {
        Kind::Cairn => (1.6 + hash2(cx, cz, seed ^ 13) * 0.8, 0.95, 0),
        Kind::Camp => (0.0, 0.0, 5 + (hash2(cx, cz, seed ^ 13) * 3.0) as u32),
        Kind::Stones => (1.9 + hash2(cx, cz, seed ^ 13) * 1.1, 0.0, 4 + (hash2(cx, cz, seed ^ 17) * 3.0) as u32),
        Kind::Stash => (3.2 + hash2(cx, cz, seed ^ 13) * 1.2, (0.55 + hash2(cx, cz, seed ^ 17) * 0.2).max(v * 0.75), 0),
    };
    let reach = footprint(kind, size);

    // Ground: the centre decides what kind of place this is; the footprint
    // ring must be dry and level enough to stand the find on.
    let mut low = centre_top;
    let mut high = centre_top;
    for i in 0..10 {
        let at = if i < 8 {
            centre + Vec2::from_angle(i as f32 * TAU / 8.0) * reach
        } else {
            centre + Vec2::from_angle(i as f32 * PI) * reach * 0.5
        };
        let (top, _) = dry_top(cfg, at)?;
        low = low.min(top);
        high = high.max(top);
    }
    let level: f32 = if kind == Kind::Stash { 1.2 } else { 1.0 };
    if high - low > level.max(v) {
        return None;
    }

    // Keep out of every authored place and away from tree trunks.
    let (x, z) = (centre.x, centre.y);
    if ruins::reserves(cfg, x, z, reach + 6.0)
        || sites::tree_reserved(cfg, x, z)
        || dungeon::reserved(cfg, Vec3::new(x, low, z))
        || surveyor::reserves(cfg, x, z)
        || planetary::trail_reserves(cfg, x, z, reach + 2.0)
        || planetary::reserves_volcanic_summit(cfg, x, z, reach)
        || !civilization::settlements_near(cfg, Vec3::new(x, low, z), SETTLEMENT_CLEAR_M).is_empty()
    {
        return None;
    }
    let tree_span = cfg.blocks_for_span(12.0);
    let (bx, bz) = (cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    let (tx, tz) = (bx.div_euclid(tree_span), bz.div_euclid(tree_span));
    for dz in -1..=1 {
        for dx in -1..=1 {
            if let Some(tree) = CachedProfiles.tree(cfg, tx + dx, tz + dz) {
                let trunk = Vec2::new(cfg.world_x(tree.anchor_x), cfg.world_z(tree.anchor_z));
                if trunk.distance(centre) < reach + 1.2 {
                    return None;
                }
            }
        }
    }

    // Cairns are waymarks: one near an authored site turns its capstone arm
    // toward that site's door, so a walker can follow it to somewhere.
    let mut points = false;
    if kind == Kind::Cairn
        && let Some(door) = door
    {
        dir = (door - centre).normalize_or(dir);
        points = true;
    }
    let item_seed = seed ^ (hash2(cx, cz, seed ^ 19) * 65535.0) as u32;
    let wants_cache = match kind {
        Kind::Cairn => hash2(cx, cz, seed ^ 23) < 0.7,
        Kind::Camp => true,
        Kind::Stones => hash2(cx, cz, seed ^ 23) < 0.4,
        Kind::Stash => hash2(cx, cz, seed ^ 23) < 0.6,
    };
    let cache = if !wants_cache {
        None
    } else {
        match kind {
            // Half tucked into the cairn's base, behind the way it points.
            Kind::Cairn => cache_box(cfg, centre - dir * 1.05),
            Kind::Camp => cache_box(cfg, centre + Vec2::from_angle(dir.to_angle() + PI * 1.6) * 2.3),
            // On the offering plinth in the middle of the ring.
            Kind::Stones => cache_box(cfg, centre).map(|b| {
                Vec3::new(b.x, low + plinth_height(v), b.z)
            }),
            // Inside the open end of the log, on the log's floor.
            Kind::Stash => cache_box(cfg, centre + dir * (size * 0.5 - 0.7)).map(|b| Vec3::new(b.x, low, b.z)),
        }
    };
    Some(Find { kind, centre, dir, floor: low, size, radius, count, cache, points, seed: item_seed })
}

/// Door of the nearest legacy site within waymark range, on the ground plane.
fn nearest_door(cfg: &WorldSampleConfig, centre: Vec2) -> Option<Vec2> {
    let span = sites::region_span(cfg);
    let (rx, rz) = ((centre.x / span).round() as i32, (centre.y / span).round() as i32);
    let mut best: Option<(f32, Vec2)> = None;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let Some(door) = sites::region_door(cfg, rx + dx, rz + dz) else { continue };
            let door = Vec2::new(door.x, door.z);
            let d = door.distance(centre);
            if (WAYMARK_MIN_M..WAYMARK_MAX_M).contains(&d) && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, door));
            }
        }
    }
    best.map(|(_, door)| door)
}

fn plinth_height(v: f32) -> f32 {
    (0.5 / v).ceil() * v
}

thread_local! {
    static FINDS: RefCell<(Option<WorldSampleConfig>, FastMap<(i32, i32), Option<Find>>)> =
        RefCell::new((None, FastMap::default()));
}

fn find(cfg: &WorldSampleConfig, cx: i32, cz: i32) -> Option<Find> {
    if let Some(hit) = FINDS.with(|f| {
        let mut f = f.borrow_mut();
        if f.0 != Some(*cfg) {
            *f = (Some(*cfg), FastMap::default());
        }
        f.1.get(&(cx, cz)).copied()
    }) {
        return hit;
    }
    let result = survey(cfg, cx, cz);
    FINDS.with(|f| {
        let mut f = f.borrow_mut();
        if f.1.len() >= 8192 {
            f.1.clear();
        }
        f.1.insert((cx, cz), result);
    });
    result
}

/// The wayside find occupying an above-ground voxel, or `Air`.
pub(crate) fn block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> B {
    if !enabled(cfg) || p.y <= column.height || p.y > column.height + cfg.blocks_for_span(REACH_M) {
        return B::Air;
    }
    let v = cfg.voxel_meters();
    let m = (p.as_vec3() + Vec3::splat(0.5)) * v;
    let Some(f) = find(cfg, (m.x / CELL_M).floor() as i32, (m.z / CELL_M).floor() as i32) else {
        return B::Air;
    };
    if let Some(c) = f.cache
        && (c.x..c.x + 1.0).contains(&m.x)
        && (c.y..c.y + 1.0).contains(&m.y)
        && (c.z..c.z + 1.0).contains(&m.z)
    {
        return match f.kind {
            Kind::Cairn | Kind::Stones => B::SurveyCache,
            Kind::Camp => B::SalvageCrate,
            Kind::Stash => B::CultivatorCache,
        };
    }
    let d = Vec2::new(m.x, m.z) - f.centre;
    let y = m.y - f.floor;
    if y < 0.0 || d.length() > footprint(f.kind, f.size) + 0.5 {
        return B::Air;
    }
    match f.kind {
        Kind::Cairn => cairn(&f, d, y, v),
        // Hearth, ring and seats are low; each rests on its own column so
        // footprint relief cannot bury them.
        Kind::Camp => camp(&f, d, m.y - cfg.world_y(column.height + 1), v),
        Kind::Stones => stones(&f, d, y, v),
        Kind::Stash => stash(&f, d, y, v),
    }
}

/// A tapering stack of courses, each a little off the last, capped in stone.
fn cairn(f: &Find, d: Vec2, y: f32, v: f32) -> B {
    // The pointer: a flat stone laid across the top course, its long end
    // reaching past the stack toward the site it marks.
    let slab = v.max(0.25);
    let arm_y = f.size - 0.5;
    if f.points && (arm_y..arm_y + slab).contains(&y) {
        let along = d.dot(f.dir);
        if (-0.3..1.6).contains(&along) && d.perp_dot(f.dir).abs() < (v * 0.5).max(0.2) {
            return B::Stone;
        }
    }
    if y >= f.size {
        return B::Air;
    }
    let course = (y / 0.5).floor();
    let lean = f.dir.perp() * (value_noise2(course * 1.7, 0.3, f.seed) - 0.5) * 0.24;
    let d = d - lean;
    let angle = d.y.atan2(d.x);
    let lump = value_noise2(angle * 1.9 + course * 5.3, course * 0.7, f.seed ^ 31) - 0.5;
    let t = y / f.size;
    let r = (f.radius * (1.0 - 0.6 * t) + lump * 0.24).max(v * 0.75);
    if d.length() > r {
        return B::Air;
    }
    if y >= f.size - 0.5 { B::Stone } else { B::Cobblestone }
}

/// A cold hearth inside a ring of stones, with two log seats on one side.
fn camp(f: &Find, d: Vec2, y: f32, v: f32) -> B {
    let r = d.length();
    if r < 0.6 && y < v.max(0.25) {
        return B::Ash;
    }
    let a0 = f.dir.to_angle();
    let ring = 0.95 + v * 0.5;
    let half = (v * 0.5).max(0.22);
    let stone_h = v.max(0.4);
    for k in 0..f.count {
        let jitter = (value_noise2(k as f32 * 3.1, 0.7, f.seed) - 0.5) * 0.5;
        let at = Vec2::from_angle(a0 + k as f32 * TAU / f.count as f32 + jitter) * ring;
        if (d - at).length() < half * 1.15 && y < stone_h {
            return B::Cobblestone;
        }
    }
    let seat_r = (v * 0.5).max(0.3);
    for j in 0..2 {
        let radial = Vec2::from_angle(a0 + PI * 0.25 + j as f32 * PI * 0.9);
        let local = d - radial * 2.2;
        let along = local.dot(radial.perp());
        let across = local.dot(radial);
        if along.abs() <= 0.9 && Vec2::new(across, y - seat_r).length() < seat_r {
            return B::Log;
        }
    }
    B::Air
}

/// Menhirs on a ring, broad faces toward the centre, with a low plinth in the
/// middle when the ring keeps an offering.
fn stones(f: &Find, d: Vec2, y: f32, v: f32) -> B {
    if f.cache.is_some() && d.x.abs().max(d.y.abs()) < 0.7 && y < plinth_height(v) {
        return B::StoneBricks;
    }
    let a0 = f.dir.to_angle();
    for k in 0..f.count {
        let jitter = (value_noise2(k as f32 * 2.3, 1.9, f.seed) - 0.5) * 0.4;
        let radial = Vec2::from_angle(a0 + k as f32 * TAU / f.count as f32 + jitter);
        let local = d - radial * 3.2;
        let height = f.size * (0.75 + 0.25 * value_noise2(k as f32 * 4.7, 0.2, f.seed ^ 7));
        let top = height - value_noise2(local.x * 2.0, local.y * 2.0, f.seed ^ k) * 0.4;
        if y >= top {
            continue;
        }
        let taper = 1.0 - 0.3 * (y / height);
        let thick = (0.28 * taper).max(v * 0.5);
        let wide = (0.42 * taper).max(v * 0.5);
        if local.dot(radial).abs() < thick && local.dot(radial.perp()).abs() < wide {
            return B::Stone;
        }
    }
    B::Air
}

/// A fallen trunk lying along `dir`; its far end is split open and hollow.
fn stash(f: &Find, d: Vec2, y: f32, v: f32) -> B {
    let along = d.dot(f.dir) + f.size * 0.5;
    let side = d.perp_dot(f.dir);
    if !(0.0..=f.size).contains(&along) {
        return B::Air;
    }
    let bark = value_noise2(along / 1.4, side / 1.1, f.seed);
    if along > f.size - bark * 0.5 {
        return B::Air;
    }
    let dist = Vec2::new(side, y - f.radius).length();
    if dist > f.radius {
        return B::Air;
    }
    let wall = (0.18_f32).max(v);
    if along > f.size - 1.3 && dist < f.radius - wall {
        return B::Air;
    }
    if y > f.radius * 1.5 && bark > 0.6 { B::GroveMoss } else { B::Log }
}

/// One surveyed find, for review tools.
#[derive(Clone, Copy, Debug)]
pub struct Census {
    pub kind: &'static str,
    pub centre: Vec2,
    pub floor_m: f32,
    pub cache: bool,
    pub waymark: bool,
    pub heading_deg: f32,
    /// The site door a waymark cairn points at.
    pub door: Option<Vec2>,
}

/// Every find whose centre lies within `radius` metres of `centre`.
pub fn census(cfg: &WorldSampleConfig, centre: Vec2, radius: f32) -> Vec<Census> {
    if !enabled(cfg) {
        return Vec::new();
    }
    let lo = ((centre - Vec2::splat(radius)) / CELL_M).floor().as_ivec2();
    let hi = ((centre + Vec2::splat(radius)) / CELL_M).floor().as_ivec2();
    let mut out = Vec::new();
    for cz in lo.y..=hi.y {
        for cx in lo.x..=hi.x {
            let Some(f) = find(cfg, cx, cz) else { continue };
            if f.centre.distance(centre) > radius {
                continue;
            }
            out.push(Census {
                kind: match f.kind {
                    Kind::Cairn => "cairn",
                    Kind::Camp => "camp",
                    Kind::Stones => "stones",
                    Kind::Stash => "stash",
                },
                centre: f.centre,
                floor_m: f.floor,
                cache: f.cache.is_some(),
                waymark: f.points,
                heading_deg: f.dir.x.atan2(f.dir.y).to_degrees().rem_euclid(360.0),
                door: if f.points { nearest_door(cfg, f.centre) } else { None },
            });
        }
    }
    out
}
