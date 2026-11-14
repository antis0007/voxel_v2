//! Landmarks change the way a city is crossed, and claim space before the lots.
use super::hull::Hull;
use super::layout::{Ground, Layout};
use super::mega::{beam, slab};
use super::shapes::{Part, Shape, grows_moss, roll};
use super::urban::{CITY_X, Urban};
use super::{CacheLoot, Kind};
use crate::BlockId as B;
use glam::{Vec2, Vec3, Vec3Swizzles};

pub(super) const CUT_STREET: f32 = -4.0;
const CUT_DEPTH: f32 = 7.0;

#[derive(Clone, Copy)]
pub(super) struct ServiceCut {
    end: f32,
    breach: f32,
    sway: f32,
    stairs: Option<f32>,
    return_stair: Option<f32>,
}

impl ServiceCut {
    pub(super) fn review_stairs(self, returning: bool) -> Option<(Vec3, Vec3)> {
        let (x, bank) = if returning {
            (self.return_stair?, -1.0)
        } else {
            (self.stairs?, 1.0)
        };
        let z = self.centre(x);
        Some((
            Vec3::new(x, 2.1, z + bank * (self.width(x) + 7.8)),
            Vec3::new(x, -5.1, z - bank * 2.5),
        ))
    }

    fn centre(self, x: f32) -> f32 {
        CUT_STREET + self.sway * (x * 0.024).sin()
    }

    fn width(self, x: f32) -> f32 {
        10.5 + 1.5 * (x * 0.062).cos()
    }

    pub fn setbacks(self, x: f32, street: f32) -> f32 {
        if street == CUT_STREET && x.abs() < self.end + 17.0 {
            9.0
        } else {
            0.0
        }
    }

    pub fn sample(self, q: Vec3, v: f32, seed: u32) -> Option<B> {
        let length = self.end - q.x.abs();
        if length < 0.0 || !(-9.0..2.0).contains(&q.y) {
            return None;
        }
        let lateral = q.z - self.centre(q.x);
        let across = lateral.abs();
        let width = self.width(q.x);
        if across > width + 1.4 {
            return None;
        }
        let floor = -((length * 0.45).min(CUT_DEPTH) / v).round() * v;
        if let Some((dx, half)) = CITY_X.into_iter().find_map(|x| {
            let half = if x == 0.0 { 7.0 } else { 4.7 };
            let dx = (q.x - x).abs();
            (dx <= half).then_some((dx, half))
        }) {
            let broken = (q.x - self.breach).abs() <= 4.7;
            if !broken || across > width - 2.7 {
                if (-v.max(1.1)..0.0).contains(&q.y) {
                    return Some(if dx < half - 1.4 {
                        B::Basalt
                    } else {
                        B::DressedStone
                    });
                }
                if dx > half - 1.1 && across < width && (-2.1..-v).contains(&q.y) {
                    return Some(B::SteelPlate);
                }
                if dx > half - 0.8 && across < width && (0.0..1.0).contains(&q.y) {
                    return Some(B::RustedSteel);
                }
            }
            if broken && (width - 1.5..width + 0.6).contains(&across) && (0.0..1.0).contains(&q.y) {
                return Some(B::RustedSteel);
            }
        }
        let stair = if lateral >= 0.0 {
            self.stairs
        } else {
            self.return_stair
        };
        if stair.is_some_and(|x| (q.x - x).abs() < 4.2) && across <= width + 1.4 && q.y < 0.0 {
            let fraction = (across / width).clamp(0.0, 1.0);
            let tread = (-CUT_DEPTH * (1.0 - fraction) / v).round() * v;
            if q.y >= tread {
                return Some(B::Air);
            }
            if q.y >= floor - v {
                return Some(if q.y >= tread - v {
                    B::DressedStone
                } else {
                    B::Concrete
                });
            }
        }
        if q.y >= 0.0 {
            return None;
        }
        if self.stairs.is_some() && across >= width - 1.1 && q.y >= floor - v {
            let joint = (q.x + if lateral > 0.0 { 3.0 } else { -4.0 }).rem_euclid(18.0);
            if joint < v.max(1.4) && (length > 19.0 || across >= width) {
                return Some(B::WeatheredBricks);
            }
            if across >= width && length > 19.0 {
                let seep = grows_moss(Vec3::new(q.x * 0.7, 0.0, q.y * 0.7), seed ^ 0x3e1);
                if seep && q.y < floor + 2.8 {
                    return Some(B::GroveMoss);
                }
                if (q.y + 2.4).abs() < v.max(0.7) * 0.53 {
                    return Some(B::DressedStone);
                }
            }
        }
        if across >= width && q.y >= floor - v {
            let joint = (q.x + 1.0).rem_euclid(8.0) < v.max(0.65);
            return Some(if joint { B::DressedStone } else { B::Concrete });
        }
        if q.y >= floor {
            return Some(B::Air);
        }
        if q.y >= floor - v {
            let rail = (across - 4.2).abs() < v.max(0.6) * 0.55;
            let route_mark = self
                .stairs
                .zip(self.return_stair)
                .is_some_and(|(entry, exit)| {
                    (entry.min(exit) - 4.0..entry.max(exit) + 4.0).contains(&q.x)
                        && (across - 2.9).abs() < v.max(0.55) * 0.55
                });
            return Some(if route_mark {
                B::GlazedTiles
            } else if rail && length > 19.0 {
                B::RustedSteel
            } else if across > 5.0 && grows_moss(q, seed) {
                B::GroveMoss
            } else if across < 4.0 {
                B::DressedStone
            } else {
                B::Gravel
            });
        }
        None
    }
}

pub(super) fn place(
    kind: Kind,
    seed: u32,
    v: f32,
    urban: Urban,
    ground: &mut Ground,
    layout: &mut Layout,
    routed: bool,
) {
    match kind {
        Kind::Offices => service_cut(seed, urban.terraced(), ground, layout, routed),
        Kind::Port => {
            let first_part = layout.parts.len();
            let first_cache = layout.role_caches.len();
            wreck(seed, v, ground, layout, routed);
            let elevation = urban.elevation(130.0);
            for part in &mut layout.parts[first_part..] {
                part.raise(elevation);
            }
            for (spot, _) in &mut layout.role_caches[first_cache..] {
                spot.y += elevation;
            }
            if urban.terraced() {
                freight_terminal(seed, urban, ground, layout);
            }
        }
    }
}

fn service_cut(seed: u32, terraced: bool, ground: &mut Ground, layout: &mut Layout, routed: bool) {
    let sign = if roll(seed, 731, 12) < 0.5 { -1.0 } else { 1.0 };
    let cut = ServiceCut {
        end: 149.0 + roll(seed, 733, 12) * 12.0,
        breach: if sign < 0.0 { -49.0 } else { 52.0 },
        sway: 1.8 * sign,
        stairs: terraced.then_some(37.0 * sign),
        return_stair: routed.then_some(-68.0 * sign),
    };
    layout.cut = Some(cut);
    ground.reserve(
        Vec2::new(-cut.end, CUT_STREET - 15.2),
        Vec2::new(cut.end, CUT_STREET + 15.2),
    );
    if let Some(x) = cut.stairs {
        stair_head(cut, x, 1.0, routed, ground, &mut layout.parts);
    }
    if let Some(x) = cut.return_stair {
        stair_head(cut, x, -1.0, routed, ground, &mut layout.parts);
    }
    let parts = &mut layout.parts;
    // Two open portal frames and a row of isolated switchgear face the lower route.
    for x in [-29.0, 31.0] {
        let z = cut.centre(x);
        for side in [-1.0, 1.0] {
            parts.push(slab(
                Vec3::new(x, -5.1, z + 6.4 * side),
                Vec3::new(1.6, 2.0, 1.25),
                B::Concrete,
                false,
            ));
        }
        parts.push(Part::new(Shape::Arch {
            c: Vec3::new(x, -3.3, z),
            radius: 6.4,
            r: 1.0,
            depth: 1.45,
            along_x: false,
            mat: B::WeatheredBricks,
        }));
    }
    let plant_x = if terraced { -18.0 * sign } else { 18.0 * sign };
    let plant = Vec3::new(plant_x, -7.0, cut.centre(plant_x) + 7.5);
    if routed {
        for x in [-4.1, 4.1] {
            let foot = plant + Vec3::new(x, 0.75, 0.0);
            parts.push(beam(
                foot,
                foot + Vec3::Y * 3.25,
                0.56,
                B::RustedSteel,
                false,
            ));
        }
    }
    parts.push(slab(
        plant + Vec3::Y * 0.5,
        Vec3::new(4.5, 0.5, 1.7),
        B::CarvedStone,
        false,
    ));
    for dx in [-3.0, 0.0, 3.0] {
        let base = plant + Vec3::X * dx;
        parts.push(slab(
            base + Vec3::Y * 1.6,
            Vec3::new(0.85, 1.25, 0.85),
            B::RustedSteel,
            false,
        ));
        parts.push(slab(
            base + Vec3::Y * 3.1,
            Vec3::new(0.6, 0.65, 0.6),
            B::Crystal,
            false,
        ));
    }
    parts.push(beam(
        plant + Vec3::new(-4.5, 4.0, 0.0),
        plant + Vec3::new(4.5, 4.0, 0.0),
        0.55,
        B::RustedSteel,
        false,
    ));
    layout
        .role_caches
        .push((plant + Vec3::new(1.6, 1.15, -2.2), CacheLoot::Relay));

    if routed {
        return;
    }
    for (i, x) in [
        -132.0f32, -124.0, -94.0, -81.0, -71.0, 66.0, 75.0, 90.0, 132.0, 141.0,
    ]
    .into_iter()
    .enumerate()
    {
        if cut.end - x.abs() < 17.0 || CITY_X.into_iter().any(|street| (street - x).abs() < 8.0) {
            continue;
        }
        let side = if i % 3 == 0 { 1.0 } else { -1.0 };
        let base = Vec3::new(x, -7.0, cut.centre(x) + side * 7.5);
        let trunk = base + Vec3::new(0.6 * side, 8.5 + roll(seed, i as i32, 741) * 5.0, 0.0);
        parts.push(beam(base, trunk, 0.55, B::Log, false));
        parts.push(Part::new(Shape::Blob {
            c: trunk,
            half: Vec3::new(4.0, 2.25, 3.7),
            mat: B::Leaves,
            seed: seed ^ i as u32,
        }));
    }
}

fn stair_head(
    cut: ServiceCut,
    x: f32,
    bank: f32,
    routed: bool,
    ground: &mut Ground,
    parts: &mut Vec<Part>,
) {
    let z = cut.centre(x) + bank * (cut.width(x) + 1.3);
    let approach = z + bank * 7.0;
    ground.reserve(
        Vec2::new(x - 7.0, (z - bank).min(approach)),
        Vec2::new(x + 7.0, (z - bank).max(approach)),
    );
    for side in [-1.0, 1.0] {
        parts.push(slab(
            Vec3::new(x + side * 5.3, 2.0, z),
            Vec3::new(1.15, 2.0, 1.6),
            B::Concrete,
            true,
        ));
        parts.push(beam(
            Vec3::new(x + side * 5.3, 0.0, z + bank * 5.5),
            Vec3::new(x + side * 5.3, 4.1, z),
            0.65,
            B::RustedSteel,
            false,
        ));
    }
    parts.push(Part::new(Shape::Arch {
        c: Vec3::new(x, 3.9, z),
        radius: 5.3,
        r: 1.0,
        depth: 1.4,
        along_x: true,
        mat: B::DressedStone,
    }));
    parts.push(slab(
        Vec3::new(x, 8.65, z + bank * 1.9),
        Vec3::new(3.1, 0.35, 2.3),
        B::SteelPlate,
        false,
    ));
    parts.push(slab(
        Vec3::new(x, 8.9, z + bank * 3.4),
        Vec3::new(2.2, 0.5, 0.6),
        if routed {
            B::GlazedTiles
        } else {
            B::HazardPaint
        },
        false,
    ));
}

fn freight_terminal(seed: u32, urban: Urban, ground: &mut Ground, layout: &mut Layout) {
    let z = 74.0;
    let sign = if roll(seed, 751, 1) < 0.5 { -1.0 } else { 1.0 };
    let extent = Vec2::new(12.8, 17.0);
    let candidates = [3, 4, 2, 5, 1, 6];
    let Some(c) = candidates
        .into_iter()
        .map(|i| {
            let i = if sign < 0.0 { i } else { 7 - i };
            Vec2::new((urban.street_x(i - 1, z) + urban.street_x(i, z)) * 0.5, z)
        })
        .find(|&c| ground.free(c - extent, c + extent, 1.0))
    else {
        return;
    };
    ground.reserve(c - extent, c + extent);
    layout.terminal = Some(c);
    let at = |x, y, z| Vec3::new(c.x + x, y, c.y + z);
    let parts = &mut layout.parts;
    // A sorting bay below, a loading floor above, and a separate service stair.
    parts.push(slab(
        at(0.0, 7.25, 0.0),
        Vec3::new(12.0, 0.75, 7.1),
        B::Concrete,
        true,
    ));
    for x in [-10.5, 10.5] {
        for z in [-5.8, 5.8] {
            parts.push(slab(
                at(x, 3.8, z),
                Vec3::new(1.55, 3.8, 1.6),
                B::Concrete,
                false,
            ));
            parts.push(beam(
                at(x, 8.0, z),
                at(x * 0.80, 14.0, z * 0.80),
                0.65,
                B::RustedSteel,
                false,
            ));
        }
    }
    for x in [-10.5, 0.0, 10.5] {
        parts.push(beam(
            at(x, 13.0, -6.6),
            at(x, 16.5, 6.6),
            0.7,
            B::SteelPlate,
            false,
        ));
    }
    for (x, mat) in [
        (-7.2, B::SteelPlate),
        (-2.4, B::WindowGlass),
        (2.4, B::WindowGlass),
        (7.2, B::SteelPlate),
    ] {
        if x > 0.0 && roll(seed, (x * 10.0) as i32, 752) < 0.43 {
            continue;
        }
        parts.push(Part::new(Shape::Sheet {
            a: at(x, 13.2, -6.6),
            b: at(x, 16.7, 6.6),
            half_width: 2.4,
            thick: 0.32,
            mat,
        }));
    }
    parts.push(beam(
        at(-11.5, 12.7, -6.9),
        at(11.5, 12.7, -6.9),
        0.65,
        B::HazardPaint,
        false,
    ));
    parts.push(slab(
        at(-7.0, 9.05, -4.6),
        Vec3::new(2.7, 1.05, 1.5),
        B::RustedSteel,
        false,
    ));
    let mut supports = Vec::new();
    super::mega_detail::service(parts, &mut supports, c.x, c.y, 8.0);
    layout
        .role_caches
        .push((at(5.0, 8.5, -4.2), CacheLoot::Dispatch));
    layout
        .role_caches
        .push((at(-7.0, 0.5, 1.7), CacheLoot::Freight));
}

pub(super) fn wreck_side(seed: u32) -> f32 {
    if roll(seed, 744, 1) < 0.5 { -1.0 } else { 1.0 }
}

fn wreck(seed: u32, v: f32, ground: &mut Ground, layout: &mut Layout, routed: bool) {
    let side = wreck_side(seed);
    let a = Vec3::new(side * 47.0, 4.6, 124.0);
    let b = Vec3::new(side * (137.0 + roll(seed, 745, 1) * 12.0), 0.1, 136.0);
    let hull = Hull {
        a,
        b,
        radius: 10.0,
        roll: side * 0.10,
        seed,
        routed,
    };
    let (lo, hi) = hull.bounds();
    ground.reserve(
        Vec2::new(lo.x - 9.0, lo.z - 2.0),
        Vec2::new(hi.x + 9.0, hi.z + 2.0),
    );
    let access = a - Vec3::Y * 3.0;
    layout.parts.push(slab(
        access + Vec3::new(side * 0.6, -0.55, 0.0),
        Vec3::new(4.2, 0.55, 2.7),
        B::RustedSteel,
        false,
    ));
    layout.parts.push(slab(
        access + Vec3::new(-side * 4.8, -1.18, 0.0),
        Vec3::new(1.3, 0.42, 2.7),
        B::Concrete,
        true,
    ));
    if routed {
        wreck_access(&hull, side, ground, &mut layout.parts);
    }
    for (i, spot) in hull.deck_spots(v).into_iter().step_by(2).enumerate() {
        layout.role_caches.push((
            spot,
            if i == 2 {
                CacheLoot::Relay
            } else {
                CacheLoot::Freight
            },
        ));
    }
    layout.parts.push(Part::new(Shape::Hull(Box::new(hull))));
    if routed {
        let foot = Vec3::new(b.x - side * 3.0, -0.2, b.z + 4.7);
        layout.parts.push(Part::new(Shape::Blob {
            c: foot,
            half: Vec3::new(6.0, 2.1, 4.0),
            mat: B::Gravel,
            seed,
        }));
        layout.parts.push(Part::new(Shape::Sheet {
            a: foot - Vec3::Y * 0.65,
            b: foot + Vec3::new(-side * 4.5, 2.3, -2.5),
            half_width: 2.4,
            thick: 0.6,
            mat: B::RustedSteel,
        }));
        return;
    }
    // The torn side panel and rubble trail point back toward the hull.
    for i in 0..8 {
        let dx = side * (i as f32 - 3.0) * 2.2;
        let z = 135.0 + (roll(seed, i, 747) - 0.5) * 16.0;
        let at = Vec3::new(b.x + dx, 0.6, z);
        layout.parts.push(Part::new(Shape::Blob {
            c: at,
            half: Vec3::new(3.0, 1.25, 2.35),
            mat: if i % 2 == 0 { B::Concrete } else { B::Gravel },
            seed: seed ^ i as u32,
        }));
    }
    let panel = Vec3::new(b.x - side * 18.0, 0.9, 149.0);
    layout.parts.push(Part::new(Shape::Sheet {
        a: panel,
        b: panel + Vec3::new(side * 8.0, 4.0, -2.0),
        half_width: 3.4,
        thick: 0.55,
        mat: B::RustedSteel,
    }));
}

fn wreck_access(hull: &Hull, side: f32, ground: &mut Ground, parts: &mut Vec<Part>) {
    let from = Vec3::new(side * 7.0, -0.55, 117.0);
    let to = Vec3::new(side * 42.0, -0.55, 124.0);
    ground.reserve(
        from.xz().min(to.xz()) - Vec2::splat(2.4),
        from.xz().max(to.xz()) + Vec2::splat(2.4),
    );
    parts.push(Part::new(Shape::Sheet {
        a: from,
        b: to,
        half_width: 2.0,
        thick: 0.55,
        mat: B::DressedStone,
    }));
    let (door, outward) = hull.cargo_exit();
    let edge = door + outward * 2.7;
    let half = Vec3::new(2.35, 0.8, 3.9);
    ground.reserve(edge.xz() - half.xz(), edge.xz() + half.xz());
    parts.push(slab(
        Vec3::new(edge.x, door.y.clamp(-1.0, 1.0) * 0.5 - half.y, edge.z),
        half,
        B::DressedStone,
        false,
    ));
}
