//! Per-voxel sampling of one district: deck, ramps, parts and caches.
use super::{BUCKET, CORNER, District, shapes};
use crate::{BlockId as B, value_noise2};
use glam::{IVec3, Vec2, Vec3};

impl District {
    pub(super) fn local(&self, m: Vec3) -> Vec3 {
        let d = super::rotate((4 - self.turn) % 4, m.x - self.centre.x, m.z - self.centre.y);
        Vec3::new(d.x, m.y - self.datum, d.y)
    }

    /// District-local metres (x along the spine, y above the deck) to world metres.
    pub fn world(&self, q: Vec3) -> Vec3 {
        let d = super::rotate(self.turn, q.x, q.z);
        Vec3::new(self.centre.x + d.x, self.datum + q.y, self.centre.y + d.y)
    }

    /// Signed distance inside the deck's rounded rectangle (positive inside).
    pub(super) fn inside(&self, q: Vec3) -> f32 {
        if let Some(urban) = self.urban {
            return urban.edge(Vec2::new(q.x, q.z));
        }
        let d = Vec2::new(q.x, q.z).abs() - (self.half - Vec2::splat(CORNER));
        CORNER - d.max(Vec2::ZERO).length() - d.max_element().min(0.0)
    }

    pub(super) fn bucket(&self, q: Vec3) -> Option<&[u16]> {
        let gx = ((q.x - self.lo.x) / BUCKET).floor() as i32;
        let gz = ((q.z - self.lo.y) / BUCKET).floor() as i32;
        if gx < 0 || gz < 0 || gx >= self.grid_w || gz >= self.grid_h {
            return None;
        }
        Some(&self.buckets[(gz * self.grid_w + gx) as usize])
    }

    /// Ramps meet the ground at the middle of the two long edges.
    pub(super) fn ramp(&self, q: Vec3, out: f32, ground: f32) -> Option<B> {
        let across = q.x.abs();
        if across > 3.5 || q.z.abs() < self.half.y - 1.0 {
            return None;
        }
        let surface = ground.signum() * out * 0.55;
        if surface.abs() > ground.abs() {
            return None;
        }
        if q.y <= surface && q.y > surface - 1.2 {
            return Some(if across > 3.0 { B::HazardPaint } else { B::Concrete });
        }
        if ground < 0.0 && q.y <= surface && q.y > ground {
            return Some(B::Concrete);
        }
        (q.y > surface && q.y < surface + 4.0).then_some(B::Air)
    }

    pub(super) fn on_approach(&self, q: Vec3, margin: f32) -> bool {
        self.approaches.iter().any(|ramp| ramp.offset(Vec2::new(q.x, q.z), margin).is_some())
    }

    fn street_ramp(&self, q: Vec3, ground: f32, v: f32) -> Option<B> {
        let (base, grade, along, across) = self.approaches.iter().find_map(|ramp| {
            ramp.offset(Vec2::new(q.x, q.z), 0.0).map(|(along, across)| (ramp.base, ramp.grade, along, across))
        })?;
        let surface = ((base + along * grade) / v).round() * v;
        if q.y < surface && q.y >= surface - v {
            return Some(if across >= super::urban::AVENUE - 1.0 { B::DressedStone } else { B::Basalt });
        }
        if q.y >= surface && q.y < ground.max(surface + 3.0) {
            return Some(B::Air);
        }
        (q.y < surface && q.y >= ground).then_some(B::WeatheredBricks)
    }

    /// The air cell above the first solid cell at or below `m` (within 4 m), for a cache.
    pub(super) fn settle(&self, m: Vec3, v: f32) -> Option<IVec3> {
        let solid = |p: IVec3| {
            let centre = (p.as_vec3() + Vec3::splat(0.5)) * v;
            self.sample(p, centre, f32::NEG_INFINITY, v).is_some_and(|b| b.is_solid())
        };
        let top = (m / v).floor().as_ivec3() + IVec3::Y;
        let mut p = top;
        while p.y > top.y - (4.0 / v) as i32 {
            if solid(p - IVec3::Y) && !solid(p) {
                return Some(p);
            }
            p.y -= 1;
        }
        None
    }

    pub(super) fn sample(&self, p: IVec3, m: Vec3, terrain_top: f32, v: f32) -> Option<B> {
        let q = self.local(m);
        if q.x < self.lo.x || q.z < self.lo.y || q.x > self.hi.x || q.z > self.hi.y {
            return None;
        }
        if let Some(b) = self.caches.get(&p) {
            return Some(*b);
        }
        let mut air = false;
        if let Some(bucket) = self.bucket(q) {
            for &i in bucket {
                let part = &self.parts[i as usize];
                if q.y < part.lo.y || q.y > part.hi.y || !part.contains_xz(q.x, q.z, 0.0) {
                    continue;
                }
                match shapes::sample(&part.shape, q - Vec3::Y * part.elevation, v, self.seed) {
                    Some(B::Air) => air = true,
                    Some(b) => return Some(b),
                    None => {}
                }
            }
        }
        if let Some(block) = self.cut.and_then(|cut| cut.sample(q, v, self.seed)) {
            return Some(block);
        }
        let ground = terrain_top - self.datum;
        let edge = self.inside(q);
        if edge < 0.0 {
            let ramp = if self.urban.is_some() { self.street_ramp(q, ground, v) } else { self.ramp(q, -edge, ground) };
            return ramp.or(air.then_some(B::Air));
        }
        if let Some(urban) = self.urban {
            let approach = (q.x.abs() <= super::urban::AVENUE + 1.0 && q.z.abs() > self.half.y - 12.0)
                || (urban.city && (q.z - super::urban::CROSS_AVENUE).abs() <= super::urban::AVENUE + 1.0
                    && q.x.abs() > self.half.x - 12.0);
            // Leave a rough apron where the district meets the original ground.
            // Streets and all occupied parcels stay on the surveyed datum.
            let shoulder = if approach { 0.0 } else { (1.0 - edge / 5.0).clamp(0.0, 1.0) };
            let grade = urban.elevation(q.z);
            let surface = ((grade + (ground - grade).clamp(-16.0, 16.0) * shoulder) / v).round() * v;
            if q.y >= surface {
                return Some(B::Air);
            }
            if q.y >= surface - v {
                return Some(if shoulder > 0.25 {
                    if shapes::grows_moss(q, self.seed) { B::Grass } else { B::Gravel }
                } else {
                    urban.paving(self.kind, q, v, self.seed)
                });
            }
            return (q.y >= ground || air).then_some(if air { B::Air } else { B::WeatheredBricks });
        }
        if q.y >= 0.0 {
            // A retaining wall holds back ground that stands above the deck.
            if edge < 2.0 && q.y < ground && q.x.abs() > 3.5 {
                return Some(B::Concrete);
            }
            return Some(B::Air);
        }
        if q.y >= -v {
            return Some(self.paving(q, edge));
        }
        (q.y > ground || air).then_some(if air { B::Air } else { B::Concrete })
    }

    pub(super) fn paving(&self, q: Vec3, edge: f32) -> B {
        // A painted service lane runs 6 m inside the kerb.
        if (6.0..6.5).contains(&edge) && q.x.rem_euclid(6.0) < 3.5 {
            return B::HazardPaint;
        }
        let crack = value_noise2(q.x / 6.5, q.z / 6.5, self.seed ^ 0xc4a);
        if crack > 0.70 {
            return B::Grass;
        }
        if crack > 0.64 {
            return B::GroveMoss;
        }
        B::Concrete
    }
}
