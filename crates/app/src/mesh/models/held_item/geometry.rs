//! Held cube, torch, pan and sprite meshes in the grip frame.
use super::*;

pub(super) struct Out<'a> {
    pub(super) m: &'a mut Model,
    pub(super) centre: Vec3,
    pub(super) rotation: Quat,
}

impl Out<'_> {
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: [f32; 3], glow: f32) {
        let surface = if glow > 0.0 { [0.48, 0.42, GLOW_KIND, glow] } else { [0.48, 0.42, KIND, 0.2] };
        self.face(corners, normal, color, surface);
    }

    fn face(&mut self, corners: [Vec3; 4], normal: Vec3, color: [f32; 3], surface: [f32; 4]) {
        let world = corners.map(|c| (self.centre + self.rotation * c) / self.m.scale);
        let n = self.rotation * normal;
        let order = if (world[1] - world[0]).cross(world[2] - world[0]).dot(n) > 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
        for i in order {
            let surface = if surface[2] == KIND {
                [corners[i].x, corners[i].y, surface[2], surface[3]]
            } else { surface };
            self.m.vertices.push(Vertex3 { pos: world[i].to_array(), color, normal: n.to_array(), surface, ao_strength: 1.0 });
        }
    }

    /// One flame facet. `surface.w` is `-2 - lift`, the same packing as a placed torch.
    fn flame_quad(&mut self, corners: [Vec3; 4], colors: [[f32; 3]; 4], lifts: [f32; 4], normal: Vec3) {
        let geometric = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        let normal = if normal.length_squared() > 0.5 { normal } else { geometric }.normalize_or_zero();
        if normal.length_squared() < 0.5 {
            return;
        }
        let surface = [0.48, 0.42, KIND, -2.0];
        let world = corners.map(|c| (self.centre + self.rotation * c) / self.m.scale);
        let n = self.rotation * normal;
        let order = if (world[1] - world[0]).cross(world[2] - world[0]).dot(n) > 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
        for i in order {
            let mut surface = surface;
            surface[0] = corners[i].x;
            surface[1] = corners[i].y;
            surface[3] = -2.0 - lifts[i].clamp(0.0, 1.0);
            self.m.vertices.push(Vertex3 { pos: world[i].to_array(), color: colors[i], normal: n.to_array(), surface, ao_strength: 1.0 });
        }
    }

    /// Closed box on the torch axis from `y0` to `y1`, `half` wide; `w` is the torch surface word.
    fn torch_box(&mut self, y0: f32, y1: f32, half: f32, color: [f32; 3], w: f32) {
        let surface = [0.48, 0.42, KIND, w];
        let p = |x: f32, y: f32, z: f32| Vec3::new(x * half, y, z * half);
        self.face([p(-1.0, y0, 1.0), p(1.0, y0, 1.0), p(1.0, y1, 1.0), p(-1.0, y1, 1.0)], Vec3::Z, color, surface);
        self.face([p(-1.0, y0, -1.0), p(1.0, y0, -1.0), p(1.0, y1, -1.0), p(-1.0, y1, -1.0)], Vec3::NEG_Z, color, surface);
        self.face([p(1.0, y0, -1.0), p(1.0, y0, 1.0), p(1.0, y1, 1.0), p(1.0, y1, -1.0)], Vec3::X, color, surface);
        self.face([p(-1.0, y0, -1.0), p(-1.0, y0, 1.0), p(-1.0, y1, 1.0), p(-1.0, y1, -1.0)], Vec3::NEG_X, color, surface);
        self.face([p(-1.0, y1, -1.0), p(1.0, y1, -1.0), p(1.0, y1, 1.0), p(-1.0, y1, 1.0)], Vec3::Y, color, surface);
        self.face([p(-1.0, y0, -1.0), p(1.0, y0, -1.0), p(1.0, y0, 1.0), p(-1.0, y0, 1.0)], Vec3::NEG_Y, color, surface);
    }
}

pub(super) fn torch_y(world_y: f32) -> f32 {
    (world_y - HELD_GRIP_Y) * HELD_TORCH_SCALE
}

/// The standing torch from `append_torch_mesh`, scaled into the fist.
/// The flame is the same six-sided taper and side tongue, capped so the shell stays closed.
pub(super) fn torch(out: &mut Out, lit: bool) {
    use crate::mesh::prop_palette::{IRON, LEATHER, OAK};
    let s = HELD_TORCH_SCALE;
    out.torch_box(torch_y(0.06), torch_y(0.72), 0.045 * s, OAK, 0.0);
    for grip in [0.31_f32, 0.37, 0.43] {
        out.torch_box(torch_y(grip) - 0.022 * s, torch_y(grip) + 0.022 * s, 0.051 * s, LEATHER, 0.1);
    }
    for collar in [0.53_f32, 0.65] {
        out.torch_box(torch_y(collar) - 0.018 * s, torch_y(collar) + 0.018 * s, 0.061 * s, IRON, 0.4);
    }
    let (fuel, detail) = if lit { ([1.00, 0.44, 0.16], -1.0) } else { ([0.12, 0.09, 0.07], 0.0) };
    out.torch_box(torch_y(0.67), torch_y(0.80), 0.065 * s, fuel, detail);
    if lit { held_flame(out, TORCH_FLAME); }
}

pub(super) fn held_flame(out: &mut Out, center: f32) {
    let s = HELD_TORCH_SCALE;
    let rings = [(-0.08_f32, 0.045, 0.0), (0.01, 0.10, 0.0), (0.13, 0.055, 0.018), (0.27, 0.003, 0.045)];
    let colors = [[1.0, 0.88, 0.47], [1.0, 0.64, 0.26], [1.0, 0.42, 0.09], [1.0, 0.29, 0.055]];
    let span = rings[3].0 - rings[0].0;
    let lift = |h: f32| ((h - rings[0].0) / span).clamp(0.0, 1.0);
    let at = |ring: usize, side: usize| {
        let (h, r, bend) = rings[ring];
        let angle = side as f32 * std::f32::consts::TAU / 6.0;
        Vec3::new((angle.cos() * r + bend) * s, center + h * s, angle.sin() * r * s)
    };
    let rim = |ring: usize| -> [Vec3; 6] { std::array::from_fn(|side| at(ring, side)) };
    flame_cap(out, rim(0), colors[0], lift(rings[0].0), false);
    for band in 0..3 {
        for side in 0..6 {
            let next = (side + 1) % 6;
            let h0 = rings[band].0;
            let h1 = rings[band + 1].0;
            out.flame_quad(
                [at(band, side), at(band + 1, side), at(band + 1, next), at(band, next)],
                [colors[band], colors[band + 1], colors[band + 1], colors[band]],
                [lift(h0), lift(h1), lift(h1), lift(h0)],
                Vec3::ZERO,
            );
        }
    }
    flame_cap(out, rim(3), colors[3], lift(rings[3].0), true);
    let place = |x: f32, y: f32, z: f32| Vec3::new(x * s, center + y * s, z * s);
    let stations = [place(-0.065, 0.025, 0.0), place(-0.105, 0.12, 0.0), place(-0.08, 0.225, 0.015)];
    let radii = [0.025_f32, 0.035, 0.002];
    let tongue = [1.0, 0.42, 0.09];
    let ring = |index: usize, side: usize| {
        let angle = side as f32 * std::f32::consts::FRAC_PI_2;
        stations[index] + Vec3::new(angle.cos(), 0.0, angle.sin()) * radii[index] * s
    };
    let tongue_rim = |index: usize| -> [Vec3; 4] { std::array::from_fn(|side| ring(index, side)) };
    flame_cap(out, tongue_rim(0), tongue, lift(0.025), false);
    for band in 0..2 {
        let h0 = [0.025_f32, 0.12][band];
        let h1 = [0.12_f32, 0.225][band];
        for side in 0..4 {
            let next = (side + 1) % 4;
            out.flame_quad(
                [ring(band, side), ring(band + 1, side), ring(band + 1, next), ring(band, next)],
                [tongue; 4],
                [lift(h0), lift(h1), lift(h1), lift(h0)],
                Vec3::ZERO,
            );
        }
    }
    flame_cap(out, tongue_rim(2), tongue, lift(0.225), true);
}

pub(super) fn flame_cap<const N: usize>(out: &mut Out, rim: [Vec3; N], color: [f32; 3], lift: f32, tip: bool) {
    let center = rim.into_iter().sum::<Vec3>() / N as f32;
    // The ring walks from +X toward +Z, so a fan cross points down.
    // Base quads run that rim backwards; the tip runs with it. The cap normal
    // is chosen so the shared edge is traversed the other way.
    let normal = if tip { Vec3::Y } else { Vec3::NEG_Y };
    for i in 0..N {
        let (a, b) = (rim[i], rim[(i + 1) % N]);
        out.flame_quad([center, a, b, b], [color; 4], [lift; 4], normal);
    }
}

pub(super) fn block_glow(block: BlockId) -> f32 {
    if matches!(block, BlockId::Crystal | BlockId::GlowCap | BlockId::Lava) { 1.2 } else { 0.0 }
}

pub(super) fn cube(out: &mut Out, block: BlockId) {
    let h = CUBE_EDGE * 0.5;
    let s = CUBE_EDGE / FACE as f32;
    let glow = block_glow(block);
    // (normal, texel column axis, texel row axis): rows run down the sides and front to back on top.
    let faces = [
        (Vec3::Z, Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::NEG_Y),
        (Vec3::X, Vec3::NEG_Z, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::NEG_Y),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::NEG_Z),
    ];
    for (n, u, w) in faces {
        let top = n.y != 0.0;
        let origin = n * h - u * h - w * h;
        for y in 0..FACE {
            for x in 0..FACE {
                let p = |dx: u32, dy: u32| origin + u * ((x + dx) as f32 * s) + w * ((y + dy) as f32 * s);
                out.quad([p(0, 0), p(1, 0), p(1, 1), p(0, 1)], n, held_art::block_texel(block, top, x, y), glow);
            }
        }
    }
}

// The vertex shader performs the single sRGB decode.
/// The near rim passes through the grip. The upper surface is a recessed bowl.
pub(super) fn pan(out: &mut Out) {
    const SIDES: usize = 12;
    let centre = Vec3::new(-0.084, 0.0, -0.063);
    let point = |i: usize, radius: f32, height: f32| {
        let (sin, cos) = (i as f32 * std::f32::consts::TAU / SIDES as f32).sin_cos();
        centre + Vec3::new(radius * cos, height, radius * sin)
    };
    let profile = [
        (0.058, -0.039),
        (0.105, -0.004),
        (0.105, 0.002),
        (0.095, 0.002),
        (0.055, -0.031),
    ];
    let colors = [[0.30, 0.32, 0.36], [0.52, 0.53, 0.57], [0.67, 0.68, 0.70], [0.41, 0.44, 0.49]];
    for i in 0..SIDES {
        let radial = (point(i, 1.0, 0.0) + point(i + 1, 1.0, 0.0) - 2.0 * centre).normalize();
        for (j, pair) in profile.windows(2).enumerate() {
            let [(r0, y0), (r1, y1)] = [pair[0], pair[1]];
            let normal = (radial * (y1 - y0) + Vec3::Y * (r0 - r1)).normalize();
            out.quad([point(i, r0, y0), point(i + 1, r0, y0),
                point(i + 1, r1, y1), point(i, r1, y1)], normal, colors[j], 0.0);
        }
    }
    for (radius, height, normal, color) in [
        (0.058, -0.039, Vec3::NEG_Y, [0.26, 0.28, 0.32]),
        (0.055, -0.031, Vec3::Y, [0.34, 0.37, 0.41]),
    ] {
        for i in (0..SIDES).step_by(2) {
            out.quad([centre + Vec3::Y * height, point(i, radius, height),
                point(i + 1, radius, height), point(i + 2, radius, height)], normal, color, 0.0);
        }
    }
}

pub(super) fn srgb(v: u8) -> f32 { v as f32 / 255.0 }

fn wood_haft(out: &mut Out, tip: f32, texel: f32, color: [f32; 3]) {
    let cross_section = [
        (-0.32, -0.49), (0.32, -0.49), (0.52, -0.30), (0.52, 0.30),
        (0.32, 0.49), (-0.32, 0.49), (-0.52, 0.30), (-0.52, -0.30),
    ];
    let face_shades = [0.75, 0.71, 0.76, 0.86, 0.93, 0.85, 0.70, 0.67];
    let heel = -4.75 * texel;
    let point = |i: usize, y: f32| {
        let (x, z) = cross_section[i % cross_section.len()];
        Vec3::new(x * texel, y, z * texel)
    };
    for (i, shade) in face_shades.into_iter().enumerate() {
        let normal = (point(i, 0.0) + point(i + 1, 0.0)).normalize();
        out.quad(
            [point(i, heel), point(i + 1, heel), point(i + 1, tip), point(i, tip)],
            normal,
            color.map(|channel| channel * shade),
            0.0,
        );
    }
    for (height, normal) in [(heel, Vec3::NEG_Y), (tip, Vec3::Y)] {
        for i in (0..cross_section.len()).step_by(2) {
            out.quad(
                [Vec3::Y * height, point(i, height), point(i + 1, height), point(i + 2, height)],
                normal,
                color.map(|channel| channel * 0.66),
                0.0,
            );
        }
    }
}

pub(super) fn sprite(out: &mut Out, icon: Icon, grip: &SpriteGrip) {
    let (w, h, mut texels) = held_art::icon_texels(icon);
    let s = grip.texel;
    let mut has_haft = false;
    if grip.shaft && (w, h) == (16, 16)
        && let Some((_, rgb)) = texels[11 * w + 4]
    {
        if matches!(icon, Icon::Hoe) {
            // The hoe's native sheet is a single image. Its head ends at row six.
            texels[7 * w..].fill(None);
            for texel in &mut texels {
                if matches!(texel, Some((0, _))) {
                    *texel = None;
                }
            }
            wood_haft(out, 5.0 * std::f32::consts::SQRT_2 * s, s, rgb.map(srgb));
            has_haft = true;
        } else if let Some(head) = held_art::held_head(icon) {
            texels = head;
            wood_haft(out, 6.0 * std::f32::consts::SQRT_2 * s, s, rgb.map(srgb));
            has_haft = true;
        }
    }
    // Authored sockets can land in a transparent pixel on another material
    // variant. Keep the socket when it already touches art or the modeled haft.
    let mut socket = grip.socket;
    if !has_haft {
        let nearest = texels.iter().enumerate().filter_map(|(i, texel)| {
            texel.as_ref()?;
            let x = (i % w) as f32 + 0.5 - w as f32 * 0.5;
            let y = h as f32 * 0.5 - (i / w) as f32 - 0.5;
            let pixel = Vec3::new(x, y, 0.0);
            let gap = ((pixel - socket).abs() - Vec3::splat(0.5)).max(Vec3::ZERO);
            Some((pixel, gap.length_squared()))
        }).min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((pixel, distance)) = nearest && distance > 0.0 {
            socket = pixel;
        }
    }
    let rotation = out.rotation * Quat::from_rotation_z(grip.align);
    let centre = out.centre - rotation * (socket * s);
    let mut out = Out { m: out.m, centre, rotation };
    let half_depth = grip.half_depth;
    let at = |x: i32, y: i32| (x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h).then(|| texels[y as usize * w + x as usize]).flatten();
    let point = |x: f32, y: f32, z: f32| Vec3::new((x - w as f32 * 0.5) * s, (h as f32 * 0.5 - y) * s, z * s);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let Some((_, rgb)) = at(x, y) else { continue };
            let color = rgb.map(srgb);
            let (x0, x1, y0, y1) = (x as f32, x as f32 + 1.0, y as f32, y as f32 + 1.0);
            for z in [half_depth, -half_depth] {
                out.quad([point(x0, y1, z), point(x1, y1, z), point(x1, y0, z), point(x0, y0, z)], Vec3::Z * z.signum(), color, 0.0);
            }
            for (dx, dy, n) in [(-1, 0, Vec3::NEG_X), (1, 0, Vec3::X), (0, -1, Vec3::Y), (0, 1, Vec3::NEG_Y)] {
                if at(x + dx, y + dy).is_some() { continue; }
                let (a, b) = if dx != 0 {
                    let ex = if dx > 0 { x1 } else { x0 };
                    ((ex, y0), (ex, y1))
                } else {
                    let ey = if dy > 0 { y1 } else { y0 };
                    ((x0, ey), (x1, ey))
                };
                let edge_color = color.map(|channel| channel * 0.90);
                out.quad([point(a.0, a.1, -half_depth), point(b.0, b.1, -half_depth), point(b.0, b.1, half_depth), point(a.0, a.1, half_depth)], n, edge_color, 0.0);
            }
        }
    }
}
