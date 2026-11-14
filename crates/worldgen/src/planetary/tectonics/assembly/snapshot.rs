use super::*;
use crate::planetary::snapshot::{Decoder, Encoder, Result, require};

impl Assembly {
    pub(in crate::planetary) fn snapshot_write(&self, e: &mut Encoder) -> Result<()> {
        require((self.sink.length_squared() - 1.0).abs() < 1e-8
            && self.residence >= 0.0 && (0.0..=1.0).contains(&self.heat), "invalid assembly state")?;
        e.v3(self.sink)?; e.flag(self.dispersing)?; e.f64(self.residence)?; e.f64(self.heat)
    }
    pub(in crate::planetary) fn snapshot_read(d: &mut Decoder<'_>) -> Result<Self> {
        let result = Self { sink: d.v3()?, dispersing: d.flag()?, residence: d.f64()?, heat: d.f64()? };
        require((result.sink.length_squared() - 1.0).abs() < 1e-8
            && result.residence >= 0.0 && (0.0..=1.0).contains(&result.heat), "invalid assembly state")?;
        Ok(result)
    }
}


#[cfg(test)]
impl Assembly {
    pub(in crate::planetary) fn snapshot_fixture(seed: u32) -> Self {
        let mut result = Self::new(seed);
        result.heat = 0.37; result.residence = 53.0; result.dispersing = true;
        result
    }
    pub(in crate::planetary) fn snapshot_assert_equal(&self, b: &Self) {
        assert_eq!(self.sink.to_array().map(f64::to_bits), b.sink.to_array().map(f64::to_bits));
        assert_eq!(self.dispersing, b.dispersing);
        assert_eq!([self.heat,self.residence].map(f64::to_bits), [b.heat,b.residence].map(f64::to_bits));
    }
}
