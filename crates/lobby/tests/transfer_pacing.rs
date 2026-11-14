//! Exercise the game's exact pacer without linking the renderer.
#[allow(dead_code)]
#[path = "../../app/src/multiplayer/pacing.rs"]
mod pacing;

use engine_net::transfer::{Receiver, Sender};
use pacing::{BULK_BYTES_PER_SECOND, Lane, Pacer};
use std::time::{Duration, Instant};

#[test]
fn two_clients_finish_terrain_with_continuous_edits_and_images() {
    let now = Instant::now();
    let terrain: Vec<_> = (0..256 * 1024).map(|i| (i % 251) as u8).collect();
    for fps in [30, 144, 500] {
        let mut peers: Vec<_> = (0..2).map(|_| (
            Pacer::new(BULK_BYTES_PER_SECOND),
            Sender::new(terrain.clone()).unwrap(), Receiver::default(), None, [0; 3],
        )).collect();
        for frame in 0..fps * 5 {
            for (pacer, tx, rx, finished, totals) in &mut peers {
                let mut room = 24;
                pacer.drain(now + Duration::from_secs_f64(frame as f64 / fps as f64), &mut room, |lane| {
                    if lane == Lane::Bulk {
                        let Some(packet) = tx.next() else { return Ok(None); };
                        let size = packet.len();
                        if let Some(bytes) = rx.receive(&packet)? { *finished = Some(bytes); }
                        tx.accepted();
                        totals[lane as usize] += size;
                        Ok::<_, anyhow::Error>(Some(size))
                    } else {
                        totals[lane as usize] += 880;
                        Ok(Some(880))
                    }
                }).unwrap();
            }
        }
        for (_, _, _, received, totals) in peers {
            assert!(received.as_deref() == Some(terrain.as_slice()), "terrain incomplete at {fps} fps");
            assert!(totals[0] >= totals[1], "own gameplay remains prioritized");
            assert!(totals[1] > 250_000, "edits continue");
            let limit = BULK_BYTES_PER_SECOND * 5.25 + 885.0;
            assert!(totals.iter().sum::<usize>() as f64 <= limit, "bandwidth unchanged");
            eprintln!("paced workload fps={fps} bytes={totals:?}");
        }
    }
}

#[test]
fn inactive_or_blocked_lane_never_banks_a_later_burst() {
    let now = Instant::now();
    let mut pacer = Pacer::new(BULK_BYTES_PER_SECOND);
    // Image is the only active lane for a minute.
    for frame in 0..3600 {
        let mut room = 20;
        pacer.drain(now + Duration::from_secs_f64(frame as f64 / 60.0), &mut room,
            |lane| Ok((lane == Lane::Image).then_some(880))).unwrap();
    }
    let mut sent = [0; 3];
    let mut room = 20;
    pacer.drain(now + Duration::from_secs(60), &mut room, |lane| {
        sent[lane as usize] += 880;
        Ok(Some(880))
    }).unwrap();
    assert!(sent.iter().all(|&bytes| bytes > 0), "returning lanes progress: {sent:?}");
    assert!(sent[0] >= sent[1] && sent[0] >= sent[2], "no saved terrain credit: {sent:?}");
}

#[test]
fn unequal_packet_sizes_share_bytes_not_packet_counts() {
    let mut pacer = Pacer::new(BULK_BYTES_PER_SECOND);
    let now = Instant::now();
    let mut bytes = [0; 3];
    for frame in 0..600 {
        let mut room = 512;
        pacer.drain(now + Duration::from_secs_f64(frame as f64 / 60.0), &mut room, |lane| {
            let size = [29, 885, 317][lane as usize];
            bytes[lane as usize] += size;
            Ok(Some(size))
        }).unwrap();
    }
    assert!(bytes[0].abs_diff(2 * bytes[1]) < 1800, "{bytes:?}");
    assert!(bytes[1].abs_diff(bytes[2]) < 900, "{bytes:?}");
}
