//! Mapped terrain, waypoints, the corner minimap and the full-screen world map.
//!
//! Streaming reports chunks through `note_chunk` and `absorb`; the UI calls
//! `pump` each playing frame. State is global because the chunk streamer and the
//! UI have no shared owner below `main.rs`.
pub mod columns;
mod minimap;
mod panel;
mod sidecar;
pub mod tiles;
pub mod waypoints;

use crate::creative::CreativeState;
use crate::mesh::GeneratedChunk;
use engine_core::Budget;
use glam::{IVec2, IVec3, Vec3};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

/// Chunk coordinates waiting for a column scan; older entries drop past this.
const MAX_PENDING: usize = 16_384;
/// Tile sidecar write interval in seconds of UI time while tiles change.
const TILE_SAVE_SECONDS: f64 = 45.0;
const UPLOADS_PER_FRAME: usize = 24;

struct MapState {
    world: Option<Arc<PathBuf>>,
    voxel_m: f32,
    tiles: tiles::TileStore,
    waypoints: waypoints::Waypoints,
    waypoints_unsaved: bool,
    pending: VecDeque<IVec3>,
    pending_set: HashSet<IVec3>,
    player: Option<(Vec3, f32)>,
    prefs: sidecar::Prefs,
    tiles_saved_at: f64,
    writer: Option<std::thread::JoinHandle<()>>,
    view: panel::View,
}

static STATE: Mutex<Option<MapState>> = Mutex::new(None);

fn state() -> MutexGuard<'static, Option<MapState>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn with<R>(f: impl FnOnce(&mut MapState) -> R) -> R {
    let mut guard = state();
    let s = guard.get_or_insert_with(|| MapState {
        world: None,
        voxel_m: 1.0,
        tiles: Default::default(),
        waypoints: Default::default(),
        waypoints_unsaved: false,
        pending: VecDeque::new(),
        pending_set: HashSet::new(),
        player: None,
        prefs: sidecar::read(&prefs_path(), sidecar::decode_prefs).unwrap_or_default(),
        tiles_saved_at: 0.0,
        writer: None,
        view: Default::default(),
    });
    f(s)
}

fn prefs_path() -> PathBuf {
    crate::world::storage::save_directory().join(sidecar::PREFS_FILE)
}

impl MapState {
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

    fn save_tiles(&mut self, background: bool) {
        let Some(root) = self.world.clone().filter(|r| r.is_dir()) else { return };
        if self.writer.as_ref().is_some_and(|w| !w.is_finished()) {
            if !background {
                self.join_writer();
            } else {
                return;
            }
        }
        let tiles = self.tiles.snapshot();
        self.tiles.unsaved = false;
        let path = root.join(sidecar::TILE_FILE);
        let write = move || {
            let bytes = sidecar::encode_tiles(tiles.iter().map(|(k, c)| (*k, c.as_slice())));
            if let Err(e) = sidecar::write_atomic(&path, &bytes) {
                eprintln!("[map] Cannot write {}: {e:#}", path.display());
            }
        };
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

    fn open_world(&mut self, root: &Arc<PathBuf>, voxel_m: f32) {
        // A background write may still be renaming this world's tile file into place.
        self.join_writer();
        if self.waypoints_unsaved {
            self.save_waypoints();
        }
        if self.tiles.unsaved {
            self.save_tiles(false);
        }
        self.world = Some(root.clone());
        self.voxel_m = voxel_m;
        self.tiles = Default::default();
        self.waypoints = sidecar::read(&root.join(sidecar::WAYPOINT_FILE), sidecar::decode_waypoints)
            .unwrap_or_default();
        self.waypoints_unsaved = false;
        for (key, colour) in sidecar::read(&root.join(sidecar::TILE_FILE), sidecar::decode_tiles)
            .unwrap_or_default()
        {
            self.tiles.insert_saved(key, colour);
        }
        self.tiles.unsaved = false;
        self.view = Default::default();
    }
}

/// Streaming hook: a chunk source is about to be meshed.
pub(crate) fn note_chunk(coord: IVec3) {
    with(|s| {
        if s.pending_set.insert(coord) {
            s.pending.push_back(coord);
        }
        while s.pending.len() > MAX_PENDING {
            if let Some(old) = s.pending.pop_front() {
                s.pending_set.remove(&old);
            }
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
        while budget.has_time() {
            let Some(coord) = s.pending.pop_front() else { break };
            s.pending_set.remove(&coord);
            if let Some(chunk) = source(coord) {
                s.tiles.apply(IVec2::new(coord.x, coord.z), coord.y, &columns::scan(chunk));
            }
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
    s.join_writer();
    if s.tiles.unsaved {
        s.save_tiles(false);
    }
    if s.waypoints_unsaved {
        s.save_waypoints();
    }
}

/// Opens or closes the full-screen world map; opening closes other panels.
pub fn toggle_panel(creative: &mut CreativeState) {
    let open = !creative.show_world_map;
    creative.close_panels();
    creative.show_world_map = open;
    if open {
        with(|s| s.view.follow = true);
    }
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

/// Replaces the death marker; for the survival death handler.
pub fn set_death_waypoint(pos: Vec3) {
    with(|s| {
        s.waypoints.set_death(pos.floor().as_ivec3());
        s.waypoints_unsaved = true;
        s.save_waypoints();
    });
}

/// Per-frame UI work: texture uploads, deferred saves and the minimap.
pub(super) fn pump(ctx: &egui::Context, pos: Vec3, yaw: f32, draw_minimap: bool, top: f32) {
    with(|s| {
        s.player = Some((pos, yaw));
        s.tiles.upload(ctx, UPLOADS_PER_FRAME);
        let typing = ctx.memory(|m| m.focused().is_some());
        if s.waypoints_unsaved && !typing {
            s.save_waypoints();
        }
        let now = ctx.input(|i| i.time);
        if s.tiles.unsaved && now - s.tiles_saved_at > TILE_SAVE_SECONDS {
            s.tiles_saved_at = now;
            s.save_tiles(true);
        }
        if draw_minimap && s.prefs.minimap {
            minimap::draw(ctx, s, top);
        }
    });
}

/// Draws the world map while `open`; closing goes through `CreativeState`.
pub(super) fn panel(ctx: &egui::Context, open: &mut bool) {
    with(|s| panel::draw(ctx, s, open));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::PADDED_CHUNK_SIZE;
    use crate::world::BlockId;

    #[test]
    fn map_absorb_maps_noted_chunks_and_resets_on_world_change() {
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
            assert!(s.pending.is_empty() && s.pending_set.is_empty());
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
        flush();
        let saved = sidecar::read(&third.join(sidecar::TILE_FILE), sidecar::decode_tiles);
        std::fs::remove_dir_all(third.as_path()).unwrap();
        assert_eq!(saved.map(|t| t.len()), Some(1), "flushed tiles reach disk");
        with(|s| assert!(!s.tiles.unsaved && s.writer.is_none()));
    }
}
