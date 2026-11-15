//! Host-authoritative shared building sessions. Survival systems are not run here.
//! Local and remote movement and edits use the same authority path.
use crate::{
    fluid::VoxelScalePreset,
    player::{Input, Player, update_player},
    world::{self, BlockId, DayNightCycle, World, WorldType},
};
use anyhow::{Context, Result, ensure};
pub mod inventory;
pub mod text;
mod travel_review;
pub(crate) mod gameplay;
pub mod mining;
mod actions;
#[cfg(test)]
pub(crate) use actions::admit_sequence;
mod authority;
pub(crate) mod edit_admission_review;
mod audio_events;
mod combat;
mod link;
mod server_files;
mod host_commands;
pub use crate::social;
pub mod coop;
mod bot;
mod rendered_review;
mod base_review;
mod realm_review;
mod open;
mod profiles;
mod pacing;
mod deltas;
mod replica;
mod coherence;
mod realm;
pub(crate) use realm::Plan as RealmPlan;
mod persist;
mod smoothing;
mod replication;
mod status;
#[cfg(test)]
mod owner_tests;
#[cfg(test)]
mod survival_tests;
#[cfg(test)]
mod edit_history_tests;

use link::{Client, Host, HostSetup, Target};
use engine_netcode::quality::{EchoProbe, Quality};
use engine_netcode::scoreboard::{self, BoardWriter, Change, Objective, Order, Slot, Unit};
use host_commands::{Deferred, HOST};

/// The host's own player id, for the HUD.
pub const HOST_ID: PlayerId = HOST;
use engine_net::{
    protocol::{Packet, PlayerId, WorldIdentity},
    transfer::{Receiver, Sender},
};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    io::Read,
    net::SocketAddr,
    time::{Duration, Instant},
};

const STEP: f32 = 1.0 / 120.0;
/// Leave room for movement while a burst of inventory or station actions uploads.
const COMMAND_FRAGMENTS_PER_TICK: usize = 4;
const MAX_OUTSTANDING_COMMANDS: usize = 128;
/// Pending cells per peer. A hosted single-player world settles water and sand across its
/// whole view while a joiner loads, so this holds a few hundred thousand.
const MAX_EDITS: usize = 262_144;
const MAX_CHUNKS: usize = 4096;
const SYNC_TIMEOUT: Duration = Duration::from_secs(90);
/// A joined peer that never names a profile would hold a slot forever: session
/// input keepalives refresh its idle timer without it ever becoming a player.
const PROFILE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Controls {
    slot:u8,
    cast:bool,cycle:bool,dash:bool,interact:bool,alt:bool,ctrl:bool,
    seq: u64,
    yaw: f32,
    #[serde(default)]
    gravity_frame: glam::Quat,
    pitch: f32,
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    jump: bool,
    sprint: bool,
    sneak: bool,
    flying: bool,
    mine: bool,
    place: bool,
    use_secondary: bool,
    block: u32,
    /// The joiner's own feet position and velocity in voxels. Joiners move
    /// themselves, so input lag never reaches their own body; the host checks it.
    body: Option<([f32; 3], [f32; 3])>,
    /// Newest position correction the joiner has applied.
    ack: u16,
    /// Fall and hazard damage the joiner's own movement step produced.
    damage: f32,
}
impl Controls {
    fn capture(seq: u64, player: &Player, input: &Input) -> Self {
        Self {
            seq,
            slot:0,cast:input.cast_spell||input.cast_pressed,cycle:input.cycle_spell,
            dash:input.dash||input.dash_pressed,interact:input.interact,alt:input.alt_down,ctrl:input.ctrl_down,
            yaw: player.yaw,
            gravity_frame: player.gravity_frame,
            pitch: player.pitch,
            forward: input.forward,
            back: input.back,
            left: input.left,
            right: input.right,
            jump: input.jump,
            sprint: input.sprint,
            sneak: input.sneak,
            flying: player.flying,
            mine: input.break_block || input.break_pressed,
            place: input.place_block || input.place_pressed,
            use_secondary: input.place_pressed,
            block: player.selected_block.as_u32(),
            body: None,
            ack: 0,
            damage: 0.0,
        }
    }
    fn input(&self) -> Input {
        let mut input = Input::default();
        input.forward = self.forward;
        input.back = self.back;
        input.left = self.left;
        input.right = self.right;
        input.jump = self.jump;
        input.sprint = self.sprint;
        input.sneak = self.sneak;
        input.cast_spell=self.cast;input.cast_pressed=self.cast;input.cycle_spell=self.cycle;
        input.dash=self.dash;input.dash_pressed=self.dash;input.interact=self.interact;
        input.alt_down=self.alt;input.ctrl_down=self.ctrl;
        input
    }
    fn validate(&self) -> Result<()> {
        ensure!(self.gravity_frame.is_finite() && (self.gravity_frame.length_squared() - 1.0).abs() < 0.001, "invalid upright frame");
        ensure!(
            self.yaw.is_finite() && self.pitch.is_finite() && self.seq > 0,
            "invalid movement command"
        );
        ensure!(self.slot<9 && self.block < BlockId::COUNT as u32 && self.yaw.abs()<1.0e6 && self.pitch.abs()<=1.55, "invalid controls");
        ensure!(self.damage.is_finite() && (0.0..=1000.0).contains(&self.damage), "invalid reported damage");
        if let Some((position, velocity)) = self.body {
            ensure!(position.iter().chain(&velocity).all(|v| v.is_finite() && v.abs() <= 1_000_000.0), "invalid body");
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pose {
    id: PlayerId,
    position: [f32; 3],
    velocity: [f32; 3],
    yaw: f32,
    #[serde(default)]
    gravity_frame: glam::Quat,
    pitch: f32,
    flying: bool,
    #[serde(default, skip_serializing_if = "zero_swing")]
    swing: f32,
    #[serde(default = "default_swing_duration", skip_serializing_if = "standard_swing_duration")]
    swing_duration: f32,
    /// Sender's clock in seconds; 0 for poses read locally on the host.
    #[serde(default, skip_serializing_if = "unstamped")]
    sent: f64,
    /// The host's traversal record changes at a committed portal crossing.
    #[serde(default)]
    crossing: [u64; 2],
}
fn unstamped(value: &f64) -> bool { *value == 0.0 }
fn default_swing_duration() -> f32 { 0.30 }
fn zero_swing(value: &f32) -> bool { *value == 0.0 }
fn standard_swing_duration(value: &f32) -> bool { *value == default_swing_duration() }
impl Pose {
    fn with_combat(mut self, adventure: &world::adventure::Adventure) -> Self {
        self.swing = adventure.swing();
        self.swing_duration = adventure.swing_duration();
        self
    }
    fn from_player(id: PlayerId, player: &Player) -> Self {
        Self {
            id,
            position: player.pos.to_array(),
            velocity: player.vel.to_array(),
            yaw: player.yaw,
            gravity_frame: player.gravity_frame,
            pitch: player.pitch,
            flying: player.flying,
            swing: 0.0,
            swing_duration: default_swing_duration(),
            sent: 0.0,
            crossing: [player.map_crossings.epoch, player.map_crossings.sequence()],
        }
    }
    fn validate(&self) -> Result<()> {
        ensure!(self.gravity_frame.is_finite() && (self.gravity_frame.length_squared() - 1.0).abs() < 0.001, "invalid upright frame");
        ensure!((0.0..=1.0).contains(&self.swing)
            && (0.05..=4.0).contains(&self.swing_duration), "invalid combat pose");
        ensure!(self.sent.is_finite() && self.sent >= 0.0, "invalid pose stamp");
        ensure!(
            self.position
                .iter()
                .chain(self.velocity.iter())
                .chain([&self.yaw, &self.pitch])
                .all(|v| v.is_finite()),
            "invalid player pose"
        );
        ensure!(
            Vec3::from_array(self.position).abs().max_element() <= 1_000_000.0,
            "player outside world range"
        );
        Ok(())
    }
    fn apply(&self, player: &mut Player) {
        player.pos = Vec3::from_array(self.position);
        player.precise_pos = player.pos.as_dvec3();
        player.vel = Vec3::from_array(self.velocity);
        player.yaw = self.yaw;
        player.gravity_frame = self.gravity_frame;
        player.pitch = self.pitch;
        player.flying = self.flying;
    }

    fn sample(&self) -> smoothing::Sample {
        smoothing::Sample {
            position: Vec3::from_array(self.position),
            yaw: self.yaw,
            pitch: self.pitch,
            gravity_frame: self.gravity_frame,
            flying: self.flying,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Chunk {
    coord: [i32; 3],
    runs: Vec<(u16, u32)>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    realm: Option<(u8, u8, u32, u32)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hell_gates: Vec<world::hell_gate::built::Threshold>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    deferred_terrain: Vec<[i32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    planet_settings: Option<engine_voxel::PlanetSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terrain_checksum: Option<u64>,
    #[serde(default)]
    gameplay: Option<Vec<u8>>,
    spawn: Pose,
    chunks: Vec<Chunk>,
    time: f32,
    day: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingSave {
    #[serde(default)]
    profile_checksum:u64,
    #[serde(default)]
    profiles: BTreeMap<String,Vec<u8>>,
    format: u32,
    seed: u32,
    generator: u32,
    cell_mm: u16,
    checksum: u64,
    snapshot: Snapshot,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Delta {
    cells: Vec<([i32; 3], u32)>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Roster {
    ids: Vec<PlayerId>,
    time: f32,
    day: u32,
}
#[derive(Default)]
struct ActionPulses {
    cast: bool,
    dash: bool,
    mine: bool,
    place: bool,
}
impl ActionPulses {
    fn record(&mut self, controls: &Controls) {
        self.cast |= controls.cast;
        self.dash |= controls.dash;
        self.mine |= controls.mine;
        self.place |= controls.place;
    }

    fn apply(self, controls: &mut Controls) {
        controls.cast |= self.cast;
        controls.dash |= self.dash;
        controls.mine |= self.mine;
        controls.place |= self.place;
    }
}
struct Actor {
    profile:String,
    personal:gameplay::Personal,
    tools:actions::Tools,
    command_sequence:u64,
    request_rx:Receiver,
    player: Player,
    controls: Controls,
    pulses: ActionPulses,
    last_input: Instant,
    last_sequence: u64,
    /// Counts portal moves accepted for the joiner's predicted body.
    portal_crossings: u64,
    edit_cooldown: f32,
    mining: world::mining::Progress,
    spell_edits: crate::creative::CreativeState,
    edits: authority::Pending,
    history: world::edit_history::EditHistory,
    creative: bool,
    /// Ignore the joiner's body reports until it echoes this correction.
    correction: u16,
    awaiting: bool,
    correction_pending: bool,
    /// Loaded from `profiles`, so it must be written back even if the join fails.
    restored: bool,
}
impl Actor {
    fn new(player: Player, now: Instant) -> Self {
        Self {
            profile:String::new(),personal:Default::default(),tools:Default::default(),command_sequence:0,request_rx:Receiver::default(),
            player,
            controls: Controls::default(),
            pulses: ActionPulses::default(),
            last_input: now,
            last_sequence: 0,
            portal_crossings: 0,
            edit_cooldown: 0.0,
            mining: Default::default(),
            spell_edits: Default::default(),
            edits: Default::default(),
            history: world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS),
            restored: false,
            creative: false,
            correction: 0,
            awaiting: false,
            correction_pending: false,
        }
    }

    fn queue_correction(&mut self) {
        if !self.correction_pending {
            self.correction = self.correction.wrapping_add(1);
        }
        self.awaiting = true;
        self.correction_pending = true;
    }

    fn flush_correction(&mut self, socket: &Host, id: PlayerId) -> Result<bool> {
        if !self.correction_pending {
            return Ok(true);
        }
        let body = &self.player;
        let packet = encode(17, &(self.correction, body.pos.to_array(), body.vel.to_array(), body.gravity_frame))?;
        let sent = socket.send_data(id, packet).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if sent {
            self.correction_pending = false;
            eprintln!("[multiplayer] correction player={id} n={} at={:?}", self.correction, body.pos);
        }
        Ok(sent)
    }

    fn pose(&self, id: PlayerId) -> Pose {
        let mut pose = Pose::from_player(id, &self.player).with_combat(&self.personal.adventure);
        pose.crossing[1] = self.portal_crossings;
        pose
    }
}
struct Output {
    contacts: std::collections::VecDeque<audio_events::ContactEvent>,
    sounds: std::collections::VecDeque<crate::audio::Event>,
    transfer: Option<Sender>,
    terrain: realm::Terrain,
    cells: BTreeMap<[i32; 3], u32>,
    coherence: coherence::HostGate,
    /// Byte budget shared by coherent images and the initial world transfer.
    pace: pacing::Pacer,
    ready: bool,
    started: Instant,
    image:Option<Sender>,
    image_due:Instant,
    /// Last encoded sections, including the frame awaiting publication.
    mirror: replication::Mirror,
    refreshed: Instant,
    water_resent: Instant,
    /// Ping echo over the reliable stream (tags 19 and 20).
    probe: EchoProbe,
}
struct Remote {
    pose: Pose,
    shown: Vec3,
    shown_yaw: f32,
    shown_pitch: f32,
    shown_frame: glam::Quat,
    shown_flying: bool,
    phase: f32,
    stride: f32,
    playback: engine_netcode::interp::SnapshotBuffer<smoothing::Sample>,
}
impl Remote {
    fn new(pose: Pose) -> Self {
        let sample = pose.sample();
        let mut playback = smoothing::buffer();
        if pose.sent > 0.0 {
            playback.push(smoothing::clock_s(), pose.sent, sample);
        }
        Self {
            pose, playback, shown: sample.position, shown_yaw: sample.yaw,
            shown_pitch: sample.pitch, shown_frame: sample.gravity_frame,
            shown_flying: sample.flying, phase: 0.0, stride: 0.0,
        }
    }

    fn receive(&mut self, pose: Pose, metres_per_cell: f32) {
        if pose.sent > 0.0 && pose.sent <= self.pose.sent {
            return;
        }
        let sample = pose.sample();
        let elapsed = if self.pose.sent > 0.0 {
            (pose.sent - self.pose.sent).clamp(0.0, 0.25) as f32
        } else {
            1.0 / 30.0
        };
        let speed = Vec3::from_array(self.pose.velocity).length()
            .max(Vec3::from_array(pose.velocity).length()) * metres_per_cell;
        let displacement = sample.position.distance(Vec3::from_array(self.pose.position)) * metres_per_cell;
        // The explicit receipt catches even a short portal. The distance
        // guard also handles respawns and other abrupt relocations.
        if pose.crossing != self.pose.crossing || displacement > 8.0 + speed * elapsed {
            self.playback.clear_samples();
            self.show(sample);
            self.phase = 0.0;
            self.stride = 0.0;
        }
        if pose.sent > 0.0 {
            self.playback.push(smoothing::clock_s(), pose.sent, sample);
        }
        self.pose = pose;
    }

    fn show(&mut self, sample: smoothing::Sample) {
        self.shown = sample.position;
        self.shown_yaw = sample.yaw;
        self.shown_pitch = sample.pitch;
        self.shown_frame = sample.gravity_frame;
        self.shown_flying = sample.flying;
    }
}
enum Role {
    Closed,
    Pending {
        host: bool,
        target: Target,
        setup: HostSetup,
        identity: WorldIdentity,
    },
    Host {
        socket: Host,
    },
    Client {
        socket: Client,
        receiver: Receiver,
        sequence: u64,
        ready: bool,
        started: Instant,
    },
}
pub struct Session {
    authority: realm::Authority,
    realm_turn: usize,
    portal_warmup: Option<crate::render::PortalWarmup>,
    realms: BTreeMap<String, Box<realm::Resident>>,
    crossings: BTreeMap<PlayerId, bool>,
    realm_loads: realm::RealmLoads,
    realm_joins: BTreeMap<PlayerId, realm::PendingJoin>,
    crossing_requested: bool,
    incoming_realm: Option<realm::Incoming>,
    mining_feedback: world::mining::Progress,
    contacts: Vec<audio_events::ContactEvent>,
    profile:String,
    profiles:BTreeMap<String,Vec<u8>>,
    join_profiles:BTreeMap<PlayerId,String>,
    joined_at:BTreeMap<PlayerId,Instant>,
    join_sent:bool,
    commands:std::collections::VecDeque<actions::Command>,
    command_sequence:u64,
    command_upload:Option<Sender>,
    unack:std::collections::VecDeque<(u64,actions::Command)>,
    pending_input:Input,
    hand:usize,
    tools:actions::Tools,
    tools_key:String,
    creative:bool,
    headless: bool,
    role: Role,
    remotes: BTreeMap<PlayerId, Remote>,
    accumulator: f32,
    network_clock: f32,
    pending_mine: bool,
    pending_place: bool,
    failed: bool,
    reset_render: bool,
    save_path: Option<std::path::PathBuf>,
    /// Friends' profiles beside a hosted single-player world, which saves through its own storage.
    sidecar: Option<std::path::PathBuf>,
    /// A departed player's profile changed since the sidecar was last written.
    profiles_dirty: bool,
    saved_revision: u64,
    /// Checksum of the profiles and host body at the last periodic save.
    saved_profiles: u64,
    last_save: Instant,
    pub social: social::Social,
    /// This side's player name; the host's own entry in the player list.
    name: String,
    key: Option<engine_net::identity::PlayerKey>,
    introduced: bool,
    /// Packets received while the handshake was completed ahead of the first frame.
    early: Vec<Packet>,
    /// Lines joiners typed, for the next command pass.
    typed: Vec<(PlayerId, String)>,
    budgets: BTreeMap<PlayerId, engine_net::social::ChatBudget>,
    roster_due: Instant,
    /// Lines a dedicated host's console typed.
    console: Vec<String>,
    stopped: bool,
    /// Newest correction the joiner applied, echoed to the host.
    correction_ack: u16,
    /// Damage from this side's own movement, not yet reported.
    pending_damage: f32,
    /// The renderer is still loading terrain around the joiner, so its body waits.
    hold: bool,
    /// Host scoreboard; flushed to ready joiners as tag 21.
    board: BoardWriter,
    /// Origin for probe timestamps.
    epoch: Instant,
    review: rendered_review::Review,
    /// World rules last sent to joiners (tag 23), so a change is resent once.
    replica_edits: replica::Queue,
}

/// Dedicated building host, or a windowless joiner for testing a server.
/// Networking and authority never wait on a renderer. Type `quit` to flush a
/// final save and close connections normally. Other lines are server commands
/// on a host and chat on a joiner.
pub fn run_headless() -> Result<bool> {
    let args: Vec<_> = std::env::args().collect();
    if !args.iter().any(|a| a == "--multiplayer-headless") { return Ok(false); }
    if args.iter().any(|arg| arg == "--realm-review-sky-and-forest") {
        realm_review::sky_and_forest()?;
        return Ok(true);
    }
    if args.iter().any(|arg| arg == "--multiplayer-realm-fixture") {
        realm_review::fixture()?;
        return Ok(true);
    }
    let hosting = args.iter().any(|a| a.starts_with("--multiplayer-host=") || a.starts_with("--multiplayer-online="));
    let limit = args.iter().find_map(|a| a.strip_prefix("--multiplayer-run-seconds="))
        .map(str::parse::<f32>).transpose()?;
    ensure!(limit.is_none_or(|s| s.is_finite() && s > 0.0), "invalid host duration");
    let Some((mut session, mut world, mut player, mut day)) = Session::start()? else { unreachable!() };
    // An opened single-player world keeps writing its own checkpoints.
    let mut saves = session.sidecar.is_some().then(world::storage::SaveService::new);
    let (line_tx, line_rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line_tx.send(line).is_err() { break; }
        }
    });
    eprintln!("[multiplayer] {}; type quit to stop", if hosting { "dedicated host; lines are server commands such as /list" } else { "windowless joiner; lines are chat" });
    // `--multiplayer-bot-walk`: a joiner that sprints in a slow circle and hops, for server load and movement checks.
    let walk = !hosting && args.iter().any(|a| a == "--multiplayer-bot-walk");
    let duel = !hosting && args.iter().any(|a| a == "--multiplayer-bot-pvp");
    let respawn = !hosting && args.iter().any(|a| a == "--multiplayer-bot-respawn");
    let mut respawn_queued = false;
    let mut work = (!hosting && args.iter().any(|a| a == "--multiplayer-bot-work")).then(bot::Bot::default);
    let mut travel = (!hosting && args.iter().any(|a| a == "--multiplayer-bot-travel")).then(travel_review::Bot::default);
    let slot_review = !hosting && args.iter().any(|a| a == "--multiplayer-bot-slots");
    let mut slot_phase = 0;
    let trace = args.iter().any(|a| a == "--multiplayer-trace");
    let review = args.iter().any(|a| a == "--multiplayer-review-bases");
    let review_realms = args.iter().any(|a| a == "--multiplayer-review-realms");
    let mut next_trace = 0.0;
    let mut update_times = Vec::<f32>::with_capacity(256);
    let mut audio = (!hosting && args.iter().any(|a| a == "--multiplayer-audio")).then(crate::audio::Audio::new);
    if let Some(audio) = &audio { audio.set_active(true); }
    let began = Instant::now(); let mut last = began;
    let mut blocks = HashSet::new(); let mut chunks = HashSet::new();
    loop {
        let now = Instant::now(); let dt = (now-last).as_secs_f32(); last=now;
        let mut input = Input::default();
        if respawn && session.ready() {
            if world.adventure.is_dead() && !respawn_queued {
                eprintln!("[network] requesting respawn");
                let _ = session.route_ui(crate::egui_ui::UiAction::Respawn);
                respawn_queued = true;
            } else if !world.adventure.is_dead() { respawn_queued = false; }
        }
        if walk && session.ready() {
            let t = began.elapsed().as_secs_f32();
            input.forward = true;
            input.sprint = true;
            input.jump = t.fract() < 0.1;
            player.yaw = (player.yaw + dt * 0.4).rem_euclid(std::f32::consts::TAU);
        }
        if let Some(bot) = work.as_mut() { input = bot.input(&mut session, &world, &mut player); }
        if let Some(bot) = travel.as_mut() { input = bot.input(&mut session, &world, &mut player); }
        if slot_review && session.ready() {
            bot::chest_slots(&mut session, &world, &player, &mut slot_phase);
        }
        if duel && session.ready() && began.elapsed().as_secs_f32()>3.0
            && let Some(remote)=session.remotes.values().next() {
            let target=Vec3::from_array(remote.pose.position)+Vec3::Y*(0.9/world.voxel_scale.meters());
            let aim=(target-player.eye(world.voxel_scale)).normalize_or_zero();
            player.yaw=aim.x.atan2(-aim.z);
            player.pitch=aim.y.asin().clamp(-1.5,1.5);
            input.break_block=true;
            input.forward=(target-player.eye(world.voxel_scale)).length()*world.voxel_scale.meters()>1.5;
        }
        let update_started = Instant::now();
        session.update(&mut world,&mut player,&mut day,&input,dt,&mut blocks,&mut chunks);
        if trace { update_times.push(update_started.elapsed().as_secs_f32() * 1000.0); }
        if walk && (began.elapsed().as_secs_f32() % 5.0) < dt {
            let others: Vec<_> = session.remotes.iter().map(|(id, r)| (*id, r.pose.position.map(|v| v.round()))).collect();
            eprintln!("[bot] at={:?} sees={others:?}", player.pos.round());
        }
        blocks.clear(); chunks.clear();
        if let Some(saves) = saves.as_mut() { saves.tick(Some(&mut world), &player, &day); }
        for text in session.social.take_unseen() { eprintln!("[chat] {text}"); }
        let sounds = std::mem::take(&mut world.adventure.sounds);
        if trace && !sounds.is_empty() { eprintln!("[network] sounds={} device={}", sounds.len(), crate::audio::status()); }
        if let Some(audio) = &mut audio {
            for sound in sounds { crate::soundscape::play_event(audio, &world, sound, player.eye(world.voxel_scale)*world.voxel_scale.meters(), player.forward_dir()); }
            session.play_contacts(audio,&world,&player);
        } else {
            session.contacts.clear();
        }
        if trace && began.elapsed().as_secs_f32() >= next_trace {
            next_trace += 2.0;
            update_times.sort_by(f32::total_cmp);
            let samples = update_times.len();
            if samples > 0 {
                eprintln!("[network-timing] samples={samples} p50_ms={:.2} p95_ms={:.2} max_ms={:.2}",
                    update_times[samples/2], update_times[(samples*95/100).min(samples-1)], update_times[samples-1]);
            }
            update_times.clear();
            if review && session.ready() { base_review::report(&world); }
            if review_realms && session.ready() {
                let stone = world.inventory.count_item(world::inventory::Item::Block(BlockId::Stone));
                eprintln!("[realm-review] realm={:?} stone={stone}", world.world_type);
            }
            eprintln!("[network] ready={} revision={} at={:?} players={} chunks={} health={:.1} inventory={:016x}",
                session.ready(), world.revision(), player.pos, session.others(), world.chunks.len(),
                world.adventure.health, engine_net::transfer::checksum(ron::to_string(&world.inventory)?.as_bytes()));
            if matches!(session.role, Role::Host { .. }) {
                let realm::Authority { actors, outputs, .. } = &session.authority;
                for (&id, actor) in actors {
                    eprintln!("[network] player={id} ready={} at={:?} health={:.1} inventory={:016x} cells_pending={} command_ack={}",
                        outputs.get(&id).is_some_and(|output| output.ready), actor.player.pos,
                        actor.personal.adventure.health,
                        engine_net::transfer::checksum(ron::to_string(&actor.personal.inventory)?.as_bytes()),
                        outputs.get(&id).map_or(0, |output| output.cells.len()), actor.command_sequence);
                }
            }
        }
        if session.stopped() { return Ok(true); }
        if session.failed() { anyhow::bail!("multiplayer session stopped"); }
        let mut quit = false;
        while let Ok(line) = line_rx.try_recv() {
            let line = line.trim();
            if line.eq_ignore_ascii_case("quit") { quit = true; }
            else if line.is_empty() {}
            else if hosting && line.starts_with(engine_net::punch::ANSWER_PREFIX) {
                match session.accept_punch_answer(line) {
                    Ok(()) => eprintln!("[multiplayer] answer accepted"),
                    Err(error) => eprintln!("[multiplayer] {error}"),
                }
            }
            else if review_realms && !hosting && line == "review-gate" {
                ensure!(session.request_gate(), "Guest is not ready");
            }
            else if review && !hosting && world::factory::assembler::review_fixture::network_enabled()
                && matches!(line, "review-assembler-batch" | "review-assembler-start") {
                let at = world::factory::assembler::review_fixture::machine(&world);
                let action = if line == "review-assembler-batch" {
                    world::factory::Action::SetRecipe(at, world::factory::assembler::SINGLE_BATCH)
                } else { world::factory::Action::Toggle(at) };
                ensure!(matches!(session.route_ui(crate::egui_ui::UiAction::Factory(action)), crate::egui_ui::UiAction::None), "assembler client action refused");
            }
            else if review && hosting && line == "review-blast" {
                let cell = base_review::blast_cell(&world);
                let point = (cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
                ensure!(world.adventure.queue_blast(point, 2.6), "review blast queue full");
            }
            else if !hosting { session.social.send(line); }
            // A console line without a slash is a command too, as on a Minecraft server console.
            else { session.console(if line.starts_with('/') { line.to_owned() } else { format!("/{line}") }); }
        }
        if quit || limit.is_some_and(|s| began.elapsed().as_secs_f32()>=s) { break; }
        std::thread::sleep(Duration::from_millis(8));
    }
    // An ephemeral host has no save file, so quitting it discards the world.
    session.close(&mut world, &player, &day)?;
    if let Some(saves) = saves.as_mut() {
        saves.request(true);
        let deadline = Instant::now() + Duration::from_secs(60);
        while saves.busy() && Instant::now() < deadline {
            saves.tick(Some(&mut world), &player, &day);
            std::thread::sleep(Duration::from_millis(10));
        }
        eprintln!("[multiplayer] world saved status={}", saves.status.as_deref().unwrap_or("none"));
    }
    Ok(true)
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
fn direct_address(text: &str) -> Result<Target> {
    if text == "punch" { return Ok(Target::PunchHost { automatic: false }); }
    #[cfg(feature="multiplayer-sdr")]
    {
        if text == "friends" { return Ok(Target::PunchHost { automatic: true }); }
        if text=="steam" {return Ok(Target::SteamSdr(0));}
        if let Some(id)=text.strip_prefix("steam:") {
            let id=id.parse::<u64>().context("Steam address must be steam:SteamID64")?;
            ensure!(id>0,"invalid Steam identity");return Ok(Target::SteamSdr(id));
        }
    }
    Ok(Target::Direct(text.parse().context("multiplayer address must be IP:port")?))
}

#[cfg(any(feature="multiplayer-steam",feature="multiplayer-sdr"))]
fn describe_target(target:&Target)->Result<(WorldIdentity,u8)> {
    let digest=executable_digest()?;
    let result=match target {
        Target::PunchJoin(offer) => {
            ensure!(offer.world.content == digest, "different game build");
            return Ok((offer.world, offer.kind));
        }
        Target::Direct(address)=>engine_net::steam::SteamClient::describe(*address,digest),
        #[cfg(feature="multiplayer-sdr")]
        Target::SteamSdr(id)=>{
            ensure!(*id>0,"join with steam: followed by the host's SteamID64");
            engine_net::steam::SteamClient::describe_p2p(*id,digest)
        }
        _=>anyhow::bail!("a direct or Steam address is required"),
    };
    result.map_err(|error|anyhow::anyhow!(error.to_string()))
}

#[cfg(not(any(feature = "multiplayer-steam", feature = "multiplayer-sdr")))]
fn direct_address(_text: &str) -> Result<Target> {
    anyhow::bail!("direct addresses need a multiplayer-steam build; use --multiplayer-online or a lobby code")
}

#[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
fn direct_invite(code: &str) -> Result<Target> {
    if engine_net::punch::is_offer(code) {
        return Ok(Target::PunchJoin(engine_net::punch::Offer::decode(code)?));
    }
    let address = engine_net::invite::decode(code).context("multiplayer invite code")?;
    Ok(Target::Direct(address.into()))
}

#[cfg(not(any(feature = "multiplayer-steam", feature = "multiplayer-sdr")))]
fn direct_invite(_code: &str) -> Result<Target> {
    anyhow::bail!("invite codes need a multiplayer-steam build; use a lobby code")
}

/// `--multiplayer-browse[=TEXT]`: prints matching lobby sessions, then exits.
pub fn browse_cli() -> Result<bool> {
    let args: Vec<_> = std::env::args().collect();
    if !args.iter().any(|a| a == "--multiplayer-browse" || a.starts_with("--multiplayer-browse=")) {
        return Ok(false);
    }
    let text = args.iter().find_map(|a| a.strip_prefix("--multiplayer-browse=")).unwrap_or("");
    let lobby: SocketAddr = args
        .iter()
        .find_map(|a| a.strip_prefix("--multiplayer-lobby="))
        .unwrap_or(engine_net::lobby::DEFAULT_LOBBY)
        .parse()
        .context("multiplayer lobby must be IP:port")?;
    let query = engine_net::lobby::wire::Query {
        text: text.to_owned(),
        mode: None,
        version: None,
        include_full: true,
        sort: Default::default(),
        offset: 0,
        limit: engine_net::lobby::wire::MAX_PAGE,
    };
    let (total, entries) = engine_net::lobby::relay::browse(lobby, &query)?;
    println!("{total} session(s) listed");
    for entry in entries {
        println!(
            "{:<10} {:>2}/{:<2} {:<9} v{:<4} {:>6} s  {} | {}",
            entry.code,
            entry.players,
            entry.max_players,
            format!("{:?}", entry.mode),
            entry.version,
            entry.age_secs,
            entry.name,
            entry.world
        );
    }
    Ok(true)
}

/// Fingerprint of this executable. Two builds with the same fingerprint share gameplay content and rules.
fn executable_digest() -> Result<[u8; 32]> {
    static DIGEST: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    if let Some(digest) = DIGEST.get() {
        return Ok(*digest);
    }
    let digest = hash_executable()?;
    Ok(*DIGEST.get_or_init(|| digest))
}
fn hash_executable() -> Result<[u8; 32]> {
    #[cfg(feature="multiplayer-sdr")]
    {
        let id=env!("VOXEL_NETWORK_BUILD_ID");let mut digest=[0u8;32];
        for (i,byte) in digest.iter_mut().enumerate(){*byte=u8::from_str_radix(&id[i*2..i*2+2],16)?;}
        return Ok(digest);
    }
    #[cfg(not(feature="multiplayer-sdr"))]
    {
    // Exact executable compatibility includes embedded gameplay content and rules.
    // This is a compatibility fingerprint, never a player authentication credential.
    let mut file = std::fs::File::open(std::env::current_exe()?)?;
    let mut hash = 0xcbf29ce484222325_u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        for b in &buffer[..n] {
            hash = (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3);
        }
    }
    let mut content = [0; 32];
    for i in 0..4 {
        content[i * 8..i * 8 + 8].copy_from_slice(&hash.wrapping_add(i as u64).to_le_bytes());
    }
    Ok(content)
    }
}
fn identity(world: &World) -> Result<WorldIdentity> {
    // Admission happens before a joining client knows the world's seed.
    let content = executable_digest()?;
    Ok(WorldIdentity {
        seed: u64::from(world.seed),
        generator: world.generator_version,
        cell_mm: (world.voxel_scale.meters() * 1000.0).round() as u16,
        content,
    })
}
fn kind_code(kind: WorldType) -> u8 {
    kind.code()
}
fn kind_from_code(code: u8) -> Result<WorldType> {
    Ok(match code {
        0 => WorldType::Superflat,
        1 => WorldType::Hills,
        2 => WorldType::Ridged,
        3 => WorldType::Nether,
        4 => WorldType::Planet,
        5 => WorldType::Manyfold,
        6 => WorldType::Hall,
        c => WorldType::from_code(c).with_context(|| format!("host world kind {c} is not known"))?,
    })
}
fn scale_code(scale: VoxelScalePreset) -> u8 {
    match scale {
        VoxelScalePreset::OneMeter => 0,
        VoxelScalePreset::HalfMeter => 1,
        VoxelScalePreset::QuarterMeter => 2,
    }
}
fn scale_from_code(code: u8) -> Result<VoxelScalePreset> {
    Ok(match code {
        0 => VoxelScalePreset::OneMeter,
        1 => VoxelScalePreset::HalfMeter,
        2 => VoxelScalePreset::QuarterMeter,
        _ => anyhow::bail!("host voxel scale {code} is not known"),
    })
}
/// The host's world as a joiner builds it. The generator comes from the lobby, so the identity matches.
fn world_for_join(params: engine_net::lobby::wire::WorldParams, generator: u32) -> Result<World> {
    ensure!(multiplayer_general_generator(generator)
        || params.kind == 4 && (generator == engine_voxel::CUSTOM_PLANET_VERSION
            || multiplayer_planet_generator(generator)
            || generator == 137)
        || params.kind == 3 && matches!(generator, engine_voxel::HELL_PORTAL_GENERATOR | engine_voxel::HELL_ECOLOGY_GENERATOR | engine_voxel::HELL_RUINS_GENERATOR | engine_voxel::HELL_SURVEYED_RUINS_GENERATOR | engine_voxel::HELL_MIRROR_GENERATOR | engine_voxel::HELL_BIOME_GENERATOR), "host generator {generator} is not supported");
    let mut world = World::with_seed(kind_from_code(params.kind)?, scale_from_code(params.scale)?, params.seed);
    world.generator_version = generator;
    Ok(world)
}

fn multiplayer_general_generator(generator: u32) -> bool {
    (37..=engine_voxel::WORKSHOP_SUPPLY_GENERATOR).contains(&generator)
        || (engine_voxel::SHARED_SURFACE_GENERATOR..=engine_voxel::CURRENT_GENERATOR_VERSION).contains(&generator)
}

fn multiplayer_planet_generator(generator: u32) -> bool {
    (engine_voxel::COASTAL_PLANET_GENERATOR..=engine_voxel::PLANET_FREIGHT_DETOUR_GENERATOR).contains(&generator)
        || (engine_voxel::WATERSHED_EROSION_GENERATOR..=engine_voxel::BRANCHING_WATERSHEDS_GENERATOR).contains(&generator)
        || generator == engine_voxel::LANDSCAPE_CYCLES_GENERATOR
        || generator == engine_voxel::DEEP_TIME_GENERATOR
        || generator == engine_voxel::FRACTAL_COAST_GENERATOR
        || generator == engine_voxel::PROCESS_COAST_GENERATOR
        || generator == engine_voxel::CLEAN_RIVERBED_GENERATOR
        || generator == engine_voxel::GROUND_BREAK_GENERATOR
        || generator == engine_voxel::LIVING_SURFACE_GENERATOR
        || generator == engine_voxel::LOWLAND_RIVER_GENERATOR
        || generator == engine_voxel::RIFT_VALLEY_GENERATOR
        || generator == engine_voxel::DIKE_SWARM_GENERATOR
        || generator == engine_voxel::CARVED_SURFACE_GENERATOR
        || generator == engine_voxel::ROCK_RELIEF_GENERATOR
        || generator == engine_voxel::DUNGEON_MECHANISMS_GENERATOR
        || matches!(generator, engine_worldgen::planetary::EVOLVING_DELTA_GENERATOR
            | engine_worldgen::planetary::LANDSCAPE_HISTORY_GENERATOR
            | engine_worldgen::planetary::MAPPED_CANYON_GENERATOR
            | engine_worldgen::planetary::FAULT_HISTORY_GENERATOR
            | engine_worldgen::planetary::WEATHERING_HISTORY_GENERATOR
            | engine_worldgen::planetary::JOINTED_CANYON_GENERATOR
            | engine_worldgen::planetary::SEDIMENT_WEATHERING_GENERATOR)
}

/// Scoreboard objectives the host keeps.
const PLAYED: u8 = 0;
const LOWEST: u8 = 1;

fn define_objectives(board: &mut BoardWriter) -> Result<()> {
    let objective = |label: &str, unit, order| Objective { label: label.into(), unit, order };
    let changes = [
        Change::Define { id: PLAYED, objective: objective("Played", Unit::Seconds, Order::HighFirst) },
        Change::Define { id: LOWEST, objective: objective("Lowest", Unit::Metres, Order::LowFirst) },
        Change::Show { slot: Slot::List, id: Some(PLAYED) },
        Change::Show { slot: Slot::Sidebar, id: Some(LOWEST) },
    ];
    for change in changes {
        board.change(change).map_err(|e| anyhow::anyhow!(e.0))?;
    }
    Ok(())
}

fn encode<T: Serialize>(tag: u8, value: &T) -> Result<Vec<u8>> {
    let mut bytes = vec![tag];
    bytes.extend_from_slice(ron::to_string(value)?.as_bytes());
    ensure!(
        bytes.len() <= 900,
        "replication message exceeds packet budget"
    );
    Ok(bytes)
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    Ok(ron::de::from_bytes(bytes)?)
}

impl Session {
    /// Explicit opt-in creates a new private building world, separate from saved sessions.
    pub fn start() -> Result<Option<(Self, World, Player, DayNightCycle)>> {
        let args: Vec<_> = std::env::args().collect();
        let host = args
            .iter()
            .find_map(|a| a.strip_prefix("--multiplayer-host="));
        let join = args
            .iter()
            .find_map(|a| a.strip_prefix("--multiplayer-join="));
        let join_code = args
            .iter()
            .find_map(|a| a.strip_prefix("--multiplayer-join-code="));
        let online = args
            .iter()
            .find_map(|a| a.strip_prefix("--multiplayer-online="));
        let lobby: SocketAddr = args
            .iter()
            .find_map(|a| a.strip_prefix("--multiplayer-lobby="))
            .unwrap_or(engine_net::lobby::DEFAULT_LOBBY)
            .parse()
            .context("multiplayer lobby must be IP:port")?;
        let choices = [host, join, join_code, online].iter().filter(|c| c.is_some()).count();
        ensure!(choices <= 1, "choose one of host, join, join code or online host");
        let target = if let Some(name) = online {
            ensure!(
                !name.trim().is_empty() && name.len() <= engine_net::lobby::wire::MAX_NAME && !name.chars().any(char::is_control),
                "online session name must be 1 to 48 printable characters"
            );
            Target::Lobby { lobby, code: String::new() }
        } else if let Some(code) = join_code.filter(|code| engine_net::punch::is_offer(code)) {
            direct_invite(code)?
        } else if let Some(code) = join_code {
            match code.chars().filter(|c| !matches!(c, '-' | ' ')).count() {
                8 => Target::Lobby { lobby, code: code.to_owned() },
                12 => direct_invite(code)?,
                _ => anyhow::bail!("a lobby code has 8 characters; an invite code has 12"),
            }
        } else if let Some(text) = host.or(join) {
            direct_address(text)?
        } else {
            return Ok(None);
        };
        let hosting = host.is_some() || online.is_some();
        let joining = join.is_some() || join_code.is_some();
        if args.iter().any(|arg| arg == "--multiplayer-review-travel") {
            ensure!(hosting && args.iter().any(|arg| arg == "--multiplayer-review-arena")
                && args.iter().any(|arg| arg == "--multiplayer-ephemeral")
                && !args.iter().any(|arg| arg.starts_with("--multiplayer-open-world=")
                    || arg.starts_with("--multiplayer-save=")),
                "travel review requires an ephemeral arena");
        }
        let review_bases = args.iter().any(|arg| arg == "--multiplayer-review-bases");
        if review_bases && hosting {
            ensure!(args.iter().any(|arg| arg == "--multiplayer-ephemeral")
                && args.iter().any(|arg| arg == "--multiplayer-headless")
                && !args.iter().any(|arg| arg.starts_with("--multiplayer-open-world=")
                    || arg.starts_with("--multiplayer-save=")),
                "base review requires an ephemeral dedicated host and cannot open a save");
        }
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        let description=match &target {
            Target::Direct(_) if joining=>Some(describe_target(&target)?),
            Target::PunchJoin(_) if joining=>Some(describe_target(&target)?),
            #[cfg(feature="multiplayer-sdr")]
            Target::SteamSdr(_) if joining=>Some(describe_target(&target)?),
            _=>None,
        };
        // A lobby join learns the host's world before any world exists, so it builds that world.
        let joined = match &target {
            Target::Lobby { lobby, code } if !hosting => {
                Some(Client::join(*lobby, code, executable_digest()?, Instant::now()).map_err(|e| anyhow::anyhow!(e.to_string()))?)
            }
            _ => None,
        };
        let seed: u32 = args
            .iter()
            .find_map(|a| a.strip_prefix("--multiplayer-seed="))
            .unwrap_or("74")
            .parse()?;
        let mut world = match &joined {
            Some((_, params)) => world_for_join(params.world, params.version)?,
            None => {
                let kind = if hosting && args.iter().any(|a| a == "--multiplayer-planet") {
                    WorldType::Planet
                } else {
                    WorldType::Hills
                };
                let scale = match args.iter().find_map(|a| a.strip_prefix("--multiplayer-voxel-mm=")) {
                    None | Some("500") => VoxelScalePreset::HalfMeter,
                    Some("1000") => VoxelScalePreset::OneMeter,
                    Some("250") => VoxelScalePreset::QuarterMeter,
                    Some(_) => anyhow::bail!("multiplayer voxel size must be 250, 500 or 1000 mm"),
                };
                let mut world = World::with_seed(kind, scale, seed);
                if let Some(version) = args.iter().find_map(|a| a.strip_prefix("--multiplayer-generator=")) {
                    let version: u32 = version.parse().context("invalid multiplayer generator")?;
                    ensure!(multiplayer_general_generator(version)
                        || kind == WorldType::Planet && multiplayer_planet_generator(version),
                        "unsupported multiplayer generator");
                    world.generator_version = version;
                }
                world
            }
        };
        // The host's handshake timer starts at the lobby join, so hello goes before any slow
        // terrain preparation; the identity needs only the seed, generator and scale.
        let mut joined = joined;
        if let Some((client, _)) = joined.as_mut() {
            let hello = identity(&world)?;
            eprintln!("[multiplayer] hello seed={} generator={} cell_mm={}", hello.seed, hello.generator, hello.cell_mm);
            client.hello(hello).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        // `--multiplayer-open-world=NAME`: host a saved single-player world, which keeps its own save.
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if let Some((description,kind))=description {world=open::described_world(description,kind)?;}
        let opened = match args.iter().find_map(|a| a.strip_prefix("--multiplayer-open-world=")) {
            Some(name) => {
                ensure!(hosting, "an opened world requires --multiplayer-host or --multiplayer-online");
                match world::storage::load_named(name)? {
                    Some(saved) => {
                        world = saved.world;
                        eprintln!("[multiplayer] opened world={name} root={}", world.regions.view.root.display());
                        Some((Some(saved.player), saved.day_night))
                    }
                    // A dedicated host creates its named world on first start, as a server would.
                    None => {
                        world::storage::name_world(world.regions.view.session_name(), name)?;
                        eprintln!("[multiplayer] created world={name} root={}", world.regions.view.root.display());
                        Some((None, DayNightCycle::default()))
                    }
                }
            }
            None => None,
        };
        let setup = HostSetup {
            name: online.unwrap_or_default().to_owned(),
            world: open::world_label(&world),
            motd: "A shared world".to_owned(),
            mode: if args.iter().any(|a| a == "--multiplayer-creative") {
                engine_net::lobby::wire::Mode::Creative
            } else {
                engine_net::lobby::wire::Mode::Survival
            },
            max_players: 8,
            unlisted: args.iter().any(|a| a == "--multiplayer-unlisted"),
            world_params: engine_net::lobby::wire::WorldParams {
                seed: world.seed,
                kind: kind_code(world.world_type),
                scale: scale_code(world.voxel_scale),
            },
        };
        let mut player = Player::new();
        let mut day = DayNightCycle::default();
        let mut profiles=BTreeMap::new();
        // A joiner's spawn comes with the host's snapshot; searching for one here would
        // spend the host's handshake window.
        if !joining {
            player.pos = world.suggested_spawn();
            player.precise_pos = player.pos.as_dvec3();
        }
        let sidecar = match opened {
            Some((saved_player, saved_day)) => {
                match saved_player {
                    Some(saved) => player = saved,
                    None => {world.adventure.begin_journey();world.ensure_starter_wand();world.sync_from_inventory();}
                }
                day = saved_day;
                let path = profiles::path_for(&world.regions.view.root);
                let (loaded, note) = profiles::load_or_set_aside(&path);
                if let Some(note) = note {
                    eprintln!("[multiplayer] {note}");
                }
                profiles = loaded;
                Some(path)
            }
            None => {
                world.adventure.begin_journey();world.ensure_starter_wand();world.sync_from_inventory();
                // A building world has no rules file; its mode comes from the command line.
                let mode = if args.iter().any(|a| a == "--multiplayer-creative") { world::rules::GameMode::Creative } else { world::rules::GameMode::Survival };
                world.rules = world::rules::Rules::new_world(mode);
                None
            }
        };
        let save_path = if hosting && sidecar.is_none() && !args.iter().any(|a| a == "--multiplayer-ephemeral") {
            Some(
                args.iter()
                    .find_map(|a| a.strip_prefix("--multiplayer-save="))
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| {
                        world::storage::save_directory()
                            .join("multiplayer-building")
                            .join(format!("seed-{seed}.ron"))
                    }),
            )
        } else {
            None
        };
        if let Some(path) = save_path.as_ref().filter(|path| path.is_file()) {
            let mut file = std::fs::File::open(path)?;
            ensure!(
                file.metadata()?.len() <= engine_net::transfer::MAX_TRANSFER as u64,
                "building save exceeds size limit"
            );
            let mut bytes = Vec::new();
            file.by_ref()
                .take(engine_net::transfer::MAX_TRANSFER as u64 + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= engine_net::transfer::MAX_TRANSFER,
                "building save exceeds size limit"
            );
            let saved: BuildingSave = if bytes.first()==Some(&b'(') {decode(&bytes)?} else {gameplay::unpack(&bytes)?};
            if saved.format>=2 {ensure!(engine_net::transfer::checksum(ron::to_string(&saved.profiles)?.as_bytes())==saved.profile_checksum,"player registry checksum mismatch");}
            ensure!(
                snapshot_checksum(&saved.snapshot,saved.format)?
                    == saved.checksum,
                "building save checksum mismatch"
            );
            ensure!(
                (1..=2).contains(&saved.format)
                    && saved.seed == seed
                    && multiplayer_general_generator(saved.generator)
                    && saved.cell_mm == 500,
                "building save configuration mismatch"
            );
            ensure!(saved.profiles.len()<=128,"player registry limit");
            profiles=saved.profiles;
            world.generator_version = saved.generator;
            apply_snapshot(
                &mut world,
                &mut player,
                &mut day,
                saved.snapshot,
                &mut HashSet::new(),
                &mut HashSet::new(),
            )?;
            eprintln!(
                "[multiplayer] loaded path={} revision={}",
                path.display(),
                world.revision()
            );
            world.expedition.say("World loaded");
        }
        // The survey is thread-local. Finish cold collision/HUD preparation
        // before opening either end of a connection with timed admission.
        // A joiner is already inside the host's handshake window, so it skips this; its first
        // collision query happens only after the snapshot arrives.
        if !joining {
            eprintln!("[multiplayer] preparing terrain");
            let _ = engine_worldgen::surface::bridge_review_view(&world.sample_config());
        }
        let signature = identity(&world)?;
        if review_bases && hosting {
            base_review::setup(&mut world, &mut player);
        }
        if args.iter().any(|arg|arg=="--multiplayer-review-arena") {
            ensure!(hosting&&args.iter().any(|arg|arg=="--multiplayer-ephemeral"), "review arena requires an ephemeral host");
            let y=player.pos.y.ceil() as i32+40;
            let mut cells=Vec::new();
            for x in -16..16 { for z in -16..16 {
                cells.push((IVec3::new(x,y-1,z),BlockId::Dirt));
                for up in 0..10 {cells.push((IVec3::new(x,y+up,z),BlockId::Air));}
            }}
            world.set_many(&cells);
            player.pos=Vec3::new(0.5,y as f32,0.5);
            player.precise_pos=player.pos.as_dvec3();
            if args.iter().any(|arg| arg == "--multiplayer-review-travel") {
                travel_review::setup(&mut world, &player);
            }
            if args.iter().any(|arg| arg == "--multiplayer-review-slots") {
                let chest = IVec3::new(2, y, 0);
                world.set(chest, BlockId::Chest);
                let mut slots = world.block_data.chest(chest);
                slots[0] = Some(world::inventory::ItemStack::new(world::inventory::Item::Block(BlockId::Stone), 8));
                world.block_data.set_chest(chest, slots);
            }
        }
        world.network_host=hosting;
        let profile=if joining{gameplay::profile()?}else{String::new()};
        let key = if joining { Some(gameplay::player_key()?) } else { None };
        // A joiner keeps the code it used, so anyone in the session can invite.
        let code = match &target {
            #[cfg(feature="multiplayer-sdr")]
            Target::SteamSdr(id)=>Some(format!("steam:{}",if hosting {engine_net::steam::local_id().map_err(|e|anyhow::anyhow!(e.to_string()))?}else{*id})),
            Target::Lobby { code, .. } if !code.is_empty() => Some(code.clone()),
            _ => None,
        };
        // Renderer creation can take longer than the handshake timeout. Open the
        // connection on the first live update, after startup has completed.
        let role = match joined {
            // The joiner already learned the host's world; it sends its identity now, before the
            // renderer starts, so the host's handshake timer is not spent on startup.
            Some((client, _)) => {
                Role::Client {
                    socket: client,
                    receiver: Receiver::default(),
                    sequence: 0,
                    ready: false,
                    started: Instant::now(),
                }
            }
            None => Role::Pending {
                host: hosting,
                target,
                setup,
                identity: signature,
            },
        };
        world.expedition.say(if hosting { "Opening building session" } else { "Connecting" });
        let parts = open::Parts {
            role, profile, profiles, creative: args.iter().any(|a| a == "--multiplayer-creative"),
            headless: args.iter().any(|a| a == "--multiplayer-headless"), save_path, sidecar,
            name: gameplay::player_name(), key, code,
        };
        let mut session = Self::assemble(parts, &world);
        session.prime();
        Ok(Some((session, world, player, day)))
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    /// Holds the joiner's own body while the terrain around it is still loading,
    /// as single-player does not step while spawning.
    pub fn hold_body(&mut self, hold: bool) {
        self.hold = hold;
    }
    /// `/stop` ran: the world is saved and the session closed.
    pub fn stopped(&self) -> bool {
        self.stopped
    }
    /// A line typed at a dedicated host's console; runs as an operator.
    pub fn console(&mut self, line: String) {
        if self.console.len() < 64 {
            self.console.push(line);
        }
    }
    /// Remote players' head positions in voxels with their names, for labels.
    pub fn name_tags(&self, scale: f32) -> Vec<(Vec3, String)> {
        self.remotes
            .iter()
            .filter_map(|(id, remote)| {
                let name = self.social.name_of(*id)?;
                let up = remote.shown_frame * Vec3::Y;
                Some((remote.shown + up * (2.1 / scale), name.to_owned()))
            })
            .collect()
    }
    pub fn configure(&mut self,creative:&crate::creative::CreativeState) {
        self.hand=creative.selected_slot.min(8);
        let tools=actions::Tools::capture(creative);
        if let Ok(key)=ron::to_string(&tools) && key!=self.tools_key {
            self.tools_key=key;self.queue(actions::Command::Tools(tools));
        }
    }
    pub fn queue_inventory(&mut self,commands:Vec<inventory::Command>) {
        for c in commands {self.queue(actions::Command::Inventory(c));}
    }
    pub fn queue_lamp(&mut self) {self.queue(actions::Command::Lamp);}
    pub fn queue_resident(&mut self, villager: u32, action: world::travel::ResidentAction) {
        self.queue(actions::Command::Resident(villager, action));
    }
    pub fn queue_edit_history(&mut self,direction:world::edit_history::Direction) {self.queue(actions::Command::EditHistory(direction));}
    pub fn is_host(&self)->bool {matches!(self.role,Role::Host{..}|Role::Pending{host:true,..})}
    pub fn inventory_view(&self,w:&World,position:Vec3)->world::inventory::SurvivalInventory {
        let mut inv=w.inventory.clone();let bench=world::factory::at_bench(w,position);
        for command in self.unack.iter().map(|(_,c)|c).chain(self.commands.iter()) {
            if let actions::Command::Inventory(c)=command {let _=c.clone().apply(&mut inv,bench);}
        }
        inv
    }
    fn queue(&mut self, command: actions::Command) {
        if self.commands.len() + self.unack.len() < MAX_OUTSTANDING_COMMANDS {
            self.commands.push_back(command);
        }
    }
    pub fn queue_scanner(&mut self, anchor: IVec3, action: world::scanner::commands::Action) {
        self.queue(actions::Command::Scanner(anchor, action));
    }
    pub fn route_ui(&mut self,a:crate::egui_ui::UiAction)->crate::egui_ui::UiAction {
        match actions::from_ui(a) {Ok(command)=>{self.queue(command);crate::egui_ui::UiAction::None},Err(a)=>a}
    }
    pub fn persistent_host(&self) -> bool {
        self.save_path.is_some()
    }
    pub fn ready(&self) -> bool {
        matches!(
            self.role,
            Role::Host { .. } | Role::Client { ready: true, .. }
        ) && !self.failed
    }
    pub fn take_world_reset(&mut self) -> bool {
        std::mem::take(&mut self.reset_render)
    }
    pub fn take_portal_warmup(&mut self) -> Option<crate::render::PortalWarmup> {
        self.portal_warmup.take()
    }
    pub fn save(&mut self, world: &mut World, player: &Player, day: &DayNightCycle) -> Result<()> {
        // A hosted single-player world saves through its own storage; only profiles are written here.
        if self.sidecar.is_some() {
            self.last_save = Instant::now();
            return self.flush_profiles();
        }
        let path = self
            .save_path
            .as_ref()
            .context("Only a persistent host can save this world")?;
        let snapshot = snapshot_state(world, player, day, 0)?;
        let mut profiles=self.profiles.clone();
        if matches!(self.role, Role::Host { .. }) {for actor in self.authority.actors.values() {
            if !actor.profile.is_empty(){profiles.insert(actor.profile.clone(),gameplay::save_player(&actor.player,&actor.personal)?);}
        }}
        let bytes = gameplay::pack(&BuildingSave {
            profile_checksum:engine_net::transfer::checksum(ron::to_string(&profiles)?.as_bytes()),
            profiles,format: 2,
            seed: world.seed,
            generator: world.generator_version,
            cell_mm: 500,
            checksum: snapshot_checksum(&snapshot,2)?,
            snapshot,
        })?;
        ensure!(
            bytes.len() <= engine_net::transfer::MAX_TRANSFER,
            "building save exceeds size limit"
        );
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension(format!("ron.{}.tmp", std::process::id()));
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        self.saved_revision = world.revision();
        self.last_save = Instant::now();
        eprintln!(
            "[multiplayer] saved revision={} bytes={} path={}",
            self.saved_revision,
            bytes.len(),
            path.display()
        );
        Ok(())
    }
    pub fn update(
        &mut self,
        world: &mut World,
        player: &mut Player,
        day: &mut DayNightCycle,
        input: &Input,
        dt: f32,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
    ) {
        if self.failed {
            return;
        }
        if let Err(error) = self.update_inner(world, player, day, input, dt, blocks, chunks) {
            // Preserve the most recent friend inventories/progression before
            // closing the role drops its live actors. Close/save can flush it.
            match self.collected_profiles() {
                Ok(profiles) => { self.profiles = profiles; self.profiles_dirty = true; }
                Err(save_error) => eprintln!("[multiplayer] disconnect profile capture failed: {save_error:#}"),
            }
            self.failed = true;
            self.role = Role::Closed;
            self.authority.actors.clear();
            self.authority.outputs.clear();
            world.expedition.say(format!("Disconnected: {error}"));
            eprintln!("multiplayer stopped: {error:#}");
        }
        let profiles_due = self.sidecar.is_some() && (self.profiles_dirty || self.others() > 0);
        let save_interval = if self.sidecar.is_some() && self.profiles_dirty {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(10)
        };
        if (self.save_path.is_some() || profiles_due)
            && self.last_save.elapsed() >= save_interval
        {
            self.last_save = Instant::now();
            if let Err(error) = self.save_periodic(world, player, day) {
                world.expedition.say(format!("Save failed: {error}"));
                eprintln!("[multiplayer] save failed: {error:#}");
            }
        }
        let playback_now = smoothing::clock_s();
        let blend = 1.0 - (-dt.min(0.25) * 16.0).exp();
        let metres_per_cell = world.voxel_scale.meters();
        for remote in self.remotes.values_mut() {
            remote.pose.swing = (remote.pose.swing - dt.max(0.0) / remote.pose.swing_duration).max(0.0);
            let previous=remote.shown;
            remote.playback.advance(playback_now);
            if let Some(sample) = remote.playback.sample() {
                remote.show(sample);
            } else {
                let target = remote.pose.sample();
                remote.shown = remote.shown.lerp(target.position, blend);
                remote.shown_yaw = smoothing::blend_yaw(remote.shown_yaw, target.yaw, blend);
                remote.shown_pitch += (target.pitch - remote.shown_pitch) * blend;
                remote.shown_frame = remote.shown_frame.slerp(target.gravity_frame, blend).normalize();
                remote.shown_flying = target.flying;
            }
            let movement = (remote.shown - previous) * metres_per_cell;
            let up = remote.shown_frame * Vec3::Y;
            let distance = (movement - up * movement.dot(up)).length();
            let speed=if dt>0.0001 && distance<2.0 {distance/dt}else{0.0};
            let moving=if remote.shown_flying {0.0}else{(speed/3.5).clamp(0.0,1.0)};
            remote.stride+=(moving-remote.stride)*blend;
            if !remote.shown_flying && distance<2.0 {
                remote.phase=(remote.phase+distance*5.5).rem_euclid(std::f32::consts::TAU);
            }
        }
    }
    fn update_inner(
        &mut self,
        world: &mut World,
        player: &mut Player,
        day: &mut DayNightCycle,
        input: &Input,
        dt: f32,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
    ) -> Result<()> {
        let now = Instant::now();
        if let Role::Pending {
            host,
            target,
            setup,
            identity,
        } = &self.role
        {
            self.role = if *host {
                let mut socket = Host::open(target, *identity, setup)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                self.install_host(&mut socket, world)?;
                Role::Host { socket }
            } else {
                let socket = Client::open(target, *identity, now)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                eprintln!("[multiplayer] connecting");
                Role::Client {
                    socket,
                    receiver: Receiver::default(),
                    sequence: 0,
                    ready: false,
                    started: now,
                }
            };
        }
        self.network_clock += dt.min(0.1);
        self.pending_mine |= input.break_pressed;
        self.pending_place |= input.place_pressed;
        self.pending_input.cast_pressed|=input.cast_pressed;
        self.pending_input.cycle_spell|=input.cycle_spell;
        self.pending_input.interact|=input.interact;
        self.pending_input.dash_pressed|=input.dash_pressed;
        let network_tick = self.network_clock >= 1.0 / 30.0;
        if network_tick {
            self.network_clock %= 1.0 / 30.0;
        }
        // Save and stop need the whole session, so they run after the role is released.
        let mut late = Vec::new();
        match &mut self.role {
            Role::Closed => anyhow::bail!("session closed"),
            Role::Pending { .. } => unreachable!("transport initialized above"),
            Role::Host { .. } => {
                let Role::Host { mut socket } = std::mem::replace(&mut self.role, Role::Closed) else { unreachable!() };
                let result = self.host_realms(&mut socket, world, player, day, input, dt, blocks, chunks, now, network_tick, &mut late);
                self.role = Role::Host { socket };
                result?;
            }
            Role::Client {
                socket,
                receiver,
                sequence,
                ready,
                started,
            } => {
                self.accumulator += dt.min(0.1);
                if let Some(realm::Incoming::Building(incoming)) = &self.incoming_realm {
                    use std::sync::mpsc::TryRecvError;
                    match incoming.try_recv() {
                        Ok(result) => {
                            self.incoming_realm = Some(realm::Incoming::Ready(Box::new(result?)));
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => anyhow::bail!("Destination preparation stopped"),
                    }
                }
                let mut packets = std::mem::take(&mut self.early);
                packets.extend(socket
                    .poll(now)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?);
                if !*ready && socket.waiting_for_punch() {
                    // World synchronization starts once the invite exchange finishes.
                    *started = now;
                    if self.social.code.is_none() && let Some(answer) = socket.punch_answer() {
                        if self.headless { eprintln!("[multiplayer] punch answer={answer}"); }
                        self.social.code = Some(answer.to_owned());
                    }
                }
                if matches!(self.incoming_realm, Some(realm::Incoming::Ready(_)))
                    && socket.send_data(vec![4]).map_err(|e| anyhow::anyhow!(e.to_string()))?
                    && let Some(realm::Incoming::Ready(prepared)) = self.incoming_realm.take()
                {
                    let (new_world, new_player, new_day, deferred, warmup) = *prepared;
                    *world = new_world;
                    *player = new_player;
                    *day = new_day;
                    self.replica_edits.set_missing(deferred);
                    self.portal_warmup = Some(warmup);
                    blocks.clear();
                    chunks.clear();
                    self.accumulator = 0.0;
                    self.reset_render = true;
                    *ready = true;
                    eprintln!("[multiplayer] synchronized player={:?}", socket.player());
                }
                self.social.own = socket.player();
                for packet in packets {
                    if let Packet::Roster { players } = &packet {
                        self.social.learn_names(players.clone());
                        continue;
                    }
                    if let Packet::Chat { kind, from, text } = &packet {
                        self.social.push(*kind, *from, text);
                        continue;
                    }
                    let Packet::Data(bytes) = packet else {
                        continue;
                    };
                    match bytes.first() {
                        Some(1 | 2) if !*ready || bytes[0] == 1 => {
                            ensure!(self.incoming_realm.is_none(), "overlapping realm baselines");
                            if *ready {
                                *ready = false;
                                *receiver = Receiver::default();
                                *started = now;
                                *sequence = 0;
                                self.commands.clear();
                                self.unack.clear();
                                self.command_upload = None;
                                self.command_sequence = 0;
                                self.crossing_requested = false;
                                self.correction_ack = 0;
                                self.pending_input = Input::default();
                                self.pending_mine = false;
                                self.pending_place = false;
                                self.pending_damage = 0.0;
                                self.accumulator = 0.0;
                                self.remotes.clear();
                                self.social.board = Default::default();
                                self.social.quality.clear();
                                self.mining_feedback = Default::default();
                                self.contacts.clear();
                                self.replica_edits = Default::default();
                            }
                            if let Some(bytes) = receiver.receive(&bytes)? {
                                let current = (kind_code(world.world_type), scale_code(world.voxel_scale), world.seed, world.generator_version);
                                self.incoming_realm = Some(realm::prepare_client(bytes, current)?);
                            }
                        }
                        Some(11|12) if *ready => {
                            if bytes[0] == 11 { self.replica_edits.begin_receive(now)?; }
                            let mut payload = bytes; payload[0] -= 10;
                            if let Some(bytes) = receiver.receive(&payload)? {
                                let prepared = replication::prepare(&bytes, world)?;
                                ensure!(prepared.command_ack <= self.command_sequence, "invalid command acknowledgement");
                                self.replica_edits.stage(prepared, now, self.command_sequence)?;
                            }
                        }
                        Some(6) if *ready => {
                            let pose: Pose = decode(&bytes[1..])?;
                            pose.validate()?;
                            // This side moves its own body; the host corrects it with message 17.
                            if Some(pose.id) != socket.player() {
                                ensure!(
                                    self.remotes.len() < 16 || self.remotes.contains_key(&pose.id),
                                    "roster exceeds player limit"
                                );
                                self.remotes
                                    .entry(pose.id)
                                    .and_modify(|r| r.receive(pose.clone(), world.voxel_scale.meters()))
                                    .or_insert_with(|| Remote::new(pose));
                            }
                        }
                        Some(25) if *ready => audio_events::receive(world, &bytes[1..])?,
                        Some(26) if *ready => audio_events::receive_contacts(&mut self.contacts, &bytes[1..])?,
                        Some(7) if *ready => {
                            let roster: Roster = decode(&bytes[1..])?;
                            ensure!(
                                roster.ids.len() <= 17 && roster.time.is_finite(),
                                "invalid roster"
                            );
                            self.remotes.retain(|id, _| roster.ids.contains(id));
                            day.set_day_index(roster.day);
                            day.set_time_of_day(roster.time);
                        }
                        Some(16) if *ready => {
                            let wire: Vec<(PlayerId, String, u8)> = decode(&bytes[1..])?;
                            ensure!(
                                wire.len() <= 17 && wire.iter().all(|(_, name, flags)| name.len() <= 64 && *flags < 4),
                                "invalid player list"
                            );
                            let rows: Vec<_> = wire
                                .into_iter()
                                .map(|(id, name, flags)| social::Row { id, name, op: flags & 1 != 0, creative: flags & 2 != 0 })
                                .collect();
                            if let Some(own) = rows.iter().find(|r| Some(r.id) == socket.player()) {
                                self.creative = own.creative;
                                player.flying &= own.creative;
                            }
                            self.social.set_roster(rows);
                        }
                        Some(19) if *ready && bytes.len() == 5 => {
                            let mut echo = bytes.clone();
                            echo[0] = 20;
                            let _ = socket.send_data(echo).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        }
                        // Assigned, not `set_rules`: the host's rules file is never written here.
                        Some(23) if *ready => {
                            let border = world.rules.border_m;
                            world.rules = world::rules::Rules::decode(&bytes[1..])?;
                            world.rules.border_m = border;
                        }
                        Some(&status::BORDER) if *ready => { world.rules.border_m = status::decode_border(&bytes[1..])?; }
                        Some(21) if *ready => {
                            scoreboard::apply_flush(&mut self.social.board, &bytes[1..]).map_err(|e| anyhow::anyhow!(e.0))?;
                        }
                        Some(22) if *ready => {
                            let pings: Vec<(PlayerId, [u8; 6])> = decode(&bytes[1..])?;
                            ensure!(pings.len() <= 17, "invalid ping table");
                            self.social.quality = pings
                                .into_iter()
                                .map(|(id, q)| Quality::decode(q).map(|q| (id, q)).context("invalid ping entry"))
                                .collect::<Result<_>>()?;
                        }
                        Some(17) if *ready => {
                              let (id, position, velocity, frame): (u16, [f32; 3], [f32; 3], glam::Quat) = decode(&bytes[1..])?;
                            ensure!(frame.is_finite() && (frame.length_squared() - 1.0).abs() < 0.001, "invalid correction frame");
                            player.gravity_frame = frame;
                            ensure!(position.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000.0), "invalid correction");
                            player.pos = Vec3::from_array(position);
                            player.precise_pos = player.pos.as_dvec3();
                              ensure!(velocity.iter().all(|value|value.is_finite()&&value.abs()<=1_000.0), "invalid correction velocity");
                              player.vel = Vec3::from_array(velocity);
                            self.correction_ack = id;
                        }
                        _ => anyhow::bail!("unexpected replication packet"),
                    }
                }
                // A gated host refuses data and input until the player is introduced.
                if !self.introduced && let Some(key) = &self.key {
                    self.introduced = socket.introduce(&self.name, key).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                }
                if self.introduced && !self.join_sent {
                    if socket.send_data(encode(10,&self.profile)?).map_err(|e|anyhow::anyhow!(e.to_string()))? {self.join_sent=true;}
                }
                if *ready {
                    if let Some(applied) = self.replica_edits.drain(world, player, day, blocks, chunks, now)? {
                        self.unack.retain(|(sequence, _)| *sequence > applied.command_ack);
                        if applied.water_mismatch {
                            eprintln!("[multiplayer] water digest mismatch; requesting a full list");
                            socket.send_data(vec![replication::RESEND_WATER]).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        }
                    }
                }
                if *ready && !self.hold {
                    // Own body at the shared fixed step, so movement never waits for the host.
                    let mut step = input.clone();
                    while self.accumulator >= STEP {
                        self.accumulator -= STEP;
                        world.adventure.creative = self.creative;
                        player.gear = world.inventory.mobility();
                        player.glide.wind = crate::player::glide::weather_wind(world, day, player.pos);
                        crate::player::traversal::prepare(world, player, &step, STEP);
                        player.flying &= self.creative;
                        let previous = (player.pos, player.precise_pos);
                        if !world::travel::steer_rider(world, player, &step, true, STEP) {
                            update_player(player, &mut step, world, STEP);
                        }
                        if self.replica_edits.missing_at(player.bounds(world.voxel_scale)) {
                            (player.pos, player.precise_pos) = previous;
                            player.vel = Vec3::ZERO;
                        }
                        crate::manyfold::weave::feedback(world, player);
                        player.flying &= self.creative;
                        self.pending_damage += player.take_pending_damage();
                        step.dash_pressed = false;
                        step.cast_pressed = false;
                        step.break_pressed = false;
                        step.place_pressed = false;
                    }
                    // Presentation follows wall time even when a slow rendered
                    // frame exceeds the bounded movement catch-up budget.
                    world.adventure.advance_multiplayer_visuals(dt.clamp(0.0, 0.25));
                    world::adventure::Adventure::advance_multiplayer_projectiles(world, dt);
                    Self::local_viewmodel(world, player, input, dt.clamp(0.0, 0.25));
                } else {
                    self.accumulator = 0.0;
                }
                if *ready && self.social.code.as_deref().is_some_and(|code| code.starts_with(engine_net::punch::ANSWER_PREFIX)) {
                    self.social.code = None;
                }
                if *ready && network_tick {
                    if let Some(sequence) = self.replica_edits.receipt()
                        && socket.send_data(coherence::ack(sequence)).map_err(|e| anyhow::anyhow!(e.to_string()))?
                    {
                        self.replica_edits.receipt_sent();
                    }
                    for line in self.social.take_outbox() {
                        let mut bytes = vec![18];
                        bytes.extend_from_slice(line.as_bytes());
                        if line.len() > engine_net::protocol::MAX_TEXT_BYTES {
                            self.social.push_text(engine_net::protocol::ChatKind::System, "message too long".into());
                        } else if !socket.send_data(bytes).map_err(|e| anyhow::anyhow!(e.to_string()))? {
                            self.social.push_text(engine_net::protocol::ChatKind::System, "message not sent; connection busy".into());
                        }
                    }
                }
                if !*ready && now.duration_since(*started) > SYNC_TIMEOUT {
                    anyhow::bail!("world synchronization timed out");
                }
                // A bounded world transfer may outlast the action-session idle
                // timeout. Keep the transport session alive while it is loading.
                if network_tick && socket.player().is_some() && self.introduced {
                    socket
                        .submit([0; 2], [0; 2], 0)
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                }
                if network_tick && *ready {
                    let next = sequence
                        .checked_add(1)
                        .context("input sequence exhausted")?;
                    let mut held = input.clone();
                    held.cast_pressed |= self.pending_input.cast_pressed;
                    held.cycle_spell |= self.pending_input.cycle_spell;
                    held.interact |= self.pending_input.interact;
                    held.dash_pressed |= self.pending_input.dash_pressed;
                    let mut controls = Controls::capture(next, player, &held);
                    controls.slot = self.hand as u8;
                    controls.mine |= self.pending_mine;
                    controls.place |= self.pending_place;
                    controls.use_secondary |= self.pending_place;
                    controls.body = Some((player.pos.to_array(), player.vel.to_array()));
                    controls.ack = self.correction_ack;
                    controls.damage = self.pending_damage.min(1000.0);
                    if socket
                        .send_data(encode(3, &controls)?)
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?
                    {
                        *sequence = next;
                        self.pending_mine = false;
                        self.pending_place = false;
                        self.pending_input = Input::default();
                        if self.crossing_requested && socket.send_data(vec![realm::CROSS_GATE]).map_err(|e| anyhow::anyhow!(e.to_string()))? {
                            self.crossing_requested = false;
                        }
                        self.pending_damage = 0.0;
                        for _ in 0..COMMAND_FRAGMENTS_PER_TICK {
                            if self.command_upload.is_none() {
                                let Some(command) = self.commands.pop_front() else { break; };
                                let sequence = self.command_sequence.checked_add(1).context("command sequence exhausted")?;
                                let request = actions::Request { sequence, command: command.clone() };
                                ensure!(ron::to_string(&request)?.len() <= 65536, "command exceeds budget");
                                self.command_upload = Some(Sender::new(gameplay::pack(&request)?)?);
                                self.command_sequence = sequence;
                                self.unack.push_back((sequence, command));
                            }
                            let sender = self.command_upload.as_mut().expect("command upload staged");
                            let mut payload = sender.next().expect("command fragment queued");
                            payload[0] += 12;
                            if !socket.send_data(payload).map_err(|e| anyhow::anyhow!(e.to_string()))? {
                                break;
                            }
                            sender.accepted();
                            if sender.next().is_none() {
                                self.command_upload = None;
                            }
                        }
                    }
                }
            }
        }
        for effect in late {
            match effect {
                Deferred::Save if self.save_path.is_some() => {
                    if let Err(error) = self.save(world, player, day) {
                        self.social.push_text(engine_net::protocol::ChatKind::System, format!("save failed: {error}"));
                    }
                }
                Deferred::Save => {
                    self.social.push_text(engine_net::protocol::ChatKind::System, "this world has no save file".into());
                }
                Deferred::Stop => {
                    if self.save_path.is_some() {
                        self.save(world, player, day)?;
                    }
                    self.stopped = true;
                    anyhow::bail!("server stopped");
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn host_frame(
        &mut self, socket: &mut Host, world: &mut World, player: &mut Player,
        day: &mut DayNightCycle, input: &Input, dt: f32, blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>, now: Instant, network_tick: bool,
        packets: Vec<(PlayerId, Vec<u8>)>, connected: &[PlayerId], bulk_per_peer: usize,
        simulation_left: &mut Duration, quantum: Duration, guaranteed_step: bool,
        late: &mut Vec<Deferred>,
    ) -> Result<()> {
        self.authority.validate_world(world)?;
        let realm::Authority {
            actors, outputs, simulation, accumulator, creature_tick, history, footsteps, factory_image,
            local_cooldown, local_mining, local_spell_edits, local_edits, sent_rules, ..
        } = &mut self.authority;
        let mut rejected = HashSet::new();
        for (id, bytes) in packets {
            if !connected.contains(&id) || rejected.contains(&id) { continue; }
            let accepted = (|| -> Result<()> {
                match bytes.first() {
                    // Messages already in flight belong to the departed world.
                    Some(3 | 13 | 14) | Some(&realm::CROSS_GATE) | Some(&coherence::ACK)
                        if outputs.get(&id).is_some_and(|o| !o.ready) => {},
                    Some(&coherence::ACK) => {
                        let output = outputs.get_mut(&id).context("Missing image recipient")?;
                        let terrain = output.coherence.acknowledge(coherence::read_ack(&bytes)?, output.image.is_none())?;
                        if terrain { output.terrain.acknowledge()?; }
                    }
                    Some(3) if outputs.get(&id).is_some_and(|o| o.ready) => {
                        let mut controls: Controls = decode(&bytes[1..])?;
                        controls.validate()?;
                        let actor = actors.get_mut(&id).context("missing player")?;
                        ensure!(
                            actor.last_sequence.checked_add(1) == Some(controls.seq),
                            "input sequence mismatch"
                        );
                        actor.last_sequence = controls.seq;
                        let elapsed = now.duration_since(actor.last_input).as_secs_f32().clamp(1.0 / 30.0, 1.0);
                        if controls.damage > 0.0 && !actor.creative {
                            let health = &mut actor.personal.adventure.health;
                            *health = (*health - controls.damage).max(0.0);
                        }
                        if let Some((position, velocity)) = controls.body {
                            if actor.awaiting && !actor.correction_pending && controls.ack == actor.correction {
                                actor.awaiting = false;
                            }
                            if !actor.awaiting && actor.personal.ride.is_none() {
                                let position = Vec3::from_array(position);
                                if let Some(movement) = admit_owner_move(world, &actor.player, position, elapsed, actor.creative) {
                                    actor.player.pos = position;
                                    actor.player.precise_pos = position.as_dvec3();
                                    actor.player.vel = Vec3::from_array(velocity);
                                    if world.world_type == WorldType::Manyfold
                                        && !engine_worldgen::manyfold::flat_maze(world.generator_version)
                                    {
                                        actor.player.gravity_frame = controls.gravity_frame;
                                    }
                                    if matches!(movement, OwnerMove::Portal) {
                                        actor.portal_crossings = actor.portal_crossings.wrapping_add(1);
                                    }
                                } else {
                                    actor.queue_correction();
                                }
                            }
                        }
                        // Preserve one-shot input until the simulation has consumed it.
                        controls.interact |= actor.controls.interact;
                        controls.cycle |= actor.controls.cycle;
                        controls.use_secondary |= actor.controls.use_secondary;
                        actor.pulses.record(&controls);
                        actor.controls = controls;
                        actor.last_input = now;
                    }
                    Some(&realm::CROSS_GATE) if bytes.len() == 1 && outputs.get(&id).is_some_and(|o| o.ready) => {
                        self.crossings.entry(id).or_insert(false);
                    }
                    Some(18) if actors.contains_key(&id) => {
                        let text = std::str::from_utf8(&bytes[1..])?;
                        ensure!(text.len() <= engine_net::protocol::MAX_TEXT_BYTES, "chat line too long");
                        if self.typed.len() < 64 {
                            self.typed.push((id, text.to_owned()));
                        }
                    }
                    Some(10) if !actors.contains_key(&id) && !self.join_profiles.contains_key(&id) => {
                        let token:String=decode(&bytes[1..])?;
                        ensure!(gameplay::valid_profile(&token),"invalid player profile");
                        // A proven key names the player; a keyless transport keys a namespaced token,
                        // so a token can never name another player's key profile.
                        let profile=profiles::registry_key(socket.fingerprint(id),socket.has_keys(),&token)?;
                        ensure!(!actors.values().any(|a|a.profile==profile) && !self.join_profiles.values().any(|p|p==&profile),"profile already connected");
                        ensure!(self.profiles.contains_key(&profile) || self.profiles.len() + actors.len() + self.join_profiles.len() < profiles::MAX_PROFILES, "player registry is full");
                        self.join_profiles.insert(id,profile);
                    }
                    Some(13|14) if outputs.get(&id).is_some_and(|o|o.ready) => {
                        let actor=actors.get_mut(&id).context("missing command actor")?;
                        let mut payload=bytes.clone();payload[0]-=12;
                        if payload[0]==1 {ensure!(payload.len()==13 && u32::from_le_bytes(payload[1..5].try_into()?)<=65536,"command transfer budget");}
                        if let Some(bytes)=actor.request_rx.receive(&payload)? {
                            let request:actions::Request=gameplay::unpack_limit(&bytes,65536)?;
                            if !actions::admit_sequence(actor.command_sequence, request.sequence)? { return Ok(()); }
                            actor.personal.swap(world);
                            let respawn=matches!(request.command,actions::Command::Respawn);
                            let result=if respawn {
                                world::adventure::death::respawn(world,&mut actor.player);Ok(())
                            } else {actions::apply(request.command,world,&actor.player,day,id,usize::from(actor.controls.slot),actor.creative,&mut actor.tools,&mut actor.history,blocks,chunks)};
                            if let Err(error)=result {world.expedition.say(error.to_string());}
                            actor.personal.swap(world);
                            // A respawn moves the body, which the joiner owns: send it there.
                            if respawn {actor.queue_correction();}
                            actor.command_sequence=request.sequence;
                            factory_image.invalidate();
                        }
                    }
                    Some(&replication::RESEND_WATER) if bytes.len() == 1 => {
                        // Throttled: each request costs the host a full water list.
                        if let Some(output) = outputs.get_mut(&id)
                            && now.duration_since(output.water_resent) >= replication::REFRESH
                        {
                            output.mirror.resend_water();
                            output.water_resent = now;
                        }
                    }
                    Some(20) if bytes.len() == 5 => {
                        // Late or unknown echoes are normal after a stall and are ignored.
                        let micros = now.duration_since(self.epoch).as_micros() as u64;
                        if let Some(output) = outputs.get_mut(&id) {
                            output.probe.echo(u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]), micros);
                        }
                    }
                    Some(4) if bytes.len() == 1 => {
                        let output = outputs
                            .get_mut(&id)
                            .context("unexpected snapshot acknowledgement")?;
                        ensure!(
                            output.transfer.is_none() && !output.ready,
                            "snapshot acknowledgement out of order"
                        );
                        output.ready = true;
                        let mut board = vec![21];
                        board.extend(scoreboard::encode(&self.board.board().snapshot()));
                        if board.len() <= 900 {
                            let _ = socket.send_data(id, board);
                        }
                        let mut rules = vec![23];
                        rules.extend(world.rules.encode());
                        let _ = socket.send_data(id, rules);
                        let _ = socket.send_data(id, status::border_message(world.rules.border_m));
                        eprintln!("[multiplayer] player_ready id={id}");
                    }
                    _ => anyhow::bail!("unexpected application message"),
                }
                Ok(())
            })();
            if let Err(error) = accepted {
                eprintln!("player {id}: {error}");
                socket.disconnect(id);
                rejected.insert(id);
            }
        }

        let connected_peer = |id: &PlayerId| connected.contains(id) && !rejected.contains(id);
        let departed:Vec<_>=actors.keys().filter(|id|!connected_peer(id)).copied().collect();
        for id in departed {let _ = self.board.change(Change::Clear { player: id }); if let Some(actor)=actors.remove(&id) {
            // A fresh profile that never finished joining is not kept, so
            // connect-and-leave churn cannot fill the 128-profile registry.
            if actor.restored || outputs.get(&id).is_some_and(|o|o.ready) {
                self.profiles.insert(actor.profile.clone(),gameplay::save_player(&actor.player,&actor.personal)?);
                self.profiles_dirty = true;
            }
        }}
        self.join_profiles.retain(|id,_|connected_peer(id));
        self.joined_at.retain(|id,_|connected_peer(id));
        for &id in connected.iter().filter(|id| connected_peer(id)) {
            let since=now.duration_since(*self.joined_at.entry(id).or_insert(now));
            if since>=PROFILE_TIMEOUT && !actors.contains_key(&id) && !self.join_profiles.contains_key(&id) {
                eprintln!("player {id}: no profile within {}s",PROFILE_TIMEOUT.as_secs());
                socket.disconnect(id);
            }
        }
        outputs.retain(|id, _| connected_peer(id));
        world.travel.riders.retain(|id, _| *id == HOST || actors.contains_key(id));
        if world.travel.follow_owner.is_some_and(|id| id != HOST && !actors.contains_key(&id)) {
            world.travel.following = None;
            world.travel.follow_owner = None;
        }
        for &id in connected.iter().filter(|id| connected_peer(id)) {
            if actors.contains_key(&id) {
                continue;
            }
            let Some(profile)=self.join_profiles.remove(&id) else {continue;};
            // A profile saved by an older build may not decode. That player starts fresh;
            // one unreadable profile must not end the session for everyone.
            let restored=match self.profiles.get(&profile).map(|b|gameplay::load_player(b)).transpose() {
                Ok(restored)=>restored,
                Err(error)=>{eprintln!("[multiplayer] profile for player {id} unreadable, starting fresh: {error:#}");None}
            };
            let fresh=restored.is_none();
            let mut actor=if let Some(saved)=restored {let mut a=Actor::new(saved.player,now);a.personal=saved.personal;a} else {Actor::new(Player::new(),now)};
            actor.profile=profile;
            actor.restored = !fresh;
            actor.creative=self.creative;
            let mut remote = actor.player;
            if fresh {
            remote.pos = player.pos;
            // Eight directions on two rings, skipping spots another player stands on.
            let metres = world.voxel_scale.meters();
            let candidates = [1.5f32, 3.0].into_iter().flat_map(|ring| {
                (0..8).map(move |i| {
                    let angle = i as f32 * std::f32::consts::FRAC_PI_4;
                    Vec3::new(angle.cos(), 0.0, angle.sin()) * ring
                })
            });
            for offset in candidates {
                let candidate = player.pos + offset / metres;
                let taken = actors.values().any(|a| (a.player.pos - candidate).length() * metres < 1.0);
                let (lo, hi) =
                    world::player_aabb(candidate, world.voxel_scale, remote.scale, 0.0);
                if !taken && !world::collides_world(world, lo, hi) {
                    remote.pos = candidate;
                    break;
                }
            }
            remote.precise_pos = remote.pos.as_dvec3();
            }
            actor.player=remote;
            if fresh {actor.personal.adventure.begin_journey();}
            actor.personal.swap(world);
            if fresh {world.ensure_starter_wand();world.sync_from_inventory();}
            if fresh && self.headless && std::env::args().any(|arg| arg == "--multiplayer-review-travel") {
                travel_review::equip(world);
            }
            let snapshot_result=make_snapshot(world,&actor.player,day,id);
            actor.personal.swap(world);
            let snapshot = match snapshot_result {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("player {id} synchronization refused: {error}");
                    socket.disconnect(id);
                    continue;
                }
            };
            self.profiles.remove(&actor.profile);
            eprintln!(
                "[multiplayer] snapshot id={id} deferred_chunks={}",
                snapshot.deferred.len()
            );
            outputs.insert(
                id,
                Output {
                    contacts: Default::default(),
                    sounds: Default::default(),
                    transfer: Some(snapshot.sender), terrain: realm::Terrain::new(snapshot.deferred), image:None,image_due:now,
                    cells: BTreeMap::new(),
                    coherence: Default::default(),
                    pace: pacing::Pacer::new(pacing::TRANSFER_BYTES_PER_SECOND),
                    ready: false,
                    started: now,
                    mirror: replication::Mirror::default(),
                    refreshed: now,
                    water_resent: now,
                    probe: EchoProbe::default(),
                },
            );
            actors.insert(id, actor);
        }
        let mut changes = BTreeMap::new();
        let mut local_input=input.clone();
        local_input.cast_pressed|=self.pending_input.cast_pressed;
        local_input.cycle_spell|=self.pending_input.cycle_spell;
        local_input.interact|=self.pending_input.interact;
        local_input.dash_pressed|=self.pending_input.dash_pressed;
        let mut local = Controls::capture(1, player, &local_input);local.slot=self.hand as u8;
        local.mine|=self.pending_mine;local.place|=self.pending_place;
        local.use_secondary |= self.pending_place;
        while let Some(command)=self.commands.pop_front() {
            factory_image.invalidate();
            if matches!(command,actions::Command::Respawn) {world::adventure::death::respawn(world,player);continue;}
            if let Err(error)=actions::apply(command,world,player,day,HOST,self.hand,self.creative,&mut self.tools,history,blocks,chunks) {world.expedition.say(error.to_string());}
        }
        // One wall budget for every catch-up step this frame, as in single-player:
        // a fresh budget per step let a busy water front lengthen the frame it was in.
        let mut budget=None;
        let mut blast_cells_left = 384;
        let mut sound_events = Vec::new();
        // Expensive world work must not trigger an unbroken burst of
        // catch-up steps before input, edit publication and transport
        // can run again. Keep a short debt without growing it forever.
        *accumulator = (*accumulator + dt.min(0.1)).min(0.1);
        let catch_up_started = Instant::now();
        let mut stepped = false;
        let limit = (*simulation_left).min(quantum);
        while *accumulator >= STEP
            && ((!stepped && guaranteed_step) || catch_up_started.elapsed() < limit)
        {
            stepped = true;
            *accumulator -= STEP;
            // Empty dedicated worlds pause. Admission and transport polling
            // continue, but no terrain, factories or AI consume idle CPU.
            if self.headless && !outputs.values().any(|output|output.ready) {
                *accumulator = 0.0;
                break;
            }
            let mut targets=Vec::new();
            let mut target=|id,body:&Player,alive| {let (lo,hi)=body.bounds(world.voxel_scale);
                targets.push(gameplay::Target{id,feet:body.pos*world.voxel_scale.meters(),min:lo*world.voxel_scale.meters(),max:hi*world.voxel_scale.meters(),alive});};
            if !self.headless {target(HOST,player,!world.adventure.is_dead());}
            for (&id,actor) in actors.iter() {if outputs.get(&id).is_some_and(|o|o.ready){target(id,&actor.player,!actor.personal.adventure.is_dead());}}
            *creature_tick = creature_tick.wrapping_add(1);
            world.multiplayer=Some(gameplay::ContextFrame{actor:HOST,players:targets,shared_tick:true,damage:Vec::new(),contacts:Vec::new(),creature_tick:*creature_tick});
            world::factory::tick_power(world, STEP);
            let mut first=true;
            if self.headless {
                authority::blast_step(world, local_spell_edits, simulation, &mut blast_cells_left, blocks, chunks);
            } else {
                player.glide.wind=crate::player::glide::weather_wind(world, day, player.pos);
                authority::step(world,player,&local,&self.tools,self.creative,false,local_cooldown,local_mining,history,STEP,&mut changes,blocks,chunks,simulation,local_edits);
                world.travel.track_rider(HOST);
                authority::blast_step(world, local_spell_edits, simulation, &mut blast_cells_left, blocks, chunks);
                audio_events::collect(world, player.pos, &mut sound_events);
                first=false;
            }
            local.cycle=false;local.interact=false;local.dash=false;local.use_secondary=false;
            self.pending_input=Input::default();self.pending_mine=false;self.pending_place=false;
            for (&id,actor) in actors.iter_mut() {
                if !outputs.get(&id).is_some_and(|o|o.ready){continue;}
                if let Some(ctx)=world.multiplayer.as_mut(){ctx.actor=id;ctx.shared_tick=first;}
                actor.personal.swap(world);
                let was_riding = world.travel.riding.is_some();
                let fresh = now.duration_since(actor.last_input) < Duration::from_millis(250);
                let mut controls=if fresh {actor.controls.clone()}
                    else {Controls{slot:actor.controls.slot,yaw:actor.player.yaw,pitch:actor.player.pitch,gravity_frame:actor.player.gravity_frame,..Default::default()}};
                let pulses = std::mem::take(&mut actor.pulses);
                if fresh { pulses.apply(&mut controls); }
                controls.flying &= actor.creative;
                // Joiners move their own bodies; the host runs only their actions.
                authority::step(world,&mut actor.player,&controls,&actor.tools,actor.creative,true,&mut actor.edit_cooldown,&mut actor.mining,&mut actor.history,STEP,&mut changes,blocks,chunks,simulation,&mut actor.edits);
                world.travel.track_rider(id);
                authority::blast_step(world, &mut actor.spell_edits, simulation, &mut blast_cells_left, blocks, chunks);
                audio_events::collect(world, actor.player.pos, &mut sound_events);
                actor.controls.cycle=false;actor.controls.interact=false;actor.controls.dash=false;actor.controls.use_secondary=false;
                actor.personal.swap(world);first=false;
                if was_riding && actor.personal.ride.is_none() { actor.queue_correction(); }
            }
            // One world pass, independent of player count. The simulation's
            // active chunk set includes every connected player's vicinity.
            let anchor=if self.headless {actors.values().next().map(|a|&a.player).unwrap_or(&*player)} else {&*player};
            crate::simulation::fixed::finish(simulation,world,anchor,day,blocks,chunks,
                *budget.get_or_insert_with(||crate::simulation::WorldSimulation::live_budget(world)),false,true);
            if let Some(error)=&simulation.authority_error {anyhow::bail!("world simulation stopped: {error}");}
            // World-step cues (weather, machines, workshop milestones)
            // also exist on dedicated hosts, outside any mounted actor.
            audio_events::collect(world, anchor.pos, &mut sound_events);
            let context=world.multiplayer.take();
            let (hits,contacts)=context.map(|context|(context.damage,context.contacts)).unwrap_or_default();
            audio_events::contacts(world,player,actors,outputs,self.headless,contacts,&mut self.contacts);
            combat::apply(world, player, actors, hits, self.headless);
            // Shared dropped items fall only on the host.
            let mut drops=std::mem::take(&mut world.inventory.dropped);
            for drop in &mut drops {let q=drop.pos-Vec3::Y*(STEP*2.5);if world.get((q/world.voxel_scale.meters()).floor().as_ivec3()).is_empty(){drop.pos=q;}}
            world.inventory.dropped=drops;
        }
        *simulation_left = simulation_left.saturating_sub(catch_up_started.elapsed());
        audio_events::distribute(world, player, actors, outputs, self.headless, sound_events);
        audio_events::steps(world, player, actors, outputs, self.headless, footsteps, &mut self.contacts);
        // Merge all writers before freezing each recipient's next terrain frame.
        for cell in std::mem::take(&mut world.network_changes) {
            changes.insert(cell.to_array(),world.get(cell).as_u32());blocks.insert(cell);world::mark_chunk_and_neighbors_dirty(chunks,cell);
        }
        let mut preparation_failed = HashSet::new();
        for (&id, output) in outputs.iter_mut() {
            let result = (|| -> Result<()> {
                for (&p, &b) in &changes {
                    if !output.terrain.defer_edit(p, b) { output.cells.insert(p, b); }
                }
                output.terrain.poll()?;
                if output.ready && network_tick && let Some(actor) = actors.get(&id) {
                    output.terrain.start(world, actor.player.pos, &mut output.cells)?;
                }
                ensure!(output.cells.len() <= MAX_EDITS, "synchronization cell budget exceeded");
                output.coherence.check_timeout(now)?;
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("player {id} image preparation refused: {error}");
                preparation_failed.insert(id);
            }
        }
        // Each recipient gets its own character plus every other actor's
        // visible effects, from the same authoritative world.
        // Serialize at most one image per rendered frame, oldest due
        // first. Multiple friends must not multiply one frame's spike.
        let recipients:Vec<_>=outputs.iter().filter(|(id,o)|!preparation_failed.contains(id)&&o.ready&&o.image.is_none()&&o.coherence.can_capture()&&now>=o.image_due)
            .min_by_key(|(_,o)|o.image_due).map(|(&id,_)|id).into_iter().collect();
        for id in recipients {
            factory_image.refresh(world, now)?;
            let Some(mut actor)=actors.remove(&id) else {continue;};
            let Some(output)=outputs.get_mut(&id) else {actors.insert(id,actor);continue;};
            let terrain = replication::terrain::Frame {
                chunk: output.terrain.take(&mut output.cells),
                cells: std::mem::take(&mut output.cells).into_iter().collect(),
            };
            let sequence = output.coherence.next_sequence()?;
            actor.personal.swap(world);
            let listener=actor.player.pos;
            let near=|position:Vec3| (position-listener).length_squared()*world.voxel_scale.meters().powi(2)<=96.0*96.0;
            let listener_m=listener*world.voxel_scale.meters();
            let mut others:Vec<_>=actors.values().filter(|other|near(other.player.pos)
                ||other.personal.adventure.projectiles.iter().any(|shot|shot.pos.distance_squared(listener_m)<=96.0*96.0))
                .map(|a|&a.personal.adventure).collect();
            if !self.headless && near(player.pos) {others.push(&actor.personal.adventure);}
            if now.duration_since(output.refreshed)>=replication::FULL_REFRESH {output.mirror.refresh();output.refreshed=now;}
            let image=replication::capture_frame(world,&actor.player,day,actor.command_sequence,others,&mut output.mirror,factory_image,sequence,&terrain);
            actor.personal.swap(world);actors.insert(id,actor);
            match image {Ok(bytes)=>{output.image=Some(Sender::new(bytes)?);output.coherence.begin(sequence, terrain.chunk.is_some(), now)?;output.image_due=now+Duration::from_millis(100);},
                Err(error)=>{eprintln!("player {id} gameplay image refused: {error}");socket.disconnect(id);}}
        }
        self.remotes.retain(|id, _| actors.contains_key(id));
        world.adventure.remote_companions.clear();
        if !self.headless {
            let viewer = player.pos * world.voxel_scale.meters();
            world.adventure.remote_companions.extend(actors.values()
                .filter_map(|actor| actor.personal.adventure.companion.as_ref())
                .filter(|cap| cap.is_rescued() && cap.pos.distance_squared(viewer) <= 96.0 * 96.0)
                .cloned());
        }
        for (&id, actor) in actors.iter() {
            let pose = actor.pose(id);
            self.remotes
                .entry(id)
                .and_modify(|r| r.receive(pose.clone(), world.voxel_scale.meters()))
                .or_insert_with(|| Remote::new(pose));
        }
        // A subject has the same pose for every nearby recipient this tick.
        // Keep encoding lazy so a subject nobody can see costs no packet work.
        let sent_at = network_tick.then(smoothing::clock_s);
        let host_pose = (network_tick && !self.headless)
            .then(|| Pose::from_player(0, player).with_combat(&world.adventure));
        let mut pose_packets = BTreeMap::<PlayerId, Vec<u8>>::new();
        let mut drop_peers: Vec<_> = preparation_failed.into_iter().collect();
        // Bulk messages use only the shared link slots realtime messages do not need.
        for (&id, output) in outputs.iter_mut() {
            if drop_peers.contains(&id) { continue; }
            let flushed = (|| -> Result<()> {
                if output.cells.len() > MAX_EDITS
                    || (!output.ready && now.duration_since(output.started) > SYNC_TIMEOUT)
                {
                    anyhow::bail!("synchronization budget exceeded");
                }
                if output.ready && let Some(actor) = actors.get_mut(&id)
                    && !actor.flush_correction(socket, id)?
                {
                    return Ok(());
                }
                // Realtime first, so a loaded link delays bulk data and never poses or the clock.
                if output.ready && let Some(sent) = sent_at {
                    audio_events::flush(socket, id, output)?;
                    let listener=actors.get(&id).map_or(player.pos,|actor|actor.player.pos);
                    let near=|position:Vec3| (position-listener).length_squared()*world.voxel_scale.meters().powi(2)<=96.0*96.0;
                    let roster = Roster {
                        ids: (!self.headless&&near(player.pos)).then_some(0).into_iter()
                            .chain(actors.iter().filter(|(_,actor)|near(actor.player.pos)).map(|(&id,_)|id)).collect(),
                        time: day.time_of_day(),
                        day: day.day_index(),
                    };
                    // Roster entries can include players still loading; their pose is stable at spawn.
                    let _ = socket.send_data(id, encode(7, &roster)?).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    {
                        let host = host_pose.iter().filter(|_| near(player.pos));
                        let peers = actors.iter()
                            .filter(|(subject, actor)| **subject != id && near(actor.player.pos))
                            .filter_map(|(subject, _)| self.remotes.get(subject).map(|remote| &remote.pose));
                        for pose in host.chain(peers)
                        {
                            let packet = match pose_packets.entry(pose.id) {
                                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                                std::collections::btree_map::Entry::Vacant(entry) => {
                                    entry.insert(encode(6, &Pose { sent, ..pose.clone() })?)
                                }
                            };
                            socket.send_pose(id, packet.clone()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        }
                    }
                }
                let ready = output.ready;
                output.pace.set_rate(if ready { pacing::BULK_BYTES_PER_SECOND } else { pacing::TRANSFER_BYTES_PER_SECOND });
                let Output { transfer, image, pace, .. } = output;
                let mut room = bulk_per_peer;
                let send = |bytes: Vec<u8>| socket.send_bulk(id, bytes).map_err(|e| anyhow::anyhow!(e.to_string()));
                pace.drain(now, &mut room, |lane| {
                    let sender = match lane {
                        pacing::Lane::Deltas => return Ok(None),
                        pacing::Lane::Image => image.as_mut().filter(|_| ready),
                        pacing::Lane::Bulk => transfer.as_mut().filter(|_| !ready),
                    };
                    let Some(sender) = sender else { return Ok(None) };
                    let Some(mut payload) = sender.next() else { return Ok(None) };
                    // The image reuses the transfer framing on tags 11 and 12.
                    if lane == pacing::Lane::Image {
                        payload[0] += 10;
                    }
                    let size = payload.len();
                    if !send(payload)? {
                        return Ok(None);
                    }
                    sender.accepted();
                    Ok(Some(size))
                })?;
                if transfer.as_ref().is_some_and(|t| t.next().is_none()) {
                    *transfer = None;
                }
                if image.as_ref().is_some_and(|t| t.next().is_none()) {
                    *image = None;
                }
                Ok(())
            })();
            if let Err(error) = flushed {
                eprintln!("player {id} dropped: {error}");
                drop_peers.push(id);
            }
        }
        for id in drop_peers {
            socket.disconnect(id);
        }
        // Joiners gate their own tools and flight on the host's rules.
        // The border rides after the rules bytes, so either change resends both.
        let mut rules = world.rules.encode();
        rules.extend_from_slice(&world.rules.border_m.to_le_bytes());
        if network_tick && rules != *sent_rules {
            let mut bytes = vec![23];
            bytes.extend_from_slice(&rules[..rules.len() - 4]);
            let border = status::border_message(world.rules.border_m);
            for (&id, _) in outputs.iter().filter(|(_, o)| o.ready) {
                let _ = socket.send_data(id, bytes.clone());
                let _ = socket.send_data(id, border.clone());
            }
            *sent_rules = rules;
        }
        if network_tick {
            let mut local = std::mem::take(&mut self.typed);
            if !self.headless {
                local.extend(self.social.take_outbox().into_iter().map(|line| (HOST, line)));
            }
            local.extend(self.console.drain(..).map(|line| (HOST, line)));
            let host_name = self.name.clone();
            let host_creative = self.creative;
            let creative = |id: PlayerId| if id == HOST { host_creative } else { actors.get(&id).is_some_and(|a| a.creative) };
            let ctx = host_commands::Context {
                seed: u64::from(world.seed),
                time: day.time_of_day(),
                host_name: (!self.headless).then_some(host_name.as_str()),
                creative: &creative,
                can_save: self.save_path.is_some(),
            };
            let (deferred, mut shown) = host_commands::run(socket, &ctx, local, &mut self.budgets, &mut self.name);
            let refresh = !deferred.is_empty() || self.name != host_name;
            for effect in deferred {
                match effect {
                    Deferred::Gamemode(id, on) if id == HOST => {
                        self.creative = on;
                        player.flying &= on;
                    }
                    Deferred::Gamemode(id, on) => {
                        if let Some(actor) = actors.get_mut(&id) {
                            actor.creative = on;
                            actor.player.flying &= on;
                        }
                    }
                    Deferred::Teleport { player: who, to } => {
                        let Some(at) = (if to == HOST { Some(player.pos) } else { actors.get(&to).map(|a| a.player.pos) }) else { continue };
                        // Beside the target, not inside it.
                        let at = at + Vec3::X * (0.8 / world.voxel_scale.meters());
                        let body = if who == HOST { Some(&mut *player) } else { actors.get_mut(&who).map(|a| &mut a.player) };
                        if let Some(body) = body {
                            body.pos = at;
                            body.precise_pos = at.as_dvec3();
                            body.vel = Vec3::ZERO;
                        }
                        if let Some(actor) = actors.get_mut(&who) {
                            actor.queue_correction();
                        }
                    }
                    Deferred::SetTime(time) => {
                        let eye = player.pos.as_dvec3() * f64::from(world.voxel_scale.meters());
                        crate::planet::set_local_time(world.world_type, eye, day, time);
                    }
                    Deferred::World { actor, target, change } => {
                        let reply = if target == HOST {
                            let body = (!self.headless).then(|| crate::world_commands::Body { player: &mut *player, creative: self.creative });
                            crate::world_commands::apply(&change, world, body, day)
                        } else if let Some(remote) = actors.get_mut(&target) {
                            // Mounted, so give and the death check read the joiner's own state.
                            remote.personal.swap(world);
                            let body = crate::world_commands::Body { player: &mut remote.player, creative: remote.creative };
                            let reply = crate::world_commands::apply(&change, world, Some(body), day);
                            remote.personal.swap(world);
                            if reply.is_ok() && matches!(change, engine_net::commands::WorldChange::TeleportTo(_)) {
                                remote.queue_correction();
                            }
                            reply
                        } else {
                            Err("That player left".to_owned())
                        };
                        let text = match reply { Ok(text) | Err(text) => text };
                        host_commands::tell(socket, (!self.headless).then_some(self.name.as_str()), actor, text, &mut shown);
                    }
                    other => late.push(other),
                }
            }
            self.social.learn_names(host_commands::names(socket, (!self.headless).then_some(self.name.as_str())));
            for (kind, from, text) in shown {
                self.social.push(kind, from, &text);
            }
            if refresh || now >= self.roster_due {
                self.roster_due = now + Duration::from_secs(1);
                let host_creative = self.creative;
                let host_name = self.name.clone();
                let creative = |id: PlayerId| if id == HOST { host_creative } else { actors.get(&id).is_some_and(|a| a.creative) };
                let ctx = host_commands::Context {
                    seed: u64::from(world.seed),
                    time: day.time_of_day(),
                    host_name: (!self.headless).then_some(host_name.as_str()),
                    creative: &creative,
                    can_save: self.save_path.is_some(),
                };
                let mut rows = host_commands::roster(socket, &ctx);
                // A peer is listed once it has a name and a world, not while it loads.
                rows.retain(|r| r.id == HOST || outputs.get(&r.id).is_some_and(|o| o.ready));
                let wire: Vec<(PlayerId, String, u8)> =
                    rows.iter().map(|r| (r.id, r.name.clone(), u8::from(r.op) | u8::from(r.creative) << 1)).collect();
                let bytes = encode(16, &wire)?;
                for (&id, output) in outputs.iter() {
                    if output.ready {
                        let _ = socket.send_data(id, bytes.clone());
                    }
                }
                self.social.own = (!self.headless).then_some(HOST);
                let micros = now.duration_since(self.epoch).as_micros() as u64;
                let mut pings = Vec::new();
                for (&id, output) in outputs.iter_mut().filter(|(_, o)| o.ready) {
                    let mut probe = vec![19];
                    probe.extend(output.probe.probe(micros).to_le_bytes());
                    let _ = socket.send_data(id, probe);
                    pings.push((id, output.probe.quality(micros).encode()));
                }
                self.social.quality = pings.iter().filter_map(|&(id, q)| Some((id, Quality::decode(q)?))).collect();
                let ping_bytes = encode(22, &pings)?;
                if self.board.board().shown(Slot::List).is_none() {
                    define_objectives(&mut self.board)?;
                }
                let metres = world.voxel_scale.meters();
                let mut bodies: Vec<(PlayerId, f32)> = actors.iter().filter(|(id, _)| outputs.get(id).is_some_and(|o| o.ready)).map(|(&id, a)| (id, a.player.pos.y)).collect();
                if !self.headless {
                    bodies.push((HOST, player.pos.y));
                }
                for (id, y) in bodies {
                    let since = if id == HOST { now.duration_since(self.epoch) } else { self.joined_at.get(&id).map_or(Duration::ZERO, |t| now.duration_since(*t)) };
                    self.board.set(PLAYED, id, since.as_secs() as i64).map_err(|e| anyhow::anyhow!(e.0))?;
                    let lowest = (y * metres).floor() as i64;
                    if self.board.board().score(LOWEST, id).is_none_or(|v| lowest < v) {
                        self.board.set(LOWEST, id, lowest).map_err(|e| anyhow::anyhow!(e.0))?;
                    }
                }
                let flush = self.board.flush();
                for (&id, output) in outputs.iter() {
                    if output.ready {
                        let _ = socket.send_data(id, ping_bytes.clone());
                        if !flush.is_empty() && flush.len() < 900 {
                            let mut bytes = vec![21];
                            bytes.extend_from_slice(&flush);
                            let _ = socket.send_data(id, bytes);
                        }
                    }
                }
                self.social.board = self.board.board().clone();
                self.social.set_roster(rows);
            }
        }
        Ok(())
    }
    pub fn append_players(&self, vertices: &mut Vec<crate::mesh::Vertex3>, scale: f32) {
        for (&id, remote) in &self.remotes {
            crate::mesh::models::append_network_player(
                vertices,
                scale,
                remote.shown,
                remote.shown_yaw,
                remote.shown_pitch,
                remote.phase,
                remote.stride,
                remote.shown_flying,
                remote.pose.swing,
                id,
                remote.shown_frame,
            );
        }
    }
}

enum OwnerMove {
    Continuous,
    Portal,
}

#[cfg(test)]
fn owner_move(world: &World, body: &Player, to: Vec3, elapsed: f32, free: bool) -> bool {
    admit_owner_move(world, body, to, elapsed, free).is_some()
}

/// Check a reported body against speed and swept collision. Manyfold also
/// admits portal moves after checking the host's entrance and landing.
fn admit_owner_move(world: &World, body: &Player, to: Vec3, elapsed: f32, free: bool) -> Option<OwnerMove> {
    use crate::player::movement::{DASH_SPEED, DASH_TIME, MOMENTUM_SPEED_CAP};
    let scale = world.voxel_scale.meters();
    let delta = (to - body.pos) * scale;
    if world.world_type == WorldType::Manyfold
        && !engine_worldgen::manyfold::flat_maze(world.generator_version)
    {
        let reach = MOMENTUM_SPEED_CAP * 1.25 * elapsed + DASH_SPEED * DASH_TIME + 1.0;
        if (free || delta.length() <= reach)
            && !crate::manyfold::weave::crosses(world, body, to)
            && crate::player::radial::admit_move(world, body, to)
        {
            return Some(OwnerMove::Continuous);
        }
        let portal_reach = if free { 64.0 } else { reach };
        return (crate::manyfold::weave::admit_owner(world, body, to, portal_reach)
            || crate::manyfold::maze_portal::admit(world, body, to, portal_reach))
            .then_some(OwnerMove::Portal);
    }
    let run = delta.with_y(0.0).length();
    // Metres. A quarter over the hardest cap plus one dash absorbs packet bunching.
    let reach = MOMENTUM_SPEED_CAP * 1.25 * elapsed + DASH_SPEED * DASH_TIME + 1.0;
    if !free && (run > reach || delta.y > 20.0 * elapsed + 2.0) {
        return None;
    }
    // A replica can trail an edit by a tick, so a thin overlap is allowed.
    let skin = Vec3::splat(0.2 / scale);
    // Swept in steps under half a voxel, so a fast report cannot pass a wall.
    let steps = ((to - body.pos).length() / 0.4).ceil().clamp(1.0, 256.0) as u32;
    (1..=steps).all(|i| {
        let at = body.pos.lerp(to, i as f32 / steps as f32);
        let (lo, hi) = world::player_aabb(at, world.voxel_scale, body.scale, body.crouch_t);
        !world::collides_world(world, lo + skin, hi - skin)
    }).then_some(OwnerMove::Continuous)
}

fn validate_cell(p: [i32; 3], b: u32) -> Result<()> {
    ensure!(
        p.iter().all(|v| v.unsigned_abs() <= 1_000_000) && b < BlockId::COUNT as u32,
        "invalid replicated cell"
    );
    Ok(())
}
fn make_snapshot(
    world: &mut World,
    player: &Player,
    day: &DayNightCycle,
    id: PlayerId,
) -> Result<realm::Baseline> {
    let center = world::chunk_coord_from_world_block(player.pos.floor().as_ivec3());
    let mut deferred = std::collections::BTreeSet::new();
    let mut snapshot = snapshot_filtered(world, player, day, id, |coord| {
        if (coord - center).abs().max_element() <= 1 {
            true
        } else {
            deferred.insert(coord.to_array());
            false
        }
    })?;
    snapshot.realm = Some((kind_code(world.world_type), scale_code(world.voxel_scale), world.seed, world.generator_version));
    snapshot.hell_gates = world.portals.hell.clone();
    ensure!(deferred.len() <= 262_144, "World terrain index exceeds transfer limit");
    snapshot.deferred_terrain = deferred.iter().copied().collect();
    let bytes=gameplay::pack(&snapshot)?;
    ensure!(bytes.len()<=engine_net::transfer::MAX_TRANSFER,"world snapshot exceeds transfer limit");
    Ok(realm::Baseline { sender: Sender::new(bytes)?, deferred })
}
fn snapshot_state(world:&mut World,player:&Player,day:&DayNightCycle,id:PlayerId)->Result<Snapshot> {
    snapshot_filtered(world, player, day, id, |_| true)
}
fn snapshot_filtered(world: &mut World, player: &Player, day: &DayNightCycle, id: PlayerId, mut include: impl FnMut(IVec3) -> bool) -> Result<Snapshot> {
    let snapshot = world.snapshot();
    let mut coords: Vec<_> = snapshot
        .chunks
        .keys()
        .chain(snapshot.regions.records.keys())
        .copied()
        .collect();
    coords.sort_by_key(|c| c.to_array());
    coords.dedup();
    coords.retain(|&coord| include(coord));
    ensure!(
        coords.len() <= MAX_CHUNKS,
        "world exceeds initial synchronization budget"
    );
    let mut chunks = Vec::new();
    for coord in coords {
        chunks.push(snapshot_chunk(&snapshot, coord)?);
    }
    Ok(Snapshot {
        realm: None,
        hell_gates: Vec::new(),
        deferred_terrain: Vec::new(),
        planet_settings: world.planet_settings,
        terrain_checksum: {
            let cfg=world.sample_config();
            engine_worldgen::geo::active(&cfg).then(|| engine_worldgen::geo::planet_for(&cfg).checksum)
        },
        gameplay:Some(gameplay::capture(world,player,day,0,Vec::new())?),
        spawn: Pose::from_player(id, player),
        chunks,
        time: day.time_of_day(),
        day: day.day_index(),
    })
}

fn snapshot_chunk(snapshot: &world::WorldSnapshot, coord: IVec3) -> Result<Chunk> {
    let chunk = if let Some(chunk) = snapshot.chunks.get(&coord) {
        chunk.clone()
    } else {
        snapshot.regions.load(coord)?.context("missing edited chunk")?.chunk
    };
    let mut runs: Vec<(u16, u32)> = Vec::new();
    for block in chunk.blocks.iter() {
        if let Some((count, id)) = runs.last_mut()
            && *id == block.as_u32() && *count < u16::MAX
        {
            *count += 1;
        } else {
            runs.push((1, block.as_u32()));
        }
    }
    Ok(Chunk { coord: coord.to_array(), runs })
}
fn apply_snapshot(
    world: &mut World,
    player: &mut Player,
    day: &mut DayNightCycle,
    snapshot: Snapshot,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> Result<()> {
    snapshot.spawn.validate()?;
    realm::validate_gates(&snapshot.hell_gates, world.voxel_scale)?;
    if let Some(settings) = snapshot.planet_settings {
        ensure!(settings.valid() && (world.world_type == WorldType::Planet
            && world.generator_version >= engine_voxel::CUSTOM_PLANET_VERSION || world.sample_config().hell_mirror()), "Invalid host planet recipe");
    }
    if let Some(expected)=snapshot.terrain_checksum {
        let mut cfg=world.sample_config();
        cfg.planet = snapshot.planet_settings;
        ensure!(engine_worldgen::geo::active(&cfg)
            && engine_worldgen::geo::planet_for(&cfg).checksum==expected,
            "generated terrain differs from host");
        eprintln!("[multiplayer] terrain checksum verified {expected:016x}");
    }
    ensure!(
        snapshot.chunks.len() <= MAX_CHUNKS && snapshot.time.is_finite(),
        "invalid initial world"
    );
    // Validate the whole payload before touching the live world.
    for chunk in &snapshot.chunks {
        validate_chunk(chunk)?;
    }
    world.planet_settings = snapshot.planet_settings;
    let mut refused = 0usize;
    for chunk in snapshot.chunks {
        let coord = IVec3::from_array(chunk.coord);
        let base = coord * 32;
        refused += usize::from(!replica::install_chunk(world, coord, &chunk.runs, blocks));
        for x in [0, 31] {
            for y in [0, 31] {
                for z in [0, 31] {
                    world::mark_chunk_and_neighbors_dirty(chunks, base + IVec3::new(x, y, z));
                }
            }
        }
    }
    if refused > 0 {
        ensure!(snapshot.realm.is_none(), "Could not prepare destination terrain");
        eprintln!("[multiplayer] replica could not load {refused} snapshot chunks");
    }
    snapshot.spawn.apply(player);
    world.portals.hell = snapshot.hell_gates;
    day.set_day_index(snapshot.day);
    day.set_time_of_day(snapshot.time);
    if let Some(bytes)=snapshot.gameplay {gameplay::unpack::<gameplay::Image>(&bytes)?.apply(world,player,day,blocks,chunks)?;}
    Ok(())
}

fn validate_chunk(chunk: &Chunk) -> Result<()> {
    ensure!(chunk.coord.iter().all(|v| v.unsigned_abs() <= 31_249), "chunk outside world range");
    let mut total = 0usize;
    for &(count, block) in &chunk.runs {
        ensure!(count > 0 && block < BlockId::COUNT as u32, "invalid chunk run");
        total += usize::from(count);
    }
    ensure!(total == 32 * 32 * 32, "invalid chunk length");
    Ok(())
}

fn snapshot_checksum(snapshot:&Snapshot,format:u32)->Result<u64> {
    if format==1 {
        #[derive(Serialize)]struct Legacy<'a>{spawn:&'a Pose,chunks:&'a Vec<Chunk>,time:f32,day:u32}
        Ok(engine_net::transfer::checksum(ron::to_string(&Legacy{spawn:&snapshot.spawn,chunks:&snapshot.chunks,time:snapshot.time,day:snapshot.day})?.as_bytes()))
    } else {Ok(engine_net::transfer::checksum(ron::to_string(snapshot)?.as_bytes()))}
}
