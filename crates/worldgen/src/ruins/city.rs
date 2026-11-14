//! City blocks front the street on two sides and leave a shared service lane.
use super::layout::{Ground, Layout};
use super::mega::{Tent, beam, slab};
use super::shapes::{Part, Shape, roll};
use super::urban::{CITY_HALF, CITY_X, CITY_Z, Urban};
use super::{CacheLoot, Kind, port};
use crate::BlockId as B;
use crate::architecture::{
    Palette, Roof, Windows, Wing,
    plan::{Edge, Entrance, Plan, Room, RoomRole, Side},
};
use glam::{Vec2, Vec3};

const ROWS: usize = 4;
const COLS: usize = 6;
const STOREY: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    Apartments,
    Clinic,
    Offices,
    Workshop,
    Relay,
}

impl Use {
    fn authored(self) -> Option<&'static crate::anchor_studio::Design> {
        crate::anchor_studio::city_review(match self {
            Self::Apartments => "Apartments",
            Self::Clinic => "Clinic",
            Self::Offices => "Offices",
            Self::Workshop => "Workshop",
            Self::Relay => "Relay",
        })
    }

    fn dimensions(self, seed: u32, terraced: bool) -> (f32, usize) {
        if !terraced {
            return (if self == Self::Offices { 4.8 } else { 4.0 }, ROWS);
        }
        let pick = roll(seed, 924, 1);
        match self {
            Self::Apartments if pick < 0.47 => (3.6, 3),
            Self::Apartments => (4.35, 4),
            Self::Clinic => (4.2, 4),
            Self::Offices => (4.8, if pick < 0.32 { 5 } else { 4 }),
            Self::Workshop => (4.45, if pick < 0.44 { 3 } else { 4 }),
            Self::Relay => (3.6, 3),
        }
    }

    fn roof_clearance(self) -> f32 {
        match self {
            Self::Workshop => 8.0,
            Self::Offices => 5.0,
            _ => 1.5,
        }
    }

    fn loot(self) -> CacheLoot {
        match self {
            Self::Apartments => CacheLoot::Household,
            Self::Clinic => CacheLoot::Clinic,
            Self::Offices => CacheLoot::Dispatch,
            Self::Workshop => CacheLoot::Workshop,
            Self::Relay => CacheLoot::Relay,
        }
    }

    fn plan(self, bay: f32, rows: usize) -> Plan {
        let (wall, frame, floor, roof, room) = match self {
            Self::Apartments => (
                B::WeatheredBricks,
                B::DressedStone,
                B::Planks,
                if rows == 3 { Roof::Hip } else { Roof::Flat },
                RoomRole::Living,
            ),
            Self::Clinic => (
                B::LimePlaster,
                B::Concrete,
                B::GlazedTiles,
                Roof::Flat,
                RoomRole::Living,
            ),
            Self::Offices => (
                B::Concrete,
                B::SteelPlate,
                B::DressedStone,
                Roof::Flat,
                RoomRole::Library,
            ),
            Self::Workshop => (
                B::WeatheredBricks,
                B::RustedSteel,
                B::Concrete,
                Roof::Gable,
                RoomRole::Store,
            ),
            Self::Relay => (
                B::Basalt,
                B::SteelPlate,
                B::CarvedStone,
                Roof::Flat,
                RoomRole::Workshop,
            ),
        };
        let mut rooms = Vec::new();
        let mut portals = Vec::new();
        if self == Self::Workshop {
            rooms.push(Room {
                role: RoomRole::Workshop,
                bounds: [0, 0, COLS, rows - 1],
            });
            for x in [0, 3] {
                rooms.push(Room {
                    role: room,
                    bounds: [x, rows - 1, x + 3, rows],
                });
                portals.push(Edge::Z {
                    x: x + 1,
                    z: rows - 1,
                });
            }
        } else {
            // Clinics and offices have a full-width reception before the core.
            let reception = usize::from(matches!(self, Self::Clinic | Self::Offices));
            if reception == 1 {
                rooms.push(Room {
                    role: RoomRole::Hall,
                    bounds: [0, 0, COLS, 1],
                });
                portals.extend([Edge::Z { x: 2, z: 1 }, Edge::Z { x: 3, z: 1 }]);
            }
            rooms.push(Room {
                role: RoomRole::Passage,
                bounds: [2, reception, 4, rows],
            });
            for z in reception..rows {
                rooms.push(Room {
                    role: room,
                    bounds: [0, z, 2, z + 1],
                });
                rooms.push(Room {
                    role: room,
                    bounds: [4, z, COLS, z + 1],
                });
                portals.extend([Edge::X { x: 2, z }, Edge::X { x: 4, z }]);
            }
        }
        Plan {
            bays: [COLS, rows],
            bay_metres: bay,
            height_metres: STOREY,
            rooms,
            portals,
            entrances: vec![
                Entrance {
                    side: Side::Front,
                    bay: 2,
                },
                Entrance {
                    side: Side::Back,
                    bay: 3,
                },
            ],
            mirror_x: false,
            roof,
            palette: Palette {
                wall,
                frame,
                plinth: B::WeatheredBricks,
                floor,
                roof: if roof == Roof::Hip {
                    B::RoofTiles
                } else {
                    B::SteelPlate
                },
                glass: B::WindowGlass,
            },
        }
    }
}

pub(super) struct CityWing {
    centre: Vec2,
    half: Vec2,
    facing: f32,
    purpose: Use,
    wing: Wing,
    collapse: Option<Collapse>,
    roof_clearance: f32,
    authored: bool,
}

#[derive(Clone, Copy)]
struct Collapse {
    side: f32,
    height: f32,
}

impl Collapse {
    /// The outer rooms failed around a diagonal break. The stair core survives.
    fn edge(self, p: Vec2, half: Vec2) -> Option<f32> {
        let x = (p.x - half.x) * self.side;
        let z = p.y - half.y;
        let toe = (0.52 * half.x - z * 0.14).max(half.x * 0.42);
        (x > toe).then_some((self.height + z * 0.30 - (x - toe) * 0.70).floor())
    }
}

impl CityWing {
    pub(super) fn skyline(&self) -> ([Vec3; 9], B, B) {
        let (mut roof, wall, cap) = self.wing.skyline();
        for p in &mut roof {
            let local = Vec2::new(p.x, p.z);
            if let Some(height) = self.collapse.and_then(|collapse| collapse.edge(local, self.half)) {
                p.y = p.y.min(height.max(1.0));
            }
            let world = self.centre + (local - self.half) * self.facing;
            p.x = world.x;
            p.z = world.y;
        }
        (roof, wall, cap)
    }

    pub(super) fn studio_view(&self) -> Option<(Vec3, Vec3)> {
        self.authored.then(|| {
            let target = Vec3::new(self.centre.x, self.wing.top() * 0.46, self.centre.y);
            let eye = Vec3::new(
                self.centre.x + self.half.x + 14.0,
                self.wing.top() + 14.0,
                self.centre.y - self.facing * (self.half.y + 19.0),
            );
            (eye, target)
        })
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        let r = self.half + Vec2::splat(1.1);
        let (lo, hi) = (self.centre - r, self.centre + r);
        (
            Vec3::new(lo.x, 0.0, lo.y),
            Vec3::new(hi.x, self.wing.top() + self.roof_clearance, hi.y),
        )
    }

    pub fn sample(&self, q: Vec3, v: f32) -> Option<B> {
        let local = (Vec2::new(q.x, q.z) - self.centre) * self.facing + self.half;
        let p = Vec3::new(local.x, q.y, local.y);
        if self.authored {
            return self.wing.sample(p, v, 0.0);
        }
        let breach = self
            .collapse
            .and_then(|collapse| collapse.edge(local, self.half));
        if breach.is_some_and(|top| p.y >= top) {
            return Some(B::Air);
        }
        if self.purpose == Use::Offices && (self.wing.top()..self.wing.top() + 4.8).contains(&p.y) {
            let core = local - self.half;
            let y = p.y - self.wing.top();
            if core.x.abs() < 3.2 && (core.y - 1.5).abs() < 2.5 && y < 2.8 {
                return Some(B::Concrete);
            }
            if core.x.abs() < 0.65 && (core.y - 1.5).abs() < 0.65 {
                return Some(B::RustedSteel);
            }
        }
        let b = self.wing.sample(p, v, 0.0)?;
        if b != B::Air || p.y >= self.wing.top() {
            if breach.is_some_and(|top| p.y > top - 2.0) {
                return Some(match b {
                    B::WindowGlass => B::Air,
                    B::Concrete | B::WeatheredBricks if super::shapes::grows_moss(q, 917) => {
                        B::GroveMoss
                    }
                    _ => b,
                });
            }
            return Some(b);
        }
        let x = p.x.min(self.wing.width() - p.x);
        let z = p.z.rem_euclid(self.wing.cell);
        let h = p.y.rem_euclid(STOREY) - v.max(0.4);
        if (1.0..2.4).contains(&x) && z > 0.7 && z < 2.25 && (0.0..v.max(0.55)).contains(&h) {
            return Some(match self.purpose {
                Use::Apartments => B::Bed,
                Use::Clinic if p.z >= self.wing.cell => B::Bed,
                Use::Clinic => B::GlazedTiles,
                Use::Offices => B::Planks,
                Use::Workshop => B::SteelPlate,
                Use::Relay => B::CarvedStone,
            });
        }
        if self.purpose == Use::Relay
            && (1.0..2.4).contains(&x)
            && (0.85..1.6).contains(&z)
            && (v.max(0.55)..v.max(0.55) * 2.0).contains(&h)
        {
            return Some(B::Crystal);
        }
        Some(B::Air)
    }

    fn cache(&self, floor: usize, side: f32, v: f32) -> Vec3 {
        let bay = self.wing.cell;
        let depth = if self.wing.nz == ROWS {
            bay * 0.5
        } else {
            ((self.wing.nz / 2) as f32 + 0.5) * bay - self.half.y
        };
        Vec3::new(
            self.centre.x + self.facing * side * bay * 1.65,
            floor as f32 * STOREY + v.max(0.4) + 0.25,
            self.centre.y + self.facing * depth,
        )
    }
}

pub(super) fn populate(
    kind: Kind,
    seed: u32,
    v: f32,
    urban: Urban,
    ground: &mut Ground,
    tent: Option<&Tent>,
    layout: &mut Layout,
    landmarks: bool,
    routed: bool,
) {
    let core_row = if roll(seed, 738, 10) < 0.5 { 0 } else { 5 };
    let core_col = if roll(seed, 738, 11) < 0.5 { 3 } else { 4 };
    if routed && kind == Kind::Offices {
        // Reserve one public square beside the through avenue before filling its lots.
        for x in [26.0, -25.0] {
            let plaza = port::yard(Vec2::new(x, 32.0), seed, true);
            if place_kit(plaza, CacheLoot::Household, ground, tent, layout) {
                break;
            }
        }
    }
    let z = [
        -CITY_HALF.y,
        CITY_Z[0],
        CITY_Z[1],
        CITY_Z[2],
        CITY_Z[3],
        CITY_Z[4],
        CITY_HALF.y,
    ];
    for (row, zs) in z.windows(2).enumerate() {
        let mid_z = (zs[0] + zs[1]) * 0.5;
        let mut x = [-CITY_HALF.x; CITY_X.len() + 2];
        for i in 0..CITY_X.len() {
            x[i + 1] = urban.street_x(i, mid_z);
        }
        x[CITY_X.len() + 1] = CITY_HALF.x;
        for (col, xs) in x.windows(2).enumerate() {
            let id = (row * 8 + col) as i32;
            let key = seed ^ (id as u32).wrapping_mul(0x3c79_ac49);
            let centre = Vec2::new((xs[0] + xs[1]) * 0.5, mid_z);
            let avenue = col == 3 || col == 4;
            let park = !routed
                && landmarks
                && (col <= 1 || col >= 6)
                && (row == 0 || row == 5)
                && roll(seed, id, 746) < 0.48;
            if park
                && place_kit(
                    port::yard(centre, key, routed),
                    CacheLoot::Household,
                    ground,
                    tent,
                    layout,
                )
            {
                continue;
            }
            if kind == Kind::Port && (row == 1 || row == 3) && !avenue && col != 0 && col != 7 {
                let kit = if col == 2 && row == 3 {
                    port::pad(centre, 13.0)
                } else {
                    port::cargo_yard(centre - Vec2::Y * 8.0, key)
                };
                if place_kit(kit, CacheLoot::Freight, ground, tent, layout) {
                    let loading = Vec2::new(centre.x, zs[1] - 15.0);
                    let shed = port::freight_shed(loading, key ^ 0x56c8, 1.0, true);
                    place_kit(shed, CacheLoot::Workshop, ground, tent, layout);
                    continue;
                }
            }
            let plaza = !routed && ((row, col) == (3, 3) || (row, col) == (4, 4));
            if plaza {
                if place_kit(
                    port::yard(centre, key, routed),
                    CacheLoot::Household,
                    ground,
                    tent,
                    layout,
                ) {
                    continue;
                }
            }
            let (purpose, height) = match (kind, row, col, avenue) {
                (Kind::Offices, 0 | 5, _, true) if landmarks => {
                    let floors = if row == core_row && col == core_col {
                        28 + (roll(seed, id, 8) * 4.0) as usize
                    } else if row == core_row || col == core_col {
                        14 + (roll(seed, id, 8) * 5.0) as usize
                    } else {
                        5 + (roll(seed, id, 8) * 4.0) as usize
                    };
                    (Use::Offices, floors)
                }
                (Kind::Offices, 0 | 5, _, true) => {
                    (Use::Offices, 22 + (roll(seed, id, 8) * 8.0) as usize)
                }
                (_, 3 | 4, 2 | 5, _) => (Use::Clinic, 2),
                (_, 1 | 4, 1 | 6, _) => (Use::Relay, 2),
                (Kind::Port, 1..=3, _, _) => (Use::Workshop, 1),
                (Kind::Offices, _, _, true) => {
                    (Use::Offices, 6 + (roll(seed, id, 8) * 7.0) as usize)
                }
                (_, _, _, _) => (Use::Apartments, 3 + (roll(seed, id, 8) * 5.0) as usize),
            };
            let other = if purpose == Use::Offices {
                Use::Apartments
            } else {
                purpose
            };
            let setback = |edge, use_: Use, key| {
                let sidewalk = urban.street_margin(edge) - 5.0;
                let building = if use_ == Use::Offices { 1.6 } else { 0.0 };
                let cut = layout.cut.map_or(0.0, |cut| cut.setbacks(centre.x, edge));
                if urban.terraced() {
                    let (bay, rows) = use_.authored().map_or_else(
                        || use_.dimensions(key, true),
                        |d| (d.plan.bay_metres, d.plan.bays[1]),
                    );
                    let verge = if cut > 0.0 {
                        16.3
                    } else {
                        urban.street_margin(edge)
                    };
                    return rows as f32 * bay * 0.5 + verge + 2.6;
                }
                15.0 + sidewalk + building + cut
            };
            // At the northern boundary the main entrance faces the first street.
            let facing = if row == 0 { -1.0 } else { 1.0 };
            let edge = zs[usize::from(row == 0)];
            let opposite = zs[usize::from(row != 0)];
            let back_key = key ^ 0x87c2;
            let offset = if urban.terraced() {
                (roll(key, 925, 1) - 0.5) * 4.0
            } else {
                0.0
            };
            let a = Vec2::new(
                centre.x + offset,
                edge + facing * setback(edge, purpose, key),
            );
            let b = Vec2::new(
                centre.x - offset,
                opposite - facing * setback(opposite, other, back_key),
            );
            let front = place_wing(
                a, facing, purpose, height, key, v, ground, tent, layout, landmarks,
            );
            let back = place_wing(
                b,
                -1.0,
                other,
                height.min(7),
                back_key,
                v,
                ground,
                tent,
                layout,
                landmarks,
            );
            if front && back && (1..=6).contains(&col) {
                let lane = Vec3::new(centre.x, ground.elevation(centre.y) - 0.4, centre.y);
                layout.parts.push(slab(
                    lane,
                    Vec3::new((xs[1] - xs[0]) * 0.5 - 5.0, 0.4, 2.0),
                    B::Gravel,
                    false,
                ));
            }
            if !routed && urban.terraced() && !front && !back && roll(key, 926, 1) > 0.24 {
                let side = if centre.x > 0.0 { 1.0 } else { -1.0 };
                let (kit, loot) = if roll(key, 926, 2) < 0.23 && avenue {
                    (port::substation(centre), CacheLoot::Relay)
                } else if kind == Kind::Port || row == 2 || row == 3 {
                    (
                        port::street_unit(centre, side, key, kind == Kind::Port),
                        CacheLoot::Workshop,
                    )
                } else {
                    (port::yard(centre, key, routed), CacheLoot::Household)
                };
                place_kit(kit, loot, ground, tent, layout);
            }
        }
    }
}

fn place_kit(
    kit: port::Kit,
    loot: CacheLoot,
    ground: &mut Ground,
    tent: Option<&Tent>,
    layout: &mut Layout,
) -> bool {
    let (lo, hi) = kit.footprint;
    let lift = Vec3::Y * ground.elevation((lo.y + hi.y) * 0.5);
    if !ground.free(lo, hi, 1.0)
        || !tent.is_none_or(|t| {
            let cap = t.max_top(lo, hi);
            kit.parts.iter().all(|p| p.hi.y + lift.y < cap)
        })
    {
        return false;
    }
    ground.reserve(lo, hi);
    layout
        .role_caches
        .extend(kit.spots.iter().take(3).map(|p| (*p + lift, loot)));
    layout
        .parts
        .extend(kit.parts.into_iter().map(|part| part.lift(lift.y)));
    true
}

fn place_wing(
    centre: Vec2,
    facing: f32,
    purpose: Use,
    desired: usize,
    seed: u32,
    v: f32,
    ground: &mut Ground,
    tent: Option<&Tent>,
    layout: &mut Layout,
    landmarks: bool,
) -> bool {
    let terraced = ground.terraced();
    let authored = purpose.authored();
    let (cols, rows, bay) = authored.map_or_else(
        || {
            let (bay, rows) = purpose.dimensions(seed, terraced);
            (COLS, rows, bay)
        },
        |d| (d.plan.bays[0], d.plan.bays[1], d.plan.bay_metres),
    );
    let half = Vec2::new(cols as f32, rows as f32) * (bay * 0.5);
    let radius = half + Vec2::splat(1.2);
    let (lo, hi) = (centre - radius, centre + radius);
    if !ground.free(lo, hi, 1.0) {
        return false;
    }
    let elevation = ground.elevation(centre.y);
    let roof_clearance = authored.map_or_else(
        || {
            if purpose == Use::Apartments && rows == 3 {
                8.0
            } else {
                purpose.roof_clearance()
            }
        },
        |d| d.roof_space(),
    );
    let cap = tent.map_or(f32::INFINITY, |t| t.max_top(lo, hi)) - roof_clearance - elevation;
    let (desired, floor_height) =
        authored.map_or((desired, STOREY), |d| (d.floors, d.plan.height_metres));
    let storeys = desired.min((cap / floor_height).floor() as usize);
    if storeys == 0 {
        return false;
    }
    let age = 0.08 + roll(seed, 0, 11) * 0.24;
    let stairs = (storeys > 1).then_some([2, rows - 2]);
    let Ok(wing) = authored.map_or_else(
        || {
            purpose.plan(bay, rows).compile_floors(
                [0.0, 0.0],
                seed,
                v,
                storeys,
                Windows::Panes,
                stairs,
                age,
            )
        },
        |d| d.compile(v, storeys).map_err(|_| "Invalid studio wing"),
    ) else {
        return false;
    };
    let failure = roll(seed, 739, 10);
    let collapse =
        (landmarks && storeys >= 3 && (failure < 0.27 || purpose == Use::Offices && storeys >= 20))
            .then(|| Collapse {
                side: if roll(seed, 739, 11) < 0.5 { -1.0 } else { 1.0 },
                height: ((storeys as f32 * (0.42 + failure * 0.23)).floor() * STOREY).max(8.0),
            });
    let wing = CityWing {
        centre,
        half,
        facing,
        purpose,
        wing,
        collapse,
        roof_clearance,
        authored: authored.is_some(),
    };
    if authored.is_some() {
        ground.reserve(lo, hi);
        layout
            .parts
            .push(Part::new(Shape::CityWing(Box::new(wing))).lift(elevation));
        return true;
    }
    let primary = wing.cache(0, 1.0, v) + Vec3::Y * elevation;
    if !landmarks || purpose != Use::Apartments || failure > 0.42 {
        layout.role_caches.push((primary, purpose.loot()));
    }
    if storeys > 2 && (!landmarks || purpose != Use::Apartments || failure > 0.84) {
        let safe_side = collapse.map_or(-1.0, |damage| -damage.side);
        let upper = wing.cache(storeys - 2, safe_side, v) + Vec3::Y * elevation;
        layout.role_caches.push((upper, purpose.loot()));
        if matches!(purpose, Use::Offices | Use::Relay) {
            let offset = if landmarks {
                Vec3::Z * 1.15
            } else {
                Vec3::X * 3.0
            };
            layout.sentries.push(upper + offset * facing);
        }
    }
    ground.reserve(lo, hi);
    if terraced && matches!(purpose, Use::Clinic | Use::Offices) {
        entrance_canopy(&wing, elevation, &mut layout.parts);
    }
    layout
        .parts
        .push(Part::new(Shape::CityWing(Box::new(wing))).lift(elevation));
    true
}

fn entrance_canopy(wing: &CityWing, elevation: f32, parts: &mut Vec<Part>) {
    let x = wing.centre.x - wing.facing * wing.wing.cell * 0.5;
    let z = wing.centre.y - wing.facing * wing.half.y;
    let roof = Vec3::new(x, elevation + 3.6, z - wing.facing * 1.3);
    let fascia = if wing.purpose == Use::Clinic {
        B::GlazedTiles
    } else {
        B::SteelPlate
    };
    parts.push(slab(roof, Vec3::new(4.0, 0.45, 1.6), B::Concrete, false));
    parts.push(slab(
        roof + Vec3::new(0.0, 0.05, -wing.facing * 1.5),
        Vec3::new(4.0, 0.38, 0.35),
        fascia,
        false,
    ));
    for side in [-1.0, 1.0] {
        let base = Vec3::new(x + side * 3.2, elevation + 2.5, z + wing.facing * 0.1);
        let end = Vec3::new(base.x, elevation + 3.5, z - wing.facing * 2.4);
        parts.push(beam(base, end, 0.32, B::RustedSteel, false));
    }
}
