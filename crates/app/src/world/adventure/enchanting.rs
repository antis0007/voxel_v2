//! Bind a held wand in place. The item ID, seed and spell slots stay with it.
use super::{wand_items, wands};
use crate::world::{BlockId, World, inventory::Item, progression::study};
use glam::IVec3;
use std::sync::LazyLock;

/// One grimoire binding. Rows live in `assets/enchantments.ron`; every reader
/// (stats, upkeep, the wand board, the guide) goes through [`binding`].
#[derive(serde::Deserialize)]
pub(crate) struct Binding {
    /// Loadout mode stored in saves.
    pub mode: u32,
    pub name: String,
    pub grimoire: String,
    pub effect: String,
    pub mana_capacity: f32,
    pub upkeep: f32,
}

#[derive(serde::Deserialize)]
pub(crate) struct Table {
    pub xp: u32,
    pub crystals: u32,
    crystal_resource: u8,
    pub bindings: Vec<Binding>,
}

impl Table {
    pub(crate) fn crystal(&self) -> Item {
        Item::Resource(self.crystal_resource)
    }
}

pub(crate) static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let table: Table = ron::from_str(include_str!("../../../../../assets/enchantments.ron"))
        .unwrap_or_else(|error| panic!("assets/enchantments.ron: {error}"));
    for (i, row) in table.bindings.iter().enumerate() {
        assert!(wands::is_grimoire(row.mode), "assets/enchantments.ron: mode {} is not a grimoire mode", row.mode);
        assert!(table.bindings[..i].iter().all(|other| other.mode != row.mode), "assets/enchantments.ron: duplicate mode {}", row.mode);
        assert!(row.mana_capacity > 0.0 && row.upkeep > 0.0, "assets/enchantments.ron: {} multipliers must be positive", row.name);
    }
    table
});

/// The binding a loadout mode carries, if any.
pub(crate) fn binding(mode: u32) -> Option<&'static Binding> {
    TABLE.bindings.iter().find(|row| row.mode == mode)
}

pub(crate) fn held(world: &World) -> Option<(usize, u16, wand_items::WandRecord)> {
    let slot = world.adventure.hand_slot;
    let Item::Wand { id, .. } = world.inventory.hotbar.get(slot)?.as_ref()?.item else {
        return None;
    };
    let record = *world.adventure.wand_record(id)?;
    (record.tier == 3).then_some((slot, id, record))
}

pub(crate) fn bind(world: &mut World, station: IVec3, mode: u32) -> bool {
    if !study::available(world) || world.adventure.health <= 0.0
        || world.get(station) != BlockId::EnchantingTable || binding(mode).is_none()
    {
        return false;
    }
    let Some((slot, id, mut record)) = held(world) else { return false };
    if world.adventure.active_wand == Some(id) {
        record.loadout = world.adventure.loadout;
    }
    let table = &*TABLE;
    if record.loadout.mode == mode || world.progression.xp < table.xp
        || !world.inventory.remove_item(table.crystal(), table.crystals)
    {
        return false;
    }
    world.progression.xp -= table.xp;
    record.loadout.mode = mode;
    world.adventure.wand_bank[id as usize] = record;
    world.adventure.spell_log.add(super::fieldcraft::SALVAGE);
    if world.adventure.active_wand == Some(id) {
        world.adventure.loadout.mode = mode;
        world.adventure.mana = world.adventure.mana.min(world.adventure.max_mana());
        world.adventure.cancel_cast_input();
    }
    if let Some(stack) = &mut world.inventory.hotbar[slot] {
        stack.item = Item::Wand { id, look: wand_items::look(&record) };
    }
    world.sync_from_inventory();
    true
}

/// Parse and check the table now; called once at startup.
pub fn validate() {
    LazyLock::force(&TABLE);
}
