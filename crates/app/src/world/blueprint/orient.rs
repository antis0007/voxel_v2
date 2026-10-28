//! How a block's facing changes when a blueprint turns or mirrors.
//!
//! Axis variants are paired by name so new rows in `materials.ron` follow
//! without a table here: a trailing `X`/`Z` swaps (`TimberBeamX`), and a `Z`
//! word marks the quarter-turned twin of a fitting (`OakDoorZUpper`).
use crate::world::BlockId;
use engine_content::ALL_BLOCK_IDS;
use glam::IVec3;
use std::sync::LazyLock;

/// Per block id: the quarter-turned block and the mirrored block.
static TURNS: LazyLock<Vec<(BlockId, BlockId)>> = LazyLock::new(|| {
    let names: Vec<String> = ALL_BLOCK_IDS.iter().map(|b| format!("{b:?}")).collect();
    let find = |name: &str| names.iter().position(|n| n == name).map(|i| ALL_BLOCK_IDS[i]);
    ALL_BLOCK_IDS
        .iter()
        .zip(&names)
        .map(|(&b, name)| {
            let turned = match b.torch_support_offset() {
                Some(o) => torch_with_support(IVec3::new(-o.z, o.y, o.x)),
                None => axis_twin(name, &find),
            };
            let mirrored = b.torch_support_offset().and_then(|o| torch_with_support(IVec3::new(-o.x, o.y, o.z)));
            (turned.unwrap_or(b), mirrored.unwrap_or(b))
        })
        .collect()
});

fn torch_with_support(offset: IVec3) -> Option<BlockId> {
    ALL_BLOCK_IDS.iter().copied().find(|t| t.is_torch() && t.torch_support_offset() == Some(offset))
}

fn axis_twin(name: &str, find: &dyn Fn(&str) -> Option<BlockId>) -> Option<BlockId> {
    let stem = &name[..name.len().saturating_sub(1)];
    let swapped = match name.chars().last() {
        Some('X') => find(&format!("{stem}Z")),
        Some('Z') => find(&format!("{stem}X")),
        _ => None,
    };
    if swapped.is_some() {
        return swapped;
    }
    let starts: Vec<usize> = name.char_indices().filter(|(_, c)| c.is_ascii_uppercase()).map(|(i, _)| i).chain([name.len()]).collect();
    let words: Vec<&str> = starts.windows(2).map(|w| &name[w[0]..w[1]]).collect();
    if let Some(at) = words.iter().position(|w| *w == "Z") {
        let mut rest = words.clone();
        rest.remove(at);
        return find(&rest.concat());
    }
    (1..=words.len()).find_map(|at| {
        let mut with = words.clone();
        with.insert(at, "Z");
        find(&with.concat())
    })
}

/// The same block facing a quarter turn further, matching `Blueprint::rotated`.
pub fn rotate_block(b: BlockId) -> BlockId {
    TURNS.get(b as usize).map_or(b, |t| t.0)
}

/// The same block mirrored across the YZ plane.
pub fn mirror_block(b: BlockId) -> BlockId {
    TURNS.get(b as usize).map_or(b, |t| t.1)
}
