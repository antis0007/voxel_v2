//! World-specific authority, separate from the host transport.
use super::*;

pub(super) const CROSS_GATE: u8 = 31;

const REALM_OPEN_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) struct Baseline {
    pub(super) sender: Sender,
    pub(super) deferred: std::collections::BTreeSet<[i32; 3]>,
}

#[derive(Default)]
pub(super) struct Terrain {
    pub(super) deferred: std::collections::BTreeSet<[i32; 3]>,
    center: Option<IVec3>,
    schedule: Vec<[i32; 3]>,
    sending: Option<[i32; 3]>,
    held: BTreeMap<[i32; 3], u32>,
    awaiting_ack: bool,
    worker: Option<std::sync::mpsc::Receiver<Result<Chunk>>>,
    prepared: Option<Chunk>,
}

impl Terrain {
    pub(super) fn new(deferred: std::collections::BTreeSet<[i32; 3]>) -> Self {
        Self { deferred, ..Self::default() }
    }

    pub(super) fn acknowledge(&mut self) -> Result<()> {
        ensure!(self.awaiting_ack, "Unexpected terrain acknowledgement");
        self.awaiting_ack = false;
        Ok(())
    }

    pub(super) fn defer_edit(&mut self, p: [i32; 3], block: u32) -> bool {
        let chunk = world::chunk_coord_from_world_block(IVec3::from_array(p)).to_array();
        if self.sending == Some(chunk) {
            self.held.insert(p, block);
            true
        } else {
            self.deferred.contains(&chunk)
        }
    }

    pub(super) fn take(&mut self, cells: &mut BTreeMap<[i32; 3], u32>) -> Option<Chunk> {
        let chunk = self.prepared.take()?;
        self.sending = None;
        self.awaiting_ack = true;
        cells.append(&mut self.held);
        Some(chunk)
    }

    pub(super) fn poll(&mut self) -> Result<()> {
        if let Some(worker) = &self.worker {
            match worker.try_recv() {
                Ok(result) => {
                    self.worker = None;
                    self.prepared = Some(result?);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => anyhow::bail!("Terrain transfer worker stopped"),
            }
        }
        Ok(())
    }

    pub(super) fn start(&mut self, world: &mut World, pos: Vec3, cells: &mut BTreeMap<[i32; 3], u32>) -> Result<()> {
        if self.sending.is_some() || self.awaiting_ack || self.deferred.is_empty() { return Ok(()); }
        let center = world::chunk_coord_from_world_block(pos.floor().as_ivec3());
        if self.center != Some(center) {
            self.center = Some(center);
            self.schedule.clear();
            self.schedule.extend(self.deferred.iter().copied());
            self.schedule.sort_unstable_by_key(|&coord| std::cmp::Reverse(
                (IVec3::from_array(coord) - center).abs().max_element()
            ));
        }
        let Some(coord) = self.schedule.pop() else { return Ok(()); };
        let snapshot = world.snapshot();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new().name("realm-terrain".into()).spawn(move || {
            let result = snapshot_chunk(&snapshot, IVec3::from_array(coord));
            let _ = tx.send(result);
        })?;
        cells.retain(|p, _| world::chunk_coord_from_world_block(IVec3::from_array(*p)).to_array() != coord);
        self.deferred.remove(&coord);
        self.sending = Some(coord);
        self.worker = Some(rx);
        Ok(())
    }
}

pub(super) fn validate_gates(gates: &[world::hell_gate::built::Threshold], scale: VoxelScalePreset) -> Result<()> {
    ensure!(gates.len() <= world::hell_gate::built::MAX_GATES, "Too many Hell portals");
    for gate in gates {
        gate.validate(f64::from(scale.meters()))?;
    }
    Ok(())
}

pub(super) type PreparedWorld = (
    World, Player, DayNightCycle, std::collections::BTreeSet<[i32; 3]>,
    crate::render::PortalWarmup,
);

pub(super) enum Incoming {
    Building(std::sync::mpsc::Receiver<Result<PreparedWorld>>),
    Ready(Box<PreparedWorld>),
}

pub(super) fn prepare_client(bytes: Vec<u8>, current: (u8, u8, u32, u32)) -> Result<Incoming> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new().name("realm-replica".into()).spawn(move || {
        let prepared = (|| {
            let mut snapshot: Snapshot = gameplay::unpack(&bytes)?;
            ensure!(snapshot.deferred_terrain.len() <= 262_144
                && snapshot.deferred_terrain.iter().flatten().all(|v| v.unsigned_abs() <= 31_249),
                "Invalid deferred terrain index");
            let deferred = std::mem::take(&mut snapshot.deferred_terrain).into_iter().collect();
            let (kind, scale, seed, generator) = snapshot.realm.unwrap_or(current);
            let mut world = world_for_join(engine_net::lobby::wire::WorldParams { kind, scale, seed }, generator)?;
            let mut player = Player::new();
            let mut day = DayNightCycle::default();
            apply_snapshot(&mut world, &mut player, &mut day, snapshot, &mut HashSet::new(), &mut HashSet::new())?;
            let center = world::chunk_coord_from_world_block(player.pos.floor().as_ivec3());
            let floor = world::chunk_coord_from_world_block((player.pos - Vec3::Y * 0.1).floor().as_ivec3());
            let warmup = crate::render::PortalWarmup::build(&mut world, center, floor);
            Ok((world, player, day, deferred, warmup))
        })();
        let _ = tx.send(prepared);
    })?;
    Ok(Incoming::Building(rx))
}

pub(super) struct Authority {
    save: String,
    pub(super) actors: BTreeMap<PlayerId, Actor>,
    pub(super) outputs: BTreeMap<PlayerId, Output>,
    pub(super) simulation: crate::simulation::WorldSimulation,
    pub(super) accumulator: f32,
    pub(super) creature_tick: u64,
    pub(super) factory_image: replication::FactoryImage,
    pub(super) history: world::edit_history::EditHistory,
    pub(super) footsteps: BTreeMap<PlayerId, (Vec3, f32)>,
    pub(super) local_cooldown: f32,
    pub(super) local_mining: world::mining::Progress,
    pub(super) local_spell_edits: crate::creative::CreativeState,
    pub(super) local_edits: authority::Pending,
    pub(super) sent_rules: Vec<u8>,
}

impl Authority {
    pub(super) fn new(world: &World) -> Self {
        let mut simulation = crate::simulation::WorldSimulation::new();
        simulation.reset_for_world(world.voxel_scale);
        Self {
            save: world.regions.view.session_name().to_owned(),
            actors: BTreeMap::new(),
            outputs: BTreeMap::new(),
            simulation,
            accumulator: 0.0,
            creature_tick: 0,
            factory_image: Default::default(),
            history: world::edit_history::EditHistory::new(world::edit_history::LOCAL_CELLS),
            footsteps: BTreeMap::new(),
            local_cooldown: 0.0,
            local_mining: Default::default(),
            local_spell_edits: Default::default(),
            local_edits: Default::default(),
            sent_rules: Vec::new(),
        }
    }

    pub(super) fn validate_world(&self, world: &World) -> Result<()> {
        ensure!(self.save == world.regions.view.session_name(), "Hosted realm does not match the active save");
        Ok(())
    }
}

pub(crate) struct Plan {
    authority: Authority,
    peers: BTreeMap<PlayerId, (gameplay::SavedPlayer, Baseline)>,
    profiles: profiles::Profiles,
    sidecar: std::path::PathBuf,
}

impl Session {
    pub(super) fn hosted_players(&self) -> usize {
        self.authority.actors.len() + self.realms.values()
            .map(|resident| resident.session.authority.actors.len()).sum::<usize>()
    }

    pub(crate) fn can_cross_realm(&self) -> bool {
        !self.failed && self.sidecar.is_some() && matches!(self.role, Role::Host { .. })
    }

    pub(crate) fn begin_realm(&mut self, world: &World, host: &Player) -> Result<()> {
        ensure!(self.can_cross_realm(), "Realm travel requires a hosted save");
        ensure!(!self.profiles.values().any(|entry| matches!(profiles::realm_entry(entry), Ok(Some(_)))),
            "Friends must return to this realm before party travel");
        ensure!(self.realm_joins.is_empty() && self.realm_loads.is_empty()
            && self.realms.values().all(|resident| resident.session.authority.actors.is_empty() && resident.session.join_profiles.is_empty()),
            "Party travel is unavailable while other realms are occupied or opening");
        self.authority.validate_world(world)?;
        let Authority { actors, outputs, .. } = &self.authority;
        ensure!(outputs.values().all(|o| o.ready) && self.join_profiles.is_empty(), "Wait for friends to finish joining");
        let scale = world.voxel_scale.meters();
        for actor in actors.values() {
            let delta = (actor.player.pos - host.pos) * scale;
            ensure!(delta.length_squared() <= 64.0 && delta.y.abs() <= 3.0
                && actor.player.anchor.is_none() && actor.personal.adventure.health > 0.0,
                "Gather all friends at the gate");
        }
        self.flush_profiles()?;
        if !self.realms.is_empty() {
            self.flush_residents()?;
            self.realms.clear();
        }
        Ok(())
    }

    pub(crate) fn prepare_realm(&self, world: &mut World, host: &Player, arrival: Vec3, day: &DayNightCycle) -> Result<Plan> {
        ensure!(self.can_cross_realm(), "Only the host can cross");
        let actors = &self.authority.actors;
        let sidecar = profiles::path_for(&world.regions.view.root);
        let mut profiles = profiles::load(&sidecar)?;
        let mut peers = BTreeMap::new();
        let mut occupied = vec![world::player_aabb(arrival, world.voxel_scale, host.scale, host.crouch_t)];
        for (&id, actor) in actors {
            let mut staged = gameplay::load_player(&gameplay::save_player(&actor.player, &actor.personal)?)?;
            let (pos, bounds) = world::spawn::portal_landing(world, arrival, &staged.player, &occupied).context("No safe landing for the whole party")?;
            occupied.push(bounds);
            if let Some(entry) = profiles.get(&actor.profile) {
                ensure!(profiles::realm_entry(entry)?.is_none(), "A friend has a saved guest in another realm of this destination");
            }
            let transfer = prepare_guest(&mut staged, pos, &self.authority.save, world, day, id)?;
            peers.insert(id, (staged, transfer));
            profiles.remove(&actor.profile);
        }
        ensure!(profiles.len() + peers.len() <= profiles::MAX_PROFILES, "Destination player registry is full");
        Ok(Plan { authority: Authority::new(world), peers, profiles, sidecar })
    }

    pub(crate) fn commit_realm(&mut self, mut plan: Plan) {
        let now = Instant::now();
        for (id, (staged, transfer)) in plan.peers {
            let Some(actor) = self.authority.actors.remove(&id) else { continue };
            plan.authority.admit(id, actor, staged, transfer, now);
        }
        self.authority = plan.authority;
        self.profiles = plan.profiles;
        self.sidecar = Some(plan.sidecar);
        self.saved_profiles = 0;
        self.profiles_dirty = true;
        self.commands.clear();
        self.contacts.clear();
        self.remotes.clear();
        self.pending_input = Input::default();
        self.pending_mine = false;
        self.pending_place = false;
        self.accumulator = 0.0;
    }
}

/// Another live world under the same transport. It has no local host body.
pub(super) struct Resident {
    session: Session,
    saved: world::storage::SavedSession,
    saves: world::storage::SaveService,
    saving_gate: bool,
    legacy_profiles: profiles::Profiles,
}

impl Resident {
    fn open(mut saved: world::storage::SavedSession, name: String) -> Result<Self> {
        let sidecar = profiles::path_for(&saved.world.regions.view.root);
        let legacy_profiles = profiles::load(&sidecar)?;
        saved.world.network_host = true;
        let session = Session::assemble(open::Parts {
            role: Role::Closed, profile: String::new(), profiles: Default::default(), creative: false, headless: true,
            save_path: None, sidecar: Some(sidecar), name, key: None, code: None,
        }, &saved.world);
        Ok(Self { session, saved, saves: world::storage::SaveService::resident(), saving_gate: false, legacy_profiles })
    }

    fn checkpoint(&mut self) -> Result<()> {
        self.saves.request(true);
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.saves.busy() {
            self.saves.tick(Some(&mut self.saved.world), &self.saved.player, &self.saved.day_night);
            ensure!(Instant::now() < deadline, "Realm checkpoint timed out");
            if self.saves.busy() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        ensure!(!self.saves.failed, "Realm checkpoint failed");
        Ok(())
    }
}

type Loading = std::sync::mpsc::Receiver<Result<world::storage::SavedSession>>;
pub(super) type RealmLoads = BTreeMap<String, Loading>;

pub(super) struct PendingJoin {
    profile: String,
    realm: String,
    since: Instant,
}

impl Session {
    pub(super) fn collect_realm_profiles(&self, profiles: &mut profiles::Profiles) -> Result<()> {
        for (profile, entry) in profiles {
            let Some((realm, _)) = profiles::realm_entry(entry)? else { continue; };
            let Some(resident) = self.realms.get(realm) else { continue; };
            let session = &resident.session;
            if let Some(actor) = session.authority.actors.values().find(|actor| &actor.profile == profile) {
                *entry = profiles::in_realm(realm, &gameplay::save_player(&actor.player, &actor.personal)?)?;
            } else if let Some(player) = session.profiles.get(profile) {
                *entry = profiles::in_realm(realm, player)?;
            }
        }
        Ok(())
    }

    fn ensure_resident(&mut self, key: &str, arrival: Option<Vec3>) -> Result<bool> {
        if self.realms.contains_key(key) { return Ok(true); }
        if let Some(load) = self.realm_loads.get(key) {
            use std::sync::mpsc::TryRecvError;
            let result = match load.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => return Ok(false),
                Err(TryRecvError::Disconnected) => Err(anyhow::anyhow!("Realm loading stopped")),
            };
            self.realm_loads.remove(key);
            let saved = result?;
            self.realms.insert(key.to_owned(), Box::new(Resident::open(saved, self.name.clone())?));
            return Ok(true);
        }
        ensure!(self.realms.len() + self.realm_loads.len() < 4, "Too many open realms");
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let name = key.to_owned();
        std::thread::Builder::new().name("realm-load".into()).spawn(move || {
            let loaded = (|| -> Result<_> {
                let mut saved = world::storage::load_named(&name)?.context("Linked realm checkpoint missing")?;
                let world = &mut saved.world;
                ensure!(world.regions.view.session_name() == name, "Saved realm identity mismatch");
                let feet = arrival.unwrap_or(saved.player.pos);
                let center = world::chunk_coord_from_world_block(feet.floor().as_ivec3());
                let floor = world::chunk_coord_from_world_block((feet - Vec3::Y).floor().as_ivec3());
                // Gate validation samples the same edited chunks many times. Read
                // them once, along with the approach, before routing any guests.
                let nearby: Vec<_> = world.regions.view.records.keys().copied()
                    .filter(|&chunk| (chunk - center).abs().max_element() <= 1).collect();
                for chunk in [center, floor].into_iter().chain(nearby) {
                    ensure!(world.admit_edit_chunk(chunk), "Realm arrival terrain unavailable");
                }
                Ok(saved)
            })();
            let _ = tx.send(loaded);
        })?;
        self.realm_loads.insert(key.to_owned(), rx);
        Ok(false)
    }

    fn checkpoint_guest(&mut self, profile: &str, entry: &[u8]) -> Result<profiles::Profiles> {
        let mut saved = self.collected_profiles()?;
        saved.insert(profile.to_owned(), entry.to_vec());
        let path = self.sidecar.as_deref().context("Hosted save missing")?;
        self.saved_profiles = profiles::save_if_changed(path, &saved, None)?;
        Ok(saved)
    }

    fn queue_realm_join(&mut self, id: PlayerId, profile: String, now: Instant) -> Result<bool> {
        let realm = if let Some(entry) = self.profiles.get(&profile) {
            let Some((realm, _)) = profiles::realm_entry(entry)? else { return Ok(false); };
            realm.to_owned()
        } else {
            // Adopt old per-realm profiles when the old routing can still find them.
            let legacy = self.realms.iter().find_map(|(key, resident)| {
                let player = resident.legacy_profiles.get(&profile)?;
                matches!(profiles::realm_entry(player), Ok(None)).then_some((key, player))
            });
            let Some((key, player)) = legacy else { return Ok(false); };
            let realm = key.clone();
            let entry = profiles::in_realm(&realm, player)?;
            self.checkpoint_guest(&profile, &entry)?;
            self.profiles.insert(profile.clone(), entry);
            realm
        };
        self.realm_joins.insert(id, PendingJoin { profile, realm, since: now });
        Ok(true)
    }

    fn resume_realm(&mut self, id: PlayerId, join: &PendingJoin, now: Instant) -> Result<bool> {
        ensure!(now.duration_since(join.since) < REALM_OPEN_TIMEOUT, "Saved realm loading timed out");
        ensure!(join.realm != self.authority.save, "Saved guest realm matches the host realm");
        if !self.ensure_resident(&join.realm, None)? { return Ok(false); }
        let resident = &mut self.realms.get_mut(&join.realm).expect("realm is resident").session;
        let entry = self.profiles.get(&join.profile).context("Saved guest profile missing")?;
        let (realm, player) = profiles::realm_entry(entry)?.context("Saved guest realm missing")?;
        ensure!(realm == join.realm, "Saved guest realm changed while joining");
        if let std::collections::btree_map::Entry::Vacant(slot) = resident.profiles.entry(join.profile.clone()) {
            gameplay::load_player(player).context("Saved guest cannot be restored")?;
            slot.insert(player.to_vec());
        }
        resident.join_profiles.insert(id, join.profile.clone());
        Ok(true)
    }

    pub(super) fn flush_residents(&mut self) -> Result<()> {
        let mut first_error = None;
        for resident in self.realms.values_mut() {
            if let Err(error) = resident.checkpoint() {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(crate) fn request_gate(&mut self) -> bool {
        if matches!(self.role, Role::Client { ready: true, .. }) {
            self.crossing_requested = true;
            true
        } else {
            false
        }
    }

    pub(super) fn host_realms(
        &mut self, socket: &mut Host, world: &mut World, player: &mut Player,
        day: &mut DayNightCycle, input: &Input, dt: f32, blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>, now: Instant, network_tick: bool, late: &mut Vec<Deferred>,
    ) -> Result<()> {
        let packets = socket.poll(now).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if self.social.code.is_none() && let Some(code) = socket.code()
            && engine_net::punch::is_offer(code)
        {
            if self.headless { eprintln!("[multiplayer] punch offer={code}"); }
            self.social.code = Some(code.to_owned());
        }
        let connected = socket.players();
        self.realm_joins.retain(|id, _| connected.contains(id));
        let mut resident_peers = BTreeMap::new();
        for (key, realm) in &self.realms {
            for &id in realm.session.authority.actors.keys().chain(realm.session.join_profiles.keys()) {
                resident_peers.insert(id, key.clone());
            }
        }

        // Profile admission is connection-wide. A second connection may not
        // mount the same character while its first connection is in another realm.
        let mut claimed: std::collections::BTreeSet<_> = self.realm_joins.values().map(|join| join.profile.clone()).collect();
        let mut primary_packets = Vec::new();
        let mut realm_packets = BTreeMap::<String, Vec<_>>::new();
        for (id, bytes) in packets {
            if self.realm_joins.contains_key(&id) { continue; }
            if bytes.first() == Some(&10) && !resident_peers.contains_key(&id)
                && !self.authority.actors.contains_key(&id) && !self.join_profiles.contains_key(&id) {
                if let Ok(token) = decode::<String>(&bytes[1..])
                    && gameplay::valid_profile(&token)
                    && let Ok(profile) = profiles::registry_key(socket.fingerprint(id), socket.has_keys(), &token) {
                    let occupied = std::iter::once(&*self).chain(self.realms.values().map(|r| &r.session)).any(|s| {
                        s.authority.actors.values().any(|a| a.profile == profile)
                            || s.join_profiles.values().any(|p| *p == profile)
                    });
                    if occupied || !claimed.insert(profile.clone()) {
                        socket.disconnect(id);
                        continue;
                    }
                    match self.queue_realm_join(id, profile, now) {
                        Ok(false) => {}
                        Ok(true) => continue,
                        Err(error) => {
                            eprintln!("[multiplayer] could not queue guest realm for player {id}: {error:#}");
                            socket.disconnect(id);
                            continue;
                        }
                    }
                }
            }
            if let Some(key) = resident_peers.get(&id) {
                realm_packets.entry(key.clone()).or_default().push((id, bytes));
            } else {
                primary_packets.push((id, bytes));
            }
        }
        for (id, join) in std::mem::take(&mut self.realm_joins) {
            match self.resume_realm(id, &join, now) {
                Ok(true) => { resident_peers.insert(id, join.realm); }
                Ok(false) => { self.realm_joins.insert(id, join); }
                Err(error) => {
                    eprintln!("[multiplayer] could not reopen guest realm for player {id}: {error:#}");
                    socket.disconnect(id);
                }
            }
        }
        let mut primary_connected = Vec::new();
        let mut realm_connected = BTreeMap::<String, Vec<_>>::new();
        let bulk_per_peer = socket.link_room().map_or(usize::MAX, |room| {
            room.saturating_sub(pacing::LINK_RESERVE) / connected.len().max(1)
        });
        for id in connected {
            if self.realm_joins.contains_key(&id) { continue; }
            if let Some(key) = resident_peers.get(&id) {
                realm_connected.entry(key.clone()).or_default().push(id);
            } else {
                primary_connected.push(id);
            }
        }
        let primary_active = !self.headless || !primary_connected.is_empty();
        let active: Vec<_> = primary_active.then_some(None).into_iter()
            .chain(realm_connected.keys().map(|key| Some(key.clone()))).collect();
        let guaranteed = active.get(self.realm_turn % active.len().max(1)).cloned();
        self.realm_turn = self.realm_turn.wrapping_add(1);
        let mut simulation_left = Duration::from_millis(8);
        let quantum = simulation_left / active.len().max(1) as u32;
        self.host_frame(socket, world, player, day, input, dt, blocks, chunks, now, network_tick,
            primary_packets, &primary_connected, bulk_per_peer, &mut simulation_left, quantum,
            guaranteed == Some(None), late)?;
        let idle = Input::default();
        for (key, resident) in &mut self.realms {
            let Resident { session, saved, saves, .. } = resident.as_mut();
            let mut resident_late = Vec::new();
            session.host_frame(socket, &mut saved.world, &mut saved.player, &mut saved.day_night, &idle, dt,
                &mut HashSet::new(), &mut HashSet::new(), now, network_tick,
                realm_packets.remove(key).unwrap_or_default(), realm_connected.get(key).map(Vec::as_slice).unwrap_or_default(),
                bulk_per_peer, &mut simulation_left, quantum,
                guaranteed.as_ref().and_then(Option::as_ref) == Some(key), &mut resident_late)?;
            late.extend(resident_late.into_iter().filter(|effect| matches!(effect, Deferred::Stop)));
            saves.tick(Some(&mut saved.world), &saved.player, &saved.day_night);
            self.profiles_dirty |= std::mem::take(&mut session.profiles_dirty);
        }
        if network_tick {
            let mut requests: Vec<_> = std::mem::take(&mut self.crossings).into_iter().map(|(id, saved)| (id, None, saved)).collect();
            for (key, resident) in &mut self.realms {
                requests.extend(std::mem::take(&mut resident.session.crossings).into_iter().map(|(id, saved)| (id, Some(key.clone()), saved)));
            }
            // Bound baseline serialization work to a single crossing per network tick.
            requests.sort_unstable_by_key(|&(id, _, _)| id);
            let mut crossed = false;
            for (id, source, mut departure_saved) in requests {
                let result = if crossed { Ok(false) } else { self.cross_guest(world, player, day, id, source.as_deref(), now, &mut departure_saved) };
                match result {
                    Ok(true) => crossed = true,
                    Ok(false) => {
                        if let Some(key) = source {
                            self.realms.get_mut(&key).expect("source is resident").session.crossings.insert(id, departure_saved);
                        } else {
                            self.crossings.insert(id, departure_saved);
                        }
                    }
                    Err(error) => host_commands::tell(socket, Some(&self.name), id, error.to_string(), &mut Vec::new()),
                }
            }
            if self.realm_joins.is_empty() && self.crossings.is_empty()
                && self.realms.values().all(|realm| realm.session.crossings.is_empty())
            {
                self.realm_loads.clear();
            }
            socket.advance().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        Ok(())
    }

    fn cross_guest(
        &mut self, world: &mut World, host: &Player, day: &DayNightCycle, id: PlayerId,
        source: Option<&str>, now: Instant, departure_saved: &mut bool,
    ) -> Result<bool> {
        ensure!(self.sidecar.is_some(), "Realm crossing requires a hosted save");
        let (source_world, source_session) = if let Some(key) = source {
            let resident = self.realms.get_mut(key).context("Source realm unavailable")?;
            (&mut resident.saved.world, &mut resident.session)
        } else {
            (&mut *world, &mut *self)
        };
        let actor = source_session.authority.actors.get_mut(&id).context("Player left the realm")?;
        actor.personal.swap(source_world);
        let linked = world::hell_gate::linked(source_world, &actor.player);
        actor.personal.swap(source_world);
        let linked = linked?;
        let arrival = linked.arrival_hint(&actor.player);
        if !*departure_saved {
            self.flush_profiles().context("Guest departure checkpoint failed")?;
            *departure_saved = true;
        }
        let destination = &linked.destination;
        let primary = destination == world.regions.view.session_name();
        if !primary && !self.ensure_resident(destination, Some(arrival))? { return Ok(false); }

        let source_authority = match source {
            Some(key) => &self.realms.get(key).context("Source realm unavailable")?.session.authority,
            None => &self.authority,
        };
        let actor = source_authority.actors.get(&id).context("Player left the realm")?;
        let source_save = source_authority.save.clone();
        let profile = actor.profile.clone();
        let mut staged = gameplay::load_player(&gameplay::save_player(&actor.player, &actor.personal)?)?;
        let (target, target_authority, target_day, center) = if primary {
            let center = linked.arrival(world, &staged.player)?;
            (&mut *world, &self.authority, day, center)
        } else {
            let resident = self.realms.get_mut(destination).context("Destination realm unavailable")?;
            if resident.saving_gate {
                if resident.saves.busy() { return Ok(false); }
                if resident.saves.failed {
                    resident.saves.request(true);
                    anyhow::bail!("Destination gate checkpoint failed");
                }
                resident.saving_gate = false;
            }
            let gate_count = resident.saved.world.portals.hell.len();
            let arrival = linked.arrival(&mut resident.saved.world, &staged.player);
            if resident.saved.world.portals.hell.len() != gate_count {
                resident.saves.request(true);
                resident.saving_gate = true;
                arrival?;
                return Ok(false);
            }
            (&mut resident.saved.world, &resident.session.authority, &resident.saved.day_night, arrival?)
        };
        let occupied: Vec<_> = target_authority.actors.values().map(|a| a.player.bounds(target.voxel_scale))
            .chain((primary && !self.headless).then(|| host.bounds(target.voxel_scale))).collect();
        let (position, _) = world::spawn::portal_landing(target, center, &staged.player, &occupied).context("No safe landing at the far gate")?;
        world::progression::thresholds::Milestone::arrival(&mut staged.personal.progression, target.world_type);
        let transfer = prepare_guest(&mut staged, position, &source_save, target, target_day, id)?;

        let player = gameplay::save_player(&staged.player, &staged.personal)?;
        let entry = if primary { player } else { profiles::in_realm(destination, &player)? };
        // This replacement is the commit point. After it succeeds, a reconnect
        // uses the staged destination even if the client never sees its baseline.
        let saved = self.checkpoint_guest(&profile, &entry)
            .context("Guest crossing checkpoint failed")?;

        let source_session = match source {
            Some(key) => &mut self.realms.get_mut(key).expect("source is resident").session,
            None => &mut *self,
        };
        let actor = source_session.authority.actors.remove(&id).expect("crossing actor is present");
        source_session.authority.outputs.remove(&id);
        source_session.authority.footsteps.remove(&id);
        source_session.joined_at.remove(&id);
        source_session.profiles.remove(&actor.profile);
        source_session.profiles_dirty = true;
        let _ = source_session.board.change(Change::Clear { player: id });
        let target_session = if primary { &mut *self } else {
            &mut self.realms.get_mut(destination).expect("destination is resident").session
        };
        target_session.profiles.remove(&actor.profile);
        target_session.profiles_dirty = true;
        target_session.joined_at.insert(id, now);
        target_session.authority.admit(id, actor, staged, transfer, now);
        self.profiles = saved;
        for actor in self.authority.actors.values() {
            self.profiles.remove(&actor.profile);
        }
        Ok(true)
    }
}

fn prepare_guest(staged: &mut gameplay::SavedPlayer, pos: Vec3, source: &str, target: &mut World,
    day: &DayNightCycle, id: PlayerId,
) -> Result<Baseline> {
    staged.player.pos = pos;
    staged.player.precise_pos = pos.as_dvec3();
    staged.player.vel = Vec3::ZERO;
    staged.player.anchor = None;
    staged.player.gravity_frame = glam::Quat::IDENTITY;
    staged.player.step_camera_offset = 0.0;
    staged.player.jump_tap_timer = 0.0;
    staged.personal.focus = None;
    staged.personal.bench = None;
    staged.personal.inventory.container = None;
    staged.personal.adventure.projectiles.clear();
    staged.personal.adventure.cancel_cast_input();
    staged.personal.transfer_spawn(source, target, pos * target.voxel_scale.meters());
    staged.personal.swap(target);
    let snapshot = make_snapshot(target, &staged.player, day, id);
    staged.personal.swap(target);
    snapshot
}

impl Authority {
    fn admit(&mut self, id: PlayerId, actor: Actor, staged: gameplay::SavedPlayer, transfer: Baseline, now: Instant) {
        let mut next = Actor::new(staged.player, now);
        next.personal = staged.personal;
        next.profile = actor.profile;
        next.creative = actor.creative;
        next.restored = true;
        self.actors.insert(id, next);
        self.outputs.insert(id, Output {
            contacts: Default::default(), sounds: Default::default(),
            transfer: Some(transfer.sender), terrain: Terrain::new(transfer.deferred), image: None, image_due: now,
            coherence: Default::default(),
            cells: Default::default(), pace: pacing::Pacer::new(pacing::TRANSFER_BYTES_PER_SECOND),
            ready: false, started: now, mirror: Default::default(), refreshed: now,
            water_resent: now, probe: Default::default(),
        });
    }
}

#[cfg(test)]
#[path = "realm_terrain_tests.rs"]
mod terrain_tests;
