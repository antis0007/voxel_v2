use super::*;
use crate::link::{Impairment, Link};

const HZ: u32 = 30;
const TICK_US: u64 = 1_000_000 / HZ as u64;
/// The client's clock reads this far ahead of the server's.
const SKEW_US: u64 = 123_456_789;

/// Server ticks go down at 30 Hz; the client stamps an input every 10 ms and
/// sends it up. Returns, per input, the stamped tick minus the server tick at
/// arrival, after a 3 s warm-up.
fn margins(impairment: Impairment, seconds: u64) -> Vec<i64> {
    let (mut down, mut up) = (Link::new(impairment, 5), Link::new(impairment, 6));
    let mut clock = TickClock::new(HZ);
    let mut out = Vec::new();
    for now in (0..seconds * 1_000_000).step_by(1_000) {
        if now % TICK_US < 1_000 {
            down.send(now, (now / TICK_US).to_le_bytes().to_vec());
        }
        while let Some(p) = down.poll(now) {
            clock.observe(u64::from_le_bytes(p[..8].try_into().unwrap()), now + SKEW_US);
        }
        if now % 10_000 == 0
            && let Some(tick) = clock.advance(now + SKEW_US, 100.0, 5.0)
        {
            up.send(now, tick.to_le_bytes().to_vec());
        }
        while let Some(p) = up.poll(now) {
            if now > 3_000_000 {
                out.push(u64::from_le_bytes(p[..8].try_into().unwrap()) as i64 - (now / TICK_US) as i64);
            }
        }
    }
    out
}

#[test]
fn inputs_arrive_just_ahead_of_the_server_over_broadband() {
    let m = margins(Impairment::BROADBAND, 12);
    let late = m.iter().filter(|&&d| d < 0).count();
    assert!(late * 100 <= m.len(), "{late} of {} late", m.len());
    // Uplink 50 ms is covered by a 143 ms lead: about 3 ticks early, never more than 5.
    assert!(m.iter().all(|&d| d <= 5), "max lead {:?}", m.iter().max());
}

#[test]
fn a_route_change_is_slewed_without_steps_backward() {
    let mut clock = TickClock::new(HZ);
    let mut stamps = Vec::new();
    for ms in 0..6_000u64 {
        let now = ms * 1_000;
        // The path gets 100 ms shorter at 3 s.
        let delay = if ms < 3_000 { 150_000 } else { 50_000 };
        if now % TICK_US < 1_000 && now >= delay {
            clock.observe((now - delay) / TICK_US, now);
        }
        if let Some(t) = clock.advance(now, 100.0, 5.0) {
            stamps.push(t);
        }
    }
    assert!(stamps.windows(2).all(|w| w[1] >= w[0] && w[1] - w[0] <= 1));
}

#[test]
fn a_large_error_jumps() {
    let mut clock = TickClock::new(HZ);
    clock.observe(100, 10_000_000);
    let before = clock.advance(10_000_000, 50.0, 2.0).unwrap();
    // The server restarted far ahead.
    clock.observe(10_000, 10_100_000);
    let after = clock.advance(10_100_000, 50.0, 2.0).unwrap();
    assert!(before < 110 && after >= 10_000, "{before} -> {after}");
}

#[test]
fn tick_load_reports_rate_mean_worst_and_budget_share() {
    let mut load = TickLoad::new(120, 240);
    assert_eq!(load.load(), Load::default());
    for i in 0..240u64 {
        load.record(i * 8_333, if i == 100 { 20_000 } else { 4_000 });
    }
    let l = load.load();
    assert!((l.ticks_per_second - 120.0).abs() < 1.0, "{l:?}");
    assert!((l.mean_ms - 4.07).abs() < 0.05 && l.worst_ms == 20.0, "{l:?}");
    assert!((0.45..0.52).contains(&l.busy), "{l:?}");
}
