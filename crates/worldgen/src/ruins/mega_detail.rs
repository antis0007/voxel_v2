//! Span kit shared by the transit viaduct and the elevated A-frame spine.
use super::mega::{Mega, beam, slab};
use super::shapes::{Part, Shape, roll};
use crate::BlockId as B;
use glam::{Vec2, Vec3};
use std::ops::Range;

pub(super) fn track(
    parts: &mut Vec<Part>,
    span: Range<f32>,
    z: f32,
    floor: f32,
    entries: &[f32],
    seed: u32,
) {
    let middle = (span.start + span.end) * 0.5;
    let half = (span.end - span.start) * 0.5;
    for side in [-1.0, 1.0] {
        let edge = z + side * 6.0;
        let rail = z + side * 2.6;
        parts.push(slab(
            Vec3::new(middle, floor - 3.0, edge - side * 1.0),
            Vec3::new(half, 1.1, 0.75),
            B::SteelPlate,
            false,
        ));
        parts.push(slab(
            Vec3::new(middle, floor + 0.38, rail),
            Vec3::new(half, 0.4, 0.34),
            B::RustedSteel,
            false,
        ));
        for (i, x) in (span.start.ceil() as i32..span.end.floor() as i32)
            .step_by(4)
            .enumerate()
        {
            let x = x as f32;
            parts.push(slab(
                Vec3::new(x, floor - 4.0, edge - side * 0.7),
                Vec3::new(0.55, 0.55, 1.1),
                B::RustedSteel,
                false,
            ));
            if (side > 0.0 && entries.iter().any(|e| (e - x).abs() < 13.0)) || roll(seed, i as i32, 227) < 0.15 {
                continue;
            }
            parts.push(slab(
                Vec3::new(x, floor + 0.64, edge),
                Vec3::new(0.42, 0.65, 0.42),
                B::SteelPlate,
                false,
            ));
            if x + 4.0 < span.end && (side < 0.0 || entries.iter().all(|e| (e - x - 2.0).abs() > 14.0)) {
                parts.push(slab(
                    Vec3::new(x + 1.7, floor + 1.35, edge),
                    Vec3::new(1.8, 0.28, 0.35),
                    B::SteelPlate,
                    false,
                ));
            }
        }
    }
    for x in (span.start.ceil() as i32..span.end.floor() as i32).step_by(6) {
        let x = x as f32;
        parts.push(slab(
            Vec3::new(x, floor - 4.3, z),
            Vec3::new(0.45, 0.65, 5.1),
            B::SteelPlate,
            false,
        ));
    }
}

pub(super) fn service(
    parts: &mut Vec<Part>,
    blocked: &mut Vec<(Vec2, Vec2)>,
    x: f32,
    z: f32,
    floor: f32,
) {
    let c = Vec3::new(x, 0.0, z + 12.0);
    parts.push(Part::new(Shape::ServiceStair { c, height: floor }));
    parts.push(slab(
        Vec3::new(x - 5.0, floor - 0.5, z + 9.4),
        Vec3::new(2.0, 0.5, 3.4),
        B::SteelPlate,
        false,
    ));
    // Four uprights carry the stairs and the transfer platform.
    for dx in [-7.2, 7.2] {
        for dz in [-3.8, 3.8] {
            let base = c + Vec3::new(dx, 0.0, dz);
            parts.push(beam(
                base,
                base + Vec3::Y * floor,
                0.6,
                B::RustedSteel,
                false,
            ));
        }
    }
    blocked.push((Vec2::new(x - 8.0, z + 7.0), Vec2::new(x + 8.0, z + 17.0)));
}

/// A boarding shelter beside the tracks, tied to the stair and deck girder.
pub(super) fn stop(parts: &mut Vec<Part>, x: f32, z: f32, floor: f32, seed: u32) {
    let at = |dx, y, dz| Vec3::new(x + dx, floor + y, z + dz);
    // A real platform outside the running rails, joined to the stair's top
    // landing. The old shelter occupied the vehicle clearance itself.
    parts.push(slab(at(0.0, -0.6, 9.0), Vec3::new(13.0, 0.6, 3.0), B::Concrete, false));
    parts.push(slab(at(0.0, 0.1, 6.6), Vec3::new(13.0, 0.1, 0.45), B::HazardPaint, false));
    for dx in [-10.0, 0.0, 10.0] {
        parts.push(beam(at(dx, -1.2, 11.6), at(dx, 4.8, 11.6), 0.48, B::RustedSteel, false));
        parts.push(beam(at(dx, 4.8, 11.6), at(dx, 4.3, 6.8), 0.48, B::SteelPlate, false));
        parts.push(beam(at(dx, -1.2, 11.6), at(dx, -4.3, 3.8), 0.65, B::RustedSteel, false));
    }
    for (i, dx) in [-10.0, -6.0, -2.0, 2.0, 6.0, 10.0].into_iter().enumerate() {
        if i == 0 || i == 5 || roll(seed, i as i32, 344) > 0.25 {
            parts.push(Part::new(Shape::Sheet {
                a: at(dx, 4.8, 12.0),
                b: at(dx, 4.3, 6.7),
                half_width: 2.05,
                thick: 0.45,
                mat: if i % 2 == 0 { B::SteelPlate } else { B::WindowGlass },
            }));
        }
    }
    parts.push(beam(at(-12.0, 4.9, 11.6), at(12.0, 4.9, 11.6), 0.48, B::RustedSteel, false));
    parts.push(slab(at(4.0, 3.2, 11.6), Vec3::new(4.5, 0.55, 0.55), B::GlazedTiles, false));
    parts.push(slab(at(4.0, 2.65, 11.6), Vec3::new(1.4, 0.50, 0.52), B::HazardPaint, false));
    // Keep a gap for the stair landing in the outside guardrail.
    for (lo, hi) in [(-13.0, -7.5), (-2.5, 13.0)] {
        parts.push(beam(at(lo, 1.2, 12.0), at(hi, 1.2, 12.0), 0.35, B::SteelPlate, false));
    }
}

/// Gap centres that leave a service pier on both broken spans.
pub(super) fn gap_window(city: bool) -> (f32, f32) {
    if city { (-64.0, 64.0) } else { (-60.0, 58.0) }
}

/// Buffer stop at a deliberate terminus, with its bracing inside the deck.
pub(super) fn buffer(parts: &mut Vec<Part>, end: f32, inward: f32, z: f32, floor: f32) {
    for side in [-1.0, 1.0] {
        parts.push(slab(Vec3::new(end + inward * 1.4, floor + 0.7, z + side * 2.6), Vec3::new(0.5, 0.7, 0.5), B::RustedSteel, false));
    }
    parts.push(slab(Vec3::new(end + inward * 1.9, floor + 1.1, z), Vec3::new(0.3, 0.45, 3.4), B::HazardPaint, false));
}

/// Pier footings reach this far below the deck datum.
pub(super) const FOOTING: f32 = 40.0;

pub(super) fn viaduct(seed: u32, half: Vec2, z: f32, city: bool, _terraced: bool, line: Option<super::line::Line>) -> Mega {
    let base = if city { 11.0 } else { 8.0 };
    let floor = (base + roll(seed, 6, 0) * 3.0).floor() * 4.0;
    let reach = half.x + 34.0 + roll(seed, 7, 0) * 14.0;
    let piers = if city {
        [-reach + 8.0].into_iter().chain(super::urban::CITY_PIERS).chain([reach - 8.0]).collect()
    } else { vec![
        -reach + 8.0,
        -109.0,
        -76.0,
        -20.0,
        20.0,
        76.0,
        109.0,
        reach - 8.0,
    ] };
    let mut parts = Vec::new();
    let mut blocked = vec![(Vec2::new(-reach, z - 14.0), Vec2::new(reach, z + 14.0))];
    for (i, x) in piers.into_iter().enumerate() {
        let at = |y, offset| Vec3::new(x, y, z + offset);
        for side in [-1.0, 1.0] {
            let offset = side * 10.5;
            parts.push(slab(
                at(-18.0, offset),
                Vec3::new(4.2, 22.0, 4.0),
                B::WeatheredBricks,
                true,
            ));
            parts.push(slab(
                at(5.0, offset),
                Vec3::new(3.4, 2.2, 3.0),
                B::Concrete,
                false,
            ));
            parts.push(slab(
                at((floor + 17.0) * 0.5, offset),
                Vec3::new(2.0, (floor + 3.0) * 0.5, 1.7),
                B::Concrete,
                false,
            ));
            for y in [10.0, floor - 6.0, floor + 7.0] {
                parts.push(slab(
                    at(y, offset),
                    Vec3::new(2.8, 0.55, 2.3),
                    B::SteelPlate,
                    false,
                ));
            }
            parts.push(beam(
                at(1.0, side * 13.3),
                at(15.0, side * 10.5),
                1.35,
                B::Concrete,
                true,
            ));
            parts.push(beam(
                at(floor - 10.0, offset),
                at(floor - 3.0, side * 3.8),
                0.85,
                B::RustedSteel,
                false,
            ));
        }
        parts.push(slab(
            at(floor - 2.8, 0.0),
            Vec3::new(2.2, 1.3, 10.5),
            B::Concrete,
            false,
        ));
        let crown = at(floor + 10.0, 0.0);
        for dx in [-1.65, 1.65] {
            parts.push(Part::new(Shape::Arch {
                c: crown + Vec3::X * dx,
                radius: 10.5,
                r: 1.5,
                depth: 0.8,
                along_x: false,
                mat: B::Concrete,
            }));
        }
        for k in 1..8 {
            if roll(seed, k, i as i32 + 210) < 0.2 {
                continue;
            }
            let angle = k as f32 * std::f32::consts::PI / 8.0;
            let joint = crown + Vec3::new(0.0, angle.sin(), angle.cos()) * 10.5;
            parts.push(slab(
                joint,
                Vec3::new(2.5, 0.64, 0.64),
                B::RustedSteel,
                false,
            ));
        }
    }

    let failure = roll(seed, 10, 209);
    let spans = if let Some(line) = line {
        match line.gap {
            Some(at) => vec![-reach..at - 6.0, at + 8.0..reach],
            None => vec![-reach..reach],
        }
    } else if failure < 0.72 {
        let at = if failure < 0.36 { -49.0 } else { 49.0 };
        vec![-reach..at - 6.0, at + 8.0..reach]
    } else {
        vec![-reach..reach]
    };
    let mut decks = Vec::new();
    let mut access = Vec::new();
    // Two useful local destinations, inside the settlement's street fabric.
    // A damaged span preserves both stations and the cause of disconnection.
    let station_x = if city { [-163.0, 164.0] } else { [-90.0, 90.0] };
    for span in spans {
        let x = (span.start + span.end) * 0.5;
        let extent = (span.end - span.start) * 0.5;
        parts.push(slab(
            Vec3::new(x, floor - 1.3, z),
            Vec3::new(extent, 1.3, 6.5),
            B::Concrete,
            true,
        ));
        let stations: Vec<f32> = station_x.into_iter()
            .filter(|x| *x > span.start + 14.0 && *x < span.end - 14.0).collect();
        track(&mut parts, span.clone(), z, floor, &stations, seed);
        for station in stations {
            service(&mut parts, &mut blocked, station, z, floor);
            stop(&mut parts, station, z, floor, seed);
            access.push(Vec2::new(station - 5.0, z + 8.4));
            decks.push(Vec3::new(station + 4.0, floor + 0.15, z + 9.0));
        }
        for (end, direction) in [(span.start, -1.0), (span.end, 1.0)] {
            if end.abs() >= reach {
                buffer(&mut parts, end, -direction, z, floor);
                continue;
            }
            for k in 0..4 {
                let edge = Vec3::new(end, floor - 2.0, z - 4.5 + k as f32 * 3.0);
                let drop = 3.0 + roll(seed, k, 221) * 8.0;
                parts.push(beam(
                    edge,
                    edge + Vec3::new(direction * 3.5, -drop, 1.6),
                    0.43,
                    B::RustedSteel,
                    false,
                ));
            }
            if end.abs() < half.x {
                parts.push(beam(
                    Vec3::new(end, 1.8, z - 3.0),
                    Vec3::new(end + direction * 7.0, 3.2, z + 3.0),
                    1.8,
                    B::Concrete,
                    true,
                ));
            }
        }
    }
    Mega {
        parts,
        blocked,
        tent: None,
        decks,
        access,
    }
}
