//! Canonical six-role palette for native-pixel UI art.
//!
//! Every sprite uses the same ramp: dark outline, shade, body, highlight,
//! gold, ember. Sprites are authored on a native pixel grid and scaled by an
//! integer number of physical pixels, so no antialiasing or subpixel seams are
//! introduced. `assets/pixel_palette.txt` is the shared source mirrored by the
//! Python generators; the test below keeps the two copies in sync.
use egui::Color32;

pub const OUTLINE: Color32 = Color32::from_rgb(29, 31, 34);
pub const PAPER: Color32 = Color32::from_rgb(235, 225, 197);
pub const GOLD: Color32 = Color32::from_rgb(218, 173, 96);
pub const EMBER: Color32 = Color32::from_rgb(193, 102, 49);
pub const TEAL: Color32 = Color32::from_rgb(101, 183, 175);
pub const HIGHLIGHT: Color32 = Color32::from_rgb(247, 228, 159);

pub const SHADE: f32 = 0.62;
pub const BOTTOM_SHADE: f32 = 0.76;
pub const TOP_LIGHT_MIX: f32 = 0.20;

fn scale(color: Color32, factor: f32) -> Color32 {
    Color32::from_rgb(
        (color.r() as f32 * factor) as u8,
        (color.g() as f32 * factor) as u8,
        (color.b() as f32 * factor) as u8,
    )
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 * (1.0 - t) + b.r() as f32 * t) as u8,
        (a.g() as f32 * (1.0 - t) + b.g() as f32 * t) as u8,
        (a.b() as f32 * (1.0 - t) + b.b() as f32 * t) as u8,
    )
}

/// Internal downward-facing shade for an accent-coloured body.
pub fn shade(accent: Color32) -> Color32 {
    scale(accent, SHADE)
}

/// The one-native-pixel light strip along exposed top edges.
pub fn top_light(accent: Color32) -> Color32 {
    mix(accent, PAPER, TOP_LIGHT_MIX)
}

/// The one-native-pixel shadow strip along exposed bottom edges.
pub fn bottom_shadow(accent: Color32) -> Color32 {
    scale(accent, BOTTOM_SHADE)
}

/// Decode the sprite glyphs used by every icon painter and generator.
/// `1` shade, `2` body, `3` outline, `4` highlight, `5` gold, `6` ember.
pub fn glyph(ch: u8, accent: Color32) -> Option<Color32> {
    Some(match ch {
        b'1' => shade(accent),
        b'2' => accent,
        b'3' => OUTLINE,
        b'4' => HIGHLIGHT,
        b'5' => GOLD,
        b'6' => EMBER,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Selection glow
// ---------------------------------------------------------------------------

/// Hue in turns, saturation and value in `[0, 1]`. Hue wraps.
fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    let h = h.rem_euclid(1.0) * 6.0;
    let sector = h.floor();
    let f = h - sector;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match sector as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let byte = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgb(byte(r), byte(g), byte(b))
}

/// Ordered 4x4 threshold, the same matrix the interaction overlay dithers with.
fn dither4(x: i32, y: i32) -> f32 {
    const CELLS: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    CELLS[((y & 3) * 4 + (x & 3)) as usize] as f32 / 16.0
}

/// Cell on the rectangle's perimeter, walking clockwise from the top-left.
fn perimeter_cell(index: i32, nx: i32, ny: i32) -> (i32, i32) {
    let (w, h) = (nx.max(1), ny.max(1));
    if index < w {
        return (index, 0);
    }
    let index = index - w;
    if index < h {
        return (w - 1, index);
    }
    let index = index - h;
    if index < w {
        return (w - 1 - index, h - 1);
    }
    (0, (h - 1 - (index - w)).max(0))
}

/// Rainbow selection frame for a chosen slot.
///
/// Cells sit on the two-pixel art lattice and roughly a third of them drop out
/// to the dither threshold, so the frame reads as pixel art instead of a stroke.
/// Hue travels twice around the perimeter and drifts with `seconds`, coverage
/// thins toward the bottom, and a short inner halo gathers under the top edge.
/// The vertical density gradient is the same one the world interaction overlay
/// uses, which is what ties the two effects together.
pub fn rainbow_selection(painter: &egui::Painter, rect: egui::Rect, seconds: f64) {
    const CELL: f32 = 2.0;
    let nx = ((rect.width() / CELL).round() as i32).max(1);
    let ny = ((rect.height() / CELL).round() as i32).max(1);
    let perimeter = 2 * (nx + ny);
    let drift = (seconds * 0.18) as f32;

    for index in 0..perimeter {
        let (cx, cy) = perimeter_cell(index, nx, ny);
        let up = 1.0 - cy as f32 / ny as f32;
        if dither4(cx, cy) > 0.35 + up * 0.55 {
            continue;
        }
        let hue = index as f32 / perimeter as f32 * 2.0 + drift;
        let cell = egui::Rect::from_min_size(
            rect.min + egui::vec2(cx as f32 * CELL, cy as f32 * CELL),
            egui::vec2(CELL, CELL),
        );
        painter.rect_filled(cell, 0, hsv(hue, 0.72, 0.72 + up * 0.28));
    }

    // Inner halo: three rows under the top edge, fading downward.
    for row in 0..3 {
        let up = 1.0 - row as f32 / 3.0;
        let alpha = (34.0 * up) as u8;
        for column in 0..nx {
            let hue = column as f32 / nx as f32 * 2.0 + drift;
            let shade = hsv(hue, 0.55, 0.88);
            let cell = egui::Rect::from_min_size(
                rect.min + egui::vec2(column as f32 * CELL, (row + 1) as f32 * CELL),
                egui::vec2(CELL, CELL),
            );
            painter.rect_filled(
                cell,
                0,
                Color32::from_rgba_unmultiplied(shade.r(), shade.g(), shade.b(), alpha),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(key: &str) -> Option<&'static str> {
        include_str!("../../../../assets/pixel_palette.txt")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.trim())
    }

    fn hex(color: Color32) -> String {
        format!("{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
    }

    fn canonical_f32(key: &str) -> Option<f32> {
        canonical(key).and_then(|value| value.parse().ok())
    }

    #[test]
    fn shared_palette_file_matches_the_sprite_ramp() {
        assert_eq!(canonical("outline"), Some(hex(OUTLINE).as_str()));
        assert_eq!(canonical("paper"), Some(hex(PAPER).as_str()));
        assert_eq!(canonical("gold"), Some(hex(GOLD).as_str()));
        assert_eq!(canonical("ember"), Some(hex(EMBER).as_str()));
        assert_eq!(canonical("teal"), Some(hex(TEAL).as_str()));
        assert_eq!(canonical("highlight"), Some(hex(HIGHLIGHT).as_str()));
        assert_eq!(canonical_f32("shade"), Some(SHADE));
        assert_eq!(canonical_f32("bottom_shade"), Some(BOTTOM_SHADE));
        assert_eq!(canonical_f32("top_light_mix"), Some(TOP_LIGHT_MIX));
    }

    #[test]
    fn every_sprite_glyph_decodes_and_shading_is_ordered() {
        for ch in b"123456" {
            assert!(glyph(*ch, TEAL).is_some(), "glyph {ch} must decode");
        }
        assert!(glyph(b'.', TEAL).is_none());
        assert!(shade(PAPER).r() < PAPER.r());
        assert!(bottom_shadow(PAPER).r() > shade(PAPER).r());
        assert!(top_light(TEAL).r() > TEAL.r());
    }
}
