//! Sessions built at runtime around a world: hosting the open world, joining by code, closing.
//! Lobby handshakes run on a worker thread, so a slow or missing lobby never stalls a frame.
use super::*;
use engine_net::lobby::relay::RendezvousHost;
use std::{path::PathBuf, sync::mpsc};

/// Everything that differs between the ways a session starts.
pub(super) struct Parts {
    pub role: Role,
    pub profile: String,
    pub profiles: profiles::Profiles,
    pub creative: bool,
    pub headless: bool,
    pub save_path: Option<PathBuf>,
    pub sidecar: Option<PathBuf>,
    pub name: String,
    pub key: Option<engine_net::identity::PlayerKey>,
    pub code: Option<String>,
}

/// How the open world is offered to friends.
#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    /// Empty keeps the session unlisted.
    pub name: String,
    pub listed: bool,
    pub creative: bool,
}

pub fn lobby_address() -> Result<SocketAddr> {
    std::env::args()
        .find_map(|a| a.strip_prefix("--multiplayer-lobby=").map(str::to_owned))
        .as_deref()
        .unwrap_or(engine_net::lobby::DEFAULT_LOBBY)
        .parse()
        .context("multiplayer lobby must be IP:port")
}

pub(super) fn world_label(world: &World) -> String {
    let kind = match world.world_type {
        WorldType::Superflat => "Superflat",
        WorldType::Hills => "Hills",
        WorldType::Ridged => "Ridged",
        WorldType::Nether => "Nether",
            WorldType::Planet => "Planet",
        WorldType::Manyfold => "Manyfold",
        WorldType::Hall => "Hall",
        WorldType::Realm(kind) => kind.short(),
        WorldType::Realm(engine_voxel::RealmKind::Gilded) => "Gilded Cave",
    };
    format!("{kind} {} m", world.voxel_scale.meters())
}

/// The lobby listing for a world. Unlisted sessions still need a name on the wire.
pub(super) fn setup_for(world: &World, options: &OpenOptions, host_name: &str) -> HostSetup {
    let name = if options.listed && !options.name.trim().is_empty() { options.name.trim().to_owned() } else { host_name.to_owned() };
    HostSetup {
        name: name.chars().take(engine_net::lobby::wire::MAX_NAME).collect(),
        world: world_label(world),
        motd: "A shared world".to_owned(),
        mode: if options.creative { engine_net::lobby::wire::Mode::Creative } else { engine_net::lobby::wire::Mode::Survival },
        max_players: 8,
        unlisted: !options.listed,
        world_params: engine_net::lobby::wire::WorldParams {
            seed: world.seed,
            kind: kind_code(world.world_type),
            scale: scale_code(world.voxel_scale),
        },
    }
}

impl Session {
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    pub fn host_direct(world:&mut World,text:&str)->Result<Self> {
        let target=direct_address(text)?;
        let sidecar=profiles::path_for(&world.regions.view.root);
        let (profiles,note)=profiles::load_or_set_aside(&sidecar);
        if let Some(note)=note {eprintln!("[multiplayer] {note}");}
        let name=gameplay::player_name();
        let setup=setup_for(world,&OpenOptions {creative:world.rules.mode.creative_tools(),..Default::default()},&name);
        let advertised=match &target {
            #[cfg(feature="multiplayer-sdr")]
            Target::SteamSdr(_)=>Some(format!("steam:{}",engine_net::steam::local_id().map_err(|e|anyhow::anyhow!(e.to_string()))?)),
            Target::PunchHost { .. } => None,
            Target::Direct(address)=>{
            let address=*address;
            let advertised=if address.ip().is_unspecified() {
            let route=std::net::UdpSocket::bind((address.ip(),0));
            let ip=route.ok().and_then(|socket| {
                let remote:SocketAddr=if address.is_ipv4(){"1.1.1.1:53"}else{"[2606:4700:4700::1111]:53"}.parse().ok()?;
                socket.connect(remote).ok()?; Some(socket.local_addr().ok()?.ip())
            }).unwrap_or_else(|| if address.is_ipv4(){std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)}else{std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)});
            SocketAddr::new(ip,address.port())
        } else {address};
            Some(advertised.to_string())
            }
            _=>anyhow::bail!("invalid hosting transport"),
        };
        let role=Role::Pending {host:true,target,setup,identity:identity(world)?};
        let parts=Parts {role,profile:String::new(),profiles,creative:world.rules.mode.creative_tools(),headless:false,
            save_path:None,sidecar:Some(sidecar),name,key:None,code:advertised};
        let session=Self::assemble(parts,world);
        world.network_host=true;world.network_changes.clear();
        Ok(session)
    }
    pub(super) fn assemble(parts: Parts, world: &World) -> Self {
        let mut social = social::Social::default();
        social.code = parts.code;
        Self {
            realms: Default::default(), crossings: Default::default(), realm_loads: Default::default(), realm_joins: Default::default(), crossing_requested: false, incoming_realm: None, contacts: Vec::new(), authority: realm::Authority::new(world), realm_turn: 0, portal_warmup: None, mining_feedback: Default::default(),
            profile: parts.profile, profiles: parts.profiles, join_profiles: Default::default(), joined_at: Default::default(),
            join_sent: false, commands: Default::default(), command_sequence: 0, command_upload: None, unack: Default::default(),
            pending_input: Input::default(), hand: 0, tools: actions::Tools::default(), tools_key: String::new(),
            creative: parts.creative, headless: parts.headless, role: parts.role, remotes: BTreeMap::new(),
            accumulator: 0.0, network_clock: 0.0, pending_mine: false, pending_place: false,
            failed: false, reset_render: false, save_path: parts.save_path, sidecar: parts.sidecar, profiles_dirty: false,
            saved_revision: world.revision(), saved_profiles: 0, last_save: Instant::now(), social, name: parts.name, key: parts.key,
            introduced: false, early: Vec::new(), typed: Vec::new(), budgets: BTreeMap::new(), roster_due: Instant::now(), console: Vec::new(),
            stopped: false, correction_ack: 0, pending_damage: 0.0, hold: false, board: BoardWriter::default(), epoch: Instant::now(), review: Default::default(), replica_edits: replica::Queue::default(),
        }
    }

    /// Installs the server files and announces the code, before anyone is admitted.
    pub(super) fn install_host(&mut self, socket: &mut Host, world: &mut World) -> Result<()> {
        if world.travel.following.is_some() && world.travel.follow_owner.is_none() && !self.headless {
            world.travel.follow_owner = Some(HOST);
        }
        // The server files sit in the save directory, so a banned or unlisted name never reaches the roster.
        let loaded = server_files::load(&world::storage::save_directory());
        for warning in &loaded.warnings {
            eprintln!("[multiplayer] {warning}");
        }
        socket.set_access(loaded.access.clone(), loaded.settings.whitelist).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        socket.set_capacity(loaded.settings.max_players.saturating_sub(usize::from(!self.headless)));
        if world::storage::save_directory().join(server_files::SETTINGS_FILE).is_file() {
            world.rules.pvp = loaded.settings.pvp;
            world.rules.spawn_protection_m = loaded.settings.spawn_protection.min(u16::MAX as u32) as u16;
        }
        let label = if self.sidecar.is_some() { "Open to Friends" } else { "Hosting building session" };
        match socket.code() {
            Some(code) => {
                eprintln!("[multiplayer] listed code={code}");
                world.expedition.say(format!("{label} · Code {code}"));
                self.social.push_text(engine_net::protocol::ChatKind::System, format!("Session code {code}"));
                self.social.code = Some(code.to_owned());
            }
            None => {
                eprintln!("[multiplayer] listening");
                world.expedition.say(label);
            }
        }
        Ok(())
    }

    /// Hosts `world` in place; it keeps its own save and its friends' profiles sit beside it.
    pub fn host_world(world: &mut World, listing: RendezvousHost, options: &OpenOptions) -> Result<Self> {
        let sidecar = profiles::path_for(&world.regions.view.root);
        let (profiles, note) = profiles::load_or_set_aside(&sidecar);
        if let Some(note) = note {
            eprintln!("[multiplayer] {note}");
        }
        let parts = Parts {
            role: Role::Closed, profile: String::new(), profiles, creative: options.creative || world.rules.mode.creative_tools(), headless: false,
            save_path: None, sidecar: Some(sidecar), name: gameplay::player_name(), key: None, code: None,
        };
        let mut session = Self::assemble(parts, world);
        let setup = setup_for(world, options, &session.name);
        let mut socket = Host::listed(
            identity(world)?, &setup, listing,
        ).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        session.install_host(&mut socket, world)?;
        session.role = Role::Host { socket };
        world.network_host = true;
        world.network_changes.clear();
        Ok(session)
    }

    /// Swaps world simulations, so a hosted world keeps its live water and activity state.
    pub fn exchange_simulation(&mut self, other: &mut crate::simulation::WorldSimulation) {
        std::mem::swap(&mut self.authority.simulation, other);
    }

    /// Finishes the handshake before the renderer and horizon build stall the loop: the host
    /// kicks a joiner that shows no profile within seconds. Waits at most two seconds.
    pub(super) fn prime(&mut self) {
        let Role::Client { socket, .. } = &mut self.role else { return };
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match socket.poll(Instant::now()) {
                Ok(packets) => self.early.extend(packets),
                Err(_) => break,
            }
            if !self.introduced && let Some(key) = &self.key {
                self.introduced = socket.introduce(&self.name, key).unwrap_or(false);
            }
            if self.introduced && !self.join_sent && let Ok(bytes) = encode(10, &self.profile) {
                self.join_sent = socket.send_data(bytes).unwrap_or(false);
            }
            if self.join_sent {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        eprintln!("[multiplayer] handshake introduced={} profile_sent={}", self.introduced, self.join_sent);
    }

    pub fn mode(&self) -> coop::Mode {
        match &self.role {
            Role::Client { .. } | Role::Pending { host: false, .. } => coop::Mode::Joined,
            _ if self.sidecar.is_some() => coop::Mode::Opened,
            _ => coop::Mode::Hosted,
        }
    }

    pub fn code(&self) -> Option<&str> {
        self.social.code.as_deref()
    }

    pub fn accept_punch_answer(&self, code: &str) -> Result<()> {
        let Role::Host { socket } = &self.role else { anyhow::bail!("Open a world for direct code exchange first"); };
        socket.accept_punch_answer(code).map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    /// Other players in the session, loading ones included.
    pub fn others(&self) -> usize {
        match &self.role {
            Role::Host { .. } => self.hosted_players(),
            _ => self.social.roster().len().saturating_sub(1),
        }
    }

    pub(super) fn collected_profiles(&self) -> Result<profiles::Profiles> {
        let mut profiles = self.profiles.clone();
        let realm::Authority { actors, outputs, .. } = &self.authority;
        for (id, actor) in actors {
            if !actor.profile.is_empty() && (actor.restored || outputs.get(id).is_some_and(|o| o.ready)) {
                profiles.insert(actor.profile.clone(), gameplay::save_player(&actor.player, &actor.personal)?);
            }
        }
        self.collect_realm_profiles(&mut profiles)?;
        Ok(profiles)
    }

    /// Writes the friends' profiles beside the hosted world.
    pub fn flush_profiles(&mut self) -> Result<()> {
        self.write_profiles(false)
    }

    pub(super) fn write_profiles(&mut self, skip_unchanged: bool) -> Result<()> {
        let Some(path) = self.sidecar.as_deref() else { return Ok(()) };
        let profiles = self.collected_profiles()?;
        let previous = skip_unchanged.then_some(self.saved_profiles);
        self.saved_profiles = profiles::save_if_changed(path, &profiles, previous)?;
        self.profiles_dirty = false;
        if previous != Some(self.saved_profiles) {
            eprintln!("[multiplayer] profiles saved count={} path={}", profiles.len(), path.display());
        }
        Ok(())
    }

    /// Ends the session: joiners are disconnected, profiles written, the world is single-player again.
    pub fn close(&mut self, world: &mut World, player: &Player, day: &DayNightCycle) -> Result<()> {
        let saved = if self.sidecar.is_some() {
            self.flush_profiles()
        } else if self.save_path.is_some() && matches!(self.role, Role::Host { .. }) {
            self.save(world, player, day)
        } else {
            Ok(())
        };
        if let Role::Host { socket, .. } = &mut self.role {
            for id in socket.players() {
                socket.disconnect(id);
            }
            let _ = socket.advance();
            eprintln!("[multiplayer] closed");
        }
        let resident_save = self.flush_residents();
        self.role = Role::Closed;
        self.realms.clear();
        self.realm_loads.clear();
        self.realm_joins.clear();
        self.authority.actors.clear();
        self.authority.outputs.clear();
        self.remotes.clear();
        world.network_host = false;
        world.travel.riders.clear();
        if world.travel.follow_owner.is_some_and(|id| id != HOST) {
            world.travel.following = None;
        }
        world.travel.follow_owner = None;
        world.network_changes.clear();
        world.multiplayer = None;
        saved.and(resident_save)
    }
}

type Delivery<T> = mpsc::Receiver<std::result::Result<T, String>>;

enum JoinTransport {
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    Direct(Target,WorldIdentity,u8),
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
pub(super) fn described_world(description: WorldIdentity, kind: u8) -> Result<World> {
    let scale=match description.cell_mm {1000=>0,500=>1,250=>2,_=>anyhow::bail!("unsupported host voxel scale")};
    world_for_join(engine_net::lobby::wire::WorldParams {
        seed:u32::try_from(description.seed).context("host seed out of range")?,kind,scale,
    },description.generator)
}

fn worker<T: Send + 'static>(name: &str, job: impl FnOnce() -> std::result::Result<T, String> + Send + 'static) -> Result<Delivery<T>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new().name(name.into()).spawn(move || {
        let _ = tx.send(job());
    })?;
    Ok(rx)
}

fn take<T>(rx: &Delivery<T>) -> Option<std::result::Result<T, String>> {
    match rx.try_recv() {
        Ok(result) => Some(result),
        Err(mpsc::TryRecvError::Empty) => None,
        Err(mpsc::TryRecvError::Disconnected) => Some(Err("lobby worker stopped".into())),
    }
}

/// A lobby registration in flight for the open world.
pub struct HostOpening {
    rx: Delivery<RendezvousHost>,
    pub options: OpenOptions,
    started: Instant,
}

impl HostOpening {
    pub fn start(world: &World, options: OpenOptions) -> Result<Self> {
        let lobby = lobby_address()?;
        let setup = setup_for(world, &options, &gameplay::player_name());
        let generator = world.generator_version;
        let (seed, cell_mm) = (u64::from(world.seed), (world.voxel_scale.meters() * 1000.0).round() as u16);
        let rx = worker("lobby-host", move || {
            let content = executable_digest().map_err(|e| e.to_string())?;
            let identity = WorldIdentity { seed, generator, cell_mm, content };
            eprintln!("[multiplayer] opening seed={seed} generator={generator} cell_mm={cell_mm}");
            link::register_lobby(lobby, &identity, &setup).map_err(|e| e.to_string())
        })?;
        Ok(Self { rx, options, started: Instant::now() })
    }
    pub fn poll(&self) -> Option<std::result::Result<RendezvousHost, String>> {
        take(&self.rx).or_else(|| (self.started.elapsed() > Duration::from_secs(30))
            .then(|| Err("Connection timed out. Try Open to Friends again.".into())))
    }
}

/// A join by code in flight. The lobby answer is kept until the caller may swap worlds.
pub struct JoinAttempt {
    rx: Delivery<JoinTransport>,
    pub code: String,
    answer: Option<JoinTransport>,
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    punch: Option<Client>,
    prepared: bool,
    started: Instant,
}

impl JoinAttempt {
    pub fn status(&self) -> &'static str {
        if self.prepared { return "Saving World"; }
        if self.reply().is_some() { return "Waiting for Host"; }
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if engine_net::punch::is_offer(&self.code) {
            return if self.code.starts_with(engine_net::punch::OFFER_PREFIX) && self.punch.is_none() {
                "Preparing Reply"
            } else {
                "Connecting"
            };
        }
        if self.answer.is_some() { "Connecting" } else { "Contacting Session" }
    }

    pub fn prepared(&self) -> bool { self.prepared }

    pub fn reply(&self) -> Option<&str> {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        { return self.punch.as_ref().and_then(Client::punch_answer); }
        #[cfg(not(any(feature = "multiplayer-steam", feature = "multiplayer-sdr")))]
        None
    }

    pub fn start(code: &str) -> Result<Self> {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if engine_net::punch::is_offer(code.trim()) {
            let target = direct_invite(code.trim())?;
            let rx = worker("punch-world", move || {
                let (world, kind) = describe_target(&target).map_err(|error| error.to_string())?;
                Ok(JoinTransport::Direct(target, world, kind))
            })?;
            return Ok(Self { rx, code: code.trim().into(), answer: None, punch: None, prepared: false, started: Instant::now() });
        }
        #[cfg(feature="multiplayer-sdr")]
        if code.trim().starts_with("steam:") {
            let target=direct_address(code.trim())?;
            let rx=worker("steam-world-query",move|| {
                let (world,kind)=describe_target(&target).map_err(|error|error.to_string())?;
                Ok(JoinTransport::Direct(target,world,kind))
            })?;
            return Ok(Self {rx,code:code.trim().into(),answer:None,punch:None,prepared:false,started:Instant::now()});
        }
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        {
            let address=code.trim().parse::<SocketAddr>().ok().or_else(||engine_net::invite::decode(code).ok().map(Into::into));
            if let Some(address)=address {
                let rx=worker("native-world-query",move|| {
                    let digest=executable_digest().map_err(|error|error.to_string())?;
                    let (world,kind)=engine_net::steam::SteamClient::describe(address,digest).map_err(|error|error.to_string())?;
                    Ok(JoinTransport::Direct(Target::Direct(address),world,kind))
                })?;
                return Ok(Self {rx,code:address.to_string(),answer:None,punch:None,prepared:false,started:Instant::now()});
            }
        }
        #[cfg(not(any(feature = "multiplayer-steam", feature = "multiplayer-sdr")))]
        anyhow::bail!("Direct UDP needs a native multiplayer build");
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        {
        let lobby = lobby_address()?;
        let code: String = code.chars().filter(|c| !c.is_ascii_whitespace() && *c != '-')
            .map(|c| c.to_ascii_uppercase()).collect();
        ensure!(code.len() == 8 && code.bytes().all(|c| c.is_ascii_alphanumeric()), "Session codes contain 8 letters or numbers");
        let wire = code.clone();
        let rx = worker("lobby-join", move || {
            let digest = executable_digest().map_err(|e| e.to_string())?;
            let (target, offer) = link::resolve_lobby(lobby, &wire, digest)
                .map_err(|e| e.to_string())?;
            let world = link::offered_world(offer, digest).map_err(|e| e.to_string())?;
            Ok(JoinTransport::Direct(target, world, offer.world.kind))
        })?;
        Ok(Self { rx, code, answer: None, punch: None, prepared: false, started: Instant::now() })
        }
    }
    /// `swap`: the previous world is saved, so the joined one may replace it now.
    pub fn poll(&mut self, swap: bool) -> Option<Result<(Session, World, Player, DayNightCycle)>> {
        if self.answer.is_none() {
            let result = take(&self.rx).or_else(|| (self.started.elapsed() > Duration::from_secs(30))
                .then(|| Err("Connection timed out. Check the code and try again.".into())))?;
            match result {
                Ok(client) => self.answer = Some(client),
                Err(error) => return Some(Err(anyhow::anyhow!(error))),
            }
        }
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if let Some(JoinTransport::Direct(target @ Target::PunchJoin(_), world, _)) = &self.answer {
            let now = Instant::now();
            let ready = (|| -> link::LinkResult<bool> {
                if self.punch.is_none() {
                    self.punch = Some(Client::open(target, *world, now)?);
                }
                self.punch.as_mut().expect("invite connection prepared").prepare_punch(now)
            })();
            match ready {
                Ok(false) => return None,
                Err(error) => return Some(Err(anyhow::anyhow!(error.to_string()))),
                Ok(true) => {}
            }
        }
        self.prepared = true;
        if !swap {
            return None;
        }
        let client = self.answer.take()?;
        Some(match client {
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            JoinTransport::Direct(target,description,kind)=>(|| {
                let mut world=described_world(description,kind)?;
                world.adventure.begin_journey(); world.ensure_starter_wand(); world.sync_from_inventory();
                let setup=setup_for(&world,&OpenOptions::default(),&gameplay::player_name());
                let role = match self.punch.take() {
                    Some(socket) => Role::Client { socket, receiver: Receiver::default(), sequence: 0,
                        ready: false, started: Instant::now() },
                    None => Role::Pending { host: false, target, setup, identity: identity(&world)? },
                };
                let code = (!engine_net::punch::is_offer(&self.code)).then(|| self.code.clone());
                let parts=Parts {role,profile:gameplay::profile()?,profiles:Default::default(),creative:false,headless:false,
                    save_path:None,sidecar:None,name:gameplay::player_name(),key:Some(gameplay::player_key()?),code};
                let session=Session::assemble(parts,&world);
                Ok((session,world,Player::new(),DayNightCycle::default()))
            })(),
        })
    }
}
