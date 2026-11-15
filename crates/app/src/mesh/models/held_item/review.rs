//! Review-capture overrides for the held item.
use super::*;

fn review_held() -> Option<String> {
    std::env::args().any(|a| a.starts_with("--benchmark-")).then(|| std::env::var("VOXEL_REVIEW_HELD").ok()).flatten()
}

/// `VOXEL_REVIEW_HELD=empty` pins a bare hand.
pub(super) static EMPTY: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| review_held().as_deref() == Some("empty"));

/// Review captures pin the held item: `VOXEL_REVIEW_HELD=stone|grass|planks|crystal|torch|food|sword|tool:<schema>|gem|fibre|sling|clock|map|cable|resource:<id>|food:<n>|block:<BlockId>|item-tool:<n>`.
/// `tool:<schema>` holds bank entry 0, which `review_bank` builds from iron, gold and wood parts.
pub(super) static REVIEW: std::sync::LazyLock<Option<Item>> = std::sync::LazyLock::new(|| {
    Some(match review_held()?.as_str() {
        "empty" => return None,
        "stone" => Item::Block(BlockId::Stone),
        "grass" => Item::Block(BlockId::Grass),
        "planks" => Item::Block(BlockId::Planks),
        "crystal" => Item::Block(BlockId::Crystal),
        "torch" => Item::Block(BlockId::Torch),
        "food" => Item::Food(crate::world::inventory::FoodKind::RoastedCap),
        "sword" => Item::Weapon(2),
        "pickaxe" => Item::Tool(8),
        "axe" => Item::Tool(14),
        "shovel" => Item::Tool(17),
        "hoe" => Item::HOE,
        "shears" => Item::Tool(11),
        "gem" => Item::Resource(6),
        "fibre" => Item::Resource(4),
        "sling" => Item::Tool(19),
        "clock" => Item::Tool(40),
        "map" => Item::Tool(41),
        "cable" => Item::Block(crate::world::factory::Kind::Cable.block()),
        raw if raw.starts_with("food:") => Item::Food(crate::world::inventory::FoodKind::from_u8(raw["food:".len()..].parse().ok()?)?),
        raw if raw.starts_with("block:") => {
            let name = &raw["block:".len()..];
            Item::Block(crate::material::ALL_BLOCK_IDS.iter().copied().find(|b| format!("{b:?}") == name)?)
        }
        raw if raw.starts_with("item-tool:") => Item::Tool(raw["item-tool:".len()..].parse().ok()?),
        raw if raw.starts_with("resource:") => Item::Resource(raw["resource:".len()..].parse().ok()?),
        raw => {
            let schema = engine_tools::PARTS.schemas.iter().position(|s| Some(s.id.as_str()) == raw.strip_prefix("tool:"))?;
            Item::Assembled { id: 0, schema: schema as u16 }
        }
    })
});

/// Comma-separated numbers from a review env var, only in benchmark runs.
fn review_numbers<const N: usize>(name: &str) -> Option<[f32; N]> {
    std::env::args().any(|a| a.starts_with("--benchmark-")).then_some(())?;
    let v: Vec<f32> = std::env::var(name).ok()?.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    v.try_into().ok()
}

/// Review overrides to tune held poses without a rebuild (radians, metres, texels):
/// `VOXEL_REVIEW_TOOL_GRIP=pitch,yaw,roll` long tools, `VOXEL_REVIEW_SPRITE_GRIP=pitch,yaw,roll` other sprites,
/// `VOXEL_REVIEW_SOLID_GRIP=pitch,yaw,roll,texel_m,socket_y` produce and food, `VOXEL_REVIEW_TORCH_TILT=pitch,yaw,roll`.
pub(super) static TOOL_GRIP: std::sync::LazyLock<Option<[f32; 3]>> = std::sync::LazyLock::new(|| review_numbers("VOXEL_REVIEW_TOOL_GRIP"));
pub(super) static SPRITE_GRIP: std::sync::LazyLock<Option<[f32; 3]>> = std::sync::LazyLock::new(|| review_numbers("VOXEL_REVIEW_SPRITE_GRIP"));
pub(super) static SOLID_GRIP: std::sync::LazyLock<Option<[f32; 5]>> = std::sync::LazyLock::new(|| review_numbers("VOXEL_REVIEW_SOLID_GRIP"));
pub(super) static TORCH_TILT: std::sync::LazyLock<Option<[f32; 3]>> = std::sync::LazyLock::new(|| review_numbers("VOXEL_REVIEW_TORCH_TILT"));

/// Tool bank a `tool:<schema>` review capture draws from, in place of the world's.
pub(crate) fn review_bank() -> Option<&'static [engine_tools::ToolAssembly]> {
    static BANK: std::sync::LazyLock<Option<Vec<engine_tools::ToolAssembly>>> = std::sync::LazyLock::new(|| {
        let Some(Item::Assembled { schema, .. }) = *REVIEW else { return None };
        let def = engine_tools::PARTS.schemas.get(schema as usize)?;
        let pick = ["iron", "gold", "wood"];
        let materials: Vec<_> = (0..def.parts.len()).map(|i| engine_tools::MaterialId(pick[i.min(2)].into())).collect();
        engine_tools::ToolAssembly::new(&def.id, &materials).ok().map(|t| vec![t])
    });
    BANK.as_deref()
}

/// A review capture pinned the held item, overriding any wand in the hand.
pub(crate) fn pinned() -> bool {
    REVIEW.is_some() || *EMPTY
}
