//! The bounded first expedition uses existing encounter and inventory save owners.
use super::{World, WorldSampleConfig, adventure::{Adventure, QuestMarker}, sites};
use glam::{IVec3, Vec3};

// Bit 14 is site-specific: foundry bypass on foundries, homecoming on starter 0.
// It is already admitted by save 32. No serialized layout or old generation changes.
const RETURNED: u32 = 1 << 14;
pub fn returned(a: &Adventure) -> bool {
    a.records.get(&IVec3::ZERO).is_some_and(|r| r & RETURNED != 0)
}
pub fn cleared(a: &Adventure) -> bool {
    a.records.get(&IVec3::ZERO).is_some_and(|r| r & 256 != 0)
}
pub fn home(cfg: &WorldSampleConfig) -> Option<Vec3> {
    let heart = sites::workshop_heart(cfg)?;
    Some(Vec3::new(-3.5, heart.y + 19.0, -39.5))
}
pub fn marker(a: &Adventure, cfg: &WorldSampleConfig, origin: Vec3) -> Option<QuestMarker> {
    if cfg.generator_version < 48 || returned(a) { return None; }
    let (pos, label) = if cleared(a) {
        (home(cfg)?, "Return to the workbench")
    } else if a.best_wand_tier() < 2 {
        (home(cfg)?, "Workshop: prepare a wand")
    } else {
        (sites::workshop_heart(cfg)?, "Mine: defeat the guardians")
    };
    let d = pos - origin;
    Some(QuestMarker { pos, label, kind: 0,
        bearing_deg: d.x.atan2(-d.z).to_degrees().rem_euclid(360.0),
        distance_m: Vec3::new(d.x, 0.0, d.z).length(), vertical_m: d.y })
}
pub fn use_bench(world: &mut World, player: Vec3) {
    let cfg = world.sample_config();
    if cfg.generator_version < 48 { return; }
    let Some(home) = home(&cfg) else { return; };
    let feet = player * cfg.voxel_meters();
    if feet.distance(home) > 8.0 { return; }
    if world.adventure.enemies.iter().all(|e| e.pos.distance(feet) > 20.0) {
        world.adventure.respawn_point = Some(feet);
    }
    if cleared(&world.adventure) && world.adventure.best_wand_tier() >= 2 && !returned(&world.adventure) {
        *world.adventure.records.entry(IVec3::ZERO).or_default() |= RETURNED;
        world.expedition.say("First expedition complete. Workshop reclaimed; continue toward the guardian relics.");
        world.adventure.sounds.push(crate::audio::Event {
            cue: crate::audio::Cue::Treasure, pos: Some(home), gain: 0.7, pitch: 1.0,
        });
    } else if !returned(&world.adventure) {
        world.expedition.say(if world.adventure.best_wand_tier() < 2 {
            "Mine the lit side bay: copper, coal, two lumen fibre and crystal. F at the smelter, then upgrade your wand in Gear."
        } else { "Wand ready. Follow the timber decline to the fungal vault, defeat its guardians, then return here." });
    }
}
