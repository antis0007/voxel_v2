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
            // Only wildlife records used 26 for salamanders. Combat records keep
            // 26 as Spectator; never globally reinterpret that identity.
            let stored_kind = read()?;
            let kind = if stored_kind == 26 { SALAMANDER } else { stored_kind };
            if bestiary::generated(kind) && !passive(kind) {
                // The species row was removed after this save; forget the animal.
                for _ in 0..8 { read()?; }
                continue;
            }
            ensure!(passive(kind) && (if kind == SALAMANDER { (2..4).contains(&key.2) } else { key.2 < 2 }) && key.0.unsigned_abs() < 1_000_000
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

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: u32) -> Vec<u32> {
        let mut state = State::default();
        let p = Vec3::new(4.0, 81.0, 4.0);
        state.residents.insert((0, 0, 0), Resident { kind, home: p, pos: p, health: 1.0, yaw: 0.0 });
        let mut bytes = Vec::new();
        state.encode(&[], &mut bytes);
        bytes.chunks_exact(4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]])).collect()
    }

    fn decode(words: Vec<u32>) -> Result<State> {
        let mut words = words.into_iter();
        State::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("short")))
    }

    #[test]
    fn a_removed_generated_species_drops_its_residents_and_the_save_loads() {
        let gone = bestiary::FIRST_GENERATED + 999;
        assert!(bestiary::species(gone).is_none());
        let state = decode(record(gone)).expect("the save still loads");
        assert!(state.residents.is_empty());
        assert!(decode(record(77)).is_err(), "an unknown authored kind is still corrupt data");
        assert_eq!(decode(record(SHEEP)).expect("sheep").residents.len(), 1);
    }
}
