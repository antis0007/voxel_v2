//! Standalone Valve GameNetworkingSockets transport. Call poll on one owner thread.
use crate::{admin::AccessList, protocol::*, session::*};
use gns::sys::ESteamNetworkingConnectionState::*;
use gns::{GnsConnection, GnsGlobal, GnsSocket, IsClient, IsReady, IsServer, SendFlags};
use std::{cell::RefCell, collections::{BTreeMap,VecDeque}, net::SocketAddr, sync::Arc, time::Instant};

pub type NetResult<T> = Result<T, Box<dyn std::error::Error>>;
const RECEIVE_BUDGET: usize = 128;
const MAX_SEND_QUEUE: u32 = 64 * 1024;

fn configure(conn: GnsConnection) -> NetResult<()> {
    use gns::{GnsConfig, sys::ESteamNetworkingConfigValue::*};
    let utils = GnsGlobal::get()?.utils();
    // Opt-in native impairment for reproducible host/join acceptance runs.
    if let Ok(value)=std::env::var("VOXEL_NET_LAG_MS") {
        let lag:i32=value.parse()?;
        if !(0..=5000).contains(&lag) {return Err(ProtocolError("invalid network lag").into());}
        utils.set_global_config_value(k_ESteamNetworkingConfig_FakePacketLag_Send,gns::GnsConfig::Int32(lag))?;
    }
    if let Ok(value)=std::env::var("VOXEL_NET_LOSS_PCT") {
        let loss:f32=value.parse()?;
        if !loss.is_finite()||!(0.0..=100.0).contains(&loss) {return Err(ProtocolError("invalid packet loss").into());}
        utils.set_global_config_value(k_ESteamNetworkingConfig_FakePacketLoss_Send,gns::GnsConfig::Float(loss))?;
    }
    for (key, value) in [
        (
            k_ESteamNetworkingConfig_RecvMaxMessageSize,
            MAX_PACKET as i32,
        ),
        (k_ESteamNetworkingConfig_RecvBufferSize, 64 * 1024),
        // Inputs and corrections must not wait for the default batching timer.
        (k_ESteamNetworkingConfig_NagleTime, 0),
        (
            k_ESteamNetworkingConfig_SendBufferSize,
            MAX_SEND_QUEUE as i32,
        ),
    ] {
        utils.set_connection_config_value(conn, key, GnsConfig::Int32(value))?;
    }
    Ok(())
}

fn terminal(state: gns::sys::ESteamNetworkingConnectionState) -> bool {
    matches!(
        state,
        k_ESteamNetworkingConnectionState_ClosedByPeer
            | k_ESteamNetworkingConnectionState_ProblemDetectedLocally
    )
}

fn send<S: IsReady>(socket: &GnsSocket<S>, conn: GnsConnection, packet: &Packet) -> NetResult<()> {
    send_encoded(socket, conn, packet.encode()?)
}

fn send_encoded<S: IsReady, P: gns::Payload + AsRef<[u8]>>(
    socket: &GnsSocket<S>, conn: GnsConnection, bytes: P,
) -> NetResult<()> {
    let (status, _) = socket.get_connection_real_time_status(conn, 0)?;
    if status
        .pending_bytes_reliable()
        .saturating_add(status.bytes_sent_unacked_reliable())
        .saturating_add(bytes.as_ref().len() as u32)
        > MAX_SEND_QUEUE
    {
        return Err(ProtocolError("slow peer: send queue exceeded").into());
    }
    // Every action and authoritative frame is ordered; no silent packet loss.
    let message =
        GnsGlobal::get()?
            .utils()
            .allocate_message(conn, SendFlags::RELIABLE, bytes);
    socket.send_message(message)?;
    Ok(())
}

pub struct SteamHost {
    socket: GnsSocket<IsServer>,
    session: HostSession,
    connections: BTreeMap<GnsConnection, PlayerId>,
    control: RefCell<BTreeMap<PlayerId,VecDeque<Packet>>>,
}
impl SteamHost {
    pub fn bind(address: SocketAddr, world: WorldIdentity) -> NetResult<Self> {
        Ok(Self {
            socket: GnsSocket::new(GnsGlobal::get()?).listen(address.ip(), address.port())?,
            session: HostSession::new(world),
            connections: BTreeMap::new(),
            control: RefCell::new(BTreeMap::new()),
        })
    }
    fn remove(&mut self, conn: GnsConnection, reason: &str) {
        if let Some(id) = self.connections.remove(&conn) {
            self.control.get_mut().remove(&id);
            self.session.disconnect(id);
            eprintln!("player {id} disconnected: {reason}");
        }
        let detail=std::ffi::CString::new(reason.chars().take(200).collect::<String>()).ok();
        let _ = self.socket.close_connection(conn, 1000, detail.as_deref(), false);
    }
    pub fn players(&self) -> Vec<PlayerId> {
        self.connections
            .values()
            .copied()
            .filter(|id| self.session.is_joined(*id))
            .collect()
    }
    pub fn send_data(&self, player: PlayerId, bytes: Vec<u8>) -> NetResult<bool> {
        self.send_packet(player, &Packet::Data(bytes))
    }
    pub fn send_bulk(&self, player: PlayerId, bytes: Vec<u8>) -> NetResult<bool> {
        self.send_data(player, bytes)
    }
    pub fn send_pose(&self, player: PlayerId, bytes: Vec<u8>) -> NetResult<()> {
        let conn = self.connections.iter().find_map(|(&conn, &id)| (id == player).then_some(conn))
            .ok_or(ProtocolError("unknown player"))?;
        let bytes = Packet::Data(bytes).encode()?;
        let flags = SendFlags::UNRELIABLE | SendFlags::NO_NAGLE | SendFlags::NO_DELAY;
        let message = GnsGlobal::get()?.utils().allocate_message(conn, flags, bytes);
        // A congested connection drops a superseded pose.
        let _ = self.socket.send_message(message);
        Ok(())
    }
    pub fn set_access(&mut self, access: AccessList, whitelist: bool) {
        self.session.set_access(access, whitelist);
    }
    pub fn set_capacity(&mut self, players: usize) { self.session.set_capacity(players); }
    pub fn set_world_kind(&mut self, kind: u8) -> NetResult<()> {Ok(self.session.set_world_kind(kind)?)}
    pub fn access(&self) -> Option<&AccessList> {
        self.session.access()
    }
    pub fn access_mut(&mut self) -> Option<&mut AccessList> {
        self.session.access_mut()
    }
    pub fn roster(&self) -> Vec<(PlayerId, String)> {
        self.session.roster()
    }
    pub fn take_commands(&mut self) -> Vec<(PlayerId, String)> {
        self.session.take_commands()
    }
    pub fn fingerprint(&self, player: PlayerId) -> Option<&str> {
        self.session.fingerprint(player)
    }
    pub fn holds_op(&self, player: PlayerId) -> bool {
        self.session.holds_op(player)
    }
    pub fn rename(&mut self, player: PlayerId, wanted: &str) -> NetResult<String> {
        Ok(self.session.rename(player, wanted)?)
    }
    /// Preserve protocol packet types for chat and administration; opaque data is separate.
    pub fn send_packet(&self, player: PlayerId, packet: &Packet) -> NetResult<bool> {
        let conn = self
            .connections
            .iter()
            .find(|(_, id)| **id == player)
            .map(|(conn, _)| *conn)
            .ok_or(ProtocolError("unknown player"))?;
        let (status, _) = self.socket.get_connection_real_time_status(conn, 0)?;
        let busy = status
            .pending_bytes_reliable()
            .saturating_add(status.bytes_sent_unacked_reliable())
            > MAX_SEND_QUEUE / 2
            || self.control.borrow().get(&player).is_some_and(|queue|!queue.is_empty());
        if busy {
            if matches!(packet,Packet::Data(_)) {return Ok(false);}
            let mut pending=self.control.borrow_mut();
            let queue=pending.entry(player).or_default();
            if queue.len()>=64 {return Err(ProtocolError("control queue exceeded").into());}
            queue.push_back(packet.clone());
            return Ok(true);
        }
        send(&self.socket, conn, packet)?;
        Ok(true)
    }
    pub fn disconnect(&mut self, player: PlayerId) {
        if let Some(conn) = self
            .connections
            .iter()
            .find(|(_, id)| **id == player)
            .map(|(conn, _)| *conn)
        {
            self.remove(conn, "application closed session");
        }
    }
    pub fn poll(&mut self, now: Instant) -> NetResult<Vec<(PlayerId, Vec<u8>)>> {
        let mut application = Vec::new();
        GnsGlobal::get()?.poll_callbacks();
        let events: Vec<_> = self.socket.receive_events().take(RECEIVE_BUDGET).collect();
        for event in events {
            let conn = event.connection();
            let state = event.info().state();
            if state == k_ESteamNetworkingConnectionState_Connecting
                && !self.connections.contains_key(&conn)
            {
                match self.session.connect(now) {
                    Ok(id) => {
                        if configure(conn).is_err() || self.socket.accept(conn).is_err() {
                            self.session.disconnect(id);
                            self.remove(conn, "accept failed");
                        } else {
                            self.connections.insert(conn, id);
                        }
                    }
                    Err(_) => self.remove(conn, "server full"),
                }
            } else if terminal(state) {
                self.remove(conn, event.info().end_debug());
            }
        }
        // One bounded batch per pump; message ownership is released by the binding.
        for message in self.socket.receive_messages::<RECEIVE_BUDGET>()? {
            let conn = message.connection();
            let Some(&id) = self.connections.get(&conn) else {
                continue;
            };
            let decoded = Packet::decode(message.payload());
            let data = match &decoded {
                Ok(Packet::Data(bytes)) => Some(bytes.clone()),
                _ => None,
            };
            let result = decoded.and_then(|p| self.session.receive(id, p, now));
            match result {
                Ok(Some(reply)) => {
                    if let Err(error) = send(&self.socket, conn, &reply) {
                        self.remove(conn, &error.to_string());
                    }
                }
                Ok(None) => {
                    if let Some(bytes) = data {
                        application.push((id, bytes));
                    }
                }
                Err(error) => self.remove(conn, &error.to_string()),
            }
        }
        for id in self.session.expired(now) {
            if let Some((&conn, _)) = self.connections.iter().find(|(_, player)| **player == id) {
                self.remove(conn, "session timeout");
            }
        }
        // Control traffic drains before the game's next bulk pump. Bound both
        // memory and each pass; an overloaded peer cannot consume host time.
        let mut failed=Vec::new();
        for (&conn,&id) in &self.connections {
            for _ in 0..8 {
                let Some(packet)=self.control.get_mut().get_mut(&id).and_then(|q|q.pop_front()) else {break};
                let (status,_)=self.socket.get_connection_real_time_status(conn,0)?;
                if status.pending_bytes_reliable().saturating_add(status.bytes_sent_unacked_reliable())>MAX_SEND_QUEUE/2 {
                    self.control.get_mut().get_mut(&id).unwrap().push_front(packet);
                    break;
                }
                if let Err(error)=send(&self.socket,conn,&packet) {failed.push((conn,error.to_string()));break;}
            }
        }
        for (conn,reason) in failed {self.remove(conn,&reason);}
        Ok(application)
    }
    /// Host game calls at its fixed-step boundary, then applies the returned frame.
    /// Failed sends drop only that peer; the next frame communicates its departure.
    pub fn advance(&mut self) -> NetResult<Packet> {
        let frame = self.session.advance()?;
        // Every peer receives the same bytes. The binding retains each Arc until
        // native delivery completes, without copying a payload per connection.
        let encoded: Arc<[u8]> = frame.encode()?.into();
        let joined: Vec<_> = self
            .connections
            .iter()
            .filter(|(_, id)| self.session.is_joined(**id))
            .map(|(&conn, _)| conn)
            .collect();
        for conn in joined {
            if let Err(error) = send_encoded(&self.socket, conn, Arc::clone(&encoded)) {
                self.remove(conn, &error.to_string());
            }
        }
        Ok(frame)
    }
}
impl Drop for SteamHost {
    fn drop(&mut self) {
        for &conn in self.connections.keys() {
            let _ = self.socket.close_connection(conn, 1000, None, false);
        }
    }
}

pub struct SteamClient {
    socket: GnsSocket<IsClient>,
    world: WorldIdentity,
    started: Instant,
    last_frame: Instant,
    hello_sent: bool,
    player: Option<PlayerId>,
    tick: u64,
    sequence: u64,
    last_sent_tick: u64,
    failed: bool,
    challenge: Option<[u8; 32]>,
    introduced: bool,
}
impl SteamClient {
    /// Query world settings without entering the player roster. The temporary
    /// connection is always closed, including failures and timeouts.
    pub fn describe(address: SocketAddr, content: [u8;32]) -> NetResult<(WorldIdentity,u8)> {
        let socket=GnsSocket::new(GnsGlobal::get()?).connect(address.ip(),address.port())?;
        let result=(|| {
            configure(socket.connection())?;
            let started=Instant::now();
            let mut sent=false;
            while started.elapsed()<HANDSHAKE_TIMEOUT {
                GnsGlobal::get()?.poll_callbacks();
                for event in socket.receive_events().take(RECEIVE_BUDGET) {
                    if terminal(event.info().state()) {return Err(format!("session closed: {}",event.info().end_debug()).into());}
                    if event.info().state()==k_ESteamNetworkingConnectionState_Connected&&!sent {
                        send(&socket,socket.connection(),&Packet::Describe)?;
                        sent=true;
                    }
                }
                for message in socket.receive_messages::<RECEIVE_BUDGET>()? {
                    let Packet::World {world,kind}=Packet::decode(message.payload())? else {
                        return Err(ProtocolError("expected world description").into());
                    };
                    if world.content!=content {return Err(ProtocolError("game build mismatch").into());}
                    return Ok((world,kind));
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(ProtocolError("world query timed out").into())
        })();
        let _=socket.close_connection(socket.connection(),1000,None,false);
        result
    }
    pub fn connect(address: SocketAddr, world: WorldIdentity, now: Instant) -> NetResult<Self> {
        let socket = GnsSocket::new(GnsGlobal::get()?).connect(address.ip(), address.port())?;
        configure(socket.connection())?;
        Ok(Self {
            socket,
            world,
            started: now,
            last_frame: now,
            hello_sent: false,
            player: None,
            tick: 0,
            sequence: 0,
            last_sent_tick: 0,
            failed: false,
            challenge: None,
            introduced: false,
        })
    }
    /// Whether `introduce` can be called: the host's welcome has arrived.
    pub fn can_introduce(&self) -> bool {
        self.challenge.is_some() && !self.introduced
    }
    /// Names this player and proves `key`; see `RelayClient::introduce`.
    pub fn introduce(&mut self, name: &str, key: &crate::identity::PlayerKey) -> NetResult<()> {
        let Some(challenge) = self.challenge else {
            return Err(ProtocolError("no welcome yet").into());
        };
        if self.introduced {
            return Err(ProtocolError("already introduced").into());
        }
        let packet = Packet::Introduce {
            name: name.to_owned(),
            key: key.public(),
            signature: key.sign_introduce(&challenge, &self.world),
        };
        send(&self.socket, self.socket.connection(), &packet)?;
        self.introduced = true;
        Ok(())
    }
    pub fn player(&self) -> Option<PlayerId> {
        self.player
    }
    pub fn tick(&self) -> u64 {
        self.tick
    }
    pub fn poll(&mut self, now: Instant) -> NetResult<Vec<Packet>> {
        if self.failed {
            return Err(ProtocolError("session closed").into());
        }
        let result = self.poll_inner(now);
        if result.is_err() {
            self.failed = true;
            let _ = self
                .socket
                .close_connection(self.socket.connection(), 1000, None, false);
        }
        result
    }
    fn poll_inner(&mut self, now: Instant) -> NetResult<Vec<Packet>> {
        GnsGlobal::get()?.poll_callbacks();
        for event in self.socket.receive_events().take(RECEIVE_BUDGET) {
            if terminal(event.info().state()) {
                return Err(format!("session closed: {}",event.info().end_debug()).into());
            }
            if event.info().state() == k_ESteamNetworkingConnectionState_Connected
                && !self.hello_sent
            {
                send(
                    &self.socket,
                    self.socket.connection(),
                    &Packet::Hello(self.world),
                )?;
                self.hello_sent = true;
            }
        }
        let mut frames = Vec::new();
        for message in self.socket.receive_messages::<RECEIVE_BUDGET>()? {
            match Packet::decode(message.payload())? {
                // The host answers `Introduce` with `Roster` and relays chat.
                packet @ (Packet::Data(_) | Packet::Roster { .. } | Packet::Chat { .. })
                    if self.player.is_some() =>
                {
                    self.last_frame = now;
                    frames.push(packet);
                }
                Packet::Welcome { player, tick, challenge } if self.hello_sent && self.player.is_none() => {
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
                _ => return Err(ProtocolError("unexpected server packet").into()),
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
    /// Tick is chosen locally from the host clock; no client position is trusted.
    pub fn submit(&mut self, movement: [i16; 2], look: [i16; 2], buttons: u16) -> NetResult<()> {
        if self.failed || self.player.is_none() {
            return Err(ProtocolError("session not ready").into());
        }
        if buttons & !0x000f != 0 {
            return Err(ProtocolError("invalid buttons").into());
        }
        let tick = self
            .tick
            .checked_add(INPUT_LEAD)
            .ok_or(ProtocolError("tick exhausted"))?;
        if tick <= self.last_sent_tick {
            return Ok(());
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(ProtocolError("sequence exhausted"))?;
        send(
            &self.socket,
            self.socket.connection(),
            &Packet::Input(Input {
                sequence,
                tick,
                movement,
                look,
                buttons,
            }),
        )?;
        self.sequence = sequence;
        self.last_sent_tick = tick;
        Ok(())
    }
    pub fn send_data(&self, bytes: Vec<u8>) -> NetResult<bool> {
        if self.failed || self.player.is_none() {
            return Err(ProtocolError("session not ready").into());
        }
        let conn = self.socket.connection();
        let (status, _) = self.socket.get_connection_real_time_status(conn, 0)?;
        if status
            .pending_bytes_reliable()
            .saturating_add(status.bytes_sent_unacked_reliable())
            > MAX_SEND_QUEUE / 2
        {
            return Ok(false);
        }
        send(&self.socket, conn, &Packet::Data(bytes))?;
        Ok(true)
    }
}
impl Drop for SteamClient {
    fn drop(&mut self) {
        let _ = self
            .socket
            .close_connection(self.socket.connection(), 1000, None, false);
    }
}
