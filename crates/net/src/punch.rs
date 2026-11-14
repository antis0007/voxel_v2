//! Invite signaling for the native direct-IP transport. Only loopback packets
//! are adapted. The public socket is used for STUN, punches and game traffic.
mod stun;

use crate::protocol::WorldIdentity;
use std::{
    collections::BTreeMap,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc},
    thread,
    time::{Duration, Instant},
};

pub const OFFER_PREFIX: &str = "VX-O1:";
pub const FRIENDS_PREFIX: &str = "VX-O2:";
pub const ANSWER_PREFIX: &str = "VX-A1:";
pub const CODE_LIMIT: usize = 384;
const RETRY: Duration = Duration::from_millis(200);
const REPLY_TIMEOUT: Duration = Duration::from_secs(300);
const FRIENDS_TIMEOUT: Duration = Duration::from_secs(35);
const TIMEOUT: Duration = Duration::from_secs(90);
const LIMIT: usize = 16;
const MAGIC: &[u8; 8] = b"VXPNCH01";
const PROBE: u8 = 0;
const SELECT: u8 = 1;
type Token = [u8; 16];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub world: WorldIdentity,
    pub kind: u8,
    token: Token,
    addresses: Vec<SocketAddrV4>,
    signaling_host: Option<u64>,
}

#[derive(Clone)]
struct Answer {
    host: Token,
    guest: Token,
    addresses: Vec<SocketAddrV4>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn token() -> io::Result<Token> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("Could not create invite key"))?;
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex<const N: usize>(text: &str) -> io::Result<[u8; N]> {
    if text.len() != N * 2 || !text.is_ascii() { return Err(invalid("Invalid invite key")); }
    let mut bytes = [0; N];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|_| invalid("Invalid invite key"))?;
    }
    Ok(bytes)
}

fn fields<'a, const N: usize>(code: &'a str, prefix: &str) -> io::Result<[&'a str; N]> {
    let code = code.trim();
    if code.len() > CODE_LIMIT { return Err(invalid("Invite code is too long")); }
    let body = code.strip_prefix(prefix).ok_or_else(|| invalid("Wrong invite code type"))?;
    body.split('|').collect::<Vec<_>>().try_into().map_err(|_| invalid("Invalid invite fields"))
}

fn addresses(text: &str) -> io::Result<Vec<SocketAddrV4>> {
    let mut addresses = Vec::new();
    for address in text.split(',') {
        let address: SocketAddrV4 = address.parse().map_err(|_| invalid("Invalid invite address"))?;
        if address.port() == 0 || address.ip().is_unspecified() || address.ip().is_broadcast()
            || address.ip().is_multicast() || addresses.len() == 4
        {
            return Err(invalid("Invalid invite address"));
        }
        if !addresses.contains(&address) { addresses.push(address); }
    }
    Ok(addresses)
}

fn display_addresses(addresses: &[SocketAddrV4]) -> String {
    addresses.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
}

impl Offer {
    pub fn decode(code: &str) -> io::Result<Self> {
        let code = code.trim();
        let ([seed, generator, cell_mm, kind, digest, key, peers], signaling_host) = if code.starts_with(FRIENDS_PREFIX) {
            let [seed, generator, cell_mm, kind, digest, key, peers, host] = fields(code, FRIENDS_PREFIX)?;
            let host = host.parse::<u64>().ok().filter(|host| *host != 0).ok_or_else(|| invalid("Invalid Steam identity"))?;
            ([seed, generator, cell_mm, kind, digest, key, peers], Some(host))
        } else {
            (fields(code, OFFER_PREFIX)?, None)
        };
        Ok(Self {
            world: WorldIdentity {
                seed: seed.parse().map_err(|_| invalid("Invalid world seed"))?,
                generator: generator.parse().map_err(|_| invalid("Invalid world generator"))?,
                cell_mm: cell_mm.parse().map_err(|_| invalid("Invalid voxel scale"))?,
                content: unhex(digest)?,
            },
            kind: kind.parse().map_err(|_| invalid("Invalid world kind"))?,
            token: unhex(key)?,
            addresses: addresses(peers)?,
            signaling_host,
        })
    }

    pub fn signaling_host(&self) -> Option<u64> { self.signaling_host }

    fn encode(&self) -> String {
        let WorldIdentity { seed, generator, cell_mm, content } = self.world;
        let body = format!("{seed}|{generator}|{cell_mm}|{}|{}|{}|{}", self.kind,
            hex(&content), hex(&self.token), display_addresses(&self.addresses));
        match self.signaling_host {
            Some(host) => format!("{FRIENDS_PREFIX}{body}|{host}"),
            None => format!("{OFFER_PREFIX}{body}"),
        }
    }
}

pub fn is_offer(code: &str) -> bool {
    code.starts_with(OFFER_PREFIX) || code.starts_with(FRIENDS_PREFIX)
}

impl Answer {
    fn decode(code: &str) -> io::Result<Self> {
        let [host, guest, peers] = fields(code, ANSWER_PREFIX)?;
        Ok(Self { host: unhex(host)?, guest: unhex(guest)?, addresses: addresses(peers)? })
    }

    fn encode(&self) -> String {
        format!("{ANSWER_PREFIX}{}|{}|{}", hex(&self.host), hex(&self.guest), display_addresses(&self.addresses))
    }

    fn packet(&self, kind: u8) -> [u8; 41] {
        let mut packet = [0; 41];
        packet[..8].copy_from_slice(MAGIC);
        packet[8..24].copy_from_slice(&self.host);
        packet[24..40].copy_from_slice(&self.guest);
        packet[40] = kind;
        packet
    }
}

enum Event { Code(String), Connected(SocketAddrV4), Error(io::Error) }

pub struct Bridge {
    host: Option<Token>,
    commands: mpsc::SyncSender<Answer>,
    events: mpsc::Receiver<Event>,
    live: Arc<AtomicBool>,
    code: Option<String>,
    connected: bool,
    local: Option<SocketAddr>,
    automatic: bool,
    #[cfg(feature = "steamworks")]
    signal_host: Option<crate::steam::invites::Host>,
    #[cfg(feature = "steamworks")]
    signal_guest: Option<crate::steam::invites::Guest>,
}

impl Bridge {
    pub fn host(local: SocketAddr, world: WorldIdentity, kind: u8) -> io::Result<Self> {
        Self::start_host(local, world, kind, None)
    }

    fn start_host(local: SocketAddr, world: WorldIdentity, kind: u8, signaling_host: Option<u64>) -> io::Result<Self> {
        Self::start(Mode::Host { local, offer: Offer { world, kind, token: token()?, addresses: Vec::new(), signaling_host } })
    }

    #[cfg(feature = "steamworks")]
    pub fn host_friends(local: SocketAddr, world: WorldIdentity, kind: u8) -> io::Result<Self> {
        let signal = crate::steam::invites::Host::start().map_err(|e| io::Error::other(e.to_string()))?;
        let mut bridge = Self::start_host(local, world, kind, Some(signal.id()))?;
        bridge.signal_host = Some(signal);
        Ok(bridge)
    }

    pub fn guest(offer: Offer) -> io::Result<Self> {
        let local = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
        let signaling_host = offer.signaling_host;
        #[cfg(not(feature = "steamworks"))]
        if signaling_host.is_some() { return Err(invalid("This invite needs the Steam build")); }
        #[allow(unused_mut)]
        let mut bridge = Self::start(Mode::Guest { local, offer, guest: token()? })?;
        #[cfg(feature = "steamworks")]
        if let Some(host) = signaling_host {
            bridge.signal_guest = Some(crate::steam::invites::Guest::start(host).map_err(|e| io::Error::other(e.to_string()))?);
        }
        Ok(bridge)
    }

    fn start(mode: Mode) -> io::Result<Self> {
        let host = match &mode { Mode::Host { offer, .. } => Some(offer.token), _ => None };
        let local = match &mode { Mode::Guest { local, .. } => Some(local.local_addr()?), _ => None };
        let automatic = match &mode { Mode::Host { offer, .. } | Mode::Guest { offer, .. } => offer.signaling_host.is_some() };
        let public = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let live = Arc::new(AtomicBool::new(true));
        let running = live.clone();
        let (commands, incoming) = mpsc::sync_channel(LIMIT);
        let (outgoing, events) = mpsc::channel();
        thread::Builder::new().name("direct-udp".into()).spawn(move || {
            if let Err(error) = run(mode, public, running, incoming, &outgoing) {
                let _ = outgoing.send(Event::Error(error));
            }
        })?;
        Ok(Self {
            host, commands, events, live, code: None, connected: false, local, automatic,
            #[cfg(feature = "steamworks")]
            signal_host: None,
            #[cfg(feature = "steamworks")]
            signal_guest: None,
        })
    }

    pub fn poll(&mut self) -> io::Result<()> {
        for event in self.events.try_iter() {
            match event {
                Event::Code(code) => self.code = Some(code),
                Event::Connected(address) => {
                    self.connected = true;
                    eprintln!("[network] punched direct UDP path to {address}");
                }
                Event::Error(error) => return Err(error),
            }
        }
        #[cfg(feature = "steamworks")]
        {
            if let Some(signal) = &self.signal_host {
                signal.receive(|reply| { let _ = self.accept(reply); });
            }
            if self.connected { self.signal_guest = None; }
            if let (Some(signal), Some(reply)) = (&mut self.signal_guest, &self.code) {
                signal.send(reply).map_err(|e| io::Error::other(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn code(&self) -> Option<&str> {
        if self.automatic && self.host.is_none() { return None; }
        self.code.as_deref()
    }

    pub fn waiting_for_path(&self) -> bool { self.host.is_none() && !self.connected }
    pub fn ready_address(&self) -> Option<SocketAddr> { self.local.filter(|_| self.connected) }

    pub fn accept(&self, code: &str) -> io::Result<()> {
        let answer = Answer::decode(code)?;
        if self.host != Some(answer.host) { return Err(invalid("Answer is for a different invite")); }
        self.commands.try_send(answer).map_err(|_| io::Error::other("Could not accept another answer"))
    }
}

impl Drop for Bridge {
    fn drop(&mut self) { self.live.store(false, Ordering::Relaxed); }
}

enum Mode {
    Host { local: SocketAddr, offer: Offer },
    Guest { local: UdpSocket, offer: Offer, guest: Token },
}

struct Route {
    socket: UdpSocket,
    game: Arc<Mutex<Option<SocketAddr>>>,
    key: Token,
    seen: Instant,
    active: Arc<AtomicBool>,
}

impl Route {
    fn start(public: &UdpSocket, socket: UdpSocket, peer: SocketAddrV4, game: Option<SocketAddr>,
        key: Token, live: &Arc<AtomicBool>) -> io::Result<Self>
    {
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let reader = socket.try_clone()?;
        let public = public.try_clone()?;
        let game = Arc::new(Mutex::new(game));
        let endpoint = game.clone();
        let active = Arc::new(AtomicBool::new(true));
        let route_live = active.clone();
        let live = live.clone();
        thread::Builder::new().name("direct-udp-peer".into()).spawn(move || {
            let mut packet = [0; 2048];
            while live.load(Ordering::Relaxed) && route_live.load(Ordering::Relaxed) {
                match reader.recv_from(&mut packet) {
                    Ok((len, from)) if from.ip().is_loopback() => {
                        let allowed = *endpoint.lock().unwrap().get_or_insert(from) == from;
                        if allowed {
                            let _ = public.send_to(&packet[..len], peer);
                        }
                    }
                    Ok(_) => {}
                    Err(error) if timeout(&error) => {}
                    Err(_) => break,
                }
            }
        })?;
        Ok(Self { socket, game, key, seen: Instant::now(), active })
    }
}

impl Drop for Route {
    fn drop(&mut self) { self.active.store(false, Ordering::Relaxed); }
}

fn timeout(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::ConnectionReset)
}

fn run(mut mode: Mode, public: UdpSocket, live: Arc<AtomicBool>, incoming: mpsc::Receiver<Answer>,
    events: &mpsc::Sender<Event>) -> io::Result<()>
{
    let candidates = stun::discover(&public)?;
    let automatic = match &mode { Mode::Host { offer, .. } | Mode::Guest { offer, .. } => offer.signaling_host.is_some() };
    let (host_key, host_game, mut guest) = match &mut mode {
        Mode::Host { local, offer } => {
            offer.addresses = candidates.addresses;
            let _ = events.send(Event::Code(offer.encode()));
            (offer.token, Some(*local), None)
        }
        Mode::Guest { local, offer, guest } => {
            let answer = Answer { host: offer.token, guest: *guest, addresses: candidates.addresses };
            let _ = events.send(Event::Code(answer.encode()));
            (offer.token, None, Some((local.try_clone()?, Answer { addresses: offer.addresses.clone(), ..answer })))
        }
    };
    public.set_read_timeout(Some(Duration::from_millis(25)))?;
    let started = Instant::now();
    let mut host_probe_at: Option<Instant> = None;
    let mut next = started;
    let mut next_stun = started + Duration::from_secs(15);
    let mut pending = Vec::<(Answer, Instant)>::new();
    let mut routes = BTreeMap::<SocketAddrV4, Route>::new();
    let mut packet = [0; 2048];
    while live.load(Ordering::Relaxed) {
        let now = Instant::now();
        for answer in incoming.try_iter() {
            if pending.len() + routes.len() < LIMIT
                && !pending.iter().any(|(p, _)| p.guest == answer.guest)
                && !routes.values().any(|route| route.key == answer.guest)
            {
                pending.push((answer, now));
            }
        }
        if guest.is_some() {
            let error = match host_probe_at {
                _ if automatic && now.duration_since(started) >= FRIENDS_TIMEOUT => Some("No direct UDP path to this invite."),
                Some(probe) if now.duration_since(probe) >= TIMEOUT => Some("No direct UDP path after the host responded."),
                None if now.duration_since(started) >= REPLY_TIMEOUT => Some("Invite reply expired without a response from the host."),
                _ => None,
            };
            if let Some(error) = error {
                return Err(io::Error::new(io::ErrorKind::TimedOut, error));
            }
        }
        if now >= next {
            next = now + RETRY;
            if host_game.is_some() {
                routes.retain(|_, route| now.saturating_duration_since(route.seen) < TIMEOUT);
            }
            pending.retain(|(_, accepted)| now.saturating_duration_since(*accepted) < TIMEOUT);
            for answer in pending.iter().map(|(answer, _)| answer).chain(guest.iter().map(|(_, answer)| answer)) {
                for address in &answer.addresses { let _ = public.send_to(&answer.packet(PROBE), address); }
            }
        }
        if now >= next_stun {
            if let Some(server) = candidates.server { stun::keepalive(&public, server); }
            next_stun = now + Duration::from_secs(15);
        }
        let (len, SocketAddr::V4(from)) = (match public.recv_from(&mut packet) {
            Ok(received) => received,
            Err(error) if timeout(&error) => continue,
            Err(error) => return Err(error),
        }) else { continue };
        if len == 41 && packet.starts_with(MAGIC) {
            if packet[8..24] != host_key { continue; }
            let key: Token = packet[24..40].try_into().unwrap();
            if let Some(game) = host_game {
                if packet[40] != PROBE { continue; }
                if routes.get(&from).is_some_and(|route| route.key == key) {
                    packet[40] = SELECT;
                    let _ = public.send_to(&packet[..41], from);
                } else if !routes.contains_key(&from) && !routes.values().any(|route| route.key == key) {
                    if let Some(index) = pending.iter().position(|(answer, _)| answer.guest == key) {
                        pending.swap_remove(index);
                    } else if !automatic || pending.len() + routes.len() >= LIMIT {
                        continue;
                    }
                    // A peer that can already reach this socket only needs the invite key.
                    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
                    let route = Route::start(&public, socket, from, Some(game), key, &live)?;
                    routes.insert(from, route);
                    packet[40] = SELECT;
                    let _ = public.send_to(&packet[..41], from);
                    let _ = events.send(Event::Connected(from));
                }
            } else if let Some((_, answer)) = &guest && key == answer.guest {
                if packet[40] == SELECT {
                    let (socket, _) = guest.take().unwrap();
                    routes.insert(from, Route::start(&public, socket, from, None, key, &live)?);
                    let _ = events.send(Event::Connected(from));
                } else if packet[40] == PROBE {
                    host_probe_at.get_or_insert(now);
                    let _ = public.send_to(&answer.packet(PROBE), from);
                }
            }
        } else if let Some(route) = routes.get_mut(&from) {
            route.seen = Instant::now();
            if let Some(game) = *route.game.lock().unwrap() { let _ = route.socket.send_to(&packet[..len], game); }
        }
    }
    Ok(())
}
