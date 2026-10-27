//! One direction of a simulated network link with latency, jitter, loss,
//! duplication and a bandwidth cap.
//!
//! Every random choice hashes (seed, datagram sequence, salt), so a run replays
//! exactly from its seed. Jitter is the only source of reordering, as on a real
//! path.
use engine_core::hash::hash_u32;
use std::{cmp::Reverse, collections::BinaryHeap};

/// Impairment for one direction. Zero `bytes_per_second` means unlimited.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Impairment {
    pub latency_us: u64,
    pub jitter_us: u64,
    pub loss_per_mille: u16,
    pub duplicate_per_mille: u16,
    pub bytes_per_second: u64,
    /// Longest the bottleneck queue may hold before tail drop.
    pub queue_us: u64,
}

impl Impairment {
    /// A home broadband path: 50 ms one way, 10 ms jitter, 1% loss.
    pub const BROADBAND: Self = Self {
        latency_us: 50_000,
        jitter_us: 10_000,
        loss_per_mille: 10,
        duplicate_per_mille: 1,
        bytes_per_second: 2_500_000,
        queue_us: 250_000,
    };
}

/// Counters for a bandwidth or loss report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkStats {
    pub sent: u64,
    pub sent_bytes: u64,
    pub lost: u64,
    pub queue_dropped: u64,
    pub duplicated: u64,
    pub delivered: u64,
    pub delivered_bytes: u64,
}

pub struct Link {
    impairment: Impairment,
    seed: u32,
    sequence: u32,
    /// Time the bottleneck finishes serialising everything accepted so far.
    busy_until_us: u64,
    in_flight: BinaryHeap<Reverse<(u64, u32, usize)>>,
    payloads: Vec<Option<Vec<u8>>>,
    free: Vec<usize>,
    stats: LinkStats,
}

const SALT_LOSS: u32 = 0x4c4f_5353;
const SALT_DUPLICATE: u32 = 0x4455_504c;
const SALT_JITTER: u32 = 0x4a49_5454;

impl Link {
    pub fn new(impairment: Impairment, seed: u32) -> Self {
        Self {
            impairment,
            seed,
            sequence: 0,
            busy_until_us: 0,
            in_flight: BinaryHeap::new(),
            payloads: Vec::new(),
            free: Vec::new(),
            stats: LinkStats::default(),
        }
    }

    pub fn stats(&self) -> LinkStats {
        self.stats
    }

    fn roll(&self, sequence: u32, salt: u32) -> u32 {
        hash_u32(self.seed ^ hash_u32(sequence.wrapping_mul(0x9e37_79b9) ^ salt))
    }

    fn per_mille(&self, sequence: u32, salt: u32, chance: u16) -> bool {
        chance > 0 && self.roll(sequence, salt) % 1000 < u32::from(chance)
    }

    fn enqueue(&mut self, deliver_us: u64, sequence: u32, payload: Vec<u8>) {
        let slot = match self.free.pop() {
            Some(slot) => {
                self.payloads[slot] = Some(payload);
                slot
            }
            None => {
                self.payloads.push(Some(payload));
                self.payloads.len() - 1
            }
        };
        self.in_flight.push(Reverse((deliver_us, sequence, slot)));
    }

    /// Offers one datagram to the link at `now_us`.
    pub fn send(&mut self, now_us: u64, payload: Vec<u8>) {
        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        self.stats.sent += 1;
        self.stats.sent_bytes += payload.len() as u64;
        let i = self.impairment;
        // Serialisation happens before loss: a dropped packet still used the wire.
        let start = self.busy_until_us.max(now_us);
        if i.bytes_per_second > 0 {
            if i.queue_us > 0 && start - now_us > i.queue_us {
                self.stats.queue_dropped += 1;
                return;
            }
            self.busy_until_us = start + payload.len() as u64 * 1_000_000 / i.bytes_per_second;
        } else {
            self.busy_until_us = start;
        }
        if self.per_mille(sequence, SALT_LOSS, i.loss_per_mille) {
            self.stats.lost += 1;
            return;
        }
        let jitter = |salt: u32| if i.jitter_us == 0 { 0 } else { u64::from(self.roll(sequence, salt)) % (i.jitter_us + 1) };
        let arrive = self.busy_until_us + i.latency_us;
        let (first, copy) = (arrive + jitter(SALT_JITTER), arrive + jitter(SALT_JITTER ^ 1));
        if self.per_mille(sequence, SALT_DUPLICATE, i.duplicate_per_mille) {
            self.stats.duplicated += 1;
            self.enqueue(copy, sequence, payload.clone());
        }
        self.enqueue(first, sequence, payload);
    }

    /// Next datagram due by `now_us`, in arrival order.
    pub fn poll(&mut self, now_us: u64) -> Option<Vec<u8>> {
        let &Reverse((due, _, slot)) = self.in_flight.peek()?;
        if due > now_us {
            return None;
        }
        self.in_flight.pop();
        let payload = self.payloads[slot].take()?;
        self.free.push(slot);
        self.stats.delivered += 1;
        self.stats.delivered_bytes += payload.len() as u64;
        Some(payload)
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(link: &mut Link, until_us: u64) -> Vec<(u64, Vec<u8>)> {
        let mut out = Vec::new();
        for t in (0..=until_us).step_by(1_000) {
            while let Some(p) = link.poll(t) {
                out.push((t, p));
            }
        }
        out
    }

    #[test]
    fn a_clean_link_delivers_everything_after_the_latency() {
        let mut link = Link::new(Impairment { latency_us: 40_000, ..Default::default() }, 7);
        for n in 0..10u8 {
            link.send(u64::from(n) * 1_000, vec![n]);
        }
        let got = drain(&mut link, 100_000);
        assert_eq!(got.iter().map(|(_, p)| p[0]).collect::<Vec<_>>(), (0..10).collect::<Vec<_>>());
        assert!(got.iter().all(|(t, p)| *t >= 40_000 + u64::from(p[0]) * 1_000));
    }

    #[test]
    fn the_same_seed_replays_the_same_losses_and_order() {
        let run = |seed| {
            let mut link = Link::new(Impairment::BROADBAND, seed);
            for n in 0..2_000u32 {
                link.send(u64::from(n) * 2_000, n.to_le_bytes().to_vec());
            }
            (drain(&mut link, 5_000_000), link.stats())
        };
        assert_eq!(run(11), run(11));
        assert_ne!(run(11).0, run(12).0);
    }

    #[test]
    fn loss_and_duplication_rates_match_the_setting() {
        let impairment = Impairment { loss_per_mille: 50, duplicate_per_mille: 20, ..Default::default() };
        let mut link = Link::new(impairment, 3);
        for n in 0..20_000u64 {
            link.send(n, vec![0]);
        }
        let s = link.stats();
        assert!((800..1200).contains(&s.lost), "{s:?}");
        assert!((280..520).contains(&s.duplicated), "{s:?}");
        assert_eq!(drain(&mut link, 30_000).len() as u64, s.sent - s.lost + s.duplicated);
    }

    #[test]
    fn jitter_reorders_but_never_delivers_early() {
        let impairment = Impairment { latency_us: 30_000, jitter_us: 20_000, ..Default::default() };
        let mut link = Link::new(impairment, 5);
        for n in 0..200u32 {
            link.send(u64::from(n) * 1_000, n.to_le_bytes().to_vec());
        }
        let got = drain(&mut link, 400_000);
        let order: Vec<u32> = got.iter().map(|(_, p)| u32::from_le_bytes(p[..4].try_into().unwrap())).collect();
        assert!(order.windows(2).any(|w| w[1] < w[0]), "expected some reordering");
        for (t, n) in got.iter().map(|(t, _)| *t).zip(&order) {
            assert!(t >= 30_000 + u64::from(*n) * 1_000);
        }
    }

    #[test]
    fn a_saturated_bottleneck_paces_then_tail_drops() {
        // 10 kB/s with a 100 ms queue: 1,000-byte datagrams take 100 ms each.
        let impairment = Impairment { bytes_per_second: 10_000, queue_us: 100_000, ..Default::default() };
        let mut link = Link::new(impairment, 1);
        for _ in 0..5 {
            link.send(0, vec![0; 1_000]);
        }
        let s = link.stats();
        assert_eq!(s.queue_dropped, 3, "{s:?}");
        let got = drain(&mut link, 1_000_000);
        assert_eq!(got.iter().map(|(t, _)| *t).collect::<Vec<_>>(), vec![100_000, 200_000]);
    }
}
