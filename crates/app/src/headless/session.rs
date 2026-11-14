use super::{Session, WorldSimulation};
use std::{collections::HashSet, path::Path};

impl Session {
    pub(super) fn load_exact(directory: &Path) -> Result<Self, String> {
        let restored = crate::world::storage::session::load(directory).map_err(|e| format!("{e:#}"))?;
        let mut sim = WorldSimulation::new();
        sim.reset_for_world(restored.saved.world.voxel_scale);
        sim.player_tick = engine_core::time::TickId(restored.player_tick);
        sim.cpu_region = Some(restored.region);
        if let Some(note) = &restored.saved.recovery_notice { eprintln!("[ca-session] {note}"); }
        Ok(Self { world: restored.saved.world, player: restored.saved.player,
            day_night: restored.saved.day_night, sim, input: restored.input,
            ticks: restored.player_tick, dirty_blocks: HashSet::new(), dirty_chunks: HashSet::new() })
    }
}
