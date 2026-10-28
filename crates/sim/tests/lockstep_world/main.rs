//! Proof of deterministic lockstep: a host and two clients each run their own `engine_sim` world.
//! Every peer makes seeded random edits, all routed through the host's `LockstepHost`, over
//! `engine_netcode` links with loss, duplication and jitter (so reordering). Every checkpoint
//! digest agrees for 5,000 ticks. Then one client's world is corrupted; the next digest exchange
//! catches it, the chunk sums narrow it, a scheduled brick resend repairs it and digests converge.
//!
//! Digest reports, chunk sums and repair data use a reliable ordered channel (`Channel`), as the
//! game's reliable stream and bulk transfer would carry them.
mod world;

use engine_netcode::link::{Impairment, Link};
use engine_netcode::lockstep::desync::{DesyncMonitor, Outcome, Verdict};
use engine_netcode::lockstep::wire::{read_client_msg, read_host_msg, write_client_msg, write_host_msg};
use engine_netcode::lockstep::{ClientStatus, Config, Frame, HOST, LockstepClient, LockstepHost, PeerId};
use engine_sim::{Cell, split_voxel};
use glam::IVec3;
use std::collections::{BTreeMap, VecDeque};
use world::{ChunkCells, Op, SAND, World, get_op, put_op, random_edit};

const SEED: u32 = 0x5EED_0001;
const TICK_US: u64 = 16_667;
const INTERVAL: u64 = 30;
const CORRUPT_AT: u64 = 5_010;
const END: u64 = 5_600;
const LAST_EDIT: u64 = END - 200;
const CFG: Config = Config::DEFAULT;
const LOSSY: Impairment =
    Impairment { latency_us: 40_000, jitter_us: 25_000, loss_per_mille: 50, duplicate_per_mille: 20, bytes_per_second: 0, queue_us: 0 };

enum Control {
    Report { tick: u64, digest: u64 },
    ChunkRequest { tick: u64 },
    ChunkSums { tick: u64, sums: Vec<(IVec3, u64)> },
    RepairData { tick: u64, chunks: Vec<(IVec3, ChunkCells)> },
}

/// Reliable and ordered, fixed latency.
#[derive(Default)]
struct Channel(VecDeque<(u64, Control)>);

impl Channel {
    fn send(&mut self, now: u64, msg: Control) {
        self.0.push_back((now + 50_000, msg));
    }
    fn poll(&mut self, now: u64) -> Option<Control> {
        if self.0.front().is_some_and(|(due, _)| *due <= now) { self.0.pop_front().map(|(_, m)| m) } else { None }
    }
}

struct Client {
    world: World,
    ls: LockstepClient<Op>,
    down: Link,
    up: Link,
    to_host: Channel,
    from_host: Channel,
    held: Option<Frame<Op>>,
    repairs: BTreeMap<u64, Vec<(IVec3, ChunkCells)>>,
    stalled: u32,
}

enum Repair {
    Idle,
    Requested,
    Scheduled(u64),
}

#[derive(Default)]
struct Events {
    corrupted: Option<u64>,
    detected: Option<u64>,
    scheduled: Option<(u64, usize)>,
    converged: Option<u64>,
}

fn differing(ours: &[(IVec3, u64)], theirs: &[(IVec3, u64)]) -> Vec<IVec3> {
    let a: BTreeMap<_, _> = ours.iter().map(|(c, s)| ((c.x, c.y, c.z), *s)).collect();
    let b: BTreeMap<_, _> = theirs.iter().map(|(c, s)| ((c.x, c.y, c.z), *s)).collect();
    let mut keys: Vec<_> = a.keys().chain(b.keys()).copied().collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter().filter(|k| a.get(k) != b.get(k)).map(|(x, y, z)| IVec3::new(x, y, z)).collect()
}

/// The differing chunks and their face neighbours inside the region: a desync spreads while the repair is in flight.
fn repair_set(diff: &[IVec3]) -> Vec<IVec3> {
    let max = (world::SIZE - IVec3::ONE) / 32;
    let mut out: Vec<IVec3> = Vec::new();
    for &c in diff {
        for d in [IVec3::ZERO, IVec3::X, -IVec3::X, IVec3::Y, -IVec3::Y, IVec3::Z, -IVec3::Z] {
            let n = c + d;
            if n.cmpge(IVec3::ZERO).all() && n.cmple(max).all() && !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

#[test]
fn three_peers_stay_identical_through_loss_and_repair_a_corrupted_client() {
    let mut host_world = World::new(SEED);
    let mut host = LockstepHost::<Op>::new(CFG, 0);
    let mut monitor = DesyncMonitor::new(INTERVAL, 16, 2);
    let mut clients: Vec<Client> = (1..=2u16)
        .map(|id| Client {
            world: World::new(SEED),
            ls: LockstepClient::new(CFG, id, 0),
            down: Link::new(LOSSY, 100 + u32::from(id)),
            up: Link::new(LOSSY, 200 + u32::from(id)),
            to_host: Channel::default(),
            from_host: Channel::default(),
            held: None,
            repairs: BTreeMap::new(),
            stalled: 0,
        })
        .collect();
    for c in &clients {
        host.add_peer(c.ls.id(), 0).expect("tick 0 is retained");
    }
    let mut repair = [Repair::Idle, Repair::Idle];
    let mut ev = Events::default();
    let mut verdicts: Vec<Verdict> = Vec::new();
    let mut edits = 0usize;

    for w in 0..END {
        let now = w * TICK_US;
        // Host: take client traffic.
        for (i, c) in clients.iter_mut().enumerate() {
            let id = i as PeerId + 1;
            while let Some(bytes) = c.up.poll(now) {
                let msg = read_client_msg(&bytes, &CFG, get_op).expect("well-formed client message");
                let _ = host.receive(id, msg);
            }
            while let Some(msg) = c.to_host.poll(now) {
                match msg {
                    Control::Report { tick, digest } => verdicts.extend(monitor.report(id, tick, digest)),
                    Control::ChunkSums { tick, sums } => {
                        let ours = host_world.chunk_sums.get(&tick).expect("host keeps recent chunk sums");
                        let chunks = repair_set(&differing(ours, &sums));
                        let at = host.sealed_until();
                        host.submit(Op::Repair { peer: id, chunks: chunks.clone() }).expect("room for a repair");
                        repair[i] = Repair::Scheduled(at);
                        ev.scheduled = Some((at, chunks.len()));
                    }
                    _ => {}
                }
            }
        }
        if w < LAST_EDIT
            && let Some(op) = random_edit(SEED, HOST, w)
        {
            let _ = host.submit(op);
        }
        // Host: seal, capture repairs at the start of the tick, step.
        let frame = host.advance();
        edits += frame.inputs.len();
        for input in &frame.inputs {
            if let Op::Repair { peer, chunks } = &input.payload {
                let data = chunks.iter().map(|&c| (c, host_world.copy_chunk(c))).collect();
                clients[*peer as usize - 1].from_host.send(now, Control::RepairData { tick: frame.tick, chunks: data });
            }
        }
        let checkpoint = monitor.is_checkpoint(frame.tick);
        host_world.run(&frame, checkpoint);
        if checkpoint {
            verdicts.extend(monitor.record_host(frame.tick, host_world.digest.total()));
        }
        for v in verdicts.drain(..) {
            let i = v.peer as usize - 1;
            match (v.outcome, &repair[i]) {
                (Outcome::Mismatch, Repair::Idle) => {
                    ev.detected.get_or_insert(v.tick);
                    clients[i].from_host.send(now, Control::ChunkRequest { tick: v.tick });
                    repair[i] = Repair::Requested;
                }
                (Outcome::Mismatch, Repair::Scheduled(at)) if v.tick >= *at => panic!("repair at {at} did not converge"),
                (Outcome::Match, Repair::Scheduled(at)) if v.tick >= *at => {
                    ev.converged.get_or_insert(v.tick);
                    repair[i] = Repair::Idle;
                }
                _ => {}
            }
        }
        // Clients.
        for (i, c) in clients.iter_mut().enumerate() {
            let id = i as PeerId + 1;
            let mut out = Vec::new();
            write_host_msg(&host.message_for(id).expect("known peer"), &mut out, put_op);
            c.down.send(now, out);
            while let Some(bytes) = c.down.poll(now) {
                c.ls.receive(read_host_msg(&bytes, &CFG, get_op).expect("well-formed host message"));
            }
            while let Some(msg) = c.from_host.poll(now) {
                match msg {
                    Control::ChunkRequest { tick } => {
                        let sums = c.world.chunk_sums.get(&tick).cloned().expect("client keeps recent chunk sums");
                        c.to_host.send(now, Control::ChunkSums { tick, sums });
                    }
                    Control::RepairData { tick, chunks } => {
                        c.repairs.insert(tick, chunks);
                    }
                    _ => {}
                }
            }
            if id == 2 && ev.corrupted.is_none() && c.ls.next_tick() >= CORRUPT_AT {
                // A desync as divergent simulation would make one: a hot grain only this peer has.
                let p = IVec3::new(12, 26, 50);
                c.world.edit(p, Cell { temperature: 3_500, ..Cell::filled(SAND) });
                ev.corrupted = Some(c.ls.next_tick());
                assert_eq!(split_voxel(p).0, IVec3::new(0, 0, 1));
            }
            for _ in 0..4 {
                let Some(frame) = c.held.take().or_else(|| c.ls.release()) else { break };
                let mine = frame.inputs.iter().any(|f| matches!(&f.payload, Op::Repair { peer, .. } if *peer == id));
                if mine {
                    let Some(data) = c.repairs.remove(&frame.tick) else {
                        c.held = Some(frame);
                        c.stalled += 1;
                        break;
                    };
                    for (chunk, cells) in &data {
                        c.world.replace_chunk(*chunk, cells);
                    }
                }
                let checkpoint = frame.tick.is_multiple_of(INTERVAL);
                c.world.run(&frame, checkpoint);
                if checkpoint {
                    let digest = c.world.digest.total();
                    c.to_host.send(now, Control::Report { tick: frame.tick, digest });
                }
            }
            assert_ne!(c.ls.status(), ClientStatus::NeedsResync, "client {id} cut off at {w}");
            if w < LAST_EDIT
                && let Some(op) = random_edit(SEED, id, w)
            {
                let _ = c.ls.submit(op);
            }
            let mut out = Vec::new();
            write_client_msg(&c.ls.message(), &mut out, put_op);
            c.up.send(now, out);
        }
    }

    let corrupted = ev.corrupted.expect("the corruption ran");
    let first_check = corrupted.div_ceil(INTERVAL) * INTERVAL;
    assert_eq!(ev.detected, Some(first_check), "the first digest exchange after the corruption catches it");
    let (repair_tick, repaired_chunks) = ev.scheduled.expect("a repair was scheduled");
    let converged = ev.converged.expect("digests converged after the repair");
    let (h, a, b) = (&host_world.checkpoints, &clients[0].world.checkpoints, &clients[1].world.checkpoints);
    let mut compared = 0;
    for (tick, &(digest, state)) in h {
        let (Some(&(da, sa)), Some(&(db, sb))) = (a.get(tick), b.get(tick)) else { continue };
        assert_eq!((da, sa), (digest, state), "client 1 diverged at tick {tick}");
        // The digest agrees exactly when the full state does.
        assert_eq!(db == digest, sb == state, "digest and state disagree at tick {tick}");
        if *tick < corrupted || *tick >= repair_tick {
            assert_eq!(db, digest, "client 2 differs at tick {tick}");
            compared += 1;
        } else {
            assert_ne!(db, digest, "client 2 was corrupted at {corrupted} but matches at {tick}");
        }
    }
    assert!(edits > 2_000, "only {edits} inputs");
    let last = *h.keys().next_back().expect("checkpoints recorded");
    assert!(compared as u64 >= END / INTERVAL - 25, "only {compared} checkpoints compared");
    assert!(a.contains_key(&last) || a.contains_key(&(last - INTERVAL)), "client 1 kept up");
    let bytes: Vec<u64> = clients.iter().map(|c| c.down.stats().sent_bytes + c.up.stats().sent_bytes).collect();
    let lost: Vec<u64> = clients.iter().map(|c| c.down.stats().lost + c.up.stats().lost).collect();
    println!(
        "ticks {END}, inputs {edits}, checkpoints compared {compared}, corrupted {corrupted}, detected {first_check}, repair tick \
         {repair_tick} ({repaired_chunks} chunks, client stalled {} ticks), converged {converged}; lockstep bytes per \
         client {bytes:?} ({} B/s), datagrams lost {lost:?}",
        clients[1].stalled,
        bytes[0] * 1_000_000 / (END * TICK_US),
    );
}
