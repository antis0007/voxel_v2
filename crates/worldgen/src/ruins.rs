//! Generator 55: abandoned Anchor districts. A sunken concrete deck carries a
//! megastructure, gutted towers, a carriage port or office lots and a downed
//! carriage. Everything is authored in metres and sampled per voxel through an
//! 8 m bucket grid, so every voxel scale shares one district.
use crate::{BlockId as B, CachedProfiles, Profiles, WATER_LEVEL_METERS, WorldSampleConfig, hash2, sample_column};
use engine_core::fast_hash::FastMap;
use glam::{IVec3, Vec2, Vec3};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) mod cargo;
mod city;
mod hull;
mod layout;
mod landmarks;
pub mod line;
mod mega;
mod mega_detail;
pub mod outskirts;
mod port;
mod sample;
mod survey;
use survey::survey;
mod shapes;
mod tower;
mod urban;
/// Dormant until `CURRENT_GENERATOR_VERSION` reaches it, like rivers (53) and ice (54).
pub const VERSION: u32 = engine_voxel::RECOVERED_SYSTEMS_GENERATOR;
pub const REBUILT_VERSION: u32 = engine_voxel::REBUILT_RUINS_GENERATOR;
pub const CITY_VERSION: u32 = engine_voxel::ANCHOR_CITY_GENERATOR;
pub const RUIN_ROUTE_VERSION: u32 = engine_voxel::ANCHOR_RUIN_ROUTE_GENERATOR;
pub const TERRACE_VERSION: u32 = engine_voxel::ANCHOR_TERRACE_GENERATOR;
pub const EXPLORATION_VERSION: u32 = engine_voxel::ANCHOR_EXPLORATION_GENERATOR;

#[derive(Clone, Copy)]
pub enum CacheLoot {
    Household,
    Clinic,
    Dispatch,
    Freight,
    Workshop,
    Relay,
}

impl CacheLoot {
    fn block(self) -> B {
        match self {
            Self::Clinic | Self::Household => B::CultivatorCache,
            Self::Dispatch | Self::Relay => B::SurveyCache,
            Self::Freight | Self::Workshop => B::SalvageCrate,
        }
    }
}
pub(crate) fn enabled(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION && matches!(cfg.world_type, crate::WorldType::Hills | crate::WorldType::Ridged | crate::WorldType::Planet)
}
pub(crate) const CELL: f32 = 1024.0;
const BUCKET: f32 = 8.0;
const CORNER: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Port,
    Offices,
}

pub struct District {
    pub centre: Vec2,
    pub datum: f32,
    pub kind: Kind,
    turn: u32,
    half: Vec2,
    urban: Option<urban::Urban>,
    approaches: Vec<urban::Approach>,
    outskirts: Option<outskirts::Outskirts>,
    lo: Vec2,
    hi: Vec2,
    grid_w: i32,
    grid_h: i32,
    buckets: Vec<Vec<u16>>,
    bucket_tops: Vec<f32>,
    parts: Vec<shapes::Part>,
    cut: Option<landmarks::ServiceCut>,
    terminal: Option<Vec2>,
    caches: FastMap<IVec3, B>,
    loot: FastMap<IVec3, CacheLoot>,
    /// Sentry posts in world metres.
    pub sentries: Vec<Vec3>,
    cell: (i32, i32),
    /// World-space xz bounds of every part, and the highest part top in metres.
    world_lo: Vec2,
    world_hi: Vec2,
    top: f32,
    seed: u32,
    /// Historical transit alignment and physical failure inside this district.
    line: Option<line::Line>,
}

/// Encounter ids: x = cell x * 256 + post, y = POST_ID_Y + cell z, z = 5.
pub const POST_ID_Y: i32 = 20_000;

impl District {
    /// Positions from the same station kit that produces the voxel geometry.
    /// Consumers must still survey track continuity before offering travel.
    pub fn stations(&self) -> Vec<crate::public_works::Station> {
        self.parts.iter().filter_map(|part| {
            let shapes::Shape::ServiceStair { c, height } = &part.shape else { return None };
            Some(crate::public_works::Station {
                entrance: self.world(*c + Vec3::new(-5.0, 0.02, -3.6)),
                platform: self.world(*c + Vec3::new(4.0, *height + 0.15, -3.0)),
            })
        }).collect()
    }
}

type Slot = Option<Rc<District>>;
thread_local! {
    static PLANS: RefCell<(Option<WorldSampleConfig>, FastMap<(i32, i32), Slot>)> = RefCell::new((None, FastMap::default()));
    static LAST: RefCell<Option<(WorldSampleConfig, (i32, i32), Slot)>> = const { RefCell::new(None) };
}

fn rotate(turn: u32, x: f32, z: f32) -> Vec2 {
    match turn {
        1 => Vec2::new(-z, x),
        2 => Vec2::new(-x, -z),
        3 => Vec2::new(z, -x),
        _ => Vec2::new(x, z),
    }
}

fn district_seed(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> u32 {
    cfg.seed ^ (ix as u32).wrapping_mul(0x27d4_eb2d) ^ (iz as u32).wrapping_mul(0x1656_67b1) ^ 0xa4c0
}

/// Centre of the district cell before the terrain survey, without planning it.
fn candidate(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<Vec2> {
    let seed = district_seed(cfg, ix, iz);
    // hash2 mirrors under negation; mix a third term so (-x,-z) rolls differently.
    if crate::hash3(ix, iz, ix.wrapping_mul(7) ^ iz, seed) > 0.62 {
        return None;
    }
    let jitter = |salt: u32| (hash2(ix, iz, seed.wrapping_add(salt)) - 0.5) * CELL * 0.4;
    let centre = Vec2::new(ix as f32 * CELL + jitter(1), iz as f32 * CELL + jitter(2));
    (centre.length() >= 520.0).then_some(centre)
}

/// Coarse district footprints for surveys run before the Planet is published.
/// The full ruins plan samples that Planet, so its footprint is unavailable here.
pub(crate) fn trail_keepouts(cfg: &WorldSampleConfig, centre: Vec2) -> Vec<Vec2> {
    if !enabled(cfg) { return Vec::new(); }
    let cell = (centre / CELL).round().as_ivec2();
    (-2..=2).flat_map(|z| (-2..=2).filter_map(move |x| {
        candidate(cfg, cell.x + x, cell.y + z)
            .filter(|p| p.distance_squared(centre) < 1500.0_f32.powi(2))
    })).collect()
}

fn plan(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Slot {
    let seed = district_seed(cfg, ix, iz);
    let centre = candidate(cfg, ix, iz)?;
    let kind = if crate::hash3(ix, iz, 3, seed) < 0.5 { Kind::Port } else { Kind::Offices };
    let destination = line::active(cfg).then(|| line::destination(cfg, ix, iz, centre)).flatten();
    let turn = match destination {
        Some(target) => line::turn_toward(centre, target),
        None => (hash2(ix, iz, seed.wrapping_add(4)) * 4.0) as u32 % 4,
    };
    let compact = match kind {
        Kind::Port => Vec2::new(120.0, 92.0),
        Kind::Offices => Vec2::new(112.0, 96.0),
    };
    let surveyed = |half| {
        // A line's bearing is its reason; terrain relief no longer turns it.
        let turn = if cfg.generator_version >= REBUILT_VERSION && destination.is_none() {
            survey::aligned_turn(cfg, centre, turn, half)
        } else {
            turn
        };
        survey(cfg, centre, turn, half).map(|datum| (half, turn, datum))
    };
    let expanded = (cfg.generator_version >= CITY_VERSION)
        .then(|| surveyed(urban::CITY_HALF)).flatten();
    let (half, turn, datum) = expanded.or_else(|| surveyed(compact))?;
    let v = cfg.voxel_meters();
    let urban = (cfg.generator_version >= REBUILT_VERSION).then(|| {
        let urban = urban::Urban::new(half, seed, expanded.is_some());
        if urban.city && cfg.generator_version >= TERRACE_VERSION {
            urban.survey_wards(cfg, centre, turn, datum, seed)
        } else {
            urban
        }
    });
    let mut approaches = Vec::new();
    if let Some(urban) = urban {
        for side in [-1.0, 1.0] {
            let end = Vec2::new(0.0, side * half.y);
            approaches.push(urban::Approach::survey(cfg, centre, turn, datum, end, Vec2::Y * side, urban.elevation(end.y)));
        }
        if urban.city {
            for side in [-1.0, 1.0] {
                let end = Vec2::new(side * half.x, urban::CROSS_AVENUE);
                approaches.push(urban::Approach::survey(cfg, centre, turn, datum, end, Vec2::X * side, urban.elevation(end.y)));
            }
        }
    }
    let city = urban.is_some_and(|u| u.city);
    let line = destination.filter(|_| urban.is_some()).map(|toward| {
        line::failure(cfg, toward, centre, turn, datum, layout::corridor(half, city), mega_detail::gap_window(city))
    });
    let mut plan = layout::layout(kind, seed, half, v, urban, line, cfg.generator_version);
    let mut lo = -half - Vec2::splat(4.0);
    let mut hi = half + Vec2::splat(4.0);
    for part in &mut plan.parts {
        if urban.is_some() {
            part.fit_voxels(v);
        }
        lo = lo.min(Vec2::new(part.lo.x, part.lo.z));
        hi = hi.max(Vec2::new(part.hi.x, part.hi.z));
    }
    // Ramps reach out past the kerb on the long edges.
    lo.y -= 60.0;
    hi.y += 60.0;
    if expanded.is_some() {
        lo.x = lo.x.min(-half.x - 62.0);
        hi.x = hi.x.max(half.x + 62.0);
    }
    let grid_w = ((hi.x - lo.x) / BUCKET).ceil() as i32;
    let grid_h = ((hi.y - lo.y) / BUCKET).ceil() as i32;
    let mut buckets = vec![Vec::new(); (grid_w * grid_h) as usize];
    let mut bucket_tops = vec![f32::NEG_INFINITY; buckets.len()];
    for (i, part) in plan.parts.iter().enumerate() {
        let g0 = ((Vec2::new(part.lo.x, part.lo.z) - lo) / BUCKET).floor().as_ivec2().max(glam::IVec2::ZERO);
        let g1 = ((Vec2::new(part.hi.x, part.hi.z) - lo) / BUCKET).floor().as_ivec2().min(glam::IVec2::new(grid_w - 1, grid_h - 1));
        for gz in g0.y..=g1.y {
            for gx in g0.x..=g1.x {
                let index = (gz * grid_w + gx) as usize;
                buckets[index].push(i as u16);
                bucket_tops[index] = bucket_tops[index].max(part.hi.y);
            }
        }
    }
    let top = datum + plan.parts.iter().map(|p| p.hi.y).fold(0.0, f32::max);
    let outskirts = urban.filter(|u| u.city && cfg.generator_version >= outskirts::VERSION).map(|urban| {
        let ramps: Vec<_> = approaches.iter().map(|ramp| ramp.span()).collect();
        outskirts::Outskirts::plan(urban, seed, &ramps, |lo, hi| {
            plan.parts.iter().any(|p| p.lo.x < hi.x && p.hi.x > lo.x && p.lo.z < hi.y && p.hi.z > lo.y)
        })
    });
    let corners = [Vec2::new(lo.x, lo.y), Vec2::new(hi.x, lo.y), Vec2::new(lo.x, hi.y), Vec2::new(hi.x, hi.y)];
    let (mut world_lo, mut world_hi) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
    for c in corners {
        let w = centre + rotate(turn, c.x, c.y);
        world_lo = world_lo.min(w);
        world_hi = world_hi.max(w);
    }
    let mut district = District {
        centre, datum, kind, turn, half, urban, approaches, outskirts, lo, hi, grid_w, grid_h, buckets, bucket_tops,
        parts: plan.parts, cut: plan.cut, terminal: plan.terminal, caches: FastMap::default(), loot: FastMap::default(), sentries: Vec::new(), cell: (ix, iz),
        world_lo, world_hi, top, seed, line,
    };
    for (spot, block) in plan.caches {
        if let Some(at) = district.settle(district.world(spot), v) {
            district.caches.insert(at, block);
        }
    }
    for (spot, loot) in plan.role_caches {
        if let Some(at) = district.settle(district.world(spot), v) {
            district.caches.insert(at, loot.block());
            district.loot.insert(at, loot);
        }
    }
    district.sentries = plan.sentries.iter().take(255).map(|s| district.world(*s)).collect();
    Some(Rc::new(district))
}

pub fn cache_loot(cfg: &WorldSampleConfig, p: IVec3, block: B) -> Option<CacheLoot> {
    if crate::civilization::archive(cfg, p, block).is_some() {
        return Some(CacheLoot::Dispatch);
    }
    if cfg.generator_version < CITY_VERSION || !enabled(cfg) {
        return None;
    }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    if let Some(loot) = district_at(cfg, m.x, m.z).and_then(|d| d.loot.get(&p).copied()) {
        return (loot.block() == block).then_some(loot);
    }
    sprawl(cfg, m.x, m.z).into_iter().flatten().find_map(|d| {
        let q = d.local(m);
        d.outskirts.as_ref()?.loot(d.site(cfg), p, Vec2::new(q.x, q.z))
    }).filter(|loot| loot.block() == block)
}


fn district_at(cfg: &WorldSampleConfig, x: f32, z: f32) -> Slot {
    let key = ((x / CELL).round() as i32, (z / CELL).round() as i32);
    if let Some(hit) = LAST.with(|l| l.borrow().as_ref().filter(|(c, k, _)| c == cfg && *k == key).map(|(_, _, s)| s.clone())) {
        return hit;
    }
    let cached = PLANS.with(|p| {
        let mut p = p.borrow_mut();
        if p.0 != Some(*cfg) {
            *p = (Some(*cfg), FastMap::default());
        }
        p.1.get(&key).cloned()
    });
    let slot = cached.unwrap_or_else(|| {
        let built = plan(cfg, key.0, key.1);
        PLANS.with(|p| {
            let mut p = p.borrow_mut();
            // A 4 km `near` query touches 81 cells; a smaller cap replans every call.
            if p.1.len() >= 512 {
                p.1.clear();
            }
            p.1.insert(key, built.clone());
        });
        built
    });
    LAST.with(|l| *l.borrow_mut() = Some((*cfg, key, slot.clone())));
    slot
}

/// The district's block at `p`, if it owns that cell.
pub fn block(cfg: &WorldSampleConfig, p: IVec3, m: Vec3, terrain_top: f32) -> Option<B> {
    if !enabled(cfg) {
        return None;
    }
    if let Some(block) = district_at(cfg, m.x, m.z).and_then(|d| d.sample(p, m, terrain_top, cfg.voxel_meters())) {
        return Some(block);
    }
    // Outskirts sit on the ground: only the band around the terrain top can hold them.
    if cfg.generator_version < outskirts::VERSION || m.y < terrain_top - 4.0 || m.y > terrain_top + 8.0 {
        return None;
    }
    sprawl(cfg, m.x, m.z).into_iter().flatten().find_map(|d| d.outskirt(cfg, p, m, terrain_top))
}

/// Farthest a district's sprawl reaches from its cell centre along one axis:
/// centre jitter, the long half of a city deck and the sprawl itself.
const SPRAWL_SPAN: f32 = CELL * 0.2 + 220.0 + outskirts::REACH;

/// Districts whose outskirts can reach (x, z): this cell's, and a neighbour's
/// where its sprawl crosses the cell border.
fn sprawl(cfg: &WorldSampleConfig, x: f32, z: f32) -> [Slot; 4] {
    let key = ((x / CELL).round() as i32, (z / CELL).round() as i32);
    let off = Vec2::new(x - key.0 as f32 * CELL, z - key.1 as f32 * CELL);
    let reach = SPRAWL_SPAN - CELL * 0.5;
    let step = |o: f32| if o.abs() > CELL * 0.5 - reach { o.signum() as i32 } else { 0 };
    let (sx, sz) = (step(off.x), step(off.y));
    let at = |dx: i32, dz: i32| {
        candidate(cfg, key.0 + dx, key.1 + dz)?;
        district_at(cfg, (key.0 + dx) as f32 * CELL, (key.1 + dz) as f32 * CELL).filter(|d| d.outskirts.is_some())
    };
    [
        at(0, 0),
        if sx != 0 { at(sx, 0) } else { None },
        if sz != 0 { at(0, sz) } else { None },
        if sx != 0 && sz != 0 { at(sx, sz) } else { None },
    ]
}

impl District {
    fn site<'a>(&self, cfg: &'a WorldSampleConfig) -> outskirts::Site<'a> {
        outskirts::Site { cfg, centre: self.centre, turn: self.turn, datum: self.datum }
    }

    /// The outskirt block at world metres `m`, outside the deck and its ramps.
    fn outskirt(&self, cfg: &WorldSampleConfig, p: IVec3, m: Vec3, terrain_top: f32) -> Option<B> {
        let outskirts = self.outskirts.as_ref()?;
        let q = self.local(m);
        if self.inside(q) > -6.0 || self.on_approach(q, 3.0) {
            return None;
        }
        outskirts.sample(self.site(cfg), p, q, terrain_top - self.datum, self.seed)
    }

    fn sprawl_clears(&self, x: f32, z: f32, reach: f32) -> bool {
        let q = self.local(Vec3::new(x, self.datum, z));
        self.outskirts.as_ref().is_some_and(|o| self.inside(q) < 0.0 && o.clears(Vec2::new(q.x, q.z), reach))
    }
}

/// True within `margin` metres of a deck, or anywhere under its structures.
pub fn reserves(cfg: &WorldSampleConfig, x: f32, z: f32, margin: f32) -> bool {
    if !enabled(cfg) {
        return false;
    }
    let Some(d) = district_at(cfg, x, z) else { return false };
    let q = d.local(Vec3::new(x, d.datum, z));
    if d.urban.is_some() {
        return d.inside(q) > -margin || d.on_approach(q, margin)
            || d.bucket(q).is_some_and(|bucket| bucket.iter().any(|&i| d.parts[i as usize].contains_xz(q.x, q.z, margin)));
    }
    d.inside(q) > -margin || (q.x > d.lo.x && q.z > d.lo.y + 60.0 && q.x < d.hi.x && q.z < d.hi.y - 60.0)
}

/// Trees and bushes stay off the deck; planters supply its own.
pub fn clears_vegetation(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    if !enabled(cfg) {
        return false;
    }
    district_at(cfg, x, z).is_some_and(|d| {
        let q = d.local(Vec3::new(x, d.datum, z));
        d.inside(q) > -3.0 || d.on_approach(q, 1.0)
    }) || (cfg.generator_version >= outskirts::VERSION
        && sprawl(cfg, x, z).into_iter().flatten().any(|d| d.sprawl_clears(x, z, 0.5)))
}

/// Evaluate a tree as a whole so the city sampler cannot cut off its trunk.
pub(crate) fn tree_conflicts(cfg: &WorldSampleConfig, x: f32, z: f32, reach: f32) -> bool {
    cfg.generator_version >= EXPLORATION_VERSION && enabled(cfg)
        && district_at(cfg, x, z).is_some_and(|d| {
            let q = d.local(Vec3::new(x, d.datum, z));
            d.inside(q) > -reach - 3.0 || d.on_approach(q, reach + 1.0)
        })
        || (cfg.generator_version >= outskirts::VERSION && enabled(cfg)
            && sprawl(cfg, x, z).into_iter().flatten().any(|d| d.sprawl_clears(x, z, reach)))
}

pub fn location(cfg: &WorldSampleConfig, m: Vec3) -> Option<&'static str> {
    if !enabled(cfg) {
        return None;
    }
    let d = district_at(cfg, m.x, m.z)?;
    (d.inside(d.local(m)) > -20.0).then_some(match d.kind {
        Kind::Port => "Anchor carriage port",
        Kind::Offices => "Anchor office district",
    })
}

/// Sentry posts within `radius` metres of `m` as encounter anchors:
/// (record id, point, lead kind | menace << 8). Every other post is manned.
pub fn sentries(cfg: &WorldSampleConfig, m: Vec3, radius: f32) -> Vec<(IVec3, Vec3, u32)> {
    if !enabled(cfg) {
        return Vec::new();
    }
    let Some(d) = district_at(cfg, m.x, m.z) else { return Vec::new() };
    d.sentries
        .iter()
        .enumerate()
        .step_by(2)
        .filter(|(_, s)| s.distance(m) < radius)
        .map(|(k, s)| {
            let id = IVec3::new(d.cell.0 * 256 + k as i32, POST_ID_Y + d.cell.1, 5);
            let lead = if k % 3 == 2 { 17 } else { 16 };
            let menace = if s.y > d.datum + 10.0 { 2 } else { 1 };
            (id, *s, lead | (menace << 8))
        })
        .collect()
}

/// Where each ramp meets natural ground, in world metres (y = ground), for roads.
pub fn ramp_feet(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Vec<Vec3> {
    if !enabled(cfg) {
        return Vec::new();
    }
    let Some(d) = district_at(cfg, ix as f32 * CELL, iz as f32 * CELL) else { return Vec::new() };
    let ground = |q: Vec3| {
        let w = d.world(q);
        cfg.world_y(sample_column(cfg, cfg.blocks_for_height(w.x), cfg.blocks_for_height(w.z)).height + 1)
    };
    let mut feet = Vec::new();
    if !d.approaches.is_empty() {
        for ramp in &d.approaches {
            let q = ramp.foot();
            let mut foot = d.world(q);
            foot.y = ground(q);
            feet.push(foot);
        }
        return feet;
    }
    for side in [-1.0, 1.0] {
        let kerb = Vec3::new(0.0, 0.0, side * d.half.y);
        let sign = (ground(kerb) - d.datum).signum();
        let mut foot = None;
        for out in 1..=60 {
            let q = kerb + Vec3::new(0.0, 0.0, side * out as f32);
            let surface = d.datum + sign * out as f32 * 0.55;
            let g = ground(q);
            if (g - surface).abs() <= 1.0 || (g - surface) * sign < 0.0 {
                foot = Some(d.world(Vec3::new(0.0, g - d.datum, q.z + side * 3.0)));
                break;
            }
        }
        if let Some(mut f) = foot {
            f.y = cfg.world_y(sample_column(cfg, cfg.blocks_for_height(f.x), cfg.blocks_for_height(f.z)).height + 1);
            feet.push(f);
        }
    }
    feet
}

pub(crate) fn road_district(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<Vec2> {
    let district = district_at(cfg, ix as f32 * CELL, iz as f32 * CELL)?;
    Some(district.centre)
}

/// Highest structure voxel over the column span `min..=max` (voxel x, z), so
/// chunk residency reaches the deck and tower tops above the tree margin.
/// Major tower silhouettes share the placed district's orientation and broken crown.
pub fn visit_skylines(
    cfg: &WorldSampleConfig,
    center: Vec2,
    radius: f32,
    cancel: &std::sync::atomic::AtomicBool,
    mut emit: impl FnMut([Vec3; 9], f32, B, B),
) {
    if !enabled(cfg)
        || (cfg.world_type == crate::WorldType::Planet && cfg.generator_version < REBUILT_VERSION)
    {
        return;
    }
    let lo = ((center - radius) / CELL).round().as_ivec2();
    let hi = ((center + radius) / CELL).round().as_ivec2();
    for z in lo.y..=hi.y {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) { return; }
        for x in lo.x..=hi.x {
            let Some(d) = district_at(cfg, x as f32 * CELL, z as f32 * CELL) else { continue };
            if center.distance_squared(center.clamp(d.world_lo, d.world_hi)) > radius * radius {
                continue;
            }
            for part in &d.parts {
                let (roof, wall, cap) = match &part.shape {
                    shapes::Shape::Tower(tower) => {
                        let ring = [(-1., -1.), (0., -1.), (1., -1.), (1., 0.), (1., 1.),
                            (0., 1.), (-1., 1.), (-1., 0.), (0., 0.)];
                        let roof = ring.map(|(x, z)| {
                            let offset = Vec2::new(x, z) * tower.half;
                            let p = tower.c + offset;
                            Vec3::new(p.x, tower.top(offset), p.y)
                        });
                        (roof, B::WeatheredBricks, B::WeatheredBricks)
                    }
                    shapes::Shape::CityWing(wing) => wing.skyline(),
                    _ => continue,
                };
                emit(roof.map(|p| d.world(p + Vec3::Y * part.elevation)),
                    d.datum + part.elevation, wall, cap);
            }
        }
    }
}

pub fn top_near(cfg: &WorldSampleConfig, min: (i32, i32), max: (i32, i32)) -> Option<i32> {
    if !enabled(cfg) {
        return None;
    }
    let v = cfg.voxel_meters();
    let (lo, hi) = (Vec2::new(min.0 as f32, min.1 as f32) * v, Vec2::new(max.0 as f32 + 1.0, max.1 as f32 + 1.0) * v);
    let mut best: Option<i32> = None;
    // A span can touch two cells; check the cell of each corner.
    let mut visited = [None; 4];
    for (i, corner) in [lo, hi, Vec2::new(lo.x, hi.y), Vec2::new(hi.x, lo.y)].into_iter().enumerate() {
        let Some(d) = district_at(cfg, corner.x, corner.y) else { continue };
        if visited[..i].contains(&Some(d.cell)) {
            continue;
        }
        visited[i] = Some(d.cell);
        if lo.x <= d.world_hi.x && hi.x >= d.world_lo.x && lo.y <= d.world_hi.y && hi.y >= d.world_lo.y {
            let height = if d.urban.is_some() { d.highest_in_span(lo, hi) } else { d.top };
            if !height.is_finite() {
                continue;
            }
            let top = cfg.blocks_for_height(height) + 1;
            best = Some(best.map_or(top, |b| b.max(top)));
        }
    }
    best
}

impl District {
    /// Camera poses for features present in this district, in world metres.
    pub fn review_view(&self, name: &str, version: u32) -> Option<(Vec3, Vec3)> {
        let (eye, target) = match name {
            "studio" => self.parts.iter().find_map(|part| {
                if let shapes::Shape::CityWing(wing) = &part.shape {
                    let (eye, target) = wing.studio_view()?;
                    let lift = Vec3::Y * part.elevation;
                    Some((eye + lift, target + lift))
                } else {
                    None
                }
            })?,
            "concourse" => {
                self.cut?;
                (Vec3::new(-86.0, -5.35, -4.5), Vec3::new(-13.0, 8.0, -4.0))
            }
            "descent" | "return" => self.cut?.review_stairs(name == "return")?,
            "terminal" => {
                let c = self.terminal?;
                (Vec3::new(c.x + 11.0, 2.4, c.y + 20.0), Vec3::new(c.x, 7.1, c.y))
            }
            "wreck" if self.kind == Kind::Port && version >= RUIN_ROUTE_VERSION => {
                let side = landmarks::wreck_side(self.seed);
                let height = self.urban.filter(|urban| urban.city)?.elevation(130.0);
                (Vec3::new(side * 26.0, 2.25 + height, 118.0), Vec3::new(side * 88.0, 10.0 + height, 127.0))
            }
            _ => return None,
        };
        Some((self.world(eye), self.world(target)))
    }

    fn highest_in_span(&self, lo: Vec2, hi: Vec2) -> f32 {
        let inverse = (4 - self.turn) % 4;
        let local = |p: Vec2| rotate(inverse, p.x - self.centre.x, p.y - self.centre.y);
        let (a, b) = (local(lo), local(hi));
        let g0 = ((a.min(b) - self.lo) / BUCKET).floor().as_ivec2().max(glam::IVec2::ZERO);
        let g1 = ((a.max(b) - self.lo) / BUCKET).floor().as_ivec2()
            .min(glam::IVec2::new(self.grid_w - 1, self.grid_h - 1));
        let mut top = f32::NEG_INFINITY;
        for z in g0.y..=g1.y {
            for x in g0.x..=g1.x {
                top = top.max(self.bucket_tops[(z * self.grid_w + x) as usize]);
            }
        }
        top + self.datum
    }
}

/// Every district whose centre lies within `radius` of `m`.
pub fn near(cfg: &WorldSampleConfig, m: Vec3, radius: f32) -> Vec<Rc<District>> {
    near_supported(cfg, m, radius, f32::INFINITY)
}

fn near_supported(cfg: &WorldSampleConfig, m: Vec3, radius: f32, support: f32) -> Vec<Rc<District>> {
    if !enabled(cfg) {
        return Vec::new();
    }
    let (x0, x1) = (((m.x - radius) / CELL).round() as i32, ((m.x + radius) / CELL).round() as i32);
    let (z0, z1) = (((m.z - radius) / CELL).round() as i32, ((m.z + radius) / CELL).round() as i32);
    let mut found = Vec::new();
    for iz in z0..=z1 {
        for ix in x0..=x1 {
            // Planning preserves this seed-derived centre; reject before terrain surveys.
            // Match district_at's f32 coordinate round trip, including large-index aliases.
            let sample_x = ix as f32 * CELL;
            let sample_z = iz as f32 * CELL;
            let sampled_key = ((sample_x / CELL).round() as i32, (sample_z / CELL).round() as i32);
            let Some(centre) = candidate(cfg, sampled_key.0, sampled_key.1) else { continue };
            let distance = centre.distance(Vec2::new(m.x, m.z));
            if !(distance < radius) || distance > support { continue; }
            if let Some(d) = district_at(cfg, ix as f32 * CELL, iz as f32 * CELL)
                && d.centre.distance(Vec2::new(m.x, m.z)) < radius
            {
                found.push(d);
            }
        }
    }
    found
}

#[cfg(test)]
mod pruning_tests;
