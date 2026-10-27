//! One end of a datagram connection: sequence, piggybacked acks, reliable
//! messages and an unreliable payload in every datagram.
//!
//! Layout: sequence, newest received sequence plus one (0 = none), a 32-bit mask
//! of the ones before it, the reliable section, then the unreliable payload
//! (snapshots, poses) to the end. Every datagram acknowledges the peer's last 33,
//! so a lost ack costs nothing while traffic flows both ways.
use crate::quality::{LinkQuality, Quality};
use crate::reliable::{ReliableReceiver, ReliableSender};
use crate::wire::{Reader, WireError, put_u64};

#[derive(Debug, PartialEq, Eq)]
pub struct Incoming {
    pub sequence: u32,
    pub reliable: Vec<Vec<u8>>,
    /// Empty when the datagram arrived after a newer one; stale state is not shown.
    pub unreliable: Vec<u8>,
}

#[derive(Default)]
pub struct Connection {
    sequence: u32,
    newest: Option<u32>,
    mask: u32,
    quality: LinkQuality,
    sender: ReliableSender,
    receiver: ReliableReceiver,
}

impl Connection {
    pub fn push_reliable(&mut self, bytes: Vec<u8>) -> Result<(), WireError> {
        self.sender.push(bytes)
    }

    pub fn unacked_reliable(&self) -> usize {
        self.sender.unacked()
    }

    pub fn quality(&mut self, now_us: u64) -> Quality {
        self.quality.quality(now_us)
    }

    /// Builds one datagram of at most `budget` bytes, unreliable payload included.
    pub fn send(&mut self, now_us: u64, unreliable: &[u8], budget: usize) -> Vec<u8> {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        let mut out = Vec::with_capacity(budget);
        put_u64(&mut out, u64::from(seq));
        put_u64(&mut out, self.newest.map_or(0, |n| u64::from(n) + 1));
        out.extend(self.mask.to_le_bytes());
        let room = budget.saturating_sub(out.len() + unreliable.len());
        let rto = self.quality.rto_us();
        self.sender.write(seq, now_us, rto, room, &mut out);
        out.extend_from_slice(unreliable);
        self.quality.on_send(seq, now_us, out.len());
        out
    }

    /// Reads one datagram; `None` for a duplicate.
    pub fn receive(&mut self, now_us: u64, bytes: &[u8]) -> Result<Option<Incoming>, WireError> {
        let mut r = Reader::new(bytes);
        let seq = r.u32()?;
        // Validated before anything is applied, so an error loses no messages.
        let acked = match r.u64()? {
            0 => None,
            v => Some(u32::try_from(v - 1).map_err(|_| WireError("ack out of range"))?),
        };
        let mask = u32::from_le_bytes([r.u8()?, r.u8()?, r.u8()?, r.u8()?]);
        let back = self.newest.map(|n| n.wrapping_sub(seq));
        let duplicate = match back {
            Some(0) => true,
            Some(b) if (1..=32).contains(&b) => self.mask & (1 << (b - 1)) != 0,
            _ => false,
        };
        if duplicate {
            return Ok(None);
        }
        let reliable = self.receiver.read(&mut r)?;
        let rest = &bytes[bytes.len() - r.remaining()..];
        self.quality.on_receive(now_us, bytes.len());

        if let Some(newest) = acked {
            self.sender.packet_acked(newest);
            (1..=32u32).filter(|i| mask & (1 << (i - 1)) != 0).for_each(|i| self.sender.packet_acked(newest.wrapping_sub(i)));
            self.quality.on_ack(newest, mask, now_us);
        }

        // Older than the newest by more than half the sequence space reads as newer.
        let is_newer = back.is_none_or(|b| b >= 1 << 31);
        if is_newer {
            let shift = self.newest.map_or(0, |n| seq.wrapping_sub(n));
            self.mask = match self.newest {
                None => 0,
                Some(_) if shift > 32 => 0,
                Some(_) => ((u64::from(self.mask) << shift) | 1 << (shift - 1)) as u32,
            };
            self.newest = Some(seq);
        } else if let Some(b) = back.filter(|b| (1..=32).contains(b)) {
            self.mask |= 1 << (b - 1);
        }
        let unreliable = if is_newer { rest.to_vec() } else { Vec::new() };
        Ok(Some(Incoming { sequence: seq, reliable, unreliable }))
    }
}

#[cfg(test)]
mod tests;
