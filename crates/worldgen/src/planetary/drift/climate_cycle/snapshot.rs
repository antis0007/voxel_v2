use super::*;
use crate::planetary::snapshot::{Decoder, Encoder, Result};

impl Climate {
    pub(in crate::planetary) fn snapshot_write(&self, e: &mut Encoder) -> Result<()> {
        for &x in &self.anomaly { e.f64(x)?; }
        Ok(())
    }
    pub(in crate::planetary) fn snapshot_read(d: &mut Decoder<'_>) -> Result<Self> {
        let mut anomaly = [0.0; N];
        for x in &mut anomaly { *x = d.f64()?; }
        Ok(Self { anomaly })
    }
}


#[cfg(test)]
impl Climate {
    pub(in crate::planetary) fn snapshot_fixture() -> Self {
        Self { anomaly: std::array::from_fn(|i| if i == 0 { -0.0 } else { i as f64 - 12.25 }) }
    }
    pub(in crate::planetary) fn snapshot_assert_equal(&self, b: &Self) {
        assert_eq!(self.anomaly.map(f64::to_bits), b.anomaly.map(f64::to_bits));
    }
}
