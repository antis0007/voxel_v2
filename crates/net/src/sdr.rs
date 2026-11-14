//! Steamworks Networking Sockets. Steam supplies identity, rendezvous and SDR;
//! the existing host session remains authoritative over the gameplay protocol.
use crate::{admin::AccessList, protocol::*, session::*, identity::PlayerKey};
use steamworks::{Client, AppId, SteamError, SteamId, networking_sockets::{ListenSocket,NetConnection},
    networking_types::{NetworkingIdentity,NetworkingConfigEntry as Config,NetworkingConfigValue as Value,
        NetworkingAvailability, NetworkingConnectionState as State, ListenSocketEvent, SendFlags, NetConnectionEnd,AppNetConnectionEnd}};
use std::{cell::RefCell,collections::{BTreeMap,VecDeque},net::SocketAddr,sync::{Arc,Mutex},time::{Instant,Duration}};
#[path = "sdr/invites.rs"]
pub mod invites;
pub type NetResult<T> = Result<T,Box<dyn std::error::Error>>;
const QUEUE:i64=64*1024;
const FRAME_BACKLOG_BYTES: usize = 128 * 1024;
const FRAME_BACKLOG_TIME: Duration = Duration::from_secs(8);
const BULK_QUEUE_DELAY_US: i64 = 12_000;
const BATCH:usize=128;
const VIRTUAL_PORT:i32=0;
const IP_CERT_WAIT: Duration = Duration::from_secs(8);
// Steam callbacks use one SDK instance and are never pumped concurrently by
// menu discovery and gameplay. The SDK is initialized only when actually used.
struct Context {
    client: Client,
    server: Option<steamworks::Server>,
    invite_listener: std::sync::OnceLock<Arc<std::sync::atomic::AtomicBool>>,
}
#[derive(Clone)]
pub struct Runtime(Arc<Context>);
impl std::ops::Deref for Runtime {type Target=Client;fn deref(&self)->&Client{&self.0.client}}
static SDK:Mutex<Option<Arc<Context>>>=Mutex::new(None);
static PUMP:Mutex<()>=Mutex::new(());
/// Declare before game resources so sockets drop before the process SDK.
pub struct RuntimeScope;
impl Drop for RuntimeScope {fn drop(&mut self){if let Ok(mut sdk)=SDK.lock(){sdk.take();}}}
pub fn runtime()->NetResult<Runtime> {
    let _guard=PUMP.lock().map_err(|_|ProtocolError("Steam callback lock failed"))?;
    let mut cache=SDK.lock().map_err(|_|ProtocolError("Steam runtime lock failed"))?;
    if let Some(context)=cache.as_ref(){return Ok(Runtime(Arc::clone(context)));}
    let app=std::env::var("VOXEL_STEAM_APP_ID").unwrap_or_else(|_|"480".into()).parse::<u32>()?;
    if app==0{return Err(ProtocolError("invalid Steam App ID").into());}
    let (client,server)=if std::env::var("VOXEL_STEAM_DEDICATED").as_deref()==Ok("1") {
        if std::env::var("SteamAppId").ok().and_then(|v|v.parse::<u32>().ok())!=Some(app){return Err(ProtocolError("dedicated SteamAppId must match VOXEL_STEAM_APP_ID").into());}
        let (server,client)=steamworks::Server::init(std::net::Ipv4Addr::UNSPECIFIED,0,steamworks::QUERY_PORT_SHARED,steamworks::ServerMode::Authentication,"0.1.0")?;
        server.set_product(&app.to_string());server.set_mod_dir("voxel");server.set_game_description("Voxel");server.set_dedicated_server(true);server.log_on_anonymous();
        eprintln!("[steamworks] anonymous_login=waiting");
        let began=Instant::now();
        // A cold Linux runtime can spend 45 seconds measuring its CM list.
        while server.steam_id().raw()&0xffff_ffff==0&&began.elapsed()<Duration::from_secs(90){client.run_callbacks();std::thread::sleep(Duration::from_millis(25));}
        if server.steam_id().raw()&0xffff_ffff==0{return Err(ProtocolError("anonymous Steam server login timed out").into());}
        (client,Some(server))
    } else {(Client::init_app(AppId(app)).map_err(|e|format!("Steam initialization failed for App ID {app}: {e}. Start Steam and sign in."))?,None)};
    let client=Runtime(Arc::new(Context{client,server,invite_listener: Default::default()}));
    configure_impairment()?;
    configure_diagnostics();
    let networking = sockets(&client);
    let mut auth = networking.init_authentication();
    if client.0.server.is_some() {
        // Anonymous login assigns an identity before its networking certificate arrives.
        let deadline = Instant::now() + Duration::from_secs(30);
        while auth != Ok(NetworkingAvailability::Current) && Instant::now() < deadline {
            client.run_callbacks();
            std::thread::sleep(Duration::from_millis(25));
            auth = networking.get_authentication_status();
        }
        if auth != Ok(NetworkingAvailability::Current) {
            return Err(format!("Steam networking certificate unavailable after anonymous login: {auth:?}").into());
        }
    }
    eprintln!("[steamworks] app={app} identity={} authentication={auth:?}",identity(&client));
    *cache=Some(Arc::clone(&client.0));Ok(client)
}
fn pump(client:&Runtime)->NetResult<()> {
    let _guard=PUMP.lock().map_err(|_|ProtocolError("Steam callback lock failed"))?;
    client.run_callbacks();Ok(())
}
fn identity(client:&Runtime)->u64{client.0.server.as_ref().map_or_else(||client.user().steam_id().raw(),|s|s.steam_id().raw())}
fn sockets(client:&Runtime)->steamworks::networking_sockets::NetworkingSockets{client.0.server.as_ref().map_or_else(||client.networking_sockets(),|s|s.networking_sockets())}
pub fn local_id()->NetResult<u64>{Ok(identity(&runtime()?))}
pub fn relay_status()->NetResult<String>{
    let sdk = runtime()?;
    sdk.networking_utils().init_relay_network_access();
    Ok(format!("{:?}",sdk.networking_utils().detailed_relay_network_status()))
}
fn force_sdr()->bool{std::env::var("VOXEL_STEAM_FORCE_SDR").as_deref()==Ok("1")}
fn configure_diagnostics() {
    use steamworks::sys::{self, ESteamNetworkingSocketsDebugOutputType as Level};
    if std::env::var_os("VOXEL_STEAM_NET_TRACE").is_none() { return; }
    unsafe extern "C" fn log(_: Level, message: *const std::ffi::c_char) {
        use std::io::Write;
        if !message.is_null() {
            // The SDK owns this terminated string until the callback returns.
            let message = unsafe { std::ffi::CStr::from_ptr(message) };
            let _ = writeln!(std::io::stderr().lock(), "[steamworks-net] {}", message.to_string_lossy());
        }
    }
    // This callback only writes diagnostics and never calls back into Steam.
    unsafe {
        let utils = sys::SteamAPI_SteamNetworkingUtils_SteamAPI_v004();
        if !utils.is_null() {
            sys::SteamAPI_ISteamNetworkingUtils_SetDebugOutputFunction(
                utils, Level::k_ESteamNetworkingSocketsDebugOutputType_Msg, Some(log),
            );
        }
    }
}
fn configure_impairment() -> NetResult<()> {
    use steamworks::sys::{self, ESteamNetworkingConfigDataType as Type,
        ESteamNetworkingConfigScope as Scope, ESteamNetworkingConfigValue as ConfigValue};
    fn set<T>(key: ConfigValue, kind: Type, value: &T) -> NetResult<()> {
        // The initialized SDK owns the interface. SetConfigValue copies the value synchronously.
        let ok = unsafe {
            let utils = sys::SteamAPI_SteamNetworkingUtils_SteamAPI_v004();
            !utils.is_null() && sys::SteamAPI_ISteamNetworkingUtils_SetConfigValue(
                utils, key, Scope::k_ESteamNetworkingConfig_Global, 0, kind,
                (value as *const T).cast(),
            )
        };
        if ok { Ok(()) } else { Err(ProtocolError("native impairment setting was refused").into()) }
    }
    if let Ok(value) = std::env::var("VOXEL_NET_LAG_MS") {
        let lag: i32 = value.parse()?;
        if !(0..=5000).contains(&lag) { return Err(ProtocolError("invalid network lag").into()); }
        set(ConfigValue::k_ESteamNetworkingConfig_FakePacketLag_Send, Type::k_ESteamNetworkingConfig_Int32, &lag)?;
    }
    if let Ok(value) = std::env::var("VOXEL_NET_LOSS_PCT") {
        let loss: f32 = value.parse()?;
        if !(0.0..=100.0).contains(&loss) { return Err(ProtocolError("invalid packet loss").into()); }
        set(ConfigValue::k_ESteamNetworkingConfig_FakePacketLoss_Send, Type::k_ESteamNetworkingConfig_Float, &loss)?;
    }
    Ok(())
}
fn options()->Vec<Config>{
    let mut values=vec![Config::new_int32(Value::NagleTime,0),Config::new_int32(Value::SendBufferSize,QUEUE as i32)];
    if force_sdr(){values.push(Config::new_int32(Value::P2PTransportICEEnable,0));}
    values
}
fn send_queued(client: &Runtime, connection: &NetConnection, bytes: &[u8], bulk: bool) -> NetResult<bool> {
    let (status, _) = sockets(client).get_realtime_connection_status(connection, 0)?;
    let pending = i64::from(status.pending_reliable()) + i64::from(status.pending_unreliable());
    // steamworks 0.13 names this accessor "bytes", but it returns m_usecQueueTime.
    if pending + bytes.len() as i64 > QUEUE / 2
        || (bulk && status.queued_send_bytes() >= BULK_QUEUE_DELAY_US)
    {
        return Ok(false);
    }
    match connection.send_message(bytes, SendFlags::RELIABLE) {
        Ok(_) => Ok(true),
        Err(SteamError::LimitExceeded) => Ok(false),
        Err(error) => Err(error.into()),
    }
}
fn send(client: &Runtime, connection: &NetConnection, packet: &Packet) -> NetResult<bool> {
    send_queued(client, connection, &packet.encode()?, false)
}
fn close(connection:NetConnection,reason:&str){
    let detail:String=reason.chars().filter(|c|*c!='\0').take(200).collect();
    connection.close(NetConnectionEnd::App(AppNetConnectionEnd::generic_normal()),Some(&detail),false);
}
fn terminal(state:State)->bool{matches!(state,State::ClosedByPeer|State::ProblemDetectedLocally)}

fn dial_ip(sdk: &Runtime, address: SocketAddr, waited: Duration) -> NetResult<Option<NetConnection>> {
    let networking = sockets(sdk);
    let auth = networking.get_authentication_status();
    // A warm host can already present its Steam certificate while a newly
    // launched client is still loading the certificates it needs to verify it.
    if auth != Ok(NetworkingAvailability::Current) && waited < IP_CERT_WAIT {
        return Ok(None);
    }
    eprintln!("[network] direct UDP connecting to {address} authentication={auth:?}");
    Ok(Some(networking.connect_by_ip_address(address, options())?))
}

pub struct SteamHost {
    sdk:Runtime,listener:ListenSocket,session:HostSession,
    connections:BTreeMap<PlayerId,NetConnection>,
    control:RefCell<BTreeMap<PlayerId,VecDeque<Packet>>>,
    frames: BTreeMap<PlayerId, FrameBacklog>,
}

struct FrameBacklog {
    frames: VecDeque<Arc<[u8]>>,
    bytes: usize,
    since: Instant,
}

impl FrameBacklog {
    fn new(now: Instant) -> Self {
        Self { frames: VecDeque::new(), bytes: 0, since: now }
    }

    fn push(&mut self, frame: Arc<[u8]>, now: Instant) -> NetResult<()> {
        if self.bytes + frame.len() > FRAME_BACKLOG_BYTES || now.duration_since(self.since) >= FRAME_BACKLOG_TIME {
            return Err(ProtocolError("slow peer: realtime queue exceeded").into());
        }
        self.bytes += frame.len();
        self.frames.push_back(frame);
        Ok(())
    }

    fn flush(&mut self, connection: &NetConnection) -> NetResult<()> {
        for _ in 0..32 {
            let Some(frame) = self.frames.front() else { break };
            match connection.send_message(frame, SendFlags::RELIABLE) {
                Ok(_) => {
                    self.bytes -= frame.len();
                    self.frames.pop_front();
                }
                Err(SteamError::LimitExceeded) => break,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

impl SteamHost {
    fn new(listener: ListenSocket, sdk: Runtime, world: WorldIdentity) -> Self {
        Self { sdk, listener, session: HostSession::new(world), connections: BTreeMap::new(),
            control: RefCell::new(BTreeMap::new()), frames: BTreeMap::new() }
    }
    pub fn bind(address:SocketAddr,world:WorldIdentity)->NetResult<Self>{
        let sdk=runtime()?;let listener=sockets(&sdk).create_listen_socket_ip(address,options())?;
        eprintln!("[network] direct UDP listening at {address}");
        Ok(Self::new(listener,sdk,world))
    }
    pub fn bind_p2p(world:WorldIdentity)->NetResult<Self>{
        let sdk=runtime()?;sdk.networking_utils().init_relay_network_access();
        let listener=sockets(&sdk).create_listen_socket_p2p(VIRTUAL_PORT,options())?;
        eprintln!("[steamworks] listening steam:{} virtual_port={VIRTUAL_PORT}",identity(&sdk));
        Ok(Self::new(listener,sdk,world))
    }
    fn remove(&mut self,id:PlayerId,reason:&str){
        self.frames.remove(&id);
        self.control.get_mut().remove(&id);self.session.disconnect(id);
        if let Some(connection)=self.connections.remove(&id){close(connection,reason);}
        eprintln!("player {id} disconnected: {reason}");
    }
    pub fn set_capacity(&mut self,n:usize){self.session.set_capacity(n)}
    pub fn set_world_kind(&mut self,k:u8)->NetResult<()>{Ok(self.session.set_world_kind(k)?)}
    pub fn set_access(&mut self,a:AccessList,w:bool){self.session.set_access(a,w)}
    pub fn access(&self)->Option<&AccessList>{self.session.access()}
    pub fn access_mut(&mut self)->Option<&mut AccessList>{self.session.access_mut()}
    pub fn fingerprint(&self,id:PlayerId)->Option<&str>{self.session.fingerprint(id)}
    pub fn holds_op(&self,id:PlayerId)->bool{self.session.holds_op(id)}
    pub fn roster(&self)->Vec<(PlayerId,String)>{self.session.roster()}
    pub fn rename(&mut self,id:PlayerId,name:&str)->NetResult<String>{Ok(self.session.rename(id,name)?)}
    pub fn take_commands(&mut self)->Vec<(PlayerId,String)>{self.session.take_commands()}
    pub fn players(&self)->Vec<PlayerId>{self.connections.keys().copied().filter(|id|self.session.is_joined(*id)).collect()}
    pub fn disconnect(&mut self,id:PlayerId){self.remove(id,"application closed session")}
    pub fn send_data(&self,id:PlayerId,bytes:Vec<u8>)->NetResult<bool>{self.send_packet(id,&Packet::Data(bytes))}
    pub fn send_bulk(&self, id: PlayerId, bytes: Vec<u8>) -> NetResult<bool> {
        let connection = self.connections.get(&id).ok_or(ProtocolError("unknown player"))?;
        if self.frames.contains_key(&id) || self.control.borrow().get(&id).is_some_and(|queue| !queue.is_empty()) {
            return Ok(false);
        }
        send_queued(&self.sdk, connection, &Packet::Data(bytes).encode()?, true)
    }
    pub fn send_pose(&self, id: PlayerId, bytes: Vec<u8>) -> NetResult<()> {
        if self.frames.contains_key(&id) { return Ok(()); }
        let connection = self.connections.get(&id).ok_or(ProtocolError("unknown player"))?;
        let bytes = Packet::Data(bytes).encode()?;
        let _ = connection.send_message(&bytes, SendFlags::UNRELIABLE_NO_DELAY);
        Ok(())
    }
    pub fn send_packet(&self,id:PlayerId,packet:&Packet)->NetResult<bool>{
        let connection=self.connections.get(&id).ok_or(ProtocolError("unknown player"))?;
        let pending=self.frames.contains_key(&id) || self.control.borrow().get(&id).is_some_and(|q|!q.is_empty());
        if !pending&&send(&self.sdk,connection,packet)? {return Ok(true);}
        if matches!(packet,Packet::Data(_)){return Ok(false);}
        let mut queues=self.control.borrow_mut();let queue=queues.entry(id).or_default();
        if queue.len()>=64{return Err(ProtocolError("control queue exceeded").into());}
        queue.push_back(packet.clone());Ok(true)
    }
    pub fn poll(&mut self,now:Instant)->NetResult<Vec<(PlayerId,Vec<u8>)>>{
        pump(&self.sdk)?;
        // ListenSocket::events() blocks waiting for another event. A game pump
        // must drain only callbacks already available, including on idle hosts.
        for event in std::iter::from_fn(||self.listener.try_receive_event()).take(BATCH).collect::<Vec<_>>(){match event {
            ListenSocketEvent::Connecting(request)=>{
                if self.connections.len()>=MAX_PLAYERS {request.reject(NetConnectionEnd::App(AppNetConnectionEnd::generic_exception()),Some("server full"));}
                else {request.accept()?;}
            }
            ListenSocketEvent::Connected(event)=>{
                let connection=event.take_connection();
                match self.session.connect(now){Ok(id)=>{connection.set_connection_user_data(i64::from(id))?;self.connections.insert(id,connection);},Err(e)=>close(connection,&e.to_string())}
            }
            ListenSocketEvent::Disconnected(_)=>{}
        }}
        let mut application=Vec::new();let mut failed=Vec::new();let mut remaining=BATCH;
        for (&id,connection) in &mut self.connections {
            if connection.info().ok().and_then(|i|i.state().ok()).is_none_or(terminal){failed.push((id,"Steam connection closed".to_string()));continue;}
            if remaining==0{break;}
            let messages = match connection.receive_messages(remaining.min(BATCH/MAX_PLAYERS)) {
                Ok(messages) => messages,
                Err(error) => { failed.push((id, error.to_string())); continue; }
            };
            remaining -= messages.len();
            for message in messages {
                let result=(||->NetResult<()> {
                    let packet=Packet::decode(message.data())?;
                    let data=if let Packet::Data(bytes)=&packet{Some(bytes.clone())}else{None};
                    if let Some(reply)=self.session.receive(id,packet,now)?{
                        if !send(&self.sdk,connection,&reply)?{return Err(ProtocolError("handshake queue full").into());}
                    }else if let Some(bytes)=data{application.push((id,bytes));}Ok(())
                })();
                if let Err(e)=result{failed.push((id,e.to_string()));break;}
            }
        }
        failed.extend(self.session.expired(now).into_iter().map(|id|(id,"session timeout".into())));
        for (id,reason) in failed{self.remove(id,&reason);}
        let mut failed = Vec::new();
        for (&id,connection) in &self.connections {
            if let Some(queue) = self.frames.get_mut(&id) {
                if let Err(error) = queue.flush(connection) {
                    failed.push((id, error.to_string()));
                    continue;
                }
                if !queue.frames.is_empty() { continue; }
                self.frames.remove(&id);
            }
            for _ in 0..8 {
                let Some(packet)=self.control.get_mut().get_mut(&id).and_then(|q|q.pop_front())else{break};
                match send(&self.sdk, connection, &packet) {
                    Ok(true) => {}
                    Ok(false) => { self.control.get_mut().get_mut(&id).unwrap().push_front(packet); break; }
                    Err(error) => { failed.push((id, error.to_string())); break; }
                }
            }
        }
        for (id, reason) in failed { self.remove(id, &reason); }
        Ok(application)
    }
    pub fn advance(&mut self) -> NetResult<Packet> {
        let frame = self.session.advance()?;
        let bytes = frame.encode()?;
        let mut shared: Option<Arc<[u8]>> = None;
        let mut failed = Vec::new();
        let now = Instant::now();
        for (&id, connection) in &self.connections {
            if !self.session.is_joined(id) { continue; }
            if !self.frames.contains_key(&id) {
                match connection.send_message(&bytes, SendFlags::RELIABLE) {
                    Ok(_) => continue,
                    Err(SteamError::LimitExceeded) => eprintln!("[network] buffering ordered frames for player {id}"),
                    Err(error) => { failed.push((id, error.to_string())); continue; }
                }
            }
            // Preserve every ordered frame while the SDK recovers from a short stall.
            let pending = self.frames.entry(id).or_insert_with(|| FrameBacklog::new(now));
            let bytes = shared.get_or_insert_with(|| Arc::from(bytes.as_slice())).clone();
            if let Err(error) = pending.push(bytes, now) { failed.push((id, error.to_string())); }
        }
        for (id, reason) in failed { self.remove(id, &reason); }
        Ok(frame)
    }
}

enum ClientSocket { WaitingForIp(SocketAddr), Live(NetConnection) }

pub struct SteamClient {
    sdk:Runtime,connection:ClientSocket,world:WorldIdentity,started:Instant,last_frame:Instant,
    hello_sent:bool,player:Option<PlayerId>,tick:u64,sequence:u64,last_sent_tick:u64,
    challenge:Option<[u8;32]>,introduced:bool,
}
impl SteamClient {
    fn new(sdk:Runtime,connection:ClientSocket,world:WorldIdentity,now:Instant)->Self{Self{sdk,connection,world,started:now,last_frame:now,hello_sent:false,player:None,tick:0,sequence:0,last_sent_tick:0,challenge:None,introduced:false}}
    pub fn connect(address:SocketAddr,world:WorldIdentity,now:Instant)->NetResult<Self>{
        Ok(Self::new(runtime()?, ClientSocket::WaitingForIp(address), world, now))
    }
    pub fn connect_p2p(id:u64,world:WorldIdentity,now:Instant)->NetResult<Self>{
        let sdk=runtime()?;
        sdk.networking_utils().init_relay_network_access();
        let connection=sdk.networking_sockets().connect_p2p(NetworkingIdentity::new_steam_id(SteamId::from_raw(id)),VIRTUAL_PORT,options())?;
        Ok(Self::new(sdk,ClientSocket::Live(connection),world,now))
    }
    fn describe_connection(sdk:Runtime,mut connection:NetConnection,content:[u8;32])->NetResult<(WorldIdentity,u8)>{
        let result=(||{let started=Instant::now();let mut sent=false;
            while started.elapsed()<Duration::from_secs(30){
                pump(&sdk)?;
                let info=connection.info()?;let state=info.state()?;
                if terminal(state){return Err(format!("Steam discovery connection closed: {info:?}").into());}
                if state==State::Connected&&!sent{sent=send(&sdk,&connection,&Packet::Describe)?;}
                for message in connection.receive_messages(8)? {
                    match Packet::decode(message.data())?{
                        Packet::World{world,kind} if world.content==content=>return Ok((world,kind)),
                        Packet::World{..}=>return Err(ProtocolError("host uses a different game build").into()),
                        _=>return Err(ProtocolError("unexpected discovery packet").into()),
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }Err(ProtocolError("Steam world discovery timed out").into())
        })();close(connection,"world discovery complete");result
    }
    pub fn describe(address:SocketAddr,content:[u8;32])->NetResult<(WorldIdentity,u8)>{
        let sdk=runtime()?;
        let started = Instant::now();
        let connection = loop {
            pump(&sdk)?;
            if let Some(connection) = dial_ip(&sdk, address, started.elapsed())? { break connection; }
            std::thread::sleep(Duration::from_millis(25));
        };
        Self::describe_connection(sdk,connection,content)
    }
    pub fn describe_p2p(id:u64,content:[u8;32])->NetResult<(WorldIdentity,u8)>{
        let sdk=runtime()?;
        sdk.networking_utils().init_relay_network_access();
        let connection=sdk.networking_sockets().connect_p2p(NetworkingIdentity::new_steam_id(SteamId::from_raw(id)),VIRTUAL_PORT,options())?;
        Self::describe_connection(sdk,connection,content)
    }
    pub fn can_introduce(&self)->bool{self.challenge.is_some()&&!self.introduced}
    fn socket(&self) -> NetResult<&NetConnection> {
        match &self.connection {
            ClientSocket::Live(connection) => Ok(connection),
            ClientSocket::WaitingForIp(_) => Err(ProtocolError("session not ready").into()),
        }
    }
    pub fn introduce(&mut self,name:&str,key:&PlayerKey)->NetResult<()> {
        let challenge=self.challenge.ok_or(ProtocolError("no welcome yet"))?;
        if self.introduced{return Err(ProtocolError("already introduced").into());}
        if !send(&self.sdk,self.socket()?,&Packet::Introduce{name:name.into(),key:key.public(),signature:key.sign_introduce(&challenge,&self.world)})?{return Err(ProtocolError("introduction queue full").into());}
        self.introduced=true;Ok(())
    }
    pub fn player(&self)->Option<PlayerId>{self.player}
    pub fn tick(&self)->u64{self.tick}
    pub fn poll(&mut self,now:Instant)->NetResult<Vec<Packet>>{
        pump(&self.sdk)?;
        if let ClientSocket::WaitingForIp(address) = self.connection {
            let Some(connection) = dial_ip(&self.sdk, address, now.saturating_duration_since(self.started))? else {
                return Ok(Vec::new());
            };
            self.connection = ClientSocket::Live(connection);
            self.started = now;
            self.last_frame = now;
        }
        let ClientSocket::Live(connection) = &mut self.connection else { unreachable!() };
        let info = connection.info()?;
        let state = info.state()?;
        if terminal(state){return Err(format!("Steam session closed: {info:?}").into());}
        if state==State::Connected&&!self.hello_sent{self.hello_sent=send(&self.sdk,connection,&Packet::Hello(self.world))?;}
        let mut packets=Vec::new();
        for message in connection.receive_messages(BATCH)?{
            match Packet::decode(message.data())?{
                Packet::Welcome{player,tick,challenge} if self.hello_sent&&self.player.is_none()=>{self.player=Some(player);self.tick=tick;self.challenge=Some(challenge);self.last_frame=now;}
                packet@(Packet::Data(_)|Packet::Roster{..}|Packet::Chat{..}) if self.player.is_some()=>{self.last_frame=now;packets.push(packet);}
                frame@Packet::Frame{..} if self.player.is_some()=>{
                    let Packet::Frame{tick,ref players}=frame else{unreachable!()};
                    if Some(tick)!=self.tick.checked_add(1)||!players.iter().any(|(id,_)|Some(*id)==self.player){return Err(ProtocolError("invalid host frame").into());}
                    self.tick=tick;self.last_frame=now;packets.push(frame);
                }
                _=>return Err(ProtocolError("unexpected Steam server packet").into()),
            }
        }
        // SDR rendezvous can take longer than an already-connected keyed handshake.
        if (self.player.is_none()&&now.duration_since(self.started)>Duration::from_secs(30))||now.duration_since(self.last_frame)>IDLE_TIMEOUT.max(Duration::from_secs(30)){return Err(ProtocolError("Steam server timeout").into());}
        Ok(packets)
    }
    pub fn submit(&mut self,movement:[i16;2],look:[i16;2],buttons:u16)->NetResult<()> {
        let tick=self.tick.checked_add(INPUT_LEAD).ok_or(ProtocolError("tick exhausted"))?;
        if tick<=self.last_sent_tick{return Ok(());}
        let sequence=self.sequence.checked_add(1).ok_or(ProtocolError("sequence exhausted"))?;
        if send(&self.sdk,self.socket()?,&Packet::Input(Input{sequence,tick,movement,look,buttons}))?{self.sequence=sequence;self.last_sent_tick=tick;}Ok(())
    }
    pub fn send_data(&self,bytes:Vec<u8>)->NetResult<bool>{if self.player.is_none(){return Err(ProtocolError("session not ready").into());}send(&self.sdk,self.socket()?,&Packet::Data(bytes))}
}
