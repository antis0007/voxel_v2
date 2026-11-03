//! Benchmarks per-tick server work: cell deltas and entity snapshots.
//! `just bench-check` compares them with `baselines.json`.
use criterion::{Criterion, criterion_group, criterion_main};
use engine_netcode::body::BodyState;
use engine_netcode::cells::CellStream;
use engine_netcode::snapshot::{SnapshotReader, SnapshotWriter};
use glam::IVec3;
use std::hint::black_box;

const BODIES: u32 = 256;

/// 64 bricks with 160 distinct cells each, every cell written three times, then one flush.
fn cells_flush(c: &mut Criterion) {
    c.bench_function("cells_64_bricks_3x160_changes_flush", |b| {
        let mut out = Vec::new();
        b.iter(|| {
            let mut stream = CellStream::default();
            for pass in 0..3u32 {
                for brick in 0..64i32 {
                    let origin = IVec3::new(brick % 4, brick / 16, (brick / 4) % 4) * 8;
                    for i in 0..160i32 {
                        let k = (i * 37) % 512;
                        let cell = origin + IVec3::new(k % 8, (k / 8) % 8, k / 64);
                        stream.changed(cell, (pass + i as u32) & 7);
                    }
                }
            }
            out.clear();
            black_box(stream.flush(IVec3::ZERO, 1 << 20, &mut out));
        });
    });
}

fn bodies(tick: u32) -> Vec<(u32, BodyState, f32)> {
    (0..BODIES)
        .map(|id| {
            // Half the bodies move each tick; the rest sleep.
            let moving = id % 2 == 0;
            let t = if moving { tick as i64 } else { 0 };
            let state = BodyState {
                position: [i64::from(id) * 300 + t * 3, 2000, t],
                rotation: id,
                velocity: [if moving { 180 } else { 0 }, 0, 0],
                angular: [0; 3],
                state: id & 3,
            };
            (id, state, 1.0 + (id % 7) as f32)
        })
        .collect()
}

/// One snapshot of 256 bodies at 1200 bytes, client decode, ack.
fn snapshot_tick(c: &mut Criterion) {
    let mut writer = SnapshotWriter::default();
    let mut reader = SnapshotReader::default();
    let mut out = Vec::new();
    let mut tick = 0u32;
    let mut bytes = 0usize;
    for _ in 0..64 {
        tick += 1;
        out.clear();
        writer.write(&bodies(tick), 1200, &mut out);
        bytes += out.len();
        reader.read(&out).ok();
        if let Some((n, e)) = reader.ack() {
            writer.acknowledge(n, e);
        }
    }
    eprintln!("snapshot: {} bytes per snapshot average over 64", bytes / 64);
    c.bench_function("snapshot_256_bodies_write_read_ack", |b| {
        b.iter(|| {
            tick += 1;
            let set = bodies(tick);
            out.clear();
            black_box(writer.write(&set, 1200, &mut out));
            black_box(reader.read(&out).ok());
            if let Some((n, e)) = reader.ack() {
                writer.acknowledge(n, e);
            }
        });
    });
}

criterion_group!(benches, cells_flush, snapshot_tick);
criterion_main!(benches);
