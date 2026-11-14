use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};

pub(super) const MAGIC: &[u8; 16] = b"AXIOM-GEO-CACHE1";
pub(super) const MAX_KEY: usize = 4096;
pub(super) const MAX_PAYLOAD: u64 = 160 * 1024 * 1024;
pub(super) const PREFIX: &str = "axiom-geology-v1-";
pub(super) const HEADER: u64 = 16 + 4 + 4 + 8 + 32;

pub(super) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(super) fn key_valid(key: &[u8]) -> io::Result<()> {
    if key.is_empty() || key.len() > MAX_KEY { return Err(invalid("cache key length")); }
    Ok(())
}

pub(super) fn digest_name(key: &[u8]) -> String {
    let bytes = Sha256::digest(key);
    let mut result = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

pub(super) fn filename(key: &[u8]) -> String {
    format!("{PREFIX}{}.bin", digest_name(key))
}

pub(super) fn owned_name(name: &str) -> Option<(&str, bool)> {
    let rest = name.strip_prefix(PREFIX)?;
    let hash = rest.get(..64)?;
    if !hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) { return None; }
    if rest.get(64..)? == ".bin" { return Some((hash, false)); }
    let suffix = rest.get(64..)?.strip_prefix('.')?.strip_suffix(".tmp")?;
    let (pid, nonce) = suffix.split_once('.')?;
    if pid.is_empty() || pid.len() > 8 || nonce.len() != 16
        || !pid.bytes().chain(nonce.bytes()).all(|b| b.is_ascii_hexdigit()) { return None; }
    Some((hash, true))
}

pub(super) struct Header {
    pub key: Vec<u8>,
    pub payload_len: u64,
    pub digest: [u8; 32],
}

impl Header {
    pub fn total_len(&self) -> u64 { HEADER + self.key.len() as u64 + self.payload_len }
}

pub(super) fn read_header(reader: &mut impl Read) -> io::Result<Header> {
    let mut magic = [0; 16]; reader.read_exact(&mut magic)?;
    let mut version = [0; 4]; reader.read_exact(&mut version)?;
    let mut key_len = [0; 4]; reader.read_exact(&mut key_len)?;
    let mut payload_len = [0; 8]; reader.read_exact(&mut payload_len)?;
    let mut digest = [0; 32]; reader.read_exact(&mut digest)?;
    if &magic != MAGIC || u32::from_le_bytes(version) != 1 { return Err(invalid("cache format")); }
    let key_len = u32::from_le_bytes(key_len) as usize;
    let payload_len = u64::from_le_bytes(payload_len);
    if key_len == 0 || key_len > MAX_KEY || payload_len > MAX_PAYLOAD { return Err(invalid("cache bounds")); }
    let mut key = vec![0; key_len]; reader.read_exact(&mut key)?;
    Ok(Header { key, payload_len, digest })
}

fn digest_prefix(key: &[u8], payload_len: u64) -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(MAGIC); hash.update(1_u32.to_le_bytes());
    hash.update((key.len() as u32).to_le_bytes()); hash.update(payload_len.to_le_bytes()); hash.update(key);
    hash
}

pub(super) fn write_entry(writer: &mut impl Write, key: &[u8], payload: &[u8]) -> io::Result<()> {
    key_valid(key)?;
    if payload.len() as u64 > MAX_PAYLOAD { return Err(invalid("cache payload too large")); }
    let mut hash = digest_prefix(key, payload.len() as u64); hash.update(payload);
    writer.write_all(MAGIC)?; writer.write_all(&1_u32.to_le_bytes())?;
    writer.write_all(&(key.len() as u32).to_le_bytes())?;
    writer.write_all(&(payload.len() as u64).to_le_bytes())?;
    writer.write_all(&hash.finalize())?; writer.write_all(key)?; writer.write_all(payload)
}

pub(super) fn read_entry(reader: &mut impl Read, file_len: u64, key: &[u8]) -> io::Result<Vec<u8>> {
    let header = read_header(reader)?;
    if header.key != key || header.total_len() != file_len { return Err(invalid("cache key or file length")); }
    let mut payload = Vec::new();
    payload.try_reserve_exact(header.payload_len as usize).map_err(|_| invalid("cache allocation"))?;
    payload.resize(header.payload_len as usize, 0);
    reader.read_exact(&mut payload)?;
    let mut extra = [0];
    if reader.read(&mut extra)? != 0 { return Err(invalid("cache trailing bytes")); }
    let mut hash = digest_prefix(key, header.payload_len); hash.update(&payload);
    let actual: [u8; 32] = hash.finalize().into();
    if actual != header.digest { return Err(invalid("cache digest")); }
    Ok(payload)
}
