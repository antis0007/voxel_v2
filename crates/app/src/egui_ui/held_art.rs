//! Item art shared by hotbar icons and the item held in first person, so both always match.
use super::icons::Icon;
use crate::world::BlockId;
use crate::world::inventory::Item;

#[derive(Clone, Copy)]
pub(crate) enum Art {
    /// A textured cube: 7×7 texels per face.
    Block(BlockId),
    /// A native-pixel glyph sprite from `icons`.
    Icon(Icon),
    /// Drawn by the wand viewmodel and `wand_icon`.
    Wand(u16),
}

pub(crate) fn art(item: Item) -> Art {
    let block = |b: BlockId| {
        if b.is_torch() {
            Art::Icon(Icon::Torch)
        } else if engine_content::thin::canonical(b) == Some(BlockId::Ladder) {
            Art::Icon(Icon::Ladder)
        } else if engine_content::thin::canonical(b) == Some(BlockId::Vines) {
            Art::Icon(Icon::Vines)
        } else if b == BlockId::Sapling {
            Art::Icon(Icon::Sapling)
        } else if let Some(kind) = crate::world::factory::Kind::from_block(b) {
            Art::Icon(Icon::Machine(kind))
        } else {
            Art::Block(b)
        }
    };
    match item {
        Item::PowerKit(k) => block(k.kind.block()),
        Item::Block(b) => block(b),
        Item::Resource(i) => Art::Icon(Icon::Resource(i as usize)),
        Item::Food(f) => Art::Icon(Icon::Food(f as u8)),
        Item::Weapon(tier) => Art::Icon(Icon::Sword(tier as u32)),
        Item::Armor(slot, tier) => Art::Icon(Icon::Armor(slot as u8, tier as u8)),
        Item::Tool(1 | 2) => Art::Icon(Icon::Bore),
        Item::Tool(t) => Art::Icon(Icon::Wand(t)),
        Item::Wand { look, .. } => Art::Wand(look),
    }
}

pub(crate) const FACE: u32 = 7;

/// One unlit face texel of a block cube in vertex colour space; `y` 0 is the top row.
pub(crate) fn block_texel(block: BlockId, top: bool, x: u32, y: u32) -> [f32; 3] {
    let c = super::block_icon_texel(block, super::material_color32(block), top, x, y);
    [c.r(), c.g(), c.b()].map(|v| v as f32 / 255.0)
}

/// Sprite texels row-major with their glyph byte; `None` is transparent. Colours are sRGB.
pub(crate) fn icon_texels(icon: Icon) -> (usize, usize, Vec<Option<(u8, [u8; 3])>>) {
    let (rows, accent) = super::icons::rows(icon);
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut texels = Vec::with_capacity(width * rows.len());
    for row in rows {
        let bytes = row.as_bytes();
        texels.extend((0..width).map(|x| {
            let ch = *bytes.get(x)?;
            super::pixel_art::glyph(ch, accent).map(|c| (ch, [c.r(), c.g(), c.b()]))
        }));
    }
    (width, rows.len(), texels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torches_and_machines_use_their_icons_and_wands_their_own_path() {
        assert!(matches!(art(Item::Block(BlockId::Torch)), Art::Icon(Icon::Torch)));
        assert!(matches!(art(Item::Block(BlockId::Stone)), Art::Block(BlockId::Stone)));
        assert!(matches!(art(Item::Wand { id: 1, look: 7 }), Art::Wand(7)));
    }

    #[test]
    fn torch_sprite_has_a_flame_above_its_stick() {
        let (w, h, texels) = icon_texels(Icon::Torch);
        assert_eq!(texels.len(), w * h);
        let flame = texels.iter().position(|t| t.is_some_and(|(ch, _)| matches!(ch, b'4' | b'5' | b'6'))).unwrap();
        let stick = texels.iter().rposition(|t| t.is_some()).unwrap();
        assert!(flame / w < stick / w);
    }
}
