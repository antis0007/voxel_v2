//! Spell discovery. A wand tier decides which spells it *can* hold; the spell
//! log decides which ones the player has actually *found*. Fresh adventures
//! start with a small starter set and earn the rest from guardians, cleared
//! sites and wand upgrades. Everything else (old saves, tests, creative
//! scenes) defaults to "everything found", so nothing a player already owned
//! is ever taken away.
use super::*;

/// The spells every fresh adventurer begins with: Spark, Mend, Frost lance and
/// Mirror ward, matching what a tier-1 wand is seeded with.
pub const STARTER_SPELLS: [u32; 4] = [0, 1, 2, 4];

const WORDS: usize = 3;
const _: () = assert!(MAX_SPELL_ID < (WORDS as u32) * 32);

/// Bit set over spell ids 0..=95. Persisted from save version 19.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpellLog {
    bits: [u32; WORDS],
}

impl SpellLog {
    /// Every spell found. This is the default so legacy state is never reduced.
    pub const fn all() -> Self {
        Self {
            bits: [!0; WORDS],
        }
    }

    pub fn starter() -> Self {
        let mut log = Self { bits: [0; WORDS] };
        for id in STARTER_SPELLS {
            log.add(id);
        }
        log
    }

    pub fn has(&self, id: u32) -> bool {
        id <= MAX_SPELL_ID && self.bits[(id / 32) as usize] & (1 << (id % 32)) != 0
    }

    /// Marks a spell found; true if it was new.
    pub fn add(&mut self, id: u32) -> bool {
        if id > MAX_SPELL_ID || self.has(id) {
            return false;
        }
        self.bits[(id / 32) as usize] |= 1 << (id % 32);
        true
    }

    /// Spells 6 and 7 are discovered by claiming reliquaries, which already
    /// gate them through the rare-magic bits, so the log never blocks them.
    pub fn grants(&self, id: u32) -> bool {
        matches!(id, 6 | 7) || self.has(id)
    }

    pub fn words(&self) -> [u32; WORDS] {
        self.bits
    }

    pub fn from_words(bits: [u32; WORDS]) -> Self {
        Self { bits }
    }
}

impl Default for SpellLog {
    fn default() -> Self {
        Self::all()
    }
}

/// The lowest wand tier that can hold this spell with no rare-magic progress.
pub fn spell_tier(id: u32) -> u32 {
    (1..=3)
        .find(|&wand| spell_available(id, wand, 0))
        .unwrap_or(3)
}

/// True for ids that name a real spell and are found by the log (not by the
/// rare-magic reliquary path).
fn discoverable(id: u32) -> bool {
    id != wands::EMPTY && !matches!(id, 6 | 7) && spell_name(id) != "Unknown spell"
}

pub(super) fn mix32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// Deterministic drop roll for a guardian's first defeat. The same enemy at the
/// same site always rolls the same number, so reloading cannot reroll loot.
/// Returns the salt to pick a spell with when the roll succeeds.
pub fn drop_roll(site: IVec3, slot: u32, menace: u8) -> Option<u32> {
    let h = mix32(
        (site.x as u32).wrapping_mul(0x9e37_79b1)
            ^ (site.y as u32).wrapping_mul(0x85eb_ca6b)
            ^ (site.z as u32).wrapping_mul(0xc2b2_ae35)
            ^ slot.wrapping_mul(0x27d4_eb2f)
            ^ 0xd15c_0000,
    );
    let chance = 28 + 9 * u32::from(menace.min(3));
    (h % 100 < chance).then_some(mix32(h ^ 0x51ed_270b))
}

/// Stable salt for the spell a cleared site teaches.
pub fn site_salt(site: IVec3) -> u32 {
    mix32(
        (site.x as u32).wrapping_mul(0x9e37_79b1)
            ^ (site.y as u32).wrapping_mul(0x85eb_ca6b)
            ^ (site.z as u32).wrapping_mul(0xc2b2_ae35)
            ^ 0x5a17_0000,
    )
}

/// Chaotic chassis: reorders the occupied slots (within the wand's capacity)
/// by a seeded Fisher-Yates pass. Empty slots stay where they are, so only the
/// order of the spells changes, never which spells are on the wand.
pub fn shuffle_deck(deck: &mut wands::Loadout, seed: u32, capacity: usize) {
    let spots: Vec<usize> = (0..wands::MAX_SLOTS.min(capacity))
        .filter(|&i| deck.slot(i) != wands::EMPTY)
        .collect();
    let mut spells: Vec<u32> = spots.iter().map(|&i| deck.slot(i)).collect();
    for i in (1..spells.len()).rev() {
        let j = mix32(seed ^ (i as u32).wrapping_mul(0x9e37_79b1)) as usize % (i + 1);
        spells.swap(i, j);
    }
    for (&spot, spell) in spots.iter().zip(spells) {
        deck.set_slot(spot, spell);
    }
}

impl Adventure {
    /// Begins a fresh adventure with only the starter spells found.
    pub fn begin_journey(&mut self) {
        self.spell_log = SpellLog::starter();
    }

    /// Can this spell be placed and cast right now: right wand tier (and rare
    /// progress where required) *and* found.
    pub fn can_wield(&self, spell: u32) -> bool {
        spell_available(spell, self.wand, self.rare_spells) && self.spell_log.grants(spell)
    }

    /// The deck with every unfound spell blanked out, for evaluation. Slot
    /// positions are preserved so the evaluator's slot indices still match.
    pub(super) fn usable_loadout(&self) -> wands::Loadout {
        let mut deck = self.loadout;
        for idx in 0..wands::MAX_SLOTS {
            let s = deck.slot(idx);
            if s != wands::EMPTY && !self.spell_log.grants(s) {
                deck.set_slot(idx, wands::EMPTY);
            }
        }
        if deck.mode == 3 {
            shuffle_deck(&mut deck, self.shuffle_seed, wands::capacity(self.wand));
        }
        deck
    }

    /// Undiscovered spells that could plausibly be found now: any tier up to one
    /// above the current wand, so a good drop can pull the player forward.
    pub fn discovery_pool(&self) -> Vec<u32> {
        let reach = self.wand.max(1) + 1;
        (0..=MAX_SPELL_ID)
            .filter(|&id| {
                discoverable(id) && !self.spell_log.has(id) && spell_tier(id) <= reach
            })
            .collect()
    }

    /// Undiscovered spells this wand tier can already hold.
    fn current_tier_pool(&self) -> Vec<u32> {
        let tier = self.wand.max(1);
        self.discovery_pool()
            .into_iter()
            .filter(|&id| spell_tier(id) <= tier)
            .collect()
    }

    /// Finds one spell, chosen deterministically by `salt`. Mostly it is a spell
    /// this wand can use now; one find in six peeks a tier ahead, which gives a
    /// reason to upgrade without flooding the player with unusable spells.
    pub fn discover_with(&mut self, salt: u32) -> Option<u32> {
        let usable = self.current_tier_pool();
        let ahead: Vec<u32> = self
            .discovery_pool()
            .into_iter()
            .filter(|id| !usable.contains(id))
            .collect();
        let peek = !ahead.is_empty() && (usable.is_empty() || mix32(salt ^ 0xa11e).is_multiple_of(6));
        let pool = if peek { &ahead } else { &usable };
        if pool.is_empty() {
            return None;
        }
        let id = pool[mix32(salt) as usize % pool.len()];
        self.spell_log.add(id);
        Some(id)
    }

    /// A guardian drop. Rarer as the current tier's spells run out, so the
    /// hunt stretches across the tier instead of ending in the first hours.
    pub fn discover_drop(&mut self, salt: u32) -> Option<u32> {
        let remaining = self.current_tier_pool().len() as f32;
        let keep = (remaining / 24.0).clamp(0.35, 1.0);
        let roll = (mix32(salt ^ 0x7e57) % 1000) as f32 / 1000.0;
        if roll >= keep {
            return None;
        }
        self.discover_with(salt)
    }

    /// Finds a spell and tells the player about it.
    pub(super) fn discover_spell(
        &mut self,
        world: &mut World,
        salt: u32,
        at: Vec3,
        guaranteed: bool,
    ) {
        let found = if guaranteed {
            self.discover_with(salt)
        } else {
            self.discover_drop(salt)
        };
        let Some(id) = found else {
            return;
        };
        world.expedition.say(format!(
            "Spell discovered: {}. {}",
            spell_name(id),
            spell_hint(id)
        ));
        sound(&mut self.sounds, Cue::Treasure, Some(at), 0.8);
        self.effect(at, spell_color(id), 1.1);
        self.spawn_voxel_burst(at, spell_color(id), 16, 2.4, 0.04);
    }

    /// A wand upgrade teaches two spells, so new slots are never empty-handed.
    /// Returns what was found, for the caller to announce.
    pub fn grant_upgrade_spells(&mut self, tier: u32) -> Vec<u32> {
        let base = 0x7a11_0000 ^ tier.wrapping_mul(0x9e37_79b1);
        (0..2u32)
            .filter_map(|n| self.discover_with(base ^ n.wrapping_mul(0x85eb_ca6b)))
            .collect()
    }

    pub fn encode_spell_log(&self, bytes: &mut Vec<u8>) {
        for word in self.spell_log.words() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }

    pub fn decode_spell_log(
        read: &mut impl FnMut() -> anyhow::Result<u32>,
    ) -> anyhow::Result<SpellLog> {
        Ok(SpellLog::from_words([read()?, read()?, read()?]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::WorldType};

    #[test]
    fn default_log_keeps_everything_and_starter_is_small() {
        let all = SpellLog::default();
        assert!((0..=MAX_SPELL_ID).all(|id| all.has(id)));
        let start = SpellLog::starter();
        for id in STARTER_SPELLS {
            assert!(start.has(id));
        }
        assert_eq!((0..=MAX_SPELL_ID).filter(|&id| start.has(id)).count(), 4);
        assert!(!start.has(3) && !start.has(29) && !start.has(70));
        assert!(!start.has(MAX_SPELL_ID + 1), "out of range is never found");
    }

    #[test]
    fn add_reports_whether_a_spell_was_new() {
        let mut log = SpellLog::starter();
        assert!(log.add(3));
        assert!(!log.add(3));
        assert!(!log.add(0));
        assert!(!log.add(MAX_SPELL_ID + 1));
    }

    #[test]
    fn starter_spells_are_castable_on_the_first_wand() {
        for id in STARTER_SPELLS {
            assert!(spell_available(id, 1, 0), "starter spell {id} needs a better wand");
        }
    }

    #[test]
    fn rare_reliquary_spells_are_never_blocked_by_the_log() {
        let log = SpellLog::starter();
        assert!(log.grants(6) && log.grants(7));
        assert!(!log.grants(3));
        let mut a = Adventure::default();
        a.wand = 3;
        a.begin_journey();
        assert!(!a.can_wield(6), "rare gating still applies");
        a.rare_spells = (1 << 6) | (1 << 7);
        assert!(a.can_wield(6) && a.can_wield(7));
    }

    #[test]
    fn the_pool_excludes_found_rare_and_overly_advanced_spells() {
        let mut a = Adventure::default();
        a.wand = 1;
        a.begin_journey();
        let pool = a.discovery_pool();
        assert!(!pool.is_empty());
        assert!(pool.iter().all(|&id| !a.spell_log.has(id)));
        assert!(!pool.contains(&6) && !pool.contains(&7));
        assert!(!pool.contains(&8) && !pool.contains(&33));
        assert!(
            pool.iter().all(|&id| spell_tier(id) <= 2),
            "a tier-1 wand may only be offered tiers 1 and 2"
        );
        assert!(!pool.contains(&29), "meteor needs the top wand");
        a.wand = 3;
        assert!(a.discovery_pool().contains(&29));
    }

    #[test]
    fn discovery_is_deterministic_never_repeats_and_terminates() {
        let mut a = Adventure::default();
        a.wand = 3;
        a.begin_journey();
        let mut b = Adventure::default();
        b.wand = 3;
        b.begin_journey();
        let mut seen = std::collections::HashSet::new();
        let mut n = 0u32;
        while let Some(id) = a.discover_with(n) {
            assert!(seen.insert(id), "spell {id} found twice");
            assert_eq!(b.discover_with(n), Some(id), "same salt, same spell");
            n += 1;
            assert!(n < 200, "discovery never terminates");
        }
        // Everything reachable was found, and only the rare pair is held back.
        let expected = (0..=MAX_SPELL_ID).filter(|&id| discoverable(id)).count() - STARTER_SPELLS.len();
        assert_eq!(seen.len(), expected);
        assert!(!a.spell_log.has(6) && !a.spell_log.has(7));
    }

    #[test]
    fn drop_rolls_are_stable_and_in_a_sane_range() {
        let site = IVec3::new(120, 3, -44);
        assert_eq!(drop_roll(site, 2, 1), drop_roll(site, 2, 1));
        for menace in 0..=3u8 {
            let hits = (0..2000u32)
                .filter(|&i| drop_roll(IVec3::new(i as i32 * 7, 1, -(i as i32)), i % 7, menace).is_some())
                .count();
            let rate = hits as f32 / 2000.0;
            let want = (28 + 9 * menace as u32) as f32 / 100.0;
            assert!((rate - want).abs() < 0.05, "menace {menace}: rate {rate} vs {want}");
        }
    }

    #[test]
    fn upgrades_teach_two_spells_and_never_duplicate() {
        let mut a = Adventure::default();
        a.wand = 2;
        a.begin_journey();
        let found = a.grant_upgrade_spells(2);
        assert_eq!(found.len(), 2);
        assert_ne!(found[0], found[1]);
        assert!(found.iter().all(|&id| a.spell_log.has(id)));
    }

    #[test]
    fn unfound_spells_cannot_be_cast_but_found_ones_can() {
        let scale = VoxelScalePreset::OneMeter;
        let mut w = World::with_seed(WorldType::Superflat, scale, 17);
        let mut p = Player::new();
        p.pos = Vec3::new(0.0, 150.0, 0.0) / scale.meters();
        p.flying = true;
        let eye = p.eye(scale) * scale.meters();
        let mut a = Adventure::default();
        a.wand = 3;
        a.rare_spells = !0;
        a.mana = 100.0;
        a.begin_journey();
        a.loadout.slots = [3, 0, wands::EMPTY, wands::EMPTY];
        a.loadout.aux_slots = [wands::EMPTY; 4];
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_eq!(a.projectiles.len(), 1, "only the found spell fires");
        assert_eq!(a.projectiles[0].spell, 0);

        a.projectiles.clear();
        a.mana = 100.0;
        a.loadout.cursor = 0;
        a.spell_log.add(3);
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_eq!(a.projectiles.len(), 1);
        assert_eq!(a.projectiles[0].spell, 3, "once found, the burst is castable");
    }

    #[test]
    fn set_wand_slot_refuses_unfound_spells() {
        let mut a = Adventure::default();
        a.wand = 3;
        a.rare_spells = !0;
        a.begin_journey();
        a.loadout.slots = [0, wands::EMPTY, wands::EMPTY, wands::EMPTY];
        a.set_wand_slot(1, 3);
        assert_eq!(a.loadout.slot(1), wands::EMPTY, "burst is not found yet");
        a.spell_log.add(3);
        a.set_wand_slot(1, 3);
        assert_eq!(a.loadout.slot(1), 3);
    }

    #[test]
    fn seeding_a_new_tier_only_uses_found_spells() {
        let mut deck = wands::Loadout::default();
        deck.slots = [wands::EMPTY; 4];
        let known = |id: u32| id == 9 || id == 3;
        wands::seed_unlocked_slots_known(&mut deck, 2, &known);
        let placed: Vec<u32> = (0..wands::MAX_SLOTS)
            .map(|i| deck.slot(i))
            .filter(|&s| s != wands::EMPTY)
            .collect();
        assert!(!placed.is_empty());
        assert!(placed.iter().all(|&s| known(s)), "{placed:?}");
    }

    #[test]
    fn spell_log_round_trips_through_the_save_block() {
        let mut a = Adventure::default();
        a.begin_journey();
        a.spell_log.add(29);
        a.spell_log.add(70);
        let mut bytes = Vec::new();
        a.encode_spell_log(&mut bytes);
        assert_eq!(bytes.len(), 12);
        let mut cursor = 0usize;
        let mut read = || {
            let w = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
            Ok(w)
        };
        let back = Adventure::decode_spell_log(&mut read).unwrap();
        assert_eq!(back, a.spell_log);
        assert!(back.has(29) && back.has(70) && !back.has(3));
    }
}

#[cfg(test)]
mod pacing {
    use super::*;

    /// Defeats `kills` guardians at distinct sites and returns how many spells
    /// were found, and how many of those were beyond the wand's own tier.
    fn simulate(a: &mut Adventure, kills: u32, menace: u8) -> (u32, u32) {
        let (mut found, mut ahead) = (0, 0);
        let tier = a.wand.max(1);
        for i in 0..kills {
            let site = IVec3::new((i / 4) as i32 * 31, 2, (i / 4) as i32 * -17);
            if let Some(salt) = drop_roll(site, i % 4, menace) {
                // Only count a peek while the wand's own tier still had spells
                // left; once it is drained, tier-ahead finds are all that remain.
                let usable_left = !a.current_tier_pool().is_empty();
                if let Some(id) = a.discover_drop(salt) {
                    found += 1;
                    if usable_left && spell_tier(id) > tier {
                        ahead += 1;
                    }
                }
            }
        }
        (found, ahead)
    }

    fn fresh(wand: u32) -> Adventure {
        let mut a = Adventure::default();
        a.wand = wand;
        a.begin_journey();
        a
    }

    #[test]
    fn the_opening_hook_is_quick_but_not_instant() {
        let (found, _) = simulate(&mut fresh(1), 12, 1);
        assert!((4..=9).contains(&found), "{found} spells in the first 12 kills");
    }

    #[test]
    fn the_hunt_stretches_across_the_tier() {
        let mut a = fresh(1);
        let (mid, _) = simulate(&mut a, 60, 1);
        assert!(mid <= 24, "{mid} found by 60 kills: too fast");
        assert!(!a.current_tier_pool().is_empty(), "tier drained by 60 kills");

        // Continue from kill 60 so the sites are not rolled twice.
        let mut kills = 60u32;
        while !a.current_tier_pool().is_empty() {
            simulate_one(&mut a, kills);
            kills += 1;
            assert!(kills < 600, "the tier never drains");
        }
        assert!((110..=260).contains(&kills), "tier drained in {kills} kills");
    }

    fn simulate_one(a: &mut Adventure, i: u32) {
        let site = IVec3::new((i / 4) as i32 * 31, 2, (i / 4) as i32 * -17);
        if let Some(salt) = drop_roll(site, i % 4, 1) {
            a.discover_drop(salt);
        }
    }

    #[test]
    fn peeking_a_tier_ahead_is_the_exception() {
        let mut a = fresh(1);
        let (found, ahead) = simulate(&mut a, 300, 2);
        assert!(found > 20);
        assert!(
            (ahead as f32) / (found as f32) < 0.35,
            "{ahead} of {found} finds were beyond the wand"
        );
        assert!(ahead > 0, "a tier-ahead find should be possible");
    }

    #[test]
    fn guaranteed_finds_ignore_the_slowdown() {
        // With almost nothing left in the tier, drops are throttled, but a site
        // clear or wand upgrade must always teach something while any spell is
        // still undiscovered.
        let mut a = fresh(1);
        while a.current_tier_pool().len() > 1 {
            let n = a.current_tier_pool().len() as u32;
            a.discover_with(n);
        }
        let throttled = (0..200u32).filter(|&s| a.clone_log_drop(s).is_none()).count();
        assert!(throttled > 60, "drops should thin out, got {throttled}/200 misses");
        assert!(a.discover_with(1).is_some());
    }

    impl Adventure {
        /// Test helper: would this salt find a spell, without keeping the find.
        fn clone_log_drop(&self, salt: u32) -> Option<u32> {
            let saved = self.spell_log;
            let mut probe = Adventure::default();
            probe.wand = self.wand;
            probe.spell_log = saved;
            probe.discover_drop(salt)
        }
    }
}
