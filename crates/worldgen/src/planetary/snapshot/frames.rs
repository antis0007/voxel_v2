use super::{Decoder, Encoder, Result, TEXELS, require};
use crate::planetary::{DriftFrame, DriftTexel, HistoryBudget, HistoryEvent};

pub(super) fn write(e: &mut Encoder, f: &DriftFrame, index: usize) -> Result<()> {
    require(f.duration_myr == crate::planetary::DEEP_TIME_MYR && usize::from(f.myr) == index * 3 && f.n == 29
        && f.texels.len() == TEXELS && f.events.len() <= 4 && f.motion.len() <= 40, "invalid snapshot frame")?;
    e.u16(f.duration_myr)?; e.u16(f.myr)?; e.len(f.n)?; e.f32(f.sea_m)?;
    let range = f.annual_range_c.ok_or("missing frame temperature range")?;
    require(range[0] <= range[1], "invalid frame temperature range")?;
    e.flag(true)?; e.f32(range[0])?; e.f32(range[1])?;
    let b = f.budget.ok_or("missing frame budget")?;
    require((18..=40).contains(&b.plates), "invalid frame plate count")?;
    e.flag(true)?;
    for v in [b.continental_km3, b.continental_error_km3, b.continental_eroded_km3,
        b.continental_created_km3, b.eroded_m3, b.sediment_m3, b.sediment_error_m3,
        b.alongshore_m3] { e.f64(v)?; }
    e.len(b.plates)?;
    e.len(f.texels.len())?;
    for t in &f.texels {
        require(t.lineage <= 18, "invalid frame lineage")?;
        e.f32(t.height_m)?;
        for v in t.stress { e.f32(v)?; }
        for v in [t.continental_km, t.ocean_age_myr, t.sediment_m, t.runoff_m2] { e.f32(v)?; }
        e.u16(t.lineage)?;
        require(f.motion.is_empty() || usize::from(t.plate) < f.motion.len(), "invalid frame plate")?;
        e.u16(t.plate)?;
        let climate = t.climate.ok_or("missing frame climate")?;
        e.flag(true)?; e.f32(climate[0])?; e.f32(climate[1])?;
        e.flag(true)?; e.f32(t.winter_c.ok_or("missing frame winter")?)?;
        for v in [t.rupture_m, t.eruption_m, t.weathered_myr] { e.f32(v)?; }
    }
    e.len(f.events.len())?;
    for event in &f.events {
        require((0.0..=f64::from(f.myr)).contains(&event.myr), "invalid event date")?;
        e.f64(event.myr)?;
        for v in event.direction { e.f32(v)?; }
        e.f32(event.metres)?; e.flag(event.eruption)?;
    }
    e.len(f.motion.len())?;
    for axis in &f.motion {
        require(axis.iter().all(|v| v.is_finite()), "invalid frame motion")?;
        for &v in axis { e.f32(v)?; }
    }
    Ok(())
}

pub(super) fn read(d: &mut Decoder<'_>, index: usize) -> Result<DriftFrame> {
    let duration_myr = d.u16()?; let myr = d.u16()?; let n = d.u32()? as usize;
    require(duration_myr == crate::planetary::DEEP_TIME_MYR && usize::from(myr) == index * 3 && n == 29,
        "invalid snapshot frame")?;
    let sea_m = d.f32()?;
    require(d.flag()?, "missing frame temperature range")?;
    let range = [d.f32()?, d.f32()?];
    require(range[0] <= range[1], "invalid frame temperature range")?;
    require(d.flag()?, "missing frame budget")?;
    let budget = HistoryBudget { continental_km3: d.f64()?, continental_error_km3: d.f64()?,
        continental_eroded_km3: d.f64()?, continental_created_km3: d.f64()?,
        eroded_m3: d.f64()?, sediment_m3: d.f64()?, sediment_error_m3: d.f64()?,
        alongshore_m3: d.f64()?, plates: d.count(18, 40, 0)? };
    let count = d.count(TEXELS, TEXELS, 60)?;
    let texels = d.items(count, 60, |d| {
        let height_m = d.f32()?; let stress = [d.f32()?, d.f32()?, d.f32()?];
        let continental_km = d.f32()?; let ocean_age_myr = d.f32()?;
        let sediment_m = d.f32()?; let runoff_m2 = d.f32()?; let lineage = d.u16()?;
        require(lineage <= 18, "invalid frame lineage")?;
        let plate = d.u16()?;
        require(plate < 40, "invalid frame plate")?;
        require(d.flag()?, "missing frame climate")?;
        let climate = Some([d.f32()?, d.f32()?]);
        require(d.flag()?, "missing frame winter")?;
        let winter_c = Some(d.f32()?);
        Ok(DriftTexel { height_m, stress, continental_km, ocean_age_myr, sediment_m,
            runoff_m2, lineage, climate, winter_c, rupture_m: d.f32()?,
            eruption_m: d.f32()?, weathered_myr: d.f32()?, plate })
    })?;
    let count = d.count(0, 4, 25)?;
    let events = d.items(count, 25, |d| {
        let at = d.f64()?;
        require((0.0..=f64::from(myr)).contains(&at), "invalid event date")?;
        Ok(HistoryEvent { myr: at, direction: [d.f32()?, d.f32()?, d.f32()?],
            metres: d.f32()?, eruption: d.flag()? })
    })?;
    let count = d.count(0, 40, 12)?;
    let motion = d.items(count, 12, |d| {
        let axis = [d.f32()?, d.f32()?, d.f32()?];
        require(axis.iter().all(|v| v.is_finite()), "invalid frame motion")?;
        Ok(axis)
    })?;
    require(texels.iter().all(|t| motion.is_empty() || usize::from(t.plate) < motion.len()), "invalid frame plate")?;
    Ok(DriftFrame { duration_myr, myr, n, texels, budget: Some(budget), sea_m,
        annual_range_c: Some(range), events, motion })
}
