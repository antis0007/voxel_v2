//! Item art shared by hotbar icons and the item held in first person, so both always match.
use super::icons::Icon;
use crate::world::BlockId;
use crate::world::inventory::Item;

thread_local! { static TOOLS: std::cell::RefCell<Vec<engine_tools::ToolAssembly>> = const { std::cell::RefCell::new(Vec::new()) }; }
pub(crate) fn sync_tools(bank:&[engine_tools::ToolAssembly]) {
    TOOLS.with(|cache| { let mut cache=cache.borrow_mut(); if cache.as_slice()!=bank {cache.clear();cache.extend_from_slice(bank);} });
}
pub(crate) fn tool(id:u16)->Option<engine_tools::ToolAssembly> {
    TOOLS.with(|bank|bank.borrow().get(id as usize).cloned())
}
pub(crate) fn tool_icon(tool:&engine_tools::ToolAssembly)->Icon {
    let schema=engine_tools::PARTS.schemas.iter().position(|s|s.id==tool.schema).unwrap_or(0) as u16;
    let mut materials=[255;8];
    for (i,(_,m)) in tool.parts.iter().enumerate().take(8) {
        materials[i]=engine_tools::MATERIALS.iter().position(|r|r.id==*m).unwrap_or(0) as u8;
    }
    Icon::BuiltTool(schema,materials,if tool.is_broken(){128}else{tool.modifiers.len().min(15) as u8})
}

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
        Item::Assembled { id, schema } => Art::Icon(tool(id).as_ref().map_or(Icon::BuiltTool(schema,[0;8],0),tool_icon)),
        Item::Part { kind,material } => Art::Icon(Icon::ToolPart(kind,material)),
        Item::Cast(kind) => Art::Icon(Icon::ToolCast(kind)),
        Item::Bucket(amount) => Art::Icon(Icon::Bucket(amount > 0)),
        Item::PowerKit(k) => block(k.kind.block()),
        Item::Block(BlockId::ClaimStake) => Art::Icon(Icon::ClaimStake),
        Item::Block(b) => block(b),
        Item::Resource(i) => Art::Icon(Icon::Resource(i as usize)),
        Item::Food(f) => Art::Icon(Icon::Food(f as u8)),
        Item::Weapon(tier) => Art::Icon(Icon::Sword(tier as u32)),
        Item::Armor(slot, tier) => Art::Icon(Icon::Armor(slot as u8, tier as u8)),
        Item::Tool(t @ 6..=8) => Art::Icon(Icon::Pickaxe(t)),
        Item::Tool(t @ 12..=14) => Art::Icon(Icon::Axe(t)),
        Item::Tool(t @ 15..=17) => Art::Icon(Icon::Shovel(t)),
        Item::HOE => Art::Icon(Icon::Hoe),
        Item::Tool(1 | 2) => Art::Icon(Icon::Bore),
        Item::Tool(9) => Art::Icon(Icon::Boat),
        Item::Tool(10) => Art::Icon(Icon::Rod),
        Item::Tool(11) => Art::Icon(Icon::Shears),
        Item::Tool(18) => Art::Icon(Icon::Pan),
        Item::Tool(19) => Art::Icon(Icon::Sling),
        Item::Tool(40) => Art::Icon(Icon::Clock),
        Item::Tool(41) => Art::Icon(Icon::Map),
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
    if let Some((width, height, mut texels)) = super::icons::tools::texels(icon) {
        // The slot outline is ink. Extruding it would add a dark layer of material
        // around the tool and fill the small gaps in its silhouette.
        for texel in &mut texels {
            if matches!(texel, Some((0, _))) {
                *texel = None;
            }
        }
        return (width, height, texels);
    }
    if let Some(sprite) = super::icons::sprites16::texels(icon) {
        return sprite;
    }
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

pub(crate) fn held_head(icon: Icon) -> Option<Vec<Option<(u8, [u8; 3])>>> {
    super::icons::tools::held_head(icon)
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
        let light = |t: &Option<(u8, [u8; 3])>| t.map_or(0, |(_, c)| c.iter().map(|&v| v as u32).sum::<u32>());
        let flame = (0..texels.len()).max_by_key(|&i| light(&texels[i])).unwrap();
        let stick = texels.iter().rposition(|t| t.is_some()).unwrap();
        assert!(flame / w < stick / w);
    }
}
