//! Test helper: solid cells in a box that no face-adjacent chain joins to the
//! ground. A roof whose steps meet only at edges, or a slab left above a
//! cleared wall, shows up here as its own island.
use crate::{BlockId as B, WorldSampleConfig};
use glam::IVec3;
use std::collections::HashMap;

/// Natural terrain and canopy ground whatever touches them and are never
/// reported themselves: a cliff lip or a pruned branch is not a building fault.
fn natural(block: B) -> bool {
    let name = format!("{block:?}");
    matches!(
        name.as_str(),
        "Stone" | "Sandstone" | "Dirt" | "Grass" | "Sand" | "Gravel" | "Clay" | "Mud" | "Ice" | "Bedrock"
            | "Limestone" | "Crystal" | "Torch" | "SalvageCrate" | "SurveyCache" | "CultivatorCache"
    ) || name.contains("Leaves")
        || name.contains("Cap")
        || name.contains("Mushroom")
        || name.contains("Snow")
        || name.contains("Ore")
        || name.contains("Soil")
}

/// Islands of structural cells inside `[lo, hi]` (inclusive, cells). A cell on
/// the box boundary counts as grounded, as its support may lie outside.
pub(crate) fn islands(cfg: &WorldSampleConfig, lo: IVec3, hi: IVec3) -> Vec<Vec<(IVec3, B)>> {
    let mut solid: HashMap<IVec3, B> = HashMap::new();
    for z in lo.z..=hi.z {
        for x in lo.x..=hi.x {
            for y in lo.y..=hi.y {
                let p = IVec3::new(x, y, z);
                let b = crate::sample_block(cfg, p);
                if b.is_solid() {
                    solid.insert(p, b);
                }
            }
        }
    }
    let keys: Vec<IVec3> = solid.keys().copied().collect();
    let index: HashMap<IVec3, usize> = keys.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut parent: Vec<usize> = (0..keys.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (i, k) in keys.iter().enumerate() {
        for d in [IVec3::X, IVec3::Y, IVec3::Z] {
            if let Some(&j) = index.get(&(*k + d)) {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                if a != b {
                    parent[a] = b;
                }
            }
        }
    }
    let mut grounded = vec![false; keys.len()];
    for (i, k) in keys.iter().enumerate() {
        let edge = k.x == lo.x || k.x == hi.x || k.y == lo.y || k.z == lo.z || k.z == hi.z;
        if edge || natural(solid[k]) {
            let r = find(&mut parent, i);
            grounded[r] = true;
        }
    }
    let mut groups: HashMap<usize, Vec<(IVec3, B)>> = HashMap::new();
    for (i, k) in keys.iter().enumerate() {
        let r = find(&mut parent, i);
        if !grounded[r] {
            groups.entry(r).or_default().push((*k, solid[k]));
        }
    }
    let mut out: Vec<_> = groups.into_values().collect();
    for g in &mut out {
        g.sort_by_key(|(p, _)| (p.y, p.x, p.z));
    }
    out.sort_by_key(|g| std::cmp::Reverse(g.len()));
    out
}

/// One line per island: size, block mix and the lowest cell.
pub(crate) fn describe(islands: &[Vec<(IVec3, B)>]) -> String {
    islands
        .iter()
        .take(12)
        .map(|g| {
            let mut kinds: Vec<String> = g.iter().map(|(_, b)| format!("{b:?}")).collect();
            kinds.sort();
            kinds.dedup();
            let lo = g.iter().fold(g[0].0, |a, (p, _)| a.min(*p));
            let hi = g.iter().fold(g[0].0, |a, (p, _)| a.max(*p));
            format!("{} cells from {:?} to {:?} ({})", g.len(), lo, hi, kinds.join(", "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
