//! Deterministic link simulator.
//!
//! Drives the real host session and the real predictor across a link with
//! configured latency and jitter, and reports what a player would feel:
//! corrections, late inputs, frame age and bandwidth. No sockets, no clock reads
//! beyond a single base instant, so a failing run reproduces exactly.
//!
//! The relay transport is reliable and ordered, so this models delay and reorder
//! of arrival ticks, not loss. Packet loss would be a different transport.

use crate::predict::Predictor;
use crate::protocol::{Input, Packet, WorldIdentity};
use crate::session::HostSession;
use std::time::{Duration, Instant};

pub const TICK_HZ: u64 = 30;

fn ticks_for(ms: u64) -> u64 {
    ms.saturating_mul(TICK_HZ).div_ceil(1000)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkConfig {
    pub up_ticks: u64,
    pub down_ticks: u64,
    /// Extra arrival delay in ticks, 0 through `jitter_ticks`.
    pub jitter_ticks: u64,
    pub seed: u32,
}

impl LinkConfig {
    /// Split a round trip into both directions and convert to ticks.
    pub fn from_rtt(rtt_ms: u64, jitter_ms: u64) -> Self {
        Self {
            up_ticks: ticks_for(rtt_ms / 2),
            down_ticks: ticks_for(rtt_ms - rtt_ms / 2),
            jitter_ticks: ticks_for(jitter_ms),
            seed: 0x9e37_79b9,
        }
    }

    pub fn lan() -> Self {
        Self::from_rtt(2, 0)
    }

    pub fn broadband() -> Self {
        Self::from_rtt(80, 20)
    }

    pub fn poor() -> Self {
        Self::from_rtt(200, 60)
    }
}

/// Deterministic offset in `0..=jitter_ticks`, stable for a tick and direction.
fn jitter_offset(config: LinkConfig, tick: u64, salt: u32) -> u64 {
    if config.jitter_ticks == 0 {
        return 0;
    }
    let mut h = (tick as u32)
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(config.seed)
        .wrapping_add(salt);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    u64::from(h) % (config.jitter_ticks + 1)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub ticks: u64,
    pub inputs_sent: u64,
    /// Inputs the host actually used. The rest arrived after their tick.
    pub inputs_applied: u64,
    pub frames: u64,
    pub reconciles: u64,
    /// Reconciles where replay changed the predicted state.
    pub corrections: u64,
    /// Largest single correction, in the simulator's units.
    pub worst_correction: i32,
    pub client_bytes: u64,
    pub host_bytes: u64,
    /// Ticks between a frame leaving the host and being used by the client.
    pub worst_frame_age: u64,
}

impl Report {
    pub fn inputs_late(&self) -> u64 {
        self.inputs_sent.saturating_sub(self.inputs_applied)
    }

    pub fn host_kib_per_s(&self) -> f32 {
        if self.ticks == 0 {
            return 0.0;
        }
        let seconds = self.ticks as f32 / TICK_HZ as f32;
        self.host_bytes as f32 / 1024.0 / seconds
    }

    pub fn client_kib_per_s(&self) -> f32 {
        if self.ticks == 0 {
            return 0.0;
        }
        let seconds = self.ticks as f32 / TICK_HZ as f32;
        self.client_bytes as f32 / 1024.0 / seconds
    }

    pub fn corrections_per_minute(&self) -> f32 {
        if self.ticks == 0 {
            return 0.0;
        }
        self.corrections as f32 * 60.0 * TICK_HZ as f32 / self.ticks as f32
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SimConfig {
    pub link: LinkConfig,
    pub lead_ticks: u64,
    pub ticks: u64,
    /// Movement applied per tick, so a host wall can be reached.
    pub stride: i16,
}

/// One client against one host over the configured link.
///
/// The client predicts with its own step and reconciles against the host's
/// recorded state for the frame's tick. Arrival ticks are clamped so each
/// direction stays ordered, which is what a reliable stream does.
pub fn run(
    config: SimConfig,
    client_step: &impl Fn(&i32, &Input) -> i32,
    host_step: &impl Fn(&i32, &Input) -> i32,
) -> Report {
    let base = Instant::now();
    let at = |tick: u64| base + Duration::from_millis(tick * 1000 / TICK_HZ);
    let world = WorldIdentity { seed: 1, generator: 1, cell_mm: 1000, content: [0; 32] };
    let mut host = HostSession::new(world);
    let id = host.connect(at(0)).expect("connect");
    host.receive(id, Packet::Hello(world), at(0)).expect("hello");

    let mut predictor: Predictor<i32> = Predictor::new(0);
    let mut truth = 0i32;
    let mut truth_by_tick: Vec<(u64, i32)> = vec![(0, 0)];
    let mut report = Report { ticks: config.ticks, ..Default::default() };
    let mut to_host: Vec<(u64, Packet)> = Vec::new();
    let mut to_client: Vec<(u64, u64, Packet)> = Vec::new();
    let mut last_up = 0u64;
    let mut last_down = 0u64;

    for tick in 1..=config.ticks {
        // Client: react now, send the intent for the tick it is aimed at.
        let intent_tick = tick + config.lead_ticks;
        let input = Input {
            sequence: tick,
            tick: intent_tick,
            movement: [config.stride, 0],
            look: [0, 0],
            buttons: 0,
        };
        predictor.push(intent_tick, input.clone(), client_step);
        // Intents aimed past the final tick are run-off, not lateness.
        if intent_tick <= config.ticks {
            report.inputs_sent += 1;
        }
        report.client_bytes += Packet::Input(input.clone()).encode().expect("encode").len() as u64;
        let due = (tick + config.link.up_ticks + jitter_offset(config.link, tick, 1)).max(last_up);
        last_up = due;
        to_host.push((due, Packet::Input(input)));

        // Host: take arrivals, then advance one authoritative tick.
        let mut index = 0;
        while index < to_host.len() {
            if to_host[index].0 <= tick {
                let (_, packet) = to_host.remove(index);
                let _ = host.receive(id, packet, at(tick));
            } else {
                index += 1;
            }
        }

        let frame = host.advance().expect("advance");
        let mut frame_tick = 0;
        if let Packet::Frame { tick: produced, players } = &frame {
            frame_tick = *produced;
            for (player, input) in players {
                if *player != id {
                    continue;
                }
                if let Some(input) = input {
                    truth = host_step(&truth, input);
                    report.inputs_applied += 1;
                }
            }
        }
        truth_by_tick.push((frame_tick, truth));
        report.frames += 1;
        report.host_bytes += frame.encode().expect("encode").len() as u64;
        let due = (tick + config.link.down_ticks + jitter_offset(config.link, tick, 2)).max(last_down);
        last_down = due;
        to_client.push((due, tick, frame));

        // Client: reconcile arriving frames against the host's state for that tick.
        let mut index = 0;
        while index < to_client.len() {
            if to_client[index].0 <= tick {
                let (_, sent, packet) = to_client.remove(index);
                if let Packet::Frame { tick: frame_tick, .. } = packet {
                    let authoritative = truth_by_tick
                        .iter()
                        .rev()
                        .find(|(recorded, _)| *recorded <= frame_tick)
                        .map(|(_, state)| *state)
                        .unwrap_or(0);
                    let before = *predictor.state();
                    let report_reconcile = predictor.reconcile(frame_tick, authoritative, client_step);
                    if !report_reconcile.stale {
                        report.reconciles += 1;
                        let after = *predictor.state();
                        if after != before {
                            report.corrections += 1;
                            report.worst_correction =
                                report.worst_correction.max((after - before).abs());
                        }
                    }
                    report.worst_frame_age = report.worst_frame_age.max(tick.saturating_sub(sent));
                }
            } else {
                index += 1;
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn honest(pos: &i32, input: &Input) -> i32 {
        pos + i32::from(input.movement[0])
    }

    fn walled(pos: &i32, input: &Input) -> i32 {
        (pos + i32::from(input.movement[0])).min(50)
    }

    fn config(link: LinkConfig, lead_ticks: u64) -> SimConfig {
        SimConfig { link, lead_ticks, ticks: 300, stride: 2 }
    }

    #[test]
    fn a_lan_link_never_corrects_an_honest_client() {
        let report = run(config(LinkConfig::lan(), 3), &honest, &honest);
        assert_eq!(report.corrections, 0, "{report:?}");
        assert_eq!(report.inputs_late(), 0, "{report:?}");
        assert!(report.reconciles > 250, "frames must reach the client: {report:?}");
        assert_eq!(report.worst_frame_age, 1, "one downlink tick");
        assert!(report.host_kib_per_s() > 0.0 && report.host_kib_per_s() < 12.0, "{report:?}");
    }

    #[test]
    fn a_fixed_short_lead_loses_inputs_on_a_poor_link() {
        let report = run(config(LinkConfig::poor(), 3), &honest, &honest);
        assert!(
            report.inputs_late() > report.inputs_sent / 2,
            "a 3 tick lead cannot cover a 200 ms round trip: {report:?}"
        );
        // The client predicts motion the host never saw, so it is corrected on
        // most frames. This is the cost of an under-sized lead, and it is why
        // the lead follows a measured round trip.
        assert!(report.corrections > report.reconciles / 2, "{report:?}");
    }

    #[test]
    fn a_lead_that_covers_the_round_trip_keeps_every_input() {
        let report = run(config(LinkConfig::poor(), 8), &honest, &honest);
        assert_eq!(report.inputs_late(), 0, "{report:?}");
        assert_eq!(report.corrections, 0, "{report:?}");
        assert!(report.worst_frame_age >= 3, "the link is still slow: {report:?}");
    }

    #[test]
    fn a_client_without_collision_knowledge_is_corrected() {
        let report = run(config(LinkConfig::broadband(), 3), &honest, &walled);
        assert!(report.corrections > 0, "{report:?}");
        assert!(report.worst_correction > 0, "{report:?}");
        // The host wall holds however long the run: the correction never grows
        // without bound because every reconcile snaps back to authority.
        assert!(report.worst_correction <= 60, "{report:?}");
    }

    #[test]
    fn bandwidth_is_reported_in_kibibytes_per_second() {
        let report = run(SimConfig { link: LinkConfig::lan(), lead_ticks: 3, ticks: 30, stride: 1 }, &honest, &honest);
        assert_eq!(report.host_kib_per_s() as u32, report.host_kib_per_s().floor() as u32);
        assert!(report.client_kib_per_s() <= report.host_kib_per_s(), "{report:?}");
        assert_eq!(report.corrections_per_minute(), 0.0);
    }

    #[test]
    fn jitter_offsets_are_deterministic_and_bounded() {
        let link = LinkConfig::broadband();
        assert_eq!(jitter_offset(link, 17, 1), jitter_offset(link, 17, 1));
        for tick in 0..200 {
            assert!(jitter_offset(link, tick, 2) <= link.jitter_ticks);
        }
    }
}
