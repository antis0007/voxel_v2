use glam::DVec3;

pub(in crate::planetary) type Result<T> = std::result::Result<T, String>;
pub(in crate::planetary) const MAX_BYTES: usize = 160 * 1024 * 1024;

pub(in crate::planetary) fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.to_owned()) }
}

pub(in crate::planetary) struct Encoder { bytes: Vec<u8> }
impl Encoder {
    pub fn new() -> Self { Self { bytes: Vec::new() } }
    pub fn finish(self) -> Vec<u8> { self.bytes }
    fn growth_target(size: usize, capacity: usize) -> usize {
        size.max(capacity.saturating_mul(2)).max(8).min(MAX_BYTES)
    }
    pub fn raw(&mut self, bytes: &[u8]) -> Result<()> {
        let size = self.bytes.len().checked_add(bytes.len()).ok_or("snapshot size overflow")?;
        require(size <= MAX_BYTES, "snapshot exceeds size limit")?;
        if size > self.bytes.capacity() {
            let target = Self::growth_target(size, self.bytes.capacity());
            self.bytes.try_reserve_exact(target - self.bytes.len())
                .map_err(|_| "snapshot allocation failed")?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub fn u8(&mut self, x: u8) -> Result<()> { self.raw(&[x]) }
    pub fn u16(&mut self, x: u16) -> Result<()> { self.raw(&x.to_le_bytes()) }
    pub fn u32(&mut self, x: u32) -> Result<()> { self.raw(&x.to_le_bytes()) }
    pub fn u64(&mut self, x: u64) -> Result<()> { self.raw(&x.to_le_bytes()) }
    pub fn flag(&mut self, x: bool) -> Result<()> { self.u8(u8::from(x)) }
    pub fn len(&mut self, x: usize) -> Result<()> {
        self.u32(u32::try_from(x).map_err(|_| "snapshot count overflow")?)
    }
    pub fn f64(&mut self, x: f64) -> Result<()> {
        require(x.is_finite(), "nonfinite snapshot f64")?;
        self.u64(x.to_bits())
    }
    pub fn f32(&mut self, x: f32) -> Result<()> {
        require(x.is_finite(), "nonfinite snapshot f32")?;
        self.u32(x.to_bits())
    }
    pub fn v3(&mut self, x: DVec3) -> Result<()> {
        for v in x.to_array() { self.f64(v)?; }
        Ok(())
    }
    pub fn values(&mut self, values: &[f64], expected: usize) -> Result<()> {
        require(values.len() == expected, "snapshot field length")?;
        self.len(values.len())?;
        for &value in values { self.f64(value)?; }
        Ok(())
    }
}

pub(in crate::planetary) struct Decoder<'a> { bytes: &'a [u8], at: usize }
impl<'a> Decoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        require(bytes.len() <= MAX_BYTES, "snapshot exceeds size limit")?;
        Ok(Self { bytes, at: 0 })
    }
    pub fn remaining(&self) -> usize { self.bytes.len() - self.at }
    pub fn need(&self, n: usize) -> Result<()> { require(n <= self.remaining(), "truncated snapshot") }
    pub fn raw(&mut self, n: usize) -> Result<&'a [u8]> {
        self.need(n)?;
        let start = self.at;
        self.at += n;
        Ok(&self.bytes[start..self.at])
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut bytes = [0; N];
        bytes.copy_from_slice(self.raw(N)?);
        Ok(bytes)
    }
    pub fn u8(&mut self) -> Result<u8> { Ok(self.array::<1>()?[0]) }
    pub fn u16(&mut self) -> Result<u16> { Ok(u16::from_le_bytes(self.array()?)) }
    pub fn u32(&mut self) -> Result<u32> { Ok(u32::from_le_bytes(self.array()?)) }
    pub fn u64(&mut self) -> Result<u64> { Ok(u64::from_le_bytes(self.array()?)) }
    pub fn flag(&mut self) -> Result<bool> {
        match self.u8()? { 0 => Ok(false), 1 => Ok(true), _ => Err("invalid snapshot boolean".to_owned()) }
    }
    pub fn f64(&mut self) -> Result<f64> {
        let x = f64::from_bits(self.u64()?);
        require(x.is_finite(), "nonfinite snapshot f64")?;
        Ok(x)
    }
    pub fn f32(&mut self) -> Result<f32> {
        let x = f32::from_bits(self.u32()?);
        require(x.is_finite(), "nonfinite snapshot f32")?;
        Ok(x)
    }
    pub fn v3(&mut self) -> Result<DVec3> { Ok(DVec3::new(self.f64()?, self.f64()?, self.f64()?)) }
    pub fn count(&mut self, low: usize, high: usize, minimum_bytes: usize) -> Result<usize> {
        let n = usize::try_from(self.u32()?).map_err(|_| "snapshot count overflow")?;
        require((low..=high).contains(&n), "invalid snapshot count")?;
        self.need(n.checked_mul(minimum_bytes).ok_or("snapshot count overflow")?)?;
        Ok(n)
    }
    pub fn items<T>(&mut self, n: usize, minimum_bytes: usize,
        mut read: impl FnMut(&mut Self) -> Result<T>) -> Result<Vec<T>>
    {
        self.need(n.checked_mul(minimum_bytes).ok_or("snapshot count overflow")?)?;
        let mut result = Vec::new();
        result.try_reserve_exact(n).map_err(|_| "snapshot allocation failed")?;
        for _ in 0..n { result.push(read(self)?); }
        Ok(result)
    }
    pub fn values(&mut self, expected: usize) -> Result<Vec<f64>> {
        let n = self.count(expected, expected, 8)?;
        self.items(n, 8, Self::f64)
    }
    pub fn end(&self) -> Result<()> { require(self.remaining() == 0, "trailing snapshot bytes") }
}

#[cfg(test)]
mod encoder_growth_tests {
    use super::*;

    #[test]
    fn growth_preserves_bytes_and_caps_the_requested_capacity() {
        let mut small = Encoder::new();
        small.raw(&[]).unwrap();
        assert_eq!(small.bytes.capacity(), 0);
        small.raw(&[1, 2, 3]).unwrap();
        let pointer = small.bytes.as_ptr();
        let capacity = small.bytes.capacity();
        small.raw(&[4, 5]).unwrap();
        assert_eq!(small.bytes.as_ptr(), pointer);
        assert_eq!(small.bytes.capacity(), capacity);
        assert_eq!(small.finish(), [1, 2, 3, 4, 5]);
        assert_eq!(Encoder::growth_target(1, 0), 8);
        assert_eq!(Encoder::growth_target(9, 8), 16);
        assert_eq!(Encoder::growth_target(40, 8), 40);

        const CROSSING: usize = 128 * 1024 * 1024;
        assert_eq!(Encoder::growth_target(CROSSING + 1, CROSSING), MAX_BYTES);
        assert_eq!(Encoder::growth_target(MAX_BYTES, CROSSING), MAX_BYTES);
        assert_eq!(Encoder::growth_target(1, usize::MAX), MAX_BYTES);
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(CROSSING).unwrap();
        bytes.resize(CROSSING, 0x5a);
        let mut large = Encoder { bytes };
        let capacity_before = large.bytes.capacity();
        large.raw(&[0xa5]).unwrap();
        assert_eq!(large.bytes.len(), CROSSING + 1);
        assert!(large.bytes[..CROSSING].iter().all(|&b| b == 0x5a));
        assert_eq!(large.bytes[CROSSING], 0xa5);
        // Allocators may grant more than requested; do not assert an upper
        // bound on capacity(), only on the production growth target above.
        if capacity_before == CROSSING { assert!(large.bytes.capacity() >= MAX_BYTES); }
        else { assert!(large.bytes.capacity() >= CROSSING + 1); }
        large.bytes.resize(MAX_BYTES - 1, 0x33);
        large.raw(&[0x7e]).unwrap();
        let pointer = large.bytes.as_ptr();
        let capacity = large.bytes.capacity();
        assert!(large.raw(&[0xff]).is_err());
        assert_eq!(large.bytes.len(), MAX_BYTES);
        assert_eq!(large.bytes[MAX_BYTES - 1], 0x7e);
        assert_eq!(large.bytes.as_ptr(), pointer);
        assert_eq!(large.bytes.capacity(), capacity);
    }
}
