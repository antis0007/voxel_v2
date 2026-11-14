//! Exact completed geology snapshots, not resumable simulation or world saves.
mod codec;
mod frames;

pub(in crate::planetary) use codec::{Decoder, Encoder, Result, require};
use super::{DriftFrame, tectonics::Plates};
use crate::geo::Progress;
use engine_voxel::{WorldSampleConfig, WorldType};

pub(in crate::planetary) const NODES: usize = 42_338;
pub(in crate::planetary) const FRAMES: usize = 241;
pub(in crate::planetary) const TEXELS: usize = 6 * 29 * 29;
// 002: per-texel plate owner and per-frame plate motion for smooth playback.
const MAGIC: &[u8; 8] = b"AXGEO002";

fn valid_config(cfg: &WorldSampleConfig) -> Result<()> {
    require(crate::planetary::deep_time(cfg.generator_version) && cfg.world_type == WorldType::Planet,
        "snapshot supports only deep-time planets (287, 288)")?;
    require(crate::planetary::full_history(cfg), "snapshot stores only the full 720 Myr history")?;
    require(cfg.planet.is_none_or(|p| p.valid()), "invalid snapshot planet recipe")
}

fn write_config(e: &mut Encoder, cfg: &WorldSampleConfig, shape_shelf: bool) -> Result<()> {
    valid_config(cfg)?;
    e.u32(cfg.generator_version)?;
    e.u32(cfg.seed)?;
    e.u8(1)?; // Stable codec tag for WorldType::Planet.
    e.u32(cfg.voxel_meters().to_bits())?;
    e.flag(cfg.planet.is_some())?;
    if let Some(p) = cfg.planet {
        e.raw(&p.values)?;
        e.u32(p.civilization.code())?;
    }
    e.flag(shape_shelf)
}

pub(super) fn encode(plates: &Plates, progress: &Progress) -> Result<Vec<u8>> {
    let cfg = plates.config();
    valid_config(&cfg)?;
    let frames = progress.drift_frames();
    require(frames.len() == FRAMES, "incomplete snapshot timeline")?;
    let mut e = Encoder::new();
    e.raw(MAGIC)?;
    write_config(&mut e, &cfg, plates.shape_shelf)?;
    plates.snapshot_write(&mut e)?;
    e.len(frames.len())?;
    for (i, frame) in frames.iter().enumerate() { frames::write(&mut e, frame, i)?; }
    Ok(e.finish())
}

pub(super) fn decode(bytes: &[u8], cfg: &WorldSampleConfig, shape_shelf: bool)
    -> Result<(Plates, Vec<DriftFrame>)>
{
    valid_config(cfg)?;
    let mut d = Decoder::new(bytes)?;
    require(d.raw(MAGIC.len())? == MAGIC, "snapshot schema mismatch")?;
    let mut expected = Encoder::new();
    write_config(&mut expected, cfg, shape_shelf)?;
    let expected = expected.finish();
    require(d.raw(expected.len())? == expected.as_slice(), "snapshot configuration mismatch")?;
    let plates = Plates::snapshot_read(&mut d, cfg, shape_shelf)?;
    let count = d.count(FRAMES, FRAMES, 32 + TEXELS * 60)?;
    let mut index = 0;
    let frames = d.items(count, 32 + TEXELS * 60, |d| {
        let frame = frames::read(d, index)?;
        index += 1;
        Ok(frame)
    })?;
    d.end()?;
    Ok((plates, frames))
}

#[cfg(test)]
mod tests;
