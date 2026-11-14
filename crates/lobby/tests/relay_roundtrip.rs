//! Real lobby on loopback with the relay transport on both ends: handshake, frames, data both ways,
//! refusals, a full session and host loss.

use engine_lobby::{directory::Limits, server::{Config, Lobby}};
use engine_net::{
    lobby::{
        relay::{RelayClient, RelayError, RelayHost},
        wire::{HostInfo, Mode, WorldParams},
    },
    protocol::{Packet, WorldIdentity},
};
use std::{net::{SocketAddr, TcpListener}, sync::Arc, thread, time::{Duration, Instant}};

fn start_lobby() -> SocketAddr {
    let lobby = Lobby::new(Config { limits: Limits { max_sessions: 64, max_per_address: 64 }, max_connections: 128 });
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = Arc::clone(&lobby);
    thread::spawn(move || {
        let _ = server.serve(listener);
    });
    addr
}

fn world(generator: u32) -> WorldIdentity {
    WorldIdentity { seed: 74, generator, cell_mm: 500, content: [9; 32] }
}

fn info(world: WorldIdentity, max_players: u8) -> HostInfo {
    HostInfo {
        name: "Relay test".into(),
        world: "Hills".into(),
        mode: Mode::Survival,
        version: world.generator,
        motd: "relay test".into(),
        digest: world.content,
        max_players,
        unlisted: false,
        world_params: WorldParams { seed: world.seed as u32, kind: 1, scale: 1 },
    }
}

/// Retries a step until it yields a value, failing the test after ten seconds.
fn wait<T>(what: &str, mut step: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = step() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn hosted_session_runs_frames_and_data_through_the_relay() {
    let addr = start_lobby();
    let w = world(62);
    let mut host = RelayHost::start(addr, info(w, 4), w).expect("host");
    let code = host.code().to_owned();
    let mut client = RelayClient::connect(addr, &code, 62, w.content, Instant::now()).expect("join");
    client.hello(w).expect("hello");

    let mut frames = 0;
    wait("the client to be welcomed", || {
        let now = Instant::now();
        host.poll(now).expect("host poll");
        host.advance().expect("advance");
        let _ = client.submit([0, 0], [0, 0], 0);
        for packet in client.poll(now).expect("client poll") {
            if matches!(packet, Packet::Frame { .. }) {
                frames += 1;
            }
        }
        client.player().map(|_| ())
    });
    let player = client.player().expect("player");
    wait("frames", || {
        let now = Instant::now();
        host.poll(now).expect("host poll");
        host.advance().expect("advance");
        let _ = client.submit([0, 0], [0, 0], 0);
        for packet in client.poll(now).expect("client poll") {
            if matches!(packet, Packet::Frame { .. }) {
                frames += 1;
            }
        }
        (frames >= 5).then_some(())
    });

    assert!(client.send_data(b"from client".to_vec()).expect("send"));
    let received = wait("client data at host", || {
        let now = Instant::now();
        host.advance().expect("advance");
        host.poll(now).expect("host poll").into_iter().find(|(id, _)| *id == player)
    });
    assert_eq!(received.1, b"from client".to_vec());

    assert!(host.send_data(player, b"from host".to_vec()).expect("host send"));
    wait("host data at client", || {
        let now = Instant::now();
        host.advance().expect("advance");
        host.poll(now).expect("host poll");
        let _ = client.submit([0, 0], [0, 0], 0);
        client
            .poll(now)
            .expect("client poll")
            .into_iter()
            .find(|packet| matches!(packet, Packet::Data(bytes) if bytes == b"from host"))
    });

    drop(host);
    wait("the client to learn the host left", || match client.poll(Instant::now()) {
        Err(RelayError::Closed(_)) => Some(()),
        Err(_) => Some(()),
        Ok(_) => None,
    });
}

#[test]
fn refusals_name_the_reason() {
    let addr = start_lobby();
    let w = world(62);
    let host = RelayHost::start(addr, info(w, 2), w).expect("host");
    let code = host.code().to_owned();

    let wrong_version = RelayClient::connect(addr, &code, 63, w.content, Instant::now());
    assert!(matches!(wrong_version, Err(RelayError::Refused(ref r)) if r == "different game version"), "{:?}", wrong_version.as_ref().err());

    let other_world = WorldIdentity { content: [1; 32], ..w };
    let wrong_world = RelayClient::connect(addr, &code, 62, other_world.content, Instant::now());
    assert!(matches!(wrong_world, Err(RelayError::Refused(ref r)) if r == "different world content"), "{:?}", wrong_world.as_ref().err());

    let unknown = RelayClient::connect(addr, "ZZZZ-ZZZZ", 62, w.content, Instant::now());
    assert!(matches!(unknown, Err(RelayError::Refused(ref r)) if r == "no session has that code"), "{:?}", unknown.as_ref().err());

    let _first = RelayClient::connect(addr, &code, 62, w.content, Instant::now()).expect("first join");
    let full = RelayClient::connect(addr, &code, 62, w.content, Instant::now());
    assert!(matches!(full, Err(RelayError::Refused(ref r)) if r == "session is full"), "{:?}", full.as_ref().err());
}

#[test]
fn browse_finds_the_session_by_name_and_hides_it_when_closed() {
    let addr = start_lobby();
    let w = world(62);
    let mut info = info(w, 4);
    info.name = "Riverside camp".into();
    let host = RelayHost::start(addr, info, w).expect("host");
    let query = engine_net::lobby::wire::Query {
        text: "riverside".into(),
        mode: None,
        version: Some(62),
        include_full: false,
        sort: Default::default(),
        offset: 0,
        limit: 10,
    };
    let (total, entries) = engine_net::lobby::relay::browse(addr, &query).expect("browse");
    assert_eq!(total, 1);
    assert_eq!(entries[0].code, host.code());
    assert_eq!(entries[0].players, 1);
    drop(host);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let (total, _) = engine_net::lobby::relay::browse(addr, &query).expect("browse");
        if total == 0 || Instant::now() > deadline {
            assert_eq!(total, 0, "closed session still listed");
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_joiner_learns_the_host_world_before_building_it() {
    let addr = start_lobby();
    // The host's session identity and its advertised parameters describe the same world.
    let w = WorldIdentity { seed: 9001, generator: 62, cell_mm: 250, content: [9; 32] };
    let mut info = info(w, 4);
    info.world_params = engine_net::lobby::wire::WorldParams { seed: 9001, kind: 2, scale: 2 };
    let mut host = RelayHost::start(addr, info, w).expect("host");
    let code = host.code().to_owned();

    // Version 0 accepts whatever the host runs. The digest still has to match.
    let mut client = RelayClient::connect(addr, &code, 0, w.content, Instant::now()).expect("join");
    let joined = client.joined();
    assert_eq!(joined.version, 62);
    assert_eq!(joined.world, engine_net::lobby::wire::WorldParams { seed: 9001, kind: 2, scale: 2 });

    // The joiner builds the world from those parameters, so its identity matches the host's.
    let built = WorldIdentity { seed: u64::from(joined.world.seed), generator: joined.version, cell_mm: 250, content: w.content };
    client.hello(built).expect("hello");
    let player = wait("the joiner to be welcomed", || {
        let now = Instant::now();
        host.poll(now).expect("host poll");
        host.advance().expect("advance");
        let _ = client.submit([0, 0], [0, 0], 0);
        client.poll(now).expect("client poll");
        client.player()
    });
    assert!(host.players().contains(&player));
}
