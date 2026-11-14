use super::*;
use anyhow::{Result, ensure};
const LIMIT: u64 = 1 << 50;
fn put(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put64(out: &mut Vec<u8>, v: u64) {
    put(out, v as u32);
    put(out, (v >> 32) as u32);
}
fn read64(read: &mut impl FnMut() -> Result<u32>) -> Result<u64> {
    Ok(u64::from(read()?) | (u64::from(read()?) << 32))
}
fn mass(read: &mut impl FnMut() -> Result<u32>, species: usize) -> Result<Mass> {
    let mut m = [0; SPECIES];
    for v in &mut m[..species] {
        *v = read64(read)?;
        ensure!(*v <= LIMIT, "Industrial mass exceeds limit");
    }
    Ok(m)
}
fn coord(read: &mut impl FnMut() -> Result<u32>) -> Result<(i32, i32, i32)> {
    let p = (read()? as i32, read()? as i32, read()? as i32);
    ensure!(
        [p.0, p.1, p.2]
            .iter()
            .all(|v| v.unsigned_abs() <= 1_000_000),
        "Industrial coordinate out of range"
    );
    Ok(p)
}
impl State {
    pub fn encode(&self, out: &mut Vec<u8>) {
        self.encode_versioned(out, true);
    }

    pub fn encode_versioned(&self, out: &mut Vec<u8>, oxygen: bool) {
        let species = if oxygen { SPECIES } else { 5 };
        put64(out, self.tick);
        put64(out, self.target);
        put64(out, self.blocked);
        for m in [
            &self.transformed,
            &self.packed,
            &self.imported,
            &self.withdrawn,
        ] {
            for &v in &m[..species] {
                put64(out, v);
            }
        }
        for v in self.wind {
            put(out, v.to_bits());
        }
        let present = |m: &Mass| m[..species].iter().any(|&v| v > 0);
        let air = self.air.iter().filter(|(_, m)| present(m));
        put(out, air.clone().count() as u32);
        for (&k, m) in air {
            for v in [k.0, k.1, k.2, k.3] {
                put(out, v as u32);
            }
            for &v in &m[..species] {
                put64(out, v);
            }
        }
        let deposits = self.deposits.iter().filter(|(_, m)| present(m));
        put(out, deposits.clone().count() as u32);
        for (&k, m) in deposits {
            for v in [k.0, k.1, k.2] {
                put(out, v as u32);
            }
            for &v in &m[..species] {
                put64(out, v);
            }
        }
    }
    pub fn decode(read: &mut impl FnMut() -> Result<u32>) -> Result<Self> {
        Self::decode_versioned(read, true)
    }

    pub fn decode_versioned(read: &mut impl FnMut() -> Result<u32>, oxygen: bool) -> Result<Self> {
        let species = if oxygen { SPECIES } else { 5 };
        let mut state = Self::default();
        state.tick = read64(read)?;
        state.target = read64(read)?;
        state.blocked = read64(read)?;
        ensure!(state.target >= state.tick, "Industrial clock reversed");
        state.transformed = mass(read, species)?;
        state.packed = mass(read, species)?;
        state.imported = mass(read, species)?;
        state.withdrawn = mass(read, species)?;
        for v in &mut state.wind {
            *v = f32::from_bits(read()?);
            ensure!(v.is_finite() && v.abs() <= 2.0, "Invalid industrial wind");
        }
        let count = read()? as usize;
        ensure!(count <= MAX_CELLS, "Industrial air exceeds cell limit");
        for _ in 0..count {
            let p = coord(read)?;
            let size = read()? as i32;
            ensure!(
                matches!(size, 1 | 2 | 4)
                    && [p.0, p.1, p.2].iter().all(|v| v.rem_euclid(size) == 0),
                "Invalid industrial cell span"
            );
            let k = (p.0, p.1, p.2, size);
            let m = mass(read, species)?;
            ensure!(
                total(&m) > 0 && state.air.insert(k, m).is_none(),
                "Duplicate or empty industrial air cell"
            );
        }
        let count = read()? as usize;
        ensure!(count <= MAX_DEPOSITS, "Industrial deposition exceeds limit");
        for _ in 0..count {
            let p = coord(read)?;
            let m = mass(read, species)?;
            ensure!(
                total(&m) > 0 && state.deposits.insert(p, m).is_none(),
                "Duplicate or empty industrial deposit"
            );
        }
        Ok(state)
    }
}
