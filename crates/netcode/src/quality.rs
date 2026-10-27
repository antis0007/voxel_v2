//! Live connection quality from datagram acknowledgements: round trip, jitter,
//! loss and throughput, and a retransmit timeout for the reliable channel.
//!
//! Round trip follows RFC 6298 (smoothed mean plus mean deviation). Loss counts
//! datagrams unacknowledged past the timeout, over the last 256 that resolved, so
//! the player list shows a link's state now rather than a keepalive from seconds ago.
use std::collections::VecDeque;
use std::hash::{BuildHasher, RandomState};

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

/// RFC 6298 smoothing of one round-trip sample.
fn smooth(srtt_us: &mut Option<f64>, rttvar_us: &mut f64, sample_us: u64) {
    let sample = sample_us as f64;
    match *srtt_us {
        None => {
            *srtt_us = Some(sample);
            *rttvar_us = sample / 2.0;
        }
        Some(srtt) => {
            *rttvar_us = 0.75 * *rttvar_us + 0.25 * (srtt - sample).abs();
            *srtt_us = Some(0.875 * srtt + 0.125 * sample);
        }
    }
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
                smooth(&mut self.srtt_us, &mut self.rttvar_us, now_us.saturating_sub(sent.at_us));
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

/// Echo not returned after this long counts as a stall.
pub const STALL_US: u64 = 2_000_000;
const PROBES: usize = 64;

/// Ping echo for stream transports such as the TCP lobby relay, which have no
/// datagram acks. The host sends `probe` ids; the guest returns each unchanged.
/// A stream never drops a probe, so one still missing after `STALL_US` is a
/// stall, the stream's form of loss. Ids come from SipHash under a 128-bit key the
/// OS supplies per probe, so a guest cannot predict one to echo it early and show
/// a lower ping. Ids are not simulation state, so they need not replay.
pub struct EchoProbe {
    key: RandomState,
    counter: u32,
    srtt_us: Option<f64>,
    rttvar_us: f64,
    outstanding: VecDeque<(u32, u64)>,
    outcomes: VecDeque<bool>,
    stalled: usize,
}

impl Default for EchoProbe {
    fn default() -> Self {
        Self {
            key: RandomState::new(),
            counter: 0,
            srtt_us: None,
            rttvar_us: 0.0,
            outstanding: VecDeque::new(),
            outcomes: VecDeque::new(),
            stalled: 0,
        }
    }
}

impl EchoProbe {
    fn record(&mut self, stalled: bool) {
        self.outcomes.push_back(stalled);
        self.stalled += usize::from(stalled);
        if self.outcomes.len() > PROBES && self.outcomes.pop_front() == Some(true) {
            self.stalled -= 1;
        }
    }

    fn resolve(&mut self, now_us: u64) {
        while self.outstanding.front().is_some_and(|&(_, at)| now_us.saturating_sub(at) > STALL_US) {
            self.outstanding.pop_front();
            self.record(true);
        }
    }

    /// Starts a probe; the caller sends the returned id.
    pub fn probe(&mut self, now_us: u64) -> u32 {
        self.resolve(now_us);
        self.counter = self.counter.wrapping_add(1);
        let id = self.key.hash_one(self.counter) as u32;
        self.outstanding.push_back((id, now_us));
        if self.outstanding.len() > PROBES {
            self.outstanding.pop_front();
            self.record(true);
        }
        id
    }

    /// An echo arrived; false for an id not outstanding (late, repeated or forged).
    pub fn echo(&mut self, id: u32, now_us: u64) -> bool {
        let Some(i) = self.outstanding.iter().position(|&(p, _)| p == id) else { return false };
        let Some((_, at)) = self.outstanding.remove(i) else { return false };
        smooth(&mut self.srtt_us, &mut self.rttvar_us, now_us.saturating_sub(at));
        self.record(false);
        true
    }

    /// Round trip, jitter and stalls; throughput is not measured here.
    pub fn quality(&mut self, now_us: u64) -> Quality {
        self.resolve(now_us);
        Quality {
            rtt_ms: self.srtt_us.unwrap_or(0.0) as f32 / 1_000.0,
            jitter_ms: self.rttvar_us as f32 / 1_000.0,
            loss_per_mille: (self.stalled * 1_000 / self.outcomes.len().max(1)) as u16,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests;
