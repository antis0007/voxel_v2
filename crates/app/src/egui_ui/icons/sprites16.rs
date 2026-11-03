//! Multi-palette 16x16 sprites from `assets/icons/*16.txt`.
//!
//! Glyph rows tint one accent ramp, so a coil cannot be copper wire on an
//! iron spool. These sprites carry their own palettes and replace the glyph
//! rows for the icons they cover; other icons keep their glyphs.
use super::Icon;
use egui::{Color32 as C, ColorImage, Painter, Rect, TextureOptions, vec2};
use std::sync::LazyLock;

const RESOURCES: &str = include_str!("../../../../../assets/icons/resources16.txt");
const ITEMS: &str = include_str!("../../../../../assets/icons/items16.txt");
const MACHINES: &str = include_str!("../../../../../assets/icons/machines16.txt");
pub(super) const SIZE: usize = 16;
/// First id of foods in the items sheet; foods follow in `FoodKind` order.
const FOOD_BASE: usize = 10;
/// First id of armour; four tiers per slot.
const ARMOR_BASE: usize = 20;
const SPELL_FRAME: usize = 36;

type Pixels = Vec<C>;

static SHEETS: LazyLock<[Vec<Option<Pixels>>; 3]> = LazyLock::new(|| {
    [parse(RESOURCES).unwrap_or_default(), parse(ITEMS).unwrap_or_default(), parse(MACHINES).unwrap_or_default()]
});

/// Sheet and sprite id for an icon drawn from a sprite sheet.
fn sprite(icon: Icon) -> Option<(usize, usize)> {
    Some(match icon {
        Icon::Resource(id) => (0, id),
        Icon::Machine(kind) => (2, kind as usize),
        Icon::Torch => (1, 2),
        Icon::Rod => (1, 4),
        Icon::Boat => (1, 5),
        Icon::Food(kind) => (1, FOOD_BASE + kind as usize),
        Icon::Armor(slot @ 0..=3, tier @ 1..=4) => (1, ARMOR_BASE + slot as usize * 4 + tier as usize - 1),
        _ => return None,
    })
}

fn hex(s: &str) -> Option<C> {
    let v = u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6)?;
    Some(C::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Parses a sheet into pixels indexed by sprite id.
pub(super) fn parse(source: &str) -> Result<Vec<Option<Pixels>>, String> {
    let mut outline = None;
    let mut sheet: Vec<Option<Pixels>> = Vec::new();
    let mut current: Option<(usize, Vec<(u8, C)>, Pixels)> = None;
    for (n, line) in source.lines().enumerate() {
        let err = |what: &str| format!("sprite sheet line {}: {what}", n + 1);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if current.is_none() {
            match words.as_slice() {
                ["outline", h] => outline = Some(hex(h).ok_or_else(|| err("bad outline"))?),
                ["sprite", id, _name] => {
                    let id = id.parse().map_err(|_| err("bad id"))?;
                    let o = outline.ok_or_else(|| err("outline must come first"))?;
                    current = Some((id, vec![(b'o', o)], Vec::with_capacity(SIZE * SIZE)));
                }
                _ => return Err(err("expected outline or sprite")),
            }
            continue;
        }
        if words.as_slice() == ["end"] {
            let Some((id, _, pixels)) = current.take() else { continue };
            if pixels.len() != SIZE * SIZE {
                return Err(err("sprite needs 16 rows"));
            }
            if sheet.len() <= id {
                sheet.resize(id + 1, None);
            }
            sheet[id] = Some(pixels);
            continue;
        }
        match (words.as_slice(), current.as_mut()) {
            ([key, h], Some((_, palette, _))) if key.len() == 1 && line.len() < SIZE => {
                palette.push((key.as_bytes()[0], hex(h).ok_or_else(|| err("bad colour"))?));
            }
            (_, Some((_, palette, pixels))) if line.len() == SIZE => {
                for ch in line.bytes() {
                    let color = match ch {
                        b'.' => C::TRANSPARENT,
                        _ => palette
                            .iter()
                            .find(|(k, _)| *k == ch)
                            .map(|(_, c)| *c)
                            .ok_or_else(|| err("undefined key"))?,
                    };
                    pixels.push(color);
                }
            }
            _ => return Err(err("unexpected line")),
        }
    }
    Ok(sheet)
}

/// Sheet texels for the held-item mesh, in `held_art::icon_texels` form.
pub(crate) fn texels(icon: Icon) -> Option<(usize, usize, Vec<Option<(u8, [u8; 3])>>)> {
    let (sheet, id) = sprite(icon)?;
    let pixels = SHEETS[sheet].get(id)?.as_ref()?;
    let texels = pixels.iter().map(|c| (c.a() == 255).then(|| (b'#', [c.r(), c.g(), c.b()]))).collect();
    Some((SIZE, SIZE, texels))
}

/// Draws `icon` at a whole-pixel scale; false if no sheet covers it.
pub(super) fn paint(p: &Painter, r: Rect, icon: Icon) -> bool {
    sprite(icon).is_some_and(|(sheet, id)| paint_sprite(p, r, sheet, id, None))
}

/// Draws the shared spell backplate; false if the sheet lacks it.
pub(super) fn paint_spell_frame(p: &Painter, r: Rect) -> bool {
    paint_sprite(p, r, 1, SPELL_FRAME, None)
}

/// Draws `icon` as one flat `tint` shape; false if no sheet covers it.
pub(super) fn paint_silhouette(p: &Painter, r: Rect, icon: Icon, tint: C) -> bool {
    sprite(icon).is_some_and(|(sheet, id)| paint_sprite(p, r, sheet, id, Some(tint)))
}

fn paint_sprite(p: &Painter, r: Rect, sheet: usize, id: usize, mask: Option<C>) -> bool {
    let Some(Some(pixels)) = SHEETS[sheet].get(id) else {
        return false;
    };
    let key = egui::Id::new(("sprite-16", sheet, id, mask.is_some()));
    let texture = p.ctx().data(|d| d.get_temp::<egui::TextureHandle>(key)).unwrap_or_else(|| {
        // A mask texture is white where opaque so the draw tint sets its colour.
        let pixels = match mask {
            Some(_) => pixels.iter().map(|c| if c.a() == 255 { C::WHITE } else { C::TRANSPARENT }).collect(),
            None => pixels.clone(),
        };
        let image = ColorImage::new([SIZE, SIZE], pixels);
        let texture = p.ctx().load_texture("sprite-16", image, TextureOptions::NEAREST);
        p.ctx().data_mut(|d| d.insert_temp(key, texture.clone()));
        texture
    });
    let ppp = p.ctx().pixels_per_point();
    // Whole physical pixels per texel, so no texel is drawn wider than its neighbour.
    let texel = ((r.width().min(r.height()) * ppp / SIZE as f32).floor().max(1.0)) / ppp;
    let side = texel * SIZE as f32;
    let min = ((r.center() - vec2(side, side) * 0.5) * ppp).round() / ppp;
    let target = Rect::from_min_size(min, vec2(side, side));
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    p.image(texture.id(), target, uv, mask.unwrap_or(C::WHITE));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_drawn(label: &str, icon: Icon) {
        let (sheet, id) = sprite(icon).unwrap_or_else(|| panic!("{label} has no sheet"));
        let pixels = SHEETS[sheet].get(id).and_then(Option::as_ref);
        let pixels = pixels.unwrap_or_else(|| panic!("{label}: sheet {sheet} has no sprite {id}"));
        let opaque = pixels.iter().filter(|c| c.a() == 255).count();
        assert!(opaque >= 40, "{label} has only {opaque} pixels");
        assert!(pixels.iter().all(|c| c.a() == 0 || c.a() == 255));
    }

    #[test]
    fn sheets_parse() {
        parse(RESOURCES).unwrap();
        parse(ITEMS).unwrap();
        parse(MACHINES).unwrap();
        assert!(SHEETS[1].get(SPELL_FRAME).is_some_and(Option::is_some));
    }

    #[test]
    fn covered_icons_all_have_sprites() {
        for id in 0..crate::world::factory::RESOURCE_COUNT {
            assert_drawn(&format!("resource {id}"), Icon::Resource(id));
        }
        let tools = [("torch", Icon::Torch), ("rod", Icon::Rod), ("boat", Icon::Boat)];
        for (label, icon) in tools {
            assert_drawn(label, icon);
        }
        for kind in crate::world::factory::Kind::ALL {
            assert_drawn(&format!("{kind:?}"), Icon::Machine(kind));
        }
        for slot in 0..4 {
            for tier in 1..=4 {
                assert_drawn(&format!("armour {slot} {tier}"), Icon::Armor(slot, tier));
            }
        }
        for kind in crate::world::inventory::FoodKind::ALL {
            assert_drawn(&format!("food {}", kind as u8), Icon::Food(kind as u8));
        }
    }

    #[test]
    fn malformed_sheets_are_rejected() {
        assert!(parse("sprite 0 x\nend\n").is_err(), "outline first");
        let short = "outline 000000\nsprite 0 x\n................\nend\n";
        assert!(parse(short).is_err(), "16 rows");
        let key = format!("outline 000000\nsprite 0 x\n{}\nend\n", vec!["q".repeat(16); 16].join("\n"));
        assert!(parse(&key).is_err(), "undefined key");
    }
}
