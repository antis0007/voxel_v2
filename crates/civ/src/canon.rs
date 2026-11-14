//! Canonical encoding: one field order for saves and for the state digest.
//!
//! Every value is written through a [`Sink`]. A byte buffer gives the save
//! image, a [`Digest`] gives the hash, and both see the same stream.

use crate::draw::mix64;

/// Receives canonical fields in a fixed order.
pub trait Sink {
    fn u64(&mut self, v: u64);
    fn u8(&mut self, v: u8) { self.u64(v as u64); }
    fn u16(&mut self, v: u16) { self.u64(v as u64); }
    fn u32(&mut self, v: u32) { self.u64(v as u64); }
    fn i64(&mut self, v: i64) { self.u64(v as u64); }
    fn i32(&mut self, v: i32) { self.u64(v as i64 as u64); }
    fn bool(&mut self, v: bool) { self.u64(v as u64); }
    /// Sequence lengths are part of the stream, so `[a][b]` differs from `[a,b]`.
    fn len(&mut self, n: usize) { self.u64(n as u64); }
}

/// Little-endian, fixed-width byte image.
impl Sink for Vec<u8> {
    fn u64(&mut self, v: u64) { self.extend_from_slice(&v.to_le_bytes()); }
    fn u8(&mut self, v: u8) { self.push(v); }
    fn u16(&mut self, v: u16) { self.extend_from_slice(&v.to_le_bytes()); }
    fn u32(&mut self, v: u32) { self.extend_from_slice(&v.to_le_bytes()); }
    fn i32(&mut self, v: i32) { self.extend_from_slice(&v.to_le_bytes()); }
    fn bool(&mut self, v: bool) { self.push(v as u8); }
}

/// 128-bit streaming digest over canonical fields. Not cryptographic.
#[derive(Clone, Copy, Debug)]
pub struct Digest {
    a: u64,
    b: u64,
    n: u64,
}

impl Default for Digest {
    fn default() -> Self { Self { a: 0x243F_6A88_85A3_08D3, b: 0x1319_8A2E_0370_7344, n: 0 } }
}

impl Digest {
    pub fn finish(self) -> Hash {
        Hash([mix64(self.a ^ self.n), mix64(self.b ^ self.a.rotate_left(17))])
    }
}

impl Sink for Digest {
    fn u64(&mut self, w: u64) {
        self.a = (self.a ^ w).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29);
        self.b = (self.b.rotate_left(31) ^ w).wrapping_mul(0xC2B2_AE3D_27D4_EB4F).wrapping_add(self.a);
        self.n += 1;
    }
}

/// Result of a [`Digest`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash(pub [u64; 2]);

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}{:016x}", self.0[0], self.0[1])
    }
}

/// A value with a canonical field order.
pub trait Canon {
    fn put(&self, s: &mut impl Sink);

    fn digest(&self) -> Hash {
        let mut d = Digest::default();
        self.put(&mut d);
        d.finish()
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.put(&mut out);
        out
    }
}

/// Why a byte image was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    /// A field held a value outside its domain; the tag names the field.
    Invalid(&'static str),
    Trailing,
}

/// Cursor over a canonical byte image.
pub struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self { Self { rest: bytes } }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let (head, tail) = self.rest.split_first_chunk::<N>().ok_or(DecodeError::Truncated)?;
        self.rest = tail;
        Ok(*head)
    }

    pub fn u8(&mut self) -> Result<u8, DecodeError> { Ok(self.take::<1>()?[0]) }
    pub fn u16(&mut self) -> Result<u16, DecodeError> { Ok(u16::from_le_bytes(self.take()?)) }
    pub fn u32(&mut self) -> Result<u32, DecodeError> { Ok(u32::from_le_bytes(self.take()?)) }
    pub fn u64(&mut self) -> Result<u64, DecodeError> { Ok(u64::from_le_bytes(self.take()?)) }
    pub fn i32(&mut self) -> Result<i32, DecodeError> { Ok(i32::from_le_bytes(self.take()?)) }
    pub fn i64(&mut self) -> Result<i64, DecodeError> { Ok(self.u64()? as i64) }

    pub fn bool(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::Invalid("bool")),
        }
    }

    /// Reads a sequence length and refuses one the remaining bytes cannot hold.
    pub fn len(&mut self, min_item_bytes: usize) -> Result<usize, DecodeError> {
        let n = self.u64()?;
        let fits = (n as u128) * (min_item_bytes.max(1) as u128) <= self.rest.len() as u128;
        if fits { Ok(n as usize) } else { Err(DecodeError::Truncated) }
    }

    pub fn end(self) -> Result<(), DecodeError> {
        if self.rest.is_empty() { Ok(()) } else { Err(DecodeError::Trailing) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Pair(u32, Vec<u16>);
    impl Canon for Pair {
        fn put(&self, s: &mut impl Sink) {
            s.u32(self.0);
            s.len(self.1.len());
            for &v in &self.1 { s.u16(v); }
        }
    }

    #[test]
    fn bytes_round_trip_and_reject_damage() {
        let bytes = Pair(7, vec![1, 2, 3]).bytes();
        assert_eq!(bytes.len(), 4 + 8 + 6);
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u32(), Ok(7));
        let n = r.len(2).unwrap();
        assert_eq!((0..n).map(|_| r.u16().unwrap()).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(r.end(), Ok(()));

        let mut short = Reader::new(&bytes[..bytes.len() - 1]);
        short.u32().unwrap();
        assert_eq!(short.len(2), Err(DecodeError::Truncated));
        assert_eq!(Reader::new(&[2]).bool(), Err(DecodeError::Invalid("bool")));
        assert_eq!(Reader::new(&[0]).end(), Err(DecodeError::Trailing));
    }

    #[test]
    fn digest_separates_order_length_and_value() {
        let h = |p: Pair| p.digest();
        let base = h(Pair(1, vec![2, 3]));
        assert_eq!(base, h(Pair(1, vec![2, 3])));
        for other in [h(Pair(1, vec![3, 2])), h(Pair(1, vec![2])), h(Pair(2, vec![2, 3])), h(Pair(1, vec![2, 3, 0]))] {
            assert_ne!(base, other);
        }
    }

    #[test]
    fn digest_has_no_short_collisions() {
        let mut seen = std::collections::BTreeSet::new();
        for a in 0..64u64 {
            for b in 0..64u64 {
                let mut d = Digest::default();
                d.u64(a);
                d.u64(b);
                assert!(seen.insert(d.finish()));
            }
        }
    }
}
