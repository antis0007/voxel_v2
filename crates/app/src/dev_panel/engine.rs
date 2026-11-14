//! Typed, bounded workbench commands. Humans and agents use the same native
//! validation, revision checks, inspection and reversible preview state.
use crate::{material_studio, mesh::models::voxel_asset, player::Player, world::{self, DayNightCycle, World}};
use serde_json::{Value, json};
use super::machine;
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};

static CAPTURE: Mutex<Option<PathBuf>> = Mutex::new(None);
pub fn take_capture() -> Option<PathBuf> { CAPTURE.lock().unwrap().take() }

/// Latest frame-time summary from the running game, published at the UI snapshot
/// cadence (5 Hz) and only while the bridge is open. Never feeds simulation.
static LIVE_PERF: Mutex<Option<(std::time::Instant, Value)>> = Mutex::new(None);
fn cpu_stages(c: &crate::perf::CpuFrameStats) -> [(&'static str, f32); 10] {
    [("ui", c.ui_ms), ("gameplay", c.gameplay_ms), ("snapshot", c.snapshot_ms), ("streaming", c.streaming_ms),
        ("horizon", c.horizon_ms), ("globe", c.globe_ms), ("culling", c.culling_ms), ("lights", c.torch_ms),
        ("weather", c.weather_ms), ("render", c.render_ms)]
}
fn gpu_passes(r: &crate::render::RenderFrameStats) -> [(&'static str, f32); 10] {
    [("shadow", r.gpu_shadow_ms), ("terrain", r.gpu_terrain_ms), ("indirect", r.gpu_indirect_ms),
        ("cloud_raymarch", r.gpu_cloud_raymarch_ms), ("cloud_temporal", r.gpu_cloud_temporal_ms), ("sky", r.gpu_sky_ms),
        ("sky_probe", r.gpu_sky_probe_ms), ("cloud_composite", r.gpu_cloud_composite_ms),
        ("postprocess", r.gpu_postprocess_ms), ("ui", r.gpu_ui_ms)]
}
fn stage_ms(stages: &[(&'static str, f32)]) -> Value {
    Value::Object(stages.iter().map(|&(name, ms)| (name.to_string(), json!((ms * 100.0).round() / 100.0))).collect())
}
/// The three largest averaged costs, CPU stages and GPU passes together, so a
/// slow session names its own cause. `cpu.render` includes waiting on the GPU.
fn largest_costs(snapshot: &crate::perf::PerfSnapshot) -> Vec<String> {
    let mut all: Vec<(String, f32)> = cpu_stages(&snapshot.cpu_avg).iter().map(|&(n, ms)| (format!("cpu.{n}"), ms)).collect();
    if snapshot.render_avg.gpu_timestamps_supported {
        all.extend(gpu_passes(&snapshot.render_avg).iter().map(|&(n, ms)| (format!("gpu.{n}"), ms)));
    }
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    all.iter().take(3).map(|(name, ms)| format!("{name} {ms:.2} ms")).collect()
}
/// Gameplay and streaming sub-stages since the previous publish, in ms per
/// frame, largest first. Splits `cpu.gameplay` and `cpu.streaming`.
fn stage_costs(fps: f32) -> Value {
    let (seconds, mut stages) = crate::gameplay_trace::take_stages();
    let frames = (seconds * f64::from(fps)).max(1.0);
    stages.sort_by(|a, b| b.1.total_cmp(&a.1));
    Value::Object(stages.into_iter().take(10)
        .map(|(name, ms)| (name, json!((ms / frames * 100.0).round() / 100.0))).collect())
}
pub fn publish_perf(snapshot: &crate::perf::PerfSnapshot) {
    let (c, s) = (&snapshot.counters_latest, &snapshot.chunk_latest);
    let value = json!({
        "samples": snapshot.sample_count,
        "fps_avg": snapshot.fps_avg, "fps_1p_low": snapshot.fps_1p_low,
        "frame_ms": {"avg": snapshot.frame_ms_avg, "p50": snapshot.frame_ms_p50, "p95": snapshot.frame_ms_p95,
            "p99": snapshot.frame_ms_p99, "max": snapshot.frame_ms_max},
        "cpu_gameplay_ms_avg": snapshot.cpu_avg.gameplay_ms,
        "resolution": snapshot.resolution, "adapter": snapshot.adapter_summary,
        "view_distance_chunks": snapshot.render_settings.view_distance_chunks,
        "largest": largest_costs(snapshot),
        "stages_ms": stage_costs(snapshot.fps_avg),
        "cpu_ms_avg": stage_ms(&cpu_stages(&snapshot.cpu_avg)),
        "gpu_ms_avg": if snapshot.render_avg.gpu_timestamps_supported { stage_ms(&gpu_passes(&snapshot.render_avg)) } else { Value::Null },
        "scene": {"draw_chunks": c.draw_chunks, "terrain_triangles": c.terrain_triangles, "lights": c.nearby_torches,
            "shadow_draws": snapshot.render_latest.shadow_draw_calls, "shadow_redraw": snapshot.render_latest.shadow_redraw,
            "visible_missing": c.visible_missing, "near_missing": c.near_missing},
        "streaming": {"pending": s.pending_schedule, "parked_offscreen": s.parked_offscreen, "generating": s.generating,
            "meshing": s.meshing, "ready_chunks": s.ready_chunks, "ready_meshes": s.ready_meshes,
            "cpu_mib": s.cpu_resident_bytes >> 20, "gpu_mib": s.gpu_resident_bytes >> 20,
            "cpu_evictions": s.cpu_evictions, "gpu_evictions": s.gpu_evictions},
    });
    if let Ok(mut slot) = LIVE_PERF.lock() { *slot = Some((std::time::Instant::now(), value)); }
}
fn perf_report() -> Result<Value, String> {
    let slot = LIVE_PERF.lock().map_err(|_| "Performance state is unavailable".to_string())?;
    let (at, value) = slot.as_ref().ok_or("No frames measured yet; wait for a world to render")?;
    Ok(json!({"perf": value, "age_ms": at.elapsed().as_millis() as u64,
        "scope": "Rolling frame history of this session on this hardware; one observation, not a controlled A/B"}))
}

#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Describe { #[serde(default)] system: Option<String> },
    Inspect { #[serde(default)] sections: Vec<String> },
    Survey { radius_m: f32 },
    Tune { revision: u64, values: BTreeMap<String, Value> },
    Material { revision: u64, family: usize, tint: [f32; 3], roughness: Option<f32>, reflectance: Option<f32> },
    Model { revision: u64, asset: String, text: String },
    Undo { revision: u64 },
    Reset { revision: u64 },
    Vitals { mana: Option<f32>, health: Option<f32>, hunger: Option<f32> },
    Camera { position_m: [f64; 3], yaw: Option<f32>, pitch: Option<f32> },
    Time { fraction: f32 },
    Capture { name: String },
    /// Rolling frame-time percentiles from the live session.
    Perf,
    Spells { #[serde(default = "default_tier")] tier: u32 },
    Wand { build: super::wand_lab::Build, #[serde(default)] equip: bool },
    /// Any solo chat command, e.g. `/give`, `/time`, `/gamemode`, `/summon`.
    Command { line: String },
    /// World systems (assets/systems.ron): lists them, or switches `set` live and saves the world sidecar.
    Systems { #[serde(default)] set: BTreeMap<String, bool> },
    /// Fills an inclusive box of voxel cells with one block.
    Fill { min: [i32; 3], max: [i32; 3], block: world::BlockId },
    Probe { position: [i32; 3] },
    /// Mana hedrons within `radius_m`, with the cells a capture frame needs.
    Hedrons { #[serde(default = "default_hedron_radius")] radius_m: f32 },
    /// Plants a mature mana flower at `position`, drops `feed` of the item it eats beside it
    /// and runs `pulses` two-second flower beats; reports items eaten and where the mana went.
    ManaFlower { position: [i32; 3], #[serde(default)] feed: u32, #[serde(default = "one")] pulses: u32 },
    /// Creatures within `radius_m` of the player, nearest first (at most 64).
    Enemies { #[serde(default = "default_machine_radius")] radius_m: f32 },
    /// Summons `count` creatures at `position_m` (default 3 m ahead), with optional affixes and element hits.
    SpawnEnemy { kind: String, #[serde(default)] position_m: Option<[f32; 3]>, #[serde(default = "one")] count: u32,
        #[serde(default)] affixes: Vec<String>, #[serde(default)] elements: Vec<String>, #[serde(default)] health: Option<f32> },
    /// Removes creatures within `radius_m` of the player, or all of them.
    ClearEnemies { #[serde(default)] radius_m: Option<f32> },
    /// Machine kinds with their ids and capability flags.
    MachineKinds,
    /// Full machine state at a cell, or the machine the player looks at.
    Machine { #[serde(default)] position: Option<[i32; 3]> },
    /// Machines within `radius_m` of the player, nearest first (at most 64).
    Machines { #[serde(default = "default_machine_radius")] radius_m: f32, #[serde(default)] kind: Option<String> },
    /// Places a machine without a kit, like review fixtures.
    PlaceMachine { position: [i32; 3], kind: String, #[serde(default)] facing: Option<[i32; 3]>,
        #[serde(default)] mode: Option<u32>, #[serde(default)] enabled: Option<bool> },
    /// Places a multiblock controller with its complete brick shell behind it (`into`, default +x).
    PlaceShell { position: [i32; 3], kind: String, #[serde(default)] into: Option<[i32; 3]> },
    /// Configures a placed machine and stocks its buffers for a test.
    SetMachine { position: [i32; 3], #[serde(default)] mode: Option<u32>, #[serde(default)] target: Option<u32>,
        #[serde(default)] enabled: Option<bool>, #[serde(default)] recipe: Option<i64>,
        #[serde(default)] filter: Option<i64>, #[serde(default)] energy: Option<u32>,
        #[serde(default)] input: BTreeMap<String, u32>, #[serde(default)] clear_output: bool,
        #[serde(default)] upgrade: Option<u32>, #[serde(default)] water: Option<u32> },
    /// Fast-forwards power and machine service by fixed steps; returns changed fields per cell.
    StepFactory { ticks: u32, #[serde(default)] probe: Vec<[i32; 3]> },
    /// One call: optionally place `kind`, apply `set`, step `ticks`, return the end state and changes.
    MachineTest { position: [i32; 3], #[serde(default)] kind: Option<String>, #[serde(default)] facing: Option<[i32; 3]>,
        #[serde(default)] set: Setup, #[serde(default)] ticks: u32, #[serde(default)] probe: Vec<[i32; 3]> },
}

/// Machine configuration shared by `set_machine` and `machine_test`. Recipe/filter -1 clears.
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Setup {
    #[serde(default)] mode: Option<u32>, #[serde(default)] target: Option<u32>, #[serde(default)] enabled: Option<bool>,
    #[serde(default)] recipe: Option<i64>, #[serde(default)] filter: Option<i64>, #[serde(default)] energy: Option<u32>,
    #[serde(default)] input: BTreeMap<String, u32>, #[serde(default)] clear_output: bool,
    /// Drive or cell level 0..=2, and stored water units, for tests that need them.
    #[serde(default)] upgrade: Option<u32>, #[serde(default)] water: Option<u32>,
}

fn place(w: &mut World, position: [i32; 3], kind: &str, facing: Option<[i32; 3]>) -> Result<glam::IVec3, String> {
    let kind = machine::kind_named(kind).ok_or(format!("Unknown machine kind {kind}; see machine_kinds"))?;
    let facing = facing.map(glam::IVec3::from_array);
    if facing.is_some_and(|f| f.abs().element_sum() != 1) { return Err("facing must be one unit axis".into()); }
    let cell = glam::IVec3::from_array(position);
    let node = world::factory::fixture_place(w, cell, kind).ok_or("placement failed")?;
    if let Some(f) = facing { node.facing = f; }
    Ok(cell)
}

fn configure(w: &mut World, cell: glam::IVec3, s: Setup) -> Result<(), String> {
    let anchor = world::factory::probe(w, cell).ok_or_else(|| format!("no machine at {:?}", cell.to_array()))?.pos;
    let mut stocked = Vec::with_capacity(s.input.len());
    for (key, count) in &s.input { stocked.push((machine::resource_named(key).ok_or(format!("Unknown resource {key}"))?, *count)); }
    let node = w.factory.nodes.get_mut(&anchor).ok_or("machine vanished")?;
    if let Some(v) = s.mode { node.mode = v; }
    if let Some(v) = s.target { node.target = v; }
    if let Some(v) = s.enabled { node.enabled = v; }
    if let Some(v) = s.recipe { node.recipe = usize::try_from(v).ok(); }
    if let Some(v) = s.filter { node.filter = usize::try_from(v).ok(); }
    if let Some(v) = s.energy { node.energy = v; }
    if let Some(v) = s.upgrade { node.upgrade = v.min(2); }
    if let Some(v) = s.water { node.water = v; }
    for (index, count) in stocked { node.input[index] = count; }
    if s.clear_output { node.output = [0; world::factory::RESOURCE_COUNT]; }
    Ok(())
}

fn report(w: &World, cell: glam::IVec3) -> Result<Value, String> {
    world::factory::probe(w, cell).map(|p| machine::json(&p)).ok_or_else(|| "chunk not resident".into())
}

/// Steps the factory and returns, per cell, the end state of the first cell and `{field: [before, after]}` changes.
fn step_report(w: &mut World, player: &Player, ticks: u32, cells: &[glam::IVec3]) -> Result<Value, String> {
    if ticks == 0 || ticks > machine::MAX_STEPS { return Err(format!("ticks 1..{}", machine::MAX_STEPS)); }
    if cells.len() > 16 { return Err("at most 16 probe cells".into()); }
    let read = |w: &World| -> Vec<Value> { cells.iter().map(|c| world::factory::probe(w, *c).map_or(Value::Null, |p| machine::json(&p))).collect() };
    let before = read(w);
    machine::step(w, player, ticks);
    let after = read(w);
    let changes: serde_json::Map<_, _> = cells.iter().zip(before.iter().zip(&after))
        .map(|(c, (a, b))| (format!("{} {} {}", c.x, c.y, c.z), machine::delta(a, b))).collect();
    let mut out = json!({"s": ticks as f32 * crate::simulation::STEP_SECONDS, "changes": changes});
    if let Some(first) = after.first() { out["end"] = first.clone(); }
    Ok(out)
}
fn default_machine_radius() -> f32 { 32.0 }
fn default_hedron_radius() -> f32 { 256.0 }
fn one() -> u32 { 1 }

fn named<T: Copy + std::fmt::Debug>(all: &[T], name: &str, what: &str) -> Result<T, String> {
    all.iter().copied().find(|v| format!("{v:?}").eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("{what}: one of {all:?}"))
}

fn enemy_report(e: &world::adventure::Enemy, at: glam::Vec3) -> Value {
    let s = &e.status;
    let mut out = json!({"kind":e.kind,"position_m":e.pos.to_array(),"distance_m":e.pos.distance(at),"health":e.health});
    let affixes: Vec<_> = e.affixes.list().into_iter().map(|a| a.name()).collect();
    if !affixes.is_empty() { out["affixes"] = json!(affixes); }
    if e.shield > 0.0 { out["shield"] = json!(e.shield); }
    for (key, v) in [("burn", s.burn), ("poison", s.poison), ("shock", s.shock), ("wet", s.wet)] {
        if v > 0.0 { out[key] = json!(v); }
    }
    out
}
fn default_tier() -> u32 { 3 }

struct Snapshot {
    tuning: world::ambience::Tuning,
    materials: Option<material_studio::Palette>,
    models: voxel_asset::Previews,
}
impl Snapshot {
    fn current() -> Self { Self { tuning: world::ambience::tuning(), materials: material_studio::preview_palette(), models: voxel_asset::previews() } }
    fn restore(self) {
        let _ = world::ambience::tune(self.tuning);
        material_studio::set_preview_palette(self.materials);
        voxel_asset::restore_previews(self.models);
    }
}

#[derive(Default)]
pub struct Engine {
    revision: u64,
    undo: Vec<Snapshot>,
    world_root: Option<std::sync::Arc<PathBuf>>,
}

impl Engine {
    pub fn bind(&mut self, world: Option<&World>) {
        let next = world.map(|w| w.regions.view.root.clone());
        let changed = self.world_root.as_ref().is_some_and(|old|
            next.as_ref().is_none_or(|new| !std::sync::Arc::ptr_eq(old,new)));
        if changed {
            let _ = world::ambience::tune(Default::default());
            material_studio::set_preview_palette(None);
            voxel_asset::restore_previews(Default::default());
            self.undo.clear();
            self.revision += 1;
        }
        self.world_root = next;
    }
    pub fn handle(&mut self, source: &str, world: Option<&mut World>, player: &mut Player, clock: &mut DayNightCycle) -> Value {
        let result = serde_json::from_str::<Request>(source).map_err(|e| e.to_string())
            .and_then(|r| self.execute(r, world, player, clock));
        match result {
            Ok(data) => json!({"ok":true,"protocol":1,"revision":self.revision,"data":data}),
            Err(error) => json!({"ok":false,"protocol":1,"revision":self.revision,"error":error}),
        }
    }

    fn expect(&self, revision: u64) -> Result<(), String> {
        if revision != self.revision { return Err(format!("Revision conflict: expected {}, received {revision}; inspect before applying", self.revision)); }
        Ok(())
    }
    fn changed(&mut self, before: Snapshot) {
        self.undo.push(before);
        if self.undo.len() > 16 { self.undo.remove(0); }
        self.revision += 1;
    }

    fn execute(&mut self, request: Request, world: Option<&mut World>, player: &mut Player, clock: &mut DayNightCycle) -> Result<Value, String> {
        if let Request::Describe { system } = &request { return describe(system.as_deref()); }
        if let Request::Perf = &request { return perf_report(); }
        if let Request::Spells { tier } = &request { return Ok(super::wand_lab::catalog(*tier)); }
        if let Request::Wand { build, equip: false } = &request { return super::wand_lab::simulate(build); }
        if let Request::MachineKinds = &request { return Ok(json!({"cols": "id name flags(p powered,r processor,s structural,x excavates)", "kinds": machine::kinds()})); }
        let w = world.ok_or("Open a connected solo world; hosted, joined and cellular sessions are excluded")?;
        match request {
            Request::Describe { .. } | Request::Perf | Request::Spells { .. } | Request::Wand { equip: false, .. }
                | Request::MachineKinds => unreachable!(),
            Request::Machine { position } => {
                let cell = match position {
                    Some(p) => glam::IVec3::from_array(p),
                    None => look_cell(w, player).ok_or("Machine: no block in reach; pass position")?,
                };
                let probe = world::factory::probe(w, cell).ok_or_else(|| format!("Machine: none at {:?}", cell.to_array()))?;
                Ok(machine::json(&probe))
            }
            Request::Machines { radius_m, kind } => {
                bounded(radius_m, 1.0, 512.0, "machine radius metres")?;
                let kind = kind.map(|k| machine::kind_named(&k).ok_or(format!("Unknown machine kind {k}; see machine_kinds"))).transpose()?;
                let scale = w.voxel_scale.meters();
                Ok(machine::nearby(w, player.pos.floor().as_ivec3(), (radius_m / scale).ceil() as i32, kind))
            }
            Request::PlaceMachine { position, kind, facing, mode, enabled } => {
                let cell = place(w, position, &kind, facing)?;
                configure(w, cell, Setup { mode, enabled, ..Setup::default() })?;
                report(w, cell)
            }
            Request::PlaceShell { position, kind, into } => {
                let controller = machine::kind_named(&kind).ok_or(format!("Unknown machine kind {kind}; see machine_kinds"))?;
                let brick = world::factory::multiblock::brick_kind(controller).ok_or("Not a multiblock controller")?;
                let into = into.map_or(glam::IVec3::X, glam::IVec3::from_array);
                if into.y != 0 || into.abs().element_sum() != 1 { return Err("into must be one horizontal unit axis".into()); }
                let cell = glam::IVec3::from_array(position);
                let size = world::factory::step(w);
                world::factory::fixture_place(w, cell, controller).ok_or("placement failed")?;
                let bricks = world::factory::multiblock::shell(controller, cell, into, size);
                for c in &bricks { world::factory::fixture_place(w, *c, brick).ok_or("brick placement failed")?; }
                Ok(json!({"at": position, "bricks": bricks.len()}))
            }
            Request::SetMachine { position, mode, target, enabled, recipe, filter, energy, input, clear_output, upgrade, water } => {
                let cell = glam::IVec3::from_array(position);
                configure(w, cell, Setup { mode, target, enabled, recipe, filter, energy, input, clear_output, upgrade, water })?;
                report(w, cell)
            }
            Request::StepFactory { ticks, probe } => {
                let cells: Vec<_> = probe.into_iter().map(glam::IVec3::from_array).collect();
                step_report(w, player, ticks, &cells)
            }
            Request::MachineTest { position, kind, facing, set, ticks, probe } => {
                let cell = match kind { Some(kind) => place(w, position, &kind, facing)?, None => glam::IVec3::from_array(position) };
                configure(w, cell, set)?;
                let mut cells = vec![cell];
                cells.extend(probe.into_iter().map(glam::IVec3::from_array));
                if ticks == 0 { return report(w, cell); }
                step_report(w, player, ticks, &cells)
            }
            Request::Wand { build, equip: true } => {
                let mut report = super::wand_lab::simulate(&build)?;
                report["equipped"] = super::wand_lab::equip(w, &build)?;
                Ok(report)
            }
            Request::Inspect { sections } => inspect(w, player, clock, &sections),
            Request::Survey { radius_m } => {
                bounded(radius_m, 4.0, 2048.0, "survey radius metres")?;
                let scale = w.voxel_scale.meters();
                let cfg = w.sample_config();
                let centre = player.pos.floor().as_ivec3();
                let reach = (radius_m / scale).round() as i32;
                let sites = [(0,0),(0,-1),(1,0),(0,1),(-1,0),(1,-1),(1,1),(-1,1),(-1,-1)]
                    .map(|(dx,dz)| {
                        let x = centre.x + dx * reach;
                        let z = centre.z + dz * reach;
                        let biome = world::landscape::sample(&cfg,x,z).biome;
                        let y = w.terrain_height_at(x,z);
                        json!({"surface_m":[(x as f32+0.5)*scale,(y as f32+1.0)*scale,(z as f32+0.5)*scale],
                            "biome":format!("{biome:?}")})
                    });
                Ok(json!({"radius_m":radius_m,"sites":sites,
                    "scope":"Nine generated surface samples; not collision clearance or a measured local ambience probe"}))
            }
            Request::Tune { revision, values } => {
                self.expect(revision)?;
                if values.is_empty() || values.len() > 8 { return Err("Choose 1..8 tuning fields".into()); }
                let before = Snapshot::current();
                let mut candidate = serde_json::to_value(before.tuning).unwrap();
                for (key, value) in values {
                    let field = candidate.get_mut(&key).ok_or_else(|| format!("Unknown tuning field: {key}; describe ambience for supported fields"))?;
                    *field = value;
                }
                let candidate = serde_json::from_value(candidate).map_err(|e| format!("Invalid tuning: {e}"))?;
                world::ambience::tune(candidate)?;
                self.changed(before);
                Ok(json!({"applied":"live","scope":"session","tuning":candidate}))
            }
            Request::Material { revision, family, tint, roughness, reflectance } => {
                self.expect(revision)?;
                if !material_studio::families().iter().any(|(id,_)| *id == family) { return Err("Unknown terrain family; describe materials for IDs".into()); }
                for v in tint { bounded(v, 0.0, 2.0, "tint")?; }
                if let Some(v) = roughness { bounded(v, 0.08, 1.0, "roughness")?; }
                if let Some(v) = reflectance { bounded(v, 0.0, 1.0, "reflectance")?; }
                let before = Snapshot::current();
                let mut palette = material_studio::snapshot();
                palette.tints[family][..3].copy_from_slice(&tint);
                if let Some(v) = roughness { palette.tints[family][3] = v; }
                if let Some(v) = reflectance { palette.properties[family][0] = v; }
                material_studio::set_preview_palette(Some(palette));
                self.changed(before);
                Ok(json!({"applied":"live","scope":"session","family":family,"tint":palette.tints[family],"properties":palette.properties[family]}))
            }
            Request::Model { revision, asset, text } => {
                self.expect(revision)?;
                let before = Snapshot::current();
                let voxels = voxel_asset::preview(&asset, &text)?;
                self.changed(before);
                Ok(json!({"applied":"live","scope":"session","asset":asset,"voxels":voxels}))
            }
            Request::Undo { revision } => {
                self.expect(revision)?;
                self.undo.pop().ok_or("No preview to undo")?.restore();
                self.revision += 1;
                Ok(json!({"applied":"live","remaining_undo":self.undo.len(),"tuning":world::ambience::tuning()}))
            }
            Request::Reset { revision } => {
                self.expect(revision)?;
                let before = Snapshot::current();
                world::ambience::tune(Default::default())?;
                material_studio::set_preview_palette(None);
                voxel_asset::restore_previews(Default::default());
                self.changed(before);
                Ok(json!({"applied":"live","scope":"session","tuning":world::ambience::tuning()}))
            }
            Request::Vitals { mana, health, hunger } => {
                if w.adventure.is_dead() { return Err("Respawn before setting vitals".into()); }
                if mana.is_none() && health.is_none() && hunger.is_none() { return Err("Choose mana, health or hunger fractions".into()); }
                for (name, value) in [("mana",mana),("health",health),("hunger",hunger)] {
                    if let Some(v) = value { bounded(v, if name == "health" {0.01} else {0.0}, 1.0, name)?; }
                }
                if let Some(v) = mana { w.adventure.mana = w.adventure.max_mana() * v; }
                if let Some(v) = health { w.adventure.health = w.adventure.max_health() * v; }
                if let Some(v) = hunger { w.adventure.hunger = v * 100.0; }
                inspect(w, player, clock, &["player".into()])
            }
            Request::Camera { position_m, yaw, pitch } => {
                if !w.rules.mode.flight() { return Err("Camera placement requires creative or spectator mode".into()); }
                if position_m.iter().any(|v| !v.is_finite() || v.abs() > 10_000_000.0) { return Err("Camera position must be finite and within 10000000 m".into()); }
                if let Some(v) = yaw { bounded(v, -std::f32::consts::TAU, std::f32::consts::TAU, "yaw radians")?; }
                if let Some(v) = pitch { bounded(v, -1.55, 1.55, "pitch radians")?; }
                player.precise_pos = glam::DVec3::from_array(position_m) / f64::from(w.voxel_scale.meters());
                player.pos = player.precise_pos.as_vec3();
                player.vel = glam::Vec3::ZERO;
                player.anchor = None;
                player.gravity_frame = glam::Quat::IDENTITY;
                if let Some(v) = yaw { player.yaw = v; }
                if let Some(v) = pitch { player.pitch = v; }
                world::ambience::refresh(w, player);
                inspect(w, player, clock, &["player".into(),"location".into()])
            }
            Request::Time { fraction } => {
                bounded(fraction, 0.0, 1.0, "day fraction")?;
                clock.set_time_of_day(fraction);
                Ok(json!({"time_of_day":clock.time_of_day()}))
            }
            Request::Systems { set } => {
                let mut rules = w.rules;
                for (id, on) in &set {
                    if !rules.systems.set(id, *on) { return Err(format!("Systems: unknown system {id:?}; see op systems")); }
                }
                if !set.is_empty() { w.set_rules(rules); }
                let rows: Vec<Value> = world::systems::catalog().iter().map(|d| json!({"id":d.id,"label":d.label,
                    "on":w.rules.systems.enabled(&d.id),"requires":d.requires,"tabs":d.tabs,"parity":d.parity})).collect();
                Ok(json!({"systems":rows,"source":"assets/systems.ron","sidecar":world::systems::FILE,
                    "note":"Live switch for prototyping; players choose systems once, on the new-world screen"}))
            }
            Request::Command { line } => {
                if !line.starts_with('/') || line.len() > 200 { return Err("Command: a chat command starting with / and at most 200 characters".into()); }
                let (action, replies) = crate::local_commands::command(&line, w, player, clock);
                Ok(json!({"replies":replies,"needs_ui":!matches!(action, crate::egui_ui::UiAction::None),
                    "scope":"Solo grammar; needs_ui marks a command whose effect is applied by the game window, not here"}))
            }
            Request::Fill { min, max, block } => {
                let (lo, hi) = (glam::IVec3::from_array(min), glam::IVec3::from_array(max));
                if lo.cmpgt(hi).any() { return Err("Fill: min must not exceed max on any axis".into()); }
                let size = (hi - lo + glam::IVec3::ONE).as_i64vec3();
                let cells = size.x * size.y * size.z;
                if cells > 4096 { return Err(format!("Fill: {cells} cells; at most 4096 per request")); }
                let mut edits = Vec::with_capacity(cells as usize);
                for y in lo.y..=hi.y { for z in lo.z..=hi.z { for x in lo.x..=hi.x {
                    edits.push((glam::IVec3::new(x, y, z), block));
                } } }
                w.set_many(&edits);
                let placed = edits.iter().filter(|(p, _)| w.get(*p) == block).count();
                Ok(json!({"cells":cells,"now_block":placed,"block":format!("{block:?}"),
                    "scope":"Voxel cells; cells in unloaded chunks stay unchanged and are not counted in now_block"}))
            }
            Request::Probe { position } => {
                let p = glam::IVec3::from_array(position);
                let scale = w.voxel_scale.meters();
                Ok(json!({"position":position,"block":format!("{:?}", w.get(p)),
                    "centre_m":((p.as_vec3() + glam::Vec3::splat(0.5)) * scale).to_array()}))
            }
            Request::Hedrons { radius_m } => {
                bounded(radius_m, 16.0, 2048.0, "hedron radius metres")?;
                let at = player.pos * w.voxel_scale.meters();
                let found: Vec<_> = world::adventure::hedrons::report(w, at, radius_m).into_iter().map(|h| json!({
                    "element":h.element,"centre_m":h.centre_m,"distance_m":h.distance_m,"captured":h.captured,
                    "pylon_block":h.pylon_block,"pylon_cells":h.pylon_cells,"mana_per_second":h.mana_per_second})).collect();
                Ok(json!({"hedrons":found,"supply_here":world::adventure::hedrons::supply(w, at, None),
                    "scope":"pylon_cells are the voxel cells a capture frame needs; fill them with pylon_block"}))
            }
            Request::ManaFlower { position, feed, pulses } => {
                use world::farming::{flowers, table};
                if !(1..=600).contains(&pulses) { return Err("pulses: expected 1..600".into()); }
                if feed > 4096 { return Err("feed: at most 4096 items".into()); }
                let (index, crop) = table::by_seed(world::BlockId::ManaFlower).ok_or("crops.ron has no mana flower row")?;
                let Some(table::Aura::Mana { feed: Some(food), .. }) = crop.aura else { return Err("The mana flower row has no feed".into()) };
                let p = glam::IVec3::from_array(position);
                if !w.chunks.contains_key(&world::chunk_coord_from_world_block(p)) { return Err("Position is not in a loaded chunk".into()); }
                if w.farming.plots.len() >= world::farming::MAX_PLOTS { return Err("Plot limit reached".into()); }
                w.set(p, crop.crop);
                let mut plot = world::farming::Plot::new(index, w.farming.clock);
                plot.stage = crop.mature();
                w.farming.plots.insert((p.x, p.y, p.z), plot);
                let scale = w.voxel_scale.meters();
                let centre = (p.as_vec3() + glam::Vec3::splat(0.5)) * scale;
                if feed > 0 {
                    w.inventory.dropped.push(world::inventory::DroppedStack { pos: centre, stack: world::inventory::ItemStack::new(food.item, feed) });
                }
                let loose = |w: &world::World| w.inventory.dropped.iter().filter(|d| d.stack.item == food.item && d.pos.distance(centre) <= food.reach_metres).map(|d| d.stack.count).sum::<u32>();
                let mana_before = w.adventure.mana;
                let feet = player.pos * scale;
                let (mut eaten, mut stored, mut to_player) = (0u32, 0u32, 0.0f32);
                for _ in 0..pulses {
                    let r = flowers::pulse(w, feet, true);
                    eaten += r.eaten;
                    stored += r.stored;
                    to_player += r.to_player;
                }
                Ok(json!({"item":food.item.name(),"mana_per_item":food.mana_per_item,"pulses":pulses,"seconds":pulses * 2,
                    "eaten":eaten,"left_loose":loose(w),"stored_in_receivers":stored,"to_player":to_player,
                    "player_mana":[mana_before, w.adventure.mana],"player_distance_m":feet.distance(centre),
                    "scope":"Runs the live flower pass; every mature flower in the world takes part, not only this one"}))
            }
            Request::Enemies { radius_m } => {
                bounded(radius_m, 1.0, 2048.0, "enemy radius metres")?;
                let at = player.pos * w.voxel_scale.meters();
                let mut near: Vec<_> = w.adventure.enemies.iter().filter(|e| e.pos.distance(at) <= radius_m).collect();
                near.sort_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)));
                Ok(json!({"count":near.len(),"enemies":near.iter().take(64).map(|e| enemy_report(e, at)).collect::<Vec<_>>()}))
            }
            Request::SpawnEnemy { kind, position_m, count, affixes, elements, health } => {
                use world::adventure::status::{Affix as A, Element as E};
                if !(1..=32).contains(&count) { return Err("count: expected 1..32".into()); }
                let (id, label) = world::adventure::species_named(&kind).ok_or("Unknown creature")?;
                let all_affixes = [A::Fiery, A::Frostbound, A::Static, A::Toxic, A::Volatile, A::Armored, A::Regenerating, A::Swift, A::Warded];
                let affixes = affixes.iter().map(|n| named(&all_affixes, n, "affix")).collect::<Result<Vec<_>, _>>()?;
                let elements = elements.iter().map(|n| named(&[E::Fire, E::Frost, E::Shock, E::Toxic, E::Wet], n, "element")).collect::<Result<Vec<_>, _>>()?;
                let at = player.pos * w.voxel_scale.meters();
                let ahead = (player.forward_dir() * glam::Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
                let base = position_m.map(glam::Vec3::from_array).unwrap_or(at + ahead * 3.0);
                let side = ahead.cross(glam::Vec3::Y);
                let mut spawned = Vec::new();
                for i in 0..count {
                    // Fan out sideways so a group does not overlap.
                    let offset = side * (((i + 1) / 2) as f32 * 1.5 * if i % 2 == 0 { 1.0 } else { -1.0 });
                    if world::adventure::summon(w, id, base + offset).is_err() { continue; }
                    let Some(e) = w.adventure.enemies.last_mut() else { continue };
                    for a in &affixes { e.affixes = e.affixes.with(*a); }
                    if let Some(h) = health { e.health = h.max(0.01); }
                    for el in &elements { let _ = e.status.apply(*el, e.affixes); }
                    spawned.push(enemy_report(e, at));
                }
                Ok(json!({"label":label,"spawned":spawned.len(),"enemies":spawned,
                    "scope":"Blocked spots are skipped; elements are applied as one hit each"}))
            }
            Request::ClearEnemies { radius_m } => {
                let at = player.pos * w.voxel_scale.meters();
                let before = w.adventure.enemies.len();
                w.adventure.enemies.retain(|e| radius_m.is_some_and(|r| e.pos.distance(at) > r));
                Ok(json!({"removed":before - w.adventure.enemies.len(),"left":w.adventure.enemies.len()}))
            }
            Request::Capture { name } => {
                if name.is_empty() || name.len() > 48 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err("Capture name: 1..48 letters, digits, hyphens or underscores".into()); }
                let mut pending = CAPTURE.lock().unwrap();
                if pending.is_some() { return Err("A capture is already queued".into()); }
                let directory = world::storage::save_directory().join("captures");
                std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                let path = directory.join(format!("{name}.png"));
                if path.exists() { return Err("Capture exists; choose a new name".into()); }
                *pending = Some(path.clone());
                Ok(json!({"capture":path,"status":"queued"}))
            }
        }
    }
}

fn bounded(value: f32, lo: f32, hi: f32, name: &str) -> Result<(), String> {
    if value.is_finite() && (lo..=hi).contains(&value) { Ok(()) } else { Err(format!("{name}: expected {lo}..{hi}")) }
}

fn describe(system: Option<&str>) -> Result<Value, String> {
    let controls = [
        ("natural_density_max","Mana density ceiling",1.05,1.95,"x",0.05),
        ("mana_recovery_max","Mana regeneration cap",1.0,12.0,"x",0.5),
        ("health_recovery_max","Health regeneration cap",1.0,6.0,"x",0.25),
        ("particles","Particle density",0.0,2.0,"x",0.1),
        ("mist","Local mist",0.0,2.0,"x",0.1),
        ("colour_grade","Local colour",0.0,2.0,"x",0.1),
        ("transition_speed","Transition speed",0.1,6.0,"1/s",0.1),
    ].map(|(key,label,min,max,unit,step)| json!({"key":key,"label":label,"type":"number","min":min,"max":max,"unit":unit,"step":step}));
    let all = json!({
        "ambience":{"mode":"live","scope":"session","source":"crates/app/src/world/ambience.rs","controls":controls,
            "switches":[{"key":"sparkles","label":"Sparkles","type":"boolean"}],"values":world::ambience::tuning()},
        "materials":{"mode":"live","scope":"session","source":"crates/app/src/material_studio.rs","families":material_studio::families(),
            "tint_range":[0.0,2.0],"roughness_range":[0.08,1.0],"reflectance_range":[0.0,1.0]},
        "models":{"mode":"live","scope":"session","source":"crates/app/src/mesh/models/voxel_asset.rs",
            "format":"size 8 8 8 text voxels","max_bytes":16384,"loaded_assets":voxel_asset::loaded_assets(),
            "limits":"Factory voxel assets loaded in the scene; rigged GLB, collision and new block IDs require rebuild"},
        "engine":{"protocol":1,"build_version":env!("CARGO_PKG_VERSION"),"undo_limit":16,
            "sections":["world","player","location","look","machine","tuning","models","objectives"],
            "operations":["describe","inspect","survey","tune","material","model","undo","reset","vitals","camera","time","capture","spells","wand","perf",
                "command","systems","fill","probe","hedrons","enemies","spawn_enemy","clear_enemies","machine_kinds","machine","machines","place_machine","place_shell","set_machine","step_factory","machine_test"],
            "machines":{"machine_test":"position, kind? (places), facing?, set {mode,target,enabled,recipe,filter (-1 clears),energy,upgrade,water,input {resource:count},clear_output}, ticks, probe? -> end state + changed fields",
                "max_ticks":machine::MAX_STEPS,"step_s":crate::simulation::STEP_SECONDS,"report":"defaults omitted; off=disabled",
                "resources":engine_content::factory_resources::NAMES},
            "wand":{"build":{"tier":"1..3","mode":"0 ordered,1 twin,2 trigger,3 rapid,4/5 grimoire","rank":"0..20","seed":"found-wand seed, 0 crafted","slots":"spell ids, 8 = empty","rare":"bitmask, omitted = all found","health":"0.01..1","foe_visible":"bool"},
                "equip":"mints the build into an empty hotbar slot of the open world","world_required":"equip only"},
            "survey":{"radius_m":[4,2048],"samples":9,"scope":"generated terrain; local flora and structures require visiting"},
            "rebuild_required":["new block IDs","new native behavior","embedded catalogs","rigged model topology"],
            "docs":"tools/axiom-dev/ENGINE_WORKBENCH.md"}
    });
    match system { Some(name) => all.get(name).cloned().ok_or_else(|| "System must be ambience, materials, models or engine".into()), None => Ok(all) }
}

fn inspect(w: &mut World, player: &Player, clock: &DayNightCycle, sections: &[String]) -> Result<Value, String> {
    let defaults = ["player".into(),"location".into()];
    let sections = if sections.is_empty() { &defaults[..] } else { sections };
    if sections.len() > 8 || sections.iter().any(|s| !["world","player","location","look","machine","tuning","models","objectives"].contains(&s.as_str())) {
        return Err("Choose up to six sections from describe engine".into());
    }
    world::ambience::refresh(w, player);
    let a = &w.adventure;
    let scale = w.voxel_scale.meters();
    // Fixed-camera review scenes set the render pose directly, like precise_eye.
    let feet = if player.precise_pos.as_vec3() == player.pos { player.precise_pos } else { player.pos.as_dvec3() };
    let mut out = serde_json::Map::new();
    for section in sections {
        let value = match section.as_str() {
            "world" => json!({"seed":w.seed,"generator":w.generator_version,"kind":format!("{:?}",w.world_type),"voxel_m":scale,
                "mode":w.rules.mode.label(),"time_of_day":clock.time_of_day(),"day":clock.day_index(),"resident_chunks":w.chunks.len(),
                "machines":w.factory.nodes.len(),"enemies":a.enemies.len(),"projectiles":a.projectiles.len()}),
            "player" => json!({"feet_m":(feet * f64::from(scale)).to_array(),"yaw":player.yaw,"pitch":player.pitch,
                "grounded":player.on_ground,"flying":player.flying,"underwater":player.underwater,
                "health":a.health,"health_max":a.max_health(),"mana":a.mana,"mana_max":a.max_mana(),"hunger":a.hunger}),
            "location" => {
                let density = a.mana_density();
                a.location.reading().map_or(json!({"ready":false,"density":density}), |s| json!({"ready":true,"name":s.place.name,
                    "sample_m":s.position.to_array(),"density":density,"mana_regen_x":world::ambience::mana_recovery(density),
                    "health_regen_x":world::ambience::health_recovery(density),"poison_hp_s":world::adventure::affinity::poison_dps(density),
                    "vitality":s.place.vitality,"sparkles":s.place.sparkles,"sparkles_enabled":world::ambience::tuning().sparkles,
                    "particle_scale":world::ambience::tuning().particles,"motes":s.place.motes,"drift":s.place.drift,
                    "mist":s.place.mist,"sky":s.around.sky,"cave":s.around.underground,"water":s.around.water,
                    "water_motion":s.around.water_motion,"flora":s.around.flora,"shielded":a.location.shielded}))
            }
            "machine" => look_cell(w, player).and_then(|c| world::factory::probe(w, c)).map_or(Value::Null, |p| machine::json(&p)),
            "look" => world::raycast_block(w,player.eye(w.voxel_scale),player.forward_dir(),8.0/scale)
                .map_or(Value::Null, |(p,_)| json!({"cell":p.to_array(),"block":w.get(p).display_name(),"solid":w.get(p).is_solid()})),
            "tuning" => serde_json::to_value(world::ambience::tuning()).unwrap(),
            "models" => json!({"loaded":voxel_asset::loaded_assets(),"overridden":voxel_asset::previews().keys().collect::<Vec<_>>()}),
            "objectives" => {
                let view = w.goals.view();
                json!({"current":view.current.as_ref().map(|r| json!({"title":r.title,"action":r.action})),
                    "step":view.rows.iter().position(|r| !r.done).map_or(view.rows.len(), |i| i + 1),
                    "total":view.rows.len(),"done":view.rows.iter().filter(|r| r.done).count(),
                    "rows":view.rows.iter().map(|r| json!({"title":r.title,"done":r.done})).collect::<Vec<_>>(),
                    "completed":w.goals.completed})
            }
            _ => unreachable!(),
        };
        out.insert(section.clone(),value);
    }
    Ok(Value::Object(out))
}

fn look_cell(w: &World, player: &Player) -> Option<glam::IVec3> {
    let scale = w.voxel_scale.meters();
    world::raycast_block(w, player.eye(w.voxel_scale), player.forward_dir(), 8.0 / scale).map(|(p, _)| p)
}
