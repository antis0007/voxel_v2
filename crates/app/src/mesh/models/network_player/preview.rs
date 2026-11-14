//! Contact sheet of the remote character across a stride and a swing.
use super::super::Model;
use glam::{Vec3, Vec2};

const CELL: usize = 160;
const FRAMES: usize = 8;

/// Z-buffered flat-shaded raster of a triangle list; `view` maps a point to (screen, depth).
fn raster(img: &mut image::RgbImage, ox: usize, m: &Model, oy: usize, view: impl Fn(Vec3) -> (Vec2, f32)) {
    let mut depth = vec![f32::INFINITY; CELL * CELL];
    let light = Vec3::new(-0.4, 0.8, -0.45).normalize();
    for tri in m.vertices.chunks_exact(3) {
        let p = tri.iter().map(|v| view(Vec3::from_array(v.pos))).collect::<Vec<_>>();
        let n = Vec3::from_array(tri[0].normal);
        let shade = 0.45 + 0.55 * n.dot(light).max(0.0);
        let c = tri[0].color.map(|c| ((c * shade).clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8);
        let (a, b, d) = (p[0].0, p[1].0, p[2].0);
        let area = (b - a).perp_dot(d - a);
        if area.abs() < 1e-6 { continue; }
        let min = a.min(b).min(d).max(Vec2::ZERO);
        let max = a.max(b).max(d).min(Vec2::splat(CELL as f32 - 1.0));
        for y in min.y as usize..=max.y as usize {
            for x in min.x as usize..=max.x as usize {
                let q = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = (b - q).perp_dot(d - q) / area;
                let w1 = (d - q).perp_dot(a - q) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 { continue; }
                let z = w0 * p[0].1 + w1 * p[1].1 + w2 * p[2].1;
                if z < depth[y * CELL + x] {
                    depth[y * CELL + x] = z;
                    img.put_pixel((ox + x) as u32, (oy + y) as u32, image::Rgb(c));
                }
            }
        }
    }
}

#[test]
#[ignore = "writes target/review/network_player_gait.png"]
fn dump_network_player_gait_sheet() {
    // Rows: side walk, front walk, side run with a swing, side attack standing.
    let rows: [(bool, f32, f32); 4] = [(true, 0.6, -1.0), (false, 0.6, -1.0), (true, 1.0, -1.0), (true, 0.0, 0.0)];
    let mut img = image::RgbImage::from_pixel((CELL * FRAMES) as u32, (CELL * rows.len()) as u32, image::Rgb([70, 74, 78]));
    for (r, &(side, stride, attack)) in rows.iter().enumerate() {
        for f in 0..FRAMES {
            let t = f as f32 / FRAMES as f32;
            let mut m = Model::new(1.0);
            let attack = if attack < 0.0 { 0.0 } else { t };
            super::draw(&mut m, Vec3::ZERO, 0.0, 0.0, t * std::f32::consts::TAU, stride, false, attack, 0);
            // Character faces -Z: the side view puts its front on the left, the front view faces the camera.
            let s = CELL as f32 / 2.4;
            raster(&mut img, f * CELL, &m, r * CELL, |p| {
                let (h, z) = if side { (p.z, -p.x) } else { (p.x, p.z) };
                (Vec2::new(CELL as f32 * 0.5 + h * s, CELL as f32 * 0.92 - p.y * s), z)
            });
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/review/network_player_gait.png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    img.save(&path).unwrap();
}
