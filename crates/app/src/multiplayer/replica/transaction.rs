//! Stages one ordered terrain/gameplay frame before publication.
use super::super::{replication, *};
use crate::{simulation::residency::Residency, world::replica::StagedChunk};
use std::collections::{BTreeSet, VecDeque};

mod grouping;

const CELLS_PER_POLL: usize = 4096;
struct Group {
    coord: IVec3,
    edits: Vec<(IVec3, BlockId)>,
    next: usize,
    staged: Option<StagedChunk>,
}
struct Pending {
    prepared: replication::Prepared,
    groups: VecDeque<Group>,
    staged: Vec<StagedChunk>,
    bytes: usize,
    terrain: Option<[i32; 3]>,
    began: Instant,
}
#[derive(Default)]
pub(in crate::multiplayer) struct Queue {
    pending: Option<Pending>,
    missing: BTreeSet<[i32; 3]>,
    committed: u64,
    receipt: Option<u64>,
    receiving: Option<Instant>,
    sources: Residency,
    #[cfg(test)]
    staged_peak: usize,
}
impl Queue {
    pub(in crate::multiplayer) fn set_missing(&mut self, missing: BTreeSet<[i32; 3]>) {
        self.missing = missing;
    }
    pub(in crate::multiplayer) fn missing_at(&self, (lo, hi): (Vec3, Vec3)) -> bool {
        if self.missing.is_empty() {
            return false;
        }
        let lo = world::chunk_coord_from_world_block(lo.floor().as_ivec3());
        let hi = world::chunk_coord_from_world_block(hi.floor().as_ivec3());
        (lo.x..=hi.x).any(|x| {
            (lo.y..=hi.y).any(|y| (lo.z..=hi.z).any(|z| self.missing.contains(&[x, y, z])))
        })
    }
    pub(in crate::multiplayer) fn begin_receive(&mut self, now: Instant) -> Result<()> {
        ensure!(
            self.receiving.is_none() && self.pending.is_none() && self.receipt.is_none(),
            "overlapping gameplay image transfers"
        );
        self.receiving = Some(now);
        Ok(())
    }
    pub(in crate::multiplayer) fn receipt(&self) -> Option<u64> {
        self.receipt
    }
    pub(in crate::multiplayer) fn receipt_sent(&mut self) {
        self.receipt = None;
    }

    pub(in crate::multiplayer) fn stage(
        &mut self,
        mut prepared: replication::Prepared,
        now: Instant,
        command_sequence: u64,
    ) -> Result<()> {
        ensure!(
            self.receiving
                .is_none_or(|began| now.saturating_duration_since(began) <= SYNC_TIMEOUT),
            "gameplay image transfer timed out"
        );
        ensure!(
            prepared.command_ack <= command_sequence,
            "invalid command acknowledgement"
        );
        ensure!(
            self.pending.is_none() && self.receipt.is_none(),
            "overlapping gameplay frames"
        );
        ensure!(
            self.committed.checked_add(1) == Some(prepared.sequence),
            "gameplay frame sequence mismatch"
        );
        prepared.terrain.validate()?;
        let (terrain, groups) = grouping::prepare(
            std::mem::take(&mut prepared.terrain),
            &self.missing,
        )?;
        let bytes = std::mem::size_of::<Pending>()
            + groups.capacity() * std::mem::size_of::<Group>()
            + groups
                .iter()
                .map(|group| group.edits.capacity() * std::mem::size_of::<(IVec3, BlockId)>())
                .sum::<usize>();
        ensure!(
            bytes <= replication::terrain::MAX_STAGED_BYTES,
            "terrain frame staging budget exceeded"
        );
        self.receiving = None;
        self.pending = Some(Pending {
            prepared,
            groups,
            staged: Vec::new(),
            bytes,
            terrain,
            began: now,
        });
        Ok(())
    }

    pub(in crate::multiplayer) fn drain(
        &mut self,
        world: &mut World,
        player: &mut Player,
        day: &mut DayNightCycle,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
        now: Instant,
    ) -> Result<Option<replication::Applied>> {
        ensure!(
            self.receiving
                .is_none_or(|began| now.saturating_duration_since(began) <= SYNC_TIMEOUT),
            "gameplay image transfer timed out"
        );
        let Some(pending) = &mut self.pending else {
            return Ok(None);
        };
        ensure!(
            now.saturating_duration_since(pending.began) <= SYNC_TIMEOUT,
            "terrain frame preparation timed out"
        );
        self.sources.poll_replica(world);
        for group in pending.groups.iter().take(8) {
            if group.staged.is_none() {
                self.sources.request(world, group.coord);
            }
        }
        let began = Instant::now();
        let mut remaining = CELLS_PER_POLL;
        while remaining > 0 && began.elapsed() < Duration::from_millis(2) {
            let Some(group) = pending.groups.front_mut() else {
                break;
            };
            if group.staged.is_none() {
                let Some(base) = self.sources.replica_source(world, group.coord) else {
                    break;
                };
                let staged = StagedChunk::new(group.coord, base);
                pending.bytes = pending.bytes.saturating_add(staged.bytes());
                ensure!(
                    pending.bytes <= replication::terrain::MAX_STAGED_BYTES,
                    "terrain frame staging budget exceeded"
                );
                group.staged = Some(staged);
            }
            let staged = group
                .staged
                .as_mut()
                .context("missing staged terrain chunk")?;
            let before = staged.bytes();
            let end = group.edits.len().min(group.next + remaining.min(256));
            for &(p, b) in &group.edits[group.next..end] {
                staged.edit(p, b);
            }
            remaining -= end - group.next;
            group.next = end;
            pending.bytes = pending
                .bytes
                .saturating_sub(before)
                .saturating_add(staged.bytes());
            #[cfg(test)]
            {
                self.staged_peak = self.staged_peak.max(pending.bytes);
            }
            ensure!(
                pending.bytes <= replication::terrain::MAX_STAGED_BYTES,
                "terrain frame staging budget exceeded"
            );
            if group.next == group.edits.len() {
                let mut group = pending
                    .groups
                    .pop_front()
                    .context("missing terrain group")?;
                pending.staged.push(
                    group
                        .staged
                        .take()
                        .context("missing staged terrain chunk")?,
                );
            }
        }
        if !pending.groups.is_empty() {
            return Ok(None);
        }
        let pending = self.pending.take().context("missing prepared image")?;
        crate::world::replica::publish(world, pending.staged, blocks, chunks)?;
        let applied = (pending.prepared.publish)(world, player, day, blocks, chunks);
        if let Some(coord) = pending.terrain {
            self.missing.remove(&coord);
        }
        self.committed = pending.prepared.sequence;
        self.receipt = Some(self.committed);
        Ok(Some(applied))
    }
}

#[cfg(test)]
mod tests;
