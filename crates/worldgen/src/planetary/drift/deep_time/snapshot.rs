use super::*;
use crate::planetary::snapshot::{Decoder, Encoder, NODES, Result, require};

impl Evolution {
    pub(in crate::planetary) fn snapshot_write(&self, e: &mut Encoder) -> Result<()> {
        require(self.provinces.len() == 12 && self.carbon > 0.0, "invalid deep-time state")?;
        e.len(self.provinces.len())?;
        for p in &self.provinces {
            require((p.centre.length_squared() - 1.0).abs() < 1e-8
                && p.vents.iter().all(|v| (v.length_squared() - 1.0).abs() < 1e-8)
                && p.lifetime > 0.0, "invalid mantle province")?;
            e.v3(p.centre)?;
            for &v in &p.vents { e.v3(v)?; }
            e.f64(p.peak)?; e.f64(p.lifetime)?;
        }
        e.f64(self.carbon)?;
        e.flag(self.climate.is_some())?;
        self.climate.as_ref().ok_or("missing terminal climate")?.snapshot_write(e)?;
        e.values(&self.melt, NODES)
    }
    pub(in crate::planetary) fn snapshot_read(d: &mut Decoder<'_>) -> Result<Self> {
        let count = d.count(12, 12, 208)?;
        let provinces = d.items(count, 208, |d| {
            let centre = d.v3()?;
            let mut vents = [DVec3::ZERO; 7];
            for v in &mut vents { *v = d.v3()?; }
            let p = Province { centre, vents, peak: d.f64()?, lifetime: d.f64()? };
            require((p.centre.length_squared() - 1.0).abs() < 1e-8
                && p.vents.iter().all(|v| (v.length_squared() - 1.0).abs() < 1e-8)
                && p.lifetime > 0.0, "invalid mantle province")?;
            Ok(p)
        })?;
        let carbon = d.f64()?;
        require(carbon > 0.0, "invalid carbon reservoir")?;
        require(d.flag()?, "missing terminal climate")?;
        let climate = Some(Climate::snapshot_read(d)?);
        let melt = d.values(NODES)?;
        Ok(Self { provinces, carbon, climate, melt })
    }
}


#[cfg(test)]
impl Evolution {
    pub(in crate::planetary) fn snapshot_fixture(seed: u32) -> Self {
        let mut result = Self::new(seed);
        result.carbon = 2.75;
        result.climate = Some(Climate::snapshot_fixture());
        result.melt = (0..NODES).map(|i| if i == 0 { -0.0 } else { i as f64 * 0.001 }).collect();
        result
    }
    pub(in crate::planetary) fn snapshot_assert_equal(&self, b: &Self) {
        assert_eq!(self.carbon.to_bits(), b.carbon.to_bits());
        assert_eq!(self.provinces.len(), b.provinces.len());
        for (a,b) in self.provinces.iter().zip(&b.provinces) {
            assert_eq!(a.centre.to_array().map(f64::to_bits), b.centre.to_array().map(f64::to_bits));
            for (a,b) in a.vents.iter().zip(&b.vents) {
                assert_eq!(a.to_array().map(f64::to_bits), b.to_array().map(f64::to_bits));
            }
            assert_eq!([a.peak,a.lifetime].map(f64::to_bits), [b.peak,b.lifetime].map(f64::to_bits));
        }
        self.climate.as_ref().unwrap().snapshot_assert_equal(b.climate.as_ref().unwrap());
        assert_eq!(self.melt.len(), b.melt.len());
        assert!(self.melt.iter().zip(&b.melt).all(|(a,b)| a.to_bits() == b.to_bits()));
    }
}
