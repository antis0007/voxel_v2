//! Generator 16 architecture. Buildings are cached blueprints (a handful of wings,
//! towers and curtain walls) that are sampled one voxel at a time. Every
//! dimension is in metres and every thickness is clamped to the voxel size so a
//! wall is one voxel thick at 1 m scale and still a wall at 0.25 m scale.
//!
//! A wing is a grid of cells. A recursive partition assigns each cell a room,
//! a spanning tree of doorways connects the rooms, switchback stairs link the
//! storeys, and a roof closes the top. Age erodes roofs and top walls so ruins
//! stay roofed but weathered.
use super::{BlockId as B, hash2, value_noise2};
use glam::Vec3;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Roof {
    Gable,
    Hip,
    Flat,
    Lean,
    Glass,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Windows {
    None,
    Panes,
    Slits,
    Glazed,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub wall: B,
    pub frame: B,
    pub plinth: B,
    pub floor: B,
    pub roof: B,
    pub glass: B,
}

impl Palette {
    pub const TIMBER: Palette = Palette {
        wall: B::Planks,
        frame: B::Log,
        plinth: B::WeatheredBricks,
        floor: B::Planks,
        roof: B::Planks,
        glass: B::Ice,
    };
    pub const STONE: Palette = Palette {
        wall: B::StoneBricks,
        frame: B::Cobblestone,
        plinth: B::WeatheredBricks,
        floor: B::StoneBricks,
        roof: B::Basalt,
        glass: B::Ice,
    };
    pub const LIMESTONE: Palette = Palette {
        wall: B::Limestone,
        frame: B::StoneBricks,
        plinth: B::WeatheredBricks,
        floor: B::Cobblestone,
        roof: B::Basalt,
        glass: B::Ice,
    };
    pub const GLASSHOUSE: Palette = Palette {
        wall: B::Limestone,
        frame: B::Log,
        plinth: B::WeatheredBricks,
        floor: B::Cobblestone,
        roof: B::Log,
        glass: B::Ice,
    };
}

/// A ground-floor opening in an exterior face. Sides: 0 = -z, 1 = +z, 2 = -x,
/// 3 = +x. `u` runs along the face in wing-local metres.
#[derive(Clone, Copy)]
pub struct Door {
    pub side: u8,
    pub u: f32,
    pub hw: f32,
    pub height: f32,
}

pub struct WingSpec {
    pub x0: f32,
    pub z0: f32,
    pub nx: usize,
    pub nz: usize,
    pub cell: f32,
    pub storeys: usize,
    pub h: f32,
    pub roof: Roof,
    pub pal: Palette,
    pub windows: Windows,
    pub seed: u32,
    pub age: f32,
    pub chimney: bool,
    pub open: bool,
    pub min_room: usize,
    pub doors: Vec<Door>,
    pub keep: Vec<(f32, f32)>,
}

impl WingSpec {
    pub fn new(x0: f32, z0: f32, nx: usize, nz: usize, seed: u32) -> Self {
        Self {
            x0,
            z0,
            nx,
            nz,
            cell: 3.6,
            storeys: 1,
            h: 3.6,
            roof: Roof::Gable,
            pal: Palette::TIMBER,
            windows: Windows::Panes,
            seed,
            age: 0.0,
            chimney: false,
            open: false,
            min_room: 5,
            doors: Vec::new(),
            keep: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Stair {
    sx: usize,
    ra: usize,
    len: usize,
}

pub struct Wing {
    pub x0: f32,
    pub z0: f32,
    pub nx: usize,
    pub nz: usize,
    pub cell: f32,
    pub storeys: usize,
    pub h: f32,
    roof: Roof,
    pal: Palette,
    windows: Windows,
    seed: u32,
    age: f32,
    chimney: bool,
    doors: Vec<Door>,
    keep: Vec<(f32, f32)>,
    rooms: Vec<Vec<u8>>,
    door_x: Vec<Vec<bool>>,
    door_z: Vec<Vec<bool>>,
    stair: Option<Stair>,
}

/// Recursive binary partition of a cell grid; leaves become rooms.
fn partition(nx: usize, nz: usize, seed: u32, min_room: usize) -> Vec<u8> {
    fn split(
        ids: &mut [u8],
        nx: usize,
        rect: (usize, usize, usize, usize),
        seed: u32,
        depth: u32,
        min_room: usize,
        next: &mut u8,
    ) {
        let (x0, x1, z0, z1) = rect;
        let (w, d) = (x1 - x0, z1 - z0);
        let area = w * d;
        let r = hash2((x0 * 31 + z0 * 17) as i32, depth as i32, seed);
        let leaf = area <= min_room
            || depth > 4
            || (w < 2 && d < 2)
            || (area <= min_room * 2 && r < 0.45)
            || (w < 2 && area <= min_room * 3);
        if leaf {
            for z in z0..z1 {
                for x in x0..x1 {
                    ids[z * nx + x] = *next;
                }
            }
            *next = next.saturating_add(1);
            return;
        }
        let cut_x = if w > d {
            true
        } else if d > w {
            false
        } else {
            r < 0.5
        };
        let r2 = hash2(
            (x0 * 13 + z1 * 7) as i32,
            depth as i32 + 9,
            seed.wrapping_add(5),
        );
        if cut_x && w >= 2 {
            let c = x0 + 1 + (r2 * (w - 1) as f32) as usize % (w - 1);
            split(ids, nx, (x0, c, z0, z1), seed, depth + 1, min_room, next);
            split(ids, nx, (c, x1, z0, z1), seed, depth + 1, min_room, next);
        } else if d >= 2 {
            let c = z0 + 1 + (r2 * (d - 1) as f32) as usize % (d - 1);
            split(ids, nx, (x0, x1, z0, c), seed, depth + 1, min_room, next);
            split(ids, nx, (x0, x1, c, z1), seed, depth + 1, min_room, next);
        } else {
            for z in z0..z1 {
                for x in x0..x1 {
                    ids[z * nx + x] = *next;
                }
            }
            *next = next.saturating_add(1);
        }
    }
    let mut ids = vec![0u8; nx * nz];
    let mut next = 0u8;
    split(
        &mut ids,
        nx,
        (0, nx, 0, nz),
        seed,
        0,
        min_room.max(2),
        &mut next,
    );
    ids
}

/// Give every connected same-id region its own id. Returns the room count.
fn relabel(ids: &mut [u8], nx: usize, nz: usize) -> usize {
    let mut out = vec![u8::MAX; nx * nz];
    let mut count = 0usize;
    for start in 0..nx * nz {
        if out[start] != u8::MAX {
            continue;
        }
        let id = ids[start];
        let label = count.min(254) as u8;
        count += 1;
        let mut stack = vec![start];
        out[start] = label;
        while let Some(i) = stack.pop() {
            let (x, z) = (i % nx, i / nx);
            let mut push = |j: usize| {
                if out[j] == u8::MAX && ids[j] == id {
                    out[j] = label;
                    stack.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < nx {
                push(i + 1);
            }
            if z > 0 {
                push(i - nx);
            }
            if z + 1 < nz {
                push(i + nx);
            }
        }
    }
    ids.copy_from_slice(&out);
    count.min(255)
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

impl Wing {
    pub fn new(spec: WingSpec) -> Wing {
        let WingSpec { nx, nz, seed, .. } = spec;
        let can_stair = spec.storeys > 1 && nx >= 2 && nz >= 2;
        let storeys = if can_stair { spec.storeys } else { 1 };
        let stair = can_stair.then(|| {
            let len = if nx >= 6 { 3 } else { 2 };
            Stair {
                sx: (hash2(1, 2, seed.wrapping_add(3)) * (nx - len + 1) as f32) as usize
                    % (nx - len + 1),
                ra: (hash2(4, 5, seed.wrapping_add(6)) * (nz - 1) as f32) as usize % (nz - 1),
                len,
            }
        });
        let mut rooms = Vec::new();
        let mut door_x = Vec::new();
        let mut door_z = Vec::new();
        for s in 0..storeys {
            let sseed = seed.wrapping_add(s as u32 * 977);
            let mut ids = if spec.open {
                vec![0u8; nx * nz]
            } else {
                partition(nx, nz, sseed, spec.min_room)
            };
            if let Some(st) = stair {
                let first = ids[st.ra * nx + st.sx];
                for z in st.ra..=st.ra + 1 {
                    for x in st.sx..st.sx + st.len {
                        ids[z * nx + x] = first;
                    }
                }
            }
            let count = relabel(&mut ids, nx, nz);
            let mut dx = vec![false; nz * (nx + 1)];
            let mut dz = vec![false; (nz + 1) * nx];
            // Candidate edges between different rooms, visited in hashed order.
            let mut edges: Vec<(f32, bool, usize, usize, usize)> = Vec::new();
            for z in 0..nz {
                for e in 1..nx {
                    let (a, b) = (ids[z * nx + e - 1], ids[z * nx + e]);
                    if a != b {
                        edges.push((
                            hash2(e as i32, z as i32, sseed.wrapping_add(11)),
                            true,
                            e,
                            z,
                            a as usize * 256 + b as usize,
                        ));
                    }
                }
            }
            for e in 1..nz {
                for x in 0..nx {
                    let (a, b) = (ids[(e - 1) * nx + x], ids[e * nx + x]);
                    if a != b {
                        edges.push((
                            hash2(x as i32, e as i32, sseed.wrapping_add(13)),
                            false,
                            x,
                            e,
                            a as usize * 256 + b as usize,
                        ));
                    }
                }
            }
            edges.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut parent: Vec<usize> = (0..count.max(1)).collect();
            for (n, (_, along_x, a, b, pair)) in edges.iter().copied().enumerate() {
                let (ra, rb) = (pair / 256, pair % 256);
                let (fa, fb) = (find(&mut parent, ra), find(&mut parent, rb));
                let extra = hash2(n as i32, 3, sseed.wrapping_add(17)) < 0.16;
                if fa != fb || extra {
                    if fa != fb {
                        parent[fa] = fb;
                    }
                    if along_x {
                        dx[b * (nx + 1) + a] = true;
                    } else {
                        dz[a + b * nx] = true;
                    }
                }
            }
            rooms.push(ids);
            door_x.push(dx);
            door_z.push(dz);
        }
        Wing {
            x0: spec.x0,
            z0: spec.z0,
            nx,
            nz,
            cell: spec.cell,
            storeys,
            h: spec.h,
            roof: spec.roof,
            pal: spec.pal,
            windows: spec.windows,
            seed,
            age: spec.age,
            chimney: spec.chimney,
            doors: spec.doors,
            keep: spec.keep,
            rooms,
            door_x,
            door_z,
            stair,
        }
    }

    pub fn width(&self) -> f32 {
        self.nx as f32 * self.cell
    }

    pub fn depth(&self) -> f32 {
        self.nz as f32 * self.cell
    }

    pub fn top(&self) -> f32 {
        self.storeys as f32 * self.h
    }

    fn kept(&self, p: Vec3) -> bool {
        self.keep
            .iter()
            .any(|(kx, kz)| (p.x - kx).hypot(p.z - kz) < 1.5)
    }

    fn erosion(&self, p: Vec3, fy: f32, s: usize) -> bool {
        if self.age <= 0.0 || s + 1 != self.storeys {
            return false;
        }
        let n = value_noise2(p.x * 0.4 + 3.1, p.z * 0.4 + 7.7, self.seed);
        fy > self.h * (1.0 - self.age * 1.3 * n)
    }

    fn flight(&self, t: usize, p: Vec3, slab: f32) -> Option<(f32, f32)> {
        let st = self.stair?;
        let row = if t.is_multiple_of(2) { st.ra } else { st.ra + 1 };
        let up = t.is_multiple_of(2);
        let (xa, xb) = (
            st.sx as f32 * self.cell,
            (st.sx + st.len) as f32 * self.cell,
        );
        let (za, zb) = (row as f32 * self.cell, (row + 1) as f32 * self.cell);
        if p.x < xa || p.x >= xb || p.z < za || p.z >= zb {
            return None;
        }
        let frac = if up {
            (p.x - xa) / (xb - xa)
        } else {
            (xb - p.x) / (xb - xa)
        };
        Some((t as f32 * self.h + slab + frac * self.h, frac))
    }

    fn stair_hole(&self, s: usize, p: Vec3, slab: f32) -> bool {
        if s == 0 {
            return false;
        }
        match self.flight(s - 1, p, slab) {
            Some((_, frac)) => frac > (self.h - slab - 2.2) / self.h,
            None => false,
        }
    }

    fn in_stair_zone(&self, cx: usize, cz: usize) -> bool {
        self.stair.is_some_and(|st| {
            cx >= st.sx && cx < st.sx + st.len && (cz == st.ra || cz == st.ra + 1)
        })
    }

    /// `th` is the natural terrain surface in wing-local metres; foundations stop there.
    pub fn sample(&self, p: Vec3, v: f32, th: f32) -> Option<B> {
        let (w, d) = (self.width(), self.depth());
        let eave = match self.roof {
            Roof::Gable | Roof::Hip | Roof::Glass => v.max(0.6),
            _ => 0.0,
        };
        if p.x < -eave || p.x > w + eave || p.z < -eave || p.z > d + eave {
            return None;
        }
        let inside = p.x >= 0.0 && p.x < w && p.z >= 0.0 && p.z < d;
        if p.y < 0.0 {
            return (inside && p.y > -FOUNDATION && p.y >= th).then_some(B::WeatheredBricks);
        }
        let slab = v.max(0.4);
        let top = self.top();
        if p.y >= top {
            return self.roof_block(p, v, w, d, top, eave, inside);
        }
        if !inside {
            return None;
        }
        if self.chimney
            && let Some(b) = self.chimney_block(p, v, w, d, top) {
                return Some(b);
            }
        let s = ((p.y / self.h) as usize).min(self.storeys - 1);
        let fy = p.y - s as f32 * self.h;
        let rel = fy - slab;
        let tread = v.max(0.5);
        for t in [s.wrapping_sub(1), s] {
            if t >= self.storeys - 1 || t == usize::MAX {
                continue;
            }
            if let Some((top_y, _)) = self.flight(t, p, slab)
                && p.y <= top_y && p.y > top_y - tread {
                    return Some(B::Planks);
                }
        }
        let wt = v.max(0.6);
        let (ex, ez) = (p.x.min(w - p.x), p.z.min(d - p.z));
        if ex < wt || ez < wt {
            return self.exterior(p, v, s, fy, rel, slab, ex, ez, wt, w, d);
        }
        let wti = v.max(0.5);
        let cx = ((p.x / self.cell) as usize).min(self.nx - 1);
        let cz = ((p.z / self.cell) as usize).min(self.nz - 1);
        let door_h = (2.3f32).max(v * 2.0);
        let door_hw = (0.9f32).max(v * 0.75);
        let ids = &self.rooms[s];
        let e = (p.x / self.cell).round() as isize;
        if e >= 1 && (e as usize) < self.nx && (p.x - e as f32 * self.cell).abs() < wti * 0.5 {
            let e = e as usize;
            if ids[cz * self.nx + e - 1] != ids[cz * self.nx + e] && !self.kept(p) {
                let u = p.z - cz as f32 * self.cell;
                let open = self.door_x[s][cz * (self.nx + 1) + e]
                    && (u - self.cell * 0.5).abs() < door_hw
                    && fy >= slab
                    && rel < door_h;
                if !open && !self.erosion(p, fy, s) {
                    return Some(self.pal.wall);
                }
            }
        }
        let e = (p.z / self.cell).round() as isize;
        if e >= 1 && (e as usize) < self.nz && (p.z - e as f32 * self.cell).abs() < wti * 0.5 {
            let e = e as usize;
            if ids[(e - 1) * self.nx + cx] != ids[e * self.nx + cx] && !self.kept(p) {
                let u = p.x - cx as f32 * self.cell;
                let open = self.door_z[s][e * self.nx + cx]
                    && (u - self.cell * 0.5).abs() < door_hw
                    && fy >= slab
                    && rel < door_h;
                if !open && !self.erosion(p, fy, s) {
                    return Some(self.pal.wall);
                }
            }
        }
        if fy < slab {
            if self.stair_hole(s, p, slab) {
                return Some(B::Air);
            }
            return Some(if s == 0 { self.pal.floor } else { B::Planks });
        }
        if let Some(b) = self.furnish(s, cx, cz, p, rel, v) {
            return Some(b);
        }
        Some(B::Air)
    }

    #[allow(clippy::too_many_arguments)]
    fn exterior(
        &self,
        p: Vec3,
        v: f32,
        s: usize,
        fy: f32,
        rel: f32,
        slab: f32,
        ex: f32,
        ez: f32,
        wt: f32,
        w: f32,
        d: f32,
    ) -> Option<B> {
        if self.erosion(p, fy, s) {
            return Some(B::Air);
        }
        let (side, u) = if ex <= ez {
            (if p.x < w * 0.5 { 2u8 } else { 3u8 }, p.z)
        } else {
            (if p.z < d * 0.5 { 0u8 } else { 1u8 }, p.x)
        };
        let corner = ex < wt && ez < wt;
        if s == 0 && fy >= slab && !corner {
            for door in &self.doors {
                let arch = (((u - door.u) / door.hw.max(0.01)).powi(2)) * 0.5;
                if door.side == side
                    && (u - door.u).abs() < door.hw.max(v * 0.75)
                    && rel < door.height - arch
                {
                    return Some(B::Air);
                }
            }
        }
        let along = u % self.cell;
        let centre = (along - self.cell * 0.5).abs();
        if !corner && fy >= slab {
            match self.windows {
                Windows::Panes => {
                    if rel > 0.9 && rel < 2.3 && centre < v.max(0.55) {
                        return Some(B::Air);
                    }
                }
                Windows::Slits => {
                    if rel > 0.9 && rel < 2.7 && centre < v.max(0.22) {
                        return Some(B::Air);
                    }
                }
                Windows::Glazed => {
                    if rel > 1.0 {
                        let mullion =
                            along < v.max(0.3) * 0.5 || along > self.cell - v.max(0.3) * 0.5;
                        return Some(if mullion {
                            self.pal.frame
                        } else {
                            self.pal.glass
                        });
                    }
                }
                Windows::None => {}
            }
        }
        if corner {
            return Some(self.pal.frame);
        }
        if s == 0 && rel < 0.9 {
            return Some(self.pal.plinth);
        }
        Some(self.pal.wall)
    }

    fn furnish(&self, s: usize, cx: usize, cz: usize, p: Vec3, rel: f32, v: f32) -> Option<B> {
        if self.in_stair_zone(cx, cz) {
            return None;
        }
        let roll = hash2(cx as i32 * 7 + s as i32 * 13, cz as i32, self.seed ^ 0xF00D);
        let u = p.x - cx as f32 * self.cell;
        let wv = p.z - cz as f32 * self.cell;
        let half = self.cell * 0.5;
        let body = v.max(0.9);
        if roll < 0.2 {
            if (u - half).abs() < 0.8 && (wv - half).abs() < 0.5 && rel < body {
                return Some(B::Planks);
            }
        } else if roll < 0.38 {
            if wv < 0.5 + v * 0.5
                && (u < half - 1.0 || u > half + 1.0)
                && u > 0.6
                && u < self.cell - 0.6
                && rel < 2.0f32.max(v * 2.0)
            {
                return Some(B::Planks);
            }
        } else if roll < 0.48 {
            if u > 0.7 && u < 0.7 + v.max(0.9) && wv > 0.7 && wv < 0.7 + v.max(0.9) && rel < body {
                return Some(B::SalvageCrate);
            }
        } else if roll < 0.64 {
            // A standing torch lights the room; interiors are otherwise dark.
            if u > 0.55 && u < 0.55 + v && wv > 0.55 && wv < 0.55 + v && rel < v {
                return Some(B::Torch);
            }
        }
        None
    }

    fn chimney_block(&self, p: Vec3, v: f32, w: f32, d: f32, top: f32) -> Option<B> {
        let (cx, cz) = if w >= d {
            (w * 0.75, d * 0.5)
        } else {
            (w * 0.5, d * 0.75)
        };
        let cw = v.max(1.2) * 0.5;
        if (p.x - cx).abs() < cw && (p.z - cz).abs() < cw && p.y < top + 3.0 {
            return Some(if p.y > top + 2.2 {
                B::Cobblestone
            } else {
                B::StoneBricks
            });
        }
        None
    }

    fn roof_block(
        &self,
        p: Vec3,
        v: f32,
        w: f32,
        d: f32,
        top: f32,
        eave: f32,
        inside: bool,
    ) -> Option<B> {
        let ry = p.y - top;
        let wt = v.max(0.6);
        if self.roof == Roof::Flat {
            if !inside {
                return None;
            }
            let thick = v.max(0.5);
            if ry < thick {
                return Some(self.pal.roof);
            }
            let edge = p.x.min(w - p.x).min(p.z).min(d - p.z).min(p.z.min(d - p.z));
            let edge = edge.min(p.x.min(w - p.x));
            if edge < wt {
                let merlon = (((p.x / 1.4).floor() + (p.z / 1.4).floor()) as i32 & 1) == 0;
                let cap = thick + if merlon { 1.2f32.max(v) } else { 0.5 };
                if ry < cap {
                    return Some(self.pal.wall);
                }
            }
            return None;
        }
        // Steep over narrow spans, but never more than ~4.5 m of ridge on a wide hall.
        let half_span = if w >= d { d } else { w } * 0.5;
        let slope = match self.roof {
            Roof::Gable | Roof::Glass => 0.8f32.min(4.5 / half_span.max(1.0)),
            Roof::Hip => 0.7f32.min(4.5 / half_span.max(1.0)),
            _ => 0.3,
        };
        let base = 0.4;
        let (rs, along, len, ac) = match self.roof {
            Roof::Hip => {
                let dx = (p.x + eave).min(w + eave - p.x);
                let dz = (p.z + eave).min(d + eave - p.z);
                (base + dx.min(dz) * slope, 0.0, 0.0, 0.0)
            }
            Roof::Lean => (base + (d + eave - p.z).max(0.0) * slope, 0.0, 0.0, 0.0),
            _ => {
                let (across, span, along, len) = if w >= d {
                    (p.z, d, p.x, w)
                } else {
                    (p.x, w, p.z, d)
                };
                let ac = (across - span * 0.5).abs();
                (base + (span * 0.5 + eave - ac) * slope, along, len, ac)
            }
        };
        if ry > rs {
            return None;
        }
        let thick = (v * (1.0 + slope)).max(0.55);
        if ry >= rs - thick {
            if self.age > 0.0 && inside {
                let n = value_noise2(p.x * 0.5 + 11.0, p.z * 0.5 - 4.0, self.seed ^ 0xA5);
                if n > 1.0 - self.age * 0.9 {
                    return Some(B::Air);
                }
            }
            if self.roof == Roof::Glass {
                let rib = (along % 2.4) < v.max(0.3) || ac < v.max(0.3) * 0.7;
                return Some(if rib { self.pal.roof } else { self.pal.glass });
            }
            return Some(self.pal.roof);
        }
        if !inside {
            return None;
        }
        if matches!(self.roof, Roof::Gable | Roof::Glass) && (along < wt || along > len - wt) {
            return Some(if self.roof == Roof::Glass {
                self.pal.glass
            } else {
                self.pal.wall
            });
        }
        Some(B::Air)
    }
}

/// A crenellated curtain wall around a courtyard, optionally pierced by a gate.
pub struct Curtain {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub t: f32,
    pub h: f32,
    pub mat: B,
    /// (side, u, half width, height); sides as for `Door`.
    pub gate: Option<(u8, f32, f32, f32)>,
}

impl Curtain {
    pub fn sample(&self, p: Vec3, v: f32, th: f32) -> Option<B> {
        if p.x < self.x0 || p.x > self.x1 || p.z < self.z0 || p.z > self.z1 {
            return None;
        }
        let t = self.t.max(v * 2.0);
        let pt = v.max(0.6);
        let ring = p.x - self.x0 < t || self.x1 - p.x < t || p.z - self.z0 < t || self.z1 - p.z < t;
        if !ring {
            return None;
        }
        if p.y < 0.0 {
            return (p.y > -FOUNDATION && p.y >= th).then_some(B::WeatheredBricks);
        }
        if let Some((side, u, hw, gh)) = self.gate {
            let (face, along) = match side {
                0 | 1 => (
                    if side == 0 {
                        p.z - self.z0
                    } else {
                        self.z1 - p.z
                    },
                    p.x,
                ),
                _ => (
                    if side == 2 {
                        p.x - self.x0
                    } else {
                        self.x1 - p.x
                    },
                    p.z,
                ),
            };
            let arch = ((along - u) / hw.max(0.01)).powi(2) * 0.8;
            if face < t && (along - u).abs() < hw && p.y < gh - arch {
                return Some(B::Air);
            }
        }
        let outer = (p.x - self.x0)
            .min(self.x1 - p.x)
            .min(p.z - self.z0)
            .min(self.z1 - p.z);
        if outer < pt {
            let merlon = (((p.x / 1.5).floor() + (p.z / 1.5).floor()) as i32 & 1) == 0;
            let cap = self.h + if merlon { 1.4f32.max(v) } else { 0.5 };
            if p.y < cap {
                return Some(self.mat);
            }
        } else if p.y < self.h {
            return Some(if p.y > self.h - v.max(0.4) {
                B::Cobblestone
            } else {
                self.mat
            });
        }
        None
    }
}

pub struct Building {
    pub wings: Vec<Wing>,
    pub curtains: Vec<Curtain>,
    /// Optional levelled plot (x0, x1, z0, z1, floor material, clear height).
    pub plot: Option<(f32, f32, f32, f32, B, f32)>,
    min: (f32, f32),
    max: (f32, f32),
    top: f32,
}

impl Building {
    pub fn new(
        wings: Vec<Wing>,
        curtains: Vec<Curtain>,
        plot: Option<(f32, f32, f32, f32, B, f32)>,
    ) -> Self {
        let mut min = (f32::MAX, f32::MAX);
        let mut max = (f32::MIN, f32::MIN);
        let mut top = 0.0f32;
        let mut grow = |a: f32, b: f32, c: f32, d: f32, t: f32| {
            min = (min.0.min(a), min.1.min(b));
            max = (max.0.max(c), max.1.max(d));
            top = top.max(t);
        };
        for w in &wings {
            grow(
                w.x0 - 1.0,
                w.z0 - 1.0,
                w.x0 + w.width() + 1.0,
                w.z0 + w.depth() + 1.0,
                w.top() + 8.0,
            );
        }
        for c in &curtains {
            grow(c.x0, c.z0, c.x1, c.z1, c.h + 2.0);
        }
        if let Some((a, b, c, d, _, h)) = plot {
            grow(a - SKIRT, c - SKIRT, b + SKIRT, d + SKIRT, h);
        }
        Self {
            wings,
            curtains,
            plot,
            min,
            max,
            top,
        }
    }

    pub fn contains_xz(&self, x: f32, z: f32) -> bool {
        x >= self.min.0 && x <= self.max.0 && z >= self.min.1 && z <= self.max.1
    }

    pub fn extent(&self) -> (f32, f32, f32, f32) {
        (self.min.0, self.min.1, self.max.0, self.max.1)
    }

    pub fn sample(&self, q: Vec3, v: f32, th: f32) -> Option<B> {
        if !self.contains_xz(q.x, q.z) || q.y > self.top || q.y < -FOUNDATION {
            return None;
        }
        let mut void = false;
        for c in &self.curtains {
            match c.sample(q, v, th) {
                Some(B::Air) => void = true,
                Some(b) => return Some(b),
                None => {}
            }
        }
        for w in &self.wings {
            match w.sample(Vec3::new(q.x - w.x0, q.y, q.z - w.z0), v, th) {
                Some(B::Air) => void = true,
                Some(b) => return Some(b),
                None => {}
            }
        }
        if void {
            return Some(B::Air);
        }
        if let Some((x0, x1, z0, z1, mat, clear)) = self.plot {
            let ox = (x0 - q.x).max(q.x - x1).max(0.0);
            let oz = (z0 - q.z).max(q.z - z1).max(0.0);
            if ox == 0.0 && oz == 0.0 {
                if q.y < 0.0 {
                    return (q.y > -FOUNDATION && q.y >= th).then_some(B::WeatheredBricks);
                }
                if q.y < v {
                    return Some(mat);
                }
                if q.y < clear.max(th + v) {
                    return Some(B::Air);
                }
            } else {
                // Embankment below the floor line, cutting above it, both rising 1:1 from the plot edge.
                let dist = ox.hypot(oz);
                if dist < SKIRT {
                    if q.y < -dist {
                        if q.y >= th && q.y > -FOUNDATION {
                            return Some(if q.y >= -dist - v { B::Grass } else { B::Dirt });
                        }
                    } else if q.y >= v.max(dist) && q.y < th + v {
                        return Some(B::Air);
                    }
                }
            }
        }
        None
    }
}

/// Deepest fill below a floor. Foundations only fill the gap down to the natural terrain.
const FOUNDATION: f32 = 40.0;
/// A levelled plot blends into the hillside over this distance at 45 degrees.
const SKIRT: f32 = 16.0;

type SiteKey = (u32, u32, bool);
type SiteSlot = Option<Rc<Building>>;

thread_local! {
    static SITE_BUILDINGS: RefCell<HashMap<SiteKey, SiteSlot>> = RefCell::new(HashMap::new());
    /// Chunks sample a column of voxels per site, so the previous hit nearly always repeats.
    static LAST_SITE: RefCell<Option<(SiteKey, SiteSlot)>> = const { RefCell::new(None) };
}

fn door(side: u8, u: f32, hw: f32, height: f32) -> Door {
    Door {
        side,
        u,
        hw,
        height,
    }
}

fn spec(x0: f32, z0: f32, nx: usize, nz: usize, seed: u32, f: impl FnOnce(&mut WingSpec)) -> Wing {
    let mut s = WingSpec::new(x0, z0, nx, nz, seed);
    f(&mut s);
    Wing::new(s)
}

/// Ruin-site buildings. Kind 1 (the watchtower) keeps its bespoke round tower.
/// Coordinates are site-local metres before the site's quarter-turn.
pub fn site_building(kind: u32, seed: u32, starter: bool) -> Option<Rc<Building>> {
    let key = (seed, kind, starter);
    if let Some(hit) = LAST_SITE.with(|l| {
        l.borrow()
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, b)| b.clone())
    }) {
        return hit;
    }
    let found = SITE_BUILDINGS.with(|c| c.borrow().get(&key).cloned());
    let slot = match found {
        Some(slot) => slot,
        None => {
            let built = build_site(kind, seed, starter).map(Rc::new);
            SITE_BUILDINGS.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() >= 128 {
                    c.clear();
                }
                c.insert(key, built.clone());
            });
            built
        }
    };
    LAST_SITE.with(|l| *l.borrow_mut() = Some((key, slot.clone())));
    slot
}

fn build_site(kind: u32, seed: u32, starter: bool) -> Option<Building> {
    let var = if starter {
        0.0
    } else {
        hash2(0, 0, seed.wrapping_add(67)) * 3.0
    };
    let age = if starter {
        0.04
    } else {
        0.12 + hash2(0, 1, seed.wrapping_add(68)) * 0.2
    };
    match kind {
        0 => {
            // Artificer workshop: tall timber-and-stone hall with side rooms.
            let nx = if var > 1.5 { 6 } else { 5 };
            let (w, d) = (nx as f32 * 3.6, 4.0 * 3.6);
            let wing = spec(-w * 0.5, -d * 0.5, nx, 4, seed, |s| {
                s.h = 4.8;
                s.age = age;
                s.chimney = true;
                s.min_room = 7;
                s.pal = Palette::TIMBER;
                s.doors = vec![door(1, w * 0.5, 1.1, 2.7), door(2, d * 0.5, 0.9, 2.4)];
                s.keep = vec![(-4.0, 2.0), (4.0, 2.0), (-4.0, -3.0)]
                    .into_iter()
                    .map(|(x, z)| (x + w * 0.5, z + d * 0.5))
                    .collect();
            });
            Some(Building::new(
                vec![wing],
                vec![],
                Some((
                    -w * 0.5 - 2.0,
                    w * 0.5 + 2.0,
                    -d * 0.5 - 2.0,
                    d * 0.5 + 2.0,
                    B::StoneBricks,
                    12.0,
                )),
            ))
        }
        1 => None,
        2 => {
            // Sunken waterworks: two pump halls flank the reservoir, a sluice hall closes the north.
            let flank = |x0: f32, door_side: u8, sd: u32| {
                spec(x0, -9.0, 2, 5, seed.wrapping_add(sd), move |s| {
                    s.pal = Palette::LIMESTONE;
                    s.age = age;
                    s.min_room = 6;
                    s.storeys = 1;
                    s.h = 4.4;
                    s.doors = vec![door(door_side, 9.0, 1.0, 2.5)];
                })
            };
            let hall = spec(-5.4, -12.6, 3, 2, seed.wrapping_add(9), |s| {
                s.pal = Palette::LIMESTONE;
                s.age = age;
                s.h = 4.4;
                s.roof = Roof::Hip;
                s.chimney = true;
                s.open = true;
                s.doors = vec![door(1, 5.4, 1.2, 2.6)];
            });
            Some(Building::new(
                vec![flank(-12.7, 3, 1), flank(5.5, 2, 2), hall],
                vec![],
                Some((-13.5, 13.5, -14.0, 14.0, B::Cobblestone, 12.0)),
            ))
        }
        3 => {
            // Abandoned foundry castle: curtain wall, four turrets, forge hall and gatehouse.
            let hw = 14.0 + var;
            let hl = 11.0;
            let keep = vec![
                (-5.0, -6.0),
                (0.0, -6.0),
                (5.0, -6.0),
                (0.0, 8.0),
                (-12.0, -7.0),
                (-9.0, -5.0),
                (9.0, -5.0),
                (-4.0, 2.0),
                (4.0, 2.0),
                (-4.0, -3.0),
            ];
            let mut wings = Vec::new();
            for (n, (sx, sz)) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .into_iter()
                .enumerate()
            {
                let (cx, cz) = (sx * (hw - 0.5), sz * (hl - 0.5));
                // Doors face the courtyard along the long side so they clear the curtain.
                let side = if sx < 0.0 { 3 } else { 2 };
                let u = if sz < 0.0 { 5.0 } else { 1.0 };
                wings.push(spec(
                    cx - 3.0,
                    cz - 3.0,
                    2,
                    2,
                    seed.wrapping_add(20 + n as u32),
                    |s| {
                        s.cell = 3.0;
                        s.storeys = 2;
                        s.roof = Roof::Flat;
                        s.pal = Palette::STONE;
                        s.windows = Windows::Slits;
                        s.age = age * 0.6;
                        s.min_room = 4;
                        s.doors = vec![door(side, u, 0.6, 2.4)];
                    },
                ));
            }
            let hall_nx = 6;
            let hall_x0 = -(hall_nx as f32) * 3.6 * 0.5;
            wings.push(spec(
                hall_x0,
                -hl + 1.6,
                hall_nx,
                2,
                seed.wrapping_add(30),
                |s| {
                    s.storeys = 2;
                    s.h = 3.8;
                    s.pal = Palette::STONE;
                    s.windows = Windows::Slits;
                    s.age = age * 0.7;
                    s.chimney = true;
                    s.min_room = 4;
                    s.doors = vec![door(1, 4.8, 1.0, 2.7), door(1, 16.8, 1.0, 2.7)];
                    s.keep = keep
                        .iter()
                        .map(|(x, z)| (x - hall_x0, z + hl - 1.6))
                        .collect();
                },
            ));
            wings.push(spec(-3.6, hl - 7.2, 2, 2, seed.wrapping_add(31), |s| {
                s.storeys = 1;
                s.h = 5.2;
                s.pal = Palette::STONE;
                s.windows = Windows::Slits;
                s.age = age * 0.5;
                s.open = true;
                s.doors = vec![door(1, 3.6, 1.9, 4.3), door(0, 3.6, 1.9, 4.3)];
            }));
            let curtain = Curtain {
                x0: -hw,
                x1: hw,
                z0: -hl,
                z1: hl,
                t: 1.6,
                h: 6.0,
                mat: B::StoneBricks,
                gate: Some((1, 0.0, 1.9, 4.3)),
            };
            Some(Building::new(
                wings,
                vec![curtain],
                Some((
                    -hw - 3.0,
                    hw + 3.0,
                    -hl - 3.0,
                    hl + 3.0,
                    B::StoneBricks,
                    14.0,
                )),
            ))
        }
        4 => {
            // Mycologist conservatory: a glazed hall under a ribbed glass roof.
            let wing = spec(-10.8, -10.8, 6, 6, seed, |s| {
                s.h = 7.2;
                s.roof = Roof::Glass;
                s.pal = Palette::GLASSHOUSE;
                s.windows = Windows::Glazed;
                s.age = age * 0.5;
                s.open = true;
                s.doors = vec![
                    door(0, 10.8, 1.6, 3.4),
                    door(1, 10.8, 1.6, 3.4),
                    door(2, 10.8, 1.6, 3.4),
                    door(3, 10.8, 1.6, 3.4),
                ];
            });
            Some(Building::new(
                vec![wing],
                vec![],
                Some((-12.0, 12.0, -12.0, 12.0, B::Cobblestone, 12.0)),
            ))
        }
        _ => {
            // Resonant archive: two storeys of aisles and reading rooms.
            let wing = spec(-9.0, -12.6, 5, 7, seed, |s| {
                s.storeys = 2;
                s.h = 4.2;
                s.pal = Palette::STONE;
                s.pal.floor = B::Planks;
                s.windows = Windows::Panes;
                s.age = age;
                s.chimney = true;
                s.min_room = 9;
                s.doors = vec![door(1, 9.0, 1.2, 2.8), door(2, 12.6, 0.9, 2.4)];
                s.keep = vec![
                    (-4.0, 2.0),
                    (4.0, 2.0),
                    (-4.0, -3.0),
                    (-5.0, 4.0),
                    (5.0, 4.0),
                    (-5.0, -4.0),
                    (5.0, -4.0),
                ]
                .into_iter()
                .map(|(x, z)| (x + 9.0, z + 12.6))
                .collect();
            });
            Some(Building::new(
                vec![wing],
                vec![],
                Some((-11.0, 11.0, -14.5, 14.5, B::Cobblestone, 12.0)),
            ))
        }
    }
}

/// Small roofed outposts scattered between the large sites.
pub fn homestead_building(seed: u32) -> Building {
    let pick = (hash2(0, 0, seed.wrapping_add(900)) * 5.0) as u32 % 5;
    let age = 0.05 + hash2(1, 0, seed.wrapping_add(901)) * 0.3;
    match pick {
        0 => {
            // Cottage with a lean-to shed.
            let house = spec(-3.6, -3.6, 2, 2, seed, |s| {
                s.age = age;
                s.chimney = true;
                s.min_room = 3;
                s.doors = vec![door(1, 3.6, 0.9, 2.4)];
            });
            let shed = spec(3.6, -2.0, 1, 2, seed.wrapping_add(1), |s| {
                s.roof = Roof::Lean;
                s.age = age;
                s.open = true;
                s.doors = vec![door(1, 1.8, 0.9, 2.3)];
            });
            Building::new(vec![house, shed], vec![], None)
        }
        1 => {
            // Farmstead: two-storey house and an open barn.
            let house = spec(-7.2, -3.6, 3, 2, seed, |s| {
                s.storeys = 2;
                s.age = age;
                s.chimney = true;
                s.min_room = 3;
                s.doors = vec![door(1, 5.4, 0.9, 2.4)];
            });
            let barn = spec(4.0, -5.4, 3, 3, seed.wrapping_add(2), |s| {
                s.h = 4.6;
                s.age = age;
                s.open = true;
                s.doors = vec![door(2, 5.4, 1.6, 3.4), door(1, 5.4, 1.6, 3.4)];
            });
            Building::new(vec![house, barn], vec![], None)
        }
        2 => {
            // Watch post: a flat-topped turret with a guard hut.
            let tower = spec(-3.0, -3.0, 2, 2, seed, |s| {
                s.cell = 3.0;
                s.storeys = 3;
                s.roof = Roof::Flat;
                s.pal = Palette::STONE;
                s.windows = Windows::Slits;
                s.age = age * 0.6;
                s.doors = vec![door(1, 3.0, 0.8, 2.4)];
            });
            let hut = spec(3.6, -1.8, 1, 1, seed.wrapping_add(3), |s| {
                s.age = age;
                s.open = true;
                s.pal = Palette::STONE;
                s.doors = vec![door(2, 1.8, 0.8, 2.3)];
            });
            Building::new(vec![tower, hut], vec![], None)
        }
        3 => {
            // Wayside chapel.
            let nave = spec(-3.6, -7.2, 2, 4, seed, |s| {
                s.pal = Palette::LIMESTONE;
                s.h = 5.0;
                s.age = age;
                s.min_room = 4;
                s.doors = vec![door(1, 3.6, 1.1, 2.8)];
            });
            Building::new(vec![nave], vec![], None)
        }
        _ => {
            // Roadside inn: long hall with a loft.
            let inn = spec(-9.0, -3.6, 5, 2, seed, |s| {
                s.storeys = 2;
                s.age = age;
                s.chimney = true;
                s.min_room = 4;
                s.doors = vec![door(1, 5.4, 1.0, 2.5), door(1, 12.6, 0.9, 2.4)];
            });
            Building::new(vec![inn], vec![], None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(b: &Building, x: f32, z: f32, v: f32) -> Vec<Option<B>> {
        (-4..40)
            .map(|i| b.sample(Vec3::new(x, (i as f32 + 0.5) * v, z), v, f32::MIN))
            .collect()
    }

    #[test]
    fn every_site_kind_has_a_roof_above_its_rooms() {
        for kind in [0u32, 2, 3, 4, 5] {
            for &v in &[1.0f32, 0.5, 0.25] {
                let b = site_building(kind, 12345 + kind, false).expect("kind has a building");
                let (x0, z0, x1, z1) = b.extent();
                let mut roofed = 0;
                let mut open = 0;
                let step = 1.0;
                let mut z = z0;
                while z < z1 {
                    let mut x = x0;
                    while x < x1 {
                        let mut interior = false;
                        let mut cover = false;
                        let mut y = 0.0;
                        while y < b.top {
                            match b.sample(Vec3::new(x, y + 0.5 * v, z), v, f32::MIN) {
                                Some(B::Air) if y < 2.0 => interior = true,
                                Some(m) if interior && m != B::Air => {
                                    cover = true;
                                    break;
                                }
                                _ => {}
                            }
                            y += v;
                        }
                        if interior {
                            if cover {
                                roofed += 1;
                            } else {
                                open += 1;
                            }
                        }
                        x += step;
                    }
                    z += step;
                }
                assert!(roofed > 0, "kind {kind} scale {v} has no covered interior");
                assert!(
                    roofed * 3 > open,
                    "kind {kind} scale {v}: {roofed} covered vs {open} open"
                );
            }
        }
    }

    #[test]
    fn sampling_is_deterministic_and_bounded() {
        let a = site_building(3, 99, false).unwrap();
        let b = Rc::new(build_site(3, 99, false).unwrap());
        for i in 0..200 {
            let p = Vec3::new(-20.0 + i as f32 * 0.2, 1.5, 3.3);
            assert_eq!(a.sample(p, 1.0, f32::MIN), b.sample(p, 1.0, f32::MIN));
        }
        assert!(
            a.sample(Vec3::new(500.0, 1.0, 0.0), 1.0, f32::MIN)
                .is_none()
        );
        assert!(column(&a, 0.0, 0.0, 1.0).len() == 44);
    }

    #[test]
    fn wings_connect_every_room_by_a_door() {
        for seed in 0..40u32 {
            let w = spec(0.0, 0.0, 6, 5, seed, |s| {
                s.storeys = 2;
                s.min_room = 4;
            });
            for s in 0..w.storeys {
                let ids = &w.rooms[s];
                let count = ids.iter().copied().max().unwrap() as usize + 1;
                let mut parent: Vec<usize> = (0..count).collect();
                for z in 0..w.nz {
                    for e in 1..w.nx {
                        if w.door_x[s][z * (w.nx + 1) + e] {
                            let (a, b) =
                                (ids[z * w.nx + e - 1] as usize, ids[z * w.nx + e] as usize);
                            let (fa, fb) = (find(&mut parent, a), find(&mut parent, b));
                            parent[fa] = fb;
                        }
                    }
                }
                for e in 1..w.nz {
                    for x in 0..w.nx {
                        if w.door_z[s][e * w.nx + x] {
                            let (a, b) =
                                (ids[(e - 1) * w.nx + x] as usize, ids[e * w.nx + x] as usize);
                            let (fa, fb) = (find(&mut parent, a), find(&mut parent, b));
                            parent[fa] = fb;
                        }
                    }
                }
                let root = find(&mut parent, 0);
                for r in 0..count {
                    assert_eq!(
                        find(&mut parent, r),
                        root,
                        "seed {seed} storey {s} room {r} is isolated"
                    );
                }
            }
        }
    }

    /// Visual aid: `cargo test dump_blueprints -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_blueprints() {
        let ch = |b: Option<B>| match b {
            None => ' ',
            Some(B::Air) => '.',
            Some(B::Planks) => 'p',
            Some(B::Log) => 'L',
            Some(B::StoneBricks) => '#',
            Some(B::WeatheredBricks) => 'w',
            Some(B::Basalt) => 'b',
            Some(B::Limestone) => 'l',
            Some(B::Cobblestone) => 'c',
            Some(B::Ice) => 'i',
            Some(B::Torch) => 't',
            _ => '?',
        };
        for kind in [0u32, 2, 3, 4, 5] {
            let b = site_building(kind, 4242, false).unwrap();
            let (x0, z0, x1, z1) = b.extent();
            for (title, y) in [
                ("ground y=1.7", 1.7f32),
                ("upper y=5.5", 5.5),
                ("roof y=9.0", 9.0),
            ] {
                println!("--- kind {kind} {title}");
                let mut z = z0;
                while z < z1 {
                    let mut line = String::new();
                    let mut x = x0;
                    while x < x1 {
                        line.push(ch(b.sample(Vec3::new(x, y, z), 0.5, f32::MIN)));
                        x += 0.5;
                    }
                    println!("{line}");
                    z += 0.5;
                }
            }
        }
    }
}
