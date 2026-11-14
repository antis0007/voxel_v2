//! The bounded first expedition uses existing encounter and inventory save owners.
use super::{World, WorldSampleConfig, adventure::{Adventure, QuestMarker, spell_defs, wands}, inventory::Item, sites};
use glam::{IVec3, Vec3};

// Bit 14 is site-specific: foundry bypass on foundries, homecoming on starter 0.
// It is already admitted by save 32. No serialized layout or old generation changes.
const RETURNED: u32 = 1 << 14;
// Valve bits are foundry-specific. At starter site zero bit 13 records the
// workshop's first iron or fuel delivery, within the saved mask.
const AUTOMATED: u32 = 1 << 13;

#[derive(Clone)]
pub struct Progress {
    pub vault: bool,
    pub reclaimed: bool,
    pub delivery: bool,
}

pub fn progress(world: &World) -> Option<Progress> {
    let cfg = world.sample_config();
    if cfg.generator_version < 48 || home(&cfg).is_none() { return None; }
    Some(Progress {
        vault: cleared(&world.adventure),
        reclaimed: returned(&world.adventure),
        delivery: automated(&world.adventure),
    })
}

pub fn automated(a: &Adventure) -> bool {
    a.records.get(&IVec3::ZERO).is_some_and(|r| r & AUTOMATED != 0)
}

/// Once per authoritative world step, independent of player count. The receipt
/// comes from accepted material at the storage end, never placement or a timer.
pub fn tick(world: &mut World) {
    if !std::mem::take(&mut world.factory.workshop_delivery) || world.generator_version < 48
        || automated(&world.adventure) { return; }
    let Some(home) = home(&world.sample_config()) else { return; };
    *world.adventure.records.entry(IVec3::ZERO).or_default() |= AUTOMATED;
    world.expedition.say(if returned(&world.adventure) { "Workshop Complete" } else { "Supply Delivered" });
    world.adventure.sounds.push(crate::audio::Event {
        cue: crate::audio::Cue::Treasure, pos: Some(home), gain: 0.7, pitch: 1.0,
    });
}
pub fn returned(a: &Adventure) -> bool {
    a.records.get(&IVec3::ZERO).is_some_and(|r| r & RETURNED != 0)
}
pub fn cleared(a: &Adventure) -> bool {
    a.records.get(&IVec3::ZERO).is_some_and(|r| r & 256 != 0)
}
#[derive(Clone, Copy)]
pub enum Gear {
    Held(Item),
    Hotbar(Item, usize),
    Pack(Item),
    EmptyDeck(Item),
    Missing,
}

impl Gear {
    pub fn prepared(self) -> bool {
        matches!(self, Self::Held(_) | Self::Hotbar(..) | Self::Pack(_))
    }
}

/// Preparation belongs to the current player's loadout. An old wand record
/// can outlive its item, and a wand with only utility cards is no combat kit.
pub fn gear(world: &World, hand_slot: usize) -> Gear {
    let a = &world.adventure;
    let mut empty_deck = None;
    let mut hotbar = None;
    for (slot, stack) in world.inventory.hotbar.iter().chain(&world.inventory.pack).enumerate() {
        let Some(item) = stack.map(|stack| stack.item) else { continue };
        let usable = match item {
            Item::Weapon(1..=5) => true,
            Item::Assembled { id, .. } => a.tool_bank.get(id as usize)
                .is_some_and(|tool| super::tool_workshop::attack_damage(tool) > super::inventory::weapon_damage(0)),
            Item::Wand { id, .. } => {
                let Some(wand) = a.wand_record(id) else { continue };
                let attack = (0..wands::capacity(wand.tier)).any(|slot| {
                    let spell = wand.loadout.slot(slot);
                    wands::projectile(spell) && spell_defs::spell_base_damage(spell) > 0.0
                        && a.can_wield_at(spell, wand.tier, wand.loadout.mode)
                });
                if !attack { empty_deck.get_or_insert(item); }
                attack
            }
            _ => false,
        };
        if !usable { continue; }
        if slot < world.inventory.hotbar.len() {
            if slot == hand_slot { return Gear::Held(item); }
            hotbar.get_or_insert(Gear::Hotbar(item, slot));
        } else {
            return hotbar.unwrap_or(Gear::Pack(item));
        }
    }
    hotbar.or_else(|| empty_deck.map(Gear::EmptyDeck)).unwrap_or(Gear::Missing)
}
pub fn home(cfg: &WorldSampleConfig) -> Option<Vec3> {
    if !matches!(cfg.world_type, super::WorldType::Hills | super::WorldType::Ridged) { return None; }
    let heart = sites::workshop_heart(cfg)?;
    Some(Vec3::new(-3.5, heart.y + 19.0, -39.5))
}
pub fn has_lanterncap(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= 48 && matches!(cfg.world_type, super::WorldType::Hills | super::WorldType::Ridged)
}

pub fn marker(a: &Adventure, cfg: &WorldSampleConfig, origin: Vec3, prepared: bool) -> Option<QuestMarker> {
    if cfg.generator_version < 48 || (returned(a) && automated(a)) { return None; }
    let home = home(cfg)?;
    let (pos, label) = if returned(a) {
        (home, "Workshop Supply")
    } else if cleared(a) && a.companion.is_none() {
        (sites::workshop_heart(cfg)?, "Free the Lanterncap")
    } else if cleared(a) {
        (home, "Return to the workbench")
    } else if !prepared {
        (home, "Workshop: prepare equipment")
    } else {
        (sites::workshop_mine_entrance(cfg)?, "Mine Entrance")
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
    let safe = world.adventure.enemies.iter().all(|e| e.health <= 0.0 || !e.hostile() || e.pos.distance(feet) > 20.0);
    let checkpoint = if !safe {
        " · Checkpoint Unchanged: Hostiles Nearby"
    } else if world.adventure.respawn_point.is_none_or(|p| p.distance(feet) > 1.0) {
        " · Checkpoint Set"
    } else {
        ""
    };
    if safe {
        world.adventure.respawn_point = Some(feet);
    }
    let status = if cleared(&world.adventure) && !returned(&world.adventure) {
        *world.adventure.records.entry(IVec3::ZERO).or_default() |= RETURNED;
        world.adventure.sounds.push(crate::audio::Event {
            cue: crate::audio::Cue::Treasure, pos: Some(home), gain: 0.7, pitch: 1.0,
        });
        if automated(&world.adventure) { "Workshop Complete" } else { "Workshop Reclaimed · Supply Pending" }
    } else if returned(&world.adventure) {
        if automated(&world.adventure) { "Workshop Complete" } else { "Iron, Steel or Bio-ethanol → Storage" }
    } else {
        match gear(world, world.adventure.hand_slot) {
            Gear::Held(_) => "Guardian Vault · Lanterncap trapped below",
            Gear::Hotbar(..) => "Select a wand or weapon from the hotbar",
            Gear::Pack(_) => "Move a wand or weapon to the hotbar",
            Gear::EmptyDeck(_) => "Wands · Slot Spark or another attack spell",
            Gear::Missing => "Copper sword · Recipe in Quest Log",
        }
    };
    world.expedition.say(format!("{status}{checkpoint}"));
}
