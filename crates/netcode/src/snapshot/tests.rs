use super::*;
use crate::link::{Impairment, Link};
use glam::{Quat, Vec3};

const INTERVAL_US: u64 = 33_333;

/// Body `id` at `t` seconds: the first `moving` tumble until 6 s, the rest lie still.
fn body(id: u32, t: f32, moving: u32) -> BodyState {
    let base = Vec3::new(id as f32 * 3.0 - 700.0, 20.0, -(id as f32));
    if id >= moving {
        return BodyState::quantise(base, Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO, 1);
    }
    let t = t.min(6.0);
    let speed = if t < 6.0 { 1.0 } else { 0.0 };
    let p = base + Vec3::new(t.sin() * 4.0, t * 0.5, t.cos() * 4.0);
    let v = Vec3::new(t.cos() * 4.0, 0.5, -t.sin() * 4.0) * speed;
    BodyState::quantise(p, Quat::from_rotation_y(t * 2.0), v, Vec3::Y * 2.0 * speed, 1)
}

struct Run {
    /// Client's newest state per id, by snapshot sequence.
    client: HashMap<u32, (u32, BodyState)>,
    bytes: Vec<usize>,
    /// Largest gap in snapshots between updates of the farthest body, while moving.
    far_gap: u32,
}

fn run(count: u32, moving: u32, budget: usize, seconds: f32, seed: u32) -> Run {
    let (mut down, mut up) = (Link::new(Impairment::BROADBAND, seed), Link::new(Impairment::BROADBAND, seed ^ 1));
    let (mut writer, mut reader) = (SnapshotWriter::default(), SnapshotReader::default());
    let mut result = Run { client: HashMap::new(), bytes: Vec::new(), far_gap: 0 };
    let (mut far_last, far) = (0u32, moving - 1);
    let snapshots = (seconds * 30.0) as u64;
    for n in 0..snapshots {
        let now = n * INTERVAL_US;
        let t = n as f32 / 30.0;
        // Priority falls with distance: id order stands in for distance here.
        let bodies: Vec<_> = (0..count).map(|id| (id, body(id, t, moving), 1.0 / (1.0 + id as f32))).collect();
        let mut out = Vec::new();
        writer.write(&bodies, budget, &mut out);
        result.bytes.push(out.len());
        down.send(now, out);
        for step in 0..=INTERVAL_US / 1_000 {
            let at = now + step * 1_000;
            while let Some(packet) = down.poll(at) {
                let Ok((seq, states)) = reader.read(&packet) else { continue };
                for (id, state) in states {
                    let slot = result.client.entry(id).or_insert((seq, state));
                    if !newer(slot.0, seq) {
                        *slot = (seq, state);
                    }
                    if id == far && t < 6.0 {
                        result.far_gap = result.far_gap.max(seq.wrapping_sub(far_last));
                        far_last = seq;
                    }
                }
                if let Some((newest, mask)) = reader.ack() {
                    up.send(at, [newest.to_le_bytes(), mask.to_le_bytes()].concat());
                }
            }
            while let Some(ack) = up.poll(at) {
                let word = |i: usize| u32::from_le_bytes([ack[i], ack[i + 1], ack[i + 2], ack[i + 3]]);
                writer.acknowledge(word(0), word(4));
            }
        }
    }
    result
}

#[test]
fn the_client_converges_on_every_body_over_a_lossy_link() {
    let r = run(300, 60, 1_200, 9.0, 3);
    for id in 0..300 {
        assert_eq!(r.client.get(&id).map(|s| s.1), Some(body(id, 9.0, 60)), "body {id}");
    }
}

#[test]
fn snapshots_stay_in_budget_and_resting_bodies_cost_nothing() {
    let r = run(300, 60, 1_200, 9.0, 4);
    assert!(r.bytes.iter().all(|&b| b <= 1_200));
    // 60 tumbling bodies, each turning and changing velocity every snapshot: about
    // 16 bytes each, or 29 KB/s at 30 Hz.
    let moving = r.bytes[60..180].iter().sum::<usize>() / 120;
    assert!(moving <= 60 * 18, "{moving} bytes per snapshot");
    // After everything settles and is acknowledged, a snapshot is the header alone.
    assert!(r.bytes[r.bytes.len() - 15..].iter().all(|&b| b <= 4), "{:?}", &r.bytes[r.bytes.len() - 15..]);
}

#[test]
fn low_priority_bodies_still_update_within_a_bounded_gap() {
    // 120 moving bodies cannot all fit; the least important must still be refreshed.
    let r = run(120, 120, 600, 5.0, 5);
    assert!(r.far_gap > 1 && r.far_gap <= 30, "gap {}", r.far_gap);
}

#[test]
fn a_reused_id_ignores_acks_from_its_earlier_incarnation() {
    let (mut writer, mut reader) = (SnapshotWriter::default(), SnapshotReader::default());
    let a = BodyState::quantise(Vec3::ONE, Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO, 0);
    let b = BodyState::quantise(Vec3::splat(9.0), Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO, 2);
    let mut first = Vec::new();
    writer.write(&[(5, a, 1.0)], 1_000, &mut first);
    reader.read(&first).unwrap();
    writer.forget(5);
    reader.forget(5);
    let mut second = Vec::new();
    writer.write(&[(5, b, 1.0)], 1_000, &mut second);
    // The late ack names the first snapshot only.
    writer.acknowledge(1, 0);
    reader.read(&second).unwrap();
    let mut third = Vec::new();
    writer.write(&[(5, b, 1.0)], 1_000, &mut third);
    assert_eq!(reader.read(&third).unwrap().1, vec![(5, b)]);
}

#[test]
fn acks_cover_reordered_and_skipped_snapshots() {
    let mut reader = SnapshotReader::default();
    let empty = |seq: u32| {
        let mut out = Vec::new();
        put_u64(&mut out, u64::from(seq));
        put_u64(&mut out, 0);
        out
    };
    for seq in [1, 2, 4, 3, 40] {
        reader.read(&empty(seq)).unwrap();
    }
    // 40 is newest; 8..=39 fit in the mask, and 1..=4 fell out of it.
    assert_eq!(reader.ack(), Some((40, 0)));
    reader.read(&empty(39)).unwrap();
    reader.read(&empty(8)).unwrap();
    assert_eq!(reader.ack(), Some((40, 1 | 1 << 31)));
}
