//! Composition of one district in its local frame: megastructure first, then
//! port furniture or office lots, the downed carriage, towers, planters and rubble.
use super::hull::Hull;
use super::line::Line;
use super::mega::{self, Mega};
use super::port;
use super::shapes::{Part, Shape, roll};
use super::tower::{FLOOR, Facade, Tower};
use super::urban::Urban;
use super::{CacheLoot, Kind};
use crate::BlockId as B;
use glam::{Vec2, Vec3};

pub(super) struct Layout {
    pub parts: Vec<Part>,
    /// Loot points (local metres) and the cache block each holds.
    pub caches: Vec<(Vec3, B)>,
    pub role_caches: Vec<(Vec3, CacheLoot)>,
    pub cut: Option<super::landmarks::ServiceCut>,
    pub terminal: Option<Vec2>,
    pub sentries: Vec<Vec3>,
}

pub(super) struct Ground {
    half: Vec2,
    blocked: Vec<(Vec2, Vec2)>,
    urban: Option<Urban>,
}

impl Ground {
    pub(super) fn free(&self, lo: Vec2, hi: Vec2, margin: f32) -> bool {
        let inside = lo.x > -self.half.x + 4.0 && lo.y > -self.half.y + 4.0 && hi.x < self.half.x - 4.0 && hi.y < self.half.y - 4.0;
        inside && self.urban.is_none_or(|urban| urban.parcel(lo, hi))
            && !self.blocked.iter().any(|(a, b)| lo.x < b.x + margin && hi.x > a.x - margin && lo.y < b.y + margin && hi.y > a.y - margin)
    }

    pub(super) fn reserve(&mut self, lo: Vec2, hi: Vec2) {
        self.blocked.push((lo, hi));
    }

    pub(super) fn elevation(&self, z: f32) -> f32 {
        self.urban.map_or(0.0, |urban| urban.elevation(z))
    }

    pub(super) fn terraced(&self) -> bool {
        self.urban.is_some_and(Urban::terraced)
    }
}

/// Local z of the transit viaduct's centreline.
pub(super) fn corridor(half: Vec2, city: bool) -> f32 {
    if city { -28.0 } else { -half.y * 0.3 }
}

pub(super) fn layout(kind: Kind, seed: u32, half: Vec2, v: f32, urban: Option<Urban>, line: Option<Line>, version: u32) -> Layout {
    let city = urban.is_some_and(|u| u.city);
    let terraced = urban.is_some_and(Urban::terraced);
    let landmarks = version >= super::RUIN_ROUTE_VERSION;
    let routed = version >= super::EXPLORATION_VERSION;
    // Historical alignment chooses the viaduct; actual connections come from
    // stations and access corridors, never from this orientation hint.
    let spine = line.is_none()
        && match kind {
            Kind::Offices => roll(seed, 60, 0) < 0.8,
            Kind::Port => roll(seed, 60, 0) < 0.2,
        };
    let Mega { mut parts, mut blocked, tent, decks, access } = if spine {
        mega::a_frame(seed, half, urban.is_some(), city, terraced)
    } else {
        mega::viaduct(seed, half, corridor(half, city), urban.is_some(), city, terraced, line)
    };
    if let Some(urban) = urban {
        for entrance in access {
            // Both current station kits enter the level central ward. Join
            // its nearest built north/south street before placing any lots.
            let candidates = if urban.city {
                (0..super::urban::CITY_X.len()).map(|i| urban.street_x(i, entrance.y)).collect::<Vec<_>>()
            } else { vec![0.0, urban.side_streets[0], urban.side_streets[1]] };
            let street_x = candidates.into_iter().min_by(|a, b|
                (a - entrance.x).abs().total_cmp(&(b - entrance.x).abs())).unwrap_or(0.0);
            let path = crate::public_works::AccessWay {
                entrance, street: Vec2::new(street_x, entrance.y), half_width: 2.0,
            };
            let (lo, hi) = path.bounds();
            blocked.push((lo, hi));
            parts.push(mega::slab(
                Vec3::new((lo.x + hi.x) * 0.5, -0.5, (lo.y + hi.y) * 0.5),
                Vec3::new((hi.x - lo.x) * 0.5, 0.5, (hi.y - lo.y) * 0.5), B::GlazedTiles, false));
        }
    }
    let mut ground = Ground { half, blocked, urban };
    let mut caches = Vec::new();
    let mut sentries: Vec<Vec3> = decks.clone();
    let cache = |i: i32, salt: i32| if roll(seed, i, salt) < 0.4 { B::SurveyCache } else { B::SalvageCrate };
    for (i, d) in decks.iter().enumerate() {
        caches.push((*d + Vec3::new(3.0, 0.0, 2.0), cache(i as i32, 70)));
    }
    if let Some(urban) = urban {
        let mut plan = Layout { parts, caches, role_caches: Vec::new(), cut: None, terminal: None, sentries };
        if city {
            plan.role_caches.extend(plan.caches.drain(..).map(|(spot, _)| (spot, CacheLoot::Dispatch)));
            if landmarks {
                super::landmarks::place(kind, seed, v, urban, &mut ground, &mut plan, routed);
            }
            super::city::populate(kind, seed, v, urban, &mut ground, tent.as_ref(), &mut plan, landmarks, routed);
        } else {
            place_blocks(kind, seed, v, urban, &mut ground, tent.as_ref(), &mut plan, routed);
        }
        return plan;
    }
    if kind == Kind::Port {
        place_port(seed, &mut ground, &mut parts, &mut caches, &mut sentries);
    }
    let crash = roll(seed, 61, 0) < if kind == Kind::Port { 0.75 } else { 0.4 };
    if crash {
        place_hull(seed, v, &mut ground, &mut parts, &mut caches, &mut sentries);
    }
    // Office lots on a 26 m grid; each tower fits its lot and the A-frame tent.
    let lots = (half / 26.0).floor().as_ivec2();
    let mut towers = 0;
    for gz in -lots.y..=lots.y {
        for gx in -lots.x..=lots.x {
            let r = roll(seed, gx * 31 + 7, gz * 17 + 3);
            let wanted = if kind == Kind::Offices { 0.8 } else { 0.45 };
            if r > wanted || towers >= 16 {
                continue;
            }
            let c = Vec2::new(gx as f32 * 26.0, gz as f32 * 26.0) + (Vec2::new(roll(seed, gx, gz + 90), roll(seed, gx + 90, gz)) - 0.5) * 6.0;
            let th = Vec2::new(5.0 + roll(seed, gx, gz + 91) * 5.0, 5.0 + roll(seed, gx + 91, gz) * 5.0);
            let (lo, hi) = (c - th, c + th);
            if !ground.free(lo, hi, 4.0) {
                continue;
            }
            let cap = tent.as_ref().map_or(f32::INFINITY, |t| t.max_top(lo - Vec2::splat(3.0), hi + Vec2::splat(3.0)));
            let wish = FLOOR * (4.0 + (roll(seed, gx + 5, gz + 5).powi(2) * 14.0).floor());
            let height = (wish.min(cap) / FLOOR).floor() * FLOOR;
            if height < FLOOR * 3.0 {
                continue;
            }
            let ruin = roll(seed, gx + 11, gz + 11).powf(0.7);
            let leans = roll(seed, gx + 12, gz + 12) < 0.15 && tent.is_none();
            let lean = if leans { Vec2::new(roll(seed, gx, gz + 13) - 0.5, roll(seed, gx + 13, gz) - 0.5) * 0.08 } else { Vec2::ZERO };
            let facade = match (roll(seed, gx + 14, gz + 14) * 3.0) as u32 { 0 => Facade::Glass, 1 => Facade::Plate, _ => Facade::Punched };
            let doors = std::array::from_fn(|k| roll(seed, gx * 4 + k as i32, gz + 15) < 0.5 || k == 0);
            let tower = Tower { c, half: th, height, lean, ruin, facade, doors, seed: seed ^ ((gx as u32) << 8) ^ gz as u32, rebuilt: false };
            for (k, spot) in tower.floor_spots(v).into_iter().enumerate() {
                caches.push((spot, cache(towers * 16 + k as i32, 71)));
                if k % 2 == 0 {
                    sentries.push(spot + Vec3::new(-th.x * 0.6, 0.0, 0.0));
                }
            }
            ground.blocked.push((lo - Vec2::splat(2.0), hi + Vec2::splat(2.0)));
            parts.push(Part::new(Shape::Tower(Box::new(tower))));
            towers += 1;
        }
    }
    place_dressing(seed, &mut ground, &mut parts, &mut sentries);
    Layout { parts, caches, role_caches: Vec::new(), cut: None, terminal: None, sentries }
}

fn place_blocks(kind: Kind, seed: u32, v: f32, urban: Urban, ground: &mut Ground, tent: Option<&mega::Tent>, plan: &mut Layout, routed: bool) {
    let cols = [(-urban.half.x + urban.side_streets[0]) * 0.5,
        urban.side_streets[0] * 0.5, urban.side_streets[1] * 0.5,
        (urban.half.x + urban.side_streets[1]) * 0.5];
    let rows = [(urban.back_streets[0] - urban.half.y) * 0.5, -4.0,
        (urban.cross_street + urban.back_streets[1]) * 0.5 + 1.0,
        (urban.back_streets[1] + urban.half.y) * 0.5];
    let landmark_side = if roll(seed, 71, 6) < 0.5 { 1 } else { 2 };
    for (row, z) in rows.into_iter().enumerate() {
        for (col, x) in cols.into_iter().enumerate() {
            let id = (row * 4 + col) as i32;
            let roll = |salt| roll(seed, id, salt);
            let c = Vec2::new(x + (roll(116) - 0.5) * 6.0, z);
            let frontage = if row == 3 { -1.0 } else { 1.0 };
            let kit = match (kind, row, col) {
                (Kind::Port, 2, _) if col == landmark_side => Some(port::pad(c, 10.5)),
                (Kind::Port, 2, _) => Some(port::cargo_yard(c, seed ^ id as u32)),
                (Kind::Port, 3, 1 | 2) if tent.is_some() => Some(port::cargo_yard(c, seed ^ id as u32)),
                (Kind::Port, 1, 0 | 3) => Some(port::freight_shed(c, seed ^ id as u32, frontage, true)),
                (Kind::Offices, 2, 1 | 2) => Some(port::freight_shed(c, seed ^ id as u32, frontage, false)),
                (_, 1, _) if col == landmark_side => Some(port::substation(c)),
                (_, 0 | 3, _) if roll(118) < 0.30 => Some(port::yard(c, seed ^ id as u32, routed)),
                _ => None,
            };
            if let Some(kit) = kit
                && ground.free(kit.footprint.0, kit.footprint.1, 0.8)
                && tent.is_none_or(|t| {
                    let cap = t.max_top(kit.footprint.0, kit.footprint.1);
                    kit.parts.iter().all(|p| p.hi.y < cap)
                })
            {
                plan.caches.extend(kit.spots.iter().step_by(2).map(|s| (*s, B::SalvageCrate)));
                add(ground, &mut plan.parts, &mut plan.sentries, kit);
                continue;
            }

            let th = Vec2::new(7.5 + roll(119) * 5.0, 5.5 + roll(120) * 3.0);
            let (lo, hi) = (c - th, c + th);
            if !ground.free(lo, hi, 1.0) {
                continue;
            }
            let cap = tent.map_or(f32::INFINITY, |t| t.max_top(lo - Vec2::splat(2.0), hi + Vec2::splat(2.0)));
            let floors = match (kind, row, col == landmark_side) {
                (Kind::Offices, 0 | 3, true) => 16.0 + roll(122) * 7.0,
                (Kind::Offices, 1 | 2, _) => 5.0 + roll(122) * 7.0,
                (Kind::Offices, _, _) => 4.0 + roll(122) * 9.0,
                (Kind::Port, 0 | 3, true) => 9.0 + roll(122) * 5.0,
                _ => 2.0 + roll(122) * 4.0,
            };
            let height = ((floors * FLOOR).min(cap) / FLOOR).floor() * FLOOR;
            if height < FLOOR * 2.0 {
                continue;
            }
            let facade = match (kind, row, roll(123)) {
                (Kind::Port, _, r) if r < 0.65 => Facade::Plate,
                (Kind::Offices, 3, r) if r < 0.65 => Facade::Punched,
                (_, _, r) if r < 0.45 => Facade::Glass,
                _ => Facade::Punched,
            };
            let tower = Tower {
                c, half: th, height, lean: Vec2::ZERO,
                ruin: roll(124).powi(2), facade, doors: [false, false, frontage > 0.0, frontage < 0.0],
                seed: seed ^ (id as u32).wrapping_mul(0x3c79_ac49), rebuilt: true,
            };
            for (k, spot) in tower.floor_spots(v).into_iter().enumerate() {
                let cache = if k % 3 == 0 { B::SurveyCache } else { B::SalvageCrate };
                plan.caches.push((spot, cache));
                if k % 2 == 0 {
                    plan.sentries.push(spot + Vec3::new(-th.x * 0.5, 0.0, 0.0));
                }
            }
            ground.blocked.push((lo, hi));
            plan.parts.push(Part::new(Shape::Tower(Box::new(tower))));
        }
    }
    // Smaller street-front units fill the long gaps beside the main buildings.
    for side in [-1.0, 1.0] {
        for (i, z) in [urban.back_streets[0] - 10.0, urban.cross_street - 12.0,
            urban.cross_street + 11.5, urban.back_streets[1] + 11.0].into_iter().enumerate() {
            let c = Vec2::new(side * 13.5, z);
            let kit = port::street_unit(c, side, seed ^ (i as u32 + 1) ^ u32::from(side < 0.0) * 0x2ab, kind == Kind::Port);
            if ground.free(kit.footprint.0, kit.footprint.1, 0.5)
                && tent.is_none_or(|t| t.max_top(kit.footprint.0, kit.footprint.1) > 6.5)
            {
                add(ground, &mut plan.parts, &mut plan.sentries, kit);
            }
        }
    }
    if !routed {
        place_dressing(seed, ground, &mut plan.parts, &mut plan.sentries);
    }
}

fn place_port(seed: u32, ground: &mut Ground, parts: &mut Vec<Part>, caches: &mut Vec<(Vec3, B)>, sentries: &mut Vec<Vec3>) {
    let half = ground.half;
    let pads = 2 + (roll(seed, 62, 0) * 2.0) as i32;
    for i in 0..pads {
        let r = 15.0 + roll(seed, i, 63) * 5.0;
        let c = Vec2::new(-half.x * 0.6 + i as f32 * half.x * 0.6, half.y * 0.45);
        let kit = port::pad(c, r);
        if !ground.free(kit.footprint.0, kit.footprint.1, 3.0) {
            continue;
        }
        add(ground, parts, sentries, kit);
        let g = c + Vec2::new(r + 6.0, -r * 0.6);
        let gantry = port::gantry(g, 30.0 + roll(seed, i, 64) * 14.0, c, seed ^ i as u32);
        if ground.free(gantry.footprint.0, gantry.footprint.1, 1.0) {
            for (k, s) in gantry.spots.iter().enumerate() {
                caches.push((*s, if k % 2 == 0 { B::SurveyCache } else { B::SalvageCrate }));
            }
            add(ground, parts, sentries, gantry);
        }
    }
    let hangar = port::hangar(Vec2::new(half.x * 0.62, half.y * 0.05), seed);
    if ground.free(hangar.footprint.0, hangar.footprint.1, 2.0) {
        for (k, s) in hangar.spots.iter().enumerate() {
            caches.push((*s, if k % 3 == 0 { B::SurveyCache } else { B::SalvageCrate }));
        }
        add(ground, parts, sentries, hangar);
    }
}

fn add(ground: &mut Ground, parts: &mut Vec<Part>, sentries: &mut Vec<Vec3>, kit: port::Kit) {
    ground.blocked.push(kit.footprint);
    sentries.extend(kit.spots.iter().step_by(2).copied());
    parts.extend(kit.parts);
}

fn place_hull(seed: u32, v: f32, ground: &mut Ground, parts: &mut Vec<Part>, caches: &mut Vec<(Vec3, B)>, sentries: &mut Vec<Vec3>) {
    let half = ground.half;
    for attempt in 0..8 {
        let len = 56.0 + roll(seed, attempt, 65) * 26.0;
        let radius = 7.0 + roll(seed, attempt, 66) * 2.5;
        let yaw = roll(seed, attempt, 67) * std::f32::consts::TAU;
        let dir = Vec2::new(yaw.cos(), yaw.sin());
        let mid = Vec2::new((roll(seed, attempt, 68) - 0.5) * half.x, (roll(seed, attempt, 69) - 0.5) * half.y);
        let (a2, b2) = (mid - dir * len * 0.5, mid + dir * len * 0.5);
        let lo = a2.min(b2) - Vec2::splat(radius);
        let hi = a2.max(b2) + Vec2::splat(radius);
        if !ground.free(lo, hi, 2.0) {
            continue;
        }
        // Nose ploughed in: the centreline dips below the deck toward the nose.
        let a = Vec3::new(a2.x, radius * 0.45, a2.y);
        let b = Vec3::new(b2.x, -radius * 0.25, b2.y);
        let hull = Hull { a, b, radius, roll: (roll(seed, attempt, 70) - 0.5) * 0.7, seed, routed: false };
        for (k, spot) in hull.deck_spots(v).into_iter().enumerate() {
            caches.push((spot, if k % 2 == 0 { B::SurveyCache } else { B::SalvageCrate }));
            sentries.push(spot);
        }
        // Torn-up deck slabs heaped ahead of the nose.
        for j in 0..6 {
            let at = b2 + dir * (radius * 0.6 + j as f32 * 2.5) + Vec2::new(-dir.y, dir.x) * (roll(seed, j, 72) - 0.5) * radius * 2.0;
            let mat = if j % 2 == 0 { B::Concrete } else { B::Gravel };
            parts.push(Part::new(Shape::Blob { c: Vec3::new(at.x, 0.6, at.y), half: Vec3::new(2.8, 1.6 + j as f32 * 0.2, 2.4), mat, seed: seed ^ (j as u32 * 77) }));
        }
        // Engine pods and a fin sheared at the root.
        let side = Vec3::new(-dir.y, 0.0, dir.x);
        for s in [-1.0, 1.0] {
            let root = a + side * s * radius * 0.9 + Vec3::Y * radius * 0.2;
            parts.push(Part::new(Shape::Beam { a: root, b: root + Vec3::new(dir.x, 0.0, dir.y) * 12.0, r: radius * 0.32, mat: B::RustedSteel, moss: false }));
        }
        parts.push(Part::new(Shape::Sheet { a: a + Vec3::Y * radius * 0.7, b: a + Vec3::Y * (radius * 1.6) - Vec3::new(dir.x, 0.0, dir.y) * 4.0, half_width: 0.4, thick: 3.0, mat: B::SteelPlate }));
        parts.push(Part::new(Shape::Hull(Box::new(hull))));
        ground.blocked.push((lo, hi));
        return;
    }
}

fn place_dressing(seed: u32, ground: &mut Ground, parts: &mut Vec<Part>, sentries: &mut Vec<Vec3>) {
    let half = ground.half;
    let mut placed = 0;
    for i in 0..40 {
        let c = Vec2::new((roll(seed, i, 80) - 0.5) * 2.0 * half.x, (roll(seed, i, 81) - 0.5) * 2.0 * half.y);
        if !ground.free(c - Vec2::splat(4.0), c + Vec2::splat(4.0), 1.0) {
            continue;
        }
        if placed % 3 == 2 {
            // Rubble heap.
            let mat = if i % 2 == 0 { B::Concrete } else { B::Gravel };
            parts.push(Part::new(Shape::Blob { c: Vec3::new(c.x, 0.4, c.y), half: Vec3::new(3.2, 1.4, 2.6), mat, seed: seed ^ i as u32 }));
            sentries.push(Vec3::new(c.x + 4.0, 0.5, c.y));
        } else {
            // A planter whose tree has outgrown it.
            let h = 6.0 + roll(seed, i, 82) * 5.0;
            parts.push(Part::new(Shape::Slab { c: Vec3::new(c.x, 0.6, c.y), half: Vec3::new(2.2, 0.6, 2.2), mat: B::Concrete, moss: true }));
            parts.push(Part::new(Shape::Beam { a: Vec3::new(c.x, 0.0, c.y), b: Vec3::new(c.x + 0.6, h, c.y - 0.4), r: 0.35, mat: B::Log, moss: false }));
            parts.push(Part::new(Shape::Blob { c: Vec3::new(c.x + 0.6, h + 0.8, c.y - 0.4), half: Vec3::new(3.8, 2.4, 3.6), mat: B::Leaves, seed: seed ^ (i as u32 + 500) }));
        }
        ground.blocked.push((c - Vec2::splat(4.0), c + Vec2::splat(4.0)));
        placed += 1;
        if placed >= 14 {
            break;
        }
    }
}
