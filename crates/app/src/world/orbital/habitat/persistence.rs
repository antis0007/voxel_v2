use super::*;
use anyhow::{Result, ensure};

pub fn encode(state: &State, out: &mut Vec<u8>) {
    // The snapshot preflight validates serialization before this footer is used.
    let bytes = ron::to_string(state).unwrap_or_default().into_bytes();
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    for chunk in bytes.chunks(4) {
        let mut word = [0; 4]; word[..chunk.len()].copy_from_slice(chunk);
        out.extend_from_slice(&word);
    }
}

pub fn decode(read: &mut impl FnMut() -> Result<u32>) -> Result<State> {
    let len = read()? as usize;
    ensure!(len <= 16 * 1024 * 1024, "Habitat state oversized");
    let mut bytes = Vec::with_capacity(len + 3);
    for _ in 0..len.div_ceil(4) { bytes.extend_from_slice(&read()?.to_le_bytes()); }
    bytes.truncate(len);
    let state: State = ron::de::from_bytes(&bytes)?;
    validate(&state)?;
    Ok(state)
}

pub fn validate(state: &State) -> Result<()> {
    super::super::cargo::validate(state)?;
    ensure!(state.habitats.len() <= 16, "Too many habitats");
    ensure!(ron::to_string(state)?.len() <= 16 * 1024 * 1024, "Habitat snapshot oversized");
    let mut cells = BTreeSet::new();
    for h in &state.habitats {
        ensure!(cells.insert((h.cell.x, h.cell.y)), "Duplicate habitat");
        ensure!(h.origin.abs().max_element() < 900_000 && h.origin.y > 512,
            "Invalid habitat origin");
        ensure!(h.berth.is_finite() && h.berth.abs().max_element() < 900_000.0,
            "Invalid habitat berth");
        ensure!((-1..=1).contains(&h.cycle), "Invalid airlock cycle");
        // Quarter metre is the largest supported construction cursor domain.
        ensure!(h.grid.valid(), "Invalid habitat grid");
        ensure!(h.residents.len() <= residents::MAX && h.residents.iter().all(|r| r.pos.is_finite()
            && r.pos.abs().max_element() < 900_000.0 && r.health.is_none_or(|v| v.is_finite() && v > 0.0 && v < 1e6)),
            "Invalid habitat residents");
        ensure!(h.build_cursor <= h.grid.cells() * 64, "Invalid habitat construction cursor");
        ensure!((!h.ready && h.gas.is_empty()) || h.gas.len() == h.grid.cells(),
            "Invalid habitat gas grid");
        ensure!(h.gas.iter().flatten().all(|q| q.is_finite() && *q >= -1.0 && *q < 1e12)
            && h.tank.into_iter().chain(h.cabin).chain(h.escaped).all(|q| q.is_finite() && q >= 0.0 && q < 1e15),
            "Invalid habitat quantities");
    }
    Ok(())
}
