//! Headless, deterministic scenario runner: `--headless=<scenario.ron>`.
//!
//! No window; GPU cellular authority is opt-in. A scenario builds a world from a seed, applies steps
//! (place blocks, hold inputs, advance ticks) and checks world state. Every
//! pass runs with `Budget::UNBOUNDED` and waits for chunk generation, so the
//! same scenario file always produces the same world hash. Output is one JSON
//! object on stdout; the exit status is non-zero when any check fails.
//! Difference from the windowed game: nothing is meshed, so water that only
//! the mesher would have seeded stays asleep until an edit wakes it.
use crate::{
    fluid::VoxelScalePreset,
    material::{ALL_BLOCK_IDS, BlockId},
    player::{Input, Player},
    simulation::{Budget, WorldSimulation, fixed, cpu_region},
    world::{DEFAULT_DAY_LENGTH_SECONDS, DEFAULT_START_TIME_OF_DAY, DayNightCycle, World, WorldType},
};
use glam::{IVec3, Vec3};
use serde::Deserialize;
use std::collections::HashSet;
mod options;
mod session;
use options::RegionOptions;

#[derive(Deserialize, Debug)]
pub struct Scenario {
    #[serde(default)]
    pub name: String,
    pub world: WorldType,
    pub scale: VoxelScalePreset,
    pub seed: u32,
    /// Pins the generator; defaults to the current one.
    #[serde(default)]
    pub generator_version: Option<u32>,
    pub steps: Vec<Step>,
}

#[derive(Deserialize, Debug)]
pub enum Step {
    /// Run `ticks` fixed simulation steps (1/120 s each).
    Advance { ticks: u32 },
    /// Hold these inputs for `ticks` steps, then release them.
    Hold {
        ticks: u32,
        #[serde(default)]
        forward: bool,
        #[serde(default)]
        back: bool,
        #[serde(default)]
        left: bool,
        #[serde(default)]
        right: bool,
        #[serde(default)]
        jump: bool,
        #[serde(default)]
        sprint: bool,
        #[serde(default)]
        cast: bool,
        #[serde(default)]
        sneak: bool,
        /// Pressed on the first tick of the hold (a tap, like the key edge).
        #[serde(default)]
        dash: bool,
        #[serde(default)]
        place: bool,
        #[serde(default)]
        mine: bool,
        #[serde(default)]
        pick: bool,
    },
    /// Set a block at a voxel offset from the player's feet.
    Place { offset: [i32; 3], block: String },
    /// Set a block at an absolute voxel coordinate.
    PlaceAt { at: [i32; 3], block: String },
    /// Exact development-region water volume (0..=64), without legacy rounding.
    Water64 { at: [i32; 3], volume: u8 },
    /// Conditional edit against the cell in the player's published view.
    EditAt { at: [i32; 3], block: String },
    /// Inventory-backed conditional placement from a selected hotbar slot.
    BuildAt { at: [i32; 3], slot: usize },
    /// Conditional mining with a retained inventory reward.
    MineAt { at: [i32; 3] },
    SelectSlot { slot: usize },
    Aim { yaw: f32, pitch: f32 },
    /// Print the published exact cell and its real CA tick to stderr.
    InspectCell { at: [i32; 3] },
    /// Move the player (metres).
    Teleport { to: [f32; 3] },
    Expect(Check),
}

#[derive(Deserialize, Debug)]
pub enum Check {
    /// The block at a voxel offset from the player's feet is `is` (a `BlockId` name).
    Block { offset: [i32; 3], is: String },
    BlockAt { at: [i32; 3], is: String },
    /// Player height in metres lies in `[min, max]`.
    PlayerY { min: f32, max: f32 },
    /// Player position and velocity are finite.
    Finite,
    /// Water volume (0-8 per cell) at a voxel offset from the feet is at least `min`.
    Water { offset: [i32; 3], min: u8 },
    /// The world hash equals this value (taken from a previous run's summary).
    Hash { is: String },
}

pub struct Session {
    pub world: World,
    pub player: Player,
    pub day_night: DayNightCycle,
    pub sim: WorldSimulation,
    pub input: Input,
    pub ticks: u64,
    dirty_blocks: HashSet<IVec3>,
    dirty_chunks: HashSet<IVec3>,
}

impl Session {
    pub fn new(
        world_type: WorldType,
        scale: VoxelScalePreset,
        seed: u32,
        generator_version: Option<u32>,
    ) -> Self {
        Self::new_with_storage(world_type, scale, seed, generator_version, None)
    }

    fn new_with_storage(world_type: WorldType, scale: VoxelScalePreset, seed: u32,
        generator_version: Option<u32>, directory: Option<&std::path::Path>) -> Self {
        let mut world = if let Some(directory) = directory {
            World::with_seed_in(world_type, scale, seed, directory)
        } else { World::with_seed(world_type, scale, seed) };
        if let Some(version) = generator_version {
            world.generator_version = version;
        }
        let mut player = Player::new();
        player.pos = world.suggested_spawn();
        player.precise_pos = player.pos.as_dvec3();
        let mut sim = WorldSimulation::new();
        sim.reset_for_world(scale);
        let mut day_night = DayNightCycle::new(DEFAULT_START_TIME_OF_DAY, DEFAULT_DAY_LENGTH_SECONDS);
        day_night.reset(DEFAULT_START_TIME_OF_DAY);
        Self {
            world,
            player,
            day_night,
            sim,
            input: Input::default(),
            ticks: 0,
            dirty_blocks: HashSet::new(),
            dirty_chunks: HashSet::new(),
        }
    }

    /// One fixed simulation step, in the same order as the windowed game
    /// (`main.rs`: expedition, factory, player, adventure, cellular/rigid, day).
    pub fn tick(&mut self) {
        if !self.sim.prepare_step() { return; }
        let world = &mut self.world;
        if let Some(region) = self.sim.cpu_region.as_ref() {
            let slot = region.tool_slot;
            crate::simulation::tools::step(&mut self.sim, world, &mut self.player, &mut self.input, slot);
        } else {
            self.player.glide.wind = crate::player::glide::weather_wind(world, &self.day_night, self.player.pos);
            fixed::gameplay(world, &mut self.player, &mut self.input, true, |_, _, _, input| {
                fixed::Actions { cast: input.cast_spell || input.cast_pressed,
                    cycle: input.cycle_spell, melee: false }
            });
        }
        fixed::finish(&mut self.sim, world, &self.player, &mut self.day_night,
            &mut self.dirty_blocks, &mut self.dirty_chunks, Budget::UNBOUNDED, true, true);
        self.ticks = self.sim.player_tick.0;
    }

    pub fn advance(&mut self, ticks: u32) {
        let Some(target) = self.ticks.checked_add(u64::from(ticks)) else {
            self.sim.authority_error = Some("headless tick target exhausted".into());
            return;
        };
        let mut waiting_since = std::time::Instant::now();
        while self.ticks < target {
            let before = self.ticks;
            self.tick();
            if self.sim.authority_error.is_some() { break; }
            if self.ticks != before {
                waiting_since = std::time::Instant::now();
            } else if waiting_since.elapsed() >= std::time::Duration::from_secs(30) {
                self.sim.authority_error = Some("headless authority wait timed out without advancing its tick".into());
                break;
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }

    fn feet(&self) -> IVec3 {
        (self.player.pos / self.world.voxel_scale.meters()).floor().as_ivec3()
    }

    /// FNV-1a over resident chunk blocks, player pose, factory node coordinates
    /// and inventory stacks. Stable across runs of the same scenario.
    pub fn hash(&self) -> u64 {
        self.hash_snapshot(false).unwrap_or(0)
    }

    fn hash_snapshot(&self, include_cold: bool) -> Result<u64, String> {
        let mut h = 0xcbf29ce484222325u64;
        let mut feed = |bytes: &[u8]| {
            for &b in bytes {
                h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
            }
        };
        if let Some(view) = &self.world.ca_view {
            feed(&view.tick.to_le_bytes());
            feed(bytemuck::cast_slice(&view.cells[..]));
        }
        let mut coords: Vec<IVec3> = self.world.chunks.keys().copied().collect();
        if include_cold { coords.extend(self.world.regions.view.records.keys().copied()); }
        coords.sort_unstable_by_key(|c| (c.y, c.z, c.x));
        coords.dedup();
        for c in coords {
            feed(&c.x.to_le_bytes());
            feed(&c.y.to_le_bytes());
            feed(&c.z.to_le_bytes());
            let cold = if self.world.chunks.contains_key(&c) { None } else {
                Some(self.world.regions.view.load(c).map_err(|e| format!("cannot hash stored chunk: {e:#}"))?
                    .ok_or("stored chunk disappeared while hashing")?)
            };
            let chunk = self.world.chunks.get(&c).or_else(|| cold.as_ref().map(|data| &data.chunk))
                .ok_or("chunk disappeared while hashing")?;
            // One byte per id below 255 keeps recorded hashes; 255 escapes a two-byte id.
            let mut blocks: Vec<u8> = Vec::with_capacity(chunk.blocks.len());
            for &b in chunk.blocks.iter() {
                match u8::try_from(b as u16) {
                    Ok(byte) if byte < u8::MAX => blocks.push(byte),
                    _ => {
                        blocks.push(u8::MAX);
                        blocks.extend_from_slice(&(b as u16).to_le_bytes());
                    }
                }
            }
            feed(&blocks);
        }
        let p = &self.player;
        for v in [p.pos.x, p.pos.y, p.pos.z, p.vel.x, p.vel.y, p.vel.z] {
            feed(&v.to_bits().to_le_bytes());
        }
        let mut nodes: Vec<IVec3> = self.world.factory.nodes.keys().copied().collect();
        nodes.sort_unstable_by_key(|c| (c.y, c.z, c.x));
        for c in nodes {
            feed(&c.x.to_le_bytes());
            feed(&c.y.to_le_bytes());
            feed(&c.z.to_le_bytes());
        }
        let inv = &self.world.inventory;
        for stack in inv.hotbar.iter().chain(inv.pack.iter()).chain(inv.equipment.iter()).flatten() {
            let (a, b) = stack.item.encode_u32();
            feed(&a.to_le_bytes());
            feed(&b.to_le_bytes());
            feed(&stack.count.to_le_bytes());
        }
        Ok(h)
    }
}

fn block_by_name(name: &str) -> Option<BlockId> {
    ALL_BLOCK_IDS.iter().copied().find(|id| format!("{id:?}") == name)
}

pub struct Outcome {
    pub name: String,
    pub ticks: u64,
    pub hash: u64,
    pub failures: Vec<String>,
    pub ca: Option<String>,
}

impl Outcome {
    pub fn json(&self) -> String {
        let failures = self
            .failures
            .iter()
            .map(|f| format!("\"{}\"", f.replace('"', "'")))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"scenario\":\"{}\",\"ticks\":{},\"hash\":\"{:016x}\",\"failed\":{},\"failures\":[{}]{}}}",
            self.name,
            self.ticks,
            self.hash,
            self.failures.len(),
            failures,
            self.ca.as_ref().map_or(String::new(), |ca| format!(",{ca}"))
        )
    }
}

pub fn run_scenario(scenario: &Scenario) -> Result<Outcome, String> {
    run_with_region(scenario, RegionOptions::default())
}

fn run_with_region(scenario: &Scenario, options: RegionOptions) -> Result<Outcome, String> {
    let mut s = if let Some(directory) = &options.session_in {
        let loaded = Session::load_exact(std::path::Path::new(directory))?;
        let cfg = loaded.world.sample_config();
        if cfg.seed != scenario.seed || cfg.world_type != scenario.world || cfg.voxel_scale != scenario.scale
            || cfg.generator_version != scenario.generator_version.unwrap_or_else(|| World::default_generator(scenario.world)) {
            return Err("exact session world differs from the continuation document".into());
        }
        if options.origin.is_some_and(|origin| loaded.sim.cpu_region.as_ref().is_none_or(|region| region.published.origin != origin)) {
            return Err("exact session origin differs from requested region".into());
        }
        loaded
    } else {
        let directory = options.session_out.as_deref().map(|directory| -> Result<_, String> {
            std::fs::create_dir_all(directory).map_err(|e| format!("cannot create exact session directory: {e}"))?;
            std::fs::canonicalize(directory).map_err(|e| format!("invalid exact session directory: {e}"))
        }).transpose()?;
        Session::new_with_storage(scenario.world, scenario.scale, scenario.seed, scenario.generator_version, directory.as_deref())
    };
    if s.sim.cpu_region.is_none() && let Some(origin) = options.origin {
        let region = if let Some(path) = &options.checkpoint_in {
            let (region, tick) = cpu_region::Region::load_checkpoint(&s.world, origin, std::path::Path::new(path))?;
            s.ticks = tick;
            s.sim.player_tick = engine_core::time::TickId(tick);
            region
        } else {
            cpu_region::Region::import(&s.world, origin)?
        };
        s.sim.cpu_region = Some(region);
    }
    if let Some(region) = s.sim.cpu_region.as_mut() {
        if options.gpu_backend { region.enable_gpu()?; }
        s.world.publish_ca_view(region.published.clone(), &mut s.dirty_blocks, &mut s.dirty_chunks);
        region.publish_reservations(&mut s.world);
    }
    let mut failures = Vec::new();
    for (index, step) in scenario.steps.iter().enumerate() {
        match step {
            Step::Advance { ticks } => s.advance(*ticks),
            Step::Hold { ticks, forward, back, left, right, jump, sprint, cast, sneak, dash, place, mine, pick } => {
                if (*place || *mine || *pick) && s.sim.cpu_region.is_none() {
                    return Err("held terrain tools require cellular authority".into());
                }
                s.input.sneak = *sneak;
                s.input.press_dash(*dash);
                s.input.forward = *forward;
                s.input.back = *back;
                s.input.left = *left;
                s.input.right = *right;
                s.input.jump = *jump;
                s.input.sprint = *sprint;
                s.input.cast_spell = *cast;
                s.input.cast_pressed = *cast;
                s.input.place_block = *place;
                s.input.break_block = *mine;
                s.input.pick_block = *pick;
                s.advance(*ticks);
                s.input = Input::default();
            }
            Step::Place { offset, block } | Step::PlaceAt { at: offset, block } => {
                let id = block_by_name(block)
                    .ok_or_else(|| format!("step {index}: unknown block `{block}`"))?;
                let at = if matches!(step, Step::Place { .. }) {
                    s.feet() + IVec3::from(*offset)
                } else {
                    IVec3::from(*offset)
                };
                if let Some(region) = s.sim.cpu_region.as_mut() {
                    let cell = if id == BlockId::Air { engine_sim::Cell::EMPTY } else { engine_sim::Cell::filled(id.sim_byte()) };
                    let receipt = region.submit(s.ticks, at, cell).map_err(|e| format!("step {index}: {e}"))?;
                    eprintln!("[ca-accepted] sequence={} player_tick={} earliest_ca_tick={}",
                        receipt.sequence, receipt.submitted_player_tick, receipt.earliest_ca_tick);
                } else if id == BlockId::Water {
                    // Water lives in the volume map (0-8 per cell), not in the block id.
                    s.world.set_water_volume(at, 8);
                } else {
                    s.world.set(at, id);
                }
            }
            Step::EditAt { at, block } => {
                let at = IVec3::from(*at);
                let id = block_by_name(block).ok_or_else(|| format!("step {index}: unknown block `{block}`"))?;
                let expected = s.world.ca_view.as_ref().and_then(|view| view.cell(at))
                    .ok_or("EditAt requires a cell inside --ca-cpu-region")?;
                let replacement = if id == BlockId::Air { engine_sim::Cell::EMPTY }
                    else { engine_sim::Cell::filled(id.sim_byte()) };
                let region = s.sim.cpu_region.as_mut().ok_or("EditAt requires --ca-cpu-region")?;
                let receipt = region.submit_replace(s.ticks, at, expected, replacement)
                    .map_err(|e| format!("step {index}: {e}"))?;
                eprintln!("[ca-accepted] sequence={} player_tick={} earliest_ca_tick={}",
                    receipt.sequence, receipt.submitted_player_tick, receipt.earliest_ca_tick);
            }
            Step::BuildAt { at, slot } => {
                let region = s.sim.cpu_region.as_mut().ok_or("BuildAt requires --ca-cpu-region")?;
                let receipt = region.place_owned(&mut s.world, s.ticks, IVec3::from(*at), *slot)
                    .map_err(|e| format!("step {index}: {e}"))?;
                eprintln!("[ca-owned] placement sequence={}", receipt.sequence);
            }
            Step::MineAt { at } => {
                let region = s.sim.cpu_region.as_mut().ok_or("MineAt requires --ca-cpu-region")?;
                let receipt = region.mine_owned(&s.world, s.ticks, IVec3::from(*at))
                    .map_err(|e| format!("step {index}: {e}"))?;
                eprintln!("[ca-owned] mining sequence={}", receipt.sequence);
            }
            Step::SelectSlot { slot } => {
                if *slot >= crate::world::inventory::HOTBAR_SLOTS { return Err("invalid hotbar slot".into()); }
                s.sim.cpu_region.as_mut().ok_or("SelectSlot requires cellular authority")?.tool_slot = *slot;
            }
            Step::Aim { yaw, pitch } => {
                if !yaw.is_finite() || !pitch.is_finite() || pitch.abs() > 1.54 {
                    return Err("invalid aim angles".into());
                }
                s.player.yaw = *yaw;
                s.player.pitch = *pitch;
            }
            Step::Water64 { at, volume } => {
                let region = s.sim.cpu_region.as_mut().ok_or("Water64 requires --ca-cpu-region")?;
                let cell = if *volume == 0 { engine_sim::Cell::EMPTY } else {
                    engine_sim::Cell { volume: *volume, ..engine_sim::Cell::filled(BlockId::Water as u8) }
                };
                let receipt = region.submit(s.ticks, IVec3::from(*at), cell).map_err(|e| format!("step {index}: {e}"))?;
                eprintln!("[ca-accepted] sequence={} player_tick={} earliest_ca_tick={}",
                    receipt.sequence, receipt.submitted_player_tick, receipt.earliest_ca_tick);
            }
            Step::InspectCell { at } => {
                let view = s.world.ca_view.as_ref().ok_or("InspectCell requires --ca-cpu-region")?;
                let cell = view.cell(IVec3::from(*at)).ok_or("cell outside CPU region and halo")?;
                eprintln!("[ca-cell] published_tick={} at={at:?} cell={cell:?}", view.tick);
            }
            Step::Teleport { to } => {
                s.player.pos = Vec3::from(*to);
                s.player.precise_pos = s.player.pos.as_dvec3();
                s.player.vel = Vec3::ZERO;
            }
            Step::Expect(check) => {
                if let Some(message) = evaluate(&s, check) {
                    failures.push(format!("step {index}: {message}"));
                }
            }
        }
        if let Some(error) = &s.sim.authority_error { return Err(error.clone()); }
    }
    let hash = s.hash_snapshot(s.world.ca_view.is_some())?;
    if failures.is_empty() && let Some(path) = &options.checkpoint_out {
        let region = s.sim.cpu_region.as_ref().ok_or("CPU checkpoint requires --ca-cpu-region")?;
        region.save_checkpoint(&s.world, s.ticks, std::path::Path::new(path))?;
    }
    if failures.is_empty() && let Some(directory) = &options.session_out {
        let captured = crate::world::storage::session::save(std::path::Path::new(directory), &mut s.world,
            &s.player, &s.day_night, &s.sim, &s.input).map_err(|e| format!("{e:#}"))?;
        if !captured { return Err("exact session deferred: region writeback must reduce capture memory".into()); }
    }
    let ca = s.sim.cpu_region.as_ref().map(cpu_region::Region::summary);
    Ok(Outcome { name: scenario.name.clone(), ticks: s.ticks, hash, failures, ca })
}

fn evaluate(s: &Session, check: &Check) -> Option<String> {
    match check {
        Check::Block { offset, is } | Check::BlockAt { at: offset, is } => {
            let at = if matches!(check, Check::Block { .. }) {
                s.feet() + IVec3::from(*offset)
            } else {
                IVec3::from(*offset)
            };
            let found = format!("{:?}", s.world.get(at));
            (&found != is).then(|| format!("block at {at:?} is {found}, expected {is}"))
        }
        Check::PlayerY { min, max } => {
            let y = s.player.pos.y;
            (!(*min..=*max).contains(&y)).then(|| format!("player y {y} outside [{min}, {max}]"))
        }
        Check::Finite => (!(s.player.pos.is_finite() && s.player.vel.is_finite())).then(|| {
            format!("non-finite player state pos={:?} vel={:?}", s.player.pos, s.player.vel)
        }),
        Check::Water { offset, min } => {
            let at = s.feet() + IVec3::from(*offset);
            let found = s.world.water_volume(at);
            (found < *min).then(|| format!("water at {at:?} is {found}, expected at least {min}"))
        }
        Check::Hash { is } => {
            let hash = match s.hash_snapshot(s.world.ca_view.is_some()) { Ok(hash) => hash, Err(error) => return Some(error) };
            let found = format!("{hash:016x}");
            (&found != is).then(|| format!("hash {found}, expected {is}"))
        }
    }
}

pub fn load(path: &str) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    ron::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// Entry for `--headless=<path>`. Prints the JSON summary; returns the exit code.
pub fn main(path: &str) -> i32 {
    let run = || {
        let options = options::parse()?;
        let scenario = load(path)?;
        run_with_region(&scenario, options)
    };
    match run() {
        Ok(outcome) => {
            println!("{}", outcome.json());
            i32::from(!outcome.failures.is_empty())
        }
        Err(message) => {
            println!("{{\"error\":\"{}\"}}", message.replace('"', "'"));
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::movement;

    fn scenarios() -> Vec<std::path::PathBuf> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios");
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .expect("assets/scenarios")
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "ron"))
            .collect();
        files.sort();
        files
    }

    /// Every scenario in assets/scenarios parses, passes its own checks and
    /// reproduces the committed summary (world hash included).
    #[test]
    fn scenarios_pass_and_match_their_snapshots() {
        for path in scenarios() {
            let scenario = load(path.to_str().unwrap()).unwrap();
            let outcome = run_scenario(&scenario).unwrap();
            assert!(outcome.failures.is_empty(), "{}: {:?}", path.display(), outcome.failures);
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            insta::assert_snapshot!(name, outcome.json());
        }
    }

    /// A placed block and a natural block both leave the world when written
    /// `Air`, in the same order the windowed tool loop uses.
    #[test]
    fn placed_and_natural_blocks_break_and_read_back() {
        let scenario = Scenario {
            name: "break_probe".into(),
            world: WorldType::Hills,
            scale: VoxelScalePreset::OneMeter,
            seed: 7,
            generator_version: None,
            steps: vec![
                Step::Advance { ticks: 120 },
                Step::Place { offset: [2, 0, 0], block: "Stone".into() },
                Step::Expect(Check::Block { offset: [2, 0, 0], is: "Stone".into() }),
                Step::Place { offset: [2, 0, 0], block: "Air".into() },
                Step::Expect(Check::Block { offset: [2, 0, 0], is: "Air".into() }),
                Step::Place { offset: [0, -1, 0], block: "Air".into() },
                Step::Expect(Check::Block { offset: [0, -1, 0], is: "Air".into() }),
            ],
        };
        let outcome = run_scenario(&scenario).expect("scenario runs");
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }

    /// A settled Superflat session facing -Z (yaw 0) with all Kinetic gear.
    fn flat() -> Session {
        let mut s = bare();
        s.player.gear_override = crate::world::inventory::Mobility::ALL;
        s
    }

    /// The same, wearing nothing that unlocks traversal.
    fn bare() -> Session {
        let mut s = Session::new(WorldType::Superflat, VoxelScalePreset::OneMeter, 7, None);
        s.advance(120);
        assert!(s.player.on_ground, "player settles on Superflat");
        s
    }

    /// A 1 m-thick stone wall along -Z, starting one block to the player's
    /// right (+X), floor to 6 m up, 40 m long.
    fn wall_on_right(s: &mut Session) {
        let feet = s.feet();
        for dz in -40..=2 {
            for dy in 0..6 {
                s.world.set(feet + IVec3::new(1, dy, dz), BlockId::Stone);
            }
        }
    }

    fn horizontal_speed(s: &Session) -> f32 {
        glam::Vec2::new(s.player.vel.x, s.player.vel.z).length()
    }

    /// Hold inputs for `ticks` steps; `setup` sets them on each tick. Keys not
    /// set by `setup` are released, as in `Step::Hold`.
    fn hold(s: &mut Session, ticks: u32, setup: impl Fn(&mut Input)) {
        for _ in 0..ticks {
            s.input = Input::default();
            setup(&mut s.input);
            s.tick();
        }
        s.input = Input::default();
    }

    #[test]
    fn movement_dash_bursts_then_carries_speed() {
        let mut s = flat();
        s.input.press_dash(true);
        s.tick();
        s.input.press_dash(false);
        assert!(s.player.movement.dashing());
        assert!(horizontal_speed(&s) > 15.0, "dash speed {}", horizontal_speed(&s));
        // After the 0.16 s burst, a second press inside the 0.75 s cooldown does nothing.
        hold(&mut s, 30, |_| {});
        assert!(!s.player.movement.dashing());
        s.input.press_dash(true);
        s.tick();
        s.input.press_dash(false);
        assert!(!s.player.movement.dashing());
        assert!(s.player.vel.is_finite() && s.player.pos.is_finite());
    }

    #[test]
    fn movement_air_dash_holds_height_and_is_spent_once() {
        let mut s = flat();
        hold(&mut s, 1, |i| i.jump = true);
        hold(&mut s, 20, |_| {});
        assert!(!s.player.on_ground);
        let y = s.player.pos.y;
        s.input.press_dash(true);
        s.tick();
        s.input.press_dash(false);
        assert!(s.player.movement.air_dash);
        hold(&mut s, 10, |i| i.forward = true);
        assert!((s.player.pos.y - y).abs() < 0.05, "air dash keeps height");
        assert_eq!(s.player.movement.dash_charges, 0);
    }

    #[test]
    fn movement_air_jump_once_without_toggling_flight() {
        let mut s = flat();
        hold(&mut s, 1, |i| i.jump = true);
        // Past the 0.25 s double-tap flight window, still rising or falling.
        hold(&mut s, 40, |_| {});
        assert!(!s.player.on_ground);
        hold(&mut s, 1, |i| i.jump = true);
        assert!(!s.player.flying, "air jump must not toggle flight");
        assert!(s.player.vel.y > 7.0, "air jump launches: {}", s.player.vel.y);
        hold(&mut s, 8, |_| {});
        let vy = s.player.vel.y;
        hold(&mut s, 1, |i| i.jump = true);
        assert!(s.player.vel.y < vy, "second air jump is refused");
        assert!(!s.player.flying);
    }

    #[test]
    fn movement_quick_double_tap_still_toggles_flight() {
        let mut s = flat();
        hold(&mut s, 1, |i| i.jump = true);
        hold(&mut s, 6, |_| {});
        hold(&mut s, 1, |i| i.jump = true);
        assert!(s.player.flying);
    }

    #[test]
    fn movement_slide_boosts_then_slide_jump_carries_it() {
        let mut s = flat();
        hold(&mut s, 120, |i| { i.forward = true; i.sprint = true; });
        let run = horizontal_speed(&s);
        hold(&mut s, 1, |i| { i.forward = true; i.sprint = true; i.sneak = true; });
        assert!(s.player.movement.sliding);
        assert!(horizontal_speed(&s) > run + 2.0, "slide boost {} vs {run}", horizontal_speed(&s));
        hold(&mut s, 20, |i| { i.forward = true; i.sneak = true; });
        assert!(s.player.movement.sliding);
        hold(&mut s, 1, |i| { i.forward = true; i.sneak = true; i.jump = true; });
        assert!(!s.player.movement.sliding);
        assert!(s.player.vel.y > 0.0);
        // Momentum is carried in the air above sprint speed.
        hold(&mut s, 20, |i| i.forward = true);
        assert!(horizontal_speed(&s) > 9.0, "carried {}", horizontal_speed(&s));
    }

    #[test]
    fn movement_slide_needs_speed_and_ends() {
        let mut s = flat();
        hold(&mut s, 1, |i| i.sneak = true);
        assert!(!s.player.movement.sliding, "standing sneak is not a slide");
        hold(&mut s, 120, |i| { i.forward = true; i.sprint = true; });
        hold(&mut s, 300, |i| i.sneak = true);
        assert!(!s.player.movement.sliding, "slide ends by time or speed");
        assert!(horizontal_speed(&s) < 3.0);
    }

    fn cues(s: &mut Session) -> Vec<crate::audio::Cue> {
        std::mem::take(&mut s.world.adventure.sounds).into_iter().map(|e| e.cue).collect()
    }

    #[test]
    fn movement_actions_queue_their_sounds_once() {
        use crate::audio::Cue;
        let mut s = flat();
        cues(&mut s);
        s.input.press_dash(true);
        s.tick();
        s.input.press_dash(false);
        assert!(cues(&mut s).contains(&Cue::Dash));
        hold(&mut s, 10, |_| {});
        assert!(!cues(&mut s).contains(&Cue::Dash), "one cue per dash");

        hold(&mut s, 120, |i| { i.forward = true; i.sprint = true; });
        cues(&mut s);
        hold(&mut s, 1, |i| { i.forward = true; i.sprint = true; i.sneak = true; });
        assert!(cues(&mut s).contains(&Cue::Slide));

        hold(&mut s, 200, |_| {});
        hold(&mut s, 1, |i| i.jump = true);
        hold(&mut s, 40, |_| {});
        cues(&mut s);
        hold(&mut s, 1, |i| i.jump = true);
        assert!(cues(&mut s).contains(&Cue::AirJump));
        // An ordinary hop lands softly: no thump.
        hold(&mut s, 200, |_| {});
        assert!(!cues(&mut s).contains(&Cue::Land));
    }

    #[test]
    fn movement_long_fall_lands_with_a_thump() {
        use crate::audio::Cue;
        let mut s = flat();
        let p = s.player.pos;
        s.player.pos = p + glam::Vec3::Y * 30.0;
        hold(&mut s, 400, |_| {});
        assert!(s.player.on_ground);
        assert!(cues(&mut s).contains(&Cue::Land));
    }

    #[test]
    fn movement_needs_kinetic_gear() {
        let mut s = bare();
        s.input.press_dash(true);
        s.tick();
        s.input.press_dash(false);
        assert!(!s.player.movement.dashing(), "no greaves, no dash");
        hold(&mut s, 1, |i| i.jump = true);
        hold(&mut s, 40, |_| {});
        let vy = s.player.vel.y;
        hold(&mut s, 1, |i| i.jump = true);
        assert!(s.player.vel.y < vy, "no greaves, no air jump");
        // The slide is a body skill and needs no gear.
        hold(&mut s, 200, |i| { i.forward = true; i.sprint = true; });
        hold(&mut s, 1, |i| { i.forward = true; i.sprint = true; i.sneak = true; });
        assert!(s.player.movement.sliding);
        // Worn greaves from the real inventory unlock the dash.
        s.world.inventory.equipment[crate::world::inventory::EquipSlot::Legs as usize] =
            Some(crate::world::inventory::ItemStack::new(
                crate::world::inventory::Item::Armor(
                    crate::world::inventory::EquipSlot::Legs,
                    crate::world::inventory::ArmorTier::Kinetic,
                ),
                1,
            ));
        hold(&mut s, 200, |_| {});
        s.input.press_dash(true);
        s.tick();
        s.input.press_dash(false);
        assert!(s.player.movement.dashing(), "greaves unlock the dash");
        assert!(!s.player.gear.wallrun, "greaves are not boots");
    }

    /// Sprint along the wall, jump, keep forward held.
    fn run_up_to_wall(s: &mut Session) {
        wall_on_right(s);
        hold(s, 90, |i| { i.forward = true; i.sprint = true; });
        hold(s, 1, |i| { i.forward = true; i.sprint = true; i.jump = true; });
    }

    #[test]
    fn movement_wallrun_holds_height_and_carries_speed() {
        let mut s = flat();
        run_up_to_wall(&mut s);
        let mut ran = false;
        for _ in 0..40 {
            hold(&mut s, 1, |i| { i.forward = true; i.sprint = true; });
            ran |= s.player.movement.wallrunning;
        }
        assert!(ran, "wall run starts beside a wall");
        assert!(s.player.movement.wallrunning);
        assert!(s.player.movement.camera_roll().abs() > 0.05, "camera leans");
        let y = s.player.pos.y;
        hold(&mut s, 60, |i| { i.forward = true; i.sprint = true; });
        assert!(s.player.pos.y > y - 1.0, "low gravity on the wall: {} -> {}", y, s.player.pos.y);
        assert!(horizontal_speed(&s) > 9.5, "run speed {}", horizontal_speed(&s));
        // The run ends on its own; gravity returns and the player lands.
        hold(&mut s, 400, |i| { i.forward = true; i.sprint = true; });
        assert!(!s.player.movement.wallrunning);
        assert!(s.player.vel.is_finite() && s.player.pos.is_finite());
    }

    #[test]
    fn movement_walljump_kicks_away_from_the_wall() {
        let mut s = flat();
        run_up_to_wall(&mut s);
        hold(&mut s, 40, |i| { i.forward = true; i.sprint = true; });
        assert!(s.player.movement.wallrunning);
        let x = s.player.pos.x;
        hold(&mut s, 1, |i| { i.forward = true; i.jump = true; });
        assert!(!s.player.movement.wallrunning);
        assert!(s.player.vel.y > 7.0);
        assert!(s.player.vel.x < -4.0, "pushed off the wall (-X): {}", s.player.vel.x);
        hold(&mut s, 20, |i| i.forward = true);
        assert!(s.player.pos.x < x - 1.0);
        assert_eq!(s.player.movement.air_jumps_used, 0, "wall jump refunds the air jump");
    }

    #[test]
    fn movement_no_wallrun_without_boots() {
        let mut s = bare();
        run_up_to_wall(&mut s);
        for _ in 0..60 {
            hold(&mut s, 1, |i| { i.forward = true; i.sprint = true; });
            assert!(!s.player.movement.wallrunning);
        }
    }

    #[test]
    fn movement_air_strafe_gains_speed_while_turning() {
        // Quake: holding a strafe key while turning toward it adds speed in
        // the air; without boots the same input only bleeds speed off.
        fn strafe(mut s: Session) -> f32 {
            s.input.press_dash(true);
            s.tick();
            s.input.press_dash(false);
            hold(&mut s, 1, |i| i.jump = true);
            hold(&mut s, 18, |_| {});
            let start = horizontal_speed(&s);
            for _ in 0..50 {
                // Hold D and keep the mouse turning so the strafe direction
                // stays just ahead of perpendicular to the velocity, as a
                // player tracking the curve does. right_dir = (cos yaw, sin yaw).
                let heading = s.player.vel.z.atan2(s.player.vel.x);
                s.player.yaw = heading + 1.50;
                hold(&mut s, 1, |i| i.right = true);
                if s.player.on_ground { break; }
            }
            horizontal_speed(&s) - start
        }
        let with_boots = strafe(flat());
        let mut greaves_only = bare();
        greaves_only.player.gear_override.thrusters = true;
        let without = strafe(greaves_only);
        assert!(with_boots > -0.2, "strafing holds speed: {with_boots}");
        assert!(with_boots > without + 0.5, "boots {with_boots} vs none {without}");
    }

    #[test]
    fn movement_speed_cap_rises_with_gear() {
        use crate::world::inventory::Mobility;
        let (none, legs, all) = (
            movement::speed_cap(Mobility::default()),
            movement::speed_cap(Mobility { thrusters: true, wallrun: false }),
            movement::speed_cap(Mobility::ALL),
        );
        assert!(none < legs && legs < all);
        assert!(legs >= movement::DASH_SPEED, "greaves can reach dash speed");
    }

    #[test]
    fn movement_walk_speed_matches_the_old_controller() {
        let mut s = flat();
        hold(&mut s, 240, |i| i.forward = true);
        let v = horizontal_speed(&s);
        assert!((v - crate::world::WALK_SPEED).abs() < 0.05, "walk {v}");
        hold(&mut s, 240, |i| { i.forward = true; i.sprint = true; });
        let v = horizontal_speed(&s);
        assert!((v - crate::world::SPRINT_SPEED).abs() < 0.05, "sprint {v}");
    }

    #[test]
    fn the_same_scenario_hashes_the_same_twice() {
        let scenario = load(scenarios()[0].to_str().unwrap()).unwrap();
        let a = run_scenario(&scenario).unwrap().hash;
        let b = run_scenario(&scenario).unwrap().hash;
        assert_eq!(a, b);
    }
}
