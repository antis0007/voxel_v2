//! Permanent discoveries derive from saved clear records, including older worlds.
use super::*;
pub struct Inscription {
    pub name: &'static str,
    pub site: &'static str,
    pub spell: u32,
    pub benefit: &'static str,
    pub story: &'static str,
}
pub const INSCRIPTIONS: [Inscription; 10] = [
    Inscription {
        name: "Artificer's mark",
        site: "Workshop",
        spell: 0,
        benefit: "Spark costs 2 less mana.",
        story: "The first engines ran on a single spark, willingly given. Their makers signed every coil.",
    },
    Inscription {
        name: "Storm lens",
        site: "Watchtower",
        spell: 0,
        benefit: "Spark jumps to a third target; chain reach increases to 5.5 m.",
        story: "The watchkeepers learned to pass lightning between their towers. No messenger crossed the storms alone.",
    },
    Inscription {
        name: "Tidal seal",
        site: "Waterworks",
        spell: 4,
        benefit: "Reflecting a bolt restores 6 mana.",
        story: "The sluice priests never fought the current. They returned its strength to the river.",
    },
    Inscription {
        name: "Cinder script",
        site: "Foundry",
        spell: 3,
        benefit: "Burst shatters frozen enemies in its blast for 12 extra damage.",
        story: "A failed casting lies split on the anvil: frost first, then a ringing blow. The flaw became a weapon.",
    },
    Inscription {
        name: "Verdant covenant",
        site: "Conservatory",
        spell: 5,
        benefit: "Spore fields last 7 seconds and heal 4 health per pulse.",
        story: "When the keepers vanished, the gardens kept their promise. Root and spore still tend the wounded.",
    },
    Inscription {
        name: "Archivist's cipher",
        site: "Archive",
        spell: 1,
        benefit: "All spells cost 20% less mana.",
        story: "The final ledger contains no inventory. Every page records a name the city refused to forget.",
    },
    Inscription {
        name: "Mycelial memory",
        site: "Luminous grove",
        spell: 1,
        benefit: "Mend also restores 12 hunger when it heals you.",
        story: "Below the oldest cap, the colony remembers sunlight. Its gift is the warmth of a meal long shared.",
    },
    Inscription {
        name: "Ember heart",
        site: "Ember hollow",
        spell: 3,
        benefit: "Defeating a guardian restores 4 health.",
        story: "A living coal sleeps beneath the ash. It beats once for every traveller who found their way home.",
    },
    Inscription {
        name: "Prism facet",
        site: "Crystal cathedral",
        spell: 2,
        benefit: "Frost lance slows guardians for 6 seconds instead of 4.",
        story: "The choir sang until their voices became crystal. A held note still waits between the pillars.",
    },
    Inscription {
        name: "Undertow knot",
        site: "Drowned grotto",
        spell: 4,
        benefit: "Mirror ward lasts 9 seconds; its three charges remain unchanged.",
        story: "A sailor tied this knot around an empty breath. The cavern drowned, but the breath endured.",
    },
];
pub fn cost(spell: u32, discovered: u32) -> f32 {
    (spell_cost(spell)
        - if spell == 0 && discovered & 1 != 0 {
            2.0
        } else {
            0.0
        })
        * if discovered & (1 << 5) != 0 { 0.8 } else { 1.0 }
}
impl Adventure {
    pub(super) fn knows(&self, id: u32) -> bool {
        self.discoveries & (1 << id) != 0
    }
    pub(super) fn restore_discoveries(&mut self, world: &World) {
        if self.discoveries_ready {
            return;
        }
        let cfg = world.sample_config();
        for (site, record) in &self.records {
            if record & reliquaries::CLAIMED != 0
                || record & 256 != 0 && record & reliquaries::PHYSICAL == 0
            {
                self.discoveries |= 1 << sites::reward_kind(&cfg, *site);
                if record & reliquaries::CLAIMED != 0 {
                    self.sigils |= quest::sigil(sites::reward_kind(&cfg, *site));
                    match sites::reward_kind(&cfg, *site) {
                        5 => self.rare_spells |= 1 << 6,
                        8 => self.rare_spells |= 1 << 7,
                        _ => {}
                    }
                }
            }
        }
        self.discoveries_ready = true;
    }
}
