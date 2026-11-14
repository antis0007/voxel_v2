//! Relay transport. Hosts and clients reach each other through a lobby over TCP, so no port
//! forwarding is needed. The packet and session rules match `steam.rs`: the lobby only moves
//! opaque bytes and never reads game packets.

use super::wire::{self, Entry, HostInfo, Message, Query, WorldParams};
use crate::{protocol::*, session::*};
use crate::admin::AccessList;
use std::{
    collections::BTreeMap,
    fmt,
    io::{self, BufReader, BufWriter, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const KEEPALIVE: Duration = Duration::from_secs(5);
/// Messages the outgoing link holds before a send reports full.
pub const QUEUE: usize = 256;
const RECEIVE_BUDGET: usize = 128;
const INBOX: usize = 4096;

#[derive(Debug)]
pub enum RelayError {
    Io(io::Error),
    /// The lobby answered with a reason, such as a wrong code or a full session.
    Refused(String),
    /// The lobby or the other side ended the connection.
    Closed(String),
    /// A message arrived that the current step does not allow.
    Protocol(&'static str),
    /// Game-level packet or session failure.
    Session(ProtocolError),
}

impl fmt::Display for RelayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "lobby connection failed: {error}"),
            Self::Refused(reason) => write!(f, "lobby refused: {reason}"),
            Self::Closed(reason) => write!(f, "session closed: {reason}"),
            Self::Protocol(reason) => write!(f, "lobby protocol error: {reason}"),
            Self::Session(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RelayError {}

impl From<io::Error> for RelayError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProtocolError> for RelayError {
    fn from(error: ProtocolError) -> Self {
        Self::Session(error)
    }
}

pub type RelayResult<T> = Result<T, RelayError>;

/// One lobby connection after the handshake. Reader and writer run on their own threads.
/// Dropping the link shuts the socket, which ends both threads.
struct Link {
    outgoing: SyncSender<Message>,
    /// Messages accepted by `outgoing` that the writer has not taken yet.
    queued: Arc<AtomicUsize>,
    incoming: Receiver<(Instant, Message)>,
    socket: TcpStream,
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}

/// Opens a connection, runs the synchronous handshake, then starts the I/O threads.
/// Returns the first reply after `first`, so the caller can read `Hosted` or `Joined`.
fn handshake(lobby: SocketAddr, first: Message) -> RelayResult<(TcpStream, Message, u16)> {
    match handshake_version(lobby, &first, wire::PROTOCOL) {
        // Retry only an explicit version refusal; preserve every admission or I/O error.
        Err(RelayError::Refused(reason)) if reason == "unsupported lobby protocol" => {
            handshake_version(lobby, &first, wire::LEGACY_PROTOCOL)
        }
        result => result,
    }
}

fn handshake_version(lobby: SocketAddr, first: &Message, protocol: u16) -> RelayResult<(TcpStream, Message, u16)> {
    let mut socket = TcpStream::connect_timeout(&lobby, CONNECT_TIMEOUT)?;
    socket.set_nodelay(true)?;
    socket.set_read_timeout(Some(CONNECT_TIMEOUT))?;
    socket.set_write_timeout(Some(WRITE_TIMEOUT))?;
    wire::write_message_version(&mut socket, &Message::Hello { protocol }, protocol)?;
    match wire::read_message_version(&mut socket, protocol)? {
        Message::Ready { protocol: accepted } if accepted == protocol => {}
        Message::Refused { reason } => return Err(RelayError::Refused(reason)),
        _ => return Err(RelayError::Protocol("expected ready")),
    }
    wire::write_message_version(&mut socket, first, protocol)?;
    let reply = wire::read_message_version(&mut socket, protocol)?;
    if let Message::Refused { reason } = reply {
        return Err(RelayError::Refused(reason));
    }
    Ok((socket, reply, protocol))
}

fn open(lobby: SocketAddr, first: Message) -> RelayResult<(Link, Message)> {
    let (socket, reply, protocol) = handshake(lobby, first)?;
    // Keepalive pings are the only traffic when a session is quiet, so reads wait without a timeout.
    socket.set_read_timeout(None)?;
    let (outgoing, outbox) = mpsc::sync_channel::<Message>(QUEUE);
    // Bounded: a full inbox stalls the reader, the lobby's queue to us fills, and
    // the lobby disconnects the flooding peer instead of this process growing.
    let (inbox_tx, incoming) = mpsc::sync_channel(INBOX);
    let mut reader = BufReader::new(socket.try_clone()?);
    let mut writer = BufWriter::new(socket.try_clone()?);
    thread::Builder::new().name("relay-reader".into()).spawn(move || {
        loop {
            match wire::read_message_version(&mut reader, protocol) {
                Ok(message) => {
                    if inbox_tx.send((Instant::now(), message)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = inbox_tx.send((Instant::now(), Message::Closed { reason: error.to_string() }));
                    break;
                }
            }
        }
        let _ = reader.get_ref().shutdown(Shutdown::Both);
    })?;
    let queued = Arc::new(AtomicUsize::new(0));
    let taken = Arc::clone(&queued);
    let writer_thread = thread::Builder::new().name("relay-writer".into()).spawn(move || {
        loop {
            let message = match outbox.recv_timeout(KEEPALIVE) {
                Ok(message) => {
                    taken.fetch_sub(1, Ordering::Relaxed);
                    message
                }
                Err(RecvTimeoutError::Timeout) => Message::Ping,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            if wire::write_message_version(&mut writer, &message, protocol).and_then(|()| writer.flush()).is_err() {
                break;
            }
        }
        // Wake a blocked reader immediately on write failure or shutdown.
        let _ = writer.get_ref().shutdown(Shutdown::Both);
    });
    if let Err(error) = writer_thread {
        let _ = socket.shutdown(Shutdown::Both);
        return Err(error.into());
    }
    Ok((Link { outgoing, queued, incoming, socket }, reply))
}

/// Browses the lobby directory. One request, one listing.
pub fn browse(lobby: SocketAddr, query: &Query) -> RelayResult<(u32, Vec<Entry>)> {
    let (socket, reply, _) = handshake(lobby, Message::Browse(query.clone()))?;
    drop(socket);
    match reply {
        Message::Listing { total, entries } => Ok((total, entries)),
        _ => Err(RelayError::Protocol("expected listing")),
    }
}

/// Every send goes through here so `queued` never falls below the channel's real depth.
fn send_full(link: &Link, message: Message) -> RelayResult<bool> {
    // Counted first: the writer may take the message before `try_send` returns.
    link.queued.fetch_add(1, Ordering::Relaxed);
    let sent = link.outgoing.try_send(message);
    if sent.is_err() {
        link.queued.fetch_sub(1, Ordering::Relaxed);
    }
    match sent {
        Ok(()) => Ok(true),
        Err(mpsc::TrySendError::Full(_)) => Ok(false),
        Err(mpsc::TrySendError::Disconnected(_)) => Err(RelayError::Closed("lobby link ended".into())),
    }
}

/// Like `send_full`, but a full queue is an error. Used for frames that must not be dropped.
fn send_ordered(link: &Link, message: Message) -> RelayResult<()> {
    if send_full(link, message)? {
        Ok(())
    } else {
        Err(RelayError::Session(ProtocolError("slow peer: send queue exceeded")))
    }
}

/// TCP registration only. A direct session never allocates relay peers.
pub struct RendezvousHost {
    link: Link,
    code: String,
    updated: Instant,
}

impl RendezvousHost {
    pub fn start(lobby: SocketAddr, info: HostInfo, port: u16) -> RelayResult<Self> {
        Self::register(lobby, Message::HostDirect { info, port })
    }

    pub fn start_steam(lobby: SocketAddr, info: HostInfo, identity: u64) -> RelayResult<Self> {
        Self::register(lobby, Message::HostSteam { info, identity })
    }

    fn register(lobby: SocketAddr, request: Message) -> RelayResult<Self> {
        let (link, reply) = open(lobby, request)?;
        let Message::Hosted { code } = reply else {
            return Err(RelayError::Protocol("expected hosted"));
        };
        Ok(Self { link, code, updated: Instant::now() })
    }

    pub fn code(&self) -> &str { &self.code }

    pub fn presence(&mut self, now: Instant, players: usize) -> RelayResult<()> {
        if now.saturating_duration_since(self.updated) >= Duration::from_secs(2) {
            if send_full(&self.link, Message::Presence { players: players.min(255) as u8 })? {
                self.updated = now;
            }
        }
        Ok(())
    }
}

pub enum InviteEndpoint {
    Address(SocketAddr),
    Steam(u64),
}

pub fn resolve_invite(lobby: SocketAddr, code: &str, digest: wire::Digest)
    -> RelayResult<(InviteEndpoint, Joined)>
{
    let (_link, reply) = open(lobby, Message::Join { code: code.to_owned(), version: 0, digest })?;
    let (endpoint, version, offered, world) = match reply {
        Message::Direct { address, version, digest, world } if address.port() != 0 =>
            (InviteEndpoint::Address(address), version, digest, world),
        Message::Steam { identity, version, digest, world } if identity != 0 =>
            (InviteEndpoint::Steam(identity), version, digest, world),
        _ => return Err(RelayError::Protocol("session does not offer native multiplayer")),
    };
    if offered != digest {
        return Err(RelayError::Protocol("invalid multiplayer offer"));
    }
    Ok((endpoint, Joined { version, world }))
}

pub struct RelayHost {
    link: Link,
    code: String,
    session: HostSession,
    player_of_peer: BTreeMap<u32, PlayerId>,
    peer_of_player: BTreeMap<PlayerId, u32>,
}

impl RelayHost {
    /// Registers a session with the lobby and returns once it is listed.
    pub fn start(lobby: SocketAddr, info: HostInfo, world: WorldIdentity) -> RelayResult<Self> {
        let (link, reply) = open(lobby, Message::Host(info))?;
        let Message::Hosted { code } = reply else {
            return Err(RelayError::Protocol("expected hosted"));
        };
        Ok(Self {
            link,
            code,
            session: HostSession::new(world),
            player_of_peer: BTreeMap::new(),
            peer_of_player: BTreeMap::new(),
        })
    }

    /// The code players type or pick in the browser.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Installs the operator, whitelist and ban lists. A refused name is dropped
    /// at the point it is introduced, so a banned player never joins the roster.
    pub fn set_access(&mut self, access: AccessList, whitelist: bool) {
        self.session.set_access(access, whitelist);
    }
    pub fn set_capacity(&mut self, players: usize) { self.session.set_capacity(players); }

    /// Live lists for the admin commands. `None` until `set_access` is called.
    pub fn access_mut(&mut self) -> Option<&mut AccessList> {
        self.session.access_mut()
    }

    /// The lists as they stand, for saving after a change.
    pub fn access(&self) -> Option<&AccessList> {
        self.session.access()
    }

    /// Names of the players currently joined.
    pub fn roster(&self) -> Vec<(PlayerId, String)> {
        self.session.roster()
    }

    /// Command lines typed by players since the last call.
    pub fn take_commands(&mut self) -> Vec<(PlayerId, String)> {
        self.session.take_commands()
    }

    /// Messages waiting in the shared link to the lobby, out of `QUEUE`.
    /// A full link kicks peers at `advance`, so bulk senders back off before it fills.
    pub fn backlog(&self) -> usize {
        self.link.queued.load(Ordering::Relaxed)
    }

    pub fn players(&self) -> Vec<PlayerId> {
        self.peer_of_player.keys().copied().filter(|id| self.session.is_joined(*id)).collect()
    }

    fn forget(&mut self, peer: u32) {
        if let Some(player) = self.player_of_peer.remove(&peer) {
            self.peer_of_player.remove(&player);
            self.session.disconnect(player);
        }
    }

    /// Removes a peer locally and tells the lobby to close its connection.
    fn kick(&mut self, peer: u32) {
        self.forget(peer);
        let _ = send_full(&self.link, Message::Kick { peer });
    }

    pub fn disconnect(&mut self, player: PlayerId) {
        if let Some(peer) = self.peer_of_player.get(&player).copied() {
            self.kick(peer);
        }
    }

    pub fn send_data(&self, player: PlayerId, bytes: Vec<u8>) -> RelayResult<bool> {
        self.send_packet(player, &Packet::Data(bytes))
    }

    /// Sends a protocol packet as itself; `send_data` would wrap it as opaque bytes.
    pub fn send_packet(&self, player: PlayerId, packet: &Packet) -> RelayResult<bool> {
        let peer = *self.peer_of_player.get(&player).ok_or(ProtocolError("unknown player"))?;
        send_full(&self.link, Message::ToPeer { peer, bytes: packet.encode()? })
    }

    /// The key fingerprint `player` proved when it introduced itself.
    pub fn fingerprint(&self, player: PlayerId) -> Option<&str> {
        self.session.fingerprint(player)
    }

    /// Whether `player` holds an operator entry bound to its proven key.
    pub fn holds_op(&self, player: PlayerId) -> bool {
        self.session.holds_op(player)
    }

    /// Applies `/name` under the same admission checks as an introduction.
    pub fn rename(&mut self, player: PlayerId, wanted: &str) -> RelayResult<String> {
        Ok(self.session.rename(player, wanted)?)
    }

    pub fn poll(&mut self, now: Instant) -> RelayResult<Vec<(PlayerId, Vec<u8>)>> {
        let mut application = Vec::new();
        for _ in 0..RECEIVE_BUDGET {
            let (arrived, message) = match self.link.incoming.try_recv() {
                Ok(message) => message,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Err(RelayError::Closed("lobby link ended".into())),
            };
            match message {
                Message::PeerJoined { peer } => match self.session.connect(now) {
                    Ok(player) => {
                        self.player_of_peer.insert(peer, player);
                        self.peer_of_player.insert(player, peer);
                    }
                    Err(_) => self.kick(peer),
                },
                Message::PeerLeft { peer } => self.forget(peer),
                Message::FromPeer { peer, bytes } => {
                    let Some(&player) = self.player_of_peer.get(&peer) else { continue };
                    let decoded = Packet::decode(&bytes);
                    let data = match &decoded {
                        Ok(Packet::Data(payload)) => Some(payload.clone()),
                        _ => None,
                    };
                    // The reader keeps receiving while a world checkpoint or realm
                    // load stalls the host. Count traffic in its arrival window.
                    match decoded.and_then(|packet| self.session.receive_at(player, packet, now, arrived)) {
                        Ok(Some(reply)) => {
                            let sent = reply.encode().map(|bytes| matches!(send_full(&self.link, Message::ToPeer { peer, bytes }), Ok(true)));
                            if !matches!(sent, Ok(true)) {
                                self.kick(peer);
                            }
                        }
                        Ok(None) => {
                            if let Some(payload) = data {
                                application.push((player, payload));
                            }
                        }
                        Err(error) => {
                            eprintln!("[relay] player {player} removed: {error}");
                            self.kick(peer);
                        }
                    }
                }
                Message::Closed { reason } => return Err(RelayError::Closed(reason)),
                _ => {}
            }
        }
        for player in self.session.expired(now) {
            if let Some(peer) = self.peer_of_player.get(&player).copied() {
                eprintln!("[relay] player {player} removed: no traffic within the session limit");
                self.kick(peer);
            }
        }
        Ok(application)
    }

    /// Host game calls at its fixed-step boundary, then applies the returned frame.
    /// A peer whose queue cannot take the frame is removed rather than skipped.
    pub fn advance(&mut self) -> RelayResult<Packet> {
        let frame = self.session.advance()?;
        let encoded = frame.encode()?;
        let targets: Vec<(PlayerId, u32)> = self
            .peer_of_player
            .iter()
            .filter(|(player, _)| self.session.is_joined(**player))
            .map(|(player, peer)| (*player, *peer))
            .collect();
        for (_, peer) in targets {
            if !matches!(send_full(&self.link, Message::ToPeer { peer, bytes: encoded.clone() }), Ok(true)) {
                self.kick(peer);
            }
        }
        Ok(frame)
    }
}

/// What the lobby reports when a join succeeds: the host's generator version and world parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Joined {
    pub version: u32,
    pub world: WorldParams,
}

pub struct RelayClient {
    link: Link,
    joined: Joined,
    hello_sent: bool,
    started: Instant,
    last_frame: Instant,
    player: Option<PlayerId>,
    tick: u64,
    sequence: u64,
    last_sent_tick: u64,
    failed: bool,
    world: Option<WorldIdentity>,
    challenge: Option<[u8; 32]>,
    introduced: bool,
}

impl RelayClient {
    /// Joins a session by code. `version` must match the host's; the lobby refuses other builds.
    /// Joins by code and returns the host's world parameters. The caller builds that world, then
    /// calls `hello` with its identity. `version` 0 accepts whatever the host runs; the digest must match.
    pub fn connect(lobby: SocketAddr, code: &str, version: u32, digest: wire::Digest, now: Instant) -> RelayResult<Self> {
        let (link, reply) = open(lobby, Message::Join { code: code.into(), version, digest })?;
        let Message::Joined { version, world, .. } = reply else {
            return Err(RelayError::Protocol("expected joined"));
        };
        Ok(Self {
            link,
            joined: Joined { version, world },
            hello_sent: false,
            started: now,
            last_frame: now,
            player: None,
            tick: 0,
            sequence: 0,
            last_sent_tick: 0,
            failed: false,
            world: None,
            challenge: None,
            introduced: false,
        })
    }

    pub fn player(&self) -> Option<PlayerId> {
        self.player
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn joined(&self) -> Joined {
        self.joined
    }

    /// Sends the identity the joiner built from `joined()`. The host refuses any other identity.
    pub fn hello(&mut self, world: WorldIdentity) -> RelayResult<()> {
        if self.hello_sent {
            return Err(ProtocolError("hello already sent").into());
        }
        send_ordered(&self.link, Message::ToHost { bytes: Packet::Hello(world).encode()? })?;
        self.hello_sent = true;
        self.world = Some(world);
        Ok(())
    }

    /// Whether `introduce` can be called: the host's welcome has arrived.
    pub fn can_introduce(&self) -> bool {
        self.challenge.is_some() && !self.introduced
    }

    /// Names this player and proves `key`. Send once, after the welcome and
    /// before any data or input: a host with access lists refuses both until then.
    pub fn introduce(&mut self, name: &str, key: &crate::identity::PlayerKey) -> RelayResult<()> {
        let (Some(challenge), Some(world)) = (self.challenge, self.world) else {
            return Err(ProtocolError("no welcome yet").into());
        };
        if self.introduced {
            return Err(ProtocolError("already introduced").into());
        }
        let packet = Packet::Introduce {
            name: name.to_owned(),
            key: key.public(),
            signature: key.sign_introduce(&challenge, &world),
        };
        send_ordered(&self.link, Message::ToHost { bytes: packet.encode()? })?;
        self.introduced = true;
        Ok(())
    }

    /// Same contract as `SteamClient::poll`: any error ends the session for good.
    pub fn poll(&mut self, now: Instant) -> RelayResult<Vec<Packet>> {
        if self.failed {
            return Err(ProtocolError("session closed").into());
        }
        let result = self.poll_inner(now);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn poll_inner(&mut self, now: Instant) -> RelayResult<Vec<Packet>> {
        let mut frames = Vec::new();
        for _ in 0..RECEIVE_BUDGET {
            let (_, message) = match self.link.incoming.try_recv() {
                Ok(message) => message,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Err(RelayError::Closed("lobby link ended".into())),
            };
            match message {
                Message::FromHost { bytes } => {
                    let packet = Packet::decode(&bytes)?;
                    match packet {
                        packet @ (Packet::Data(_) | Packet::Roster { .. } | Packet::Chat { .. })
                            if self.player.is_some() =>
                        {
                            self.last_frame = now;
                            frames.push(packet);
                        }
                        Packet::Welcome { player, tick, challenge } if self.player.is_none() => {
                            self.challenge = Some(challenge);
                            self.player = Some(player);
                            self.tick = tick;
                            self.last_frame = now;
                        }
                        frame @ Packet::Frame { .. } if self.player.is_some() => {
                            let Packet::Frame { tick, ref players } = frame else {
                                unreachable!()
                            };
                            if Some(tick) != self.tick.checked_add(1) {
                                return Err(ProtocolError("frame gap").into());
                            }
                            if !players.iter().any(|(id, _)| Some(*id) == self.player) {
                                return Err(ProtocolError("removed from roster").into());
                            }
                            self.tick = tick;
                            self.last_frame = now;
                            frames.push(frame);
                        }
                        _ => return Err(ProtocolError("unexpected host packet").into()),
                    }
                }
                Message::Closed { reason } => return Err(RelayError::Closed(reason)),
                _ => {}
            }
        }
        if (self.player.is_none() && now.duration_since(self.started) >= HANDSHAKE_TIMEOUT)
            || now.duration_since(self.last_frame) >= IDLE_TIMEOUT
        {
            return Err(ProtocolError("server timeout").into());
        }
        Ok(frames)
    }

    /// Supply one intent per observed host tick, including neutral input when idle.
    pub fn submit(&mut self, movement: [i16; 2], look: [i16; 2], buttons: u16) -> RelayResult<()> {
        if self.failed || self.player.is_none() {
            return Err(ProtocolError("session not ready").into());
        }
        if buttons & !0x000f != 0 {
            return Err(ProtocolError("invalid buttons").into());
        }
        let tick = self.tick.checked_add(INPUT_LEAD).ok_or(ProtocolError("tick exhausted"))?;
        if tick <= self.last_sent_tick {
            return Ok(());
        }
        let sequence = self.sequence.checked_add(1).ok_or(ProtocolError("sequence exhausted"))?;
        let bytes = Packet::Input(Input { sequence, tick, movement, look, buttons }).encode()?;
        send_ordered(&self.link, Message::ToHost { bytes })?;
        self.sequence = sequence;
        self.last_sent_tick = tick;
        Ok(())
    }

    /// `false` when the outgoing queue is full; the caller may retry on a later step.
    pub fn send_data(&self, bytes: Vec<u8>) -> RelayResult<bool> {
        if self.failed || self.player.is_none() {
            return Err(ProtocolError("session not ready").into());
        }
        send_full(&self.link, Message::ToHost { bytes: Packet::Data(bytes).encode()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_name_their_cause() {
        assert_eq!(RelayError::Refused("session is full".into()).to_string(), "lobby refused: session is full");
        assert_eq!(RelayError::Protocol("expected ready").to_string(), "lobby protocol error: expected ready");
    }
}
