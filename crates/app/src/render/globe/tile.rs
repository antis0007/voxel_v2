//! Cube-sphere LOD tile addressing: ids, chart rectangles, body-space bounds, screen error.

use super::chart::{self, HALF_M, RADIUS_M};
use engine_core::cube_sphere::Face;
use glam::{DVec2, DVec3, Vec4};

pub const TILE_CELLS: u32 = 64;
pub const TILE_SAMPLES: u32 = 65;
pub const MAX_LEVEL: u8 = 10;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct TileId {
    pub face: u8,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub fn root(face: u8) -> TileId {
        TileId { face, level: 0, x: 0, y: 0 }
    }

    pub fn children(self) -> [TileId; 4] {
        debug_assert!(self.level < MAX_LEVEL);
        let (level, x, y) = (self.level + 1, self.x * 2, self.y * 2);
        let c = |dx, dy| TileId { face: self.face, level, x: x + dx, y: y + dy };
        [c(0, 0), c(1, 0), c(0, 1), c(1, 1)]
    }

    pub fn parent(self) -> Option<TileId> {
        (self.level > 0).then(|| TileId {
            face: self.face,
            level: self.level - 1,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    pub fn cell_m(self) -> f64 {
        2.0 * HALF_M / (u64::from(TILE_CELLS) << self.level) as f64
    }

    fn width_m(self) -> f64 {
        self.cell_m() * f64::from(TILE_CELLS)
    }

    pub fn chart_min(self) -> DVec2 {
        let w = self.width_m();
        DVec2::new(-HALF_M + f64::from(self.x) * w, -HALF_M + f64::from(self.y) * w)
    }

    pub fn chart_max(self) -> DVec2 {
        self.chart_min() + DVec2::splat(self.width_m())
    }

    /// Sample (i, j) lies on the tile's inclusive 65x65 lattice.
    pub fn sample_chart(self, i: u32, j: u32) -> DVec2 {
        self.chart_min() + DVec2::new(f64::from(i), f64::from(j)) * self.cell_m()
    }

    fn body_at(self, p: DVec2) -> DVec3 {
        let face = Face::ALL[usize::from(self.face) % Face::ALL.len()];
        chart::to_body(chart::join(face, DVec3::new(p.x, 0.0, p.y)))
    }

    pub fn centre_body(self) -> DVec3 {
        self.body_at((self.chart_min() + self.chart_max()) * 0.5)
    }

    /// Nearest projected patch point, without the culling bounds' height slack.
    fn distance_to_patch(self, eye: DVec3) -> f64 {
        let axes = Face::ALL[usize::from(self.face)].basis();
        let normal = eye.dot(axes.n.as_dvec3());
        if normal <= 0.0 { return eye.distance(self.centre_body()); }
        let chart = DVec2::new(eye.dot(axes.u.as_dvec3()).atan2(normal),
            eye.dot(axes.v.as_dvec3()).atan2(normal)) * (HALF_M/std::f64::consts::FRAC_PI_4);
        eye.distance(self.body_at(chart.clamp(self.chart_min(),self.chart_max())))
    }

    pub fn corners_body(self) -> [DVec3; 4] {
        let (a, b) = (self.chart_min(), self.chart_max());
        [a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)].map(|p| self.body_at(p))
    }

    /// Terrain height is not in the corners, so a slack term covers it.
    pub fn bounding_radius_m(self) -> f64 {
        self.corner_radius(self.centre_body()) + 2_000.0
    }

    fn corner_radius(self, centre: DVec3) -> f64 {
        self.corners_body().iter().map(|p| p.distance(centre)).fold(0.0, f64::max)
    }

    pub fn key(self) -> u64 {
        u64::from(self.face) << 56
            | u64::from(self.level) << 48
            | u64::from(self.x & 0xFF_FFFF) << 24
            | u64::from(self.y & 0xFF_FFFF)
    }
}

/// Screen-space size of one cell; zero for tiles hidden behind the limb.
pub fn projected_error_px(id: TileId, eye_body: DVec3, focal_px: f64) -> f64 {
    projected_error_in_view(id, eye_body, focal_px, &[])
}

/// Frustum planes are in camera-relative body space, like the globe vertices.
pub(super) fn projected_error_in_view(id: TileId, eye_body: DVec3, focal_px: f64, planes: &[Vec4]) -> f64 {
    let centre = id.centre_body();
    let corner_radius = id.corner_radius(centre);
    // Allow for the full terrain height range and the longest mesh skirt.
    let view_radius = corner_radius + 600.0 + 4.0 * id.cell_m();
    let relative = (centre - eye_body).as_vec3();
    if planes.iter().any(|p| p.truncate().dot(relative) + p.w < -(view_radius as f32)) {
        return 0.0;
    }
    let angle = centre.normalize().dot(eye_body.normalize()).clamp(-1.0, 1.0).acos();
    let tile_reach = ((corner_radius + 2_000.0) / RADIUS_M).min(1.0).asin();
    let horizon = (RADIUS_M / eye_body.length()).min(1.0).acos();
    if angle - tile_reach > horizon {
        return 0.0;
    }
    // A patch straddling the eye can be much nearer than its centre. Use its
    // nearest projected surface point, excluding the 2 km culling slack.
    focal_px * id.cell_m() / id.distance_to_patch(eye_body).max(id.cell_m())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face_toward_z(sign: f64) -> u8 {
        (0..6u8)
            .find(|f| {
                let c = TileId::root(*f).centre_body();
                c.z.signum() == sign && c.z.abs() > RADIUS_M * 0.99
            })
            .expect("a face centred on the z axis")
    }

    #[test]
    fn levels_halve_the_cell_and_quarter_the_rect() {
        let root = TileId::root(2);
        assert_eq!(root.cell_m(), 2.0 * HALF_M / 64.0);
        let kids = root.children();
        assert!(kids.iter().all(|k| k.cell_m() == HALF_M / 64.0 && k.parent() == Some(root)));
        assert_eq!(
            TileId { face: 2, level: 10, x: 1023, y: 1023 }.cell_m(),
            2.0 * HALF_M / 65_536.0
        );
    }

    #[test]
    fn level_0_corners_land_on_the_sphere_and_meet_across_faces() {
        for face in 0..6u8 {
            for p in TileId::root(face).corners_body() {
                assert!((p.length() - RADIUS_M).abs() < 1e-6, "face {face}");
            }
        }
        let mut pairs_with_two = 0;
        for fa in 0..6u8 {
            for fb in (fa + 1)..6 {
                let a = TileId::root(fa).corners_body();
                let b = TileId::root(fb).corners_body();
                let shared = a.iter().filter(|p| b.iter().any(|q| p.distance(*q) < 1e-6)).count();
                assert!(shared <= 2, "faces {fa},{fb} share {shared}");
                pairs_with_two += usize::from(shared == 2);
            }
        }
        assert!(pairs_with_two > 0);
    }

    #[test]
    fn samples_cover_the_rect_inclusive_and_match_the_chart() {
        let id = TileId { face: 3, level: 4, x: 5, y: 9 };
        assert_eq!(id.sample_chart(0, 0), id.chart_min());
        assert_eq!(id.sample_chart(64, 64), id.chart_max());
        assert!((id.chart_max().x - id.chart_min().x - 64.0 * id.cell_m()).abs() < 1e-9);
    }

    #[test]
    fn projected_error_is_zero_behind_the_limb_and_scales_with_distance() {
        let eye = DVec3::new(0.0, 0.0, RADIUS_M + 100_000.0);
        let front = TileId::root(face_toward_z(1.0));
        let back = TileId::root(face_toward_z(-1.0));
        assert_eq!(projected_error_px(back, eye, 900.0), 0.0);
        let near = projected_error_px(front, eye, 900.0);
        let far = projected_error_px(front, eye * 2.0, 900.0);
        assert!(near > far && far > 0.0);
    }

    #[test]
    fn a_tile_straddling_the_limb_keeps_its_error() {
        let root = TileId::root(0);
        let c = root.centre_body().normalize();
        let u = c.cross(DVec3::Y).normalize();
        let d = RADIUS_M + 100_000.0;
        let horizon = (RADIUS_M / d).acos();
        let a = horizon + 0.1;
        assert!((root.bounding_radius_m() / RADIUS_M).min(1.0).asin() > 0.1);
        let eye = (c * a.cos() + u * a.sin()) * d;
        assert!(projected_error_px(root, eye, 900.0) > 0.0);
    }
}
