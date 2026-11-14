use super::{Cause, Event, MAX_EVENTS, STEP_YEARS, YEARS};
use crate::civilization::{Burial, LOTS, Region, WorldSampleConfig, encode, hash};
use glam::IVec2;

pub(in crate::civilization) fn run(
    cfg: &WorldSampleConfig,
    key: IVec2,
    sites: &mut Region,
    linked: bool,
) -> Vec<Event> {
    let mut events = Vec::with_capacity(MAX_EVENTS);
    let mut stores = [0; 2];
    let ids = [encode(key, 0, 0), encode(key, 1, 0)];
    for step in 1..=YEARS / STEP_YEARS {
        let year = step * STEP_YEARS;
        // One regional exposure, independent of site enumeration and voxel size.
        let weather = hash(cfg, key, 0x6873_0000 + step) % 10_000;
        let mut current: [Option<Event>; 2] = [None, None];
        for (i, site) in sites.iter_mut().enumerate() {
            let Some(site) = site else { continue };
            let h = &mut site.history;
            if year < h.founded || h.abandoned.is_some() {
                continue;
            }
            let founded = year == h.founded;
            let households_before = h.households;
            let stores_before = stores[i];
            if founded {
                h.households = 2;
                stores[i] = 8;
            }
            let flood = weather < site.landscape.flood_risk_ppm * 28 / 10_000;
            let slide = !flood && matches!(h.burial, Burial::Scree) && weather < 500;
            let cause = if flood {
                Cause::Flood
            } else if slide {
                Cause::Rockfall
            } else if weather >= 7600 {
                Cause::Drought
            } else {
                Cause::Ordinary
            };
            let penalty = match cause {
                Cause::Flood | Cause::Rockfall => 14,
                Cause::Drought => 9,
                Cause::Ordinary => 0,
            };
            let harvest = (h.livelihood.harvest() + site.shelter - penalty).max(0);
            let demand = h.households as i32 * 3;
            let balance = stores[i] + harvest - demand;
            stores[i] = balance.min(30);
            if flood {
                h.flood_year = Some(year);
            }
            if slide {
                h.rockfall_year = Some(year);
            }
            current[i] = Some(Event {
                id: (u64::from(ids[i]) << 32) | u64::from(year),
                site: ids[i],
                year,
                cause,
                peer: linked.then_some(ids[1 - i]),
                founded,
                households_before,
                households_after: 0,
                stores_before,
                stores_after: 0,
                harvest,
                demand,
                surplus_discarded: (balance - 30).max(0),
                unmet_demand: 0,
                aid_received: 0,
                aid_sent: 0,
                migrants_in: 0,
                migrants_out: 0,
                shortage_departures: 0,
                growth: 0,
            });
        }

        if linked {
            let [left, right] = sites;
            let [left_event, right_event] = &mut current;
            if let (Some(left), Some(right), Some(le), Some(re)) =
                (left, right, left_event, right_event)
            {
                let pair = [&mut left.history, &mut right.history];
                let logs = [le, re];
                for (from, to) in [(0, 1), (1, 0)] {
                    if pair[from].households == 0
                        || pair[to].households == 0
                        || stores[from] >= 0
                        || stores[to] < 9
                    {
                        continue;
                    }
                    let aid = (-stores[from]).min(4);
                    stores[from] += aid;
                    stores[to] -= aid;
                    logs[from].aid_received += aid;
                    logs[to].aid_sent += aid;
                    pair[from].neighbor = Some(pair[to].livelihood);
                    if stores[from] < 0 && pair[to].households < LOTS {
                        pair[from].households -= 1;
                        pair[to].households += 1;
                        pair[to].refugees_received += 1;
                        stores[from] += 3;
                        stores[to] -= 3;
                        logs[from].demand -= 3;
                        logs[to].demand += 3;
                        logs[from].migrants_out += 1;
                        logs[to].migrants_in += 1;
                    }
                }
            }
        }

        for (i, site) in sites.iter_mut().enumerate() {
            let (Some(site), Some(event)) = (site, &mut current[i]) else {
                continue;
            };
            let h = &mut site.history;
            if stores[i] < 0 {
                event.shortage_departures = h.households.min(1 + u32::from(stores[i] < -6));
                h.households -= event.shortage_departures;
                event.unmet_demand = -stores[i];
                stores[i] = 0;
            } else if stores[i] >= 14 && h.households < LOTS {
                h.households += 1;
                stores[i] -= 8;
                event.growth = 1;
            }
            if h.households > h.peak_households {
                h.peak_households = h.households;
                h.peak_year = year;
            }
            if h.households == 0 {
                h.abandoned = Some(year);
            }
            event.households_after = h.households;
            event.stores_after = stores[i];
        }
        events.extend(current.into_iter().flatten());
    }
    for site in sites.iter_mut().flatten() {
        let h = &mut site.history;
        if (matches!(h.burial, Burial::Alluvium) && h.flood_year.is_none())
            || (matches!(h.burial, Burial::Scree) && h.rockfall_year.is_none())
        {
            h.burial = Burial::Bedrock;
        }
    }
    events.shrink_to_fit();
    events
}
