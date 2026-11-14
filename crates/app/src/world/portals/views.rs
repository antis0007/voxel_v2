//! Bounded framebuffer portal view planning. All world transforms stay in f64;
//! GPU positions and clip planes are relative to each view's own eye.
use super::{Frame, MAX_TRAVERSALS, Network, Portal, Realm};
use glam::{DMat4, DQuat, DVec3, DVec4};

/// The primary view plus four destinations; each destination is a full
/// terrain pass, so this bounds the frame cost, not just memory.
pub const MAX_VIEWS: usize = 5;
/// Windows seen through windows; deeper openings show the veil alone.
pub const MAX_VIEW_DEPTH: usize = 2;
/// Two full-resolution 1440p destinations.
pub const MAX_TARGET_PIXELS: u64 = 8_388_608;
pub const MAX_TARGET_SIDE: u32 = 4096;
/// Openings under this share of the screen get no window: a full terrain pass
/// for a few hundred pixels is wasted, and the membrane reads better there.
/// `portal_composite.wgsl` fades windows into the membrane up to twice this.
pub const MIN_WINDOW_SHARE: f64 = 0.0012;

#[derive(Clone, Debug)]
pub struct Camera {
    pub realm: Realm,
    pub eye: DVec3,
    /// Camera local -Z is forward and +Y is up.
    pub rotation: DQuat,
    /// WebGPU clip volume, including reverse Z: -w <= x,y <= w, 0 <= z <= w.
    pub projection: DMat4,
}
impl Camera {
    pub fn valid(&self) -> bool {
        self.realm.validate().is_ok()
            && self.eye.is_finite()
            && self.rotation.is_finite()
            && (self.rotation.length_squared() - 1.0).abs() < 1e-6
            && self.projection.is_finite()
            && self.projection.determinant().abs() > 1e-15
    }
    pub fn through(&self, portal: &Portal) -> Self {
        Self {
            realm: portal.destination.clone(),
            eye: portal.point(self.eye),
            rotation: portal.rotation() * self.rotation,
            projection: self.projection,
        }
    }
    pub fn relative_view_projection(&self) -> DMat4 {
        self.projection * DMat4::from_quat(self.rotation.conjugate())
    }
}

/// Keep the positive half space. Avoid computing n.dot(absolute_position) in
/// f32: subtracting the view origin first preserves the portal seam far away.
#[derive(Clone, Copy, Debug)]
pub struct ClipPlane {
    pub point: DVec3,
    pub normal: DVec3,
}
impl ClipPlane {
    pub fn relative_to(self, eye: DVec3) -> [f32; 4] {
        self.normal
            .extend((eye - self.point).dot(self.normal))
            .as_vec4()
            .to_array()
    }
    fn through(self, portal: &Portal) -> Self {
        Self {
            point: portal.point(self.point),
            normal: portal.vector(self.normal),
        }
    }
    fn distance(self, p: DVec3) -> f64 {
        (p - self.point).dot(self.normal)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub min: [f64; 2],
    pub max: [f64; 2],
}
impl Rect {
    const FULL: Self = Self {
        min: [-1.0, -1.0],
        max: [1.0, 1.0],
    };
    fn intersect(self, other: Self) -> Option<Self> {
        let rect = Self {
            min: [self.min[0].max(other.min[0]), self.min[1].max(other.min[1])],
            max: [self.max[0].min(other.max[0]), self.max[1].min(other.max[1])],
        };
        (rect.min[0] < rect.max[0] && rect.min[1] < rect.max[1]).then_some(rect)
    }
    fn pixels(self, size: [u32; 2]) -> f64 {
        (self.max[0] - self.min[0])
            * (self.max[1] - self.min[1])
            * 0.25
            * size[0] as f64
            * size[1] as f64
    }
}

#[derive(Clone, Debug)]
pub struct View {
    /// None only for the root view. Children composite into this parent.
    pub parent: Option<usize>,
    pub portal_id: Option<u64>,
    pub camera: Camera,
    pub target_size: [u32; 2],
    pub screen_rect: Rect,
    /// Source opening, relative to the parent's camera; the compositor draws
    /// this quad with depth testing/writes, sampling child clip-space UVs.
    pub aperture: [[f32; 3]; 4],
    /// Exit front clipping plus inherited portal aperture side planes.
    pub clip_planes: Vec<ClipPlane>,
    pub depth: usize,
}

#[derive(Default, Debug)]
pub struct Plan {
    pub views: Vec<View>,
}
impl Plan {
    /// Destinations must have their own resident world/cache. `ready` must not
    /// block waiting for a chunk or a GPU query; unready views are omitted.
    pub fn build(
        network: &Network,
        root: Camera,
        size: [u32; 2],
        mut ready: impl FnMut(&Realm, DVec3, Frame) -> bool,
    ) -> Self {
        if !root.valid() || size.contains(&0) || network.validate().is_err() {
            return Self::default();
        }
        let mut plan = Self {
            views: vec![View {
                parent: None,
                portal_id: None,
                camera: root,
                target_size: size,
                screen_rect: Rect::FULL,
                aperture: [[0.0; 3]; 4],
                clip_planes: Vec::new(),
                depth: 0,
            }],
        };
        let mut pixels_left = MAX_TARGET_PIXELS;
        let mut parent = 0;
        while parent < plan.views.len() && plan.views.len() < MAX_VIEWS {
            let view = plan.views[parent].clone();
            if view.depth >= MAX_VIEW_DEPTH.min(MAX_TRAVERSALS) {
                parent += 1;
                continue;
            }
            // Largest openings win a bounded budget, with stable ID ties.
            let through = view.portal_id.and_then(|id| network.portals.iter().find(|p| p.id == id));
            let mut visible: Vec<_> = network
                .portals
                .iter()
                .filter(|p| p.source == view.camera.realm)
                .filter(|p| through.is_none_or(|t| !coincides(p, t)))
                .filter_map(|p| visible_rect(p, &view).map(|rect| (p, rect)))
                .filter(|(_, r)| r.pixels(size) >= (MIN_WINDOW_SHARE * size[0] as f64 * size[1] as f64).max(4.0))
                .collect();
            visible.sort_by(|(a, ar), (b, br)| {
                br.pixels(size)
                    .total_cmp(&ar.pixels(size))
                    .then(a.id.cmp(&b.id))
            });
            for (portal, rect) in visible {
                if plan.views.len() == MAX_VIEWS {
                    break;
                }
                let camera = view.camera.through(portal);
                if !ready(&camera.realm, camera.eye, portal.exit) {
                    continue;
                }
                // Full-aspect targets retain projective UVs. Resolution follows
                // on-screen coverage in eighths, so a window filling the view is
                // pixel-exact and a distant one is cheap; steps limit reallocation.
                let coverage = rect.pixels(size) / (size[0] as f64 * size[1] as f64);
                let eighths = ((coverage.sqrt() * 1.5).clamp(0.25, 1.0) * 8.0).ceil() / 8.0;
                let factor = (MAX_TARGET_SIDE as f64 / size[0].max(size[1]) as f64).min(1.0) * eighths
                    / (1u32 << view.depth.min(3)) as f64;
                let target_size = size.map(|n| (n as f64 * factor).ceil().max(1.0) as u32);
                let pixels = target_size[0] as u64 * target_size[1] as u64;
                if pixels > pixels_left {
                    continue;
                }
                pixels_left -= pixels;
                let corners = portal.entrance.corners();
                let mut clip_planes: Vec<_> =
                    view.clip_planes.iter().map(|p| p.through(portal)).collect();
                clip_planes.push(ClipPlane {
                    point: portal.exit.center,
                    normal: portal.exit.normal(),
                });
                // Restrict descendants to the parent's aperture, including
                // oblique openings; an axis-aligned screen rect alone leaks.
                for i in 0..4 {
                    let a = corners[i] - view.camera.eye;
                    let b = corners[(i + 1) % 4] - view.camera.eye;
                    if let Some(mut normal) = a.cross(b).try_normalize() {
                        if normal.dot(portal.entrance.center - view.camera.eye) < 0.0 {
                            normal = -normal;
                        }
                        clip_planes.push(ClipPlane {
                            point: camera.eye,
                            normal: portal.vector(normal),
                        });
                    }
                }
                plan.views.push(View {
                    parent: Some(parent),
                    portal_id: Some(portal.id),
                    camera,
                    target_size,
                    screen_rect: rect,
                    aperture: corners.map(|p| (p - view.camera.eye).as_vec3().to_array()),
                    clip_planes,
                    depth: view.depth + 1,
                });
            }
            parent += 1;
        }
        plan
    }
    /// Framebuffer recursion renders descendants before compositing ancestors.
    pub fn render_order(&self) -> impl Iterator<Item = (usize, &View)> {
        self.views.iter().enumerate().rev()
    }
}

/// An opening lying in the plane a view looks out through: the far face of the
/// same gate. Its corners sit exactly on the exit clip plane, so the clip test
/// alone would keep it and draw it across the whole destination.
pub fn coincides(candidate: &Portal, through: &Portal) -> bool {
    candidate.entrance.center.distance_squared(through.exit.center) < 1e-6
}

fn visible_rect(portal: &Portal, view: &View) -> Option<Rect> {
    if portal.entrance.clip_distance(view.camera.eye) <= 1e-7 {
        return None;
    }
    let mut points = portal.entrance.corners().to_vec();
    for plane in &view.clip_planes {
        points = clip(points, |p| plane.distance(p));
        if points.len() < 3 {
            return None;
        }
    }
    let matrix = view.camera.relative_view_projection();
    let mut polygon: Vec<_> = points
        .into_iter()
        .map(|p| matrix * (p - view.camera.eye).extend(1.0))
        .collect();
    // Clip before dividing by W: a near-plane-straddling aperture otherwise
    // flips bounds or vanishes exactly as the player's camera crosses it.
    for plane in [
        DVec4::new(1.0, 0.0, 0.0, 1.0),
        DVec4::new(-1.0, 0.0, 0.0, 1.0),
        DVec4::new(0.0, 1.0, 0.0, 1.0),
        DVec4::new(0.0, -1.0, 0.0, 1.0),
    ] {
        polygon = clip(polygon, |p| p.dot(plane));
        if polygon.len() < 3 {
            return None;
        }
    }
    // The aperture compositor depth-clamps the opening. Applying the camera's
    // near plane here would hide the whole window just before crossing it.
    polygon = clip(polygon, |p| p.w - 1e-9);
    if polygon.len() < 3 {
        return None;
    }
    let mut rect = Rect {
        min: [f64::INFINITY; 2],
        max: [f64::NEG_INFINITY; 2],
    };
    for p in polygon {
        if p.w <= 1e-12 {
            return None;
        }
        for axis in 0..2 {
            let v = p[axis] / p.w;
            rect.min[axis] = rect.min[axis].min(v);
            rect.max[axis] = rect.max[axis].max(v);
        }
    }
    rect.intersect(view.screen_rect)
}

fn clip<T>(polygon: Vec<T>, distance: impl Fn(T) -> f64) -> Vec<T>
where
    T: Copy
        + std::ops::Sub<Output = T>
        + std::ops::Add<Output = T>
        + std::ops::Mul<f64, Output = T>,
{
    let Some(&last) = polygon.last() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(polygon.len() + 1);
    let mut a = last;
    let mut da = distance(a);
    for b in polygon {
        let db = distance(b);
        if (da >= 0.0) != (db >= 0.0) {
            out.push(a + (b - a) * (da / (da - db)));
        }
        if db >= 0.0 {
            out.push(b);
        }
        a = b;
        da = db;
    }
    out
}
