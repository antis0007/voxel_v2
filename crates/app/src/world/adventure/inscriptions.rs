//! Permanent discoveries derive from saved clear records, including older worlds.
use super::*;
pub struct Inscription {
    pub name: &'static str,
    pub site: &'static str,
    pub spell: u32,
    pub benefit: &'static str,
}
pub const INSCRIPTIONS: [Inscription; 11] = [
    Inscription {
        name: "Artificer's mark",
        site: "Workshop",
        spell: 0,
        benefit: "Spark costs 2 less mana.",
    },
    Inscription {
        name: "Storm lens",
        site: "Watchtower",
        spell: 0,
        benefit: "Spark jumps to a third target; chain reach increases to 5.5 m.",
    },
    Inscription {
        name: "Tidal seal",
        site: "Waterworks",
        spell: 4,
        benefit: "Reflecting a bolt restores 6 mana.",
    },
    Inscription {
        name: "Cinder script",
        site: "Foundry",
        spell: 3,
        benefit: "Burst shatters frozen enemies in its blast for 12 extra damage.",
    },
    Inscription {
        name: "Verdant covenant",
        site: "Conservatory",
        spell: 5,
        benefit: "Spore fields last 7 seconds and heal 4 health per pulse.",
    },
    Inscription {
        name: "Archivist's cipher",
        site: "Archive",
        spell: 1,
        benefit: "All spells cost 20% less mana.",
    },
    Inscription {
        name: "Mycelial memory",
        site: "Luminous grove",
        spell: 1,
        benefit: "Mend also restores 12 hunger when it heals you.",
    },
    Inscription {
        name: "Ember heart",
        site: "Ember hollow",
        spell: 3,
        benefit: "Defeating a guardian restores 4 health.",
    },
    Inscription {
        name: "Prism facet",
        site: "Crystal cathedral",
        spell: 2,
        benefit: "Frost lance slows guardians for 6 seconds instead of 4.",
    },
    Inscription {
        name: "Undertow knot",
        site: "Drowned grotto",
        spell: 4,
        benefit: "Mirror ward lasts 9 seconds; its three charges remain unchanged.",
    },
    Inscription {
        name: "Keeper's passage",
        site: "Ruined toll crossing",
        spell: 4,
        benefit: "Mirror ward gains a fourth charge.",
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
