//! Authored voxel swords: one painted grid per tier, extruded with a bevelled blade for the held mesh and flattened for icons.
use super::{Model, Vertex3};
use glam::{Mat4, Vec3};
use std::sync::LazyLock;

mod assembled;
pub(crate) use assembled::{Parts, assembled_parts, draw_assembled};

const W: usize = 24;
const H: usize = 64;
const PIX: f32 = 0.009;
/// Row the hand closes on; the mesh origin sits here.
const GRIP_ROW: f32 = 53.5;
const CENTRE: f32 = W as f32 * 0.5;
const SURFACE_KIND: f32 = 55.0;
/// Below the shader's kit-metal band (0.3..0.5): metallic 0.78 drops diffuse to a fifth and
/// the painted steel reads black, so the palette carries the shine instead.
const METAL: f32 = 0.26;
const DULL: f32 = 0.2;

#[derive(Clone, Copy)]
struct Cell { rgb: [u8; 3], finish: f32, depth: f32 }

struct Palette { edge: [u8; 3], light: [u8; 3], mid: [u8; 3], dark: [u8; 3], core: [u8; 3], guard: [u8; 3], guard_dark: [u8; 3], wrap: [u8; 3], wrap_dark: [u8; 3] }

/// `square` keeps the full width to the top row: a chopping head instead of a point.
struct Design { tip: usize, half: usize, guard_half: usize, horns: bool, square: bool, palette: Palette }

fn design(tier: u8) -> Design {
    match tier {
        1 => Design { tip: 9, half: 3, guard_half: 5, horns: false, square: false, palette: Palette {
            edge: [250, 214, 170], light: [222, 142, 82], mid: [190, 108, 62], dark: [132, 70, 44], core: [112, 58, 38],
            guard: [206, 128, 72], guard_dark: [140, 80, 46], wrap: [92, 60, 42], wrap_dark: [62, 40, 30] } },
        3 => Design { tip: 2, half: 4, guard_half: 8, horns: true, square: false, palette: Palette {
            edge: [214, 196, 182], light: [96, 84, 84], mid: [62, 54, 58], dark: [36, 30, 36], core: [255, 150, 48],
            guard: [74, 64, 66], guard_dark: [40, 34, 38], wrap: [120, 52, 36], wrap_dark: [74, 32, 26] } },
        4 => Design { tip: 2, half: 3, guard_half: 7, horns: true, square: false, palette: Palette {
            edge: [240, 252, 255], light: [170, 226, 246], mid: [112, 182, 224], dark: [66, 116, 170], core: [220, 246, 255],
            guard: [150, 208, 236], guard_dark: [78, 128, 178], wrap: [58, 78, 108], wrap_dark: [36, 52, 78] } },
        5 => Design { tip: 0, half: 0, guard_half: 0, horns: false, square: false, palette: Palette {
            edge: [196, 238, 222], light: [138, 190, 176], mid: [96, 144, 134], dark: [58, 94, 90], core: [168, 255, 226],
            guard: [120, 170, 158], guard_dark: [64, 100, 94], wrap: [84, 64, 48], wrap_dark: [56, 42, 32] } },
        _ => Design { tip: 2, half: 3, guard_half: 6, horns: false, square: false, palette: Palette {
            edge: [246, 248, 252], light: [200, 208, 220], mid: [158, 168, 184], dark: [104, 114, 132], core: [122, 132, 148],
            guard: [186, 150, 82], guard_dark: [120, 92, 48], wrap: [96, 62, 44], wrap_dark: [64, 42, 32] } },
    }
}

fn wear(x: usize, y: usize) -> f32 {
    let h = (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663);
    if h % 11 == 0 { 0.92 } else if h % 7 == 0 { 1.05 } else { 1.0 }
}

fn shade(rgb: [u8; 3], k: f32) -> [u8; 3] { rgb.map(|c| (c as f32 * k).round().clamp(0.0, 255.0) as u8) }

fn paint(tier: u8) -> Vec<Option<Cell>> { paint_design(&design(tier), tier == 5) }

fn paint_design(d: &Design, maul: bool) -> Vec<Option<Cell>> {
    let p = &d.palette;
    let mut grid = vec![None; W * H];
    let mut put = |x: i32, y: usize, rgb: [u8; 3], finish: f32, depth: f32| {
        if x >= 0 && (x as usize) < W { grid[y * W + x as usize] = Some(Cell { rgb: shade(rgb, wear(x as usize, y)), finish, depth }); }
    };
    let mid = W as i32 / 2;
    if maul { paint_maul(&mut put, p); return grid; }
    let (guard_row, grip_top, grip_bottom) = (46usize, 49usize, 57usize);
    let hw = d.half as i32;
    for y in d.tip..guard_row {
        let taper = if d.square { hw } else { ((y - d.tip) as i32 + 1).min(hw) };
        let tip_row = !d.square && y - d.tip < d.half;
        for dx in -taper..taper {
            let x = mid + dx;
            let left = dx < 0;
            let outer = dx == -taper || dx == taper - 1;
            let fuller = (dx == 0 || dx == -1) && y > d.tip + 4 && !tip_row;
            let rgb = if outer { if left { p.edge } else { p.dark } }
                else if fuller { p.core }
                else if dx == 0 || dx == -1 { p.mid }
                else if left { p.light } else { p.mid };
            // Thickness steps up from the edge to the spine; the fuller is a shallow groove in it.
            let ring = (dx + taper).min(taper - 1 - dx);
            let depth = if fuller && taper > 2 { 0.0042 } else { [0.0016, 0.0032, 0.0052][ring.min(2) as usize] };
            put(x, y, rgb, METAL, depth);
        }
    }
    for y in guard_row..guard_row + 3 {
        let gh = d.guard_half as i32;
        for dx in -gh..gh {
            let x = mid + dx;
            let lift = if d.horns { ((if dx < 0 { -dx - 1 } else { dx }) - gh / 2 + 1).max(0) } else { 0 };
            let row = y as i32 - lift;
            if row < 0 { continue; }
            let rgb = if y == guard_row { p.guard } else if y == guard_row + 2 { p.guard_dark } else if dx < 0 { p.guard } else { p.guard_dark };
            let end = dx == -gh || dx == gh - 1;
            put(x, row as usize, rgb, METAL, if end { 0.011 } else { 0.015 });
        }
    }
    for y in grip_top..=grip_bottom {
        let band = (y - grip_top) / 2 % 2 == 0;
        for dx in -2..2 {
            // Outer columns sit shallower so the wrapped grip reads round, not as a slab.
            put(mid + dx, y, if band == (dx < 0) { p.wrap } else { p.wrap_dark }, DULL, if dx == -2 || dx == 1 { 0.0075 } else { 0.0115 });
        }
    }
    for (i, y) in (grip_bottom + 1..grip_bottom + 4).enumerate() {
        let ph = if i == 1 { 3 } else { 2 };
        for dx in -ph..ph { put(mid + dx, y, if dx < 0 { p.guard } else { p.guard_dark }, METAL, if dx == -ph || dx == ph - 1 { 0.009 } else { 0.0135 }); }
    }
    grid
}

/// Hammer head across the top of a long haft; the only non-blade weapon.
fn paint_maul(put: &mut impl FnMut(i32, usize, [u8; 3], f32, f32), p: &Palette) {
    let mid = W as i32 / 2;
    for y in 8..26 {
        for dx in -10..10 {
            let outer = y == 8 || y == 25 || dx == -10 || dx == 9;
            let rgb = if outer { if dx < 0 || y == 8 { p.edge } else { p.dark } } else if y < 14 { p.light } else if dx < 0 { p.mid } else { p.dark };
            put(mid + dx, y, rgb, METAL, 0.022);
        }
    }
    for dx in -3..3 { for y in [16usize, 17] { put(mid + dx, y, p.core, METAL, 0.026); } }
    for y in 26..58 {
        for dx in -2..2 {
            let band = y >= 44 && (y - 44) / 2 % 2 == 0;
            let rgb = if y >= 44 { if band == (dx < 0) { p.wrap } else { p.wrap_dark } } else if dx < 0 { p.guard } else { p.guard_dark };
            put(mid + dx, y, rgb, if y >= 44 { DULL } else { METAL }, 0.012);
        }
    }
    for y in 58..61 { for dx in -3..3 { put(mid + dx, y, if dx < 0 { p.guard } else { p.guard_dark }, METAL, 0.016); } }
}

fn slot(tier: u8) -> usize { if (1..=5).contains(&tier) { tier as usize - 1 } else { 1 } }

static GRIDS: LazyLock<Vec<Vec<Option<Cell>>>> = LazyLock::new(|| (1..=5).map(paint).collect());
static MESHES: LazyLock<Vec<Vec<Vertex3>>> = LazyLock::new(|| GRIDS.iter().map(|g| extrude(g)).collect());

fn extrude(grid: &[Option<Cell>]) -> Vec<Vertex3> {
    let at = |x: i32, y: i32| if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H { grid[y as usize * W + x as usize] } else { None };
    let point = |x: f32, y: f32, z: f32| Vec3::new((x - CENTRE) * PIX, (GRIP_ROW - y) * PIX, z);
    let mut out = Vec::new();
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let Some(cell) = at(x, y) else { continue };
            // The world vertex shader decodes sRGB; do not decode it twice.
            let color = cell.rgb.map(|c| c as f32 / 255.0);
            let z = cell.depth;
            let (l, r, t, b) = (x as f32, x as f32 + 1.0, y as f32, y as f32 + 1.0);
            let mut quad = |p: [Vec3; 4], n: Vec3| {
                let order = if (p[1] - p[0]).cross(p[2] - p[0]).dot(n) > 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
                for i in order {
                    out.push(Vertex3 { pos: p[i].to_array(), normal: n.to_array(), color, surface: [0.48, 0.42, SURFACE_KIND, cell.finish], ao_strength: 1.0 });
                }
            };
            for s in [-1.0, 1.0] {
                quad([point(l, b, s * z), point(r, b, s * z), point(r, t, s * z), point(l, t, s * z)], Vec3::Z * s);
            }
            for (dx, dy, n, a, c) in [(-1, 0, Vec3::NEG_X, (l, t), (l, b)), (1, 0, Vec3::X, (r, t), (r, b)), (0, -1, Vec3::Y, (l, t), (r, t)), (0, 1, Vec3::NEG_Y, (l, b), (r, b))] {
                // A thinner neighbour exposes a step on both faces; the mesh stays closed.
                let nz = at(x + dx, y + dy).map_or(0.0, |c| c.depth);
                if nz <= 0.0 {
                    quad([point(a.0, a.1, -z), point(c.0, c.1, -z), point(c.0, c.1, z), point(a.0, a.1, z)], n);
                } else if nz < z {
                    for (lo, hi) in [(nz, z), (-z, -nz)] {
                        quad([point(a.0, a.1, lo), point(c.0, c.1, lo), point(c.0, c.1, hi), point(a.0, a.1, hi)], n);
                    }
                }
            }
        }
    }
    out
}

/// Appends the tier's sword with its grip at the frame origin.
pub(crate) fn draw(m: &mut Model, frame: Mat4, tier: u8) { append(m, frame, &MESHES[slot(tier)]); }

fn append(m: &mut Model, frame: Mat4, mesh: &[Vertex3]) {
    let transform = Mat4::from_scale(Vec3::splat(1.0 / m.scale)) * frame;
    let normal = frame.inverse().transpose();
    m.vertices.extend(mesh.iter().map(|v| {
        let mut out = *v;
        out.pos = transform.transform_point3(Vec3::from_array(v.pos)).to_array();
        out.normal = normal.transform_vector3(Vec3::from_array(v.normal)).normalize_or_zero().to_array();
        out
    }));
}

/// RGBA texels of the same painted grid, trimmed to its occupied rows.
pub(crate) fn icon(tier: u8) -> ([usize; 2], Vec<u8>) {
    let grid = &GRIDS[slot(tier)];
    let rows: Vec<usize> = (0..H).filter(|y| (0..W).any(|x| grid[y * W + x].is_some())).collect();
    let (top, bottom) = (rows[0], rows[rows.len() - 1]);
    let height = bottom - top + 1;
    let mut rgba = vec![0u8; W * height * 4];
    for y in top..=bottom {
        for x in 0..W {
            if let Some(c) = grid[y * W + x] {
                let i = ((y - top) * W + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[c.rgb[0], c.rgb[1], c.rgb[2], 255]);
            }
        }
    }
    ([W, height], rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tier_paints_and_extrudes() {
        for tier in 0..=6u8 {
            let (size, rgba) = icon(tier);
            assert!(rgba.chunks_exact(4).any(|p| p[3] == 255), "tier {tier} empty");
            assert_eq!(rgba.len(), size[0] * size[1] * 4);
            assert!(!MESHES[slot(tier)].is_empty());
        }
    }

    #[test]
    fn every_tier_mesh_is_closed() {
        for tier in 1..=5u8 {
            let sum = MESHES[slot(tier)].chunks_exact(3).fold(Vec3::ZERO, |acc, t| {
                let [a, b, c] = [t[0].pos, t[1].pos, t[2].pos].map(Vec3::from_array);
                acc + (b - a).cross(c - a)
            });
            assert!(sum.length() < 1e-7, "tier {tier}: {sum}");
        }
    }

    #[test]
    fn blade_is_thinner_at_the_edge_than_the_spine() {
        let g = &GRIDS[slot(2)];
        let row: Vec<f32> = (0..W).filter_map(|x| g[30 * W + x].map(|c| c.depth)).collect();
        assert!(row[0] < row[row.len() / 2 - 2], "{row:?}");
    }

    #[test]
    fn tiers_look_different() {
        let sheets: Vec<_> = (1..=5u8).map(|t| icon(t).1).collect();
        for a in 0..5 { for b in a + 1..5 { assert_ne!(sheets[a], sheets[b], "{a} vs {b}"); } }
    }

    #[test]
    fn grid_is_left_right_balanced_about_the_grip_axis() {
        for tier in 1..=5u8 {
            let g = &GRIDS[slot(tier)];
            for y in 0..H {
                let left = (0..W / 2).filter(|&x| g[y * W + x].is_some()).count();
                let right = (W / 2..W).filter(|&x| g[y * W + x].is_some()).count();
                assert_eq!(left, right, "tier {tier} row {y}");
            }
        }
    }
}
