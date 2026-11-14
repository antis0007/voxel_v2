use super::*;
use crate::planetary::snapshot::{Decoder, Encoder, Result, require};

impl Plates {
    pub(in crate::planetary) fn snapshot_write(&self, e: &mut Encoder) -> Result<()> {
        // New plate state must trigger an explicit snapshot-completeness review.
        let Self { cfg: _, plates: _, seed: _, scale: _, ocean: _, relief: _,
            coastal: _, shape_shelf: _, faults: _, traverse: _, rifted_margins: _,
            prior_spins: _, motion_history: _, parents: _, drift: _, assembly: _ } = self;
        let n = self.plates.len();
        require((18..=40).contains(&n) && self.parents.len() == n
            && self.prior_spins.len() == n && self.motion_history.len() == usize::from(crate::planetary::DEEP_TIME_MYR),
            "incomplete plate snapshot")?;
        e.u32(self.seed)?;
        for x in [self.scale, self.ocean, self.relief] { e.f64(x)?; }
        for x in [self.coastal, self.shape_shelf, self.faults, self.traverse, self.rifted_margins] { e.flag(x)?; }
        e.len(n)?;
        for p in &self.plates {
            require((p.centre.length_squared() - 1.0).abs() < 1e-8
                && (0.0..=1.0).contains(&p.continental), "invalid plate geometry")?;
            e.v3(p.centre)?; e.v3(p.spin)?; e.f64(p.continental)?;
        }
        e.len(self.prior_spins.len())?;
        for &spin in &self.prior_spins { e.v3(spin)?; }
        e.len(self.parents.len())?;
        for (i, &parent) in self.parents.iter().enumerate() {
            require(if i < 18 { parent == i } else { parent < i }, "invalid plate ancestry")?;
            e.len(parent)?;
        }
        e.len(self.motion_history.len())?;
        let mut previous = 18;
        for (radians, spins) in &self.motion_history {
            require(*radians > 0.0 && (previous..=n).contains(&spins.len()), "invalid plate motion")?;
            e.f64(*radians)?; e.len(spins.len())?;
            for &spin in spins { e.v3(spin)?; }
            previous = spins.len();
        }
        require(previous == n, "incomplete final motion stage")?;
        e.flag(self.assembly.is_some())?;
        self.assembly.as_ref().ok_or("missing assembly state")?.snapshot_write(e)?;
        e.flag(self.drift.is_some())?;
        self.drift.as_ref().ok_or("missing completed drift")?.snapshot_write(e)
    }

    pub(in crate::planetary) fn snapshot_read(d: &mut Decoder<'_>, cfg: &WorldSampleConfig,
        shape_shelf: bool) -> Result<Self>
    {
        let mut result = Self::new(cfg);
        result.shape_shelf = shape_shelf;
        require(d.u32()? == result.seed, "plate seed mismatch")?;
        for expected in [result.scale, result.ocean, result.relief] {
            require(d.f64()?.to_bits() == expected.to_bits(), "plate recipe mismatch")?;
        }
        for expected in [result.coastal, result.shape_shelf, result.faults, result.traverse, result.rifted_margins] {
            require(d.flag()? == expected, "plate flag mismatch")?;
        }
        let n = d.count(18, 40, 56)?;
        result.plates = d.items(n, 56, |d| {
            let centre = d.v3()?; let spin = d.v3()?; let continental = d.f64()?;
            require((centre.length_squared() - 1.0).abs() < 1e-8
                && (0.0..=1.0).contains(&continental), "invalid plate geometry")?;
            Ok(Plate { centre, spin, continental })
        })?;
        let count = d.count(n, n, 24)?;
        result.prior_spins = d.items(count, 24, Decoder::v3)?;
        let count = d.count(n, n, 4)?;
        result.parents = d.items(count, 4, |d| Ok(d.u32()? as usize))?;
        for (i, &parent) in result.parents.iter().enumerate() {
            require(if i < 18 { parent == i } else { parent < i }, "invalid plate ancestry")?;
        }
        let count = d.count(usize::from(crate::planetary::DEEP_TIME_MYR), usize::from(crate::planetary::DEEP_TIME_MYR), 12 + 18 * 24)?;
        let mut previous = 18;
        result.motion_history = d.items(count, 12 + 18 * 24, |d| {
            let radians = d.f64()?;
            require(radians > 0.0, "invalid plate rotation")?;
            let count = d.count(previous, n, 24)?;
            previous = count;
            Ok((radians, d.items(count, 24, Decoder::v3)?))
        })?;
        require(previous == n, "incomplete final motion stage")?;
        require(d.flag()?, "missing assembly state")?;
        result.assembly = Some(assembly::Assembly::snapshot_read(d)?);
        require(d.flag()?, "missing completed drift")?;
        result.drift = Some(Drift::snapshot_read(d)?);
        Ok(result)
    }
}


#[cfg(test)]
impl Plates {
    pub(in crate::planetary) fn snapshot_fixture(cfg: &WorldSampleConfig, shape_shelf: bool) -> Self {
        let mut result = Self::new(cfg);
        result.shape_shelf = shape_shelf;
        let mut daughter = result.plates[7];
        daughter.centre = DQuat::from_rotation_y(0.05) * daughter.centre;
        daughter.spin *= 0.75;
        result.plates.push(daughter); result.parents.push(7);
        result.prior_spins = result.plates.iter().map(|p| p.spin * 0.9).collect();
        result.motion_history = (0..usize::from(crate::planetary::DEEP_TIME_MYR)).map(|step| {
            let n = if step < 360 { 18 } else { 19 };
            (0.0001 + step as f64 * 0.00000001,
                result.plates[..n].iter().map(|p| p.spin * (1.0 + step as f64 * 0.0001)).collect())
        }).collect();
        result.assembly = Some(assembly::Assembly::snapshot_fixture(cfg.seed));
        result.drift = Some(Drift::snapshot_fixture(cfg.seed));
        result
    }
    pub(in crate::planetary) fn snapshot_assert_equal(&self, b: &Self) {
        assert_eq!(self.cfg, b.cfg); assert_eq!(self.seed, b.seed);
        assert_eq!([self.scale,self.ocean,self.relief].map(f64::to_bits), [b.scale,b.ocean,b.relief].map(f64::to_bits));
        assert_eq!([self.coastal,self.shape_shelf,self.faults,self.traverse,self.rifted_margins],
            [b.coastal,b.shape_shelf,b.faults,b.traverse,b.rifted_margins]);
        assert_eq!(self.plates.len(), b.plates.len());
        for (a,b) in self.plates.iter().zip(&b.plates) {
            assert_eq!(a.centre.to_array().map(f64::to_bits), b.centre.to_array().map(f64::to_bits));
            assert_eq!(a.spin.to_array().map(f64::to_bits), b.spin.to_array().map(f64::to_bits));
            assert_eq!(a.continental.to_bits(), b.continental.to_bits());
        }
        assert_eq!(self.parents, b.parents);
        assert_eq!(self.prior_spins.len(), b.prior_spins.len());
        for (a,b) in self.prior_spins.iter().zip(&b.prior_spins) {
            assert_eq!(a.to_array().map(f64::to_bits), b.to_array().map(f64::to_bits));
        }
        assert_eq!(self.motion_history.len(), b.motion_history.len());
        for ((aa, av),(ba,bv)) in self.motion_history.iter().zip(&b.motion_history) {
            assert_eq!(aa.to_bits(), ba.to_bits()); assert_eq!(av.len(), bv.len());
            for (a,b) in av.iter().zip(bv) {
                assert_eq!(a.to_array().map(f64::to_bits), b.to_array().map(f64::to_bits));
            }
        }
        self.assembly.as_ref().unwrap().snapshot_assert_equal(b.assembly.as_ref().unwrap());
        self.drift.as_ref().unwrap().snapshot_assert_equal(b.drift.as_ref().unwrap());
    }
}
