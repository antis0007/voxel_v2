//! Headless, deterministic scenario runner: `--headless=<scenario.ron>`.
//!
//! No window, no GPU. A scenario builds a world from a seed, applies steps
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
    },
    /// Set a block at a voxel offset from the player's feet.
    Place { offset: [i32; 3], block: String },
    /// Set a block at an absolute voxel coordinate.
    PlaceAt { at: [i32; 3], block: String },
    /// Exact development-region water volume (0..=64), without legacy rounding.
    Water64 { at: [i32; 3], volume: u8 },
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
        let mut world = World::with_seed(world_type, scale, seed);
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
        fixed::gameplay(world, &mut self.player, &mut self.input, true, |_, _, _, input| {
            fixed::Actions { cast: input.cast_spell || input.cast_pressed,
                cycle: input.cycle_spell, melee: false }
        });
        fixed::finish(&mut self.sim, world, &self.player, &mut self.day_night,
            &mut self.dirty_blocks, &mut self.dirty_chunks, Budget::UNBOUNDED, true, true);
        self.ticks = self.sim.player_tick.0;
    }

    pub fn advance(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.tick();
            if self.sim.authority_error.is_some() { break; }
        }
    }

    fn feet(&self) -> IVec3 {
        (self.player.pos / self.world.voxel_scale.meters()).floor().as_ivec3()
    }

    /// FNV-1a over resident chunk blocks, player pose, factory node coordinates
    /// and inventory stacks. Stable across runs of the same scenario.
    pub fn hash(&self) -> u64 {
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
        coords.sort_unstable_by_key(|c| (c.y, c.z, c.x));
        for c in coords {
            feed(&c.x.to_le_bytes());
            feed(&c.y.to_le_bytes());
            feed(&c.z.to_le_bytes());
            let blocks: Vec<u8> = self.world.chunks[&c].blocks.iter().map(|&b| b as u8).collect();
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
        h
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
    run_with_region(scenario, None)
}

fn run_with_region(scenario: &Scenario, origin: Option<IVec3>) -> Result<Outcome, String> {
    let mut s = Session::new(scenario.world, scenario.scale, scenario.seed, scenario.generator_version);
    if let Some(origin) = origin {
        let region = cpu_region::Region::import(&s.world, origin)?;
        s.world.ca_view = Some(region.published.clone());
        s.sim.cpu_region = Some(region);
    }
    let mut failures = Vec::new();
    for (index, step) in scenario.steps.iter().enumerate() {
        match step {
            Step::Advance { ticks } => s.advance(*ticks),
            Step::Hold { ticks, forward, back, left, right, jump, sprint, cast } => {
                s.input.forward = *forward;
                s.input.back = *back;
                s.input.left = *left;
                s.input.right = *right;
                s.input.jump = *jump;
                s.input.sprint = *sprint;
                s.input.cast_spell = *cast;
                s.input.cast_pressed = *cast;
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
                    let cell = if id == BlockId::Air { engine_sim::Cell::EMPTY } else { engine_sim::Cell::filled(id as u8) };
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
    let ca = s.sim.cpu_region.as_ref().map(cpu_region::Region::summary);
    Ok(Outcome { name: scenario.name.clone(), ticks: s.ticks, hash: s.hash(), failures, ca })
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
            let found = format!("{:016x}", s.hash());
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
    let origin = std::env::args().find_map(|a| a.strip_prefix("--ca-cpu-region=").map(str::to_owned));
    let run = || {
        if origin.is_none() && std::env::args().any(|a| a.starts_with("--ca-cpu-region")) {
            return Err("expected --ca-cpu-region=x,y,z".into());
        }
        let origin = origin.map(|text| {
            let values: Result<Vec<i32>, _> = text.split(',').map(str::parse).collect();
            let values = values.map_err(|_| "invalid --ca-cpu-region=x,y,z")?;
            if values.len() != 3 { return Err("expected --ca-cpu-region=x,y,z"); }
            Ok(IVec3::new(values[0], values[1], values[2]))
        }).transpose()?;
        let scenario = load(path)?;
        if origin.is_some() { run_with_region(&scenario, origin) } else { run_scenario(&scenario) }
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

    #[test]
    fn the_same_scenario_hashes_the_same_twice() {
        let scenario = load(scenarios()[0].to_str().unwrap()).unwrap();
        let a = run_scenario(&scenario).unwrap().hash;
        let b = run_scenario(&scenario).unwrap().hash;
        assert_eq!(a, b);
    }
}
