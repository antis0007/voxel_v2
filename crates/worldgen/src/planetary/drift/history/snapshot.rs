use super::*;
use crate::planetary::snapshot::{Decoder, Encoder, NODES, Result, require};

impl Field {
    pub(in crate::planetary) fn snapshot_write(&self, e: &mut Encoder) -> Result<()> {
        require(self.records.len() == NODES && self.next.is_empty(), "history is not sealed")?;
        e.len(self.records.len())?;
        for r in &self.records {
            require(r.column.duration == crate::planetary::DEEP_TIME_MYR, "invalid history duration")?;
            e.u16(r.column.duration)?;
            // All stored beds, including future and otherwise latent fields.
            for b in r.column.beds.iter().chain(&r.column.recent) {
                for v in [b.depth, b.wet, b.organic, b.carbonate, b.lava, b.date] { e.f64(v)?; }
            }
            for v in [r.weathered_myr, r.locked, r.magma, r.quake_m, r.eruption_m,
                r.wet, r.temperature, r.winter_c, r.basement_cut] { e.f64(v)?; }
        }
        Ok(())
    }

    pub(in crate::planetary) fn snapshot_read(d: &mut Decoder<'_>) -> Result<Self> {
        let count = d.count(NODES, NODES, 1322)?;
        let records = d.items(count, 1322, |d| {
            let duration = d.u16()?;
            require(duration == crate::planetary::DEEP_TIME_MYR, "invalid history duration")?;
            let mut column = Column { duration, ..Column::default() };
            for b in column.beds.iter_mut().chain(&mut column.recent) {
                *b = Bed { depth: d.f64()?, wet: d.f64()?, organic: d.f64()?,
                    carbonate: d.f64()?, lava: d.f64()?, date: d.f64()? };
            }
            Ok(Record { column, weathered_myr: d.f64()?, locked: d.f64()?, magma: d.f64()?,
                quake_m: d.f64()?, eruption_m: d.f64()?, wet: d.f64()?,
                temperature: d.f64()?, winter_c: d.f64()?, basement_cut: d.f64()? })
        })?;
        // coupled::solve calls seal after its final datum translation.
        Ok(Self { records, next: Vec::new() })
    }
}


#[cfg(test)]
impl Field {
    pub(in crate::planetary) fn snapshot_fixture() -> Self {
        let mut field = Self::with_duration(NODES, crate::planetary::DEEP_TIME_MYR);
        for (i, r) in field.records.iter_mut().enumerate() {
            for (j, b) in r.column.beds.iter_mut().chain(&mut r.column.recent).enumerate() {
                let x = 0.125 + (i % 17) as f64 * 0.01 + j as f64;
                *b = Bed { depth: x, wet: x * 0.3, organic: -0.0, carbonate: x * 0.7,
                    lava: x * 0.1, date: x * (731.0 + j as f64) };
            }
            let x = i as f64 * 0.001;
            r.weathered_myr = x; r.locked = x + 1.0; r.magma = x + 2.0;
            r.quake_m = -x; r.eruption_m = x + 3.0; r.wet = x + 4.0;
            r.temperature = x - 5.0; r.winter_c = -0.0; r.basement_cut = x + 6.0;
        }
        field.seal();
        field
    }
    pub(in crate::planetary) fn snapshot_assert_equal(&self, other: &Self) {
        assert!(self.next.is_empty() && other.next.is_empty());
        assert_eq!(self.records.len(), other.records.len());
        for (a, b) in self.records.iter().zip(&other.records) {
            assert_eq!(a.column.duration, b.column.duration);
            for (a, b) in a.column.beds.iter().chain(&a.column.recent)
                .zip(b.column.beds.iter().chain(&b.column.recent)) {
                assert_eq!([a.depth,a.wet,a.organic,a.carbonate,a.lava,a.date].map(f64::to_bits),
                    [b.depth,b.wet,b.organic,b.carbonate,b.lava,b.date].map(f64::to_bits));
            }
            assert_eq!([a.weathered_myr,a.locked,a.magma,a.quake_m,a.eruption_m,
                a.wet,a.temperature,a.winter_c,a.basement_cut].map(f64::to_bits),
                [b.weathered_myr,b.locked,b.magma,b.quake_m,b.eruption_m,
                b.wet,b.temperature,b.winter_c,b.basement_cut].map(f64::to_bits));
        }
    }
}
