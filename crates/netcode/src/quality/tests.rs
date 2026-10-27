use super::*;
use crate::link::{Impairment, Link};

/// Sends a 200-byte datagram every 1/60 s for `seconds`; the peer acks each
/// arrival with its newest sequence and a 32-bit mask. Returns the final figures.
fn measure(impairment: Impairment, seconds: u64, seed: u32) -> Quality {
    let (mut down, mut up) = (Link::new(impairment, seed), Link::new(impairment, seed ^ 7));
    let mut q = LinkQuality::default();
    let (mut newest, mut mask): (Option<u32>, u32) = (None, 0);
    let mut seq = 0u32;
    let end = seconds * 1_000_000;
    for now in (0..end).step_by(1_000) {
        if now % 16_667 < 1_000 {
            q.on_send(seq, now, 200);
            down.send(now, seq.to_le_bytes().to_vec());
            seq += 1;
        }
        while let Some(p) = down.poll(now) {
            let s = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
            match newest {
                Some(n) if s.wrapping_sub(n) < 1 << 31 && s != n => {
                    let shift = s - n;
                    mask = if shift > 32 { 0 } else { ((u64::from(mask) << shift) | 1 << (shift - 1)) as u32 };
                    newest = Some(s);
                }
                Some(n) if n != s && (1..=32).contains(&(n - s)) => mask |= 1 << (n - s - 1),
                Some(_) => {}
                None => newest = Some(s),
            }
            let n = newest.unwrap_or(s);
            up.send(now, [n.to_le_bytes(), mask.to_le_bytes()].concat());
        }
        while let Some(a) = up.poll(now) {
            let word = |i: usize| u32::from_le_bytes([a[i], a[i + 1], a[i + 2], a[i + 3]]);
            q.on_receive(now, a.len());
            q.on_ack(word(0), word(4), now);
        }
    }
    q.quality(end)
}

#[test]
fn a_clean_link_reads_its_round_trip_and_no_loss() {
    let q = measure(Impairment { latency_us: 15_000, ..Default::default() }, 5, 1);
    assert!((q.rtt_ms - 30.0).abs() < 2.0, "{q:?}");
    assert!(q.jitter_ms < 2.0, "{q:?}");
    assert_eq!(q.loss_per_mille, 0);
    assert_eq!(q.bars(), 5);
    assert!((11_000..=13_000).contains(&q.up_bytes_per_s), "{q:?}");
}

#[test]
fn broadband_shows_its_delay_jitter_and_loss() {
    let q = measure(Impairment::BROADBAND, 20, 2);
    assert!((95.0..125.0).contains(&q.rtt_ms), "{q:?}");
    assert!(q.jitter_ms > 1.0, "{q:?}");
    // 1% forward loss over a 256-datagram window; ack loss is covered by the mask.
    assert!((1..=25).contains(&q.loss_per_mille), "{q:?}");
    assert!((3..=4).contains(&q.bars()), "{q:?}");
}

#[test]
fn a_dead_link_counts_as_full_loss_and_backs_off() {
    let q = measure(Impairment { loss_per_mille: 1_000, ..Default::default() }, 5, 3);
    assert_eq!(q.loss_per_mille, 1_000);
    assert_eq!(q.bars(), 0);
    assert_eq!(LinkQuality::default().rto_us(), 500_000);
}

#[test]
fn the_player_list_form_round_trips_and_refuses_bad_bars() {
    let q = Quality { rtt_ms: 87.0, jitter_ms: 6.0, loss_per_mille: 12, ..Default::default() };
    let back = Quality::decode(q.encode()).unwrap();
    assert_eq!((back.rtt_ms, back.jitter_ms, back.loss_per_mille), (87.0, 6.0, 12));
    assert_eq!(back.bars(), q.bars());
    let mut bad = q.encode();
    bad[5] = 9;
    assert!(Quality::decode(bad).is_none());
}

#[test]
fn bars_fall_with_delay_jitter_and_loss() {
    let at = |rtt_ms, jitter_ms, loss_per_mille| Quality { rtt_ms, jitter_ms, loss_per_mille, ..Default::default() }.bars();
    assert!(at(30.0, 2.0, 0) > at(30.0, 40.0, 0));
    assert!(at(30.0, 2.0, 0) > at(30.0, 2.0, 50));
    assert!(at(80.0, 5.0, 0) > at(500.0, 5.0, 0));
}
