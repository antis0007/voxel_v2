use super::desync::{DesyncMonitor, Outcome, Verdict};
use super::wire::{read_client_msg, read_host_msg, write_client_msg, write_host_msg};
use super::*;
use crate::link::{Impairment, Link};
use crate::wire::{Reader, WireError, put_u64};
use engine_core::hash::hash_u32;

const CFG: Config = Config { input_delay: 3, resync_lag: 40, ..Config::DEFAULT };

fn put(p: &u32, out: &mut Vec<u8>) {
    put_u64(out, u64::from(*p));
}

fn get(r: &mut Reader) -> Result<u32, WireError> {
    r.u32()
}

/// Host message to a client, through bytes.
fn relay(host: &LockstepHost<u32>, id: PeerId, client: &mut LockstepClient<u32>) {
    let mut out = Vec::new();
    write_host_msg(&host.message_for(id).unwrap(), &mut out, put);
    client.receive(read_host_msg(&out, &CFG, get).unwrap());
}

#[test]
fn the_host_stamps_inputs_input_delay_ticks_ahead() {
    let mut host = LockstepHost::<u32>::new(CFG, 100);
    assert_eq!((host.next_tick(), host.sealed_until()), (100, 103));
    host.submit(7).unwrap();
    for tick in 100..103 {
        assert_eq!(host.advance(), Frame { tick, inputs: vec![] }, "pre-sealed ticks are empty");
    }
    let frame = host.advance();
    assert_eq!(frame.tick, 103);
    assert_eq!(frame.inputs, vec![Input { peer: HOST, sequence: 1, order: 0, payload: 7 }]);
}

#[test]
fn a_client_releases_a_tick_only_when_it_holds_it() {
    let mut host = LockstepHost::<u32>::new(CFG, 0);
    host.add_peer(1, 0).unwrap();
    let mut client = LockstepClient::new(CFG, 1, 0);
    assert!(client.release().is_none(), "nothing sealed has arrived");
    host.submit(5).unwrap();
    for _ in 0..6 {
        host.advance();
    }
    let all = host.message_for(1).unwrap();
    assert_eq!(all.frames.len(), 9);
    // Deliver every frame but tick 2: ticks 0 and 1 come out, then the client waits.
    let mut gap = all.clone();
    gap.frames.remove(2);
    client.receive(gap);
    assert_eq!(client.message().frames_ack, 2);
    assert_eq!(client.release().map(|f| f.tick), Some(0));
    assert_eq!(client.release().map(|f| f.tick), Some(1));
    assert!(client.release().is_none());
    client.receive(all);
    let rest: Vec<u64> = std::iter::from_fn(|| client.release()).map(|f| f.tick).collect();
    assert_eq!(rest, (2..9).collect::<Vec<_>>());
}

#[test]
fn duplicated_and_reordered_client_messages_add_each_input_once() {
    let mut host = LockstepHost::<u32>::new(CFG, 0);
    host.add_peer(1, 0).unwrap();
    let mut client = LockstepClient::new(CFG, 1, 0);
    client.submit(10).unwrap();
    let older = client.message();
    client.submit(11).unwrap();
    let newer = client.message();
    host.receive(1, newer.clone()).unwrap();
    host.receive(1, older).unwrap();
    host.receive(1, newer).unwrap();
    let frame = host.advance();
    assert!(frame.inputs.is_empty());
    let mut sealed = Vec::new();
    for _ in 0..4 {
        sealed.extend(host.advance().inputs.into_iter().map(|i| (i.sequence, i.payload)));
    }
    assert_eq!(sealed, vec![(1, 10), (2, 11)]);
    relay(&host, 1, &mut client);
    assert_eq!(client.unacked(), 0, "the acknowledgement clears the outbox");
}

#[test]
fn a_gap_in_client_inputs_waits_for_the_resend() {
    let mut host = LockstepHost::<u32>::new(CFG, 0);
    host.add_peer(1, 0).unwrap();
    host.receive(1, ClientMsg { frames_ack: 0, first_sequence: 2, inputs: vec![20] }).unwrap();
    assert_eq!(host.message_for(1).unwrap().input_ack, 1, "sequence 1 is missing, 2 is not taken");
    host.receive(1, ClientMsg { frames_ack: 0, first_sequence: 1, inputs: vec![10, 20] }).unwrap();
    assert_eq!(host.message_for(1).unwrap().input_ack, 3);
}

#[test]
fn a_silent_peer_is_cut_off_and_host_memory_stays_bounded() {
    let mut host = LockstepHost::<u32>::new(CFG, 0);
    host.add_peer(1, 0).unwrap();
    host.add_peer(2, 0).unwrap();
    let mut live = LockstepClient::new(CFG, 2, 0);
    let mut silent = LockstepClient::new(CFG, 1, 0);
    for t in 0..200u32 {
        host.submit(t).unwrap();
        host.advance();
        relay(&host, 2, &mut live);
        while live.release().is_some() {}
        host.receive(2, live.message()).unwrap();
        assert!(host.retained() as u64 <= CFG.resync_lag + CFG.input_delay + 1, "retained {}", host.retained());
    }
    assert!(host.needs_resync(1) && !host.needs_resync(2));
    relay(&host, 1, &mut silent);
    assert_eq!(silent.status(), ClientStatus::NeedsResync);
    assert!(matches!(live.status(), ClientStatus::Trailing(n) if n <= CFG.input_delay));
    // The checkpoint is the state at the start of the host's next tick; frames flow again from it.
    let from = host.next_tick();
    host.resynced(1, from).unwrap();
    silent.resync(from);
    host.advance();
    relay(&host, 1, &mut silent);
    assert_eq!(silent.release().map(|f| f.tick), Some(from));
    assert!(matches!(silent.status(), ClientStatus::Trailing(_)));
}

#[test]
fn a_late_joiner_starts_from_the_checkpoint_tick() {
    let mut host = LockstepHost::<u32>::new(CFG, 0);
    for _ in 0..50 {
        host.advance();
    }
    assert_eq!(host.add_peer(3, 10), Err(LockstepError::TickNotRetained));
    let from = host.next_tick();
    host.add_peer(3, from).unwrap();
    assert_eq!(host.add_peer(3, from), Err(LockstepError::DuplicatePeer));
    let mut c = LockstepClient::new(CFG, 3, from);
    relay(&host, 3, &mut c);
    assert_eq!(c.release().map(|f| f.tick), Some(from));
}

#[test]
fn pending_inputs_are_bounded_on_both_sides() {
    let cfg = Config { max_pending_inputs: 4, ..CFG };
    let mut c = LockstepClient::<u32>::new(cfg, 1, 0);
    for i in 0..4 {
        c.submit(i).unwrap();
    }
    assert_eq!(c.submit(9), Err(LockstepError::InputsFull));
    let mut host = LockstepHost::<u32>::new(cfg, 0);
    host.add_peer(1, 0).unwrap();
    host.submit(0).unwrap();
    assert_eq!(host.receive(1, c.message()), Err(LockstepError::InputsFull));
    assert_eq!(host.message_for(1).unwrap().input_ack, 4, "three taken, the fourth waits for room");
}

#[test]
fn malformed_messages_are_refused() {
    let msg = HostMsg { sealed_until: 5, input_ack: 1, resync: false, frames: vec![Frame { tick: 4, inputs: vec![] }] };
    let mut out = Vec::new();
    write_host_msg(&msg, &mut out, put);
    assert_eq!(read_host_msg(&out, &CFG, get), Ok(msg));
    assert!(read_host_msg(&out[..out.len() - 1], &CFG, get).is_err());
    let mut long = out.clone();
    long.push(0);
    assert!(read_host_msg(&long, &CFG, get).is_err());
    let past_seal = HostMsg { sealed_until: 4, input_ack: 1, resync: false, frames: vec![Frame { tick: 4, inputs: vec![] }] };
    let mut bad = Vec::new();
    write_host_msg(&past_seal, &mut bad, put);
    assert_eq!(read_host_msg(&bad, &CFG, get), Err(WireError("frame tick")));
    let flood = ClientMsg { frames_ack: 0, first_sequence: 1, inputs: vec![0u32; CFG.inputs_per_message + 1] };
    let mut bytes = Vec::new();
    write_client_msg(&flood, &mut bytes, put);
    assert_eq!(read_client_msg(&bytes, &CFG, get), Err(WireError("input count")));
}

#[test]
fn the_monitor_matches_reports_early_late_and_expired() {
    let mut m = DesyncMonitor::new(10, 2, 4);
    assert!(m.is_checkpoint(20) && !m.is_checkpoint(21));
    assert_eq!(m.report(1, 10, 7), None, "the host has not reached tick 10");
    let v = m.record_host(10, 7);
    assert_eq!(v, vec![Verdict { peer: 1, tick: 10, outcome: Outcome::Match }]);
    assert_eq!(m.report(2, 10, 8).map(|v| v.outcome), Some(Outcome::Mismatch));
    m.record_host(20, 1);
    m.record_host(30, 1);
    assert_eq!(m.report(2, 10, 7).map(|v| v.outcome), Some(Outcome::Expired));
}

/// Host and two clients over lossy, jittery, duplicating links: every peer releases the same frames
/// in the same order, each input exactly once.
#[test]
fn three_peers_over_bad_links_release_identical_frames() {
    let bad = Impairment { latency_us: 40_000, jitter_us: 30_000, loss_per_mille: 100, duplicate_per_mille: 50, ..Default::default() };
    let mut host = LockstepHost::<u32>::new(CFG, 0);
    let mut clients: Vec<LockstepClient<u32>> = (1..=2).map(|id| LockstepClient::new(CFG, id, 0)).collect();
    let mut down: Vec<Link> = (0..2).map(|i| Link::new(bad, 10 + i)).collect();
    let mut up: Vec<Link> = (0..2).map(|i| Link::new(bad, 20 + i)).collect();
    for c in &clients {
        host.add_peer(c.id(), 0).unwrap();
    }
    let mut logs: Vec<Vec<Frame<u32>>> = vec![Vec::new(); 3];
    let mut submitted = [0u32; 3];
    for tick in 0..3_000u64 {
        let now = tick * 16_667;
        for (i, link) in up.iter_mut().enumerate() {
            while let Some(bytes) = link.poll(now) {
                host.receive(i as PeerId + 1, read_client_msg(&bytes, &CFG, get).unwrap()).ok();
            }
        }
        if tick < 2_900 && hash_u32(tick as u32).is_multiple_of(5) && host.submit(tick as u32).is_ok() {
            submitted[0] += 1;
        }
        logs[0].push(host.advance());
        for (i, c) in clients.iter_mut().enumerate() {
            let mut out = Vec::new();
            write_host_msg(&host.message_for(c.id()).unwrap(), &mut out, put);
            down[i].send(now, out);
            while let Some(bytes) = down[i].poll(now) {
                c.receive(read_host_msg(&bytes, &CFG, get).unwrap());
            }
            if tick < 2_900 && hash_u32(tick as u32 ^ (i as u32 + 1) << 20).is_multiple_of(4) && c.submit(tick as u32).is_ok() {
                submitted[i + 1] += 1;
            }
            while let Some(f) = c.release() {
                logs[i + 1].push(f);
            }
            assert_ne!(c.status(), ClientStatus::NeedsResync, "client {} cut off at tick {tick}", c.id());
            let mut out = Vec::new();
            write_client_msg(&c.message(), &mut out, put);
            up[i].send(now, out);
        }
    }
    let n = logs[1].len().min(logs[2].len());
    assert!(n > 2_900, "clients stalled at {n}");
    assert_eq!(logs[0][..n], logs[1][..n]);
    assert_eq!(logs[0][..n], logs[2][..n]);
    // Submissions stopped 100 ticks before the end: every input is in a frame, once, in sequence.
    for peer in 0..3u16 {
        let seen: Vec<u32> = logs[0].iter().flat_map(|f| &f.inputs).filter(|i| i.peer == peer).map(|i| i.sequence).collect();
        assert_eq!(seen, (1..=submitted[peer as usize]).collect::<Vec<_>>(), "peer {peer}");
    }
    assert!(clients.iter().all(|c| c.unacked() == 0));
}
