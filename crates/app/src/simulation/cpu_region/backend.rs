use super::{CELLS, CaBackend, Command, EDGE, Snapshot, local, EditOutcome, EditStatus, StepResult};
use crate::world::{BlockId as B, World};
use engine_sim::{BrickStore, Cell, StepStats, split_voxel};
use glam::IVec3;
mod checkpoint;
pub(super) mod gpu;

/// This first adapter admits only ambient water/sand and inert containment.
/// Unsupported state is rejected, never approximated or erased on import.
pub fn validate(cell: Cell) -> Result<(), String> {
    let material = B::from_u32(cell.material as u32);
    if !matches!(
        material,
        B::Air | B::Water | B::Sand | B::Stone | B::Bedrock
    ) || cell.material != material.sim_byte()
        || cell.volume > 64
        || cell.temperature != Cell::EMPTY.temperature
        || cell.pressure != 0
        || cell.charge != 0
        || (material == B::Air && cell != Cell::EMPTY)
        || (material != B::Air && cell.volume == 0)
        || (!matches!(material, B::Air | B::Water) && cell.volume != 64)
    {
        return Err("CPU region supports ambient air/water/sand/stone/bedrock only".into());
    }
    Ok(())
}

pub(super) fn validate_origin(origin: IVec3) -> Result<(), String> {
    if (origin.x | origin.y | origin.z) & 7 != 0
        || origin.min_element() < i32::MIN + 32
        || origin.max_element() > i32::MAX - 32
    {
        return Err("CPU region origin must be 8-cell aligned and leave coordinate headroom".into());
    }
    Ok(())
}

pub struct Cpu {
    store: BrickStore,
    origin: IVec3,
    seed: u32,
    completed: u64,
    applied_sequence: u64,
}

impl Cpu {
    pub fn import(world: &World, origin: IVec3) -> Result<(Self, Snapshot), String> {
        let mut cpu = Self::reserved(origin, world.seed)?;
        cpu.import_cells(world)?;
        let snapshot = cpu.snapshot(StepStats::default());
        Ok((cpu, snapshot))
    }

    fn reserved(origin: IVec3, seed: u32) -> Result<Self, String> {
        validate_origin(origin)?;
        let mut cpu = Self {
            store: BrickStore::new(27),
            origin,
            seed,
            completed: 0,
            applied_sequence: 0,
        };
        // All bricks touched by the interior plus sealed one-cell halo are
        // reserved up front. Pool exhaustion rejects activation atomically.
        for y in -1..=1 {
            for z in -1..=1 {
                for x in -1..=1 {
                    let (c, p) = split_voxel(origin + IVec3::new(x, y, z) * 8);
                    let (b, _) = engine_sim::brick_and_cell(p);
                    cpu.store
                        .reserve(c, b)
                        .map_err(|_| "CPU region brick capacity exhausted")?;
                }
            }
        }
        Ok(cpu)
    }

    fn import_cells(&mut self, world: &World) -> Result<(), String> {
        for y in -1..=EDGE {
            for z in -1..=EDGE {
                for x in -1..=EDGE {
                    let q = IVec3::new(x, y, z);
                    let p = self.origin + q;
                    let shell = q.min_element() < 0 || q.max_element() >= EDGE;
                    let block = if shell { B::Bedrock } else { world.get(p) };
                    let cell = if block == B::Air {
                        Cell::EMPTY
                    } else {
                        Cell {
                            volume: if block == B::Water {
                                let scaled = world.water_microlitres(p) * 64;
                                let capacity = engine_core::environment::cell_volume(world.voxel_scale);
                                if scaled % capacity != 0 || scaled / capacity > 64 {
                                    return Err(format!("water at {p} is not exactly representable in 64 units"));
                                }
                                (scaled / capacity) as u8
                            } else {
                                64
                            },
                            ..Cell::filled(block.sim_byte())
                        }
                    };
                    validate(cell).map_err(|e| format!("{e} at {p}"))?;
                    let (c, p) = split_voxel(p);
                    self.store
                        .set(c, p, cell)
                        .map_err(|_| "CPU region import capacity exhausted")?;
                }
            }
        }
        Ok(())
    }

    fn snapshot(&self, stats: StepStats) -> Snapshot {
        let mut cells = Box::new([Cell::EMPTY; CELLS]);
        for (i, cell) in cells.iter_mut().enumerate() {
            let (c, p) = split_voxel(self.origin + local(i));
            *cell = self.store.get(c, p);
        }
        Snapshot {
            origin: self.origin,
            tick: self.completed,
            applied_sequence: self.applied_sequence,
            cells,
            hash: self.store.state_hash(),
            stats,
        }
    }
}

impl Cpu {
    fn copy_state(&self) -> Self {
        Self { store: self.store.clone(), origin: self.origin, seed: self.seed,
            completed: self.completed, applied_sequence: self.applied_sequence }
    }

    fn apply_commands(&mut self, tick: u64, commands: &[Command]) -> Result<Vec<EditOutcome>, String> {
        let mut outcomes = Vec::with_capacity(commands.len());
        for command in commands {
            let (c, p) = split_voxel(command.at);
            let before = self.store.get(c, p);
            let status = if command.expected.is_some_and(|expected| expected != before) {
                EditStatus::Conflict
            } else if before == command.cell {
                EditStatus::NoChange
            } else if command.expected.is_some() && before.material == B::Bedrock as u8 {
                EditStatus::Protected
            } else {
                EditStatus::Applied
            };
            outcomes.push(EditOutcome { sequence: command.sequence, tick, at: command.at,
                before, after: if status == EditStatus::Applied { command.cell } else { before }, status });
            self.applied_sequence = command.sequence;
            if status != EditStatus::Applied { continue; }
            self.store
                .set(c, p, command.cell)
                .map_err(|_| "reserved CPU command brick missing")?;
            // A newly opened cell must wake sleeping neighbours on every face.
            for d in [
                IVec3::ZERO,
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ] {
                let (c, p) = split_voxel(command.at + d);
                self.store.wake(c, engine_sim::brick_and_cell(p).0);
            }
        }
        Ok(outcomes)
    }
}

impl CaBackend for Cpu {
    fn advance(&mut self, tick: u64, commands: &[Command]) -> Result<StepResult, String> {
        let kernel_tick = u32::try_from(tick - 1).map_err(|_| "CPU kernel tick exhausted")?;
        let outcomes = self.apply_commands(tick, commands)?;
        let stats = engine_sim::step(
            &mut self.store,
            &engine_content::SIM_RULES,
            self.seed,
            kernel_tick,
        );
        self.completed = tick;
        Ok(StepResult { snapshot: self.snapshot(stats), outcomes })
    }
}
