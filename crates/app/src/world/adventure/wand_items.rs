//! Wands are inventory items. Each owns a tier, a chassis seed and its own deck;
//! `Adventure::wand`/`loadout` are the working copy of whichever wand is held,
//! written back to the bank every tick so dropping or storing one loses nothing.
use super::*;
use crate::wand_style::{GLOWS, GRIPS, SHAFTS, TRAILS, WandStyle};
use crate::world::inventory::{Item, ItemStack, SurvivalInventory};
use engine_core::hash::{hash01, hash_u32};

pub const MAX_WANDS: usize = 4096;
pub const GRIMOIRE_LOOK: u16 = 1 << 15;
/// Largest multiple of the rank's mana pool a found wand can roll.
pub const MAX_MANA_SCALE: f32 = 1.7;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct WandRecord {
    pub tier: u32,
    pub loadout: wands::Loadout,
}

fn roll(seed: u32, salt: u32) -> f32 {
    hash01(hash_u32(seed ^ salt.wrapping_mul(0x9E37_79B9)))
}
fn pick(seed: u32, salt: u32, n: usize) -> usize {
    ((roll(seed, salt) * n as f32) as usize).min(n.saturating_sub(1))
}

const TITLES: [&str; 24] = [
    "Ashen Rod", "Gloam Sceptre", "Brittle Switch", "Hollow Reed", "Cinder Baton",
    "Tidecaller", "Rimeglass Spire", "Bramble Crook", "Lantern Staff", "Mire Prod",
    "Starling Wand", "Coalfork", "Saltbone Wand", "Pilgrim's Stave", "Thornlight",
    "Quartz Needle", "Vesper Rod", "Hearthstick", "Driftwood Wand", "Gravel Sceptre",
    "Moth Crook", "Echo Spindle", "Sunken Rod", "Wildfire Switch",
];

/// Seeded chassis: the same seed always rolls the same wand. Seed 0 is the plain
/// crafted chassis and returns the tier's table stats unchanged.
pub fn stats_for(tier: u32, loadout: &wands::Loadout, rank: u32) -> wands::WandStats {
    let mut s = wands::stats(tier, loadout.mode, rank);
    let grimoire = wands::is_grimoire(loadout.mode);
    if let Some(binding) = super::enchanting::binding(loadout.mode) {
        s.name = &binding.grimoire;
        s.chassis = "Bound";
        s.mana_max *= binding.mana_capacity;
    }
    let seed = loadout.seed;
    if seed == 0 || tier == 0 {
        return s;
    }
    let span = |salt, lo: f32, hi: f32| lo + (hi - lo) * roll(seed, salt);
    if !grimoire {
        s.name = TITLES[pick(seed, 0, TITLES.len())];
    }
    s.cast_delay = (s.cast_delay * span(1, 0.55, 1.6)).max(0.02);
    s.recharge_time = (s.recharge_time * span(2, 0.5, 1.7)).max(0.08);
    s.mana_max = (s.mana_max * span(3, 0.7, MAX_MANA_SCALE)).round();
    s.mana_charge_speed *= charge_scale(loadout);
    s.spread_deg = (s.spread_deg + span(5, -2.0, 6.0)).max(0.0);
    if !grimoire && shuffled(seed, tier) {
        s.shuffle = true;
        s.recharge_time *= 0.7;
        s.chassis = "Shuffle";
    }
    if !grimoire && tier >= 2 && roll(seed, 6) < 0.22 {
        s.spells_per_cast += 1;
    }
    s
}

pub fn charge_scale(loadout: &wands::Loadout) -> f32 {
    if loadout.seed == 0 { 1.0 } else { 0.65 + 0.95 * roll(loadout.seed, 4) }
}

/// Shuffle wands are cheaper to recharge, as in Noita; riskier at low tiers.
pub fn shuffled(seed: u32, tier: u32) -> bool {
    seed != 0 && roll(seed, 7) < 0.34 - tier as f32 * 0.06
}

/// The look of a wand. Crafted wands wear the player's atelier style.
pub fn style(seed: u32, crafted: WandStyle) -> WandStyle {
    if seed == 0 {
        return crafted;
    }
    WandStyle {
        shaft: pick(seed, 10, SHAFTS.len()) as u8,
        // Existing saved seeds were rolled against six families. Keep those
        // identities stable; appended families are selectable in the atelier.
        head: pick(seed, 11, 6) as u8,
        trail: pick(seed, 12, TRAILS.len()) as u8,
        grip: pick(seed, 13, GRIPS.len()) as u8,
        glow: pick(seed, 14, GLOWS.len()) as u8,
        runes: pick(seed, 15, 3) as u8,
    }
}

/// Icon bits carried on the item itself: tier, shaft, head and glow.
pub fn look(record: &WandRecord) -> u16 {
    let s = style(record.loadout.seed, crate::wand_style::current());
    (record.tier.min(3) as u16)
        | (s.shaft as u16 & 7) << 2
        | (s.head as u16 & 7) << 5
        | (s.glow as u16 & 7) << 8
        | u16::from(record.loadout.seed == 0) << 11
        | (s.grip as u16 & 7) << 12
        | if wands::is_grimoire(record.loadout.mode) { GRIMOIRE_LOOK } else { 0 }
}

/// A wand lying on the ground: a small tilted shaft with its head glowing.
pub fn draw_dropped(m: &mut crate::mesh::models::Model, centre: Vec3, rot: Quat, look: u16) {
    crate::mesh::models::wand_kit::draw_dropped(m, centre, rot, look);
}

fn usable(id: u32, tier: u32) -> bool {
    id != wands::EMPTY
        && !matches!(id, 6 | 7 | super::spells::STORM_SURGE)
        && wands::fits_instrument(id, 0)
        && spell_name(id) != "Unknown spell"
        && spell_available(id, tier, 0)
}

/// A found wand: seeded chassis, mode and a pre-built deck that leads with a bolt.
pub fn generate(seed: u32, tier: u32) -> WandRecord {
    let seed = seed.max(1);
    let tier = tier.clamp(1, 3);
    let bolts: Vec<u32> = (0..=MAX_SPELL_ID).filter(|&i| usable(i, tier) && wands::projectile(i)).collect();
    let others: Vec<u32> = (0..=MAX_SPELL_ID).filter(|&i| usable(i, tier) && !wands::projectile(i)).collect();
    let mut loadout = wands::Loadout {
        slots: [wands::EMPTY; 4],
        aux_slots: [wands::EMPTY; 4],
        cursor: 0,
        mode: if tier >= 2 { pick(seed, 20, 3) as u32 } else { 0 },
        seed,
    };
    let cap = wands::capacity(tier);
    let filled = (cap as f32 * (0.5 + 0.5 * roll(seed, 21))).ceil() as usize;
    for i in 0..filled.min(cap) {
        let use_bolt = i == 0 || others.is_empty() || roll(seed, 30 + i as u32) < 0.6;
        let pool = if use_bolt { &bolts } else { &others };
        if !pool.is_empty() {
            loadout.set_slot(i, pool[pick(seed, 40 + i as u32, pool.len())]);
        }
    }
    WandRecord { tier, loadout }
}

/// Which defeated foes leave a wand: every guardian, and a few ordinary foes,
/// more often when they were dangerous. Deterministic per encounter slot.
pub fn drop_roll(site: IVec3, slot: u32, kind: u32, menace: u8, boss: bool, rank: u32) -> Option<(u32, u32)> {
    let seed = hash_u32(
        (site.x as u32).wrapping_mul(73_856_093)
            ^ (site.y as u32).wrapping_mul(19_349_663)
            ^ (site.z as u32).wrapping_mul(83_492_791)
            ^ slot.wrapping_mul(0x27d4_eb2d)
            ^ kind.wrapping_mul(0x1656_67b1),
    )
    .max(1);
    let menace = menace as u32;
    let tier = if boss { 2 + u32::from(menace >= 1 || rank >= 3) } else { 1 + menace / 2 + u32::from(rank >= 3) };
    let chance = 0.05 + menace as f32 * 0.04;
    (boss || roll(seed, 99) < chance).then_some((seed, tier.min(3)))
}

impl Adventure {
    /// The held deck as the viewmodel shows it: one entry per slot the wand has.
    pub fn deck_view(&self) -> Vec<Option<[f32; 3]>> {
        (0..wands::capacity(self.wand))
            .map(|i| {
                let s = self.loadout.slot(i);
                (s != wands::EMPTY).then(|| spell_color(s))
            })
            .collect()
    }
    /// The best tier among the held wand and every wand ever owned.
    pub fn best_wand_tier(&self) -> u32 {
        self.wand_bank.iter().map(|r| r.tier).max().unwrap_or(0).max(self.wand)
    }
    pub fn wand_record(&self, id: u16) -> Option<&WandRecord> {
        self.wand_bank.get(id as usize)
    }
    /// Registers a wand and returns its item. Bounded: a full bank mints nothing.
    pub fn mint_wand(&mut self, record: WandRecord) -> Option<Item> {
        if self.wand_bank.len() >= MAX_WANDS {
            return None;
        }
        let id = self.wand_bank.len() as u16;
        self.wand_bank.push(record);
        Some(Item::Wand { id, look: look(&record) })
    }
    /// Keeps the held wand and the inventory in agreement. `drop_at` receives a
    /// newly crafted wand that does not fit in the pack.
    pub fn sync_wands(&mut self, inv: &mut SurvivalInventory, selected: usize, drop_at: Vec3) {
        if self.wand == 0 {
            self.active_wand = None;
        } else {
            self.clear_wand_channels();
        }
        if let Some(a) = self.active_wand
            && let Some(record) = self.wand_bank.get_mut(a as usize)
        {
            let changed_tier = record.tier != self.wand;
            *record = WandRecord { tier: self.wand, loadout: self.loadout };
            if changed_tier {
                let new_look = look(record);
                for slot in inv.hotbar.iter_mut().chain(inv.pack.iter_mut()).flatten() {
                    if let Item::Wand { id, look } = &mut slot.item
                        && *id == a
                    {
                        *look = new_look;
                    }
                }
            }
        }
        // A wand assembled at the bench (or an old save's wand) becomes an item.
        if self.active_wand.is_none() && self.wand > 0 {
            let record = WandRecord { tier: self.wand, loadout: self.loadout };
            if let Some(item) = self.mint_wand(record) {
                if let Item::Wand { id, .. } = item {
                    self.active_wand = Some(id);
                }
                let (inserted, _) = inv.try_insert(ItemStack::new(item, 1));
                if inserted == 0 {
                    inv.dropped.push(crate::world::inventory::DroppedStack { pos: drop_at, stack: ItemStack::new(item, 1) });
                }
            }
        }
        let selected_wand = inv.hotbar.get(selected).copied().flatten().and_then(|stack| {
            match stack.item {
                Item::Wand { id, .. } => Some(id),
                _ => None,
            }
        });
        if let Some(id) = selected_wand
            && let Some(record) = self.wand_bank.get(id as usize).copied()
        {
            if self.active_wand != Some(id) {
                self.equip(id, record);
            }
        } else {
            // Ordinary hotbar items own the hand. The saved bank record retains
            // this wand's deck for the next explicit selection.
            self.active_wand = None;
            self.wand = 0;
            self.spell = 0;
            self.loadout = wands::Loadout::default();
        }
    }
    fn equip(&mut self, id: u16, record: WandRecord) {
        self.active_wand = Some(id);
        self.wand = record.tier;
        self.loadout = record.loadout;
        self.loadout.cursor = 0;
        // Spells mounted on a found wand are learned by holding it.
        for i in 0..wands::MAX_SLOTS {
            let s = record.loadout.slot(i);
            if s != wands::EMPTY {
                self.spell_log.add(s);
            }
        }
        self.clear_wand_channels();
        self.mana = self.mana.min(self.max_mana());
    }

    /// Older ordinary wands may still carry a channel. Keep the learned card
    /// in the spellbook when clearing its incompatible slot.
    fn clear_wand_channels(&mut self) {
        if wands::is_grimoire(self.loadout.mode) {
            return;
        }
        for i in 0..wands::MAX_SLOTS {
            let spell = self.loadout.slot(i);
            if !wands::fits_instrument(spell, self.loadout.mode) {
                self.spell_log.add(spell);
                self.loadout.set_slot(i, wands::EMPTY);
            }
        }
    }

    pub fn encode_wand_bank(&self, bytes: &mut Vec<u8>) {
        let mut put = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        put(self.wand_bank.len() as u32);
        put(self.active_wand.map_or(u32::MAX, u32::from));
        for r in &self.wand_bank {
            put(r.tier);
            put(r.loadout.mode);
            put(r.loadout.seed);
            for i in 0..wands::MAX_SLOTS {
                put(r.loadout.slot(i));
            }
        }
    }
    pub fn decode_wand_bank(&mut self, read: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<()> {
        use anyhow::ensure;
        let count = read()? as usize;
        ensure!(count <= MAX_WANDS, "Too many wands");
        let active = read()?;
        ensure!(active == u32::MAX || (active as usize) < count, "Invalid held wand");
        let mut bank = Vec::with_capacity(count);
        for _ in 0..count {
            let tier = read()?;
            let mode = read()?;
            let seed = read()?;
            ensure!((1..=3).contains(&tier)
                && (mode <= 3 || tier == 3 && wands::is_grimoire(mode)), "Invalid wand chassis");
            let mut loadout = wands::Loadout { slots: [wands::EMPTY; 4], aux_slots: [wands::EMPTY; 4], cursor: 0, mode, seed };
            for i in 0..wands::MAX_SLOTS {
                let s = read()?;
                ensure!(s <= MAX_SPELL_ID, "Invalid wand spell");
                loadout.set_slot(i, s);
            }
            bank.push(WandRecord { tier, loadout });
        }
        self.wand_bank = bank;
        self.active_wand = (active != u32::MAX).then_some(active as u16);
        if let Some(a) = self.active_wand {
            // The legacy block already restored the working deck and cursor.
            self.loadout.seed = self.wand_bank[a as usize].loadout.seed;
        }
        ensure!(self.mana <= self.max_mana(), "Mana exceeds the held wand's pool");
        Ok(())
    }
}

#[cfg(test)]
mod tests;
