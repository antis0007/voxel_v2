//! Connection handling and relay routing. Each connection has a reader thread and a writer thread
//! fed by a bounded queue. A consumer that falls behind is disconnected, never silently skipped:
//! the game's frames are ordered and reliable, so a dropped frame would corrupt the session.

use crate::directory::{Directory, Limits, RegisterError};
use engine_net::lobby::wire::{self, Entry, HostInfo, Message, PROTOCOL, Query};
use std::{
    collections::HashMap,
    io::{self, BufReader, BufWriter, Write},
    net::{IpAddr, Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

pub const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const OUTBOX: usize = 256;
/// Browse requests per connection per second. A browse is a full pass over the directory.
const BROWSES_PER_SECOND: u32 = 8;
/// Twice the host session's own per-peer budget, so only a flood trips it.
const PEER_MESSAGES_PER_SECOND: u32 = 240;

/// Fixed one-second window. Simple, and enough to stop a single connection from flooding queries.
struct BrowseBudget {
    window_start: Instant,
    used: u32,
}

impl BrowseBudget {
    fn new(now: Instant) -> Self {
        Self { window_start: now, used: 0 }
    }

    fn allow(&mut self, now: Instant) -> bool {
        if now.duration_since(self.window_start) >= Duration::from_secs(1) {
            self.window_start = now;
            self.used = 0;
        }
        if self.used >= BROWSES_PER_SECOND {
            return false;
        }
        self.used += 1;
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub limits: Limits,
    pub max_connections: usize,
}

struct Route {
    host: SyncSender<Message>,
    peers: HashMap<u32, SyncSender<Message>>,
    next_peer: u32,
    direct: Option<Endpoint>,
}

#[derive(Clone, Copy)]
enum Endpoint {
    Address(std::net::SocketAddr),
    Steam(u64),
}

/// Unknown-code joins allowed per address per window. Codes are 40 bits, so a guess succeeds with
/// probability near zero; this bound makes even a live-session search impractical.
const FAILED_JOINS: u32 = 10;
const FAILED_WINDOW: Duration = Duration::from_secs(60);
const FAILURE_TABLE_LIMIT: usize = 4096;
const UNKNOWN_CODE: &str = "no session has that code";

struct Failures {
    window_start: Instant,
    count: u32,
}

struct Registry {
    directory: Directory,
    routes: HashMap<u32, Route>,
    failures: HashMap<IpAddr, Failures>,
}

pub struct Lobby {
    registry: Mutex<Registry>,
    connections: AtomicUsize,
    max_connections: usize,
}

impl Lobby {
    pub fn new(config: Config) -> Arc<Self> {
        Arc::new(Self {
            registry: Mutex::new(Registry {
                directory: Directory::new(config.limits),
                routes: HashMap::new(),
                failures: HashMap::new(),
            }),
            connections: AtomicUsize::new(0),
            max_connections: config.max_connections,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Registry> {
        // A panicked connection must not take the directory down with it.
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn join_blocked(&self, address: IpAddr, now: Instant) -> bool {
        self.lock().failures.get(&address).is_some_and(|failures| {
            now.duration_since(failures.window_start) < FAILED_WINDOW && failures.count >= FAILED_JOINS
        })
    }

    fn record_failed_join(&self, address: IpAddr, now: Instant) {
        let mut registry = self.lock();
        if registry.failures.len() >= FAILURE_TABLE_LIMIT {
            registry.failures.retain(|_, failures| now.duration_since(failures.window_start) < FAILED_WINDOW);
        }
        if registry.failures.len() >= FAILURE_TABLE_LIMIT && !registry.failures.contains_key(&address) {
            return;
        }
        let failures = registry.failures.entry(address).or_insert(Failures { window_start: now, count: 0 });
        if now.duration_since(failures.window_start) >= FAILED_WINDOW {
            failures.window_start = now;
            failures.count = 0;
        }
        failures.count += 1;
    }

    pub fn browse(&self, query: &Query) -> (u32, Vec<Entry>) {
        self.lock().directory.browse(query, Instant::now())
    }

    pub fn session_count(&self) -> usize {
        self.lock().directory.len()
    }

    /// Accepts connections until the listener fails. Each connection runs on its own thread.
    pub fn serve(self: &Arc<Self>, listener: TcpListener) -> io::Result<()> {
        for stream in listener.incoming() {
            let stream = match stream {
                Ok(stream) => stream,
                Err(error) => {
                    eprintln!("[lobby] accept failed: {error}");
                    continue;
                }
            };
            if self.connections.fetch_add(1, Ordering::AcqRel) >= self.max_connections {
                self.connections.fetch_sub(1, Ordering::AcqRel);
                refuse_and_close(stream, "lobby is busy");
                continue;
            }
            let lobby = Arc::clone(self);
            thread::spawn(move || {
                // Released on unwind too, so a panicking handler cannot leak a slot.
                let _slot = Slot(&lobby);
                let _ = handle(&lobby, stream);
            });
        }
        Ok(())
    }
}

struct Slot<'a>(&'a Lobby);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

fn refuse_and_close(mut stream: TcpStream, reason: &str) {
    let _ = wire::write_message(&mut stream, &Message::Refused { reason: reason.into() });
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

fn spawn_writer(stream: TcpStream, outbox: Receiver<Message>) {
    thread::spawn(move || {
        let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
        let mut out = BufWriter::new(&stream);
        for message in outbox {
            if wire::write_message(&mut out, &message).and_then(|()| out.flush()).is_err() {
                break;
            }
        }
        let _ = stream.shutdown(Shutdown::Both);
    });
}

fn handle(lobby: &Lobby, stream: TcpStream) -> io::Result<()> {
    let owner = stream.peer_addr()?.ip();
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(HELLO_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let (outbox, inbox) = mpsc::sync_channel(OUTBOX);
    spawn_writer(stream.try_clone()?, inbox);
    match wire::read_message(&mut reader)? {
        Message::Hello { protocol: PROTOCOL } => {}
        Message::Hello { .. } => {
            let _ = outbox.send(Message::Refused { reason: "unsupported lobby protocol".into() });
            return Ok(());
        }
        _ => {
            let _ = outbox.send(Message::Refused { reason: "expected hello".into() });
            return Ok(());
        }
    }
    let _ = outbox.send(Message::Ready { protocol: PROTOCOL });
    reader.get_ref().set_read_timeout(Some(IDLE_TIMEOUT))?;
    let mut budget = BrowseBudget::new(Instant::now());
    loop {
        let Ok(message) = wire::read_message(&mut reader) else { return Ok(()) };
        match message {
            Message::Browse(query) => {
                if !budget.allow(Instant::now()) {
                    let _ = outbox.send(Message::Refused { reason: "slow down".into() });
                    continue;
                }
                let (total, entries) = lobby.browse(&query);
                let _ = outbox.send(Message::Listing { total, entries });
            }
            Message::Host(info) => return host_session(lobby, &outbox, &mut reader, owner, info, None),
            Message::HostDirect { info, port } =>
                return host_session(lobby, &outbox, &mut reader, owner, info, Some(Endpoint::Address(std::net::SocketAddr::new(owner, port)))),
            Message::HostSteam { info, identity } =>
                return host_session(lobby, &outbox, &mut reader, owner, info, Some(Endpoint::Steam(identity))),
            Message::Join { code, version, digest } => {
                return client_session(lobby, &outbox, &mut reader, owner, &code, version, digest);
            }
            Message::Ping => {}
            _ => {
                let _ = outbox.send(Message::Refused { reason: "unexpected message".into() });
                return Ok(());
            }
        }
    }
}

fn host_session(
    lobby: &Lobby,
    outbox: &SyncSender<Message>,
    reader: &mut BufReader<TcpStream>,
    owner: IpAddr,
    info: HostInfo,
    endpoint: Option<Endpoint>,
) -> io::Result<()> {
    let registered = {
        let mut registry = lobby.lock();
        registry.directory.register(owner, &info, Instant::now()).map(|(id, code)| {
            registry.routes.insert(id, Route {
                host: outbox.clone(), peers: HashMap::new(), next_peer: 1,
                direct: endpoint,
            });
            (id, code)
        })
    };
    let (id, code) = match registered {
        Ok(registered) => registered,
        Err(error) => {
            let reason = match error {
                RegisterError::Full => "lobby has no free session slots",
                RegisterError::TooManyFromAddress => "too many sessions from this address",
                RegisterError::CodeBusy => "lobby could not allocate a code",
                RegisterError::Entropy => "lobby cannot issue codes right now",
            };
            let _ = outbox.send(Message::Refused { reason: reason.into() });
            return Ok(());
        }
    };
    eprintln!("[lobby] hosted code={code} mode={:?} max={} name={:?}", info.mode, info.max_players, info.name);
    let _ = outbox.send(Message::Hosted { code: code.clone() });
    let result = host_loop(lobby, id, reader);
    close_session(lobby, id);
    eprintln!("[lobby] closed code={code}");
    result
}

fn host_loop(lobby: &Lobby, id: u32, reader: &mut BufReader<TcpStream>) -> io::Result<()> {
    loop {
        let Ok(message) = wire::read_message(reader) else { return Ok(()) };
        match message {
            Message::Presence { players } => {
                let mut registry = lobby.lock();
                if !registry.routes.get(&id).is_some_and(|route| route.direct.is_some()) { return Ok(()); }
                registry.directory.set_players(id, players);
            }
            Message::ToPeer { peer, bytes } => {
                let mut registry = lobby.lock();
                let delivered = registry.routes.get(&id).and_then(|route| route.peers.get(&peer)).map(|tx| tx.try_send(Message::FromHost { bytes }).is_ok());
                if delivered == Some(false) {
                    if let Some(route) = registry.routes.get_mut(&id) {
                        route.peers.remove(&peer);
                    }
                    sync_players(&mut registry, id);
                    notify_host(&registry, id, Message::PeerLeft { peer });
                }
            }
            Message::Kick { peer } => {
                let mut registry = lobby.lock();
                if let Some(route) = registry.routes.get_mut(&id) {
                    if let Some(peer_tx) = route.peers.remove(&peer) {
                        let _ = peer_tx.try_send(Message::Closed { reason: "removed by host".into() });
                    }
                }
                sync_players(&mut registry, id);
            }
            Message::Ping => {}
            _ => return Ok(()),
        }
    }
}

fn close_session(lobby: &Lobby, id: u32) {
    let mut registry = lobby.lock();
    if let Some(route) = registry.routes.remove(&id) {
        for peer in route.peers.values() {
            let _ = peer.try_send(Message::Closed { reason: "host left".into() });
        }
    }
    registry.directory.remove(id);
}

fn client_session(
    lobby: &Lobby,
    outbox: &SyncSender<Message>,
    reader: &mut BufReader<TcpStream>,
    owner: IpAddr,
    code: &str,
    version: u32,
    digest: wire::Digest,
) -> io::Result<()> {
    let now = Instant::now();
    if lobby.join_blocked(owner, now) {
        let _ = outbox.send(Message::Refused { reason: "too many failed joins; wait a minute".into() });
        return Ok(());
    }
    let direct = {
        let registry = lobby.lock();
        registry.directory.find_code(code).and_then(|session| {
            let endpoint = registry.routes.get(&session.id)?.direct?;
            Some(if version != 0 && version != session.info.version {
                Message::Refused { reason: "different game version".into() }
            } else if digest != session.info.digest {
                Message::Refused { reason: "different world content".into() }
            } else {
                let version = session.info.version;
                let world = session.info.world_params;
                match endpoint {
                    Endpoint::Address(address) => Message::Direct { address, version, digest, world },
                    Endpoint::Steam(identity) => Message::Steam { identity, version, digest, world },
                }
            })
        })
    };
    if let Some(reply) = direct {
        let _ = outbox.send(reply);
        return Ok(());
    }
    let joined = {
        let mut registry = lobby.lock();
        let found = registry
            .directory
            .find_code(code)
            .map(|session| (session.id, session.info.version, session.info.digest, session.info.max_players, session.info.world_params));
        let outcome = match found {
            None => Err(UNKNOWN_CODE),
            Some((_, host_version, _, _, _)) if version != 0 && host_version != version => Err("different game version"),
            Some((_, _, host_digest, _, _)) if host_digest != digest => Err("different world content"),
            Some((id, host_version, _, max_players, world)) => match registry.routes.get_mut(&id) {
                None => Err("session is closing"),
                Some(route) if route.direct.is_some() => Err("session changed; retry"),
                Some(route) if 1 + route.peers.len() >= usize::from(max_players) => Err("session is full"),
                Some(route) => {
                    let peer = route.next_peer;
                    route.next_peer = route.next_peer.wrapping_add(1).max(1);
                    route.peers.insert(peer, outbox.clone());
                    let _ = route.host.try_send(Message::PeerJoined { peer });
                    Ok((id, peer, host_version, world))
                }
            },
        };
        if let Ok((id, _, _, _)) = outcome {
            sync_players(&mut registry, id);
        }
        outcome
    };
    let (id, peer, host_version, world) = match joined {
        Ok(joined) => joined,
        Err(reason) => {
            if reason == UNKNOWN_CODE {
                lobby.record_failed_join(owner, now);
            }
            let _ = outbox.send(Message::Refused { reason: reason.into() });
            return Ok(());
        }
    };
    let _ = outbox.send(Message::Joined { peer, version: host_version, digest, world });
    let result = peer_loop(lobby, id, peer, reader);
    leave_session(lobby, id, peer);
    result
}

fn peer_loop(lobby: &Lobby, id: u32, peer: u32, reader: &mut BufReader<TcpStream>) -> io::Result<()> {
    let (mut window, mut sent) = (Instant::now(), 0u32);
    loop {
        let Ok(message) = wire::read_message(reader) else { return Ok(()) };
        let now = Instant::now();
        if now.duration_since(window) >= Duration::from_secs(1) {
            (window, sent) = (now, 0);
        }
        sent += 1;
        if sent > PEER_MESSAGES_PER_SECOND {
            return Ok(());
        }
        match message {
            Message::ToHost { bytes } => {
                // A kicked peer is gone from the route; its socket must not keep talking.
                let host = lobby
                    .lock()
                    .routes
                    .get(&id)
                    .filter(|route| route.peers.contains_key(&peer))
                    .map(|route| route.host.clone());
                let Some(host) = host else { return Ok(()) };
                {
                    if host.try_send(Message::FromPeer { peer, bytes }).is_err() {
                        // The host is not keeping up. Disconnect this peer rather than drop a frame.
                        return Ok(());
                    }
                }
            }
            Message::Ping => {}
            _ => return Ok(()),
        }
    }
}

fn leave_session(lobby: &Lobby, id: u32, peer: u32) {
    let mut registry = lobby.lock();
    let removed = registry.routes.get_mut(&id).and_then(|route| route.peers.remove(&peer)).is_some();
    if removed {
        sync_players(&mut registry, id);
        notify_host(&registry, id, Message::PeerLeft { peer });
    }
}

/// Keeps the directory's player count equal to the host plus connected peers.
fn sync_players(registry: &mut Registry, id: u32) {
    let players = registry.routes.get(&id).map(|route| 1 + route.peers.len());
    if let Some(players) = players {
        registry.directory.set_players(id, u8::try_from(players).unwrap_or(u8::MAX));
    }
}

fn notify_host(registry: &Registry, id: u32, message: Message) {
    if let Some(route) = registry.routes.get(&id) {
        let _ = route.host.try_send(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_net::lobby::wire::{Mode, Sort, WorldParams};
    use std::{net::SocketAddr, time::Duration};

    fn start(max_connections: usize) -> (SocketAddr, Arc<Lobby>) {
        let lobby = Lobby::new(Config {
            limits: Limits { max_sessions: 64, max_per_address: 64 },
            max_connections,
        });
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = Arc::clone(&lobby);
        thread::spawn(move || {
            let _ = server.serve(listener);
        });
        (addr, lobby)
    }

    struct Client {
        stream: TcpStream,
    }

    impl Client {
        fn connect(addr: SocketAddr) -> Self {
            let stream = TcpStream::connect(addr).expect("connect");
            stream.set_read_timeout(Some(Duration::from_secs(10))).expect("timeout");
            let mut client = Self { stream };
            client.send(&Message::Hello { protocol: PROTOCOL });
            assert_eq!(client.recv(), Message::Ready { protocol: PROTOCOL });
            client
        }

        fn send(&mut self, message: &Message) {
            wire::write_message(&mut self.stream, message).expect("send");
            self.stream.flush().expect("flush");
        }

        fn recv(&mut self) -> Message {
            wire::read_message(&mut self.stream).expect("recv")
        }
    }

    fn info(name: &str, max: u8) -> HostInfo {
        HostInfo { name: name.into(), world: "Hills".into(), mode: Mode::Survival, version: 3, digest: [5; 32], max_players: max, unlisted: false, world_params: WorldParams { seed: 74, kind: 1, scale: 1 }, motd: "hills at dusk".into() }
    }

    fn host(addr: SocketAddr, name: &str, max: u8) -> (Client, String) {
        let mut client = Client::connect(addr);
        client.send(&Message::Host(info(name, max)));
        let Message::Hosted { code } = client.recv() else { panic!("not hosted") };
        (client, code)
    }

    fn join(addr: SocketAddr, code: &str) -> Message {
        let mut client = Client::connect(addr);
        client.send(&Message::Join { code: code.into(), version: 3, digest: [5; 32] });
        client.recv()
    }

    fn query(text: &str) -> Query {
        Query { text: text.into(), mode: None, version: None, include_full: true, sort: Sort::Players, offset: 0, limit: 32 }
    }

    #[test]
    fn hosts_are_listed_searched_and_joined_through_the_relay() {
        let (addr, lobby) = start(64);
        let (mut host_conn, code) = host(addr, "Riverside", 4);

        let mut browser = Client::connect(addr);
        browser.send(&Message::Browse(query("river")));
        let Message::Listing { total, entries } = browser.recv() else { panic!("no listing") };
        assert_eq!((total, entries.len()), (1, 1));
        assert_eq!(entries[0].code, code);
        assert_eq!(entries[0].players, 1);

        let mut peer = Client::connect(addr);
        peer.send(&Message::Join { code: code.to_lowercase(), version: 3, digest: [5; 32] });
        let Message::Joined { peer: id, .. } = peer.recv() else { panic!("not joined") };
        assert_eq!(host_conn.recv(), Message::PeerJoined { peer: id });
        assert_eq!(lobby.browse(&query("")).1[0].players, 2);

        peer.send(&Message::ToHost { bytes: vec![1, 2, 3] });
        assert_eq!(host_conn.recv(), Message::FromPeer { peer: id, bytes: vec![1, 2, 3] });
        host_conn.send(&Message::ToPeer { peer: id, bytes: vec![9] });
        assert_eq!(peer.recv(), Message::FromHost { bytes: vec![9] });

        drop(peer);
        assert_eq!(host_conn.recv(), Message::PeerLeft { peer: id });
        assert_eq!(lobby.browse(&query("")).1[0].players, 1);
    }

    #[test]
    fn join_refusals_name_the_reason() {
        let (addr, _lobby) = start(64);
        let (_host, code) = host(addr, "Small", 1);
        assert_eq!(join(addr, "ZZZZ-ZZZZ"), Message::Refused { reason: "no session has that code".into() });
        let mut other_version = Client::connect(addr);
        other_version.send(&Message::Join { code: code.clone(), version: 4, digest: [5; 32] });
        assert_eq!(other_version.recv(), Message::Refused { reason: "different game version".into() });
        let mut other_world = Client::connect(addr);
        other_world.send(&Message::Join { code: code.clone(), version: 3, digest: [6; 32] });
        assert_eq!(other_world.recv(), Message::Refused { reason: "different world content".into() });
        assert_eq!(join(addr, &code), Message::Refused { reason: "session is full".into() });
    }

    #[test]
    fn host_leaving_closes_the_session_for_peers() {
        let (addr, lobby) = start(64);
        let (host_conn, code) = host(addr, "Brief", 4);
        let mut peer = Client::connect(addr);
        peer.send(&Message::Join { code, version: 3, digest: [5; 32] });
        assert!(matches!(peer.recv(), Message::Joined { .. }));
        drop(host_conn);
        assert_eq!(peer.recv(), Message::Closed { reason: "host left".into() });
        let deadline = Instant::now() + Duration::from_secs(5);
        while lobby.session_count() > 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(lobby.session_count(), 0);
    }

    #[test]
    fn unexpected_messages_end_the_connection() {
        let (addr, _lobby) = start(64);
        let mut client = Client::connect(addr);
        client.send(&Message::Listing { total: 0, entries: vec![] });
        assert!(matches!(client.recv(), Message::Refused { .. }));
        assert!(wire::read_message(&mut client.stream).is_err());
    }

    #[test]
    fn a_wrong_protocol_is_refused_before_anything_else() {
        let (addr, _lobby) = start(64);
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(10))).expect("timeout");
        wire::write_message(&mut stream, &Message::Hello { protocol: 99 }).expect("send");
        assert_eq!(wire::read_message(&mut stream).expect("recv"), Message::Refused { reason: "unsupported lobby protocol".into() });
    }

    #[test]
    fn browse_flood_is_refused_after_the_window_budget() {
        let (addr, _lobby) = start(64);
        let mut client = Client::connect(addr);
        for _ in 0..BROWSES_PER_SECOND + 1 {
            client.send(&Message::Browse(query("")));
        }
        for _ in 0..BROWSES_PER_SECOND {
            assert!(matches!(client.recv(), Message::Listing { .. }));
        }
        assert_eq!(client.recv(), Message::Refused { reason: "slow down".into() });
    }

    #[test]
    fn failed_join_guessing_is_refused_after_the_window_budget() {
        let (addr, _lobby) = start(64);
        let (_host, code) = host(addr, "Guarded", 4);
        for _ in 0..FAILED_JOINS {
            assert_eq!(join(addr, "ZZZZ-ZZZZ"), Message::Refused { reason: UNKNOWN_CODE.into() });
        }
        assert_eq!(join(addr, "ZZZZ-ZZZZ"), Message::Refused { reason: "too many failed joins; wait a minute".into() });
        // The same address is blocked from guessing, but a correct code is refused only by the same rule.
        assert_eq!(join(addr, &code), Message::Refused { reason: "too many failed joins; wait a minute".into() });
    }

    #[test]
    fn connection_cap_refuses_extra_clients() {
        let (addr, _lobby) = start(1);
        let _held = Client::connect(addr);
        let mut extra = TcpStream::connect(addr).expect("connect");
        extra.set_read_timeout(Some(Duration::from_secs(10))).expect("timeout");
        assert_eq!(wire::read_message(&mut extra).expect("recv"), Message::Refused { reason: "lobby is busy".into() });
    }
}
