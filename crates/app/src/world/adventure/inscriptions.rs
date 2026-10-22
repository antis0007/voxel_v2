//! Permanent discoveries derive from saved clear records, including older worlds.
use super::*;
pub struct Inscription {
    pub name: &'static str,
    pub site: &'static str,
    pub spell: u32,
    pub benefit: &'static str,
    pub story: &'static str,
}
pub const INSCRIPTIONS: [Inscription; 11] = [
    Inscription {
        name: "Artificer's mark",
        site: "Workshop",
        spell: 0,
        benefit: "Spark costs 2 less mana.",
        story: "Every coil is signed, and every signature carries a tally: sparks given, sparks owed. The last maker's tally ends at zero. Beneath it, another hand: \"Paid. Go up.\"",
    },
    Inscription {
        name: "Storm lens",
        site: "Watchtower",
        spell: 0,
        benefit: "Spark jumps to a third target; chain reach increases to 5.5 m.",
        story: "The watch never looked for armies. Its lenses point straight up, and its log keeps one entry every third evening: a light descending the braid, on time. The entries stop. The light did not.",
    },
    Inscription {
        name: "Tidal seal",
        site: "Waterworks",
        spell: 4,
        benefit: "Reflecting a bolt restores 6 mana.",
        story: "The sluice priests weighed water, not prayers. Their gauges read in a unit called the held breath. The tables beside them convert it to a weight the anchor still had to carry.",
    },
    Inscription {
        name: "Cinder script",
        site: "Foundry",
        spell: 3,
        benefit: "Burst shatters frozen enemies in its blast for 12 extra damage.",
        story: "Split on the anvil: a coupling for a braid thicker than a tower. Frost first, then the blow. The order book says the flawed half shipped upward anyway. The Loom would finish it.",
    },
    Inscription {
        name: "Verdant covenant",
        site: "Conservatory",
        spell: 5,
        benefit: "Spore fields last 7 seconds and heal 4 health per pulse.",
        story: "The seed trays are labelled for two gardens: this one, and one eighteen kilometres above the berth. The keepers went up. The lower garden kept its half of the promise and tends whoever stays.",
    },
    Inscription {
        name: "Archivist's cipher",
        site: "Archive",
        spell: 1,
        benefit: "All spells cost 20% less mana.",
        story: "The final ledger is not an inventory. It lists those who stayed below so the rest could rise. One line near the end has been scraped blank, carefully, by someone who knew the name.",
    },
    Inscription {
        name: "Mycelial memory",
        site: "Luminous grove",
        spell: 1,
        benefit: "Mend also restores 12 hunger when it heals you.",
        story: "Below the oldest cap the colony keeps the warmth of the last meal the Ballast shared. Its threads still run toward the toll road, as if carrying bread to someone waiting at the gate.",
    },
    Inscription {
        name: "Ember heart",
        site: "Ember hollow",
        spell: 3,
        benefit: "Defeating a guardian restores 4 health.",
        story: "A living coal sleeps beneath the ash, banked by someone who meant to come back down. It beats once for every traveller who returns home. Until you came, it had not beaten in a long time.",
    },
    Inscription {
        name: "Prism facet",
        site: "Crystal cathedral",
        spell: 2,
        benefit: "Frost lance slows guardians for 6 seconds instead of 4.",
        story: "The choir sang the tension of the braid, one voice to a strand. When a strand failed, a voice stopped. The rest held their notes until they turned to crystal, waiting for an answer from above.",
    },
    Inscription {
        name: "Undertow knot",
        site: "Drowned grotto",
        spell: 4,
        benefit: "Mirror ward lasts 9 seconds; its three charges remain unchanged.",
        story: "A sailor tied this knot around an empty breath and lowered it into the cistern as ballast. The cavern drowned. The breath endured, and the tables say it still weighs what it should.",
    },
    Inscription {
        name: "Keeper's passage",
        site: "Ruined toll crossing",
        spell: 4,
        benefit: "Mirror ward gains a fourth charge.",
        story: "The toll was never coin. Each traveller going up left one spark for those staying down. The last keeper opened the gate and cut into the timber: \"Nothing is owed now. Pass.\" The warden never read it.",
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
