//! Encounter record admission. Past the soft cap a new site evicts a record
//! that a fresh visit would recreate bit for bit, so new sites keep working.
use super::*;

/// Hard limit on stored records; saves above it are rejected on load.
pub(super) const RECORD_CEILING: usize = MAX_RECORDS * 4;

impl Adventure {
    /// Makes room for `id`'s record; `nearby` lists the sites in play this tick.
    pub(super) fn admit_record(&mut self, id: IVec3, nearby: &[IVec3]) -> bool {
        if self.records.contains_key(&id) || self.records.len() < MAX_RECORDS {
            return true;
        }
        if let Some(site) = self.untouched_record(id, nearby) {
            self.records.remove(&site);
            return true;
        }
        self.records.len() < RECORD_CEILING
    }

    /// The untouched record farthest from `id`, ties broken by site key.
    /// A first visit stores exactly `PHYSICAL`; any defeat, clear, claim or
    /// quest bit makes the record carry player history.
    fn untouched_record(&self, id: IVec3, nearby: &[IVec3]) -> Option<IVec3> {
        self.records
            .iter()
            .filter(|&(site, &mask)| {
                mask == reliquaries::PHYSICAL
                    && *site != id
                    && !nearby.contains(site)
                    && !self.enemies.iter().any(|e| e.site == *site)
                    && !self.reliquaries.iter().any(|r| r.site == *site)
                    && !self.tollkeepers.contains_key(site)
            })
            .map(|(site, _)| *site)
            .max_by_key(|site| (region_distance(*site, id), site.x, site.y, site.z))
    }
}

/// Site ids are region cells; descent layers add multiples of 1000 to y.
fn region_distance(a: IVec3, b: IVec3) -> i64 {
    let dy = i64::from(a.y) - i64::from(b.y);
    let dy = (-3..=3).map(|layer: i64| (dy - layer * 1_000).abs()).min().unwrap_or(dy.abs());
    (i64::from(a.x) - i64::from(b.x)).abs() + dy
}
