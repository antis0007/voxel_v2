//! Stair voxels of the shell share one cylinder. The field alone left the cubes straight.
use glam::Vec3;

use super::shell;

/// Flat band and the outer edge where the pin meets the unmoved mesh, in metres.
const FLAT: f32 = 0.72;
const SKIN: f32 = 1.05;

/// Cylinder the shell mesh and the walking body share.
#[derive(Clone, Copy)]
pub struct ShellCurve {
    pub center: Vec3,
    pub mean: f32,
    pub swing: f32,
    pub wave: f32,
    pub bulge: f32,
    pub bulge_k: f32,
    pub bulge_phase: f32,
    pub crust: f32,
}

/// `None` for the flat maze and every world that is not the swelling shell.
pub fn shell_curve(version: u32) -> Option<ShellCurve> {
    if version < shell::VERSION || super::flat_maze(version) {
        return None;
    }
    let (center, t) = shell::wave_terms(version);
    Some(ShellCurve {
        center,
        mean: t[0],
        swing: t[1],
        wave: t[2],
        bulge: t[3],
        bulge_k: t[4],
        bulge_phase: t[5],
        crust: t[6],
    })
}

impl ShellCurve {
    /// Squared radial bounds of the only band where mesh vertices move.
    pub fn deformation_bounds(self, world_z: f32) -> std::ops::Range<f32> {
        let radius = self.radius(world_z - self.center.z);
        (radius - SKIN).max(0.0).powi(2)..(radius + SKIN).powi(2)
    }

    pub fn radius(self, z: f32) -> f32 {
        self.mean
            + self.swing * (z * self.wave).sin()
            + self.bulge * (z * self.wave * self.bulge_k + self.bulge_phase).sin()
    }

    pub fn offset(self, point: Vec3) -> f32 {
        let rel = point - self.center;
        let radial = Vec3::new(rel.x, rel.y, 0.0).length();
        radial - self.radius(rel.z)
    }

    /// Move a point onto the cylinder when it sits in the stair band.
    pub fn project(self, point: Vec3) -> Vec3 {
        let rel = point - self.center;
        let radial = Vec3::new(rel.x, rel.y, 0.0);
        let r = radial.length();
        if r < 0.2 {
            return point;
        }
        let shell = self.radius(rel.z);
        let curved = shell + pin_offset(r - shell);
        let dir = radial / r;
        self.center + Vec3::new(dir.x * curved, dir.y * curved, rel.z)
    }

    /// Same angle and height, on the inner face of the shell.
    pub fn floor_point(self, point: Vec3, inward: f32) -> Vec3 {
        let rel = point - self.center;
        let radial = Vec3::new(rel.x, rel.y, 0.0);
        let r = radial.length();
        if r < 0.2 {
            return point;
        }
        let dir = radial / r;
        let shell = self.radius(rel.z) - inward;
        self.center + Vec3::new(dir.x * shell, dir.y * shell, rel.z)
    }
}

fn pin_offset(offset: f32) -> f32 {
    let a = offset.abs();
    if a <= FLAT {
        return 0.0;
    }
    if a >= SKIN {
        return offset;
    }
    let t = (a - FLAT) / (SKIN - FLAT);
    let s = t * t * (3.0 - 2.0 * t);
    // s * a meets the unmoved mesh with a matching slope. s * SKIN corners there.
    offset.signum() * s * a
}

impl ShellCurve {
    /// Centre sits in the crust band the stair collision treats as the cylinder.
    pub fn crust(self, centre: Vec3) -> bool {
        let off = self.offset(centre);
        off > 0.0 && off < self.crust + 0.75
    }

    fn radius_slope(self, z: f32) -> f32 {
        self.swing * self.wave * (z * self.wave).cos()
            + self.bulge * self.bulge_k * self.wave
                * (z * self.wave * self.bulge_k + self.bulge_phase).cos()
    }

    /// First time the segment meets the cylinder, as a fraction from `a` to `b`.
    pub fn crossing(self, a: Vec3, b: Vec3) -> Option<f32> {
        let radius = self.radius(a.z - self.center.z);
        let mut t = cylinder_root(a, b, self.center, radius)?;
        let dz = b.z - a.z;
        let ox = a.x - self.center.x;
        let oy = a.y - self.center.y;
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        for _ in 0..8 {
            let z = a.z + dz * t - self.center.z;
            let x = ox + dx * t;
            let y = oy + dy * t;
            let r = (x * x + y * y).sqrt();
            if r < 0.2 {
                return None;
            }
            let err = r - self.radius(z);
            if err.abs() < 1e-3 {
                return Some(t);
            }
            let slope = (x * dx + y * dy) / r - self.radius_slope(z) * dz;
            if slope.abs() < 1e-6 {
                break;
            }
            t = (t - err / slope).clamp(0.0, 1.0);
        }
        let z = a.z + dz * t - self.center.z;
        let r = Vec3::new(ox + dx * t, oy + dy * t, 0.0).length();
        ((r - self.radius(z)).abs() < 1e-2).then_some(t)
    }
}

fn cylinder_root(a: Vec3, b: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let ox = a.x - center.x;
    let oy = a.y - center.y;
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let a2 = dx * dx + dy * dy;
    if a2 < 1e-10 {
        return None;
    }
    let b2 = 2.0 * (ox * dx + oy * dy);
    let c2 = ox * ox + oy * oy - radius * radius;
    let disc = b2 * b2 - 4.0 * a2 * c2;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let inv = 0.5 / a2;
    let t0 = (-b2 - s) * inv;
    let t1 = (-b2 + s) * inv;
    [t0, t1]
        .into_iter()
        .filter(|t| (0.0..=1.0).contains(t))
        .min_by(f32::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::shell::{self, GRAND, VERSION};

    #[test]
    fn stair_corners_share_the_cylinder_and_the_hall_does_not() {
        let curve = shell_curve(GRAND).expect("grand shell");
        let z = 40.0;
        let shell_r = curve.radius(z);
        assert!((shell_r - shell::shell_radius(GRAND, z)).abs() < 1e-4);
        for offset in [-0.6_f32, -0.2, 0.0, 0.35, 0.7] {
            let p = curve.center + Vec3::new(0.0, -(shell_r + offset), z);
            let q = curve.project(p);
            let r = Vec3::new(q.x - curve.center.x, q.y - curve.center.y, 0.0).length();
            assert!((r - shell_r).abs() < 1e-3, "offset {offset} -> {}", r - shell_r);
        }
        let hall = curve.center + Vec3::new(0.0, -(shell_r - 3.0), z);
        let q = curve.project(hall);
        let r = Vec3::new(q.x - curve.center.x, q.y - curve.center.y, 0.0).length();
        assert!((r - (shell_r - 3.0)).abs() < 1e-3, "hall moved");
        let floor = curve.floor_point(hall, 0.04);
        let fr = Vec3::new(floor.x - curve.center.x, floor.y - curve.center.y, 0.0).length();
        assert!((fr - (shell_r - 0.04)).abs() < 1e-3);
        let edge = pin_offset(SKIN);
        let inside = pin_offset(SKIN - 0.001);
        assert!((edge - SKIN).abs() < 1e-4);
        assert!((edge - inside).abs() < 0.02, "pin jumps at the skin");
        assert!(shell_curve(VERSION - 1).is_none());
        let inside_p = curve.center + Vec3::new(0.0, -(shell_r - 1.0), z);
        let outside_p = curve.center + Vec3::new(0.0, -(shell_r + 1.0), z);
        let t = curve.crossing(inside_p, outside_p).expect("radial cross");
        let hit = inside_p + (outside_p - inside_p) * t;
        let hit_r = Vec3::new(hit.x - curve.center.x, hit.y - curve.center.y, 0.0).length();
        assert!((hit_r - shell_r).abs() < 1e-2, "cross at {}", hit_r - shell_r);
        let along = inside_p + Vec3::new(1.5, 0.2, 3.0);
        assert!(curve.crossing(inside_p, along).is_none(), "hollow segment meets the shell");
        let z1 = z + 30.0;
        let far = curve.center + Vec3::new(0.0, -(curve.radius(z1) + 1.5), z1);
        let t = curve.crossing(inside_p, far).expect("sloped cross");
        let hit = inside_p + (far - inside_p) * t;
        let shell = curve.radius(hit.z - curve.center.z);
        let hit_r = Vec3::new(hit.x - curve.center.x, hit.y - curve.center.y, 0.0).length();
        assert!((hit_r - shell).abs() < 0.05, "{hit_r} vs {shell}");
    }
}
