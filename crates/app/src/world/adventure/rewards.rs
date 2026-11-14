use super::*;

pub(super) const KILN_CRAWLER: u32 = 12;

pub(super) fn hell_camp(world: &World, site: IVec3) -> bool {
    site.y == 500 && site.z == 1 && world.sample_config().is_hell()
}

/// Awarded through the normal encounter death record, once per guardian.
pub(super) fn hell_camp_drop(world: &mut World, enemy: &Enemy) {
    use super::super::inventory::{DroppedStack, Item, ItemStack};
    if enemy.kind != KILN_CRAWLER || !hell_camp(world, enemy.site) {
        return;
    }
    world.inventory.dropped.push(DroppedStack {
        pos: enemy.center(),
        stack: ItemStack::new(Item::Block(super::super::BlockId::KilnScale), 1),
    });
}

// Quantities are fixed-point inventory units (64 per whole item), not voxels.
pub(super) fn guardian(kind: u32) -> &'static [(usize, u32)] {
    match kind {
        0 => &[(13, 128)],                      // Cave grub: biomass.
        1 => &[(12, 64), (16, 32)],             // Sentinel: circuitry and coil parts.
        2 => &[(5, 64), (12, 32)],              // Wisp: crystal and arcane circuitry.
        3 => &[(13, 256), (4, 64)],             // Brute: richer fungal materials.
        4 => &[(4, 128), (5, 32)],              // Spitter: luminous fibre and crystal.
        6 => &[(15, 384), (3, 256), (17, 64)],  // Foundry: forging and power.
        7 => &[(12, 256), (5, 256), (16, 128)], // Archive: spell engineering.
        8 => &[(5, 384), (16, 192), (17, 64)],  // Prism: crystal and core experiments.
        9 => &[(13, 384), (3, 128), (17, 16)],  // Colossus: bulk biomass and coal.
        10 => &[(12, 96), (14, 128)],
        11 => &[(7, 128), (13, 128)],
        12 => &[(3, 128), (15, 64)],
        13 => &[(14, 128), (16, 64)],
        16 => &[(15, 192), (12, 96), (16, 32)],  // Sentry: steel, circuits, a coil.
        17 => &[(15, 96), (12, 64)],             // Hound: steel and circuits.
        hall_warden::FURNACE => &[(15, 384), (17, 128), (3, 256)],
        hall_warden::ASTRONOMER => &[(5, 384), (12, 256), (17, 128)],
        _ => &[(12, 128), (15, 64), (17, 16)], // Warden: steel and a core fragment.
    }
}

impl Adventure {
    /// One award path for new victories and previously stranded single-room saves.
    pub(super) fn complete_site(&mut self, world: &mut World, id: IVec3, at: Vec3) {
        let all = encounters::clear_mask(id);
        let record = self.records.entry(id).or_default();
        if *record & all != all || *record & 256 != 0 {
            return;
        }
        *record |= 256;
        if hell_camp(world, id) {
            world.expedition.say("Hell camp cleared");
            sound(&mut self.sounds, Cue::Treasure, Some(at), 0.75);
            return;
        }
        let cfg = world.sample_config();
        let reward = sanctum(&cfg, id);
        for &(item, amount) in &reward.items {
            world.expedition.add(item, amount);
        }
        self.reward = Some(reward);
        let message = if id == IVec3::ZERO && self.companion.is_none() && super::super::workshop::has_lanterncap(&cfg) {
            "Vault cleared · Lanterncap can be freed at the reliquary"
        } else {
            "The reliquary is unsealed. Claim its relic with F; new materials are ready for crafting."
        };
        world.expedition.say(message);
        self.discover_spell(world, discovery::site_salt(id), at, true);
        sound(&mut self.sounds, Cue::Treasure, Some(at), 0.75);
    }
}

pub(super) fn sanctum(cfg: &super::super::WorldSampleConfig, id: IVec3) -> Reward {
    let (name, bonus): (&str, &[(usize, u32)]) = match sites::reward_kind(cfg, id) {
        0 => ("Workshop reclaimed", &[(14, 192), (16, 64)]),
        1 => ("Watchtower reclaimed", &[(5, 192), (16, 128)]),
        2 => ("Waterworks reclaimed", &[(7, 256), (14, 192)]),
        3 => ("Foundry reclaimed", &[(15, 256), (16, 128)]),
        4 => ("Conservatory restored", &[(13, 512), (4, 256)]),
        5 => ("Archive unsealed", &[(17, 64), (5, 192)]),
        6 => ("Luminous grove restored", &[(4, 256), (13, 384)]),
        7 => ("Ember hollow cleansed", &[(3, 512), (15, 128)]),
        8 => ("Crystal cathedral unsealed", &[(5, 256), (17, 64)]),
        10 => ("Toll crossing unsealed", &[(15, 128), (4, 128)]),
        11 => ("Dock post silenced", &[(15, 256), (12, 128), (17, 32)]),
        _ => ("Drowned grotto reclaimed", &[(7, 256), (16, 128)]),
    };
    let mut items = vec![(18, 128), (12, 64)];
    items.extend_from_slice(bonus);
    Reward {
        name: if observatory::is_site(id) { "Observatory recovered" } else { name },
        items,
        seconds: 7.0,
        discovery: None,
    }
}
