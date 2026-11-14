//! Real loopback relay, signed peers, contended transfers and a fresh rejoin baseline.
#[allow(dead_code)]
#[path = "../../app/src/multiplayer/pacing.rs"]
mod pacing;

use engine_lobby::{directory::Limits, server::{Config, Lobby}};
use engine_net::{
    admin::AccessList,
    identity::{PlayerKey, fingerprint},
    lobby::{relay::{RelayClient, RelayHost}, wire::{HostInfo, Mode, WorldParams}},
    protocol::{Packet, PlayerId, WorldIdentity},
    transfer::{Receiver, Sender},
};
use pacing::{BULK_BYTES_PER_SECOND, Lane, Pacer};
use std::{net::{SocketAddr, TcpListener}, thread, time::{Duration, Instant}};

fn start() -> (SocketAddr, WorldIdentity, RelayHost) {
    let lobby = Lobby::new(Config {
        limits: Limits { max_sessions: 4, max_per_address: 4 }, max_connections: 12,
    });
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || { let _ = lobby.serve(listener); });
    let world = WorldIdentity { seed: 74, generator: 62, cell_mm: 250, content: [9; 32] };
    let info = HostInfo {
        name: "Streaming acceptance".into(), world: "Test".into(), mode: Mode::Survival,
        version: world.generator, digest: world.content, max_players: 4, unlisted: true,
        world_params: WorldParams { seed: world.seed as u32, kind: 1, scale: 2 }, motd: String::new(),
    };
    let mut host = RelayHost::start(address, info, world).unwrap();
    host.set_access(AccessList::default(), false);
    (address, world, host)
}

fn connect(address: SocketAddr, host: &RelayHost, world: WorldIdentity) -> RelayClient {
    let mut client = RelayClient::connect(address, host.code(), world.generator,
        world.content, Instant::now()).unwrap();
    client.hello(world).unwrap();
    client
}

fn tick(host: &mut RelayHost, clients: &mut [RelayClient], keys: &[PlayerKey]) {
    let now = Instant::now();
    host.poll(now).unwrap();
    host.advance().unwrap();
    for (index, client) in clients.iter_mut().enumerate() {
        client.poll(now).unwrap();
        if client.can_introduce() { client.introduce(&format!("Peer{index}"), &keys[index]).unwrap(); }
    }
    thread::sleep(Duration::from_millis(4));
}

fn admit(host: &mut RelayHost, clients: &mut [RelayClient], keys: &[PlayerKey]) -> Vec<PlayerId> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        tick(host, clients, keys);
        if clients.iter().all(|client| client.player().is_some_and(|id| host.fingerprint(id).is_some())) {
            return clients.iter().map(|client| client.player().unwrap()).collect();
        }
        assert!(Instant::now() < deadline, "signed admission timed out");
    }
}

fn stream(host: &mut RelayHost, clients: &mut [RelayClient], terrain: &[u8]) {
    let ids: Vec<_> = clients.iter().map(|client| client.player().unwrap()).collect();
    let mut streams: Vec<_> = clients.iter().map(|_| (
        Pacer::new(BULK_BYTES_PER_SECOND), Sender::new(terrain.to_vec()).unwrap(),
        Receiver::default(), None, [0_usize; 3], 0_u64,
    )).collect();
    let started = Instant::now();
    let deadline = started + Duration::from_secs(10);
    while streams.iter().any(|(_, _, _, complete, _, _)| complete.is_none()) {
        let now = Instant::now();
        host.poll(now).unwrap();
        host.advance().unwrap();
        for (index, (pacer, sender, _, _, _, _)) in streams.iter_mut().enumerate() {
            // Leave room for session frames just as the game does.
            let mut room = (engine_net::lobby::relay::QUEUE - pacing::LINK_RESERVE)
                .saturating_sub(host.backlog()).min(12);
            pacer.drain(now, &mut room, |lane| {
                let packet = if lane == Lane::Bulk {
                    let Some(packet) = sender.next() else { return Ok(None); };
                    packet
                } else { vec![50 + lane as u8; 880] };
                let size = packet.len();
                if !host.send_data(ids[index], packet)? { return Ok(None); }
                if lane == Lane::Bulk { sender.accepted(); }
                Ok(Some(size))
            }).unwrap();
        }
        for (client, (_, _, receiver, complete, totals, frames)) in clients.iter_mut().zip(&mut streams) {
            for packet in client.poll(now).unwrap() {
                match packet {
                    Packet::Frame { .. } => *frames += 1,
                    Packet::Data(bytes) => match bytes[0] {
                        1 | 2 => {
                            totals[2] += bytes.len();
                            if let Some(value) = receiver.receive(&bytes).unwrap() { *complete = Some(value); }
                        }
                        50 | 51 => totals[usize::from(bytes[0] - 50)] += bytes.len(),
                        tag => panic!("unexpected payload {tag}"),
                    },
                    _ => {}
                }
            }
        }
        assert!(Instant::now() < deadline, "terrain starved behind continuous state and edit traffic");
        thread::sleep(Duration::from_millis(4));
    }
    for (_, _, _, complete, totals, frames) in streams {
        assert_eq!(complete.as_deref(), Some(terrain));
        assert!(totals[0] > 0 && totals[1] > 0 && frames > 10);
        eprintln!("loopback transfer elapsed_ms={} bytes={totals:?} frames={frames}", started.elapsed().as_millis());
    }
}

#[test]
fn two_signed_clients_stream_during_edits_and_rejoin_current_state() {
    let (address, world, mut host) = start();
    let keys = [PlayerKey::from_bytes([61; 32]), PlayerKey::from_bytes([62; 32])];
    let mut clients = vec![connect(address, &host, world), connect(address, &host, world)];
    let ids = admit(&mut host, &mut clients, &keys);
    assert_ne!(ids[0], ids[1]);
    for (id, key) in ids.iter().zip(&keys) { assert_eq!(host.fingerprint(*id), Some(fingerprint(&key.public())).as_deref()); }
    let terrain: Vec<_> = (0..64 * 1024).map(|i| (i % 251) as u8).collect();
    stream(&mut host, &mut clients, &terrain);
    // Explicit disconnect frees the first identity's connection without ending the host.
    clients.remove(0);
    let deadline = Instant::now() + Duration::from_secs(5);
    while host.players().contains(&ids[0]) {
        tick(&mut host, &mut clients, &keys[1..]);
        assert!(Instant::now() < deadline, "disconnect not observed");
    }
    clients.insert(0, connect(address, &host, world));
    let new_ids = admit(&mut host, &mut clients, &keys);
    assert_ne!(new_ids[0], ids[0]);
    assert_eq!(new_ids[1], ids[1]);
    assert_eq!(host.fingerprint(new_ids[0]), Some(fingerprint(&keys[0].public())).as_deref());
    let mut changed = terrain;
    changed[100] ^= 0xff;
    stream(&mut host, &mut clients, &changed);
    drop(host);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut closed = [false; 2];
    while !closed.iter().all(|&value| value) {
        for (client, closed) in clients.iter_mut().zip(&mut closed) {
            if !*closed { *closed = client.poll(Instant::now()).is_err(); }
        }
        assert!(Instant::now() < deadline, "host loss not observed");
        thread::sleep(Duration::from_millis(4));
    }
}
