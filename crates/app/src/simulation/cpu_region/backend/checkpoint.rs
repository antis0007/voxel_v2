use super::{B, CELLS, Cell, Cpu, EDGE, IVec3, Snapshot, split_voxel, validate};
use crate::simulation::cpu_region::checkpoint::codec::{Reader, cell, number};

impl Cpu {
    pub(in crate::simulation::cpu_region) fn encode(&self, out: &mut Vec<u8>) {
        let awake = self.store.dirty_bricks();
        for y in -1..=1 {
            for z in -1..=1 {
                for x in -1..=1 {
                    let base = self.origin + IVec3::new(x, y, z) * 8;
                    let (chunk, local) = split_voxel(base);
                    let brick = engine_sim::brick_and_cell(local).0;
                    number(
                        out,
                        u64::from(awake.iter().any(|b| b.chunk == chunk && b.brick == brick)),
                    );
                    for i in 0..CELLS {
                        let (c, p) = split_voxel(base + super::super::local(i));
                        cell(out, self.store.get(c, p));
                    }
                }
            }
        }
    }

    pub(in crate::simulation::cpu_region) fn decode(
        input: &mut Reader<'_>,
        origin: IVec3,
        seed: u32,
        completed: u64,
        applied: u64,
    ) -> Result<Self, String> {
        let mut cpu = Self::reserved(origin, seed)?;
        let mut awake = Vec::with_capacity(27);
        for y in -1..=1 {
            for z in -1..=1 {
                for x in -1..=1 {
                    let base = origin + IVec3::new(x, y, z) * 8;
                    let active = input.count(1)? == 1;
                    let (chunk, local) = split_voxel(base);
                    let brick = engine_sim::brick_and_cell(local).0;
                    if active {
                        awake.push((chunk, brick));
                    }
                    for i in 0..CELLS {
                        let at = base + super::super::local(i);
                        let q = at - origin;
                        let value = input.cell()?;
                        validate(value)?;
                        let interior = q.min_element() >= 0 && q.max_element() < EDGE;
                        let shell = q.min_element() >= -1 && q.max_element() <= EDGE;
                        if !interior
                            && value
                                != if shell {
                                    Cell::filled(B::Bedrock as u8)
                                } else {
                                    Cell::EMPTY
                                }
                        {
                            return Err("CPU checkpoint changed its sealed boundary".into());
                        }
                        let (c, p) = split_voxel(at);
                        cpu.store
                            .set(c, p, value)
                            .map_err(|_| "CPU checkpoint capacity exhausted")?;
                    }
                }
            }
        }
        cpu.store.take_all_dirty();
        for (chunk, brick) in awake {
            cpu.store.wake(chunk, brick);
        }
        cpu.completed = completed;
        cpu.applied_sequence = applied;
        Ok(cpu)
    }

    pub(in crate::simulation::cpu_region) fn matches(&self, snapshot: &Snapshot) -> bool {
        let current = self.snapshot(snapshot.stats);
        current.hash == snapshot.hash && current.cells == snapshot.cells
    }
}
