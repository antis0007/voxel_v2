//! Mapped terrain, waypoints, the corner minimap and the full-screen world map.
//!
//! Streaming supplies explored surface tiles in ordinary worlds. Folded interiors
//! retain local observation charts for their changing gravity and topology.
pub mod columns;
mod knowledge;
mod perception;
mod minimap;
mod panel;
pub(super) mod sidecar;
pub mod tiles;
pub mod waypoints;

use crate::creative::CreativeState;
use crate::mesh::GeneratedChunk;
use engine_core::Budget;
use glam::{IVec2, IVec3, Vec3};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

/// Tile sidecar write interval in seconds of UI time while tiles change.
const TILE_SAVE_SECONDS: f64 = 45.0;
/// Map texture bytes queued per frame: two new regions or 128 changed tiles.
const UPLOAD_BYTES_PER_FRAME: usize = 512 * 1024;
type SavedTiles = Vec<(IVec2, Vec<egui::Color32>)>;

struct MapState {
    pending: std::collections::VecDeque<IVec3>,
    knowledge: knowledge::Knowledge,
    perception_cursor: usize,
    unknown_arrival: bool,
    objective: Option<waypoints::Waypoint>,
    delivery: Option<waypoints::Waypoint>,
    workshop_marker: Option<waypoints::Waypoint>,
    freight: Option<crate::world::farming::planet_freight::Report>,
    delivery_stock: (bool, u32),
    delivery_notice_until: f64,
    config: Option<engine_worldgen::WorldSampleConfig>,
    world: Option<Arc<PathBuf>>,
    voxel_m: f32,
    tiles: tiles::TileStore,
    waypoints: waypoints::Waypoints,
    waypoints_unsaved: bool,
    player: Option<(Vec3, f32)>,
    prefs: sidecar::Prefs,
    tiles_saved_at: f64,
    writer: Option<std::thread::JoinHandle<()>>,
    /// Background read of the open world's tile sidecar.
    loading: Option<std::thread::JoinHandle<SavedTiles>>,
    view: panel::View,
}

static STATE: Mutex<Option<MapState>> = Mutex::new(None);

fn state() -> MutexGuard<'static, Option<MapState>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn with<R>(f: impl FnOnce(&mut MapState) -> R) -> R {
    let mut guard = state();
    let s = guard.get_or_insert_with(|| MapState {
        pending: Default::default(),
        knowledge: Default::default(),
        perception_cursor: 0,
        unknown_arrival: false,
        objective: None,
        delivery: None,
        workshop_marker: None,
        freight: None,
        delivery_stock: (false, 0),
        delivery_notice_until: 0.0,
        config: None,
        world: None,
        voxel_m: 1.0,
        tiles: Default::default(),
        waypoints: Default::default(),
        waypoints_unsaved: false,
        player: None,
        prefs: sidecar::read(&prefs_path(), sidecar::decode_prefs).unwrap_or_default(),
        tiles_saved_at: 0.0,
        writer: None,
        loading: None,
        view: Default::default(),
    });
    f(s)
}

fn prefs_path() -> PathBuf {
    crate::world::storage::save_directory().join(sidecar::PREFS_FILE)
}

impl MapState {
    fn pin(&mut self, mut waypoint: waypoints::Waypoint) {
        waypoint.name = waypoints::clip_name(&waypoint.name);
        if self.waypoints.list.iter().any(|w| w.pos == waypoint.pos && w.name == waypoint.name) {
            return;
        }
        if let Some(i) = self.waypoints.add(waypoint.pos) {
            self.waypoints.list[i] = waypoint;
            self.waypoints_unsaved = true;
        }
    }

    fn player_cell(&self) -> Option<IVec3> {
        self.player.map(|(p, _)| p.floor().as_ivec3())
    }

    /// Writes only into a world directory a save has already created.
    fn save_waypoints(&mut self) {
        let Some(root) = self.world.clone().filter(|r| r.is_dir()) else { return };
        let path = root.join(sidecar::WAYPOINT_FILE);
        // A failed write is reported once; the next waypoint change retries it.
        if let Err(e) = sidecar::write_atomic(&path, &sidecar::encode_waypoints(&self.waypoints)) {
            eprintln!("[map] Cannot write {}: {e:#}", path.display());
        }
        self.waypoints_unsaved = false;
    }

    /// Snapshot of unsaved tiles as a write job; `None` without a world directory.
    fn tile_write(&mut self) -> Option<impl FnOnce() + Send + 'static> {
        let root = self.world.clone().filter(|r| r.is_dir())?;
        let tiles = self.tiles.snapshot();
        self.tiles.unsaved = false;
        let path = root.join(sidecar::TILE_FILE);
        Some(move || {
            let bytes = sidecar::encode_tiles(tiles.iter().map(|(k, c)| (*k, c.as_slice())));
            if let Err(e) = sidecar::write_atomic(&path, &bytes) {
                eprintln!("[map] Cannot write {}: {e:#}", path.display());
            }
        })
    }

    fn save_tiles(&mut self, background: bool) {
        // Writing before the saved tiles are restored would drop them from disk.
        self.finish_load(!background);
        if self.loading.is_some() {
            return;
        }
        if self.writer.as_ref().is_some_and(|w| !w.is_finished()) {
            if !background {
                self.join_writer();
            } else {
                return;
            }
        }
        let Some(write) = self.tile_write() else { return };
        if background {
            self.writer = std::thread::Builder::new().name("map-save".into()).spawn(write).ok();
        } else {
            write();
        }
    }

    fn join_writer(&mut self) {
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }

    /// Merges the background tile read once done, or now when `wait`.
    fn finish_load(&mut self, wait: bool) {
        if !wait && self.loading.as_ref().is_some_and(|l| !l.is_finished()) {
            return;
        }
        let Some(load) = self.loading.take() else { return };
        for (key, colour) in load.join().unwrap_or_default() {
            self.tiles.merge_saved(key, colour);
        }
    }

    /// Saves the previous world's tiles and reads the new world's tiles on a
    /// worker; live chunks mapped meanwhile keep their columns.
    fn open_world(&mut self, root: &Arc<PathBuf>, voxel_m: f32) {
        self.workshop_marker = None;
        self.unknown_arrival = self.world.is_some();
        self.knowledge.open(root, false);
        self.perception_cursor = 0;
        // The previous world's saved tiles must be restored before they are written back.
        self.finish_load(true);
        if self.waypoints_unsaved {
            self.save_waypoints();
        }
        let write = if self.tiles.unsaved { self.tile_write() } else { None };
        // A previous write may still be renaming a tile file into place; the
        // worker finishes it before writing or reading.
        let previous = self.writer.take();
        let path = root.join(sidecar::TILE_FILE);
        let load = move || {
            if let Some(w) = previous {
                let _ = w.join();
            }
            if let Some(write) = write {
                write();
            }
            sidecar::read(&path, sidecar::decode_tiles).unwrap_or_default()
        };
        self.loading = std::thread::Builder::new().name("map-load".into()).spawn(load)
            .inspect_err(|e| eprintln!("[map] Cannot start the tile load: {e}"))
            .ok();
        self.world = Some(root.clone());
        self.delivery = None;
        self.freight = None;
        self.delivery_stock = (false, 0);
        self.delivery_notice_until = 0.0;
        self.voxel_m = voxel_m;
        self.tiles = Default::default();
        self.waypoints = sidecar::read(&root.join(sidecar::WAYPOINT_FILE), sidecar::decode_waypoints)
            .unwrap_or_default();
        self.waypoints_unsaved = false;
        self.view = Default::default();
    }

    fn upload_tiles(&mut self, ctx: &egui::Context) {
        let near = self.player_cell().map_or(IVec2::ZERO, |cell| IVec2::new(
            engine_core::floor_div(cell.x, crate::world::CHUNK_SIZE),
            engine_core::floor_div(cell.z, crate::world::CHUNK_SIZE),
        ));
        self.tiles.upload(ctx, UPLOAD_BYTES_PER_FRAME, near);
    }
}

/// Streaming hook: a chunk source is about to be meshed.
pub(crate) fn note_chunk(coord: IVec3) {
    with(|s| {
        if !s.pending.contains(&coord) {
            if s.pending.len()>=4096 {s.pending.pop_front();}
            s.pending.push_back(coord);
        }
    });
}

/// Streaming hook: scan noted chunks still resident in `source`, within `budget`.
pub(crate) fn absorb<'a>(
    world: &Arc<PathBuf>,
    voxel_m: f32,
    source: impl Fn(IVec3) -> Option<&'a GeneratedChunk>,
    budget: Budget,
) {
    with(|s| {
        if !s.world.as_ref().is_some_and(|w| Arc::ptr_eq(w, world) || w == world) {
            s.open_world(world, voxel_m);
        }
        s.finish_load(false);
        let count=s.pending.len().min(8);
        for _ in 0..count {
            if !budget.has_time() {break;}
            let Some(coord)=s.pending.pop_front() else {break;};
            if let Some(chunk)=source(coord) {
                s.tiles.apply(IVec2::new(coord.x,coord.z),coord.y,&columns::scan(chunk));
            } else {s.pending.push_back(coord);}
        }
        if let Some(cell) = s.player_cell() {
            let tile = IVec2::new(
                engine_core::floor_div(cell.x, crate::world::CHUNK_SIZE),
                engine_core::floor_div(cell.z, crate::world::CHUNK_SIZE),
            );
            s.tiles.evict_far(tile, tiles::MAX_TILES);
        }
    });
}

/// Waits for the background tile write, then writes unsaved tiles and waypoints now.
pub fn flush() {
    let mut guard = state();
    let Some(s) = guard.as_mut() else { return };
    s.knowledge.flush();
    s.finish_load(true);
    s.join_writer();
    if s.tiles.unsaved {
        s.save_tiles(false);
    }
    if s.waypoints_unsaved {
        s.save_waypoints();
    }
}

/// Opens or closes the full-screen world map; opening closes other panels.
pub fn toggle_panel(creative: &mut CreativeState, config: Option<engine_worldgen::WorldSampleConfig>) {
    let open = !creative.show_world_map;
    creative.close_panels();
    creative.show_world_map = open;
    if open {
        with(|s| {
            s.view.follow = true;
            s.config = config;
            s.workshop_marker = None;
        });
    }
}

/// Saved waypoints for the other map views: voxel position, label, colour.
pub(crate) fn waypoint_marks() -> Vec<(IVec3, String, egui::Color32)> {
    with(|s| s.waypoints.list.iter()
        .map(|w| (w.pos, w.name.clone(), waypoints::PALETTE[usize::from(w.colour) % waypoints::PALETTE.len()]))
        .collect())
}

/// Shows or hides the corner minimap and stores the choice.
pub fn toggle_minimap() {
    with(|s| {
        s.prefs.minimap = !s.prefs.minimap;
        if let Err(e) = sidecar::write_atomic(&prefs_path(), &sidecar::encode_prefs(s.prefs)) {
            eprintln!("[map] Cannot write minimap preference: {e:#}");
        }
    });
}

pub fn minimap_enabled() -> bool {
    with(|s| s.prefs.minimap)
}

/// Adds a waypoint at the last drawn player cell; `None` before play or when full.
pub fn add_waypoint_at_player() -> Option<usize> {
    with(|s| {
        let cell = s.player_cell()?;
        let index = s.waypoints.add(cell)?;
        s.waypoints_unsaved = true;
        s.save_waypoints();
        Some(index)
    })
}

pub(super) fn outpost_name(cell: IVec3) -> Option<String> {
    with(|s| s.waypoints.list.iter().find(|w| w.pos == cell && w.kind == waypoints::Kind::Marker)
        .map(|w| w.name.clone()))
}

pub(super) fn pin_outpost(cell: IVec3, name: String) {
    with(|s| s.pin(waypoints::Waypoint { name, pos: cell, colour: 0, kind: waypoints::Kind::Marker }));
}

pub(super) fn pin_lead(root: &Arc<PathBuf>, cfg: engine_worldgen::WorldSampleConfig, pos: Vec3, name: &str) {
    with(|s| {
        if s.world.as_ref() != Some(root) {
            s.open_world(root, cfg.voxel_meters());
        }
        s.pin(waypoints::Waypoint {
            name: name.into(),
            pos: (pos / cfg.voxel_meters()).floor().as_ivec3(),
            colour: 0,
            kind: waypoints::Kind::Marker,
        });
        if s.waypoints_unsaved {
            s.save_waypoints();
        }
    });
}

/// Replaces the death marker; for the survival death handler.
pub fn set_death_waypoint(pos: Vec3) {
    with(|s| {
        s.waypoints.set_death(pos.floor().as_ivec3());
        s.waypoints_unsaved = true;
        s.save_waypoints();
    });
}

/// Per-frame UI work: texture uploads, deferred saves and the minimap.
pub(super) fn pump(ctx: &egui::Context, pos: Vec3, yaw: f32, draw_minimap: bool, top: f32,
    objective: Option<&crate::world::adventure::QuestMarker>,
    receiver: &crate::world::orbital::receiver::View, voxel_m: f32) {
    with(|s| {
        s.knowledge.maintenance();
        s.voxel_m = voxel_m;
        let now = ctx.input(|i| i.time);
        let stock = (receiver.cargo_ready, receiver.delivered_iron);
        let ready = receiver.position.is_some() && (stock.0 || stock.1 > 0);
        if ready && ((!s.delivery_stock.0 && stock.0) || stock.1 > s.delivery_stock.1) {
            s.delivery_notice_until = now + 6.0;
        }
        s.delivery_stock = stock;
        s.delivery = receiver.position.filter(|_| ready).map(|pos| waypoints::Waypoint {
            name: "Receiver · Cargo Ready".into(), pos: (pos / voxel_m).floor().as_ivec3(),
            colour: 0, kind: waypoints::Kind::Marker,
        });
        if ready && now < s.delivery_notice_until && draw_minimap {
            let slot = super::hud_layout::place(ctx, "receiver_notice");
            let shown = egui::Area::new(egui::Id::new("receiver_delivery_notice"))
                .anchor(slot.align, slot.offset)
                .interactable(false).show(ctx, |ui| {
                    super::presentation::hud_frame(ctx).show(ui, |ui| {
                        ui.label("Receiver · Cargo Ready");
                    });
                });
            super::hud_layout::claim(ctx, "receiver_notice", shown.response.rect);
        }
        if let Some(q) = objective {
            let pos = (q.pos / voxel_m).floor().as_ivec3();
            if let Some(current) = s.objective.as_mut() {
                current.pos = pos;
                if current.name != q.label { current.name.clear(); current.name.push_str(q.label); }
            } else {
                s.objective = Some(waypoints::Waypoint {
                    name: q.label.into(), pos, colour: 0, kind: waypoints::Kind::Marker,
                });
            }
        } else { s.objective = None; }
        s.player = Some((pos, yaw));
        s.finish_load(false);
        // Tiles feed only the minimap and the world map; hidden, they wait.
        let minimap = draw_minimap && s.prefs.minimap;
        if minimap {
            s.upload_tiles(ctx);
        }
        let typing = ctx.memory(|m| m.focused().is_some());
        if s.waypoints_unsaved && !typing {
            s.save_waypoints();
        }
        if s.tiles.unsaved && now - s.tiles_saved_at > TILE_SAVE_SECONDS {
            s.tiles_saved_at = now;
            s.save_tiles(true);
        }
        if minimap {
            minimap::draw(ctx, s, top);
        }
    });
}

/// Record a text page explicitly inspected in the current world.
pub(super) fn inspect_record(world: &crate::world::World, cell: IVec3, text: &[String]) {
    with(|s| {
        if s.world.as_ref() == Some(&world.regions.view.root) { s.knowledge.inspect_record(cell, text); }
    });
}

/// Called with the actual render light set. No actor knowledge is replicated.
pub(crate) fn perceive(world: &crate::world::World, player: &crate::player::Player,
    cache: &crate::render::ChunkCache, lights: &[crate::render::TorchLight],
    held: Option<crate::render::TorchLight>, daylight: bool, fov: f32, aspect: f32) {
    with(|s| {
        if s.world.as_ref() != Some(&world.regions.view.root) {
            s.open_world(&world.regions.view.root, world.voxel_scale.meters());
        }
        if s.knowledge.creative != world.rules.mode.creative_tools() {
            s.knowledge.open(&world.regions.view.root, world.rules.mode.creative_tools());
            s.view = Default::default();
        }
        // Ordinary worlds use the explored surface map. Folded interiors keep
        // the local chart because their gravity cannot project onto world X/Z.
        s.knowledge.legacy=world.world_type!=crate::world::WorldType::Manyfold;
        s.freight = crate::world::farming::planet_freight::report(world, player.pos * s.voxel_m);
        if s.unknown_arrival {
            s.knowledge.unknown_arrival();
            s.unknown_arrival = false;
        }
        perception::capture(&mut s.knowledge, perception::View { world, player, cache, lights, held, daylight, fov, aspect }, &mut s.perception_cursor);
    });
}

/// Open a surveyed destination as a normal saved waypoint.
pub fn show_destination(creative: &mut CreativeState, cfg: engine_worldgen::WorldSampleConfig,
    pos_m: Vec3, name: &str) {
    creative.close_panels();
    creative.show_world_map = true;
    with(|s| {
        let pos = (pos_m / cfg.voxel_meters()).floor().as_ivec3();
        s.config = Some(cfg);
        s.pin(waypoints::Waypoint { pos, name: waypoints::clip_name(name), colour: 4, kind: waypoints::Kind::Marker });
        if s.knowledge.legacy { s.view.focus(pos); }
        else { s.view.focus_chart(s.knowledge.project(pos.as_vec3() * s.voxel_m, false)); }
    });
}

pub(crate) fn show_workshop(
    creative: &mut CreativeState,
    cfg: engine_worldgen::WorldSampleConfig,
    target: Option<(IVec3, &'static str)>,
) {
    creative.close_panels();
    creative.show_world_map = true;
    with(|s| {
        s.config = Some(cfg);
        s.workshop_marker = target.map(|(pos, name)| {
            if s.knowledge.legacy { s.view.focus(pos); }
            else { s.view.focus_chart(s.knowledge.project(pos.as_vec3() * cfg.voxel_meters(), false)); }
            waypoints::Waypoint { pos, name: name.into(), colour: 4, kind: waypoints::Kind::Marker }
        });
        if target.is_none() { s.view.follow = true; }
    });
}

/// Draws the world map while `open`; closing goes through `CreativeState`.
pub(super) fn panel(ctx: &egui::Context, open: &mut bool, workshop: bool) -> Option<u32> {
    if !*open { return None; }
    with(|s| {
        s.upload_tiles(ctx);
        panel::draw(ctx, s, open, workshop)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::PADDED_CHUNK_SIZE;
    use crate::world::BlockId;

    #[test]
    fn map_absorb_maps_resident_columns_and_preserves_saved_tiles() {
        let p = PADDED_CHUNK_SIZE as usize;
        let mut blocks = vec![BlockId::Air; p * p * p];
        blocks[..p * p * 4].fill(BlockId::Stone);
        let chunk = GeneratedChunk::new(IVec3::new(-3, 0, 5), blocks);
        // Directories that do not exist: nothing is read or written.
        let first = Arc::new(std::env::temp_dir().join("voxel-map-test-missing-a"));
        let second = Arc::new(std::env::temp_dir().join("voxel-map-test-missing-b"));
        note_chunk(IVec3::new(-3, 0, 5));
        note_chunk(IVec3::new(9, 0, 9));
        absorb(&first, 1.0, |c| (c == chunk.chunk).then_some(&chunk), Budget::UNBOUNDED);
        with(|s| {
            assert!(s.tiles.is_mapped(IVec2::new(-3, 5)));
            assert_eq!(s.tiles.len(), 1);
        });
        absorb(&second, 0.5, |_| None, Budget::UNBOUNDED);
        with(|s| {
            assert_eq!(s.tiles.len(), 0);
            assert_eq!(s.voxel_m, 0.5);
        });
        // A world that exists on disk: `flush` writes its tiles without waiting for the timer.
        let third = Arc::new(std::env::temp_dir().join(format!("voxel-map-test-flush-{}", std::process::id())));
        std::fs::create_dir_all(third.as_path()).unwrap();
        note_chunk(IVec3::new(-3, 0, 5));
        absorb(&third, 1.0, |c| (c == chunk.chunk).then_some(&chunk), Budget::UNBOUNDED);
        with(|s| {
            s.tiles.insert_saved(IVec2::new(-3, 5), vec![egui::Color32::GRAY; columns::COLUMNS]);
            s.tiles.unsaved = true;
        });
        flush();
        let saved = sidecar::read(&third.join(sidecar::TILE_FILE), sidecar::decode_tiles);
        std::fs::remove_dir_all(third.as_path()).unwrap();
        assert_eq!(saved.map(|t| t.len()), Some(1), "flushed tiles reach disk");
        with(|s| assert!(!s.tiles.unsaved && s.writer.is_none()));
    }
}
