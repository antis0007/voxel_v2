//! Stable, bounded Undercrown object state.
//!
//! State records history and knowledge. It never grants a route by itself:
//! geometry and mechanisms remain authoritative for what is actually usable.
use anyhow::{Context, ensure};
use std::collections::BTreeMap;

pub const MAX_RECORDS: usize = 2048;
pub const KNOWN_FLAGS: u32 = 0x00FF;

pub mod flags {
    pub const DISCOVERED: u32 = 1 << 0;
    pub const OPERATED: u32 = 1 << 1;
    pub const REPAIRED: u32 = 1 << 2;
    pub const OPEN: u32 = 1 << 3;
    pub const RESTORED: u32 = 1 << 4;
    pub const CLAIMED: u32 = 1 << 5;
    pub const KNOWN: u32 = 1 << 6;
    pub const FAILED: u32 = 1 << 7;
}

const DISTRICT_BITS: u32 = 4;
const ROOM_BITS: u32 = 10;
const PROP_BITS: u32 = 8;
const DISTRICT_MASK: u32 = (1 << DISTRICT_BITS) - 1;
const ROOM_MASK: u32 = (1 << ROOM_BITS) - 1;
const PROP_MASK: u32 = (1 << PROP_BITS) - 1;
const MAX_RAW: u32 = 1 << (DISTRICT_BITS + ROOM_BITS + PROP_BITS);

/// Permanent identity for one authored object. Never derive this from array
/// order or coordinates; reordering content must not renumber saves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(u32);

impl ObjectId {
    pub fn new(district: u8, room: u16, prop: u8) -> Option<Self> {
        if u32::from(district) > DISTRICT_MASK
            || u32::from(room) > ROOM_MASK
            || u32::from(prop) > PROP_MASK
        {
            return None;
        }
        Some(Self(
            u32::from(district) | (u32::from(room) << DISTRICT_BITS) | (u32::from(prop) << 14),
        ))
    }

    pub fn from_raw(raw: u32) -> Option<Self> {
        (raw < MAX_RAW).then_some(Self(raw))
    }

    pub fn raw(self) -> u32 {
        self.0
    }

    pub fn district(self) -> u8 {
        (self.0 & DISTRICT_MASK) as u8
    }

    pub fn room(self) -> u16 {
        ((self.0 >> DISTRICT_BITS) & ROOM_MASK) as u16
    }

    pub fn prop(self) -> u8 {
        ((self.0 >> 14) & PROP_MASK) as u8
    }
}

#[derive(Clone, Debug, Default)]
pub struct State {
    records: BTreeMap<ObjectId, u32>,
    /// Runtime-only interaction debounce. Never serialized.
    pub cooldown: f32,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn get(&self, id: ObjectId) -> u32 {
        self.records.get(&id).copied().unwrap_or(0)
    }

    pub fn has(&self, id: ObjectId, flag: u32) -> bool {
        self.get(id) & flag != 0
    }

    /// ORs known flags into a record. Returns false at capacity, leaving every
    /// existing record untouched.
    pub fn set(&mut self, id: ObjectId, flags: u32) -> bool {
        if self.records.len() >= MAX_RECORDS && !self.records.contains_key(&id) {
            return false;
        }
        *self.records.entry(id).or_insert(0) |= flags & KNOWN_FLAGS;
        true
    }

    pub fn clear(&mut self, id: ObjectId, flags: u32) {
        if let Some(record) = self.records.get_mut(&id) {
            *record &= !(flags & KNOWN_FLAGS);
            if *record == 0 {
                self.records.remove(&id);
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.cooldown = (self.cooldown - dt.max(0.0)).max(0.0);
    }

    pub fn encode(&self, bytes: &mut Vec<u8>) {
        let mut put = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        let count = self.records.len().min(MAX_RECORDS);
        put(count as u32);
        for (id, flags) in self.records.iter().take(count) {
            put(id.raw());
            put(*flags & KNOWN_FLAGS);
        }
    }

    pub fn decode(read: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<Self> {
        let count = read()? as usize;
        ensure!(count <= MAX_RECORDS, "Too many Undercrown records");
        let mut records = BTreeMap::new();
        let mut previous = None;
        for _ in 0..count {
            let raw = read()?;
            let id = ObjectId::from_raw(raw).context("Invalid Undercrown object id")?;
            ensure!(
                previous.is_none_or(|p| id > p),
                "Duplicate or unsorted Undercrown record"
            );
            previous = Some(id);
            let flags = read()?;
            ensure!(flags & !KNOWN_FLAGS == 0, "Unknown Undercrown flags");
            ensure!(
                records.insert(id, flags).is_none(),
                "Duplicate Undercrown record"
            );
        }
        Ok(Self {
            records,
            cooldown: 0.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flags::*;

    fn encode(state: &State) -> Vec<u8> {
        let mut bytes = Vec::new();
        state.encode(&mut bytes);
        bytes
    }

    fn decode(bytes: &[u8]) -> anyhow::Result<State> {
        let mut offset = 0;
        State::decode(&mut || {
            let end = offset + 4;
            let value = bytes.get(offset..end).context("truncated test state")?;
            offset = end;
            Ok(u32::from_le_bytes(value.try_into()?))
        })
    }

    #[test]
    fn round_trip_is_sorted_and_stable() {
        let mut state = State::default();
        let a = ObjectId::new(1, 2, 3).unwrap();
        let b = ObjectId::new(7, 1023, 255).unwrap();
        assert!(state.set(b, OPEN | REPAIRED));
        assert!(state.set(a, DISCOVERED));
        let bytes = encode(&state);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.get(a), DISCOVERED);
        assert_eq!(decoded.get(b), OPEN | REPAIRED);
        assert_eq!(encode(&decoded), bytes);
    }

    #[test]
    fn ids_round_trip_and_reject_out_of_range() {
        let id = ObjectId::new(15, 1023, 255).unwrap();
        assert_eq!(ObjectId::from_raw(id.raw()), Some(id));
        assert_eq!(id.district(), 15);
        assert_eq!(id.room(), 1023);
        assert_eq!(id.prop(), 255);
        assert!(ObjectId::new(16, 0, 0).is_none());
        assert!(ObjectId::new(0, 1024, 0).is_none());
        assert!(ObjectId::from_raw(MAX_RAW).is_none());
    }

    #[test]
    fn set_masks_unknown_bits_and_respects_capacity() {
        let id = ObjectId::new(0, 0, 1).unwrap();
        let mut state = State::default();
        assert!(state.set(id, 0xFFFF_FFFF));
        assert_eq!(state.get(id), KNOWN_FLAGS);
        for raw in 0..MAX_RECORDS as u32 {
            let id = ObjectId::from_raw(raw).unwrap();
            state.set(id, DISCOVERED);
        }
        let overflow = ObjectId::new(15, 1023, 255).unwrap();
        assert_eq!(state.len(), MAX_RECORDS);
        assert!(!state.set(overflow, DISCOVERED));
        assert_eq!(state.len(), MAX_RECORDS);
    }

    #[test]
    fn clear_removes_empty_records() {
        let id = ObjectId::new(0, 0, 1).unwrap();
        let mut state = State::default();
        state.set(id, OPEN);
        state.clear(id, OPEN);
        assert_eq!(state.get(id), 0);
        assert!(state.is_empty());
    }

    #[test]
    fn decode_rejects_bad_payloads() {
        let id = ObjectId::new(0, 0, 1).unwrap();
        let mut duplicate = Vec::new();
        for _ in 0..2 {
            duplicate.extend_from_slice(&id.raw().to_le_bytes());
            duplicate.extend_from_slice(&DISCOVERED.to_le_bytes());
        }
        let mut bytes = (2u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(&duplicate);
        assert!(decode(&bytes).is_err());

        let mut unknown = Vec::new();
        unknown.extend_from_slice(&id.raw().to_le_bytes());
        unknown.extend_from_slice(&(1u32 << 31).to_le_bytes());
        let mut bytes = (1u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(&unknown);
        assert!(decode(&bytes).is_err());

        let bytes = (MAX_RECORDS as u32 + 1).to_le_bytes().to_vec();
        assert!(decode(&bytes).is_err());
    }
}
