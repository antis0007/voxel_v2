//! Reliable ordered messages carried inside unreliable datagrams.
//!
//! Each message has an id. The sender keeps it until a datagram that carried it is
//! acknowledged, and resends it once a retransmit timeout passes without that.
//! The receiver buffers out-of-order ids and releases them in order exactly once.
//! Unlike a TCP stream, a lost datagram delays only the reliable messages behind
//! it, never the unreliable snapshots sharing the connection.
use crate::wire::{Reader, WireError, put_u64};
use std::collections::{HashMap, VecDeque};

/// Unacknowledged messages a sender may hold; also the receiver's reorder window.
pub const WINDOW: u32 = 1_024;
/// Largest message; bigger payloads go through the bulk transfer.
pub const MAX_MESSAGE: usize = 1_000;
/// Datagrams remembered for mapping acks back to messages.
const PACKETS: usize = 256;

struct Pending {
    id: u32,
    bytes: Vec<u8>,
    last_sent_us: Option<u64>,
    acked: bool,
}

#[derive(Default)]
pub struct ReliableSender {
    next_id: u32,
    queue: VecDeque<Pending>,
    carried: HashMap<u32, Vec<u32>>,
    carried_order: VecDeque<u32>,
}

impl ReliableSender {
    pub fn push(&mut self, bytes: Vec<u8>) -> Result<(), WireError> {
        if bytes.len() > MAX_MESSAGE {
            return Err(WireError("message too large"));
        }
        if self.queue.len() >= WINDOW as usize {
            return Err(WireError("reliable window full"));
        }
        self.queue.push_back(Pending { id: self.next_id, bytes, last_sent_us: None, acked: false });
        self.next_id = self.next_id.wrapping_add(1);
        Ok(())
    }

    pub fn unacked(&self) -> usize {
        self.queue.len()
    }

    /// Writes due messages, oldest first, within `budget` bytes, as part of datagram `packet`.
    pub fn write(&mut self, packet: u32, now_us: u64, rto_us: u64, budget: usize, out: &mut Vec<u8>) {
        let mut body = Vec::new();
        let mut ids = Vec::new();
        for p in self.queue.iter_mut().filter(|p| !p.acked) {
            if p.last_sent_us.is_some_and(|t| now_us.saturating_sub(t) < rto_us) {
                continue;
            }
            let mut entry = Vec::with_capacity(p.bytes.len() + 6);
            put_u64(&mut entry, u64::from(p.id));
            put_u64(&mut entry, p.bytes.len() as u64);
            entry.extend_from_slice(&p.bytes);
            if body.len() + entry.len() + 3 > budget {
                continue;
            }
            body.extend_from_slice(&entry);
            p.last_sent_us = Some(now_us);
            ids.push(p.id);
        }
        put_u64(out, ids.len() as u64);
        out.extend_from_slice(&body);
        if !ids.is_empty() {
            self.carried.insert(packet, ids);
            self.carried_order.push_back(packet);
            while self.carried_order.len() > PACKETS {
                if let Some(old) = self.carried_order.pop_front() {
                    self.carried.remove(&old);
                }
            }
        }
    }

    /// Datagram `packet` arrived; its messages are delivered.
    pub fn packet_acked(&mut self, packet: u32) {
        let Some(ids) = self.carried.remove(&packet) else { return };
        for p in self.queue.iter_mut().filter(|p| ids.contains(&p.id)) {
            p.acked = true;
        }
        while self.queue.front().is_some_and(|p| p.acked) {
            self.queue.pop_front();
        }
    }
}

#[derive(Default)]
pub struct ReliableReceiver {
    next: u32,
    held: HashMap<u32, Vec<u8>>,
}

impl ReliableReceiver {
    /// Reads one datagram's reliable section; returns messages now deliverable in order.
    pub fn read(&mut self, r: &mut Reader) -> Result<Vec<Vec<u8>>, WireError> {
        let count = r.u64()?;
        let mut incoming = Vec::new();
        for _ in 0..count {
            let id = r.u32()?;
            let len = r.u64()?;
            if len > MAX_MESSAGE as u64 {
                return Err(WireError("message too large"));
            }
            let bytes = (0..len).map(|_| r.u8()).collect::<Result<Vec<u8>, _>>()?;
            incoming.push((id, bytes));
        }
        for (id, bytes) in incoming {
            // Behind `next` wraps to a huge offset: an old duplicate.
            if id.wrapping_sub(self.next) < WINDOW {
                self.held.entry(id).or_insert(bytes);
            }
        }
        let mut out = Vec::new();
        while let Some(bytes) = self.held.remove(&self.next) {
            out.push(bytes);
            self.next = self.next.wrapping_add(1);
        }
        Ok(out)
    }
}
