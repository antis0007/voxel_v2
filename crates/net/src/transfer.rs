//! One ordered, checksummed transfer per peer. Callers own pacing and timeout.
use crate::protocol::ProtocolError;
pub const MAX_TRANSFER: usize = 16 * 1024 * 1024;
const CHUNK: usize = 880;
const HASH_SEED: u64 = 0xcbf29ce484222325;

pub fn checksum(bytes: &[u8]) -> u64 {
    extend_checksum(HASH_SEED, bytes)
}

fn extend_checksum(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
pub struct Sender {
    bytes: Vec<u8>,
    offset: usize,
    header: bool,
    checksum: u64,
}
impl Sender {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProtocolError> {
        if bytes.is_empty() || bytes.len() > MAX_TRANSFER {
            return Err(ProtocolError("transfer size limit"));
        }
        Ok(Self {
            checksum: checksum(&bytes),
            bytes,
            offset: 0,
            header: false,
        })
    }
    /// Advance only after the transport accepts this payload.
    pub fn next(&self) -> Option<Vec<u8>> {
        if !self.header {
            let mut out = vec![1];
            out.extend_from_slice(&(self.bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&self.checksum.to_le_bytes());
            Some(out)
        } else if self.offset < self.bytes.len() {
            let end = (self.offset + CHUNK).min(self.bytes.len());
            let mut out = vec![2];
            out.extend_from_slice(&(self.offset as u32).to_le_bytes());
            out.extend_from_slice(&self.bytes[self.offset..end]);
            Some(out)
        } else {
            None
        }
    }
    pub fn accepted(&mut self) {
        if !self.header {
            self.header = true;
        } else {
            self.offset = (self.offset + CHUNK).min(self.bytes.len());
        }
    }
}
#[derive(Default)]
pub struct Receiver {
    bytes: Vec<u8>,
    size: usize,
    expected: u64,
    checksum: u64,
}
impl Receiver {
    pub fn receive(&mut self, payload: &[u8]) -> Result<Option<Vec<u8>>, ProtocolError> {
        match payload.first() {
            Some(1) if payload.len() == 13 && self.size == 0 => {
                let size = u32::from_le_bytes(payload[1..5].try_into().unwrap()) as usize;
                if size == 0 || size > MAX_TRANSFER {
                    return Err(ProtocolError("transfer size limit"));
                }
                self.expected = u64::from_le_bytes(payload[5..13].try_into().unwrap());
                // Grow only as bytes arrive; a header alone cannot allocate 16 MiB.
                self.bytes.clear();
                self.size = size;
                self.checksum = HASH_SEED;
            }
            Some(2) if payload.len() > 5 && payload.len() <= CHUNK + 5 && self.size > 0 => {
                let offset = u32::from_le_bytes(payload[1..5].try_into().unwrap()) as usize;
                if offset != self.bytes.len()
                    || payload.len() - 5 > self.size - offset.min(self.size)
                {
                    return Err(ProtocolError("transfer offset mismatch"));
                }
                self.bytes.extend_from_slice(&payload[5..]);
                self.checksum = extend_checksum(self.checksum, &payload[5..]);
                if self.bytes.len() == self.size {
                    self.size = 0;
                    let bytes = std::mem::take(&mut self.bytes);
                    if self.checksum != self.expected {
                        return Err(ProtocolError("transfer checksum mismatch"));
                    }
                    return Ok(Some(bytes));
                }
            }
            _ => return Err(ProtocolError("unexpected transfer payload")),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
