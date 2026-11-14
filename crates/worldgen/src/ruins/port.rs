//! Carriage port furniture: landing pads, lattice gantries with crane jibs and
//! a corrugated hangar whose roof has partly fallen in.
use super::shapes::{Part, Shape, roll};
use crate::BlockId as B;
use glam::{Vec2, Vec3};

pub(super) struct Kit {
    pub parts: Vec<Part>,
    pub footprint: (Vec2, Vec2),
    /// Floor points for loot and sentries.
    pub spots: Vec<Vec3>,
}

fn beam(a: Vec3, b: Vec3, r: f32, mat: B) -> Part {
    Part::new(Shape::Beam { a, b, r, mat, moss: false })
}

fn slab(c: Vec3, half: Vec3, mat: B) -> Part {
    Part::new(Shape::Slab { c, half, mat, moss: false })
}

pub(super) fn pad(c: Vec2, r: f32) -> Kit {
    let parts = vec![Part::new(Shape::Pad { c: Vec3::new(c.x, 0.0, c.y), r, h: 1.2 })];
    let spots = vec![Vec3::new(c.x + r * 0.5, 1.7, c.y), Vec3::new(c.x - r * 0.5, 1.7, c.y)];
    Kit { parts, footprint: (c - Vec2::splat(r), c + Vec2::splat(r)), spots }
}

/// A square lattice tower; the jib points toward `toward`.
pub(super) fn gantry(c: Vec2, height: f32, toward: Vec2, seed: u32) -> Kit {
    let half = 2.6;
    let mut parts = Vec::new();
    let corner = |sx: f32, sz: f32, y: f32| Vec3::new(c.x + sx * half, y, c.y + sz * half);
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for &(sx, sz) in &corners {
        parts.push(beam(corner(sx, sz, 0.0), corner(sx, sz, height), 0.35, B::SteelPlate));
    }
    let mut spots = Vec::new();
    let mut y = 0.0;
    let mut level = 0;
    while y + 6.0 <= height {
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            parts.push(beam(corner(a.0, a.1, y + 6.0), corner(b.0, b.1, y + 6.0), 0.25, B::RustedSteel));
            // X bracing on every face; one diagonal per face lost to corrosion.
            parts.push(beam(corner(a.0, a.1, y), corner(b.0, b.1, y + 6.0), 0.2, B::RustedSteel));
            if roll(seed, level, k as i32) > 0.25 {
                parts.push(beam(corner(b.0, b.1, y), corner(a.0, a.1, y + 6.0), 0.2, B::RustedSteel));
            }
        }
        if level % 2 == 1 {
            let at = Vec3::new(c.x, y + 6.0, c.y);
            parts.push(slab(at, Vec3::new(half + 0.6, 0.2, half + 0.6), B::SteelPlate));
            spots.push(at + Vec3::new(1.0, 0.6, 1.0));
        }
        y += 6.0;
        level += 1;
    }
    // Jib, counter-jib, cab and a cable still hanging with its hook block.
    let dir = (toward - c).normalize_or(Vec2::X);
    let top = Vec3::new(c.x, height + 1.0, c.y);
    let reach = Vec3::new(dir.x, 0.0, dir.y);
    parts.push(beam(top - reach * 8.0, top + reach * 22.0, 0.7, B::HazardPaint));
    parts.push(slab(top - reach * 7.0 - Vec3::Y * 1.5, Vec3::new(1.8, 1.5, 1.8), B::Concrete));
    parts.push(slab(top + Vec3::Y * 2.0, Vec3::new(1.8, 1.4, 1.8), B::WindowGlass));
    let hook = top + reach * 18.0;
    let drop = 6.0 + roll(seed, 99, 0) * (height * 0.6);
    parts.push(beam(hook, hook - Vec3::Y * drop, 0.08, B::SteelPlate));
    parts.push(slab(hook - Vec3::Y * (drop + 0.8), Vec3::new(0.8, 0.8, 0.8), B::HazardPaint));
    let footprint = (c - Vec2::splat(half + 1.0), c + Vec2::splat(half + 1.0));
    Kit { parts, footprint, spots }
}

/// A 44 m x 28 m shed open on its +z end. Bays are 6 m; some wall panels and
/// roof sheets are gone.
pub(super) fn hangar(c: Vec2, seed: u32) -> Kit {
    let (hx, hz, h) = (14.0, 22.0, 13.0);
    let mut parts = Vec::new();
    let mut spots = Vec::new();
    let bays = (2.0 * hz / 6.0) as i32;
    for i in 0..=bays {
        let z = c.y - hz + i as f32 * 6.0;
        for side in [-1.0, 1.0] {
            parts.push(beam(Vec3::new(c.x + side * hx, 0.0, z), Vec3::new(c.x + side * hx, h, z), 0.45, B::SteelPlate));
        }
        // Portal rafter rising to the ridge.
        let ridge = Vec3::new(c.x, h + 4.0, z);
        parts.push(beam(Vec3::new(c.x - hx, h, z), ridge, 0.4, B::SteelPlate));
        parts.push(beam(Vec3::new(c.x + hx, h, z), ridge, 0.4, B::SteelPlate));
        if i == bays {
            continue;
        }
        let mid = z + 3.0;
        for side in [-1.0, 1.0] {
            if roll(seed, i, side as i32) > 0.2 {
                parts.push(slab(Vec3::new(c.x + side * hx, h * 0.5, mid), Vec3::new(0.25, h * 0.5, 3.0), B::RustedSteel));
            }
            // Roof sheets on each pitch; about a third have fallen in.
            if roll(seed, i, 10 + side as i32) > 0.33 {
                let a = Vec3::new(c.x + side * (hx + 0.6), h + 0.25, mid);
                let b = Vec3::new(c.x, h + 4.4, mid);
                parts.push(Part::new(Shape::Sheet { a, b, half_width: 3.0, thick: 0.2, mat: B::SteelPlate }));
            } else {
                spots.push(Vec3::new(c.x + side * hx * 0.5, 0.6, mid));
            }
        }
    }
    // Back wall.
    parts.push(slab(Vec3::new(c.x, h * 0.5, c.y - hz), Vec3::new(hx, h * 0.5, 0.25), B::RustedSteel));
    // Fallen sheets lie where the roof failed; cargo cradles line the floor.
    for j in 0..4 {
        let at = Vec3::new(c.x + (roll(seed, j, 20) - 0.5) * hx * 1.4, 0.5, c.y + (roll(seed, j, 21) - 0.5) * hz * 1.6);
        parts.push(slab(at, Vec3::new(2.5, 0.3, 1.8), B::SteelPlate));
        spots.push(at + Vec3::Y * 0.6);
    }
    let footprint = (c - Vec2::new(hx + 1.0, hz + 1.0), c + Vec2::new(hx + 1.0, hz + 6.0));
    Kit { parts, footprint, spots }
}

/// A street-front depot or market hall. The canopy covers the loading apron,
/// while the rear wall carries the roof even where the outer panels have fallen.
pub(super) fn freight_shed(c: Vec2, seed: u32, frontage: f32, depot: bool) -> Kit {
    let mut parts = Vec::new();
    let at = |x, y, z| Vec3::new(c.x + x, y, c.y + z * frontage);
    let wall = if depot { B::RustedSteel } else { B::WeatheredBricks };
    let roof = if depot { B::SteelPlate } else { B::RoofTiles };
    for i in -2..=2 {
        let x = i as f32 * 5.0;
        for z in [-6.0, 4.0] {
            parts.push(slab(at(x, 3.0, z), Vec3::new(0.55, 3.0, 0.6), B::DressedStone));
        }
        parts.push(beam(at(x, 6.0, -6.0), at(x, 6.0, 8.0), 0.5, B::SteelPlate));
        parts.push(beam(at(x, 6.0, -6.0), at(x, 8.4, -1.0), 0.4, B::SteelPlate));
        parts.push(beam(at(x, 8.4, -1.0), at(x, 6.0, 4.0), 0.4, B::SteelPlate));
        if i == 2 {
            continue;
        }
        let bay = x + 2.5;
        if roll(seed, i, 201) > 0.23 {
            parts.push(slab(at(bay, 5.9, 6.1), Vec3::new(2.5, 0.35, 2.0), roof));
        }
        for (z, salt) in [(-6.0, 202), (4.0, 203)] {
            if roll(seed, i, salt) > 0.24 {
                parts.push(Part::new(Shape::Sheet {
                    a: at(bay, 6.15, z), b: at(bay, 8.6, -1.0),
                    half_width: 2.5, thick: 0.4, mat: roof,
                }));
            }
        }
        parts.push(slab(at(bay, 1.2, -6.0), Vec3::new(2.5, 1.2, 0.6), wall));
        if roll(seed, i, 204) > 0.38 {
            parts.push(slab(at(bay, 4.0, -6.0), Vec3::new(2.5, 1.5, 0.5), B::WindowGlass));
        }
        if i % 2 == 0 {
            parts.push(slab(at(bay, 0.6, -3.0), Vec3::new(1.4, 0.6, 1.0), wall));
        }
    }
    for x in [-10.0, 10.0] {
        parts.push(slab(at(x, 1.3, -1.0), Vec3::new(0.6, 1.3, 5.0), wall));
    }
    let footprint = (c - Vec2::new(11.0, 9.0), c + Vec2::new(11.0, 9.0));
    let spots = vec![at(-6.0, 0.5, -2.0), at(4.0, 0.5, 1.5), at(6.0, 0.5, -2.0)];
    Kit { parts, footprint, spots }
}

/// An open square with a ruined fountain and seats outside its walking loop.
pub(super) fn yard(c: Vec2, seed: u32, routed: bool) -> Kit {
    let mut parts = Vec::new();
    let at = |x, y, z| Vec3::new(c.x + x, y, c.y + z);
    parts.push(slab(at(0.0, -0.4, 0.0), Vec3::new(10.0, 0.4, 7.5), B::DressedStone));
    parts.push(slab(at(0.0, 0.7, 0.0), Vec3::new(1.1, 0.7, 1.1), B::CarvedStone));
    for side in [-1.0, 1.0] {
        parts.push(slab(at(0.0, 0.35, side * 3.0), Vec3::new(4.0, 0.35, 0.55), B::StoneBricks));
        parts.push(slab(at(side * 3.7, 0.35, 0.0), Vec3::new(0.55, 0.35, 2.6), B::StoneBricks));
        let x = side * 7.5;
        parts.push(slab(at(x, 0.35, -4.0), Vec3::new(1.8, 0.35, 1.8), B::WeatheredBricks));
        if routed {
            parts.push(Part::new(Shape::PlanterTree { base: at(x, 0.45, -4.0) }));
        } else {
            parts.push(beam(at(x, 0.5, -4.0), at(x, 6.5, -4.0), 0.5, B::Log));
            parts.push(Part::new(Shape::Blob {
                c: at(x, 6.2, -4.0), half: Vec3::new(3.0, 2.4, 2.9), mat: B::Leaves, seed,
            }));
        }
    }
    Kit {
        parts,
        footprint: (c - Vec2::new(11.0, 8.0), c + Vec2::new(11.0, 8.0)),
        spots: if routed { Vec::new() } else { vec![at(5.0, 0.5, 3.0)] },
    }
}

/// Storage faces the loading aisle. A few modules were stripped or never stacked.
pub(super) fn cargo_yard(c: Vec2, seed: u32) -> Kit {
    let mut parts = Vec::new();
    let mut spots = Vec::new();
    for side in [-1.0, 1.0] {
        for slot in 0..3 {
            let z = (slot as f32 - 1.0) * 5.1;
            let at = Vec3::new(c.x + side * 8.0, 0.0, c.y + z);
            let turn = if side < 0.0 { 0 } else { 2 };
            let pick = roll(seed, slot, side as i32 + 261);
            if pick < 0.15 { continue; }
            let mat = match (slot + i32::from(side < 0.0)) % 3 {
                0 => B::RustedSteel,
                1 => B::SteelPlate,
                _ => B::HazardPaint,
            };
            let open = pick > 0.48;
            parts.push(Part::new(Shape::Container { c: at, turn, mat, open }));
            if open {
                spots.push(at + Vec3::new(side * 3.5, 1.25, 0.0));
            }
            if pick > 0.65 {
                parts.push(Part::new(Shape::Container {
                    c: at + Vec3::Y * super::cargo::HEIGHT,
                    turn, mat: B::RustedSteel, open: false,
                }));
            }
        }
    }
    if roll(seed, 0, 269) > 0.35 {
        let at = |x, y, z| Vec3::new(c.x + x, y, c.y + z);
        for x in [-13.0, 13.0] {
            parts.push(beam(at(x, 0.0, 2.5), at(x, 10.4, 2.5), 0.6, B::RustedSteel));
            parts.push(beam(at(x, 8.0, 2.5), at(x * 0.74, 10.4, 2.5), 0.5, B::RustedSteel));
        }
        parts.push(beam(at(-13.0, 10.4, 2.5), at(13.0, 10.4, 2.5), 0.65, B::HazardPaint));
        let x = (roll(seed, 1, 269) - 0.5) * 5.0;
        parts.push(beam(at(x, 10.2, 2.5), at(x, 4.0, 2.5), 0.15, B::SteelPlate));
        parts.push(slab(at(x, 3.9, 2.5), Vec3::new(0.72, 0.45, 0.52), B::RustedSteel));
    }
    Kit { parts, footprint: (c - Vec2::new(14.0, 8.0), c + Vec2::new(14.0, 8.0)), spots }
}

/// A crystal regulator feeds the old tram supply through copper busbars.
pub(super) fn substation(c: Vec2) -> Kit {
    let at = |x, y, z| Vec3::new(c.x + x, y, c.y + z);
    let mut parts = vec![
        slab(at(0.0, 0.55, 0.0), Vec3::new(4.0, 0.55, 3.3), B::DressedStone),
        slab(at(0.0, 1.7, 0.0), Vec3::new(2.3, 0.75, 2.1), B::CarvedStone),
        slab(at(0.0, 3.0, 0.0), Vec3::new(0.65, 0.75, 0.65), B::Crystal),
    ];
    for side in [-1.0, 1.0] {
        let x = side * 3.0;
        parts.push(beam(at(x, 1.0, 0.0), at(x, 6.8, 0.0), 0.65, B::RustedSteel));
        parts.push(beam(at(x, 6.8, 0.0), at(side, 7.7, 0.0), 0.55, B::SteelPlate));
        for y in [3.2, 4.6, 6.0] {
            parts.push(slab(at(x, y, 0.0), Vec3::new(1.15, 0.28, 0.95), B::GlazedTiles));
        }
        parts.push(beam(at(x, 4.0, 0.0), at(side * 0.8, 3.0, 0.0), 0.35, B::BronzeBlock));
    }
    Kit {
        parts, footprint: (c - Vec2::new(5.0, 4.0), c + Vec2::new(5.0, 4.0)),
        spots: vec![at(0.0, 0.5, 3.6)],
    }
}

pub(super) fn street_unit(c: Vec2, side: f32, seed: u32, industrial: bool) -> Kit {
    let at = |x, y, z| Vec3::new(c.x + x * side, y, c.y + z);
    let wall = if industrial { B::RustedSteel } else { B::WeatheredBricks };
    let mut parts = vec![
        slab(at(3.2, 2.2, 0.0), Vec3::new(0.55, 2.2, 4.1), wall),
        slab(at(0.0, 4.6, 0.0), Vec3::new(3.8, 0.45, 4.4), B::SteelPlate),
        slab(at(-4.0, 3.3, 0.0), Vec3::new(1.4, 0.4, 4.3), B::RustedSteel),
    ];
    for end in [-1.0, 1.0] {
        parts.push(slab(at(0.0, 2.1, end * 3.9), Vec3::new(3.4, 2.1, 0.55), wall));
        parts.push(slab(at(-3.4, 1.65, end * 3.35), Vec3::new(0.5, 1.65, 0.55), B::Concrete));
        if roll(seed, end as i32, 271) > 0.55 {
            parts.push(slab(at(-3.35, 1.4, end * 1.9), Vec3::new(0.35, 1.4, 0.8), B::WindowGlass));
        }
    }
    let fascia = if industrial { B::HazardPaint } else { B::GlazedTiles };
    parts.push(slab(at(-4.9, 3.85, 0.0), Vec3::new(0.45, 0.48, 2.2), fascia));
    if roll(seed, 0, 272) < 0.7 {
        parts.push(Part::new(Shape::Sheet {
            a: at(-2.9, 5.15, 0.0), b: at(2.4, 6.05, 0.0),
            half_width: 2.4, thick: 0.25, mat: B::WindowGlass,
        }));
    }
    let (lo, hi) = (c - Vec2::new(5.5, 4.7), c + Vec2::new(5.5, 4.7));
    Kit { parts, footprint: (lo, hi), spots: Vec::new() }
}
