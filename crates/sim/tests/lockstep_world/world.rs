//! One peer's copy of the matter simulation, the lockstep payload and its codec.
use engine_core::hash::hash_u32;
use engine_core::material::{MovementClass, SIM_MATERIAL_ROWS, SimMaterial};
use engine_core::reaction::{ReactionTable, SimRules};
use engine_netcode::lockstep::{Frame, PeerId};
use engine_netcode::wire::{Reader, WireError, put_i64, put_u64};
use engine_sim::{BrickStore, CHUNK_CELLS, Cell, RegionDigest, VOLUME_FULL, split_voxel, step_tracked};
use glam::IVec3;
use std::collections::BTreeMap;

pub const STONE: u8 = 1;
pub const SAND: u8 = 2;
pub const WATER: u8 = 3;
/// The region: 64 x 32 x 64 cells, 2 x 1 x 2 chunks, inside a stone tank.
pub const SIZE: IVec3 = IVec3::new(64, 32, 64);

pub fn rules() -> SimRules {
    let row = |class, kg: f32, flow, k: f32, cp: u16| {
        let density = (kg * 100.0) as u32;
        SimMaterial {
            class,
            density,
            flow,
            conductivity: (k * 100.0).round() as u16,
            capacity: cp,
            thermal_mass: ((density as u64 * cp as u64) >> 10) as u32,
            ..SimMaterial::INERT
        }
    };
    let mut t = [SimMaterial::INERT; SIM_MATERIAL_ROWS];
    t[0] = row(MovementClass::Void, 1.2, 0, 0.026, 1005);
    t[STONE as usize] = row(MovementClass::Solid, 2600.0, 0, 2.5, 790);
    t[SAND as usize] = row(MovementClass::Powder, 1700.0, 0, 0.3, 830);
    t[WATER as usize] = row(MovementClass::Liquid, 1000.0, 255, 0.6, 4186);
    SimRules::new(t, ReactionTable::default())
}

/// What a lockstep frame carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Edit { at: IVec3, cell: Cell },
    /// Replace these chunks on `peer` with the host's state at the start of the frame's tick.
    Repair { peer: PeerId, chunks: Vec<IVec3> },
}

pub fn put_op(op: &Op, out: &mut Vec<u8>) {
    match op {
        Op::Edit { at, cell } => {
            put_u64(out, 0);
            [at.x, at.y, at.z].iter().for_each(|&c| put_i64(out, i64::from(c)));
            put_u64(out, u64::from(cell.material));
            put_u64(out, u64::from(cell.volume));
            put_u64(out, u64::from(cell.temperature));
        }
        Op::Repair { peer, chunks } => {
            put_u64(out, 1);
            put_u64(out, u64::from(*peer));
            put_u64(out, chunks.len() as u64);
            chunks.iter().for_each(|c| [c.x, c.y, c.z].iter().for_each(|&v| put_i64(out, i64::from(v))));
        }
    }
}

pub fn get_op(r: &mut Reader) -> Result<Op, WireError> {
    let ivec = |r: &mut Reader| -> Result<IVec3, WireError> { Ok(IVec3::new(r.i32()?, r.i32()?, r.i32()?)) };
    let byte = |r: &mut Reader| u8::try_from(r.u64()?).map_err(|_| WireError("byte"));
    match r.u64()? {
        0 => {
            let at = ivec(r)?;
            let (material, volume) = (byte(r)?, byte(r)?);
            let temperature = u16::try_from(r.u64()?).map_err(|_| WireError("temperature"))?;
            Ok(Op::Edit { at, cell: Cell { material, volume, temperature, pressure: 0, charge: 0 } })
        }
        1 => {
            let peer = PeerId::try_from(r.u64()?).map_err(|_| WireError("peer"))?;
            let n = r.u64()?;
            if n > 64 {
                return Err(WireError("chunk count"));
            }
            let chunks = (0..n).map(|_| ivec(r)).collect::<Result<Vec<_>, _>>()?;
            Ok(Op::Repair { peer, chunks })
        }
        _ => Err(WireError("op tag")),
    }
}

/// A random edit inside the tank, a pure function of (seed, peer, tick).
pub fn random_edit(seed: u32, peer: PeerId, tick: u64) -> Option<Op> {
    let r = hash_u32(seed ^ hash_u32(u32::from(peer) << 24 ^ tick as u32));
    if !r.is_multiple_of(6) {
        return None;
    }
    let s = hash_u32(r);
    let at = IVec3::new(1 + (s % 62) as i32, 1 + ((s >> 8) % 24) as i32, 1 + ((s >> 16) % 62) as i32);
    let cell = match (s >> 24) % 5 {
        0 | 1 => Cell::filled(SAND),
        2 => Cell { volume: 8 + (r >> 8) as u8 % (VOLUME_FULL - 8), ..Cell::filled(WATER) },
        3 => Cell::filled(STONE),
        _ => Cell::EMPTY,
    };
    Some(Op::Edit { at, cell })
}

/// One whole chunk's cells, `CHUNK_CELLS` long, indexed x + 32(z + 32y).
pub type ChunkCells = Vec<Cell>;

fn flat(l: IVec3) -> usize {
    (l.x + 32 * (l.z + 32 * l.y)) as usize
}

pub struct World {
    pub store: BrickStore,
    pub digest: RegionDigest,
    rules: SimRules,
    seed: u32,
    /// Checkpoint tick to (digest, full state hash) after stepping it.
    pub checkpoints: BTreeMap<u64, (u64, u64)>,
    /// Chunk sums at the most recent checkpoints, for narrowing a mismatch.
    pub chunk_sums: BTreeMap<u64, Vec<(IVec3, u64)>>,
}

impl World {
    /// The same start on every peer: a stone tank holding a sand heap and a pool.
    pub fn new(seed: u32) -> Self {
        let mut store = BrickStore::new(4000);
        for y in 0..SIZE.y {
            for z in 0..SIZE.z {
                for x in 0..SIZE.x {
                    let wall = y == 0 || x == 0 || z == 0 || x == SIZE.x - 1 || z == SIZE.z - 1;
                    let cell = if wall && y < 28 {
                        Cell::filled(STONE)
                    } else if (20..30).contains(&x) && (20..30).contains(&z) && y < 12 {
                        Cell::filled(SAND)
                    } else if x > 40 && z > 40 && y < 6 {
                        Cell::filled(WATER)
                    } else {
                        continue;
                    };
                    let (chunk, local) = split_voxel(IVec3::new(x, y, z));
                    store.set(chunk, local, cell).expect("pool holds the region");
                }
            }
        }
        let digest = RegionDigest::of(&store);
        Self { store, digest, rules: rules(), seed, checkpoints: BTreeMap::new(), chunk_sums: BTreeMap::new() }
    }

    pub fn edit(&mut self, at: IVec3, cell: Cell) {
        let (chunk, local) = split_voxel(at);
        self.store.set(chunk, local, cell).expect("pool holds the region");
    }

    pub fn copy_chunk(&self, chunk: IVec3) -> ChunkCells {
        let mut cells = vec![Cell::EMPTY; CHUNK_CELLS];
        self.store.for_each_cell(chunk, |l, c| cells[flat(l)] = c);
        cells
    }

    pub fn replace_chunk(&mut self, chunk: IVec3, cells: &ChunkCells) {
        self.store.fill_chunk(chunk, |l| cells[flat(l)]).expect("pool holds the region");
        self.digest.mark_chunk(chunk);
    }

    /// Apply a frame's edits (repairs are the caller's), step its tick, record a checkpoint.
    pub fn run(&mut self, frame: &Frame<Op>, checkpoint: bool) {
        for input in &frame.inputs {
            if let Op::Edit { at, cell } = input.payload {
                self.edit(at, cell);
            }
        }
        step_tracked(&mut self.store, &mut self.digest, &self.rules, self.seed, frame.tick as u32);
        if checkpoint {
            self.digest.refresh(&self.store);
            self.checkpoints.insert(frame.tick, (self.digest.total(), self.store.state_hash()));
            self.chunk_sums.insert(frame.tick, self.digest.chunk_sums());
            while self.chunk_sums.len() > 8 {
                self.chunk_sums.pop_first();
            }
        }
    }
}
