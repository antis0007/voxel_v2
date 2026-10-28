//! Variable-length integers and a bounds-checked reader for snapshot payloads.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireError(pub &'static str);

pub fn put_u64(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

pub fn put_i64(out: &mut Vec<u8>, v: i64) {
    put_u64(out, ((v << 1) ^ (v >> 63)) as u64);
}

pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub fn done(&self) -> bool {
        self.at == self.bytes.len()
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    pub fn u8(&mut self) -> Result<u8, WireError> {
        let b = *self.bytes.get(self.at).ok_or(WireError("short payload"))?;
        self.at += 1;
        Ok(b)
    }

    pub fn u64(&mut self) -> Result<u64, WireError> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(WireError("varint too long"))
    }

    pub fn u32(&mut self) -> Result<u32, WireError> {
        u32::try_from(self.u64()?).map_err(|_| WireError("varint out of range"))
    }

    pub fn i64(&mut self) -> Result<i64, WireError> {
        let z = self.u64()?;
        Ok((z >> 1) as i64 ^ -((z & 1) as i64))
    }

    pub fn i32(&mut self) -> Result<i32, WireError> {
        i32::try_from(self.i64()?).map_err(|_| WireError("varint out of range"))
    }

    /// The next `n` raw bytes.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or(WireError("short payload"))?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip_at_the_edges() {
        let mut out = Vec::new();
        let unsigned = [0, 1, 127, 128, 16_383, 16_384, u64::from(u32::MAX), u64::MAX];
        let signed = [0, -1, 1, -64, 64, i64::MIN, i64::MAX];
        unsigned.iter().for_each(|&v| put_u64(&mut out, v));
        signed.iter().for_each(|&v| put_i64(&mut out, v));
        let mut r = Reader::new(&out);
        assert!(unsigned.iter().all(|&v| r.u64() == Ok(v)));
        assert!(signed.iter().all(|&v| r.i64() == Ok(v)));
        assert!(r.done());
    }

    #[test]
    fn small_values_take_one_byte_and_bad_input_is_refused() {
        let mut out = Vec::new();
        put_i64(&mut out, -63);
        assert_eq!(out.len(), 1);
        assert!(Reader::new(&[0x80]).u64().is_err());
        assert!(Reader::new(&[0xff; 11]).u64().is_err());
        let mut big = Vec::new();
        put_u64(&mut big, u64::from(u32::MAX) + 1);
        assert!(Reader::new(&big).u32().is_err());
        let mut r = Reader::new(&[1, 2, 3]);
        assert_eq!(r.take(2), Ok(&[1, 2][..]));
        assert!(r.take(2).is_err() && r.take(usize::MAX).is_err());
        assert_eq!(r.take(1), Ok(&[3][..]));
    }
}
