use crate::protocol::*;
use crate::social;
use crate::admin::{AccessList, Admission};
use crate::identity;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(15);
/// Messages one peer may send per second.
const MESSAGE_RATE: u32 = 120;
/// Longest host stall, in seconds, whose backlog is excused from the rate limit.
const MAX_STALL_SECONDS: u32 = 30;
pub const INPUT_LEAD: u64 = 3;
pub const MAX_LEAD: u64 = 8;
const MAX_QUEUED_COMMANDS: usize = 16;

struct Peer {
    joined: bool,
    described: bool,
    connected: Instant,
    last_input: Instant,
    /// Sent gameplay traffic at least once; a peer still loading the world is not idle.
    active: bool,
    sequence: u64,
    pending: BTreeMap<u64, Input>,
    budget: u32,
    /// Messages allowed in the current window; grows after a host stall, see `receive`.
    budget_limit: u32,
    budget_start: Instant,
    name: Option<String>,
    commands: Vec<String>,
    challenge: [u8; 32],
    /// Fingerprint of the key that answered `challenge`; fixed once set.
    key: Option<String>,
}

/// IDs are assigned by the host, bound to a transport connection, and never reused.
pub struct HostSession {
    world: WorldIdentity,
    tick: u64,
    next_player: PlayerId,
    peers: BTreeMap<PlayerId, Peer>,
    access: Option<AccessList>,
    whitelist: bool,
    capacity: usize,
    world_kind: u8,
}
impl HostSession {
    pub fn new(world: WorldIdentity) -> Self {
        Self {
            world,
            tick: 0,
            next_player: 1,
            peers: BTreeMap::new(),
            access: None,
            whitelist: false,
            capacity: MAX_PLAYERS,
            world_kind: 1,
        }
    }

    /// Install the operator, whitelist and ban lists. Without this every name is
    /// admitted, which is the behaviour of a private session.
    pub fn set_access(&mut self, access: AccessList, whitelist: bool) {
        self.access = Some(access);
        self.whitelist = whitelist;
    }

    pub fn access(&self) -> Option<&AccessList> {
        self.access.as_ref()
    }
    pub fn set_capacity(&mut self, players: usize) { self.capacity = players.min(MAX_PLAYERS); }
    pub fn set_world_kind(&mut self, kind: u8) -> Result<(),ProtocolError> {
        crate::protocol::validate_world_kind(kind)?;
        self.world_kind=kind; Ok(())
    }

    pub fn access_mut(&mut self) -> Option<&mut AccessList> {
        self.access.as_mut()
    }
    pub fn connect(&mut self, now: Instant) -> Result<PlayerId, ProtocolError> {
        if self.peers.len() >= MAX_PLAYERS || self.peers.values().filter(|peer|!peer.described).count()>=self.capacity {
            return Err(ProtocolError("server full"));
        }
        let challenge =
            identity::random_bytes().map_err(|_| ProtocolError("no secure randomness"))?;
        let id = self.next_player;
        self.next_player = id
            .checked_add(1)
            .ok_or(ProtocolError("player IDs exhausted"))?;
        self.peers.insert(
            id,
            Peer {
                joined: false,
                described: false,
                connected: now,
                last_input: now,
                active: false,
                sequence: 0,
                pending: BTreeMap::new(),
                budget: 0,
                budget_limit: MESSAGE_RATE,
                budget_start: now,
                name: None,
                commands: Vec::new(),
                challenge,
                key: None,
            },
        );
        Ok(id)
    }
    pub fn disconnect(&mut self, id: PlayerId) {
        self.peers.remove(&id);
    }
    pub fn is_joined(&self, id: PlayerId) -> bool {
        self.peers.get(&id).is_some_and(|p| p.joined)
    }
    pub fn receive(
        &mut self,
        id: PlayerId,
        packet: Packet,
        now: Instant,
    ) -> Result<Option<Packet>, ProtocolError> {
        self.receive_at(id, packet, now, now)
    }

    pub(crate) fn receive_at(
        &mut self,
        id: PlayerId,
        packet: Packet,
        now: Instant,
        arrived: Instant,
    ) -> Result<Option<Packet>, ProtocolError> {
        // With an access list, only a peer admitted under a name may act; the
        // gate is checked on that name, so an unnamed peer would bypass it.
        let gated = self.access.is_some();
        let capacity_available=self.peers.values().filter(|peer|!peer.described).count()<self.capacity;
        let p = self
            .peers
            .get_mut(&id)
            .ok_or(ProtocolError("unknown connection"))?;
        let window = arrived.saturating_duration_since(p.budget_start);
        if window >= Duration::from_secs(1) {
            // The host reads its queue once per frame. A frame that stalled for `window` seconds
            // delivers that many seconds of a well-behaved peer's traffic in one burst.
            p.budget = 0;
            p.budget_limit = MESSAGE_RATE * (window.as_secs() as u32 + 1).min(MAX_STALL_SECONDS);
            p.budget_start = arrived;
        }
        p.budget += 1;
        if p.budget > p.budget_limit {
            return Err(ProtocolError("message rate exceeded"));
        }
        let joined = p.joined;
        let admitted = joined && (p.name.is_some() || !gated);
        if let Packet::Introduce { name, key, signature } = &packet {
            if !joined {
                return Err(ProtocolError("unexpected packet for session state"));
            }
            if !identity::verify_introduce(key, &p.challenge, &self.world, signature) {
                return Err(ProtocolError("identity not proven"));
            }
            let fingerprint = identity::fingerprint(key);
            if p.key.as_ref().is_some_and(|held| *held != fingerprint) {
                return Err(ProtocolError("identity changed"));
            }
            self.assign_name(id, name, Some(&fingerprint))?;
            if let Some(p) = self.peers.get_mut(&id) {
                p.key = Some(fingerprint);
                p.last_input = now;
            }
            return Ok(Some(Packet::Roster { players: self.roster() }));
        }
        let p = self
            .peers
            .get_mut(&id)
            .ok_or(ProtocolError("unknown connection"))?;
        match packet {
            Packet::Describe if !p.joined => {
                p.described=true;
                Ok(Some(Packet::World {world:self.world,kind:self.world_kind}))
            }
            Packet::Data(bytes) if admitted => {
                p.last_input = now;
                // Profile (10) and snapshot acknowledgement (4) are sent once while loading; a
                // joiner waiting for a slow host snapshot is silent afterwards, so they do not
                // start the idle clock.
                p.active |= !matches!(bytes.first(), Some(4 | 10));
                Ok(None)
            }
            Packet::Hello(world) if !p.joined => {
                if p.described&&!capacity_available {return Err(ProtocolError("server full"));}
                if world != self.world {
                    return Err(ProtocolError("world or content mismatch"));
                }
                p.joined = true;
                p.described = false;
                p.last_input = now;
                Ok(Some(Packet::Welcome {
                    player: id,
                    tick: self.tick,
                    challenge: p.challenge,
                }))
            }
            Packet::Input(input) if admitted => {
                if input.buttons & !0x000f != 0 {
                    return Err(ProtocolError("invalid buttons"));
                }
                // Reliable channel guarantees order; gaps/replays are protocol errors.
                if Some(input.sequence) != p.sequence.checked_add(1) {
                    return Err(ProtocolError("invalid input sequence"));
                }
                p.sequence = input.sequence;
                p.last_input = now;
                p.active = true;
                // Late inputs are harmless and never rewind the authoritative host.
                if input.tick <= self.tick {
                    return Ok(None);
                }
                if input.tick - self.tick > MAX_LEAD {
                    return Err(ProtocolError("input too far ahead"));
                }
                if p.pending.insert(input.tick, input).is_some() {
                    return Err(ProtocolError("duplicate tick input"));
                }
                Ok(None)
            }
            Packet::Command { text } if admitted => {
                let line = social::sanitize_chat(&text)
                    .map_err(|_| ProtocolError("invalid command"))?;
                // Nothing may drain the queue yet, so it must not grow per message.
                if p.commands.len() >= MAX_QUEUED_COMMANDS {
                    return Err(ProtocolError("too many queued commands"));
                }
                p.commands.push(line);
                p.last_input = now;
                p.active = true;
                Ok(None)
            }
            _ => Err(ProtocolError("unexpected packet for session state")),
        }
    }

    /// Applies `/name` (`Effect::Rename`) under the same checks as `Introduce`.
    pub fn rename(&mut self, id: PlayerId, wanted: &str) -> Result<String, ProtocolError> {
        let key = match self.peers.get(&id) {
            Some(p) if p.joined => p.key.clone(),
            _ => return Err(ProtocolError("unknown connection")),
        };
        self.assign_name(id, wanted, key.as_deref())
    }

    /// Fingerprint of the key a player proved on join.
    pub fn fingerprint(&self, id: PlayerId) -> Option<&str> {
        self.peers.get(&id).and_then(|p| p.key.as_deref())
    }

    /// Whether this player holds operator rights: their name's operator entry
    /// is bound to the key they proved.
    pub fn holds_op(&self, id: PlayerId) -> bool {
        let Some(p) = self.peers.get(&id) else { return false };
        match (&self.access, &p.name, &p.key) {
            (Some(access), Some(name), Some(key)) => access.op_holds(name, key),
            _ => false,
        }
    }

    /// Admits `raw` for `id` and stores the first free form of it.
    ///
    /// Names are unique ignoring case because the access lists are keyed by
    /// lower-case name. An operator name bound to another key is refused, so
    /// the name shown always matches whoever holds the rights.
    fn assign_name(&mut self, id: PlayerId, raw: &str, key: Option<&str>) -> Result<String, ProtocolError> {
        let wanted = social::sanitize_name(raw).map_err(|_| ProtocolError("invalid name"))?;
        if let Some(access) = &self.access {
            match access.admits(&wanted, key, self.whitelist) {
                Admission::Allowed => {}
                Admission::NotWhitelisted => {
                    return Err(ProtocolError("not on this server's whitelist"));
                }
                Admission::Banned(_) => return Err(ProtocolError("banned from this server")),
            }
            if let Some(Some(bound)) = access.op_binding(&wanted)
                && key.is_none_or(|key| !bound.eq_ignore_ascii_case(key))
            {
                return Err(ProtocolError("name belongs to an operator"));
            }
        }
        let others: Vec<String> = self
            .roster()
            .into_iter()
            .filter(|(other, _)| *other != id)
            .map(|(_, name)| name)
            .collect();
        let assigned =
            social::unique_name(&wanted, |c| others.iter().any(|n| n.eq_ignore_ascii_case(c)));
        // The first proven key to take a whitelisted name claims its entry.
        if let (Some(access), Some(key)) = (self.access.as_mut(), key)
            && assigned == wanted
            && access.is_whitelisted(&wanted)
        {
            access
                .bind_whitelist(&wanted, key)
                .map_err(|_| ProtocolError("not on this server's whitelist"))?;
        }
        let p = self.peers.get_mut(&id).ok_or(ProtocolError("unknown connection"))?;
        p.name = Some(assigned.clone());
        Ok(assigned)
    }

    /// Names of every joined player, sorted by id.
    pub fn roster(&self) -> Vec<(PlayerId, String)> {
        self.peers
            .iter()
            .filter(|(_, p)| p.joined)
            .map(|(&id, p)| (id, p.name.clone().unwrap_or_else(|| format!("player{id}"))))
            .collect()
    }

    /// Command lines received since the last call, in arrival order.
    pub fn take_commands(&mut self) -> Vec<(PlayerId, String)> {
        let mut taken = Vec::new();
        for (&id, peer) in self.peers.iter_mut() {
            for text in peer.commands.drain(..) {
                taken.push((id, text));
            }
        }
        taken
    }
    pub fn expired(&self, now: Instant) -> Vec<PlayerId> {
        self.peers
            .iter()
            .filter_map(|(&id, p)| {
                // Hello alone does not complete admission on a keyed server.
                // Repeated introductions must not extend this absolute deadline.
                let expired = if !p.joined || (self.access.is_some() && p.key.is_none()) {
                    now.duration_since(p.connected) >= HANDSHAKE_TIMEOUT
                } else {
                    // A joiner building its world and renderer sends nothing for longer than
                    // the idle limit; the game bounds that phase with its own sync timeout.
                    p.active && now.duration_since(p.last_input) >= IDLE_TIMEOUT
                };
                if expired {
                    eprintln!(
                        "[session] player {id} expired joined={} keyed={} active={} since_connect_ms={} since_input_ms={}",
                        p.joined, p.key.is_some(), p.active,
                        now.duration_since(p.connected).as_millis(), now.duration_since(p.last_input).as_millis()
                    );
                }
                expired.then_some(id)
            })
            .collect()
    }
    /// Call once per simulation tick. Returned actions still require game validation.
    pub fn advance(&mut self) -> Result<Packet, ProtocolError> {
        self.tick = self
            .tick
            .checked_add(1)
            .ok_or(ProtocolError("tick exhausted"))?;
        let players = self
            .peers
            .iter_mut()
            .filter(|(_, p)| p.joined)
            .map(|(&id, p)| (id, p.pending.remove(&self.tick)))
            .collect();
        Ok(Packet::Frame {
            tick: self.tick,
            players,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> WorldIdentity {
        WorldIdentity {
            seed: 1,
            generator: 1,
            cell_mm: 1000,
            content: [0; 32],
        }
    }

    fn join(host: &mut HostSession, now: Instant) -> PlayerId {
        let id = host.connect(now).expect("connect");
        host.receive(id, Packet::Hello(world()), now).expect("hello");
        id
    }

    fn key(id: PlayerId) -> crate::identity::PlayerKey {
        crate::identity::PlayerKey::from_bytes([id as u8; 32])
    }

    #[test]
    fn discovery_does_not_occupy_or_bypass_the_last_player_slot() {
        let now=Instant::now();
        let mut host=HostSession::new(world());
        host.set_capacity(1);
        host.set_world_kind(2).unwrap();
        let query=host.connect(now).unwrap();
        assert_eq!(host.receive(query,Packet::Describe,now).unwrap(),Some(Packet::World{world:world(),kind:2}));
        let player=host.connect(now).expect("query no longer occupies the gameplay slot");
        assert_eq!(host.receive(query,Packet::Hello(world()),now),Err(ProtocolError("server full")));
        host.receive(player,Packet::Hello(world()),now).unwrap();
        assert_eq!(host.connect(now),Err(ProtocolError("server full")));
        assert!(host.expired(now+HANDSHAKE_TIMEOUT).contains(&query));
    }

    #[test]
    fn a_loading_joiner_outlives_the_idle_limit_until_it_sends_input() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let id = join(&mut host, now);
        let late = now + IDLE_TIMEOUT + HANDSHAKE_TIMEOUT;
        assert!(host.expired(late).is_empty(), "no gameplay traffic yet");
        host.receive(id, Packet::Data(vec![10]), late).expect("profile");
        assert!(host.expired(late + IDLE_TIMEOUT * 4).is_empty(), "the profile alone does not start the idle clock");
        host.receive(id, Packet::Data(vec![3]), late).expect("controls");
        assert!(host.expired(late + IDLE_TIMEOUT - Duration::from_millis(1)).is_empty());
        assert_eq!(host.expired(late + IDLE_TIMEOUT), vec![id]);
    }

    #[test]
    fn a_host_stall_does_not_turn_queued_traffic_into_a_flood() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let id = join(&mut host, now);
        let later = now + Duration::from_secs(8);
        // Eight seconds of 60 Hz traffic arrives in one frame.
        for _ in 0..480 {
            host.receive(id, Packet::Data(vec![3]), later).expect("queued traffic within the excused backlog");
        }
        // A real flood in the same window is still refused.
        let mut refused = false;
        for _ in 0..2000 {
            refused |= host.receive(id, Packet::Data(vec![3]), later).is_err();
        }
        assert!(refused);
    }

    /// `Introduce` signed by player `id`'s test key over its own challenge.
    fn intro(host: &HostSession, id: PlayerId, name: &str) -> Packet {
        let key = key(id);
        Packet::Introduce {
            name: name.into(),
            key: key.public(),
            signature: key.sign_introduce(&host.peers[&id].challenge, &world()),
        }
    }

    #[test]
    fn names_are_unique_and_commands_are_collected() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let first = join(&mut host, now);
        let second = join(&mut host, now);

        let reply = host
            .receive(first, intro(&host, first, "Ana"), now)
            .expect("introduce");
        match reply {
            Some(Packet::Roster { players }) => {
                assert!(
                    players.contains(&(first, "Ana".to_owned())),
                    "{players:?}"
                );
            }
            other => panic!("unexpected reply {other:?}"),
        }

        let reply = host
            .receive(second, intro(&host, second, "Ana"), now)
            .expect("introduce");
        match reply {
            Some(Packet::Roster { players }) => {
                assert_eq!(players.last(), Some(&(second, "Ana (2)".to_owned())));
            }
            other => panic!("unexpected reply {other:?}"),
        }

        assert_eq!(
            host.receive(first, Packet::Command { text: "/list".into() }, now)
                .expect("command"),
            None
        );
        assert_eq!(host.take_commands(), vec![(first, "/list".to_owned())]);
        assert!(host.take_commands().is_empty(), "commands are drained once");

        assert_eq!(
            host.roster(),
            vec![(first, "Ana".to_owned()), (second, "Ana (2)".to_owned())]
        );
        assert!(
            host.receive(first, intro(&host, first, "bad\nname"), now)
                .is_err(),
            "a name with a control character is refused"
        );
    }

    #[test]
    fn unnamed_players_get_a_placeholder() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let id = join(&mut host, now);
        assert_eq!(host.roster(), vec![(id, format!("player{id}"))]);
    }

    #[test]
    fn the_access_list_gates_names_at_the_door() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let mut access = AccessList::default();
        access.whitelist_add("Ana").unwrap();
        access.ban("Bo", "grief").unwrap();
        host.set_access(access, true);

        let first = join(&mut host, now);
        let second = join(&mut host, now);
        let third = join(&mut host, now);

        assert!(
            host.receive(first, intro(&host, first, "Ana"), now)
                .is_ok()
        );
        assert_eq!(
            host.receive(second, intro(&host, second, "Bo"), now),
            Err(ProtocolError("banned from this server"))
        );
        assert_eq!(
            host.receive(third, intro(&host, third, "Cy"), now),
            Err(ProtocolError("not on this server's whitelist"))
        );
        assert_eq!(host.roster().len(), 3, "peers stay until the host disconnects them");
    }

    #[test]
    fn a_session_without_an_access_list_admits_everyone() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let id = join(&mut host, now);
        assert!(host.receive(id, intro(&host, id, "Anyone"), now).is_ok());
    }

    #[test]
    fn an_access_list_refuses_actions_before_admission() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let mut access = AccessList::default();
        access.whitelist_add("Ana").unwrap();
        host.set_access(access, true);
        let id = join(&mut host, now);
        let refused = Err(ProtocolError("unexpected packet for session state"));
        assert_eq!(host.receive(id, Packet::Command { text: "/op Cy".into() }, now), refused);
        assert_eq!(host.receive(id, Packet::Data(vec![1]), now), refused);
        host.receive(id, intro(&host, id, "Ana"), now).expect("admitted");
        assert!(host.receive(id, Packet::Command { text: "/list".into() }, now).is_ok());
    }

    #[test]
    fn names_differing_only_in_case_are_one_name() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let mut access = AccessList::default();
        access.set_op("Admin", true).unwrap();
        host.set_access(access, false);
        let op = join(&mut host, now);
        let other = join(&mut host, now);
        host.receive(op, intro(&host, op, "Admin"), now).expect("op");
        host.receive(other, intro(&host, other, "ADMIN"), now).expect("named");
        assert_eq!(host.roster(), vec![(op, "Admin".into()), (other, "ADMIN (2)".into())]);
    }

    #[test]
    fn rename_runs_the_admission_checks() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let mut access = AccessList::default();
        access.set_op("Admin", true).unwrap();
        access.bind_op("Admin", &key(1).fingerprint()).unwrap();
        access.ban("Bo", "grief").unwrap();
        host.set_access(access, false);
        let op = join(&mut host, now);
        let id = join(&mut host, now);
        assert_eq!(op, 1, "the bound key is player 1's test key");
        host.receive(op, intro(&host, op, "Admin"), now).expect("op");
        host.receive(id, intro(&host, id, "Cy"), now).expect("named");
        assert_eq!(host.rename(id, "admin"), Err(ProtocolError("name belongs to an operator")));
        assert_eq!(
            host.receive(id, intro(&host, id, "admin"), now),
            Err(ProtocolError("name belongs to an operator")),
            "a second Introduce is a rename"
        );
        assert_eq!(host.rename(id, "bo"), Err(ProtocolError("banned from this server")));
        assert_eq!(host.rename(op, "ADMIN"), Ok("ADMIN".into()));
        assert_eq!(host.rename(id, "Dee"), Ok("Dee".into()));
    }

    #[test]
    fn operator_rights_follow_the_proven_key_not_the_name() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let mut access = AccessList::default();
        access.set_op("Admin", true).unwrap();
        access.bind_op("Admin", &key(1).fingerprint()).unwrap();
        access.set_op("Spare", true).unwrap();
        host.set_access(access, false);
        let op = join(&mut host, now);
        let impostor = join(&mut host, now);
        host.receive(op, intro(&host, op, "Admin"), now).expect("op");
        assert!(host.holds_op(op));
        host.disconnect(op);
        assert_eq!(
            host.receive(impostor, intro(&host, impostor, "Admin"), now),
            Err(ProtocolError("name belongs to an operator")),
            "an offline operator's name stays theirs"
        );
        host.receive(impostor, intro(&host, impostor, "Spare"), now).expect("unbound name");
        assert!(!host.holds_op(impostor), "an unbound operator entry grants nothing");
    }

    #[test]
    fn a_whitelisted_name_belongs_to_the_first_key_that_proves_it() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let mut access = AccessList::default();
        access.whitelist_add("Ana").unwrap();
        host.set_access(access, true);
        let ana = join(&mut host, now);
        let impostor = join(&mut host, now);
        host.receive(ana, intro(&host, ana, "Ana"), now).expect("first key claims the entry");
        host.disconnect(ana);
        assert_eq!(
            host.receive(impostor, intro(&host, impostor, "ana"), now),
            Err(ProtocolError("not on this server's whitelist")),
            "typing a whitelisted name is not enough"
        );
        let back = join(&mut host, now);
        let returning = Packet::Introduce {
            name: "Ana".into(),
            key: key(ana).public(),
            signature: key(ana).sign_introduce(&host.peers[&back].challenge, &world()),
        };
        host.receive(back, returning, now).expect("the bound key returns");
        assert!(host.access().is_some_and(|a| a.to_text().contains(&format!("white=ana|{}", key(ana).fingerprint()))));
    }

    #[test]
    fn introductions_must_prove_one_unchanging_key() {
        let now = Instant::now();
        let mut host = HostSession::new(world());
        let id = join(&mut host, now);
        let Packet::Introduce { name, key: public, .. } = intro(&host, id, "Ana") else { unreachable!() };
        let forged = Packet::Introduce { name, key: public, signature: [0; 64] };
        assert_eq!(host.receive(id, forged, now), Err(ProtocolError("identity not proven")));
        let other = key(9);
        let replayed = Packet::Introduce {
            name: "Ana".into(),
            key: other.public(),
            signature: other.sign_introduce(&[0; 32], &world()),
        };
        assert_eq!(host.receive(id, replayed, now), Err(ProtocolError("identity not proven")));
        host.receive(id, intro(&host, id, "Ana"), now).expect("proven");
        assert_eq!(host.fingerprint(id), Some(key(id).fingerprint().as_str()));
        let swap = Packet::Introduce {
            name: "Ana".into(),
            key: other.public(),
            signature: other.sign_introduce(&host.peers[&id].challenge, &world()),
        };
        assert_eq!(host.receive(id, swap, now), Err(ProtocolError("identity changed")));
    }
}
