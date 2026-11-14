//! Lettering cut into the gate lintel. The opening itself is drawn by the
//! portal compositor (`world::portals::realm`), never by stand-in geometry.
use crate::{mesh::models::Model, world::World};
use engine_worldgen::hell_gate as gate;
use glam::{Quat, Vec3};

/// Largest groove width; a one-metre lintel cell gets narrower ones.
const MAX_PIXEL: f32 = 0.045;
const ROWS: usize = 9;
/// Two lines, the gap between them and both ruled lines, in grooves.
const BLOCK_PIXELS: f32 = 2.0 * ROWS as f32 + 6.0;
/// Basalt albedo from `assets/materials.ron`; cuts are shades of the stone.
const STONE: [f32; 3] = [0.2, 0.21, 0.24];
/// Shades of the stone for the groove floor, the lit lower bevel and the
/// shadowed upper bevel. Models take sky ambient that buried terrain does not,
/// so any lift on the groove floor reads as paint rather than depth.
const GROOVE: f32 = 0.0;
const LIT: f32 = 0.45;
const SHADOW: f32 = 0.0;
const LINES: [&str; 2] = ["ABANDON ALL HOPE", "YE WHO ENTER HERE"];

pub fn draw(mesh: &mut Model, world: &World, viewer: Vec3) {
    let cfg = world.sample_config();
    if !gate::enabled(&cfg) { return; }
    let center = gate::center(&cfg, viewer);
    if (viewer.x - center.x).abs() > 48.0 || (viewer.z - center.z).abs() > 48.0 { return; }
    let base = gate::floor(&cfg, center);
    if (viewer.y - base).abs() > 20.0 || !super::intact(world, center, base) { return; }
    let c = center + Vec3::Y * base;
    // Only the face toward the viewer; the lintel hides the other one.
    let side = if viewer.z >= c.z { 1.0 } else { -1.0 };
    let vm = cfg.voxel_meters();
    let face = (0.75 / vm - 0.5).ceil() * vm + 0.004;
    // The lintel is the cells whose centres lie 5.0 to 6.5 m above the floor
    // (`engine_worldgen::hell_gate::block`), one metre tall at metre scale.
    let lintel = [5.0f32, 6.5].map(|y| (y / vm - 0.5).ceil() * vm);
    let pixel = MAX_PIXEL.min((lintel[1] - lintel[0] - 0.08) / BLOCK_PIXELS);
    let shade = |k: f32| STONE.map(|s| s * k);
    let mut cut = |x0: f32, x1: f32, y: f32, h: f32, k: f32| {
        // x runs along the reading direction, so mirror it on the -z face.
        // The lit bevel sits 2 mm proud of the groove floor so they never z-fight.
        let x = side * (x0 + x1) * 0.5;
        let lift = if k == LIT { 0.002 } else { 0.0 };
        mesh.cuboid(c + Vec3::new(x, y, side * (face + lift)), Vec3::new(x1 - x0, h, 0.006),
            Quat::IDENTITY, shade(k), 0.0);
    };
    let line_height = ROWS as f32 * pixel;
    let top = (lintel[0] + lintel[1]) * 0.5 + (BLOCK_PIXELS * 0.5 - 2.0) * pixel;
    for (line, words) in LINES.into_iter().enumerate() {
        let width = text_width(words) as f32 * pixel;
        let y0 = top - line as f32 * (line_height + 2.0 * pixel);
        let mut col = 0usize;
        for ch in words.bytes() {
            let Some(rows) = glyph(ch) else {
                // Interpunct between words, as on cut Roman capitals.
                let x = -width * 0.5 + (col as f32 + 1.5) * pixel;
                cut(x, x + pixel, y0 - 4.5 * pixel, pixel, GROOVE);
                col += 4;
                continue;
            };
            for (r, bits) in rows.iter().enumerate() {
                let above = r.checked_sub(1).map(|i| rows[i].as_bytes());
                let below = rows.get(r + 1).map(|s| s.as_bytes());
                let y = y0 - (r as f32 + 0.5) * pixel;
                // Runs of filled pixels become one groove; the lower bevel of a
                // V-cut faces up and catches light, the upper one is in shadow.
                for (a, b) in runs(bits.as_bytes(), |_| true) {
                    let x0 = -width * 0.5 + (col + a) as f32 * pixel;
                    cut(x0, x0 + (b - a) as f32 * pixel, y, pixel, GROOVE);
                }
                for (a, b) in runs(bits.as_bytes(), |i| below.is_none_or(|s| s[i] != b'#')) {
                    let x0 = -width * 0.5 + (col + a) as f32 * pixel;
                    cut(x0, x0 + (b - a) as f32 * pixel, y - pixel * 0.36, pixel * 0.28, LIT);
                }
                for (a, b) in runs(bits.as_bytes(), |i| above.is_none_or(|s| s[i] != b'#')) {
                    let x0 = -width * 0.5 + (col + a) as f32 * pixel;
                    cut(x0, x0 + (b - a) as f32 * pixel, y + pixel * 0.36, pixel * 0.28, SHADOW);
                }
            }
            col += 8;
        }
    }
    // Ruled lines above and below the text block.
    let half = text_width(LINES[1]) as f32 * pixel * 0.5 + 3.0 * pixel;
    let bottom = top - 2.0 * line_height - 2.0 * pixel;
    for y in [top + 2.0 * pixel, bottom - 2.0 * pixel] {
        cut(-half, half, y, pixel * 0.7, GROOVE);
        cut(-half, half, y - pixel * 0.45, pixel * 0.2, LIT);
    }
}

fn text_width(words: &str) -> usize {
    words.bytes().map(|b| if glyph(b).is_some() { 8 } else { 4 }).sum::<usize>() - 1
}

/// Half-open column ranges of filled pixels that also pass `keep`.
fn runs(bits: &[u8], keep: impl Fn(usize) -> bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for i in 0..=bits.len() {
        let on = i < bits.len() && bits[i] == b'#' && keep(i);
        match (on, start) {
            (true, None) => start = Some(i),
            (false, Some(a)) => { out.push((a, i)); start = None; }
            _ => {}
        }
    }
    out
}

/// Seven-by-nine serif capitals for the letters the inscription uses.
fn glyph(ch: u8) -> Option<[&'static str; ROWS]> {
    Some(match ch {
        b'A' => ["...#...", "...#...", "..#.#..", "..#.#..", ".#...#.", ".#####.", ".#...#.", "#.....#", "##...##"],
        b'B' => ["######.", ".#....#", ".#....#", ".#....#", ".#####.", ".#....#", ".#....#", ".#....#", "######."],
        b'D' => ["#####..", ".#...#.", ".#....#", ".#....#", ".#....#", ".#....#", ".#....#", ".#...#.", "#####.."],
        b'E' => ["#######", ".#....#", ".#.....", ".#..#..", ".####..", ".#..#..", ".#.....", ".#....#", "#######"],
        b'H' => ["###.###", ".#...#.", ".#...#.", ".#...#.", ".#####.", ".#...#.", ".#...#.", ".#...#.", "###.###"],
        b'L' => ["###....", ".#.....", ".#.....", ".#.....", ".#.....", ".#.....", ".#.....", ".#....#", "#######"],
        b'N' => ["##..###", ".#...#.", ".##..#.", ".#.#.#.", ".#.#.#.", ".#..##.", ".#..##.", ".#...#.", "###..#."],
        b'O' => ["..###..", ".#...#.", "#.....#", "#.....#", "#.....#", "#.....#", "#.....#", ".#...#.", "..###.."],
        b'P' => ["######.", ".#....#", ".#....#", ".#....#", ".#####.", ".#.....", ".#.....", ".#.....", "###...."],
        b'R' => ["######.", ".#....#", ".#....#", ".#....#", ".#####.", ".#..#..", ".#...#.", ".#...#.", "###..##"],
        b'T' => ["#######", "#..#..#", "...#...", "...#...", "...#...", "...#...", "...#...", "...#...", "..###.."],
        b'W' => ["###.###", ".#...#.", ".#...#.", ".#.#.#.", ".#.#.#.", ".#.#.#.", ".##.##.", ".##.##.", ".#...#."],
        b'Y' => ["###.###", ".#...#.", "..#.#..", "..#.#..", "...#...", "...#...", "...#...", "...#...", "..###.."],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inscription_fits_the_lintel_and_every_glyph_is_seven_wide() {
        for words in LINES {
            assert!(text_width(words) as f32 * MAX_PIXEL < 6.6, "{words} overruns the 7 m lintel");
            for ch in words.bytes().filter(|b| *b != b' ') {
                let rows = glyph(ch).unwrap_or_else(|| panic!("no glyph for {}", ch as char));
                assert!(rows.iter().all(|r| r.len() == 7), "{} is not 7 wide", ch as char);
            }
        }
    }
}
