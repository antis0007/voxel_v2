//! Sparse atmospheric changes, frozen with each shared factory image.
use super::{wire, words};
use crate::world::industry::{Mass, State, MAX_CELLS, MAX_DEPOSITS};
use anyhow::{Result, ensure};
use engine_netcode::wire::{Reader, put_u64};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Default)]
pub(super) struct Image {
    state: State,
}
impl Image {
    pub fn capture(state: &State) -> Self {
        let mut copy = metadata(state);
        copy.air.extend(state.air.iter().filter(|(_, m)| present(m)).map(|(&k, &m)| (k, m)));
        copy.deposits.extend(state.deposits.iter().filter(|(_, m)| present(m)).map(|(&k, &m)| (k, m)));
        Self { state: copy }
    }
}
#[derive(Default)]
pub(super) struct Sent {
    previous: Option<Arc<Image>>,
}
impl Sent {
    pub fn write(&mut self, image: &Arc<Image>) -> Option<Vec<u8>> {
        if self.previous.as_ref().is_some_and(|p| Arc::ptr_eq(p, image)) { return None; }
        let full = self.previous.is_none();
        let current = &image.state;
        let mut next = metadata(current);
        let (air_clear, deposit_clear) = if let Some(old) = &self.previous {
            let (set, clear) = changes(&current.air, &old.state.air);
            next.air = set;
            let (set, deposits) = changes(&current.deposits, &old.state.deposits);
            next.deposits = set;
            (clear, deposits)
        } else {
            next.air.clone_from(&current.air);
            next.deposits.clone_from(&current.deposits);
            (Vec::new(), Vec::new())
        };
        let mut data = Vec::new();
        next.encode(&mut data);
        let mut out = vec![u8::from(full)];
        put_u64(&mut out, data.len() as u64);
        out.extend_from_slice(&data);
        put_u64(&mut out, air_clear.len() as u64);
        for (x, y, z, size) in air_clear {
            for v in [x, y, z, size] { out.extend_from_slice(&v.to_le_bytes()); }
        }
        put_u64(&mut out, deposit_clear.len() as u64);
        for (x, y, z) in deposit_clear {
            for v in [x, y, z] { out.extend_from_slice(&v.to_le_bytes()); }
        }
        let full_len = data.len() + (current.air.len() - next.air.len()) * 64
            + (current.deposits.len() - next.deposits.len()) * 60;
        let mut length = Vec::new();
        put_u64(&mut length, full_len as u64);
        if !full && out.len() > full_len + length.len() + 3 {
            data.clear();
            current.encode(&mut data);
            out.clear();
            out.push(1);
            put_u64(&mut out, data.len() as u64);
            out.extend_from_slice(&data);
            put_u64(&mut out, 0);
            put_u64(&mut out, 0);
        }
        self.previous = Some(Arc::clone(image));
        Some(out)
    }
}

fn present(m: &Mass) -> bool { m.iter().any(|&v| v != 0) }
fn metadata(state: &State) -> State {
    let mut copy = State::default();
    copy.tick = state.tick;
    copy.target = state.target;
    copy.transformed = state.transformed;
    copy.packed = state.packed;
    copy.imported = state.imported;
    copy.withdrawn = state.withdrawn;
    copy.blocked = state.blocked;
    copy.wind = state.wind;
    copy
}

fn changes<K: Copy + Ord>(current: &BTreeMap<K, Mass>, old: &BTreeMap<K, Mass>) -> (BTreeMap<K, Mass>, Vec<K>) {
    let mut before = old.iter().peekable();
    let (mut set, mut clear) = (BTreeMap::new(), Vec::new());
    for (&key, &mass) in current {
        while before.peek().is_some_and(|(k, _)| **k < key) {
            if let Some((&k, _)) = before.next() { clear.push(k); }
        }
        if before.peek().is_some_and(|(k, _)| **k == key) {
            if let Some((_, m)) = before.next() && *m != mass { set.insert(key, mass); }
        } else { set.insert(key, mass); }
    }
    clear.extend(before.map(|(&k, _)| k));
    (set, clear)
}

pub(super) fn read(bytes: &[u8], previous: &State) -> Result<State> {
    ensure!(bytes.len() <= 2 * 1024 * 1024, "industrial replica exceeds byte budget");
    let mut r = Reader::new(bytes);
    let full = match wire(r.u8())? { 0 => false, 1 => true, _ => anyhow::bail!("invalid industrial replica kind") };
    let len = usize::try_from(wire(r.u64())?)?;
    let mut next = words(wire(r.take(len))?, |r| State::decode(&mut || r()))?;
    let air = removals::<4>(&mut r, MAX_CELLS, full)?;
    let deposits = removals::<3>(&mut r, MAX_DEPOSITS, full)?;
    ensure!(r.done(), "trailing industrial replica data");
    for &[x, y, z, size] in &air {
        ensure!(matches!(size, 1 | 2 | 4) && [x, y, z].iter().all(|v| v.rem_euclid(size) == 0), "invalid industrial clear span");
        ensure!(!next.air.contains_key(&(x, y, z, size)), "industrial cell set and cleared");
    }
    for &[x, y, z] in &deposits {
        ensure!(!next.deposits.contains_key(&(x, y, z)), "industrial deposit set and cleared");
    }
    if !full {
        let mut current = previous.air.clone();
        for [x, y, z, size] in air { current.remove(&(x, y, z, size)); }
        current.append(&mut next.air);
        ensure!(current.len() <= MAX_CELLS, "industrial replica exceeds air cell limit");
        next.air = current;
        let mut current = previous.deposits.clone();
        for [x, y, z] in deposits { current.remove(&(x, y, z)); }
        current.append(&mut next.deposits);
        ensure!(current.len() <= MAX_DEPOSITS, "industrial replica exceeds deposit limit");
        next.deposits = current;
    }
    Ok(next)
}

fn removals<const N: usize>(r: &mut Reader<'_>, limit: usize, full: bool) -> Result<Vec<[i32; N]>> {
    let count = usize::try_from(wire(r.u64())?)?;
    ensure!(count <= limit && count <= r.remaining() / (N * 4) && (!full || count == 0), "invalid industrial clear count");
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut key = [0; N];
        for (axis, v) in key.iter_mut().enumerate() {
            *v = i32::from_le_bytes(wire(r.take(4))?.try_into()?);
            ensure!(axis >= 3 || v.unsigned_abs() <= 1_000_000, "industrial clear coordinate out of range");
        }
        ensure!(out.last().is_none_or(|old| old < &key), "industrial clear list out of order");
        out.push(key);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
