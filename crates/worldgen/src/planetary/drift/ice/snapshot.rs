use super::*;
use crate::planetary::snapshot::{Decoder, Encoder, NODES, Result};

impl Ice {
    pub(in crate::planetary) fn snapshot_write(&self, e: &mut Encoder) -> Result<()> {
        for values in [&self.excess, &self.target, &self.baseline, &self.former,
            &self.moved, &self.cut, &self.load, &self.carbon] { e.values(values, NODES)?; }
        Ok(())
    }
    pub(in crate::planetary) fn snapshot_read(d: &mut Decoder<'_>) -> Result<Self> {
        Ok(Self { excess: d.values(NODES)?, target: d.values(NODES)?, baseline: d.values(NODES)?,
            former: d.values(NODES)?, moved: d.values(NODES)?, cut: d.values(NODES)?,
            load: d.values(NODES)?, carbon: d.values(NODES)? })
    }
}


#[cfg(test)]
impl Ice {
    pub(in crate::planetary) fn snapshot_fixture() -> Self {
        let mut result = Self::new(NODES);
        for (j, values) in [&mut result.excess, &mut result.target, &mut result.baseline,
            &mut result.former, &mut result.moved, &mut result.cut, &mut result.load,
            &mut result.carbon].into_iter().enumerate() {
            for (i, x) in values.iter_mut().enumerate() {
                *x = if i == 0 { -0.0 } else { j as f64 * 0.01 + (i % 11) as f64 * 0.001 };
            }
        }
        result
    }
    pub(in crate::planetary) fn snapshot_assert_equal(&self, b: &Self) {
        for (a,b) in [&self.excess,&self.target,&self.baseline,&self.former,&self.moved,&self.cut,&self.load,&self.carbon]
            .into_iter().zip([&b.excess,&b.target,&b.baseline,&b.former,&b.moved,&b.cut,&b.load,&b.carbon]) {
            assert_eq!(a.len(), b.len());
            assert!(a.iter().zip(b).all(|(a,b)| a.to_bits() == b.to_bits()));
        }
    }
}
