//! Finite free-water effects shared by machines and spells.
use super::*;

#[derive(Clone, Copy)]
pub enum WaterDirection {
    IntoBuffer,
    IntoWorld,
}

/// Exact portable buffer exchange, quantized only for legacy cell storage.
pub fn exchange_water_ul(world: &mut World, p: IVec3, buffer: &mut u32,
    capacity: u32, direction: WaterDirection) -> Result<Commit<u64>, Failure> {
    admit(world, p)?;
    if *buffer > capacity || !matches!(world.get(p), BlockId::Air | BlockId::Water) {
        return Err(Failure::Rejected);
    }
    let before = world.water_microlitres(p);
    let exact = environment::State::enabled(world.generator_version);
    let cell_capacity = if exact { world.environment.water_capacity_ul(p) }
        else { engine_core::environment::cell_volume(world.voxel_scale) };
    let mut moved = match direction {
        WaterDirection::IntoBuffer => before.min(u64::from(capacity - *buffer)),
        WaterDirection::IntoWorld => cell_capacity.saturating_sub(before).min(u64::from(*buffer)),
    };
    if !exact {
        let quantum = engine_core::environment::cell_volume(world.voxel_scale) / 8;
        moved -= moved % quantum;
    }
    if moved == 0 { return Err(Failure::NoChange); }
    let after = match direction { WaterDirection::IntoBuffer => before - moved,
        WaterDirection::IntoWorld => before + moved };
    if !commit_water(world, &[(p, after)]) { return Err(Failure::Deferred); }
    match direction { WaterDirection::IntoBuffer => *buffer += moved as u32,
        WaterDirection::IntoWorld => *buffer -= moved as u32 }
    Ok(Commit { position: p, revision: world.revision(), value: moved })
}

/// Transfer up to `limit` machine units. The exclusively borrowed buffer and
/// cell commit synchronously; a failed admission changes neither quantity.
/// Legacy worlds move whole cell packets, exact worlds preserve subpacket water.
/// This transfers free water only; sediment remains in the intake cell.
pub fn exchange_water(
    world: &mut World,
    p: IVec3,
    buffer: &mut u32,
    capacity: u32,
    limit: u32,
    direction: WaterDirection,
) -> Result<Commit<u32>, Failure> {
    admit(world, p)?;
    let (amount, after) = plan(world, p, *buffer, capacity, limit, direction)?;
    if !commit_water(world, &[(p, after)]) {
        return Err(Failure::Deferred);
    }
    match direction {
        WaterDirection::IntoBuffer => *buffer += amount,
        WaterDirection::IntoWorld => *buffer -= amount,
    }
    Ok(Commit {
        position: p,
        revision: world.revision(),
        value: amount,
    })
}

fn plan(
    world: &World,
    p: IVec3,
    buffer: u32,
    capacity: u32,
    limit: u32,
    direction: WaterDirection,
) -> Result<(u32, u64), Failure> {
    if buffer > capacity || !matches!(world.get(p), BlockId::Air | BlockId::Water) {
        return Err(Failure::Rejected);
    }
    let exact = environment::State::enabled(world.generator_version);
    let before = world.water_microlitres(p);
    let cell_capacity = if exact {
        world.environment.water_capacity_ul(p)
    } else {
        engine_core::environment::cell_volume(world.voxel_scale)
    };
    let available = match direction {
        WaterDirection::IntoBuffer => (before / WATER_UNIT_UL).min(u64::from(capacity - buffer)),
        WaterDirection::IntoWorld => {
            ((cell_capacity.saturating_sub(before)) / WATER_UNIT_UL).min(u64::from(buffer))
        }
    };
    let mut amount = available.min(u64::from(limit)) as u32;
    let packet =
        (engine_core::environment::cell_volume(world.voxel_scale) / 8 / WATER_UNIT_UL) as u32;
    if !exact {
        amount -= amount % packet;
    }
    if amount == 0 {
        return Err(Failure::NoChange);
    }
    let delta = u64::from(amount) * WATER_UNIT_UL;
    let after = match direction {
        WaterDirection::IntoBuffer => before - delta,
        WaterDirection::IntoWorld => before + delta,
    };
    Ok((amount, after))
}

/// Admits both environmental footprints and encodings before touching either
/// owner. Free water moves; sediment stays in its cell, as with pump extraction.
/// The caller has admitted resident geometry and the aggregate cellular wake.
fn commit_water(world: &mut World, amounts: &[(IVec3, u64)]) -> bool {
    let exact = environment::State::enabled(world.generator_version);
    let mut wake = Vec::with_capacity(8);
    for &(p, _) in amounts {
        let impact = world.fluid_impact(p, p);
        for &q in impact.positions() {
            if !wake.contains(&q) {
                wake.push(q);
            }
        }
    }
    if exact && (!world.environment.can_encode(amounts) || !world.environment.can_wake(&wake)) {
        return false;
    }
    let quantum = engine_core::environment::cell_volume(world.voxel_scale) / 8;
    for &(p, amount) in amounts {
        let coord = world.commit_resident_water_cell(p, amount.div_ceil(quantum) as u8);
        if exact {
            world.environment.encode_water(p, amount);
        }
        world.wake_acting(p);
        world.invalidate_snapshot_at(coord);
    }
    if exact {
        for p in wake {
            world.environment.wake(p);
        }
    }
    world.revision = world.revision.wrapping_add(1);
    true
}

#[must_use = "Publish both ends of a committed water transfer"]
pub struct Transfer {
    source: Commit<()>,
    destination: Commit<u32>,
}

impl Transfer {
    pub fn publish_voxels(self, world: &World, cells: &mut Vec<IVec3>) -> u32 {
        self.source.publish_voxel(world, cells);
        self.destination.publish_voxel(world, cells)
    }
}

/// Atomic world-to-world use of the same finite buffer exchange as a pump.
/// The temporary buffer is a planning value only; no water leaves world storage
/// before both endpoints pass admission. A full destination changes nothing.
pub fn transfer_water(
    world: &mut World,
    from: IVec3,
    to: IVec3,
    limit: u32,
) -> Result<Transfer, Failure> {
    if from == to {
        return Err(Failure::NoChange);
    }
    admit_cells(world, &[from, to])?;
    let (available, _) = plan(world, from, 0, limit, limit, WaterDirection::IntoBuffer)?;
    let (amount, target) = plan(
        world,
        to,
        available,
        limit,
        limit,
        WaterDirection::IntoWorld,
    )?;
    let (_, source) = plan(world, from, 0, amount, amount, WaterDirection::IntoBuffer)?;
    if !commit_water(world, &[(from, source), (to, target)]) {
        return Err(Failure::Deferred);
    }
    let revision = world.revision();
    Ok(Transfer {
        source: Commit {
            position: from,
            revision,
            value: (),
        },
        destination: Commit {
            position: to,
            revision,
            value: amount,
        },
    })
}
