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
        b'A' => top_light(accent),
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
    if index < h - 1 {
        return (w - 1, index + 1);
    }
    let index = index - (h - 1);
    if index < w - 1 {
        return (w - 2 - index, h - 1);
    }
    (0, (h - 2 - (index - (w - 1))).max(1))
}

/// Rainbow selection frame for a chosen slot.
///
/// Cells sit on the two-pixel art lattice and roughly a third of them drop out
/// to the dither threshold, so the frame reads as pixel art instead of a stroke.
/// Hue travels twice around the perimeter and drifts with `seconds`, coverage
/// thins toward the bottom, with shaded pixels and one falling spark per side.
/// The vertical density gradient is the same one the world interaction overlay
/// uses, which is what ties the two effects together.
pub fn rainbow_selection(painter: &egui::Painter, rect: egui::Rect, seconds: f64) {
    if !painter.is_visible() || !rect.intersects(painter.clip_rect()) { return; }
    let still = super::magic_fx::reduced(painter.ctx());
    let seconds = if still { 2.0 } else { (seconds * 12.0).floor() / 12.0 };
    // Whole physical pixels at fractional desktop/UI scales as well as 1x.
    let pixels_per_point = painter.ctx().pixels_per_point().max(0.1);
    let cell_size = (2.0 * pixels_per_point).round().max(1.0) / pixels_per_point;
    let rect = egui::Rect::from_min_max(
        egui::pos2((rect.left()*pixels_per_point).round()/pixels_per_point, (rect.top()*pixels_per_point).round()/pixels_per_point),
        rect.max,
    );
    let nx = ((rect.width() / cell_size).floor() as i32).max(2);
    let ny = ((rect.height() / cell_size).floor() as i32).max(2);
    let perimeter = 2 * (nx + ny) - 4;
    let drift = (seconds * 0.18) as f32;

    for index in 0..perimeter {
        let (cx, cy) = perimeter_cell(index, nx, ny);
        let up = 1.0 - cy as f32 / ny as f32;
        if dither4(cx, cy) > 0.35 + up * 0.55 {
            continue;
        }
        let hue = ((index as f32 / perimeter as f32 * 2.0 + drift) * 12.0).floor() / 12.0;
        let cell = egui::Rect::from_min_size(
            rect.min + egui::vec2(cx as f32 * cell_size, cy as f32 * cell_size),
            egui::vec2(cell_size, cell_size),
        );
        painter.rect_filled(cell, 0, hsv(hue, 0.72, 0.72 + up * 0.28));
    }

    // Dithered shade pixels replace translucent rainbow fog.
    for row in 0..2 {
        let up = 1.0 - row as f32 / 3.0;
        let alpha = (24.0 * up) as u8;
        for column in 0..nx {
            if dither4(column, row) > 0.3 * up { continue; }
            let hue = column as f32 / nx as f32 * 2.0 + drift;
            let shade = hsv(hue, 0.55, 0.88);
            let cell = egui::Rect::from_min_size(
                rect.min + egui::vec2(column as f32 * cell_size, (row + 1) as f32 * cell_size),
                egui::vec2(cell_size, cell_size),
            );
            painter.rect_filled(
                cell,
                0,
                Color32::from_rgba_unmultiplied(shade.r(), shade.g(), shade.b(), alpha),
            );
        }
    }
    // Keep the icon and stack count clear: sparks fall in the narrow side gutters.
    // One per side; more read as noise next to the halo and the frame.
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)));
    for i in 0..2 {
        let seed = ((i * 37 + 11) % 67) as f32 / 67.0;
        let age = (seconds as f32 * (0.22 + seed * 0.10) + i as f32 * 0.5).fract();
        let y = rect.top() + 3.0 + age * age * (rect.height() - 14.0).max(0.0);
        let inset = 2.0 + ((seed * 2.0 + age).floor() % 2.0) * cell_size;
        let x = if i % 2 == 0 { rect.left() + inset } else { rect.right() - inset - cell_size };
        let hue = ((drift + seed) * 12.0).floor() / 12.0;
        let body = hsv(hue, 0.55, 0.85);
        for tail in 0..2 {
            let alpha = ((1.0 - age) * (120.0 - tail as f32 * 60.0)) as u8;
            let color = if tail == 0 { mix(body, PAPER, 0.55) } else { body };
            clipped.rect_filled(egui::Rect::from_min_size(
                egui::pos2((x / cell_size).floor() * cell_size, (y / cell_size).floor() * cell_size - tail as f32 * cell_size),
                egui::vec2(cell_size, cell_size)), 0,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha));
        }
    }
    if !still { painter.ctx().request_repaint_after(std::time::Duration::from_millis(83)); }
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
