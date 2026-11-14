//! The lobby discovers direct addresses. Native UDP carries the session.
use engine_net::{
    admin::AccessList,
    identity::PlayerKey,
    lobby::{
        relay::{Joined, RelayClient, RelayHost, RendezvousHost},
        wire::{Digest, HostInfo, Mode, WorldParams},
    },
    protocol::{Packet, PlayerId, WorldIdentity},
};
#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
use engine_net::steam::{SteamClient, SteamHost};
use std::{error::Error, net::SocketAddr, time::Instant};

pub type LinkResult<T> = Result<T, Box<dyn Error>>;

/// Where a session connects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Lobby that publishes the host's UDP address under a session code.
    Lobby { lobby: SocketAddr, code: String },
    /// Direct GameNetworkingSockets address.
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    Direct(SocketAddr),
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    PunchHost { automatic: bool },
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    PunchJoin(engine_net::punch::Offer),
    #[cfg(feature = "multiplayer-sdr")]
    SteamSdr(u64),
}

/// What a host advertises in the lobby.
#[derive(Clone, Debug)]
pub struct HostSetup {
    pub name: String,
    pub world: String,
    /// Server message shown in a listing.
    pub motd: String,
    pub mode: Mode,
    pub max_players: u8,
    pub unlisted: bool,
    pub world_params: WorldParams,
}

/// The lobby listing for `setup`; the version and digest come from the world identity.
pub fn host_info(world: &WorldIdentity, setup: &HostSetup) -> HostInfo {
    HostInfo {
        name: setup.name.clone(),
        world: setup.world.clone(),
        motd: setup.motd.clone(),
        mode: setup.mode,
        version: world.generator,
        digest: world.content,
        max_players: setup.max_players,
        unlisted: setup.unlisted,
        world_params: setup.world_params,
    }
}

pub enum Host {
    Relay(RelayHost),
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    Direct(DirectHost),
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
pub struct DirectHost {
    socket: SteamHost,
    listing: Option<RendezvousHost>,
    punch: Option<engine_net::punch::Bridge>,
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
impl std::ops::Deref for DirectHost {
    type Target = SteamHost;
    fn deref(&self) -> &Self::Target { &self.socket }
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
impl std::ops::DerefMut for DirectHost {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.socket }
}

pub fn listed_address() -> LinkResult<SocketAddr> {
    if !cfg!(any(feature = "multiplayer-steam", feature = "multiplayer-sdr")) {
        return Err("Direct UDP needs a native multiplayer build".into());
    }
    let port: u16 = std::env::args().find_map(|arg|
        arg.strip_prefix("--multiplayer-udp-port=").map(str::to_owned))
        .map_or(Ok(27020), |port| port.parse())?;
    if port == 0 { return Err("Direct UDP port must be nonzero".into()); }
    Ok(SocketAddr::new(std::net::Ipv6Addr::UNSPECIFIED.into(), port))
}

pub fn register_lobby(lobby: SocketAddr, world: &WorldIdentity, setup: &HostSetup) -> LinkResult<RendezvousHost> {
    let info = host_info(world, setup);
    Ok(RendezvousHost::start(lobby, info, listed_address()?.port())?)
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
pub fn resolve_lobby(lobby: SocketAddr, code: &str, digest: Digest) -> LinkResult<(Target, Joined)> {
    use engine_net::lobby::relay::InviteEndpoint;
    let (endpoint, joined) = engine_net::lobby::relay::resolve_invite(lobby, code, digest)?;
    let InviteEndpoint::Address(address) = endpoint else {
        return Err("This invite does not offer a direct UDP address".into());
    };
    eprintln!("[multiplayer] invite resolved; connecting directly to {address}");
    Ok((Target::Direct(address), joined))
}

pub fn offered_world(params: Joined, content: Digest) -> LinkResult<WorldIdentity> {
    let cell_mm = match params.world.scale {
        0 => 1000,
        1 => 500,
        2 => 250,
        _ => return Err("Unsupported host voxel scale".into()),
    };
    Ok(WorldIdentity { seed: params.world.seed.into(), generator: params.version, cell_mm, content })
}

impl Host {
    pub fn listed(world: WorldIdentity, setup: &HostSetup, listing: RendezvousHost) -> LinkResult<Self> {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        {
            let mut socket = SteamHost::bind(listed_address()?, world)?;
            socket.set_world_kind(setup.world_params.kind)?;
            Ok(Self::Direct(DirectHost { socket, listing: Some(listing), punch: None }))
        }
        #[cfg(not(any(feature = "multiplayer-steam", feature = "multiplayer-sdr")))]
        {
            let _ = (world, setup, listing);
            Err("Direct UDP needs a native multiplayer build".into())
        }
    }

    pub fn set_capacity(&mut self, players: usize) {
        match self {
            Self::Relay(host) => host.set_capacity(players),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.set_capacity(players),
        }
    }
    pub fn open(target: &Target, world: WorldIdentity, setup: &HostSetup) -> LinkResult<Self> {
        match target {
            #[cfg(feature = "multiplayer-sdr")]
            Target::SteamSdr(_) => {
                let mut host=SteamHost::bind_p2p(world)?;
                host.set_world_kind(setup.world_params.kind)?;
                Ok(Self::Direct(DirectHost { socket: host, listing: None, punch: None }))
            }
            Target::Lobby { lobby, .. } => {
                let listing = register_lobby(*lobby, &world, setup)?;
                Self::listed(world, setup, listing)
            }
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Target::Direct(address) => {
                let mut host=SteamHost::bind(*address,world)?;
                host.set_world_kind(setup.world_params.kind)?;
                Ok(Self::Direct(DirectHost { socket: host, listing: None, punch: None }))
            }
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Target::PunchHost { automatic } => {
                let reservation = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
                let address = reservation.local_addr()?;
                drop(reservation);
                let mut socket = SteamHost::bind(address, world)?;
                socket.set_world_kind(setup.world_params.kind)?;
                #[cfg(feature = "multiplayer-sdr")]
                let punch = if *automatic {
                    engine_net::punch::Bridge::host_friends(address, world, setup.world_params.kind)?
                } else {
                    engine_net::punch::Bridge::host(address, world, setup.world_params.kind)?
                };
                #[cfg(not(feature = "multiplayer-sdr"))]
                let punch = {
                    debug_assert!(!automatic);
                    engine_net::punch::Bridge::host(address, world, setup.world_params.kind)?
                };
                Ok(Self::Direct(DirectHost { socket, listing: None, punch: Some(punch) }))
            }
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Target::PunchJoin(_) => Err("Join an offer as a guest".into()),
        }
    }

    /// The lobby code for a relayed host; `None` for a direct host.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Relay(host) => Some(host.code()),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.listing.as_ref().map(RendezvousHost::code)
                .or_else(|| host.punch.as_ref().and_then(engine_net::punch::Bridge::code)),
        }
    }

    pub fn poll(&mut self, now: Instant) -> LinkResult<Vec<(PlayerId, Vec<u8>)>> {
        match self {
            Self::Relay(host) => Ok(host.poll(now)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => {
                if let Some(punch) = host.punch.as_mut() { punch.poll()?; }
                let packets = host.socket.poll(now)?;
                let players = host.socket.players().len() + 1;
                if let Some(listing) = host.listing.as_mut()
                    && let Err(error) = listing.presence(now, players)
                {
                    eprintln!("[multiplayer] invite listing ended: {error}");
                    host.listing = None;
                }
                Ok(packets)
            }
        }
    }

    pub fn accept_punch_answer(&self, code: &str) -> LinkResult<()> {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if let Self::Direct(host) = self && let Some(punch) = host.punch.as_ref() {
            return Ok(punch.accept(code)?);
        }
        let _ = code;
        Err("This world is not hosting a direct code exchange".into())
    }

    pub fn advance(&mut self) -> LinkResult<Packet> {
        match self {
            Self::Relay(host) => Ok(host.advance()?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.advance(),
        }
    }

    pub fn send_data(&self, player: PlayerId, bytes: Vec<u8>) -> LinkResult<bool> {
        match self {
            Self::Relay(host) => Ok(host.send_data(player, bytes)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.send_data(player, bytes),
        }
    }

    pub fn send_bulk(&self, player: PlayerId, bytes: Vec<u8>) -> LinkResult<bool> {
        match self {
            Self::Relay(host) => Ok(host.send_data(player, bytes)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.send_bulk(player, bytes),
        }
    }

    pub fn send_pose(&self, player: PlayerId, bytes: Vec<u8>) -> LinkResult<()> {
        match self {
            Self::Relay(host) => { let _ = host.send_data(player, bytes)?; Ok(()) }
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.send_pose(player, bytes),
        }
    }

    pub fn disconnect(&mut self, player: PlayerId) {
        match self {
            Self::Relay(host) => host.disconnect(player),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.disconnect(player),
        }
    }

    /// Free slots in the shared outgoing link; `None` when the transport paces itself.
    pub fn link_room(&self) -> Option<usize> {
        match self {
            Self::Relay(host) => Some(engine_net::lobby::relay::QUEUE.saturating_sub(host.backlog())),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(_) => None,
        }
    }

    pub fn players(&self) -> Vec<PlayerId> {
        match self {
            Self::Relay(host) => host.players(),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.players(),
        }
    }

    /// Names of the joined players, for the roster and command lookup.
    pub fn roster(&self) -> Vec<(PlayerId, String)> {
        match self {
            Self::Relay(host) => host.roster(),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.roster(),
        }
    }

    /// Command lines typed by players since the last call.
    pub fn take_commands(&mut self) -> Vec<(PlayerId, String)> {
        match self {
            Self::Relay(host) => host.take_commands(),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.take_commands(),
        }
    }

    /// A snapshot of the access lists, for saving after a change.
    pub fn access(&self) -> Option<AccessList> {
        match self {
            Self::Relay(host) => host.access().cloned(),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.access().cloned(),
        }
    }

    /// Live lists, for applying an admin command.
    pub fn access_mut(&mut self) -> Option<&mut AccessList> {
        match self {
            Self::Relay(host) => host.access_mut(),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.access_mut(),
        }
    }

    /// Installs the access lists; a refused name never joins the roster.
    pub fn set_access(&mut self, access: AccessList, whitelist: bool) -> LinkResult<()> {
        match self {
            Self::Relay(host) => {
                host.set_access(access, whitelist);
                Ok(())
            }
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => {
                host.set_access(access, whitelist);
                Ok(())
            }
        }
    }

    /// Sends a protocol packet such as `Chat` as itself, not wrapped as data.
    pub fn send_packet(&self, player: PlayerId, packet: &Packet) -> LinkResult<bool> {
        match self {
            Self::Relay(host) => Ok(host.send_packet(player, packet)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.send_packet(player, packet),
        }
    }

    /// Both game transports install access lists and require signed introductions.
    pub fn has_keys(&self) -> bool {
        true
    }

    /// The key fingerprint `player` proved; `None` on a transport without keys.
    pub fn fingerprint(&self, player: PlayerId) -> Option<String> {
        match self {
            Self::Relay(host) => host.fingerprint(player).map(str::to_owned),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.fingerprint(player).map(str::to_owned),
        }
    }

    /// Whether `player` holds an operator entry bound to its proven key.
    pub fn holds_op(&self, player: PlayerId) -> bool {
        match self {
            Self::Relay(host) => host.holds_op(player),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.holds_op(player),
        }
    }

    /// Applies `/name` under the session's admission checks; returns the name given.
    pub fn rename(&mut self, player: PlayerId, wanted: &str) -> LinkResult<String> {
        match self {
            Self::Relay(host) => Ok(host.rename(player, wanted)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(host) => host.rename(player, wanted),
        }
    }
}

pub enum Client {
    Relay(RelayClient),
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    Direct(DirectClient),
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
pub struct DirectClient {
    socket: Option<SteamClient>,
    punch: Option<(engine_net::punch::Bridge, WorldIdentity)>,
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
impl DirectClient {
    fn connected(socket: SteamClient) -> Self {
        Self { socket: Some(socket), punch: None }
    }

    fn prepare_punch(&mut self, now: Instant) -> LinkResult<bool> {
        if let Some((punch, world)) = self.punch.as_mut() {
            punch.poll()?;
            if self.socket.is_none() && let Some(address) = punch.ready_address() {
                self.socket = Some(SteamClient::connect(address, *world, now)?);
            }
        }
        Ok(self.socket.is_some())
    }

    fn poll(&mut self, now: Instant) -> LinkResult<Vec<Packet>> {
        self.prepare_punch(now)?;
        self.socket.as_mut().map_or(Ok(Vec::new()), |socket| socket.poll(now))
    }
}

impl Client {
    /// Direct sessions already know the world, so they can say hello at once.
    pub fn open(target: &Target, identity: WorldIdentity, now: Instant) -> LinkResult<Self> {
        match target {
            #[cfg(feature = "multiplayer-sdr")]
            Target::SteamSdr(id) => Ok(Self::Direct(DirectClient::connected(SteamClient::connect_p2p(*id,identity,now)?))),
            Target::Lobby { .. } => {
                let _ = (identity, now);
                Err("lobby sessions join with Client::join".into())
            }
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Target::Direct(address) => Ok(Self::Direct(DirectClient::connected(SteamClient::connect(*address, identity, now)?))),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Target::PunchJoin(offer) => Ok(Self::Direct(DirectClient {
                socket: None, punch: Some((engine_net::punch::Bridge::guest(offer.clone())?, identity)),
            })),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Target::PunchHost { .. } => Err("Join with the host's offer code".into()),
        }
    }

    /// Joins a lobby session by code. The host's world parameters come back before any world
    /// exists, so the joiner builds that world and then calls `hello`. `digest` is this build's
    /// fingerprint; the lobby refuses a different build.
    pub fn join(lobby: SocketAddr, code: &str, digest: Digest, now: Instant) -> LinkResult<(Self, Joined)> {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        {
            let (target, joined) = resolve_lobby(lobby, code, digest)?;
            let identity = offered_world(joined, digest)?;
            Ok((Self::open(&target, identity, now)?, joined))
        }
        #[cfg(not(any(feature = "multiplayer-steam", feature = "multiplayer-sdr")))]
        {
            let _ = (lobby, code, digest, now);
            Err("Direct UDP needs a native multiplayer build".into())
        }
    }

    /// Sends the joiner's identity. Direct sessions sent it when they opened.
    pub fn hello(&mut self, identity: WorldIdentity) -> LinkResult<()> {
        match self {
            Self::Relay(client) => Ok(client.hello(identity)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(_) => {
                let _ = identity;
                Ok(())
            }
        }
    }

    pub fn poll(&mut self, now: Instant) -> LinkResult<Vec<Packet>> {
        match self {
            Self::Relay(client) => Ok(client.poll(now)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(client) => client.poll(now),
        }
    }

    pub fn punch_answer(&self) -> Option<&str> {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if let Self::Direct(client) = self {
            if client.socket.is_none() {
                return client.punch.as_ref().and_then(|(punch, _)| punch.code());
            }
        }
        None
    }

    pub fn waiting_for_punch(&self) -> bool {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if let Self::Direct(client) = self {
            return client.punch.as_ref().is_some_and(|(bridge, _)| bridge.waiting_for_path());
        }
        false
    }

    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    pub(super) fn prepare_punch(&mut self, now: Instant) -> LinkResult<bool> {
        match self {
            Self::Direct(client) => client.prepare_punch(now),
            Self::Relay(_) => Err("Invite requires a direct connection".into()),
        }
    }

    pub fn player(&self) -> Option<PlayerId> {
        match self {
            Self::Relay(client) => client.player(),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(client) => client.socket.as_ref().and_then(SteamClient::player),
        }
    }

    pub fn submit(&mut self, movement: [i16; 2], look: [i16; 2], buttons: u16) -> LinkResult<()> {
        match self {
            Self::Relay(client) => Ok(client.submit(movement, look, buttons)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(client) => client.socket.as_mut().map_or(Ok(()), |socket| socket.submit(movement, look, buttons)),
        }
    }

    /// Names this player and proves its key, once, after the welcome.
    /// `false` until the welcome has arrived.
    pub fn introduce(&mut self, name: &str, key: &PlayerKey) -> LinkResult<bool> {
        match self {
            Self::Relay(client) if client.can_introduce() => {
                client.introduce(name, key)?;
                Ok(true)
            }
            Self::Relay(_) => Ok(false),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(client) => {
                let Some(socket) = client.socket.as_mut().filter(|socket| socket.can_introduce()) else {
                    return Ok(false);
                };
                socket.introduce(name, key)?;
                Ok(true)
            }
        }
    }

    pub fn send_data(&self, bytes: Vec<u8>) -> LinkResult<bool> {
        match self {
            Self::Relay(client) => Ok(client.send_data(bytes)?),
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Self::Direct(client) => client.socket.as_ref().map_or(Ok(false), |socket| socket.send_data(bytes)),
        }
    }
}
