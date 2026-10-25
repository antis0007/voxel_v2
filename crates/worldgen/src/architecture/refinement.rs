//! Shared exterior vocabulary, with door clearances taking precedence over
//! overlapping wing/curtain masonry. Only generator 39+ uses this sampler.
use super::*;

/// Low service ranges frame the court without filling its entrance axis or
/// the forge's working area. Their doors face inward, like the corner towers.
pub(super) fn courtyard(mut b: Building, kind: u32, seed: u32) -> Building {
    if kind == 3
        && let Some(c) = b.curtains.first()
    {
        for (i, x) in [c.x0 + 2.0, c.x1 - 5.6].into_iter().enumerate() {
            b.wings.push(spec(x, -1.2, 1, 2, seed.wrapping_add(80 + i as u32), |s| {
                s.pal = if i == 0 { Palette::TIMBER } else { Palette::STONE };
                s.roof = Roof::Gable;
                s.h = 3.6;
                s.open = true;
                s.age = 0.08;
                s.doors = vec![door(if i == 0 { 3 } else { 2 }, 3.6, 1.2, 2.7)];
            }));
        }
    }
    Building::new(b.wings, b.curtains, b.plot)
}

pub(super) fn silhouette(mut b: Building, kind: u32, seed: u32) -> Building {
    if kind == 3 {
        // A keep, watch turrets and a low gatehouse have distinct roles in the
        // skyline. Rebuild resized wings so stairs and room plans match them.
        for (i, w) in b.wings.iter_mut().enumerate() {
            if i > 4 {
                continue;
            }
            let mut s = WingSpec::new(w.x0, w.z0, w.nx, w.nz, w.seed);
            s.cell = w.cell;
            s.h = w.h;
            s.storeys = if i == 4 || i == (seed as usize % 4) {
                3
            } else {
                2
            };
            s.roof = if i == 4 { Roof::Hip } else { Roof::Flat };
            s.pal = w.pal;
            s.windows = w.windows;
            s.age = w.age;
            s.chimney = w.chimney;
            s.doors = w.doors.clone();
            s.keep = w.keep.clone();
            s.min_room = 4;
            *w = Wing::new(s);
        }
    }
    Building::new(b.wings, b.curtains, b.plot)
}

/// A shallow gatehouse leaves a usable forecourt between the entrance and
/// service ranges. Its exterior threshold stays on the same curtain face.
pub(super) fn open_court(mut b: Building, kind: u32) -> Building {
    if kind == 3
        && let Some(w) = b.wings.get_mut(5)
    {
        let mut s = WingSpec::new(w.x0, w.z0 + w.cell, w.nx, 1, w.seed);
        s.cell = w.cell;
        s.h = w.h;
        s.storeys = w.storeys;
        s.roof = w.roof;
        s.pal = w.pal;
        s.windows = w.windows;
        s.age = w.age;
        s.open = true;
        s.doors = w.doors.clone();
        s.keep = w.keep.clone();
        *w = Wing::new(s);
    }
    Building::new(b.wings, b.curtains, b.plot)
}

impl Building {
    /// Recessed planted courts and low seating occupy only the unused side
    /// bays. The gate axis and every existing threshold lane remain paved.
    pub(super) fn sample_court_gardens(&self, q: Vec3, v: f32, th: f32) -> Option<B> {
        let base = self.sample_circulation(q, v, th);
        let Some(c) = self.curtains.first() else { return base; };
        if q.y < 0.0 || q.y >= v * 2.0 || q.x.abs() < 3.0 {
            return base;
        }
        // Ask the actual floor sampler before adding anything above it: this
        // excludes wings, tower thresholds, cross lanes and the main spine.
        let floor = self.sample_circulation(Vec3::new(q.x, v * 0.5, q.z), v, th);
        if floor != Some(B::WeatheredBricks) { return base; }
        if self.wings.iter().any(|w| q.x > w.x0 - 1.0 && q.x < w.x0 + w.width() + 1.0
            && q.z > w.z0 - 1.0 && q.z < w.z0 + w.depth() + 1.0) {
            return base;
        }
        let centre_x = if q.x < 0.0 { c.x0 * 0.45 } else { c.x1 * 0.45 };
        let dx = (q.x - centre_x).abs();
        let dz = (q.z - 4.8).abs();
        if dx >= 2.4 || dz >= 2.6 { return base; }
        if q.y < v {
            return Some(if dx < 1.5 && dz < 1.7 { B::Grass } else { B::Limestone });
        }
        // A low bench on the outside edge leaves three open approaches.
        if dx > 1.5 && q.x.abs() > centre_x.abs() && dz < 1.7
            && base == Some(B::Air) {
            return Some(B::Limestone);
        }
        base
    }

    /// Paved routes join the gate, hall and tower thresholds. Only replace
    /// courtyard paving: walls, fixtures and the existing floor elevations win.
    pub(super) fn sample_circulation(&self, q: Vec3, v: f32, th: f32) -> Option<B> {
        let base = self.sample_refined(q, v, th);
        let Some(c) = self.curtains.first() else { return base; };
        if q.y < 0.0 || q.y >= v || base != Some(B::StoneBricks) {
            return base;
        }
        if q.x <= c.x0 + c.t || q.x >= c.x1 - c.t
            || q.z <= c.z0 + c.t || q.z >= c.z1 - c.t
            || self.wings.iter().any(|w| q.x >= w.x0 && q.x < w.x0 + w.width()
                && q.z >= w.z0 && q.z < w.z0 + w.depth()) {
            return base;
        }
        let mut route = q.x.abs() < 1.5;
        for w in &self.wings {
            for d in &w.doors {
                let (x, z) = match d.side {
                    0 => (w.x0 + d.u, w.z0),
                    1 => (w.x0 + d.u, w.z0 + w.depth()),
                    2 => (w.x0, w.z0 + d.u),
                    _ => (w.x0 + w.width(), w.z0 + d.u),
                };
                // Each threshold joins the central spine through a straight
                // transverse lane; no new geometry can obstruct the court.
                route |= (q.z - z).abs() < 1.25
                    && q.x >= x.min(0.0) - 1.25 && q.x <= x.max(0.0) + 1.25;
            }
        }
        Some(if route { B::Cobblestone } else { B::WeatheredBricks })
    }

    pub(super) fn sample_refined(&self, q: Vec3, v: f32, th: f32) -> Option<B> {
        if !self.contains_xz(q.x, q.z) || q.y < -FOUNDATION {
            return None;
        }
        let slab = v.max(0.4);
        for w in &self.wings {
            let p = q - Vec3::new(w.x0, 0.0, w.z0);
            // Open a shared doorway through both pieces, not just whichever
            // wing was sampled first. Never cut the floor or an upper storey.
            if p.y >= slab {
                for d in &w.doors {
                    let (face, along) = face(w, p, d.side);
                    let half = d.hw.max(v * 0.75);
                    let arch = ((along - d.u) / half).powi(2) * 0.5;
                    if face >= -v
                        && face < 1.8f32.max(v * 2.0)
                        && (along - d.u).abs() < half
                        && p.y - slab < d.height - arch
                    {
                        return Some(B::Air);
                    }
                }
            }
        }
        if q.y < 0.0 && q.y >= th - 2.0 && self.occupies(q.x, q.z) {
            return Some(B::WeatheredBricks);
        }
        let base = self.sample_plot(q, v, th, true);
        // Dress only exposed faces; the same vocabulary works on a cottage,
        // pump house, gatehouse, archive or castle without interior clutter.
        for w in &self.wings {
            let p = q - Vec3::new(w.x0, 0.0, w.z0);
            if let Some(b) = w.facade(p, v, base) {
                return Some(b);
            }
        }
        if base.is_some() {
            return base;
        }
        // Trees must not grow out of roofs or leave trunks in the room's
        // headroom when the natural terrain is higher than the floor.
        if self.occupies(q.x, q.z) && q.y >= 0.0 && q.y <= self.top.max(th) + 12.0 {
            return Some(B::Air);
        }
        None
    }
}

fn face(w: &Wing, p: Vec3, side: u8) -> (f32, f32) {
    match side {
        0 => (p.z, p.x),
        1 => (w.depth() - p.z, p.x),
        2 => (p.x, p.z),
        _ => (w.width() - p.x, p.z),
    }
}

impl Wing {
    fn facade(&self, p: Vec3, v: f32, base: Option<B>) -> Option<B> {
        if p.y < 0.0 || p.y > self.top() + v {
            return None;
        }
        let wt = v.max(0.6);
        let band = v.max(0.35);
        let stone = self.pal.wall != B::Planks && self.windows != Windows::Glazed;
        for side in 0..4 {
            let (f, u) = face(self, p, side);
            let span = if side < 2 { self.width() } else { self.depth() };
            if f < -v.max(0.75) || f > wt || u < 0.0 || u >= span {
                continue;
            }
            let fy = p.y.rem_euclid(self.h);
            let bay = u.rem_euclid(self.cell);
            let pier = bay.min(self.cell - bay) < v.max(0.45);
            // Keep doors and their approach free, including tower doors where
            // two building pieces intersect.
            if self.doors.iter().any(|d| {
                d.side == side && (u - d.u).abs() < d.hw + v && p.y < d.height + v.max(0.6)
            }) {
                continue;
            }
            if f >= 0.0 {
                if base == Some(self.pal.wall) && (fy < band || fy > self.h - band || pier) {
                    return Some(self.pal.frame);
                }
                continue;
            }
            // Do not overwrite the solid body of an adjacent wing.
            if base.is_some_and(|b| b != B::Air) {
                continue;
            }
            if stone && pier && p.y < self.top() - band {
                let reach = if p.y < 1.2 { v.max(0.75) } else { v.max(0.4) };
                if f >= -reach {
                    return Some(self.pal.plinth);
                }
            }
            if stone && p.y >= self.top() - band && p.y < self.top() {
                return Some(self.pal.frame);
            }
            if matches!(self.windows, Windows::Panes | Windows::Slits) {
                let centre = (bay - self.cell * 0.5).abs();
                let rel = fy - v.max(0.4);
                let top = if self.windows == Windows::Slits {
                    2.7
                } else {
                    2.3
                };
                if centre < v.max(0.8)
                    && ((0.9 - band..0.9).contains(&rel) || (top..top + band).contains(&rel))
                {
                    return Some(self.pal.frame);
                }
            }
        }
        None
    }
}
