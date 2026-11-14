//! Per-peer outbound pacing. Realtime messages (poses, corrections, roster, frames) bypass it
//! and go first. Images, cell deltas and terrain then share a byte budget over wall time.
//! A busy city's edits must not starve terrain streaming to an already joined player.
use anyhow::Result;
use std::time::Instant;

/// Bytes per second per joined peer for its image and cell deltas.
// Gameplay images include combat state. At 64 KiB/s, a 32 KiB image
// contending with terrain took a full second, despite the 100ms image cadence.
pub const BULK_BYTES_PER_SECOND: f64 = 256.0 * 1024.0;
/// Bytes per second per peer while it loads the world; nothing else competes then.
pub const TRANSFER_BYTES_PER_SECOND: f64 = 256.0 * 1024.0;
/// Unspent budget a quiet peer may bank, so a short burst goes out without waiting.
const BURST_SECONDS: f64 = 0.25;
/// Shared link slots kept free for realtime messages and session frames: the relay
/// removes every peer whose frame finds the link full.
pub const LINK_RESERVE: usize = 96;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lane {
    /// The joiner's own inventory, health and machines.
    Image,
    /// Cell changes.
    Deltas,
    /// Initial world transfer or terrain streamed after joining.
    Bulk,
}

pub struct Pacer {
    rate: f64,
    tokens: f64,
    last: Option<Instant>,
    /// Weighted bytes served; an image byte costs half a delta or terrain byte.
    service: [u64; 3],
}

impl Pacer {
    pub fn new(rate: f64) -> Self {
        Self { rate, tokens: rate * BURST_SECONDS, last: None, service: [0; 3] }
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
    }

    fn refill(&mut self, now: Instant) {
        if let Some(last) = self.last {
            let elapsed = now.saturating_duration_since(last).as_secs_f64();
            self.tokens = (self.tokens + elapsed * self.rate).min(self.rate * BURST_SECONDS);
        }
        self.last = Some(now);
    }

    /// Concurrent images, edits and terrain receive 2:1:1 byte shares.
    fn pick(&self, open: [bool; 3]) -> Option<Lane> {
        [Lane::Image, Lane::Deltas, Lane::Bulk].into_iter()
            .filter(|lane| open[*lane as usize])
            .min_by_key(|lane| self.service[*lane as usize])
    }

    fn normalize(&mut self, open: [bool; 3]) {
        let floor = self.service.iter().enumerate()
            .filter(|(index, _)| open[*index])
            .map(|(_, &bytes)| bytes).min().unwrap_or(0);
        for (index, bytes) in self.service.iter_mut().enumerate() {
            *bytes = if open[index] { bytes.saturating_sub(floor) } else { 0 };
        }
    }

    /// Sends from the lanes until the byte budget, `room` (shared link slots) or every lane
    /// runs out. `send` queues one message from a lane and returns its size, or `None` when
    /// that lane has nothing it can send now. One message may overdraw the budget, so a
    /// message larger than a frame's share never stalls.
    pub fn drain(&mut self, now: Instant, room: &mut usize, mut send: impl FnMut(Lane) -> Result<Option<usize>>) -> Result<()> {
        self.refill(now);
        let mut open = [true; 3];
        while self.tokens > 0.0 && *room > 0 {
            let Some(lane) = self.pick(open) else { break };
            match send(lane)? {
                Some(bytes) => {
                    self.tokens -= bytes as f64;
                    *room -= 1;
                    let weight = if lane == Lane::Image { 1 } else { 2 };
                    self.service[lane as usize] += bytes as u64 * weight;
                }
                None => {
                    open[lane as usize] = false;
                }
            }
        }
        // Idle or backpressured lanes bank no credit for a later burst.
        self.normalize(open);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Bytes each lane sends over `seconds` at `fps`; `lanes` marks the lanes that have data.
    fn run(pacer: &mut Pacer, seconds: f64, fps: f64, lanes: [bool; 3]) -> [usize; 3] {
        let start = Instant::now();
        let mut sent = [0; 3];
        let frames = (seconds * fps) as u32;
        for f in 0..=frames {
            let now = start + Duration::from_secs_f64(f64::from(f) / fps);
            let mut room = usize::MAX;
            pacer
                .drain(now, &mut room, |lane| {
                    Ok(lanes[lane as usize].then(|| {
                        sent[lane as usize] += 880;
                        880
                    }))
                })
                .expect("drain");
        }
        sent
    }

    #[test]
    fn budget_follows_wall_time_not_frame_rate() {
        for fps in [30.0, 144.0, 500.0] {
            let mut pacer = Pacer::new(BULK_BYTES_PER_SECOND);
            let total: usize = run(&mut pacer, 10.0, fps, [false, true, false]).iter().sum();
            let expected = BULK_BYTES_PER_SECOND * (10.0 + BURST_SECONDS);
            assert!((total as f64 - expected).abs() < expected * 0.02, "{fps} fps sent {total}");
        }
    }

    #[test]
    fn ongoing_edits_and_images_do_not_starve_terrain() {
        let mut pacer = Pacer::new(BULK_BYTES_PER_SECOND);
        let [image, deltas, bulk] = run(&mut pacer, 5.0, 144.0, [true, true, true]);
        assert!(bulk as f64 > BULK_BYTES_PER_SECOND * 1.2);
        assert!(image.abs_diff(deltas * 2) <= 1760, "image {image} deltas {deltas}");
        assert!(bulk.abs_diff(deltas) <= 880, "bulk {bulk} deltas {deltas}");
        // Alone, either lane takes the whole budget.
        let [image, ..] = run(&mut Pacer::new(BULK_BYTES_PER_SECOND), 5.0, 144.0, [true, false, false]);
        assert!(image as f64 > BULK_BYTES_PER_SECOND * 4.9);
    }

    #[test]
    fn link_room_stops_sending() {
        let mut pacer = Pacer::new(1.0e9);
        let mut room = 5;
        let mut count = 0;
        pacer
            .drain(Instant::now(), &mut room, |_| {
                count += 1;
                Ok(Some(100))
            })
            .expect("drain");
        assert_eq!((count, room), (5, 0));
    }
}
