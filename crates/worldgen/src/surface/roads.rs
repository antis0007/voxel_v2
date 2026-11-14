//! Generator 55 regional roads. Every ruin door, homestead door, district
//! ramp foot and one flat waypoint per region is a node; each node links to
//! its nearest two neighbours within `REACH`, so the road web continues past
//! the spawn trails. Links are owned by one 192 m region and cached per
//! region; a voxel reads one merged neighbourhood, never nine plans.
use super::{Trail, Vec3, WorldSampleConfig, routes};
use crate::{hash2, sample_column};
use crate::BlockId as B;
use engine_core::fast_hash::FastMap;
use std::cell::RefCell;
use std::rc::Rc;

pub(super) const VERSION: u32 = crate::ruins::VERSION;
const REACH: f32 = 300.0;
const DISTRICT_CELL: f32 = 1024.0;

type Nodes = Rc<Vec<Vec3>>;
type Links = Rc<Vec<Trail>>;
pub(super) struct Neighbourhood {
    pub trails: Vec<Trail>,
}

thread_local! {
    static NODES: RefCell<(Option<WorldSampleConfig>, FastMap<(i32, i32), Nodes>)> = RefCell::new((None, FastMap::default()));
    static LINKS: RefCell<(Option<WorldSampleConfig>, FastMap<(i32, i32), Links>)> = RefCell::new((None, FastMap::default()));
    static KEPT: RefCell<(Option<WorldSampleConfig>, FastMap<(i32, i32), Links>)> = RefCell::new((None, FastMap::default()));
    static HOODS: RefCell<(Option<WorldSampleConfig>, FastMap<(i32, i32), Rc<Neighbourhood>>)> = RefCell::new((None, FastMap::default()));
    static LAST: RefCell<Option<(WorldSampleConfig, (i32, i32), Rc<Neighbourhood>)>> = const { RefCell::new(None) };
}

pub(super) fn cached<K: Eq + std::hash::Hash, T: Clone>(
    cell: &'static std::thread::LocalKey<RefCell<(Option<WorldSampleConfig>, FastMap<K, T>)>>,
    cfg: &WorldSampleConfig,
    key: K,
    cap: usize,
    build: impl FnOnce() -> T,
) -> T {
    let hit = cell.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != Some(*cfg) {
            *c = (Some(*cfg), FastMap::default());
        }
        c.1.get(&key).cloned()
    });
    if let Some(hit) = hit {
        return hit;
    }
    let built = build();
    cell.with(|c| {
        let mut c = c.borrow_mut();
        if c.1.len() >= cap {
            c.1.clear();
        }
        c.1.insert(key, built.clone());
    });
    built
}

fn region_of(cfg: &WorldSampleConfig, p: Vec3) -> (i32, i32) {
    let span = crate::sites::region_span(cfg);
    ((p.x / span).round() as i32, (p.z / span).round() as i32)
}

/// A flat, dry, unclaimed spot in the region where roads can meet. Places
/// are sparse on hilly ground; without it most regions would carry no road.
fn waypoint(cfg: &WorldSampleConfig, rx: i32, rz: i32, x0: f32, z0: f32, span: f32) -> Option<Vec3> {
    let ground = |x: f32, z: f32| cfg.world_y(sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height + 1);
    for k in 0..8 {
        let x = x0 + span * (0.15 + 0.7 * hash2(rx, rz, cfg.seed.wrapping_add(7700 + k)));
        let z = z0 + span * (0.15 + 0.7 * hash2(rx, rz, cfg.seed.wrapping_add(7800 + k)));
        if x.hypot(z) < 40.0 {
            continue;
        }
        let h = ground(x, z);
        if h <= 15.0 {
            continue;
        }
        let relief = [(-6.0, -6.0), (6.0, -6.0), (-6.0, 6.0), (6.0, 6.0), (0.0, 7.0), (0.0, -7.0), (7.0, 0.0), (-7.0, 0.0)]
            .iter()
            .map(|(dx, dz)| (ground(x + dx, z + dz) - h).abs())
            .fold(0.0, f32::max);
        if relief > 2.5
            || crate::valley::reserves(cfg, x, z)
            || crate::amberroot::reserves(cfg, x, z)
            || crate::undercrown::reserved(cfg, Vec3::new(x, h, z))
            || crate::ruins::reserves(cfg, x, z, 12.0)
            || crate::sites::trail_obstructed(cfg, x, z)
        {
            continue;
        }
        return Some(Vec3::new(x, h, z));
    }
    None
}

/// Road endpoints whose region is (rx, rz).
fn nodes(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Nodes {
    cached(&NODES, cfg, (rx, rz), 4096, || {
        let span = crate::sites::region_span(cfg);
        let (x0, z0) = ((rx as f32 - 0.5) * span, (rz as f32 - 0.5) * span);
        let (x1, z1) = (x0 + span, z0 + span);
        let inside = |p: &Vec3| p.x >= x0 && p.x < x1 && p.z >= z0 && p.z < z1;
        let mut out = Vec::new();
        out.extend(waypoint(cfg, rx, rz, x0, z0, span));
        out.extend(crate::sites::region_door(cfg, rx, rz).filter(inside));
        out.extend(crate::sites::homestead_doors(cfg, x0, z0, x1, z1));
        let (ix0, ix1) = (((x0 - 80.0) / DISTRICT_CELL).round() as i32, ((x1 + 80.0) / DISTRICT_CELL).round() as i32);
        let (iz0, iz1) = (((z0 - 80.0) / DISTRICT_CELL).round() as i32, ((z1 + 80.0) / DISTRICT_CELL).round() as i32);
        for iz in iz0..=iz1 {
            for ix in ix0..=ix1 {
                out.extend(crate::ruins::ramp_feet(cfg, ix, iz).into_iter().filter(inside));
            }
        }
        Rc::new(out)
    })
}

fn before(a: Vec3, b: Vec3) -> bool {
    (a.x, a.z) < (b.x, b.z)
}

/// Each node's nearest two neighbours within reach, judged from its own 3x3.
fn wanted(cfg: &WorldSampleConfig, a: Vec3) -> Vec<Vec3> {
    let (ax, az) = region_of(cfg, a);
    let mut near: Vec<(f32, Vec3)> = Vec::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            for &b in nodes(cfg, ax + dx, az + dz).iter() {
                if b == a {
                    continue;
                }
                let d = (b.x - a.x).hypot(b.z - a.z);
                if d <= REACH {
                    near.push((d, b));
                }
            }
        }
    }
    near.sort_by(|p, q| p.0.total_cmp(&q.0));
    near.into_iter().take(2).map(|(_, b)| b).collect()
}

/// True when a third node is nearer to both ends than they are to each other.
///
/// Dropping such links leaves the relative neighbourhood graph: no two roads
/// leave a node side by side, and a busy node keeps one spoke per direction.
/// Judged from the first node's 5x5 regions, which hold every node within
/// `REACH` of it, so each owner reaches the same verdict.
fn shadowed(cfg: &WorldSampleConfig, a: Vec3, b: Vec3) -> bool {
    let flat = |p: Vec3, q: Vec3| (p.x - q.x).hypot(p.z - q.z);
    let span = flat(a, b);
    let (ax, az) = region_of(cfg, a);
    (-2..=2).any(|dz| {
        (-2..=2).any(|dx| {
            nodes(cfg, ax + dx, az + dz).iter().any(|&c| c != a && c != b && flat(a, c) < span && flat(c, b) < span)
        })
    })
}

/// Links owned by region (rx, rz): pairs whose lexicographically first node is here.
fn links(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Links {
    cached(&LINKS, cfg, (rx, rz), 1024, || {
        let mut pairs: Vec<(Vec3, Vec3)> = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                for &a in nodes(cfg, rx + dx, rz + dz).iter() {
                    for b in wanted(cfg, a) {
                        let (first, second) = if before(a, b) { (a, b) } else { (b, a) };
                        if region_of(cfg, first) == (rx, rz) && !pairs.contains(&(first, second)) {
                            pairs.push((first, second));
                        }
                    }
                }
            }
        }
        if cfg.generator_version >= super::TRAIL_SURVEY_VERSION {
            pairs.retain(|&(a, b)| !shadowed(cfg, a, b));
        }
        let mut out = Vec::new();
        for (k, (a, b)) in pairs.into_iter().enumerate() {
            let salt = 55_000u32.wrapping_add(k as u32).wrapping_add((rx as u32).wrapping_mul(97) ^ (rz as u32).wrapping_mul(1009));
            if let Some(route) = routes::link(cfg, a, b, salt) {
                out.extend(route);
            }
        }
        Rc::new(out)
    })
}

/// Smallest angle between two roads leaving one node, below which the longer is dropped.
const MIN_SPOKE_COS: f32 = 0.906;

/// Start point and unit departure direction of each end of a road leg.
fn departures(t: &Trail) -> [((f32, f32), (f32, f32)); 2] {
    let reach = (6.0 / t.length().max(1.0)).min(0.5);
    let dir = |from: (f32, f32), to: (f32, f32)| {
        let (dx, dz) = (to.0 - from.0, to.1 - from.1);
        let l = dx.hypot(dz).max(1e-6);
        (dx / l, dz / l)
    };
    let (a, b) = (t.at(0.0), t.at(1.0));
    [(a, dir(a, t.at(reach))), (b, dir(b, t.at(1.0 - reach)))]
}

/// True when a shorter road leaves one of `t`'s ends in nearly the same direction.
fn crowded(t: &Trail, rivals: &[Trail]) -> bool {
    departures(t).iter().any(|&(at, d)| {
        rivals.iter().any(|r| {
            let shorter = r.length() < t.length() || (r.length() == t.length() && (r.ax, r.az, r.bx, r.bz) < (t.ax, t.az, t.bx, t.bz));
            shorter && departures(r).iter().any(|&(rat, rd)| rat == at && d.0 * rd.0 + d.1 * rd.1 > MIN_SPOKE_COS)
        })
    })
}

/// Region links without legs crowded by a shorter road from the same node.
///
/// Judged against the raw links of the 5x5 regions around the owner, which
/// hold every road that can share a node with these, so verdicts agree.
fn kept(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Links {
    if cfg.generator_version < super::TRAIL_SURVEY_VERSION {
        return links(cfg, rx, rz);
    }
    cached(&KEPT, cfg, (rx, rz), 1024, || {
        let mut rivals = Vec::new();
        for dz in -2..=2 {
            for dx in -2..=2 {
                rivals.extend(links(cfg, rx + dx, rz + dz).iter().cloned());
            }
        }
        Rc::new(links(cfg, rx, rz).iter().filter(|t| !crowded(t, &rivals)).cloned().collect())
    })
}

fn neighbourhood(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Rc<Neighbourhood> {
    cached(&HOODS, cfg, (rx, rz), 512, || {
        let span = crate::sites::region_span(cfg);
        let (x0, z0) = ((rx as f32 - 0.5) * span, (rz as f32 - 0.5) * span);
        let (x1, z1) = (x0 + span, z0 + span);
        let touches = |t: &Trail| t.max_x >= x0 && t.min_x <= x1 && t.max_z >= z0 && t.min_z <= z1;
        let mut trails = Vec::new();
        // A link reaches at most REACH plus its detours, so two regions out covers it.
        for dz in -2..=2 {
            for dx in -2..=2 {
                trails.extend(kept(cfg, rx + dx, rz + dz).iter().filter(|t| touches(t)).cloned());
            }
        }
        Rc::new(Neighbourhood { trails })
    })
}

pub(super) fn near(cfg: &WorldSampleConfig, x: f32, z: f32) -> Rc<Neighbourhood> {
    let key = region_of(cfg, Vec3::new(x, 0.0, z));
    if let Some(hit) = LAST.with(|l| l.borrow().as_ref().filter(|(c, k, _)| c == cfg && *k == key).map(|(_, _, h)| h.clone())) {
        return hit;
    }
    let hood = neighbourhood(cfg, key.0, key.1);
    LAST.with(|l| *l.borrow_mut() = Some((*cfg, key, hood.clone())));
    hood
}

/// The regional road's block at `m`, unless an older trail whose centreline
/// is `other` metres away is at least as close.
pub(super) fn block(cfg: &WorldSampleConfig, m: Vec3, natural_top: f32, other: Option<f32>) -> Option<B> {
    if !crate::ruins::enabled(cfg) {
        return None;
    }
    let hood = near(cfg, m.x, m.z);
    let best = routes::nearest(hood.trails.iter(), m)?;
    if other.is_some_and(|d| d <= best.0) {
        return None;
    }
    routes::paint(cfg, best, m, natural_top)
}

/// Road or verge within clearing distance of `m` (trees stay off it).
pub(super) fn clears(cfg: &WorldSampleConfig, m: Vec3) -> bool {
    if !crate::ruins::enabled(cfg) {
        return false;
    }
    let hood = near(cfg, m.x, m.z);
    hood.trails.iter().any(|t| {
        m.x >= t.min_x && m.x <= t.max_x && m.z >= t.min_z && m.z <= t.max_z && t.nearest(m.x, m.z).0 < t.half_width + 1.6
    })
}

pub(super) fn first_link(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Option<Trail> {
    if !crate::ruins::enabled(cfg) {
        return None;
    }
    kept(cfg, rx, rz).first().cloned()
}

