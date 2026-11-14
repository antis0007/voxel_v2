use super::{
    BlockId, World, WorldSnapshot, chunk_coord_from_world_block, mark_chunk_and_neighbors_dirty,
};
use crate::simulation::cpu_region::{EDGE, Snapshot};
use glam::IVec3;
use std::{collections::HashSet, sync::Arc};

pub(super) fn cells(view: &Snapshot) -> impl Iterator<Item = IVec3> + '_ {
    (-1..=EDGE).flat_map(move |y| {
        (-1..=EDGE).flat_map(move |z| (-1..=EDGE).map(move |x| view.origin + IVec3::new(x, y, z)))
    })
}

pub(super) fn block(cell: engine_sim::Cell) -> BlockId {
    if cell.volume == 0 {
        BlockId::Air
    } else {
        BlockId::from_u32(u32::from(cell.material))
    }
}

impl World {
    /// Publish one coherent view and invalidate only its changed dependency halos.
    pub(crate) fn publish_ca_view(
        &mut self,
        view: Arc<Snapshot>,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
    ) {
        let mut changed = false;
        for p in cells(&view) {
            if self.ca_view.as_ref().and_then(|old| old.cell(p)) == view.cell(p) {
                continue;
            }
            blocks.insert(p);
            mark_chunk_and_neighbors_dirty(chunks, p);
            changed = true;
        }
        self.ca_view = Some(view);
        if changed {
            self.revision = self.revision.wrapping_add(1);
            self.full_snapshot_cache = None;
            self.snapshot_revision = u64::MAX;
        }
    }
}

impl WorldSnapshot {
    pub(crate) fn ca_chunks(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.ca_view.iter().flat_map(|view| {
            let lo = chunk_coord_from_world_block(view.origin - IVec3::ONE);
            let hi = chunk_coord_from_world_block(view.origin + IVec3::splat(EDGE));
            (lo.y..=hi.y).flat_map(move |y| {
                (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z)))
            })
        })
    }

    pub(crate) fn apply_ca_cells(&self, target: &mut crate::mesh::GeneratedChunk) {
        let Some(view) = &self.ca_view else { return };
        let min = target.origin();
        let max = min + IVec3::splat(crate::mesh::PADDED_CHUNK_SIZE);
        for p in cells(view).filter(|p| p.cmpge(min).all() && p.cmplt(max).all()) {
            if let Some(cell) = view.cell(p) {
                target.set_world(p, block(cell));
            }
        }
    }
}

impl super::water::WaterSnapshot {
    pub(super) fn with_ca(mut self, view: Option<&Snapshot>, cell_volume: u64) -> Self {
        let Some(view) = view else { return self };
        for p in cells(view) {
            let Some(value) = view.cell(p) else { continue };
            self.set_ca_water(p, value, cell_volume);
        }
        self
    }
}
