use super::{Model, Vec3};
use crate::world::adventure::magic_fx::Family;

impl Model {
    /// A four-sided tube with shared corners at each bend and no internal caps.
    pub(super) fn spell_thread(&mut self, path: &[Vec3; 7], color: [f32; 3], returning: bool) {
        let axis = (path[6] - path[0]).normalize_or_zero();
        if axis == Vec3::ZERO {
            return;
        }
        let side = axis.any_orthonormal_vector();
        let mut distance = 0.0;
        let mut previous = [Vec3::ZERO; 4];
        let alpha = std::mem::replace(&mut self.alpha, 0.88);
        for step in 0..path.len() {
            let i = if returning { path.len() - 1 - step } else { step };
            let point = path[i];
            let tangent = (path[(i + 1).min(6)] - path[i.saturating_sub(1)]).normalize_or(axis);
            let up = tangent.cross(side).normalize_or_zero() * 0.016;
            let across = up.cross(tangent);
            let ring = [point + across, point + up, point - across, point - up];
            if step > 0 {
                let from = path[if returning { i + 1 } else { i - 1 }];
                let span = point - from;
                let length = span.length();
                if length > 0.0001 {
                    let mark = self.mark();
                    for j in 0..4 {
                        let k = (j + 1) % 4;
                        let normal = (ring[j] + ring[k] - point * 2.0).normalize_or_zero();
                        self.triangle([previous[j], ring[j], ring[k]], normal, color, 1.25);
                        self.triangle([previous[j], ring[k], previous[k]], normal, color, 1.25);
                    }
                    let direction = span / length;
                    for v in &mut self.vertices[mark..] {
                        let offset = Vec3::from_array(v.pos) * self.scale - from;
                        // Each vertex belongs to one of the two section rings.
                        let along = if offset.dot(direction) > length * 0.5 { length } else { 0.0 };
                        v.surface[1] = -2.0 - (distance + along);
                    }
                    distance += length;
                }
            }
            previous = ring;
        }
        self.alpha = alpha;
    }

    /// Tapered shards carry a longitudinal heat coordinate in the glow material.
    pub(crate) fn spark_shard(
        &mut self, at: Vec3, direction: Vec3, size: f32, stretch: f32,
        color: [f32; 3], glow: f32, alpha: f32,
    ) {
        let side = direction.any_orthonormal_vector() * size * 0.45;
        let up = direction.cross(side);
        let ring = [at + side, at + up, at - side, at - up];
        let axis = direction * size * stretch;
        let tip = at + axis * 0.38;
        let tail = at - axis * 0.62;
        let mark = self.mark();
        let previous = std::mem::replace(&mut self.alpha, alpha);
        for i in 0..4 {
            let a = ring[i];
            let b = ring[(i + 1) % 4];
            let normal = (a + b - at * 2.0).normalize_or_zero();
            self.triangle([tip, a, b], normal, color, glow);
            self.triangle([tail, b, a], normal, color, glow);
        }
        self.alpha = previous;
        for v in &mut self.vertices[mark..] {
            let offset = Vec3::from_array(v.pos) * self.scale - tail;
            let heat = (offset.dot(direction) / (size * stretch)).clamp(0.0, 1.0);
            v.surface[1] = -1.1 - heat * 0.8;
        }
    }

    pub(super) fn spell_impact(
        &mut self, at: Vec3, normal: Vec3, tint: [f32; 3], radius: f32, life: f32, family: Family,
    ) {
        let fade = life * life;
        let progress = 1.0 - life;
        let size = radius * if family == Family::Rift { 0.38 + life * 0.58 } else { 0.2 + 0.8 * (1.0 - fade) };
        let side = Vec3::Y.cross(normal).normalize_or(normal.any_orthonormal_vector());
        let up = normal.cross(side);
        let count = match family {
            Family::Water => 8,
            Family::Frost => 6,
            Family::Shock | Family::Spore | Family::Ember => 5,
            Family::Rift | Family::Acid | Family::Lumen => 3,
            _ => 4,
        };
        let step = std::f32::consts::TAU / count as f32;
        let alpha = std::mem::replace(&mut self.alpha, 0.68 * fade);
        for i in 0..count {
            let angle = i as f32 * step + if family == Family::Rift { progress * 1.5 } else { 0.0 };
            let (sin, cos) = angle.sin_cos();
            let radial = side * cos + up * sin;
            let tangent = normal.cross(radial);
            let color = family.color(tint, life, (i as u32 * 83 + 37) & 255);
            let glow = match family { Family::Spore => 0.52, Family::Acid | Family::Water => 0.86, _ => 1.45 };
            let width = 0.005 + 0.011 * fade;
            let start = at + radial * size * 0.40;
            let end = at + radial * size;
            let mut stroke = |a, b| self.impact_stroke(a, b, normal, width, color, glow * fade);
            match family {
                Family::Frost => {
                    stroke(start, end);
                    stroke(at + radial * size * 0.68, at + (radial * 0.9 + tangent * 0.19) * size);
                }
                Family::Shock => {
                    let kink = at + (radial * 0.64 + tangent * 0.20) * size;
                    stroke(start, kink);
                    stroke(kink, end);
                }
                Family::Ember => stroke(start, end + up * size * 0.5),
                Family::Rift => stroke(end, at + (radial * 0.48 + tangent * 0.65) * size),
                Family::Thread => stroke(start + tangent * size * 0.16, end - tangent * size * 0.27),
                Family::Lumen => stroke(end - tangent * size * 0.34, end + tangent * size * 0.18),
                Family::Spore => self.glow_orb(end + up * progress * 0.22, 0.037 * life, color, glow, 0.42 * life),
                Family::Acid => stroke(start, end - up * size * 0.44),
                _ => {
                    let (sin, cos) = (angle + step).sin_cos();
                    let next = at + (side * cos + up * sin) * size;
                    stroke(end.lerp(next, 0.13), end.lerp(next, 0.87));
                }
            }
        }
        self.alpha = alpha;
        if life > 0.55 && family != Family::Spore {
            self.glow_orb(at, radius * 0.30 * fade, family.color(tint, life, 127), 1.65 * fade, 0.66 * fade);
        }
    }

    fn impact_stroke(&mut self, a: Vec3, b: Vec3, normal: Vec3, width: f32, color: [f32; 3], glow: f32) {
        let across = normal.cross(b - a).normalize_or_zero() * width;
        for facing in [normal, -normal] {
            self.triangle([a - across, b - across, b + across], facing, color, glow);
            self.triangle([a - across, b + across, a + across], facing, color, glow);
        }
    }
}
