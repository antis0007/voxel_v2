use super::super::{Decoder, MAX_FILE_BYTES, checksum, push};
use anyhow::{Context, Result, ensure};
use glam::IVec3;

pub(super) const MAGIC: &[u8; 8] = b"VXCASE01";
pub(super) const MAX_ACTOR: usize = 64 * 1024;
const MAX_CA: usize = 160 * 1024;

pub(super) struct Parts<'a> {
    pub model: u64,
    pub origin: IVec3,
    pub world: &'a [u8],
    pub region: &'a [u8],
    pub actors: &'a [u8],
}

fn u64_value(input: &mut Decoder<'_>) -> Result<u64> {
    Ok(u64::from(input.u32()?) | (u64::from(input.u32()?) << 32))
}

fn blob<'a>(input: &mut Decoder<'a>, limit: usize) -> Result<&'a [u8]> {
    let len = input.u32()? as usize;
    ensure!(len <= limit, "Exact session section exceeds size limit");
    let end = input.offset.checked_add(len).context("Exact session length overflow")?;
    let value = input.bytes.get(input.offset..end).context("Truncated exact session section")?;
    input.offset = end;
    Ok(value)
}

pub(super) fn parse(bytes: &[u8]) -> Result<Parts<'_>> {
    ensure!((56..=MAX_FILE_BYTES as usize).contains(&bytes.len()), "Invalid exact session size");
    ensure!(bytes.starts_with(MAGIC), "Unsupported exact session format");
    let end = bytes.len() - 8;
    ensure!(checksum(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?), "Exact session checksum mismatch");
    let mut input = Decoder { bytes: &bytes[..end], offset: 8 };
    let model = u64_value(&mut input)?;
    let origin = IVec3::new(input.u32()? as i32, input.u32()? as i32, input.u32()? as i32);
    let world = blob(&mut input, MAX_FILE_BYTES as usize)?;
    let region = blob(&mut input, MAX_CA)?;
    let actors = blob(&mut input, MAX_ACTOR)?;
    let commit = u64_value(&mut input)?;
    ensure!(input.offset == end && world.len() >= 16, "Unexpected exact session data");
    let world_end = world.len() - 8;
    ensure!(checksum(&world[..world_end]) == u64::from_le_bytes(world[world_end..].try_into()?), "Embedded world checksum mismatch");
    ensure!(commit == u64::from_le_bytes(world[world_end-8..world_end].try_into()?), "Exact session commit mismatch");
    Ok(Parts { model, origin, world, region, actors })
}

pub(super) fn encode(model: u64, origin: IVec3, world: &[u8], region: &[u8], actors: &[u8], commit: u64) -> Result<Vec<u8>> {
    ensure!(region.len() <= MAX_CA && actors.len() <= MAX_ACTOR, "Exact session section exceeds size limit");
    let total = world.len().checked_add(region.len()).and_then(|n| n.checked_add(actors.len()))
        .and_then(|n| n.checked_add(56)).context("Exact session size overflow")?;
    ensure!(total <= MAX_FILE_BYTES as usize, "Exact session exceeds size limit");
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&model.to_le_bytes());
    for axis in origin.to_array() { push(&mut bytes, axis as u32); }
    for section in [world, region, actors] {
        push(&mut bytes, section.len() as u32);
        bytes.extend_from_slice(section);
    }
    bytes.extend_from_slice(&commit.to_le_bytes());
    let sum = checksum(&bytes);
    bytes.extend_from_slice(&sum.to_le_bytes());
    Ok(bytes)
}
