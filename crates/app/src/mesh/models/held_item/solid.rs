//! Produce, plants and food held as rounded voxel solids built from their 16x16 sprites.
//!
//! The sprite's silhouette is filled as a union of balls on its distance field, so a
//! round apple swells into a sphere, a carrot into a tapered cone and a one-texel stem
//! stays one texel thick. Depths snap to half texels to keep the stepped voxel look.
use super::geometry::{Out, soften_ink, srgb};
use glam::{Vec2, Vec3};

type Texels = Vec<Option<(u8, [u8; 3])>>;

/// Half-depth in texels of every filled texel; 0 where the sprite is empty.
pub(super) fn ball_depths(w: usize, h: usize, texels: &[Option<(u8, [u8; 3])>], flatten: f32) -> Vec<f32> {
    // Chamfer distance from each filled texel to the nearest empty one (outside the sheet is empty).
    const DIAG: f32 = std::f32::consts::SQRT_2;
    let mut dist: Vec<f32> = texels.iter().map(|t| if t.is_some() { f32::MAX } else { 0.0 }).collect();
    let get = |dist: &[f32], x: i32, y: i32| {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h { 0.0 } else { dist[y as usize * w + x as usize] }
    };
    for pass in 0..2 {
        let (step, offsets): (i32, [(i32, i32, f32); 4]) = if pass == 0 {
            (1, [(-1, 0, 1.0), (0, -1, 1.0), (-1, -1, DIAG), (1, -1, DIAG)])
        } else {
            (-1, [(1, 0, 1.0), (0, 1, 1.0), (1, 1, DIAG), (-1, 1, DIAG)])
        };
        let mut i = if step > 0 { 0 } else { w * h - 1 };
        for _ in 0..w * h {
            if dist[i] > 0.0 {
                let (x, y) = ((i % w) as i32, (i / w) as i32);
                let best = offsets.iter().map(|&(dx, dy, c)| get(&dist, x + dx, y + dy) + c).fold(f32::MAX, f32::min);
                dist[i] = dist[i].min(best);
            }
            i = i.wrapping_add_signed(step as isize);
        }
    }
    let mut depth = vec![0.0; w * h];
    for i in 0..w * h {
        if dist[i] <= 0.0 { continue; }
        let p = Vec2::new((i % w) as f32, (i / w) as f32);
        let mut best = 0.0f32;
        for j in 0..w * h {
            let radius = dist[j] - 0.5;
            if radius <= 0.0 { continue; }
            let q = Vec2::new((j % w) as f32, (j / w) as f32);
            let gap = radius * radius - p.distance_squared(q);
            if gap > best * best { best = gap.sqrt(); }
        }
        depth[i] = ((best * flatten * 2.0).round() * 0.5).max(0.5);
    }
    depth
}

/// Appends the solid with the sprite's filled centroid plus `socket` (texels, +Y up) at `out.centre`.
pub(super) fn solid(out: &mut Out, (w, h, mut texels): (usize, usize, Texels), texel: f32, socket: Vec2, flatten: f32) {
    soften_ink(w, h, &mut texels);
    let depth = ball_depths(w, h, &texels, flatten);
    let filled: Vec<Vec2> = (0..w * h).filter(|&i| texels[i].is_some())
        .map(|i| Vec2::new((i % w) as f32 + 0.5 - w as f32 * 0.5, h as f32 * 0.5 - (i / w) as f32 - 0.5)).collect();
    if filled.is_empty() { return; }
    let centroid = filled.iter().copied().sum::<Vec2>() / filled.len() as f32 + socket;
    let point = |x: f32, y: f32, z: f32| {
        Vec3::new((x - w as f32 * 0.5 - centroid.x) * texel, (h as f32 * 0.5 - y - centroid.y) * texel, z * texel)
    };
    let at = |x: i32, y: i32| {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h { 0.0 } else { depth[y as usize * w + x as usize] }
    };
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let Some((_, rgb)) = texels[y as usize * w + x as usize] else { continue };
            let color = rgb.map(srgb);
            let half = at(x, y);
            let (x0, x1, y0, y1) = (x as f32, x as f32 + 1.0, y as f32, y as f32 + 1.0);
            for z in [half, -half] {
                out.quad([point(x0, y1, z), point(x1, y1, z), point(x1, y0, z), point(x0, y0, z)], Vec3::Z * z.signum(), color, 0.0);
            }
            for (dx, dy, n) in [(-1, 0, Vec3::NEG_X), (1, 0, Vec3::X), (0, -1, Vec3::Y), (0, 1, Vec3::NEG_Y)] {
                let inner = at(x + dx, y + dy);
                if inner >= half { continue; }
                let (a, b) = if dx != 0 {
                    let ex = if dx > 0 { x1 } else { x0 };
                    ((ex, y0), (ex, y1))
                } else {
                    let ey = if dy > 0 { y1 } else { y0 };
                    ((x0, ey), (x1, ey))
                };
                let side = color.map(|c| c * 0.86);
                let spans: &[(f32, f32)] = if inner > 0.0 { &[(inner, half), (-half, -inner)] } else { &[(-half, half)] };
                for &(lo, hi) in spans {
                    out.quad([point(a.0, a.1, lo), point(b.0, b.1, lo), point(b.0, b.1, hi), point(a.0, a.1, hi)], n, side, 0.0);
                }
            }
        }
    }
}
