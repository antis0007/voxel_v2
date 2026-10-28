//! Checked, bounded wildlife records appended by save version 37.
use super::*;
use anyhow::{Result, ensure};

impl State {
    pub(crate) fn encode(&self, actors: &[Enemy], bytes: &mut Vec<u8>) {
        let mut put = |word: u32| bytes.extend_from_slice(&word.to_le_bytes());
        put(self.residents.len() as u32);
        for (&key, r) in &self.residents {
            let live = actors.iter().find(|e| passive(e.kind) && id(e) == key);
            let pos = live.map_or(r.pos, |e| e.pos);
            let health = live.map_or(r.health, |e| e.health.max(0.0));
            let yaw = live.map_or(r.yaw, |e| e.yaw);
            for word in [key.0 as u32, key.1 as u32, key.2, r.kind] { put(word); }
            for v in r.home.to_array().into_iter().chain(pos.to_array()).chain([health, yaw]) { put(v.to_bits()); }
        }
    }
    pub(crate) fn decode(read: &mut impl FnMut() -> Result<u32>) -> Result<Self> {
        let count = read()? as usize;
        ensure!(count <= RECORD_LIMIT, "Too many wildlife records");
        let mut state = State::default();
        for _ in 0..count {
            let key = (read()? as i32, read()? as i32, read()?);
            let kind = read()?;
            ensure!(passive(kind) && key.2 < 2 && key.0.unsigned_abs() < 1_000_000
                && key.1.unsigned_abs() < 1_000_000, "Invalid wildlife identity");
            let home = Vec3::new(f32::from_bits(read()?), f32::from_bits(read()?), f32::from_bits(read()?));
            let pos = Vec3::new(f32::from_bits(read()?), f32::from_bits(read()?), f32::from_bits(read()?));
            let health = f32::from_bits(read()?);
            let yaw = f32::from_bits(read()?);
            ensure!(home.is_finite() && pos.is_finite() && home.abs().max_element() < 50_000_000.0
                && pos.abs().max_element() < 50_000_000.0 && health.is_finite()
                && (0.0..=super::super::enemy_health(kind)).contains(&health) && yaw.is_finite(),
                "Invalid wildlife state");
            ensure!(state.residents.insert(key, Resident { kind, home, pos, health, yaw }).is_none(),
                "Duplicate wildlife identity");
        }
        Ok(state)
    }
}
