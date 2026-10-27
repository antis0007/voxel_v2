use super::*;
use crate::link::{Impairment, Link};

/// Message `n`: its id in the first four bytes, 4 to 300 bytes long.
fn message(n: u32) -> Vec<u8> {
    let len = 4 + (engine_core::hash::hash_u32(n) % 296) as usize;
    let mut m = n.to_le_bytes().to_vec();
    m.resize(len, n as u8);
    m
}

struct Outcome {
    delivered: Vec<u32>,
    /// Milliseconds from push to delivery, sorted.
    delays: Vec<u64>,
    unreliable: usize,
}

/// The server pushes `total` messages at 100 a second (about 15 KB/s, half of what
/// 30 Hz datagrams of 1,200 bytes carry) and sends at 30 Hz; the client sends at
/// 60 Hz. Both carry a 120-byte unreliable payload.
fn exchange(impairment: Impairment, total: u32, seconds: u64, seed: u32) -> Outcome {
    let (mut down, mut up) = (Link::new(impairment, seed), Link::new(impairment, seed ^ 3));
    let (mut server, mut client) = (Connection::default(), Connection::default());
    let mut pushed_at = Vec::new();
    let mut out = Outcome { delivered: Vec::new(), delays: Vec::new(), unreliable: 0 };
    for now in (0..seconds * 1_000_000).step_by(1_000) {
        while (pushed_at.len() as u32) < total && pushed_at.len() as u64 * 10_000 <= now {
            if server.push_reliable(message(pushed_at.len() as u32)).is_err() {
                break;
            }
            pushed_at.push(now);
        }
        if now % 33_333 < 1_000 {
            down.send(now, server.send(now, &[1; 120], 1_200));
        }
        if now % 16_667 < 1_000 {
            up.send(now, client.send(now, &[2; 120], 1_200));
        }
        while let Some(d) = down.poll(now) {
            let Some(incoming) = client.receive(now, &d).unwrap() else { continue };
            out.unreliable += usize::from(!incoming.unreliable.is_empty());
            for m in incoming.reliable {
                let id = u32::from_le_bytes([m[0], m[1], m[2], m[3]]);
                assert_eq!(m, message(id));
                out.delays.push((now - pushed_at[id as usize]) / 1_000);
                out.delivered.push(id);
            }
        }
        while let Some(d) = up.poll(now) {
            server.receive(now, &d).unwrap();
        }
    }
    out.delays.sort_unstable();
    out
}

#[test]
fn broadband_delivers_every_message_once_in_order_and_promptly() {
    let o = exchange(Impairment::BROADBAND, 1_000, 13, 1);
    assert_eq!(o.delivered, (0..1_000).collect::<Vec<_>>());
    // One-way 50 ms plus up to one 33 ms send interval; resends after a loss add a timeout.
    let p50 = o.delays[o.delays.len() / 2];
    let p99 = o.delays[o.delays.len() * 99 / 100];
    assert!(p50 <= 110 && p99 <= 400, "p50 {p50} ms, p99 {p99} ms");
    assert!(o.unreliable > 300);
}

#[test]
fn heavy_loss_and_duplication_still_deliver_exactly_once() {
    let rough = Impairment { loss_per_mille: 100, duplicate_per_mille: 50, ..Impairment::BROADBAND };
    let o = exchange(rough, 800, 16, 2);
    assert_eq!(o.delivered, (0..800).collect::<Vec<_>>());
}

#[test]
fn quality_comes_from_the_same_acks() {
    let (mut a, mut b) = (Connection::default(), Connection::default());
    for t in 0..200u64 {
        let now = t * 20_000;
        let d = a.send(now, &[], 1_200);
        b.receive(now + 15_000, &d).unwrap();
        let back = b.send(now + 15_000, &[], 1_200);
        a.receive(now + 30_000, &back).unwrap();
    }
    let q = a.quality(4_000_000);
    assert!((q.rtt_ms - 30.0).abs() < 1.0, "{q:?}");
    assert_eq!(q.loss_per_mille, 0);
}

#[test]
fn a_stale_datagram_keeps_its_reliable_messages_but_drops_its_snapshot() {
    let (mut a, mut b) = (Connection::default(), Connection::default());
    a.push_reliable(vec![9]).unwrap();
    let old = a.send(0, &[7], 1_200);
    let new = a.send(10, &[8], 1_200);
    assert_eq!(b.receive(20, &new).unwrap().map(|i| i.unreliable), Some(vec![8]));
    let late = b.receive(30, &old).unwrap().unwrap();
    assert_eq!((late.reliable, late.unreliable), (vec![vec![9]], vec![]));
    assert_eq!(b.receive(40, &old).unwrap(), None);
}

#[test]
fn limits_and_hostile_datagrams_are_refused() {
    let mut a = Connection::default();
    assert!(a.push_reliable(vec![0; crate::reliable::MAX_MESSAGE + 1]).is_err());
    for _ in 0..crate::reliable::WINDOW {
        a.push_reliable(vec![1]).unwrap();
    }
    assert!(a.push_reliable(vec![1]).is_err());
    let mut b = Connection::default();
    assert!(b.receive(0, &[]).is_err());
    // Sequence 0, ack value past u32, then a mask: refused before anything applies.
    let mut bad = vec![0];
    put_u64(&mut bad, u64::from(u32::MAX) + 2);
    bad.extend([0; 4]);
    assert!(b.receive(0, &bad).is_err());
    // A reliable message claiming 2,000 bytes.
    let mut big = vec![0, 0, 0, 0, 0, 0, 1, 0];
    put_u64(&mut big, 2_000);
    assert!(b.receive(0, &big).is_err());
}
