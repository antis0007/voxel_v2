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
