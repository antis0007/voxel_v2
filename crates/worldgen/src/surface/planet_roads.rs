//! Regional roads join the built gateways of Anchor districts on the same face.
use super::{B, Trail, Vec3, WorldSampleConfig, roads::cached, routes};
use crate::{WorldType, ruins};
use engine_core::{cube_sphere::atlas, fast_hash::FastMap};
use glam::{IVec3, Vec2};
use std::{cell::RefCell, rc::Rc};

type Cell = (i32, i32);
type Links = Rc<Vec<Trail>>;
type LinkCache = RefCell<(Option<WorldSampleConfig>, FastMap<Cell, Links>)>;
type EdgeCache = RefCell<(
    Option<WorldSampleConfig>,
    FastMap<(Cell, Cell), Option<Rc<Trail>>>,
)>;
const REACH: f32 = 1_650.0;
const HALF_WIDTH: f32 = 2.7;
const PATH_REACH: f32 = 170.0;

thread_local! {
    static EDGES: EdgeCache = RefCell::new((None, FastMap::default()));
    static LINKS: LinkCache = RefCell::new((None, FastMap::default()));
    static NEARBY: LinkCache = RefCell::new((None, FastMap::default()));
    static LAST: RefCell<Option<(WorldSampleConfig, Cell, Links)>> = const { RefCell::new(None) };
    static SPURS: LinkCache = RefCell::new((None, FastMap::default()));
    static LOCAL: LinkCache = RefCell::new((None, FastMap::default()));
    static LAST_LOCAL: RefCell<Option<(WorldSampleConfig, Cell, Links)>> = const { RefCell::new(None) };
}

pub(super) fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Planet
        && cfg.generator_version >= engine_voxel::ANCHOR_ROAD_GENERATOR
}

fn cell_centre((x, z): Cell) -> Vec2 {
    Vec2::new(x as f32, z as f32) * ruins::CELL
}

fn neighbours((x, z): Cell) -> impl Iterator<Item = Cell> {
    (-1..=1).flat_map(move |dz| (-1..=1).map(move |dx| (x + dx, z + dz)))
}

fn face(p: Vec2) -> Option<engine_core::cube_sphere::Face> {
    let (face, local) = atlas::split(Vec3::new(p.x, 0.0, p.y).as_dvec3());
    (local.x.abs() < atlas::HALF_M && local.z.abs() < atlas::HALF_M).then_some(face)
}

fn build_link(
    cfg: &WorldSampleConfig,
    a_cell: Cell,
    a: Vec2,
    b_cell: Cell,
    b: Vec2,
) -> Option<Trail> {
    let gates_a = ruins::ramp_feet(cfg, a_cell.0, a_cell.1);
    let gates_b = ruins::ramp_feet(cfg, b_cell.0, b_cell.1);
    let mut pairs = Vec::new();
    for &from in &gates_a {
        for &to in &gates_b {
            let p = Vec2::new(from.x, from.z);
            let q = Vec2::new(to.x, to.z);
            let dir = q - p;
            // Use the gates on the facing sides instead of routing around the city wall.
            if dir.dot(p - a) > 0.0 && dir.dot(b - q) > 0.0 && dir.length() <= REACH {
                pairs.push((dir.length_squared(), from, to));
            }
        }
    }
    pairs.sort_by(|p, q| p.0.total_cmp(&q.0));
    let lo = cell_centre(a_cell).min(cell_centre(b_cell)) - Vec2::splat(ruins::CELL * 0.5);
    let hi = cell_centre(a_cell).max(cell_centre(b_cell)) + Vec2::splat(ruins::CELL * 0.5);
    let on_face = face(a)?;
    let in_bounds = |t: &Trail| {
        // Bound ownership to the cells between the two cities so adjacent samples
        // see exactly the same road, even when the survey takes a detour.
        t.min_x >= lo.x
            && t.max_x <= hi.x
            && t.min_z >= lo.y
            && t.max_z <= hi.y
            && face(Vec2::new(t.min_x, t.min_z)) == Some(on_face)
            && face(Vec2::new(t.max_x, t.max_z)) == Some(on_face)
    };
    for survey in [false, true] {
        for &(_, from, to) in pairs.iter().take(if survey { 2 } else { 4 }) {
            if let Some(t) = routes::anchor_road(cfg, from, to, HALF_WIDTH, survey)
                && in_bounds(&t)
            {
                return Some(t);
            }
        }
    }
    None
}

fn edge(cfg: &WorldSampleConfig, a: (Cell, Vec2), b: (Cell, Vec2)) -> Option<Rc<Trail>> {
    let (a, b) = if a.0 < b.0 { (a, b) } else { (b, a) };
    cached(&EDGES, cfg, (a.0, b.0), 1024, || {
        build_link(cfg, a.0, a.1, b.0, b.1).map(Rc::new)
    })
}

fn links(cfg: &WorldSampleConfig, key: Cell) -> Links {
    cached(&LINKS, cfg, key, 512, || {
        let Some(a) = ruins::road_district(cfg, key.0, key.1) else {
            return Rc::default();
        };
        let Some(face_a) = face(a) else {
            return Rc::default();
        };
        let nearby: Vec<_> = neighbours(key)
            .filter(|&cell| cell != key)
            .filter_map(|cell| {
                let p = ruins::road_district(cfg, cell.0, cell.1)?;
                (face(p) == Some(face_a)).then_some((cell, p))
            })
            .collect();
        let mut roads = Vec::new();
        for &(cell, b) in &nearby {
            if cell <= key {
                continue;
            }
            let distance = a.distance(b);
            // Prune a shortcut only when both shorter connections can actually be built.
            if distance > REACH
                || nearby.iter().any(|&(other, c)| {
                    other != cell
                        && c.distance(a) < distance
                        && c.distance(b) < distance
                        && (other.0 - cell.0).abs() <= 1
                        && (other.1 - cell.1).abs() <= 1
                        && edge(cfg, (key, a), (other, c)).is_some()
                        && edge(cfg, (other, c), (cell, b)).is_some()
                })
            {
                continue;
            }
            roads.extend(edge(cfg, (key, a), (cell, b)).as_deref().cloned());
        }
        Rc::new(roads)
    })
}

fn near(cfg: &WorldSampleConfig, x: f32, z: f32) -> Links {
    let key = (
        (x / ruins::CELL).round() as i32,
        (z / ruins::CELL).round() as i32,
    );
    if let Some(hit) = LAST.with(|last| {
        last.borrow()
            .as_ref()
            .filter(|(c, k, _)| c == cfg && *k == key)
            .map(|(_, _, roads)| roads.clone())
    }) {
        return hit;
    }
    let roads = cached(&NEARBY, cfg, key, 256, || {
        let centre = cell_centre(key);
        let half = ruins::CELL * 0.5;
        let mut roads = Vec::new();
        for cell in neighbours(key) {
            roads.extend(
                links(cfg, cell)
                    .iter()
                    .filter(|t| {
                        t.min_x <= centre.x + half
                            && t.max_x >= centre.x - half
                            && t.min_z <= centre.y + half
                            && t.max_z >= centre.y - half
                    })
                    .cloned(),
            );
        }
        Rc::new(roads)
    });
    LAST.with(|last| *last.borrow_mut() = Some((*cfg, key, roads.clone())));
    roads
}

fn spurs(cfg: &WorldSampleConfig, key: Cell, span: f32) -> Links {
    cached(&SPURS, cfg, key, 1024, || {
        let centre = Vec2::new(key.0 as f32, key.1 as f32) * span;
        let lo = centre - Vec2::splat(span * 0.5);
        let hi = centre + Vec2::splat(span * 0.5);
        let ruin = crate::sites::region_door(cfg, key.0, key.1);
        let homes = crate::sites::homestead_doors(cfg, lo.x, lo.y, hi.x, hi.y);
        if ruin.is_none() && homes.is_empty() {
            return Rc::default();
        }
        let public_places: Vec<_> = neighbours(key)
            .filter_map(|cell| Some((cell, crate::sites::region_door(cfg, cell.0, cell.1)?)))
            .collect();
        let mut paths = Vec::new();
        if key == (0, 0)
            && let Some(door) = ruin
        {
            let column = crate::sample_column(cfg, 0, 0);
            let clearing = Vec3::new(0.0, cfg.world_y(column.height + 1), 0.0);
            paths.extend(footpath(cfg, clearing, door));
        }
        for (from, is_ruin) in ruin
            .into_iter()
            .map(|p| (p, true))
            .chain(homes.into_iter().map(|p| (p, false)))
        {
            let Some(on_face) = face(Vec2::new(from.x, from.z)) else {
                continue;
            };
            let roads = near(cfg, from.x, from.z);
            let mut targets = Vec::new();
            for road in roads.iter() {
                let (distance, t) = road.nearest(from.x, from.z);
                if distance <= PATH_REACH
                    && !road.bridges.iter().any(|s| (s.start..=s.end).contains(&t))
                {
                    let (x, z) = road.at(t);
                    targets.push((distance, Vec3::new(x, routes::height_at(road, t), z)));
                }
            }
            // A district can still serve its immediate hinterland when terrain
            // prevents a longer road to the next district.
            let first_x = ((from.x - PATH_REACH) / ruins::CELL).round() as i32;
            let last_x = ((from.x + PATH_REACH) / ruins::CELL).round() as i32;
            let first_z = ((from.z - PATH_REACH) / ruins::CELL).round() as i32;
            let last_z = ((from.z + PATH_REACH) / ruins::CELL).round() as i32;
            for z in first_z..=last_z {
                for x in first_x..=last_x {
                    for gate in ruins::ramp_feet(cfg, x, z) {
                        let distance = (from.x - gate.x).hypot(from.z - gate.z);
                        if distance <= PATH_REACH {
                            targets.push((distance + 20.0, gate));
                        }
                    }
                }
            }
            for &(cell, place) in &public_places {
                // Ruin paths have a single owner. A homestead can use the ruin
                // in its own cell as well as a neighbouring one.
                if is_ruin && cell >= key {
                    continue;
                }
                let distance = (from.x - place.x).hypot(from.z - place.z);
                if distance <= PATH_REACH {
                    targets.push((distance + 40.0, place));
                }
            }
            targets.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, to) in targets.into_iter().take(3) {
                let length = (from.x - to.x).hypot(from.z - to.z);
                if length < 8.0 || face(Vec2::new(to.x, to.z)) != Some(on_face) {
                    continue;
                }
                let fits = |t: &Trail| {
                    t.min_x >= lo.x - span
                        && t.max_x <= hi.x + span
                        && t.min_z >= lo.y - span
                        && t.max_z <= hi.y + span
                        && face(Vec2::new(t.min_x, t.min_z)) == Some(on_face)
                        && face(Vec2::new(t.max_x, t.max_z)) == Some(on_face)
                };
                if let Some(path) = footpath(cfg, from, to).filter(fits) {
                    paths.push(path);
                    break;
                }
            }
        }
        Rc::new(paths)
    })
}

fn footpath(cfg: &WorldSampleConfig, from: Vec3, to: Vec3) -> Option<Trail> {
    let width = routes::FOOTPATH_HALF_WIDTH;
    routes::anchor_road(cfg, from, to, width, false)
        .or_else(|| routes::anchor_road(cfg, from, to, width, true))
}

fn paths(cfg: &WorldSampleConfig, x: f32, z: f32) -> Links {
    let span = crate::sites::region_span(cfg);
    let key = ((x / span).round() as i32, (z / span).round() as i32);
    if let Some(hit) = LAST_LOCAL.with(|last| {
        last.borrow()
            .as_ref()
            .filter(|(c, k, _)| c == cfg && *k == key)
            .map(|(_, _, paths)| paths.clone())
    }) {
        return hit;
    }
    let paths = cached(&LOCAL, cfg, key, 512, || {
        let x = key.0 as f32 * span;
        let z = key.1 as f32 * span;
        let half = span * 0.5;
        let mut paths = Vec::new();
        for cell in neighbours(key) {
            paths.extend(
                spurs(cfg, cell, span)
                    .iter()
                    .filter(|t| {
                        t.min_x <= x + half
                            && t.max_x >= x - half
                            && t.min_z <= z + half
                            && t.max_z >= z - half
                    })
                    .cloned(),
            );
        }
        Rc::new(paths)
    });
    LAST_LOCAL.with(|last| *last.borrow_mut() = Some((*cfg, key, paths.clone())));
    paths
}

pub(super) fn first_link(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Option<Trail> {
    links(cfg, (rx, rz)).first().cloned()
}

pub(super) fn first_bridge(cfg: &WorldSampleConfig) -> Option<Trail> {
    let spawn = crate::planetary::spawn_point(cfg).as_vec3();
    near(cfg, spawn.x, spawn.z)
        .iter()
        .chain(paths(cfg, spawn.x, spawn.z).iter())
        .find(|t| !t.bridges.is_empty())
        .cloned()
}

pub(super) fn block(cfg: &WorldSampleConfig, p: IVec3, terrain_top: f32) -> Option<B> {
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    if !(terrain_top - 5.5..=terrain_top + 18.0).contains(&m.y) {
        return None;
    }
    let roads = near(cfg, m.x, m.z);
    routes::block(cfg, &roads, m, terrain_top)
        .or_else(|| routes::block(cfg, &paths(cfg, m.x, m.z), m, terrain_top))
}

pub(super) fn clears(cfg: &WorldSampleConfig, m: Vec3) -> bool {
    let clear = |links: &[Trail]| {
        routes::nearest(links.iter(), m).is_some_and(|(d, _, t)| d <= t.half_width + 1.6)
    };
    clear(&near(cfg, m.x, m.z)) || clear(&paths(cfg, m.x, m.z))
}
