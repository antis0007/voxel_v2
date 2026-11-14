//! The district's megastructure: a tented A-frame spine carrying a deck, or a
//! hoop-pylon viaduct. Rebuilt transit has stations and deliberate termini;
//! interior breaks follow a failed frame or surveyed ground failure.
use super::shapes::{Part, Shape, roll};
use crate::BlockId as B;
use glam::{Vec2, Vec3};

/// Ground rectangles the towers must avoid, and the height they may reach.
pub(super) struct Mega {
    pub parts: Vec<Part>,
    pub blocked: Vec<(Vec2, Vec2)>,
    pub tent: Option<Tent>,
    /// Elevated walking surfaces (local metres) for sentries and caches.
    pub decks: Vec<Vec3>,
    /// Ground entrances to stations; layout joins these to actual streets.
    pub access: Vec<Vec2>,
}

/// The A-frame's legs and lattice lie in two sloped planes meeting at the apex.
pub(super) struct Tent {
    h: f32,
    w: f32,
    deck: f32,
    frames: Vec<f32>,
}

impl Tent {
    /// Highest tower top that keeps 3 m clear of every member.
    pub fn max_top(&self, lo: Vec2, hi: Vec2) -> f32 {
        let zmin = if lo.y <= 0.0 && hi.y >= 0.0 { 0.0 } else { lo.y.abs().min(hi.y.abs()) };
        let zmax = lo.y.abs().max(hi.y.abs());
        if zmin > self.w + 6.0 {
            return f32::INFINITY;
        }
        let mut top = self.h * (1.0 - (zmax + 6.0) / self.w);
        if zmin < 10.0 {
            top = top.min(self.deck - 4.0);
        }
        if self.frames.iter().any(|x| *x > lo.x - 5.0 && *x < hi.x + 5.0) {
            top = top.min(self.deck - 5.0);
        }
        top
    }
}

pub(super) fn beam(a: Vec3, b: Vec3, r: f32, mat: B, moss: bool) -> Part {
    Part::new(Shape::Beam { a, b, r, mat, moss })
}

pub(super) fn slab(c: Vec3, half: Vec3, mat: B, moss: bool) -> Part {
    Part::new(Shape::Slab { c, half, mat, moss })
}

fn vines(parts: &mut Vec<Part>, seed: u32, x0: f32, x1: f32, z: f32, top: f32, salt: i32) {
    let mut x = x0;
    let mut i = 0;
    while x < x1 {
        let r = roll(seed, i, salt);
        if r < 0.35 {
            parts.push(Part::new(Shape::Vine { x, z, top, len: 2.0 + r * 24.0 }));
        }
        x += 2.0 + roll(seed, i, salt + 1) * 2.4;
        i += 1;
    }
}

pub(super) fn a_frame(seed: u32, half: Vec2, rebuilt: bool, city: bool, _terraced: bool) -> Mega {
    let n = if city { super::urban::CITY_PIERS.len() } else { 3 + (roll(seed, 1, 0) * 3.0) as usize };
    let spacing = 34.0;
    let h = if city { 130.0 } else { 95.0 } + roll(seed, 2, 0) * 30.0;
    let w = (if city { 70.0 } else { 44.0 } + roll(seed, 3, 0) * 10.0).min(half.y - 12.0);
    let deck = if rebuilt { (0.42 * h / 4.0).round() * 4.0 - 1.2 } else { (0.42 * h).round() };
    let xs: Vec<f32> = (0..n).map(|i| {
        if city {
            super::urban::CITY_PIERS[i]
        } else if rebuilt {
            // Keep the main avenue and both side streets between footings.
            [-85.0, -35.0, -15.0, 25.0, 85.0][i + (5 - n) / 2]
        } else {
            (i as f32 - (n - 1) as f32 * 0.5) * spacing
        }
    }).collect();
    let broken = (n >= 4 && roll(seed, 4, 0) < 0.6).then(|| {
        if rebuilt { 1 } else { 1 + (roll(seed, 5, 0) * (n - 2) as f32) as usize }
    });
    let leg = |x: f32, side: f32, t: f32| Vec3::new(x, h * t, side * w * (1.0 - t));
    let mut parts = Vec::new();
    let mut blocked = Vec::new();
    for (i, &x) in xs.iter().enumerate() {
        let reach = if broken == Some(i) { 0.33 } else { 1.0 };
        for side in [-1.0, 1.0] {
            parts.push(beam(leg(x, side, 0.0), leg(x, side, reach), 2.6, B::Concrete, true));
            parts.push(slab(Vec3::new(x, 3.0, side * w), Vec3::new(5.5, 3.0, 5.5), B::Concrete, true));
            blocked.push((Vec2::new(x - 7.0, side * w - 7.0), Vec2::new(x + 7.0, side * w + 7.0)));
            if rebuilt {
                parts.push(slab(Vec3::new(x, 1.1, side * w), Vec3::new(7.0, 1.1, 6.5), B::WeatheredBricks, true));
                for t in [0.12, 0.37, 0.61, 0.83] {
                    if t < reach - 0.04 {
                        parts.push(beam(leg(x, side, t - 0.016), leg(x, side, t + 0.016), 3.1, B::RustedSteel, false));
                    }
                }
                if reach < 1.0 {
                    for dx in [-1.6, 1.6] {
                        let a = leg(x + dx, side, reach - 0.02);
                        parts.push(beam(a, a + Vec3::new(dx * 0.8, 5.0, side * 2.5), 0.3, B::RustedSteel, false));
                    }
                }
            }
        }
        if broken != Some(i) {
            let t = deck / h;
            parts.push(beam(leg(x, -1.0, t), leg(x, 1.0, t), 1.8, B::Concrete, true));
        }
        let Some(&next) = xs.get(i + 1) else { continue };
        if broken == Some(i) || broken == Some(i + 1) {
            continue;
        }
        // Rusting lattice in each sloped plane, a ridge chord along the apex.
        for side in [-1.0, 1.0] {
            for k in 0..4 {
                let (t0, t1) = (0.18 + k as f32 * 0.2, 0.38 + k as f32 * 0.2);
                parts.push(beam(leg(x, side, t0), leg(next, side, t0), 0.5, B::SteelPlate, false));
                parts.push(beam(leg(x, side, t0), leg(next, side, t1.min(0.98)), 0.4, B::RustedSteel, false));
                if rebuilt {
                    parts.push(beam(leg(x, side, t1.min(0.98)), leg(next, side, t0), 0.45, B::SteelPlate, false));
                    if k >= 1 && roll(seed, i as i32 * 9 + k, side as i32 + 240) > 0.66 {
                        // Surviving glazing hangs from the cross-bracing.
                        let mid = (x + next) * 0.5;
                        parts.push(Part::new(Shape::Sheet {
                            a: leg(mid, side, t0), b: leg(mid, side, t1.min(0.98)),
                            half_width: (next - x) * 0.28, thick: 0.45, mat: B::WindowGlass,
                        }));
                    }
                }
            }
        }
        parts.push(beam(leg(x, 1.0, 1.0), leg(next, 1.0, 1.0), 1.4, B::SteelPlate, false));
    }
    // The deck runs past the end frames and breaks where a frame came down.
    let (start, end) = (xs[0] - spacing * 0.6, xs[n - 1] + spacing * 0.6);
    let gap = broken.map(|i| (xs[i] - 11.0, xs[i] + 9.0));
    let spans = match gap {
        Some((g0, g1)) => vec![(start, g0), (g1, end)],
        None => vec![(start, end)],
    };
    let mut decks = Vec::new();
    let mut access = Vec::new();
    for (k, &(x0, x1)) in spans.iter().enumerate() {
        let stations: Vec<f32> = [(x0 == start).then_some(x0 + 17.0), (x1 == end).then_some(x1 - 17.0)]
            .into_iter().flatten().collect();
        let c = Vec3::new((x0 + x1) * 0.5, deck, 0.0);
        parts.push(slab(c, Vec3::new((x1 - x0) * 0.5, 1.2, 7.0), B::Concrete, true));
        for side in [-1.0, 1.0] {
            if !rebuilt {
                parts.push(slab(c + Vec3::new(0.0, 1.6, side * 6.8), Vec3::new((x1 - x0) * 0.5, 0.45, 0.2), B::SteelPlate, false));
            }
            // Strands start inside the slab edge so they hang from concrete, not air.
            vines(&mut parts, seed, x0, x1, side * 6.6, deck - 1.2, 10 + k as i32 * 4 + side as i32);
        }
        if rebuilt {
            super::mega_detail::track(&mut parts, x0..x1, 0.0, deck + 1.2, &stations, seed);
            for x in stations {
                super::mega_detail::service(&mut parts, &mut blocked, x, 0.0, deck + 1.2);
                super::mega_detail::stop(&mut parts, x, 0.0, deck + 1.2, seed);
                access.push(Vec2::new(x - 5.0, 8.4));
            }
            if x0 == start { super::mega_detail::buffer(&mut parts, x0, 1.0, 0.0, deck + 1.2); }
            if x1 == end { super::mega_detail::buffer(&mut parts, x1, -1.0, 0.0, deck + 1.2); }
        }
        decks.push(c + Vec3::Y * 1.3);
    }
    if let (Some((g0, g1)), Some(i)) = (gap, broken) {
        // Snapped beams hang off both ends of the gap; the fallen leg lies below.
        parts.push(beam(Vec3::new(g0, deck, -4.0), Vec3::new(g0 + 6.0, deck - 14.0, -5.0), 0.6, B::RustedSteel, false));
        parts.push(beam(Vec3::new(g1, deck, 3.0), Vec3::new(g1 - 5.0, deck - 11.0, 4.0), 0.6, B::RustedSteel, false));
        let x = xs[i];
        parts.push(beam(Vec3::new(x + 4.0, 2.4, -w * 0.4), Vec3::new(x + 18.0, 2.4, w * 0.55), 2.6, B::Concrete, true));
        for j in 0..5 {
            let at = Vec3::new(x - 6.0 + roll(seed, j, 30) * 24.0, 0.8, -12.0 + roll(seed, j, 31) * 24.0);
            parts.push(Part::new(Shape::Blob { c: at, half: Vec3::new(3.5, 1.8, 3.0), mat: B::Concrete, seed: seed ^ j as u32 }));
        }
        blocked.push((Vec2::new(x - 10.0, -w * 0.6), Vec2::new(x + 24.0, w * 0.7)));
    }
    let tent = Tent { h, w, deck, frames: xs };
    Mega { parts, blocked, tent: Some(tent), decks, access }
}

pub(super) fn viaduct(seed: u32, half: Vec2, z: f32, rebuilt: bool, city: bool, terraced: bool, line: Option<super::line::Line>) -> Mega {
    if rebuilt {
        return super::mega_detail::viaduct(seed, half, z, city, terraced, line);
    }
    let deck = (30.0 + roll(seed, 6, 0) * 10.0).round();
    let reach = half.x + 30.0 + roll(seed, 7, 0) * 20.0;
    let mut parts = Vec::new();
    // Hoop pylons: two flat legs and a half-ring with a painted inner band.
    let mut x = -reach + 12.0;
    while x < reach - 8.0 {
        // Legs run 40 m below the deck datum so they reach ground below a bluff.
        let (bottom, top) = (-40.0, deck + 10.0);
        for side in [-1.0, 1.0] {
            let c = Vec3::new(x, (bottom + top) * 0.5, z + side * 9.0);
            parts.push(slab(c, Vec3::new(2.0, (top - bottom) * 0.5, 1.6), B::Concrete, false));
        }
        let crown = Vec3::new(x, top, z);
        parts.push(Part::new(Shape::Arch { c: crown, radius: 9.0, r: 1.7, depth: 2.0, along_x: false, mat: B::Concrete }));
        parts.push(Part::new(Shape::Arch { c: crown, radius: 7.1, r: 0.25, depth: 2.1, along_x: false, mat: B::HazardPaint }));
        x += 44.0;
    }
    let ends = (-reach + roll(seed, 8, 0) * 10.0, reach - roll(seed, 9, 0) * 10.0);
    let c = Vec3::new((ends.0 + ends.1) * 0.5, deck, z);
    let span = (ends.1 - ends.0) * 0.5;
    parts.push(slab(c, Vec3::new(span, 1.3, 6.5), B::Concrete, true));
    for side in [-1.0, 1.0] {
        parts.push(slab(c + Vec3::new(0.0, 1.8, side * 6.3), Vec3::new(span, 0.5, 0.2), B::HazardPaint, false));
        vines(&mut parts, seed, ends.0, ends.1, z + side * 6.1, deck - 1.3, 40 + side as i32);
    }
    // A service main rides the south edge on brackets.
    let pipe = Vec3::new(0.0, deck - 3.5, z + 8.2);
    parts.push(beam(pipe + Vec3::X * ends.0, pipe + Vec3::X * ends.1, 1.1, B::RustedSteel, false));
    let mut bx = ends.0 + 6.0;
    while bx < ends.1 {
        parts.push(slab(Vec3::new(bx, deck - 2.0, z + 7.4), Vec3::new(0.3, 1.1, 1.4), B::SteelPlate, false));
        bx += 9.0;
    }
    // Rebar and torn beams droop from both broken ends.
    for (k, end) in [ends.0, ends.1].into_iter().enumerate() {
        let out = if k == 0 { -1.0 } else { 1.0 };
        for j in 0..3 {
            let zz = z - 4.0 + j as f32 * 4.0;
            let drop = 6.0 + roll(seed, j, 50 + k as i32) * 9.0;
            parts.push(beam(Vec3::new(end, deck, zz), Vec3::new(end + out * 3.0, deck - drop, zz + 1.0), 0.3, B::RustedSteel, false));
        }
    }
    let blocked = vec![(Vec2::new(-reach, z - 12.0), Vec2::new(reach, z + 12.0))];
    let decks = vec![c + Vec3::new(-span * 0.5, 1.4, 0.0), c + Vec3::Y * 1.4, c + Vec3::new(span * 0.5, 1.4, 0.0)];
    Mega { parts, blocked, tent: None, decks, access: Vec::new() }
}
