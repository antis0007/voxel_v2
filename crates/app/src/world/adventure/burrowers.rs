//! Burrowers: segmented worms, eels and serpents that live inside the ground.
//!
//! A burrower is an ordinary `Enemy` (health, statuses, spell hits, death) whose
//! position is its head. The body is a rope of segments that follows the head,
//! so a sinuous head path becomes an S-curved body. Four motions share one
//! steering core:
//! - `Burrow` swims through rock and falls ballistically across open caves;
//! - `Breach` does the same but launches out of the ground in tall arcs;
//! - `Slither` crawls along floors and climbs single steps;
//! - `Swim` stays inside liquid and leaps out of it to strike.
//!
//! Strains (element, palette, size ranges, motion, ornament, trail) are rows in
//! `assets/burrowers.ron`. Each body rolls its proportions from a positional
//! seed, so two worms of one strain differ. Bodies are transient like night
//! spawns: `BURROW_Z` site rows never write a site record and nothing is saved.
//!
//! Burrowers grinding through rock publish a camera tremor (`shake`), knock
//! grit off nearby cave surfaces and shake dust from the ceiling over the
//! player. Rock is never edited: tunnels close behind them.
use super::{Adventure, Cue, Enemy, MAX_ENEMIES, World, difficulty::Difficulty, sound_pitched};
use crate::mesh::models::Model;
use crate::world::{BlockId, chunk_coord_from_world_block};
use engine_core::hash::hash_u32;
use glam::{IVec3, Mat3, Quat, Vec3};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, Ordering};

/// Site row of every burrower; never written to the record table.
pub(super) const BURROW_Z: i32 = -33;
/// Burrowers sense footsteps through rock inside this range (metres).
const SENSE_M: f32 = 26.0;
/// Bodies further than this from the player are removed.
const DESPAWN_M: f32 = 80.0;
/// Tremor falls to zero at this distance from the nearest body part.
const TREMOR_RANGE_M: f32 = 30.0;
/// Upper bound of the summed tremor, metres of camera offset.
const TREMOR_MAX_M: f32 = 0.14;
/// Spawn ring around the player.
const RING_MIN_M: f32 = 14.0;
const RING_MAX_M: f32 = 28.0;
const SPAWN_ATTEMPTS: u32 = 5;

/// Camera tremor in metres, published for the frame thread.
static TREMOR_M: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(super) enum Motion {
    Burrow,
    Breach,
    Slither,
    Swim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(super) enum Ornament {
    Mandibles,
    Plates,
    Crystals,
    Caps,
    Frills,
    Spines,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(super) enum Trail {
    Grit,
    Embers,
    Frost,
    Spores,
    Bubbles,
    Motes,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(super) struct Strain {
    pub kind: u32,
    pub motion: Motion,
    pub hosts: Vec<BlockId>,
    pub weight: f32,
    pub segments: (u32, u32),
    pub radius_m: (f32, f32),
    pub taper: f32,
    pub speed: f32,
    pub lunge: f32,
    pub weave_m: f32,
    pub weave_hz: f32,
    pub turn: f32,
    pub bite: f32,
    pub rumble: f32,
    pub head: [f32; 3],
    pub body: [f32; 3],
    pub belly: [f32; 3],
    pub glow: [f32; 3],
    pub glow_strength: f32,
    pub tint_jitter: f32,
    pub ornament: Ornament,
    pub trail: Trail,
    /// Undercrown district (`District::index`) this strain lives in (any
    /// world with the v2 layout; spawning is runtime, not generation): it rises only inside that district's rooms, one body at
    /// a time, and nowhere else.
    #[serde(default)]
    pub district: Option<u8>,
    /// Seconds of pursuit before a hunter breaks off and withdraws; 0 never.
    #[serde(default)]
    pub patience_s: f32,
    /// Seconds a hunter stays away after breaking off.
    #[serde(default)]
    pub rest_s: f32,
    /// Hunts by sound: its sense range scales with how fast the player
    /// moves, so standing still or creeping slips it.
    #[serde(default)]
    pub hears: bool,
    /// Rock overhead (metres) needed where a body rises; keeps giants out of
    /// shallow ground and starter terrain.
    #[serde(default)]
    pub min_cover_m: f32,
    /// Seconds a lunge shows itself (dust at the player's feet, a warning
    /// groan) before the head can hurt.
    #[serde(default = "default_tell_s")]
    pub tell_s: f32,
}

fn default_tell_s() -> f32 {
    1.0
}

const STRAINS_RON: &str = include_str!("../../../../../assets/burrowers.ron");
pub(super) static STRAINS: std::sync::LazyLock<Vec<Strain>> = std::sync::LazyLock::new(|| {
    ron::from_str(STRAINS_RON).unwrap_or_else(|error| panic!("assets/burrowers.ron: {error}"))
});

pub(super) fn strain(kind: u32) -> Option<&'static Strain> {
    STRAINS.iter().find(|s| s.kind == kind)
}

pub(super) fn is_kind(kind: u32) -> bool {
    (80..=95).contains(&kind) && strain(kind).is_some()
}

pub(super) fn is_burrow_body(e: &Enemy) -> bool {
    e.site.z == BURROW_Z
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Roam,
    Stalk,
    Lunge,
    Dive,
    /// A hunter that pressed too long falls back to its lair and ignores the
    /// player until `rest_s` runs out.
    Withdraw,
}

/// Per-body state. Not replicated: a peer's replica draws a straight chain.
#[derive(Clone, Debug, Default)]
pub(crate) struct Body {
    segs: Vec<Vec3>,
    vel: Vec3,
    heading: Vec3,
    radius: f32,
    spacing: f32,
    weave: f32,
    tint: [f32; 3],
    seed: u32,
    ticks: u32,
    mode: Mode,
    timer: f32,
    cooldown: f32,
    goal: Vec3,
    phase: f32,
    air: f32,
    grind: f32,
    jolt: f32,
    bite_cd: f32,
    fx_clock: f32,
    /// Slither strike: 0 on the floor, 1 with the head reared up.
    rear: f32,
    /// Seconds spent pursuing since the last rest (hunters only).
    pressure: f32,
    /// How loud the player is to a hearing strain, 0..1, smoothed.
    noise: f32,
    /// Player feet last tick, for the hearing estimate.
    last_feet: Option<Vec3>,
    /// Rose in the open: it grazes and breaches but does not hunt unless hurt
    /// or the player comes within `WARY_REACH_M`.
    wary: bool,
    /// Seconds of lunge tell left; the head cannot bite until it runs out.
    tell: f32,
    /// Seconds until the next look at whether the player is under open sky.
    look: f32,
    /// The player stands under open sky (refreshed twice a second).
    exposed: bool,
}

/// A wary body turns on a player this close.
const WARY_REACH_M: f32 = 4.0;
/// Share of open-ground bodies that are wary.
const WARY_SHARE: f32 = 0.85;
/// Spawn passes (seconds) after a session starts before any body rises.
const SPAWN_GRACE_PASSES: u32 = 20;

/// Quietest the player can be to a hearing strain: standing still in reach
/// is still heard at this fraction of the sense range.
const QUIET: f32 = 0.2;
/// Player speed (m/s) heard at the full sense range.
const LOUD_SPEED: f32 = 6.0;

/// Effects a tick asks for; emitted after the enemy loop releases its borrow.
pub(super) enum Fx {
    Chip { at: Vec3, out: Vec3, color: [f32; 3] },
    Trail { at: Vec3, trail: Trail, glow: [f32; 3] },
    Burst { at: Vec3, color: [f32; 3], count: usize },
    Sound { cue: Cue, at: Vec3, gain: f32, pitch: f32 },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    passes: u32,
    /// Loud player acts (casting, mining, blasts) heard by `hears` strains;
    /// 0..1, decays over about two seconds.
    pub(crate) ruckus: f32,
    rumble_clock: f32,
    dust_clock: f32,
}

impl State {
    /// A loud act at `level` (0..1); the loudest recent act wins.
    pub(crate) fn make_noise(&mut self, level: f32) {
        self.ruckus = self.ruckus.max(level.clamp(0.0, 1.0));
    }

    pub(crate) fn settle_noise(&mut self, dt: f32) {
        self.ruckus = (self.ruckus - dt * 0.5).max(0.0);
    }
}

fn unit(h: u32) -> f32 {
    (h & 0xff_ffff) as f32 / 16_777_216.0
}

fn roll(seed: u32, salt: u32) -> f32 {
    unit(hash_u32(seed ^ salt.wrapping_mul(0x9e37_79b9)))
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Block at `p` (metres), or `None` when its chunk is not resident. Never let a
/// burrower read disk for a nonresident chunk.
fn block(world: &World, p: Vec3) -> Option<BlockId> {
    let s = world.voxel_scale.meters();
    let cell = (p / s).floor().as_ivec3();
    world.chunks.contains_key(&chunk_coord_from_world_block(cell)).then(|| world.get(cell))
}

/// True when `b` is the medium this motion travels inside.
fn inside(motion: Motion, b: BlockId) -> bool {
    match motion {
        Motion::Swim => b.is_liquid(),
        Motion::Burrow | Motion::Breach => b.is_solid() || b.is_liquid(),
        Motion::Slither => false,
    }
}

fn rock(b: BlockId) -> bool {
    b.is_solid() && !b.is_liquid()
}

fn turn_toward(from: Vec3, to: Vec3, max: f32) -> Vec3 {
    let (from, to) = (from.normalize_or(Vec3::X), to.normalize_or(Vec3::X));
    let angle = from.angle_between(to);
    if !angle.is_finite() || angle <= max {
        return to;
    }
    let q = Quat::from_rotation_arc(from, to);
    (Quat::IDENTITY.slerp(q, max / angle) * from).normalize_or(from)
}

/// Body frame: `z` forward, `y` up-ish (belly faces -y), `x` to the side.
fn frame(dir: Vec3) -> Quat {
    let z = dir.normalize_or(Vec3::Z);
    let x = Vec3::Y.cross(z).try_normalize().unwrap_or(Vec3::X);
    let y = z.cross(x);
    Quat::from_mat3(&Mat3::from_cols(x, y, z))
}

impl Body {
    fn new(strain: &Strain, seed: u32, head: Vec3, heading: Vec3) -> Self {
        let (lo, hi) = strain.segments;
        let count = lo + (hash_u32(seed ^ 0x51) % (hi.saturating_sub(lo) + 1));
        let radius = lerp(strain.radius_m.0, strain.radius_m.1, roll(seed, 2));
        let spacing = radius * 1.25;
        let heading = heading.normalize_or(Vec3::X);
        let j = strain.tint_jitter;
        let tint = [
            1.0 + (roll(seed, 3) * 2.0 - 1.0) * j,
            1.0 + (roll(seed, 4) * 2.0 - 1.0) * j,
            1.0 + (roll(seed, 5) * 2.0 - 1.0) * j,
        ];
        Self {
            segs: (0..count.max(3)).map(|i| head - heading * spacing * i as f32).collect(),
            vel: heading * strain.speed,
            heading,
            radius,
            spacing,
            weave: strain.weave_m * lerp(0.7, 1.3, roll(seed, 6)),
            tint,
            seed,
            goal: head,
            phase: roll(seed, 7) * TAU,
            cooldown: 1.5,
            ..Self::default()
        }
    }

    fn radius_at(&self, strain: &Strain, i: usize) -> f32 {
        let n = self.segs.len().max(2);
        let t = i as f32 / (n - 1) as f32;
        let profile = if t < 0.15 {
            0.92 + t / 0.15 * 0.13
        } else {
            lerp(1.05, strain.taper, ((t - 0.15) / 0.85).powf(1.3))
        };
        self.radius * profile
    }

    fn rng(&mut self, salt: u32) -> f32 {
        self.ticks = self.ticks.wrapping_add(1);
        roll(self.seed ^ self.ticks.wrapping_mul(0x85eb_ca6b), salt)
    }

    /// Rope follow: every segment keeps `spacing` behind the one before it.
    fn follow(&mut self) {
        for i in 1..self.segs.len() {
            let lead = self.segs[i - 1];
            let delta = self.segs[i] - lead;
            let length = delta.length();
            if length > self.spacing {
                self.segs[i] = lead + delta / length * self.spacing;
            }
        }
    }
}

/// One spawn pass near `feet`. Runs once per adventure spawn second.
pub(super) fn spawn(adv: &mut Adventure, world: &World, feet: Vec3) {
    adv.burrowers.passes = adv.burrowers.passes.wrapping_add(1);
    adv.enemies.retain(|e| !is_burrow_body(e) || {
        let d = e.pos - feet;
        d.length() < DESPAWN_M
    });
    if adv.difficulty == Difficulty::Peaceful || !world.rules.mob_spawning {
        return;
    }
    if adv.safety_guards(feet) {
        return;
    }
    // A fresh session (a load, a crossing, a headless test) starts quiet.
    if adv.burrowers.passes <= SPAWN_GRACE_PASSES {
        return;
    }
    let cap = if adv.difficulty == Difficulty::Hard { 3 } else { 2 };
    let alive = adv.enemies.iter().filter(|e| is_burrow_body(e)).count();
    let wild = adv.enemies.iter().filter(|e| e.habitat.is_none()).count();
    if alive >= cap || wild >= MAX_ENEMIES {
        return;
    }
    let s = world.voxel_scale.meters();
    let roofed = underground(world, feet + Vec3::Y * 1.6);
    let chunk = (feet / (s * crate::world::CHUNK_SIZE as f32)).floor().as_ivec3();
    let pass = hash_u32(world.seed ^ adv.burrowers.passes.wrapping_mul(0x9e37_79b9) ^ 0xb0_77)
        ^ hash_u32((chunk.x as u32).wrapping_mul(0x85eb_ca6b) ^ (chunk.z as u32).wrapping_mul(0xc2b2_ae35));
    // Underground, a body rises every few passes; in the open only surface
    // hunters (breachers in sand, swimmers in ponds) and rarely.
    let district = district_at(world, feet);
    let lair_strain = district.and_then(|d| STRAINS.iter().find(|st| st.district == Some(d)));
    let hunter_alive = lair_strain.is_some_and(|st| adv.enemies.iter().any(|e| is_burrow_body(e) && e.kind == st.kind));
    // A district with a resident hunter calls it up readily until it is out.
    let chance = if lair_strain.is_some() && !hunter_alive { 0.6 } else if roofed { 0.30 } else { 0.12 };
    if roll(pass, 1) > chance {
        return;
    }
    for attempt in 0..SPAWN_ATTEMPTS {
        let h = hash_u32(pass ^ attempt.wrapping_mul(0x27d4_eb2f));
        let angle = roll(h, 2) * TAU;
        let distance = lerp(RING_MIN_M, RING_MAX_M, roll(h, 3));
        let lift = lerp(-9.0, 4.0, roll(h, 4));
        let at = feet + Vec3::new(angle.cos() * distance, lift, angle.sin() * distance);
        let Some(host) = block(world, at) else { continue };
        let cover = cover_above(world, at, 48.0);
        let candidates: Vec<&Strain> = STRAINS
            .iter()
            .filter(|st| cover >= st.min_cover_m)
            .filter(|st| st.hosts.contains(&host) && (roofed || matches!(st.motion, Motion::Breach | Motion::Swim)))
            .filter(|st| match st.district {
                None => true,
                Some(d) => !hunter_alive && district == Some(d) && district_at(world, at) == Some(d),
            })
            .collect();
        let total: f32 = candidates.iter().map(|st| st.weight.max(0.0)).sum();
        if total <= 0.0 {
            continue;
        }
        let mut pick = roll(h, 5) * total;
        let Some(st) = candidates.iter().find(|st| {
            pick -= st.weight.max(0.0);
            pick <= 0.0
        }).or(candidates.last()) else { continue };
        // In the open a weight is also an absolute chance: a strain that is
        // the only candidate under grass must not rise at full rate.
        if !roofed && st.district.is_none() && roll(h, 6) > st.weight.min(1.0) {
            continue;
        }
        let head = match st.motion {
            Motion::Slither => match floor_above(world, at, 6.0) {
                Some(p) => p,
                None => continue,
            },
            _ => at,
        };
        let site = IVec3::new((head.x / s).floor() as i32, (head.z / s).floor() as i32, BURROW_Z);
        let menace = u8::from(adv.difficulty == Difficulty::Hard);
        let mut e = Enemy::new(site, adv.burrowers.passes, st.kind, head, menace);
        let toward = Vec3::new(feet.x - head.x, 0.0, feet.z - head.z);
        e.burrow = Body::new(st, h, head, toward.normalize_or(Vec3::X).rotate_axis(Vec3::Y, 1.2));
        e.burrow.wary = !roofed && st.district.is_none() && roll(h, 7) < WARY_SHARE;
        e.home = head;
        adv.enemies.push(e);
        return;
    }
}

/// Undercrown district whose room holds `m` (metres), from generator 302;
/// `None` outside the Undercrown and on older worlds.
fn district_at(world: &World, m: Vec3) -> Option<u8> {
    // Spawning is runtime state, not generation, so district hunters live in
    // every world with the v2 layout (whose district ids are stable).
    let plan = engine_worldgen::undercrown::plan(&world.sample_config());
    plan.v2.as_ref()?;
    plan.room_at(m).map(|room| room.id.district())
}

/// Metres of solid ground stacked above `p`, scanning up `span` metres.
fn cover_above(world: &World, p: Vec3, span: f32) -> f32 {
    let s = world.voxel_scale.meters();
    let steps = (span / s).ceil() as i32;
    let mut solid = 0;
    for i in 1..=steps {
        match block(world, p + Vec3::Y * (i as f32 * s)) {
            Some(b) if b.is_solid() => solid += 1,
            Some(_) => {}
            None => break,
        }
    }
    solid as f32 * s
}

fn underground(world: &World, eye: Vec3) -> bool {
    let s = world.voxel_scale.meters();
    let steps = (20.0 / s).ceil() as i32;
    let mut solid = 0;
    for i in 1..=steps {
        match block(world, eye + Vec3::Y * (i as f32 * s)) {
            Some(b) if b.is_solid() => solid += 1,
            Some(_) => {}
            None => break,
        }
    }
    solid as f32 * s >= 4.0
}

/// The first air cell resting on rock, scanning up from `from` (metres).
fn floor_above(world: &World, from: Vec3, span: f32) -> Option<Vec3> {
    let s = world.voxel_scale.meters();
    let steps = (span / s).ceil() as i32;
    let base = (from / s).floor() * s;
    for i in 0..steps {
        let below = base + Vec3::Y * (i as f32 * s);
        let here = below + Vec3::Y * s;
        if block(world, below).is_some_and(rock) && block(world, here).is_some_and(|b| !b.is_solid() && !b.is_liquid()) {
            return Some(here + Vec3::new(s * 0.5, 0.0, s * 0.5));
        }
    }
    None
}

/// The floor under `p` within `span` metres, as the top surface height.
fn floor_below(world: &World, p: Vec3, span: f32) -> Option<f32> {
    let s = world.voxel_scale.meters();
    let steps = (span / s).ceil() as i32;
    for i in 0..steps {
        let q = p - Vec3::Y * (i as f32 * s);
        if block(world, q)?.is_solid() {
            return Some(((q.y / s).floor() + 1.0) * s);
        }
    }
    None
}

/// Advances one burrower. Returns damage dealt to the player and its direction,
/// the same contract as `bosses::tick`.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    e: &mut Enemy,
    world: &World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    time: f32,
    hunt: bool,
    ruckus: f32,
    fx: &mut Vec<Fx>,
) -> (f32, Vec3) {
    let Some(st) = strain(e.kind) else { return (0.0, Vec3::ZERO) };
    if e.burrow.segs.is_empty() {
        let heading = Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos());
        e.burrow = Body::new(st, hash_u32(e.slot ^ e.site.x as u32 ^ (e.site.y as u32).rotate_left(16)), e.pos, heading);
    }
    let hostile = e.hostile();
    // Wary bodies stay wild until hurt or crowded; then they hunt for good.
    let hurt = e.health < e.max_health();
    let crowded = e.burrow.segs.first().is_some_and(|h| h.distance(feet) < WARY_REACH_M);
    if e.burrow.wary && (hurt || crowded) {
        e.burrow.wary = false;
    }
    // Diggers do not hunt a player under open sky: it takes a wound or a body
    // within reach to rouse them, and then the lunge still shows itself first.
    let calm = matches!(st.motion, Motion::Burrow | Motion::Breach)
        && st.district.is_none()
        && e.burrow.exposed
        && !hurt
        && !crowded;
    let hunt = hunt && !e.burrow.wary && !calm;
    let pace = e.affixes.speed_mul() * if e.slow > 0.0 { 0.5 } else { 1.0 };
    let menace = e.menace;
    let b = &mut e.burrow;
    b.bite_cd = (b.bite_cd - dt).max(0.0);
    b.cooldown = (b.cooldown - dt).max(0.0);
    let was_tell = b.tell;
    b.tell = (b.tell - dt).max(0.0);
    b.look -= dt;
    if b.look <= 0.0 {
        b.look = 0.5;
        b.exposed = !underground(world, eye);
    }
    b.timer -= dt;
    b.jolt = (b.jolt - dt * 3.0).max(0.0);
    let head = b.segs[0];
    let target = eye - Vec3::Y * 0.6;
    let to_player = target - head;
    let distance = to_player.length();
    let mut sense = if st.motion == Motion::Swim { SENSE_M * 0.6 } else { SENSE_M };
    if st.hears {
        let moved = b.last_feet.map_or(0.0, |last| Vec3::new(feet.x - last.x, 0.0, feet.z - last.z).length());
        let loud = (QUIET + moved / dt.max(1e-3) / LOUD_SPEED).max(ruckus).clamp(QUIET, 1.0);
        // Sound carries at once; quiet settles over about a second.
        b.noise = if loud > b.noise { loud } else { b.noise + (loud - b.noise) * (dt * 1.2).min(1.0) };
        sense *= b.noise;
    }
    b.last_feet = Some(feet);
    if st.patience_s > 0.0 {
        if matches!(b.mode, Mode::Stalk | Mode::Lunge | Mode::Dive) {
            b.pressure += dt;
        } else {
            b.pressure = (b.pressure - dt * 0.5).max(0.0);
        }
        if b.mode == Mode::Stalk && b.pressure >= st.patience_s {
            b.mode = Mode::Withdraw;
            b.timer = st.rest_s.max(1.0);
            fx.push(Fx::Sound { cue: Cue::Splash, at: head, gain: 0.35, pitch: 0.45 });
        }
    }

    // Decide where to go.
    match b.mode {
        Mode::Roam => {
            if hunt && hostile && distance < sense {
                b.mode = Mode::Stalk;
                b.cooldown = b.cooldown.max(1.0 + b.rng(11) * 1.5);
            } else if b.timer <= 0.0 || (b.goal - head).length() < 2.5 {
                let a = b.rng(12) * TAU;
                let r = 4.0 + b.rng(13) * 10.0;
                let y = if st.motion == Motion::Slither { 0.0 } else { (b.rng(14) * 2.0 - 1.0) * 5.0 };
                b.goal = e.home + Vec3::new(a.cos() * r, y, a.sin() * r);
                b.timer = 5.0 + b.rng(15) * 4.0;
            }
        }
        Mode::Stalk => {
            if !hunt || distance > sense * 1.6 {
                b.mode = Mode::Roam;
                if st.district.is_none() {
                    e.home = head;
                }
                b.timer = 0.0;
            } else {
                // Circle beneath the footsteps, then strike.
                let orbit = time * 0.55 + (b.seed & 0xff) as f32 * 0.1;
                let (depth, ring) = match st.motion {
                    Motion::Breach => (6.0, 4.0),
                    Motion::Burrow => (3.0, 5.0),
                    Motion::Swim => (1.5, 3.0),
                    Motion::Slither => (0.0, 3.5),
                };
                b.goal = feet + Vec3::new(orbit.cos() * ring, -depth, orbit.sin() * ring);
                let reach = match st.motion {
                    Motion::Breach => 11.0,
                    Motion::Slither => 4.5,
                    _ => 8.0,
                };
                if b.cooldown <= 0.0 && distance < reach {
                    b.mode = Mode::Lunge;
                    b.tell = st.tell_s;
                    b.timer = st.tell_s + if st.motion == Motion::Breach { 2.0 } else { 1.3 };
                    fx.push(Fx::Sound { cue: Cue::Warning, at: head, gain: 0.45, pitch: 0.55 + b.rng(16) * 0.2 });
                }
            }
        }
        Mode::Lunge => {
            b.goal = match st.motion {
                // Leap through the space above the player so the arc crosses them.
                Motion::Breach => feet + Vec3::Y * 4.5 + (feet - head) * Vec3::new(0.35, 0.0, 0.35),
                _ => target,
            };
            if b.timer <= 0.0 {
                b.mode = Mode::Dive;
                b.timer = 1.6;
            }
        }
        Mode::Withdraw => {
            // Back to the lair, deep, until the rest runs out.
            b.goal = e.home - Vec3::Y * if st.motion == Motion::Slither { 0.0 } else { 2.0 };
            if b.timer <= 0.0 {
                b.mode = Mode::Roam;
                b.pressure = 0.0;
                b.timer = 0.0;
            }
        }
        Mode::Dive => {
            let away = Vec3::new(head.x - feet.x, 0.0, head.z - feet.z).normalize_or(Vec3::X);
            b.goal = head + away * 6.0 - Vec3::Y * if st.motion == Motion::Slither { 0.0 } else { 7.0 };
            if b.timer <= 0.0 {
                b.mode = Mode::Stalk;
                b.cooldown = 2.5 + b.rng(17) * 2.5;
            }
        }
    }
    // While the tell runs the body gathers under the player's feet: ground
    // dust on the beat, then the strike.
    if b.tell > 0.0 && b.mode == Mode::Lunge && (b.tell * 4.0).floor() != (was_tell * 4.0).floor() {
        let at = Vec3::new(feet.x, feet.y + 0.05, feet.z);
        fx.push(Fx::Burst { at, color: st.body, count: 7 });
    }
    let speed = st.speed * match b.mode {
        Mode::Lunge if b.tell > 0.0 => 0.25,
        Mode::Lunge => st.lunge,
        Mode::Dive => 1.3,
        Mode::Roam => 0.6,
        Mode::Stalk => 1.0,
        Mode::Withdraw => 1.1,
    } * pace;

    b.phase += dt * TAU * st.weave_hz * (speed / st.speed.max(0.1)).clamp(0.5, 2.0);
    let here = block(world, head);
    let previous_air = b.air;
    match st.motion {
        Motion::Slither => slither(b, st, world, head, speed, dt),
        motion => tunnel(b, st, motion, world, head, here, speed, dt),
    }
    let new_head = b.segs[0];
    let now = block(world, new_head);

    // Breach and re-entry: a jolt, a burst of the host material, a thud.
    let medium = |x: Option<BlockId>| x.is_some_and(|x| inside(st.motion, x));
    if st.motion != Motion::Slither {
        if medium(here) && !medium(now) && now.is_some() {
            let color = here.map_or(st.body, |h| h.color(world_cell(world, head)));
            b.jolt = 1.0;
            fx.push(Fx::Burst { at: new_head, color, count: 14 });
            let cue = if st.motion == Motion::Swim { Cue::Splash } else { Cue::ForgeSlam };
            fx.push(Fx::Sound { cue, at: new_head, gain: 0.8, pitch: 0.38 + b.rng(18) * 0.12 });
        } else if !medium(here) && medium(now) && previous_air > 0.2 {
            let color = now.map_or(st.body, |h| h.color(world_cell(world, new_head)));
            b.jolt = b.jolt.max(0.6);
            fx.push(Fx::Burst { at: new_head, color, count: 10 });
            let cue = if st.motion == Motion::Swim { Cue::Splash } else { Cue::Land };
            fx.push(Fx::Sound { cue, at: new_head, gain: 0.7, pitch: 0.45 });
        }
    }

    // Grinding: the head is in rock and moving.
    let grinding = st.motion != Motion::Swim && now.is_some_and(rock);
    let want = if grinding { (speed / st.speed.max(0.1)).clamp(0.3, 1.6) } else { 0.0 };
    b.grind += (want - b.grind) * (1.0 - (-dt * 4.0).exp());

    // Particles: grit off nearby cave surfaces and the element's trail.
    b.fx_clock += dt * if grinding { 14.0 } else { 9.0 };
    while b.fx_clock >= 1.0 {
        b.fx_clock -= 1.0;
        let i = (b.rng(19) * b.segs.len() as f32) as usize % b.segs.len();
        let p = b.segs[i];
        let a = b.rng(20) * TAU;
        let c = b.rng(21) * 2.0 - 1.0;
        let dir = Vec3::new(a.cos() * (1.0 - c * c).sqrt(), c, a.sin() * (1.0 - c * c).sqrt());
        let probe = p + dir * (b.radius_at(st, i) + 0.7);
        let at_p = block(world, p);
        let at_probe = block(world, probe);
        if at_p.is_some_and(rock) && at_probe.is_some_and(|x| !x.is_solid()) {
            // A body part just under a surface: chips burst outward.
            let color = at_p.map_or(st.body, |x| x.color(world_cell(world, p)));
            fx.push(Fx::Chip { at: probe - dir * 0.3, out: dir, color });
        } else if at_p.is_some_and(|x| !x.is_solid()) {
            let glow = st.glow;
            fx.push(Fx::Trail { at: p + dir * b.radius_at(st, i), trail: if at_p.is_some_and(|x| x.is_liquid()) && st.trail != Trail::Embers { Trail::Bubbles } else { st.trail }, glow });
        }
    }

    // Bites: the head strikes; the body only bruises.
    let mut damage = 0.0;
    if hunt && hostile && b.bite_cd <= 0.0 && b.mode == Mode::Lunge && b.tell <= 0.0 {
        if new_head.distance(target) < b.radius * 1.3 + 0.7 {
            damage = st.bite * (1.0 + menace as f32 * 0.2);
            b.bite_cd = 1.1;
            if b.mode == Mode::Lunge {
                b.mode = Mode::Dive;
                b.timer = 1.6;
            }
            fx.push(Fx::Sound { cue: Cue::Impact, at: new_head, gain: 0.9, pitch: 0.7 });
        } else if b.segs.iter().enumerate().skip(2).any(|(i, s)| s.distance(target) < b.radius_at(st, i) + 0.45) {
            damage = st.bite * 0.3;
            b.bite_cd = 0.8;
        }
    }
    b.air = if medium(now) || st.motion == Motion::Slither { 0.0 } else { b.air + dt };
    if st.motion == Motion::Swim && b.air > 4.0 {
        // Stranded on dry ground.
        e.health -= 12.0 * dt;
    }
    e.pos = new_head;
    let h = b.heading;
    e.yaw = (-h.x).atan2(-h.z);
    e.velocity = b.vel;
    let direction = if damage > 0.0 { (new_head - eye).normalize_or_zero() * damage } else { Vec3::ZERO };
    (damage, direction)
}

fn world_cell(world: &World, p: Vec3) -> IVec3 {
    (p / world.voxel_scale.meters()).floor().as_ivec3()
}

/// Burrow, Breach and Swim: steer inside the medium, fall outside it.
#[allow(clippy::too_many_arguments)]
fn tunnel(b: &mut Body, st: &Strain, motion: Motion, world: &World, head: Vec3, here: Option<BlockId>, speed: f32, dt: f32) {
    let Some(here) = here else {
        // Unloaded ahead: turn back towards the goal without moving.
        b.heading = -b.heading;
        return;
    };
    let in_medium = inside(motion, here);
    if in_medium {
        let want = b.goal - head;
        let rate = st.turn * if b.mode == Mode::Lunge { 1.6 } else { 1.0 };
        b.heading = turn_toward(b.heading, want, rate * dt);
        b.vel = b.heading * speed;
    } else {
        // Ballistic across open space; breachers hang a little longer.
        let gravity = match motion {
            Motion::Breach => 11.0,
            Motion::Swim => 12.0,
            _ => 15.0,
        };
        b.vel.y -= gravity * dt;
        // A little air control toward the goal, as a worm twists mid-leap.
        let flat = Vec3::new(b.goal.x - head.x, 0.0, b.goal.z - head.z).normalize_or_zero();
        b.vel += flat * speed * 0.25 * dt;
        b.heading = b.vel.normalize_or(b.heading);
    }
    // Swimmers turn back before leaving water unless striking.
    if motion == Motion::Swim && in_medium && b.mode != Mode::Lunge {
        let ahead = head + b.heading * (b.radius + 0.6);
        if block(world, ahead).is_some_and(|x| !x.is_liquid()) {
            b.heading = turn_toward(b.heading, Vec3::new(-b.heading.x, -b.heading.y.abs() - 0.5, -b.heading.z), st.turn * 2.0 * dt);
            b.vel = b.heading * speed;
        }
    }
    // Swimmers do not pass through rock: bounce off it.
    let side = b.heading.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
    let lift = side.cross(b.heading);
    let weave = (side * b.phase.cos() + lift * (b.phase * 0.5).sin() * 0.35)
        * b.weave * TAU * st.weave_hz * if in_medium { 1.0 } else { 0.25 };
    let step = (b.vel + weave) * dt;
    let next = head + step;
    match block(world, next) {
        None => b.heading = -b.heading,
        Some(x) if motion == Motion::Swim && rock(x) => {
            b.vel = -b.vel * 0.3;
            b.heading = b.vel.normalize_or(-b.heading);
        }
        Some(_) => {
            b.segs[0] = next;
            b.follow();
        }
    }
}

/// Slither: crawl along the floor, climb single steps, turn at walls.
fn slither(b: &mut Body, st: &Strain, world: &World, head: Vec3, speed: f32, dt: f32) {
    let s = world.voxel_scale.meters();
    // Solve on the floor; the strike rear is added back afterwards.
    let rear = b.radius * 1.2;
    let head = head - Vec3::Y * b.rear * rear;
    let want = Vec3::new(b.goal.x - head.x, 0.0, b.goal.z - head.z);
    let flat = Vec3::new(b.heading.x, 0.0, b.heading.z).normalize_or(Vec3::X);
    let mut heading = turn_toward(flat, want.normalize_or(flat), st.turn * dt);
    let side = heading.cross(Vec3::Y).normalize_or(Vec3::X);
    let weave = side * b.phase.cos() * b.weave * TAU * st.weave_hz;
    let mut step = (heading * speed + weave) * dt;
    let r = b.radius;
    let body_y = |p: Vec3| p + Vec3::Y * (r + s * 0.5);
    let wall = |p: Vec3| block(world, body_y(p)).is_none_or(|x| x.is_solid());
    if wall(head + step) {
        let climb = head + step + Vec3::Y * s;
        if !wall(climb) {
            step.y += s;
        } else {
            // Pick the freer side and turn hard.
            let left = !wall(head + heading.rotate_axis(Vec3::Y, 1.2) * s);
            heading = heading.rotate_axis(Vec3::Y, if left { 1.0 } else { -1.0 } * st.turn * 3.0 * dt);
            step = Vec3::ZERO;
        }
    }
    let mut next = head + step;
    match floor_below(world, next + Vec3::Y * s, 3.0) {
        Some(top) if top >= next.y - r - 0.05 => {
            next.y = top + r;
            b.vel = step / dt.max(0.001);
        }
        _ => {
            b.vel.y -= 18.0 * dt;
            next.y += b.vel.y * dt;
            if let Some(top) = floor_below(world, next + Vec3::Y * s, 1.0) {
                next.y = next.y.max(top + r);
                b.vel.y = 0.0;
            }
        }
    }
    // Strike: rear the head up toward the eye during a lunge.
    let reared = if b.mode == Mode::Lunge { 1.0 } else { 0.0 };
    b.rear += (reared - b.rear) * (1.0 - (-dt * 10.0).exp());
    next.y += b.rear * rear;
    b.heading = heading;
    b.segs[0] = next;
    b.follow();
    // Body segments rest on the floor beneath them.
    for i in 1..b.segs.len() {
        let p = b.segs[i];
        if let Some(top) = floor_below(world, p + Vec3::Y * s, 1.5) {
            let rest = top + b.radius_at(st, i);
            b.segs[i].y += (rest - p.y) * (1.0 - (-dt * 12.0).exp());
        }
    }
}

/// Precise hit test along the whole body.
pub(super) fn hit(e: &Enemy, at: Vec3, pad: f32) -> bool {
    let Some(st) = strain(e.kind) else { return (e.pos - at).length() < 0.6 + pad };
    if e.burrow.segs.is_empty() {
        return (e.pos - at).length() < 0.6 + pad;
    }
    e.burrow.segs.iter().enumerate().any(|(i, s)| s.distance(at) < e.burrow.radius_at(st, i) + 0.2 + pad)
}

/// The body part closest to the aim ray, so a blade can strike any segment.
pub(super) fn melee_point(e: &Enemy, eye: Vec3, forward: Vec3) -> Vec3 {
    let forward = forward.normalize_or(Vec3::Z);
    e.burrow.segs.iter().copied().min_by(|a, b| {
        let off = |p: Vec3| {
            let along = (p - eye).dot(forward).max(0.0);
            (p - (eye + forward * along)).length_squared()
        };
        off(*a).total_cmp(&off(*b))
    }).unwrap_or(e.pos)
}

/// Emits queued effects, then the shared tremor, rumble and ceiling dust.
pub(super) fn after_tick(adv: &mut Adventure, world: &World, feet: Vec3, eye: Vec3, dt: f32, fx: Vec<Fx>) {
    for f in fx {
        match f {
            Fx::Chip { at, out, color } => {
                let k = hash_u32(at.x.to_bits() ^ at.z.to_bits().rotate_left(9));
                let v = out * (1.5 + unit(k) * 2.0) + Vec3::Y * 1.2;
                adv.spawn_voxel_particle(at, v, color, 0.035 + unit(k >> 8) * 0.05, 0.0, 1.0, 9.0, 1.1);
            }
            Fx::Trail { at, trail, glow } => {
                let k = hash_u32(at.x.to_bits() ^ at.y.to_bits().rotate_left(13));
                let jitter = Vec3::new(unit(k) - 0.5, unit(k >> 5) - 0.5, unit(k >> 10) - 0.5);
                let (color, size, light, gravity, life, vel) = match trail {
                    Trail::Grit => ([0.42, 0.38, 0.33], 0.03, 0.0, 9.0, 0.9, jitter * 0.6),
                    Trail::Embers => (glow, 0.025, 1.8, -1.6, 0.9, jitter * 0.8 + Vec3::Y * 0.4),
                    Trail::Frost => ([0.86, 0.94, 1.0], 0.022, 0.6, 0.4, 1.4, jitter * 0.5),
                    Trail::Spores => (glow, 0.02, 1.2, -0.15, 2.4, jitter * 0.35),
                    Trail::Bubbles => ([0.80, 0.90, 0.96], 0.03, 0.2, -3.0, 0.8, jitter * 0.3),
                    Trail::Motes => (glow, 0.02, 1.6, 0.0, 1.6, jitter * 0.5),
                };
                adv.spawn_voxel_particle(at, vel, color, size, light, 0.9, gravity, life);
            }
            Fx::Burst { at, color, count } => adv.spawn_voxel_burst(at, color, count, 3.2, 0.05),
            Fx::Sound { cue, at, gain, pitch } => sound_pitched(&mut adv.sounds, cue, Some(at), gain, pitch),
        }
    }

    // Tremor: every body part near the player contributes; a breach adds a jolt.
    let mut tremor = 0.0f32;
    let mut loudest: Option<(f32, Vec3)> = None;
    for e in adv.enemies.iter().filter(|e| e.health > 0.0 && is_kind(e.kind)) {
        let Some(st) = strain(e.kind) else { continue };
        let b = &e.burrow;
        let nearest = if b.segs.is_empty() {
            e.pos.distance(feet)
        } else {
            b.segs.iter().step_by(3).chain(b.segs.last()).map(|s| s.distance(feet)).fold(f32::MAX, f32::min)
        };
        let falloff = (1.0 - nearest / TREMOR_RANGE_M).max(0.0).powi(2);
        let grind = if b.segs.is_empty() { 0.6 } else { 0.25 + 0.75 * b.grind };
        let amount = st.rumble * (grind + b.jolt * 1.5) * falloff;
        tremor += amount;
        if loudest.is_none_or(|(a, _)| amount > a) {
            loudest = Some((amount, e.pos));
        }
    }
    let tremor = tremor.min(TREMOR_MAX_M);
    TREMOR_M.store(tremor.to_bits(), Ordering::Relaxed);

    // Rumble: low crunches whose cadence and pitch wander with the grind.
    adv.burrowers.rumble_clock -= dt;
    if let Some((amount, at)) = loudest
        && amount > 0.004
        && adv.burrowers.rumble_clock <= 0.0
    {
        let k = hash_u32(adv.burrowers.passes ^ (adv.time * 64.0) as u32);
        let gain = (amount * 14.0).clamp(0.12, 1.0);
        let cue = if unit(k) < 0.6 { Cue::Mine } else { Cue::Land };
        sound_pitched(&mut adv.sounds, cue, Some(at), gain, 0.28 + unit(k >> 7) * 0.14);
        adv.burrowers.rumble_clock = 0.22 + unit(k >> 14) * 0.45;
    }

    // Dust and pebbles shaken loose from the ceiling over the player.
    if tremor > 0.012 {
        adv.burrowers.dust_clock += dt * tremor * 260.0;
        let s = world.voxel_scale.meters();
        while adv.burrowers.dust_clock >= 1.0 {
            adv.burrowers.dust_clock -= 1.0;
            let k = hash_u32(adv.burrowers.passes ^ (adv.time * 997.0) as u32 ^ adv.particles.len() as u32);
            let offset = Vec3::new(unit(k) - 0.5, 0.0, unit(k >> 8) - 0.5) * 10.0;
            let from = eye + offset;
            let steps = (8.0 / s).ceil() as i32;
            for i in 1..=steps {
                let p = from + Vec3::Y * (i as f32 * s);
                match block(world, p) {
                    Some(c) if c.is_solid() => {
                        let underside = ((p.y / s).floor()) * s - 0.02;
                        let at = Vec3::new(p.x, underside, p.z);
                        let pebble = unit(k >> 16) < 0.15;
                        let color = c.color(world_cell(world, p));
                        adv.spawn_voxel_particle(at, Vec3::ZERO, color,
                            if pebble { 0.06 } else { 0.022 }, 0.0, if pebble { 1.0 } else { 0.7 }, 9.0, 1.6);
                        break;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        }
    } else {
        adv.burrowers.dust_clock = 0.0;
    }
}

/// Dead burrowers crumble into their colours; no record, no site completion.
pub(super) fn fall(adv: &mut Adventure) {
    let mut bursts = Vec::new();
    for e in adv.enemies.iter().filter(|e| is_burrow_body(e) && e.health <= 0.0) {
        let Some(st) = strain(e.kind) else { continue };
        let tint = e.burrow.tint;
        let color = [st.body[0] * tint[0], st.body[1] * tint[1], st.body[2] * tint[2]];
        if e.burrow.segs.is_empty() {
            bursts.push((e.pos, color));
        }
        for s in e.burrow.segs.iter().step_by(2) {
            bursts.push((*s, color));
        }
    }
    for (i, (at, color)) in bursts.into_iter().enumerate() {
        adv.spawn_voxel_burst(at, color, 5, 2.4, 0.06);
        if i == 0 {
            super::sound(&mut adv.sounds, Cue::Defeat, Some(at), 0.7);
        }
    }
    adv.enemies.retain(|e| !is_burrow_body(e) || e.health > 0.0);
}

/// Camera offset in metres for a frame at `time_s`: grinding surges over a
/// jitter of incommensurate wobbles, scaled by the published tremor.
pub fn shake(time_s: f32) -> Vec3 {
    let a = f32::from_bits(TREMOR_M.load(Ordering::Relaxed));
    if !a.is_finite() || a <= 0.0 {
        return Vec3::ZERO;
    }
    let surge = 0.65 + 0.35 * ((time_s * 1.7).sin() * (time_s * 0.63 + 1.1).sin()).abs().sqrt();
    let w = |f: f32, p: f32| {
        (time_s * f + p).sin() * 0.55 + (time_s * f * 1.93 + p * 2.3).sin() * 0.3 + (time_s * f * 3.71 + p).sin() * 0.15
    };
    Vec3::new(w(31.0, 0.0), w(23.0, 1.7) * 0.8, w(37.0, 2.9)) * a * surge
}

fn tinted(c: [f32; 3], tint: [f32; 3], shade: f32) -> [f32; 3] {
    [
        (c[0] * tint[0] * shade).clamp(0.0, 1.0),
        (c[1] * tint[1] * shade).clamp(0.0, 1.0),
        (c[2] * tint[2] * shade).clamp(0.0, 1.0),
    ]
}

fn flashed(c: [f32; 3], flash: f32) -> [f32; 3] {
    let t = (flash * 5.0).clamp(0.0, 1.0);
    [lerp(c[0], 1.0, t), lerp(c[1], 0.94, t), lerp(c[2], 0.69, t)]
}

/// Chunky voxel body: one oriented box per segment, a belly plate, the
/// strain's ornament and a head with a mouth that opens to strike.
pub(super) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let Some(st) = strain(e.kind) else { return };
    let b = &e.burrow;
    let fallback;
    let (segs, radius, spacing, tint): (&[Vec3], f32, f32, [f32; 3]) = if b.segs.is_empty() {
        let r = (st.radius_m.0 + st.radius_m.1) * 0.5;
        let back = Vec3::new(e.yaw.sin(), 0.0, e.yaw.cos());
        fallback = (0..st.segments.0.max(3)).map(|i| e.pos + back * r * 1.25 * i as f32).collect::<Vec<_>>();
        (&fallback, r, r * 1.25, [1.0; 3])
    } else {
        (&b.segs, b.radius, b.spacing, b.tint)
    };
    let n = segs.len();
    let radius_at = |i: usize| {
        let t = i as f32 / (n.max(2) - 1) as f32;
        radius * if t < 0.15 { 0.92 + t / 0.15 * 0.13 } else { lerp(1.05, st.taper, ((t - 0.15) / 0.85).powf(1.3)) }
    };
    let pulse = 0.75 + 0.25 * (time * 2.3 + (b.seed & 0xff) as f32 * 0.07).sin();
    let length = if st.ornament == Ornament::Plates { 0.92 } else { 1.3 };
    for i in (0..n).rev() {
        let p = segs[i];
        let dir = if i == 0 {
            if b.segs.is_empty() { Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos()) } else { b.heading }
        } else {
            segs[i - 1] - p
        };
        let q = frame(dir);
        let (x, y) = (q * Vec3::X, q * Vec3::Y);
        let r = radius_at(i);
        let band = if i % 3 == 0 { 0.88 } else if i % 3 == 1 { 1.0 } else { 0.95 };
        let body = flashed(tinted(st.body, tint, band), e.flash);
        let belly = flashed(tinted(st.belly, tint, band), e.flash);
        if i == 0 {
            head(e, st, m, p, q, r, tint, time);
            continue;
        }
        m.material_box(p, Vec3::new(2.0 * r, 1.84 * r, spacing * length), q, body, 70.0, 0.0);
        m.material_box(p - y * r * 0.62, Vec3::new(1.5 * r, 0.7 * r, spacing * length * 0.92), q, belly, 70.0, 0.0);
        let glow = st.glow_strength * pulse;
        match st.ornament {
            Ornament::Plates => {
                if i + 1 < n {
                    let joint = (p + segs[i + 1]) * 0.5;
                    m.cuboid(joint, Vec3::new(1.7 * r, 1.6 * r, spacing * 0.4), q, st.glow, glow);
                }
            }
            Ornament::Crystals if i % 3 == 1 => {
                let lean = if (i / 3) % 2 == 0 { 0.5 } else { -0.5 };
                m.cuboid(p + y * r * 1.15, Vec3::new(0.3 * r, 1.0 * r, 0.3 * r), q * Quat::from_rotation_z(lean), st.glow, glow);
            }
            Ornament::Caps if i % 3 == 2 => {
                m.material_box(p + y * r * 1.05, Vec3::new(0.25 * r, 0.5 * r, 0.25 * r), q, belly, 70.0, 0.0);
                m.cuboid(p + y * r * 1.35, Vec3::new(1.1 * r, 0.3 * r, 1.1 * r), q, st.glow, glow * 0.7);
            }
            Ornament::Frills => {
                let flutter = (time * 7.0 + i as f32 * 0.8).sin() * 0.35;
                for side in [-1.0f32, 1.0] {
                    m.cuboid(p + x * side * r * 1.1, Vec3::new(0.06, 0.95 * r, spacing * 0.9), q * Quat::from_rotation_z(side * flutter),
                        tinted(st.glow, tint, 0.8), glow * 0.4);
                }
            }
            Ornament::Spines if i % 2 == 0 => {
                m.material_box(p + y * r * 1.05, Vec3::new(0.14 * r, 0.75 * r, 0.45 * r), q * Quat::from_rotation_x(-0.5),
                    tinted(st.body, tint, 0.7), 70.0, 0.0);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn head(e: &Enemy, st: &Strain, m: &mut Model, p: Vec3, q: Quat, r: f32, tint: [f32; 3], time: f32) {
    let (x, y, z) = (q * Vec3::X, q * Vec3::Y, q * Vec3::Z);
    let open = if e.burrow.mode == Mode::Lunge { 1.0 } else { 0.25 + 0.15 * (time * 3.1 + e.slot as f32).sin() };
    let skin = flashed(tinted(st.head, tint, 1.0), e.flash);
    let r = r * 1.18;
    m.material_box(p, Vec3::new(2.0 * r, 1.85 * r, 1.6 * r), q, skin, 70.0, 0.0);
    // Maw: a dark throat ringed by four pale teeth that splay as it opens.
    let front = p + z * r * 0.82;
    m.cuboid(front, Vec3::new(1.2 * r * (0.6 + 0.4 * open), 1.2 * r * (0.6 + 0.4 * open), 0.06), q, [0.05, 0.03, 0.03], 0.0);
    let tooth = [0.86, 0.82, 0.70];
    for k in 0..4 {
        let a = k as f32 * TAU / 4.0 + std::f32::consts::FRAC_PI_4;
        let radial = x * a.cos() + y * a.sin();
        let splay = Quat::from_axis_angle(radial.cross(z).normalize_or(x), -0.5 * open);
        m.cuboid(front + radial * r * 0.55 + z * r * 0.12, Vec3::new(0.14 * r, 0.14 * r, 0.5 * r), splay * q, tooth, 0.0);
    }
    match st.ornament {
        Ornament::Mandibles => {
            for side in [-1.0f32, 1.0] {
                let pivot = p + z * r * 0.7 + x * side * r * 0.75;
                let swing = Quat::from_axis_angle(y, side * (0.25 + 0.55 * open));
                let reach = (swing * z) * r * 1.3;
                m.material_beam(pivot, pivot + reach, r * 0.14, tinted(st.head, tint, 0.55), 70.0);
                m.material_beam(pivot + reach, pivot + reach - x * side * r * 0.5, r * 0.11, tinted(st.head, tint, 0.45), 70.0);
            }
        }
        _ => {
            // Eyes: glowing beads for luminous strains, dark beads otherwise.
            let (color, glow) = if st.glow_strength > 0.0 { (st.glow, st.glow_strength * 1.5) } else { ([0.04, 0.03, 0.03], 0.0) };
            for side in [-1.0f32, 1.0] {
                m.cuboid(p + z * r * 0.55 + x * side * r * 0.62 + y * r * 0.45, Vec3::splat(0.22 * r), q, color, glow);
            }
        }
    }
}

#[cfg(test)]
#[path = "burrowers_tests.rs"]
mod tests;
