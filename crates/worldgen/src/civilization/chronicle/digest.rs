use super::Chronicle;

impl Chronicle {
    /// Fixed-order integer digest for reproducible generation evidence.
    pub fn fingerprint(&self) -> u64 {
        let mut digest = 0xcbf2_9ce4_8422_2325_u64;
        let mut word = |v: u64| {
            for byte in v.to_le_bytes() {
                digest = (digest ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        };
        let p = self.provenance;
        for v in [
            u64::from(p.seed),
            u64::from(p.generator),
            p.planet_recipe,
            p.landscape_checksum,
            u64::from(p.model_revision),
            p.region[0] as u64,
            p.region[1] as u64,
            u64::from(p.deep_time_myr),
            u64::from(p.end_year),
            u64::from(p.step_years),
            u64::from(self.linked),
        ] {
            word(v);
        }
        for site in self.sites {
            word(u64::from(site.is_some()));
            let Some(s) = site else { continue };
            let g = s.landscape;
            let h = s.history;
            for v in [
                u64::from(s.id),
                s.center_mm[0] as u64,
                s.center_mm[1] as u64,
                s.center_mm[2] as u64,
                g.top_mm as u64,
                u64::from(g.slope_ppm),
                u64::from(g.moisture_ppm),
                u64::from(g.sediment_ppm),
                u64::from(g.flood_risk_ppm),
                u64::from(g.scree_mm),
                g.bedrock as u64,
                s.shelter as u64,
                s.burial_candidate as u64,
                h.livelihood as u64,
                u64::from(h.founded),
                u64::from(h.peak_year),
                u64::from(h.peak_households),
                u64::from(h.households),
                u64::from(h.abandoned.unwrap_or(0)),
                u64::from(h.flood_year.unwrap_or(0)),
                u64::from(h.rockfall_year.unwrap_or(0)),
                u64::from(h.refugees_received),
                h.burial as u64,
                h.rock as u64,
                h.cap_rock as u64,
                h.neighbor.map_or(0, |n| n as u64 + 1),
            ] {
                word(v);
            }
        }
        word(self.events.len() as u64);
        for e in &self.events {
            for v in [
                e.id,
                u64::from(e.site),
                u64::from(e.year),
                e.cause as u64,
                u64::from(e.peer.unwrap_or(0)),
                u64::from(e.founded),
                u64::from(e.households_before),
                u64::from(e.households_after),
                e.stores_before as u64,
                e.stores_after as u64,
                e.harvest as u64,
                e.demand as u64,
                e.surplus_discarded as u64,
                e.unmet_demand as u64,
                e.aid_received as u64,
                e.aid_sent as u64,
                u64::from(e.migrants_in),
                u64::from(e.migrants_out),
                u64::from(e.shortage_departures),
                u64::from(e.growth),
            ] {
                word(v);
            }
        }
        digest
    }
}
