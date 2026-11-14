//! Exact checkpoints for the sealed development region, independent of world saves.
pub(super) mod codec;
mod file;
mod ingress;
mod ownership;
use super::{CELLS, Region, Snapshot, backend};
use crate::world::{World, WorldType};
use codec::{Reader, cell, hash, number, position};
use engine_core::time::PLAYER_STEPS_PER_CA;
use engine_sim::{Cell, StepStats};
use glam::IVec3;
use std::{collections::VecDeque, path::Path, sync::Arc};

const MAGIC: &[u8; 8] = b"VXCA0004";

fn model_hash() -> u64 {
    let mut bytes = format!("{:?}", &*engine_content::SIM_RULES).into_bytes();
    // Source changes reject incompatible continuations even without a format change.
    for source in [
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim/src/kernel.rs")).as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim/src/rules.rs")).as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim/src/react.rs")).as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim/src/pinned.rs")).as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sim/src/anchors.rs"
        ))
        .as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim/src/cell.rs")).as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sim/src/offload.rs"
        ))
        .as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim/src/store.rs")).as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sim/src/store/access.rs"
        ))
        .as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sim/src/store/activity.rs"
        ))
        .as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../core/src/hash.rs")).as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../core/src/material.rs"
        ))
        .as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../core/src/reaction.rs"
        ))
        .as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../core/src/time.rs")).as_slice(),
        include_bytes!("backend.rs").as_slice(),
        include_bytes!("ingress.rs").as_slice(),
        include_bytes!("ownership.rs").as_slice(),
        include_bytes!("backend/gpu.rs").as_slice(),
        engine_sim_gpu::RULE_WGSL.as_bytes(),
        engine_sim_gpu::resident::RESIDENT_WGSL.as_bytes(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim_gpu/src/resident.rs")).as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim_gpu/src/resident_async.rs")).as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim_gpu/src/resident_edits.rs")).as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim_gpu/src/resident_readback.rs")).as_slice(),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sim_gpu/src/lib.rs")).as_slice(),
        include_bytes!("../cpu_region.rs").as_slice(),
    ] {
        bytes.extend_from_slice(source);
    }
    hash(&bytes)
}

fn identity(world: &World) -> [u64; 4] {
    let cfg = world.sample_config();
    [
        u64::from(cfg.seed),
        u64::from(cfg.generator_version),
        u64::from(cfg.voxel_meters().to_bits()),
        u64::from(cfg.world_type.code()),
    ]
}

fn encode_snapshot(out: &mut Vec<u8>, snapshot: &Snapshot) {
    for value in [
        snapshot.tick,
        snapshot.applied_sequence,
        snapshot.hash,
        u64::from(snapshot.stats.active_bricks),
        u64::from(snapshot.stats.blocks),
        u64::from(snapshot.stats.changed_blocks),
        u64::from(snapshot.stats.stalled_blocks),
    ] {
        number(out, value);
    }
    for value in snapshot.cells.iter() {
        cell(out, *value);
    }
}

fn decode_snapshot(input: &mut Reader<'_>, origin: IVec3) -> Result<Snapshot, String> {
    let tick = input.number()?;
    let applied_sequence = input.number()?;
    let hash = input.number()?;
    let mut stats = [0; 4];
    for value in &mut stats {
        *value = u32::try_from(input.number()?).map_err(|_| "invalid CPU snapshot counters")?;
    }
    let mut cells = Box::new([Cell::EMPTY; CELLS]);
    for cell in cells.iter_mut() {
        *cell = input.cell()?;
        backend::validate(*cell)?;
    }
    Ok(Snapshot {
        origin,
        tick,
        applied_sequence,
        hash,
        cells,
        stats: StepStats {
            active_bricks: stats[0],
            blocks: stats[1],
            changed_blocks: stats[2],
            stalled_blocks: stats[3],
        },
    })
}

impl Region {
    pub fn save_checkpoint(
        &self,
        world: &World,
        player_tick: u64,
        path: &Path,
    ) -> Result<(), String> {
        if self.inventory_bound {
            return Err("inventory-owned CA edits require an exact application session checkpoint".into());
        }
        file::publish(path, &self.encode_checkpoint(world, player_tick)?)
    }

    pub(crate) fn encode_checkpoint(&self, world: &World, player_tick: u64) -> Result<Vec<u8>, String> {
        if player_tick / PLAYER_STEPS_PER_CA != self.completed {
            return Err("CPU checkpoint must follow a completed player step".into());
        }
        let mut bytes = Vec::with_capacity(128 * 1024);
        bytes.extend_from_slice(MAGIC);
        number(&mut bytes, model_hash());
        for value in identity(world) {
            number(&mut bytes, value);
        }
        position(&mut bytes, self.published.origin);
        for value in [player_tick, self.completed, self.next_sequence] {
            number(&mut bytes, value);
        }
        number(&mut bytes, self.results.len() as u64);
        for snapshot in &self.results {
            encode_snapshot(&mut bytes, snapshot);
        }
        ingress::encode(self, &mut bytes);
        ownership::encode(self, &mut bytes);
        number(&mut bytes, self.tool_slot as u64);
        number(&mut bytes, self.tool_after);
        self.backend.encode(&mut bytes);
        let checksum = hash(&bytes);
        number(&mut bytes, checksum);
        Ok(bytes)
    }

    pub fn load_checkpoint(
        world: &World,
        origin: IVec3,
        path: &Path,
    ) -> Result<(Self, u64), String> {
        let bytes = file::read(path)?;
        let restored = Self::decode_checkpoint(world, origin, &bytes)?;
        if restored.0.inventory_bound {
            return Err("inventory-owned CA edits must be restored with their application session".into());
        }
        Ok(restored)
    }

    pub(crate) fn decode_checkpoint(world: &World, origin: IVec3, bytes: &[u8]) -> Result<(Self, u64), String> {
        backend::validate_origin(origin)?;
        if bytes.len() > 160 * 1024 { return Err("CPU checkpoint exceeds size limit".into()); }
        let end = bytes
            .len()
            .checked_sub(8)
            .ok_or("truncated CPU checkpoint")?;
        let expected = Reader(&bytes[end..]).number()?;
        if hash(&bytes[..end]) != expected {
            return Err("CPU checkpoint checksum mismatch".into());
        }
        let mut input = Reader(&bytes[..end]);
        if &input.bytes::<8>()? != MAGIC {
            return Err("unsupported CPU checkpoint format".into());
        }
        if input.number()? != model_hash() {
            return Err("CPU checkpoint simulation model changed".into());
        }
        for expected in identity(world) {
            if input.number()? != expected {
                return Err("CPU checkpoint world configuration mismatch".into());
            }
        }
        if input.position()? != origin {
            return Err("CPU checkpoint origin mismatch".into());
        }
        let player_tick = input.number()?;
        let completed = input.number()?;
        let next_sequence = input.number()?;
        if player_tick / PLAYER_STEPS_PER_CA != completed
            || completed > u64::from(u32::MAX) + 1
            || next_sequence == 0
        {
            return Err("invalid CPU checkpoint clock or sequence".into());
        }
        let due = completed.saturating_sub(2);
        let count = input.count(3)?;
        if count as u64 != completed - due + 1 {
            return Err("missing delayed CPU snapshots".into());
        }
        let mut results = VecDeque::with_capacity(3);
        let mut applied = 0;
        for i in 0..count {
            let snapshot = decode_snapshot(&mut input, origin)?;
            if snapshot.tick != due + i as u64
                || snapshot.applied_sequence < applied
                || snapshot.applied_sequence >= next_sequence
                || (snapshot.tick == 0 && snapshot.applied_sequence != 0)
            {
                return Err("invalid CPU snapshot timeline".into());
            }
            applied = snapshot.applied_sequence;
            results.push_back(Arc::new(snapshot));
        }
        let published = results
            .front()
            .cloned()
            .ok_or("missing published CPU snapshot")?;
        let (pending, outcomes) = ingress::decode(
            &mut input,
            &published,
            applied,
            next_sequence,
            player_tick,
            completed,
        )?;
        let (inventory_bound, owned) = ownership::decode(&mut input, &pending, &outcomes)?;
        let tool_slot = input.count(crate::world::inventory::HOTBAR_SLOTS - 1)?;
        let tool_after = input.number()?;
        if tool_after > player_tick.saturating_add(12) {
            return Err("invalid cellular tool deadline".into());
        }
        let backend = backend::Cpu::decode(&mut input, origin, world.seed, completed, applied)?;
        let latest = results.back().ok_or("missing authoritative CPU snapshot")?;
        if !input.0.is_empty() || !backend.matches(latest) {
            return Err("CPU checkpoint authoritative state mismatch".into());
        }
        Ok((
            Self {
                backend,
                gpu: None,
                pending,
                results,
                published,
                completed,
                next_sequence,
                outcomes,
                owned,
                inventory_bound,
                tool_slot,
                tool_after,
            },
            player_tick,
        ))
    }
}
