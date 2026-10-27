//! Wand families from `assets/wands.ron`, extruded once and recoloured per draw.
//! Local frame: origin at the grip socket, +Y along the shaft to the tip, metres.
use super::{Model, Vertex3};
use crate::wand_appearance::{self as table, Family, GRIP_KEYS, SHAFT_KEYS, Sprite, TABLE};
use crate::wand_style::{GRIPS, SHAFTS, WandStyle};
use glam::{Mat4, Quat, Vec3};
use std::sync::LazyLock;

/// Solid wand material in the model shader.
pub const BODY_KIND: f32 = 55.0;
const GLOW_KIND: f32 = 40.0;
/// Below this cast energy emitters stay painted rather than glowing.
const QUIET: f32 = 0.04;

struct Built {
    vertices: Vec<Vertex3>,
    keys: Vec<u8>,
    sprite: Sprite,
    pixel: f32,
    fixed: [[f32; 3]; 128],
    emitter: [bool; 128],
}

static KIT: LazyLock<Vec<Option<Built>>> = LazyLock::new(|| {
    if let Err(error) = TABLE.validate() { eprintln!("[wand-kit] assets/wands.ron: {error}"); }
    let pixel = table::pixel_m();
    TABLE.families.iter().map(|f| f.sprite(TABLE.width, pixel).ok().map(|s| build(f, s, pixel))).collect()
});

fn built(head: u8) -> Option<&'static Built> {
    KIT.get(TABLE.family_index(head))?.as_ref()
}

/// Run width through (x, y), capped: thin parts extrude square, broad parts stay slim.
fn depth(s: &Sprite, x: i32, y: i32, cap: u8) -> f32 {
    if s.at(x, y) == 0 { return 0.0; }
    let (mut a, mut b) = (x, x);
    while s.at(a - 1, y) != 0 { a -= 1; }
    while s.at(b + 1, y) != 0 { b += 1; }
    ((b - a + 1) as f32).min(cap as f32)
}

fn build(family: &Family, sprite: Sprite, pixel: f32) -> Built {
    let mut fixed = [[0.0; 3]; 128];
    let mut emitter = [false; 128];
    for (&key, m) in &family.materials {
        let k = key as usize & 127;
        fixed[k] = table::parse_srgb(&m.srgb).unwrap_or([1.0, 0.0, 1.0]);
        emitter[k] = m.emitter;
    }
    let (mut vertices, mut keys) = (Vec::new(), Vec::new());
    let half_w = sprite.width as f32 * 0.5;
    let corner = |x: f32, y: f32, z: f32| Vec3::new((x - half_w) * pixel, (sprite.grip_row - y) * pixel, z * pixel);
    let mut quad = |p: [Vec3; 4], n: Vec3, key: u8| {
        let order = if (p[1] - p[0]).cross(p[2] - p[0]).dot(n) > 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
        for i in order {
            vertices.push(Vertex3 { pos: p[i].to_array(), color: [1.0; 3], normal: n.to_array(), surface: [0.48, 0.42, BODY_KIND, 0.2], ao_strength: 1.0 });
            keys.push(key & 127);
        }
    };
    for y in 0..sprite.height as i32 {
        let mut x = 0;
        while x < sprite.width as i32 {
            let key = sprite.at(x, y);
            if key == 0 { x += 1; continue; }
            let d = depth(&sprite, x, y, family.max_depth);
            // Front and back faces merge along runs of one key and depth.
            let mut end = x + 1;
            while sprite.at(end, y) == key && depth(&sprite, end, y, family.max_depth) == d { end += 1; }
            let (x0, x1, yt, yb) = (x as f32, end as f32, y as f32, y as f32 + 1.0);
            for s in [1.0f32, -1.0] {
                let z = d * 0.5 * s;
                quad([corner(x0, yb, z), corner(x1, yb, z), corner(x1, yt, z), corner(x0, yt, z)], Vec3::Z * s, key);
            }
            for cx in x..end {
                for (dx, dy, n) in [(-1, 0, Vec3::NEG_X), (1, 0, Vec3::X), (0, -1, Vec3::Y), (0, 1, Vec3::NEG_Y)] {
                    let nd = depth(&sprite, cx + dx, y + dy, family.max_depth);
                    if nd >= d { continue; }
                    let (a, b) = if dx != 0 {
                        let ex = cx as f32 + if dx > 0 { 1.0 } else { 0.0 };
                        ((ex, yt), (ex, yb))
                    } else {
                        let ey = if dy > 0 { yb } else { yt };
                        ((cx as f32, ey), (cx as f32 + 1.0, ey))
                    };
                    // A shallower neighbour leaves two exposed bands, an empty one a full face.
                    let bands: &[(f32, f32)] = if nd == 0.0 { &[(-d * 0.5, d * 0.5)] } else { &[(-d * 0.5, -nd * 0.5), (nd * 0.5, d * 0.5)] };
                    for &(z0, z1) in bands {
                        quad([corner(a.0, a.1, z0), corner(b.0, b.1, z0), corner(b.0, b.1, z1), corner(a.0, a.1, z1)], n, key);
                    }
                }
            }
            x = end;
        }
    }
    Built { vertices, keys, sprite, pixel, fixed, emitter }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Linear colours per key for one style: shaft ramp, grip ramp, family materials.
fn palette(b: &Built, style: WandStyle) -> [[f32; 3]; 128] {
    let style = style.clamped();
    let shaft = &SHAFTS[style.shaft as usize];
    let grip = GRIPS[style.grip as usize].1;
    let mut p = b.fixed;
    let ramp = [shaft.dark.map(|c| c * 0.6), shaft.dark, shaft.base, mix(shaft.base, shaft.accent, 0.45)];
    for (k, c) in SHAFT_KEYS.iter().zip(ramp) { p[*k as usize] = c; }
    let grips = [grip.map(|c| c * 0.55), grip, grip.map(|c| (c * 1.35 + 0.04).min(1.0))];
    for (k, c) in GRIP_KEYS.iter().zip(grips) { p[*k as usize] = c; }
    p
}

/// Casting origin in the local frame.
pub(crate) fn muzzle(style: WandStyle) -> Vec3 {
    built(style.clamped().head).map_or(Vec3::Y * 0.4, |b| {
        let s = &b.sprite;
        Vec3::new((s.muzzle[0] - s.width as f32 * 0.5) * b.pixel, (s.grip_row - s.muzzle[1]) * b.pixel, 0.0)
    })
}

/// Appends the wand in `frame` (metres). Emitters take `glow` and emit with `energy`.
pub(crate) fn draw(m: &mut Model, frame: Mat4, style: WandStyle, glow: [f32; 3], energy: f32, kind: f32) {
    let Some(b) = built(style.clamped().head) else { return };
    let palette = palette(b, style);
    let transform = Mat4::from_scale(Vec3::splat(1.0 / m.scale)) * frame;
    let normal = frame.inverse().transpose();
    let lit = energy > QUIET;
    m.vertices.reserve(b.vertices.len());
    m.vertices.extend(b.vertices.iter().zip(&b.keys).map(|(s, &k)| {
        let mut out = *s;
        out.pos = transform.transform_point3(Vec3::from_array(s.pos)).to_array();
        out.normal = normal.transform_vector3(Vec3::from_array(s.normal)).normalize_or_zero().to_array();
        out.surface[2] = kind;
        out.color = palette[k as usize];
        if b.emitter[k as usize] {
            // Idle emitters lean toward the spell colour without lighting up.
            out.color = mix(out.color, glow, if lit { 1.0 } else { 0.25 });
            if lit {
                out.surface[2] = GLOW_KIND;
                out.surface[3] = energy * 1.6;
            }
        }
        out
    }));
}

/// Style carried in an item's look bits (see `wand_items::look`). The save
/// rejects bits above 11, so items carry no grip and draw the default binding.
pub(crate) fn style_from_look(look: u16) -> WandStyle {
    WandStyle {
        shaft: ((look >> 2) & 7) as u8,
        head: ((look >> 5) & 7) as u8,
        glow: ((look >> 8) & 7) as u8,
        ..WandStyle::default()
    }
    .clamped()
}

/// Items carry no spell, so "spell colour" glows show the shaft accent.
fn item_glow(style: WandStyle) -> [f32; 3] {
    style.glow_colour(SHAFTS[style.shaft as usize].accent)
}

/// A wand lying on the ground, centred on its length.
pub(crate) fn draw_dropped(m: &mut Model, centre: Vec3, rotation: Quat, look: u16) {
    let style = style_from_look(look);
    let Some(b) = built(style.head) else { return };
    let rotation = rotation * Quat::from_rotation_z(0.8);
    let mid = (b.sprite.grip_row - b.sprite.height as f32 * 0.5) * b.pixel;
    let frame = Mat4::from_rotation_translation(rotation, centre - rotation * Vec3::Y * mid);
    draw(m, frame, style, item_glow(style), 0.0, super::AUTHORED_SURFACE);
}

/// Front view at one texel per pixel, sRGB RGBA, for inventory icons.
pub(crate) fn icon_rgba(look: u16) -> Option<([usize; 2], Vec<u8>)> {
    let style = style_from_look(look);
    let b = built(style.head)?;
    let palette = palette(b, style);
    let glow = item_glow(style);
    let srgb = |v: f32| { let v = v.clamp(0.0, 1.0); ((if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }) * 255.0).round() as u8 };
    let s = &b.sprite;
    let mut out = vec![0; s.width * s.height * 4];
    for (i, &k) in s.cells.iter().enumerate().filter(|(_, k)| **k != 0) {
        let k = k as usize & 127;
        let c = if b.emitter[k] { mix(palette[k], glow, 0.25) } else { palette[k] };
        out[i * 4..i * 4 + 4].copy_from_slice(&[srgb(c[0]), srgb(c[1]), srgb(c[2]), 255]);
    }
    Some(([s.width, s.height], out))
}

/// Which modeled family a look draws, for icon caching.
pub(crate) fn icon_key(look: u16) -> (u8, u8, u8, u8) {
    let s = style_from_look(look);
    (s.head, s.shaft, s.grip, s.glow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_styles() -> impl Iterator<Item = WandStyle> {
        (0..6u8).map(|head| WandStyle { head, ..WandStyle::default() })
    }

    #[test]
    fn every_style_builds_closed_lit_geometry() {
        for style in all_styles() {
            let mut m = Model::new(1.0);
            draw(&mut m, Mat4::IDENTITY, style, [1.0, 0.5, 0.2], 0.0, BODY_KIND);
            assert!(!m.vertices.is_empty() && m.vertices.len().is_multiple_of(3));
            assert!(m.vertices.len() < 30_000, "head {} has {} vertices", style.head, m.vertices.len());
            assert!(m.vertices.iter().all(|v| v.surface[2] == BODY_KIND), "idle wand must not glow");
            // Closed surface: area-weighted normals cancel.
            let sum = m.vertices.chunks_exact(3).fold(Vec3::ZERO, |acc, t| {
                let [a, b, c] = [t[0].pos, t[1].pos, t[2].pos].map(Vec3::from_array);
                acc + (b - a).cross(c - a)
            });
            assert!(sum.length() < 1e-6, "head {} is not closed: {sum}", style.head);
            let lit = { let mut m = Model::new(1.0); draw(&mut m, Mat4::IDENTITY, style, [1.0, 0.5, 0.2], 1.0, BODY_KIND); m };
            assert!(lit.vertices.iter().any(|v| v.surface[2] == GLOW_KIND), "head {} has no emitter", style.head);
        }
    }

    #[test]
    fn muzzle_is_above_the_grip_at_the_emitter() {
        for style in all_styles() {
            let tip = muzzle(style);
            assert!(tip.y > 0.25 && tip.y < 0.7 && tip.x.abs() < 0.05, "head {} muzzle {tip}", style.head);
        }
    }

    #[test]
    fn faces_follow_their_winding() {
        let mut m = Model::new(1.0);
        draw(&mut m, Mat4::IDENTITY, WandStyle::default(), [1.0; 3], 0.0, BODY_KIND);
        for t in m.vertices.chunks_exact(3) {
            let [a, b, c] = [t[0].pos, t[1].pos, t[2].pos].map(Vec3::from_array);
            assert!((b - a).cross(c - a).dot(Vec3::from_array(t[0].normal)) > 0.0);
        }
    }

    #[test]
    fn mesh_is_deterministic_and_restyles_without_rebuilding() {
        let digest = |style: WandStyle| {
            let mut m = Model::new(1.0);
            draw(&mut m, Mat4::IDENTITY, style, [1.0; 3], 0.0, BODY_KIND);
            m.vertices.iter().fold(0xcbf29ce484222325u64, |h, v| {
                v.pos.iter().chain(&v.color).fold(h, |h, f| (h ^ f.to_bits() as u64).wrapping_mul(0x100000001b3))
            })
        };
        let a = WandStyle { shaft: 1, grip: 4, ..WandStyle::default() };
        assert_eq!(digest(a), digest(a));
        assert_ne!(digest(a), digest(WandStyle { shaft: 3, ..a }), "shaft material must recolour");
    }

    /// `cargo test dump_wand_icons -- --ignored`: icons at 4x and vertex counts for review.
    #[test]
    #[ignore]
    fn dump_wand_icons() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wand-icons");
        std::fs::create_dir_all(&dir).unwrap();
        for head in 0..6u16 {
            for shaft in 0..6u16 {
                let ([w, h], rgba) = icon_rgba(shaft << 2 | head << 5).unwrap();
                let image = image::RgbaImage::from_raw(w as u32, h as u32, rgba).unwrap();
                let big = image::imageops::resize(&image, w as u32 * 4, h as u32 * 4, image::imageops::FilterType::Nearest);
                big.save(dir.join(format!("h{head}-s{shaft}.png"))).unwrap();
            }
            let b = built(head as u8).unwrap();
            println!("head {head} {}: {}x{} texels, {} vertices, {} bytes", TABLE.families[TABLE.family_index(head as u8)].id, b.sprite.width, b.sprite.height, b.vertices.len(), b.vertices.len() * (std::mem::size_of::<Vertex3>() + 1));
        }
    }

    #[test]
    fn icons_match_sprites_and_carry_style() {
        for head in 0..6u16 {
            let ([w, h], rgba) = icon_rgba(head << 5).unwrap();
            assert_eq!(rgba.len(), w * h * 4);
            assert!(rgba.chunks_exact(4).filter(|p| p[3] == 255).count() > 100);
        }
        assert_ne!(icon_rgba(1 << 2).unwrap().1, icon_rgba(3 << 2).unwrap().1);
    }
}
