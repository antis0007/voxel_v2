//! Live connection quality from datagram acknowledgements: round trip, jitter,
//! loss and throughput, and a retransmit timeout for the reliable channel.
//!
//! Round trip follows RFC 6298 (smoothed mean plus mean deviation). Loss counts
//! datagrams unacknowledged past the timeout, over the last 256 that resolved, so
//! the player list shows a link's state now rather than a keepalive from seconds ago.
use std::collections::VecDeque;

const WINDOW: usize = 256;
const RATE_US: u64 = 1_000_000;
const MIN_RTO_US: f64 = 50_000.0;
const MAX_RTO_US: f64 = 2_000_000.0;

/// One connection's figures for display and pacing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quality {
    pub rtt_ms: f32,
    /// Mean deviation of the round trip.
    pub jitter_ms: f32,
    pub loss_per_mille: u16,
    pub up_bytes_per_s: u32,
    pub down_bytes_per_s: u32,
}

impl Quality {
    /// Signal bars 0..=5. Jitter and loss count as added delay, since both show up
    /// as late or repeated state.
    pub fn bars(&self) -> u8 {
        let felt = self.rtt_ms + 2.0 * self.jitter_ms + f32::from(self.loss_per_mille) * 4.0;
        match felt {
            f if f < 60.0 => 5,
            f if f < 120.0 => 4,
            f if f < 220.0 => 3,
            f if f < 400.0 => 2,
            f if f < 1_000.0 => 1,
            _ => 0,
        }
    }

    /// Wire form for the player list: 6 bytes.
    pub fn encode(&self) -> [u8; 6] {
        let rtt = (self.rtt_ms.round() as u32).min(u32::from(u16::MAX)) as u16;
        let jitter = (self.jitter_ms.round() as u32).min(255) as u8;
        let loss = self.loss_per_mille.min(1_000);
        let mut out = [0u8; 6];
        out[..2].copy_from_slice(&rtt.to_le_bytes());
        out[2] = jitter;
        out[3..5].copy_from_slice(&loss.to_le_bytes());
        out[5] = self.bars();
        out
    }

    /// Reads the player-list form; throughput is not carried.
    pub fn decode(bytes: [u8; 6]) -> Option<Self> {
        let loss = u16::from_le_bytes([bytes[3], bytes[4]]);
        (loss <= 1_000 && bytes[5] <= 5).then(|| Self {
            rtt_ms: f32::from(u16::from_le_bytes([bytes[0], bytes[1]])),
            jitter_ms: f32::from(bytes[2]),
            loss_per_mille: loss,
            ..Default::default()
        })
    }
}

#[derive(Clone, Copy)]
struct Sent {
    seq: u32,
    at_us: u64,
    acked: bool,
}

/// Sender-side estimator, fed by sends and acks of datagram sequences.
#[derive(Default)]
pub struct LinkQuality {
    srtt_us: Option<f64>,
    rttvar_us: f64,
    in_flight: VecDeque<Sent>,
    /// Resolved datagrams, newest last: true when lost.
    outcomes: VecDeque<bool>,
    lost: usize,
    up: VecDeque<(u64, u32)>,
    down: VecDeque<(u64, u32)>,
}

fn rate(log: &mut VecDeque<(u64, u32)>, now_us: u64) -> u32 {
    while log.front().is_some_and(|&(t, _)| now_us.saturating_sub(t) >= RATE_US) {
        log.pop_front();
    }
    log.iter().map(|&(_, b)| b).sum()
}

impl LinkQuality {
    pub fn on_send(&mut self, seq: u32, now_us: u64, bytes: usize) {
        self.in_flight.push_back(Sent { seq, at_us: now_us, acked: false });
        self.up.push_back((now_us, u32::try_from(bytes).unwrap_or(u32::MAX)));
        self.resolve(now_us);
    }

    pub fn on_receive(&mut self, now_us: u64, bytes: usize) {
        self.down.push_back((now_us, u32::try_from(bytes).unwrap_or(u32::MAX)));
    }

    /// The peer received `newest` and each `newest - 1 - i` whose bit `i` is set.
    pub fn on_ack(&mut self, newest: u32, earlier: u32, now_us: u64) {
        for sent in self.in_flight.iter_mut().filter(|s| !s.acked) {
            let back = newest.wrapping_sub(sent.seq);
            let hit = back == 0 || (1..=32).contains(&back) && earlier & (1 << (back - 1)) != 0;
            if !hit {
                continue;
            }
            sent.acked = true;
            // Only the newest gives a clean sample; older ones waited for a later datagram.
            if back == 0 {
                let sample = now_us.saturating_sub(sent.at_us) as f64;
                match self.srtt_us {
                    None => {
                        self.srtt_us = Some(sample);
                        self.rttvar_us = sample / 2.0;
                    }
                    Some(srtt) => {
                        self.rttvar_us = 0.75 * self.rttvar_us + 0.25 * (srtt - sample).abs();
                        self.srtt_us = Some(0.875 * srtt + 0.125 * sample);
                    }
                }
            }
        }
        self.resolve(now_us);
    }

    /// Moves acknowledged and timed-out datagrams into the loss window.
    fn resolve(&mut self, now_us: u64) {
        let timeout = self.rto_us();
        while let Some(&front) = self.in_flight.front() {
            let expired = now_us.saturating_sub(front.at_us) > timeout;
            if !front.acked && !expired {
                break;
            }
            self.in_flight.pop_front();
            self.outcomes.push_back(!front.acked);
            self.lost += usize::from(!front.acked);
            if self.outcomes.len() > WINDOW && self.outcomes.pop_front() == Some(true) {
                self.lost -= 1;
            }
        }
    }

    /// Retransmit timeout: smoothed round trip plus four deviations, 50 ms to 2 s.
    pub fn rto_us(&self) -> u64 {
        match self.srtt_us {
            Some(srtt) => (srtt + 4.0 * self.rttvar_us).clamp(MIN_RTO_US, MAX_RTO_US) as u64,
            None => 500_000,
        }
    }

    pub fn quality(&mut self, now_us: u64) -> Quality {
        self.resolve(now_us);
        let resolved = self.outcomes.len().max(1);
        Quality {
            rtt_ms: self.srtt_us.unwrap_or(0.0) as f32 / 1_000.0,
            jitter_ms: self.rttvar_us as f32 / 1_000.0,
            loss_per_mille: (self.lost * 1_000 / resolved) as u16,
            up_bytes_per_s: rate(&mut self.up, now_us),
            down_bytes_per_s: rate(&mut self.down, now_us),
        }
    }
}

#[cfg(test)]
mod tests;
