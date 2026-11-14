//! Groups validated cells while retaining lexicographic publication order.
use super::{BlockId, Group, IVec3, replication, world};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type PreparedGroups = (Option<[i32; 3]>, VecDeque<Group>);

pub(super) fn prepare(
    frame: replication::terrain::Frame,
    missing: &BTreeSet<[i32; 3]>,
) -> Result<PreparedGroups> {
    let terrain = frame.chunk.as_ref().map(|chunk| chunk.coord);
    if let Some(coord) = terrain {
        ensure!(missing.contains(&coord), "unexpected terrain frame chunk");
    }
    let mut groups = BTreeMap::<[i32; 3], Vec<(IVec3, BlockId)>>::new();
    for (p, block) in frame.cells {
        let p = IVec3::from_array(p);
        let coord = world::chunk_coord_from_world_block(p).to_array();
        ensure!(
            !missing.contains(&coord) || terrain == Some(coord),
            "terrain edit precedes its chunk"
        );
        groups.entry(coord).or_default().push((p, BlockId::from_u32(block)));
    }
    if let Some(chunk) = frame.chunk {
        let mut blocks = vec![BlockId::Air; 32768];
        let mut next = 0;
        for (count, block) in chunk.runs {
            let end = next + usize::from(count);
            blocks[next..end].fill(BlockId::from_u32(block));
            next = end;
        }
        let edits = groups.entry(chunk.coord).or_default();
        // Explicit cells override the full-chunk image before staging any write.
        for &(p, block) in edits.iter() {
            blocks[world::ChunkData::local_index(p.x, p.y, p.z)] = block;
        }
        edits.clear();
        edits.reserve(32768);
        let base = IVec3::from_array(chunk.coord) * 32;
        for x in 0..32 {
            for y in 0..32 {
                for z in 0..32 {
                    let p = base + IVec3::new(x, y, z);
                    edits.push((p, blocks[world::ChunkData::local_index(x, y, z)]));
                }
            }
        }
    }
    let groups = groups.into_iter().map(|(coord, mut edits)| {
        if terrain != Some(coord) {
            // Frame validation already rejected duplicate explicit cells.
            edits.sort_unstable_by_key(|(p, _)| p.to_array());
        }
        Group {
            coord: IVec3::from_array(coord),
            edits: edits.into_boxed_slice().into_vec(),
            next: 0,
            staged: None,
        }
    }).collect();
    Ok((terrain, groups))
}

#[cfg(test)]
mod tests;
