//! Settlement identity and legacy histories. New settlements delegate economic
//! work to persistent communities; their ledgers never write simulated stock.
use super::{World, day, vehicle};
use crate::world::{BlockId as B, chunk_coord_from_world_block};
use engine_worldgen::civilization::{self as geo, EXTENSION_LOTS, Settlement};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) mod politics;

pub(crate) const MAX_SETTLEMENTS: usize = 512;
const BUILD_BATCH: usize = 32;
const REMOTE_LABOR_PER_DAY: usize = 512;
const MAX_REMOTE_LABOR: usize = 100_000;
const MAX_JOURNEYS: usize = 32;
const MIGRATION_METRES_PER_DAY: f32 = 160.0;
const REGIONAL_DISCOVERY_RADIUS: f32 = 550.0;

#[derive(Clone, Copy)]
struct Charter {
    name: &'static str,
    housing_first: bool,
}

const PLAYER_CHARTER: u8 = 12;
const CHARTERS: [Charter; 13] = [
    Charter {
        name: "Alder Commons",
        housing_first: true,
    },
    Charter {
        name: "Red Quarry League",
        housing_first: false,
    },
    Charter {
        name: "Reedwater Council",
        housing_first: true,
    },
    Charter {
        name: "Brazen Guild",
        housing_first: false,
    },
    Charter {
        name: "Ashbank Compact",
        housing_first: true,
    },
    Charter {
        name: "Grey Anvil Union",
        housing_first: false,
    },
    Charter {
        name: "High Pasture Assembly",
        housing_first: true,
    },
    Charter {
        name: "Kilnward League",
        housing_first: false,
    },
    Charter {
        name: "White Willow Commons",
        housing_first: true,
    },
    Charter {
        name: "Stoneway Guild",
        housing_first: false,
    },
    Charter {
        name: "Lantern Council",
        housing_first: true,
    },
    Charter {
        name: "Copper Reach Union",
        housing_first: false,
    },
    Charter {
        name: "Player Charter",
        housing_first: true,
    },
];

fn regional_charter(id: u32) -> u8 {
    // Decode the coarse geography encoded in settlement IDs, then share one
    // charter across each four-by-four group of planning regions.
    let x = ((id >> 11) & 127) / 4;
    let z = ((id >> 4) & 127) / 4;
    (engine_core::hash::hash_u32(x | (z << 5)) % u32::from(PLAYER_CHARTER)) as u8
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Journey {
    resident: u32,
    /// A destination and earliest arrival day. Without a route, the resident waits in transit.
    route: Option<(u32, u64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Lot {
    plot: [i32; 3],
    workshop: bool,
    cursor: usize,
    finished: bool,
    blocked: bool,
    resident: Option<u32>,
    #[serde(default)]
    remote_labor: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Ledger {
    #[serde(default)]
    physical: bool,
    #[serde(default)]
    residents: BTreeSet<u32>,
    #[serde(default)]
    charter: Option<u8>,
    last_day: u64,
    food: i32,
    materials: i32,
    base: u32,
    next_resident: u8,
    lots: BTreeMap<u8, Lot>,
    migrants: u32,
    #[serde(default)]
    journeys: Vec<Journey>,
    #[serde(default)]
    founder: Option<u32>,
}

impl Ledger {
    fn new(site: Settlement, now: u64) -> Self {
        Self {
            physical: site.history.households > 0,
            residents: (site.id..site.id + site.history.households).collect(),
            charter: Some(regional_charter(site.id)),
            last_day: now,
            food: site.history.households as i32 * 2 + 8,
            materials: 9,
            base: site.history.households,
            next_resident: 0,
            lots: BTreeMap::new(),
            migrants: 0,
            journeys: Vec::new(),
            founder: None,
        }
    }

    fn charter_id(&self, site: u32) -> u8 {
        self.charter.unwrap_or_else(|| regional_charter(site))
    }

    fn population(&self) -> u32 {
        if self.physical && !self.residents.is_empty() { return self.residents.len() as u32; }
        self.base
            + u32::from(self.founder.is_some())
            + self
                .lots
                .values()
                .filter(|lot| lot.resident.is_some())
                .count() as u32
    }

    fn worker(&self, world: &World, id: u32, date: u64) -> Option<u32> {
        self.identities(id)
            .filter(|&resident| {
                available(world, resident) && world.travel.away.get(&resident) != Some(&date)
            })
            .min_by_key(|&resident| {
                (
                    engine_worldgen::households::role(resident) != Some(1),
                    resident,
                )
            })
    }

    fn vacancy(&mut self) -> Option<&mut Lot> {
        self.lots
            .values_mut()
            .find(|lot| lot.finished && !lot.workshop && lot.resident.is_none())
    }

    pub(super) fn valid(&self) -> bool {
        self.base <= 6
            && self.residents.len() <= crate::world::farming::jobs::crew::CAPACITY
            && self.residents.iter().copied().all(geo::resident_id)
            && self.founder.is_none_or(geo::resident_id)
            && self.charter.is_none_or(|id| (id as usize) < CHARTERS.len())
            && self.food.abs_diff(0) <= 128
            && (0..=64).contains(&self.materials)
            && self.lots.len() <= EXTENSION_LOTS.into()
            && self.journeys.len() <= MAX_JOURNEYS
            && self.journeys.iter().all(|journey| {
                geo::resident_id(journey.resident)
                    && journey
                        .route
                        .is_none_or(|(target, _)| geo::original_settlement(target) == Some(target))
            })
            && self.lots.iter().all(|(&i, lot)| {
                i < EXTENSION_LOTS
                    && lot.cursor <= 100_000
                    && lot.remote_labor <= MAX_REMOTE_LABOR
                    && lot.plot.iter().all(|value| value.abs_diff(0) < 2_000_000)
                    && lot
                        .resident
                        .is_none_or(|id| lot.finished && !lot.workshop && geo::resident_id(id))
            })
    }

    fn identities(&self, site: u32) -> impl Iterator<Item = u32> + '_ {
        let physical = self.physical && !self.residents.is_empty();
        self.residents.iter().copied().filter(move |_| physical)
            .chain((site..site + self.base).filter(move |_| !physical))
            .chain(self.founder.filter(|_| !physical))
            .chain(self.lots.values().filter_map(move |lot| (!physical).then_some(lot.resident).flatten()))
    }
}

fn available(world: &World, resident: u32) -> bool {
    present(world, resident) && world.travel.following != Some(resident)
        && !world.travel.homes.contains_key(&resident)
        && !world.travel.guard_posts.contains_key(&resident)
        && !crate::world::farming::jobs::assigned(world, resident)
        && !world.travel.politics.mobilized(resident)
}

fn stocks(world: &World, site: u32, ledger: &Ledger) -> (i32, i32) {
    if ledger.physical {
        let (food, timber) = crate::world::farming::jobs::communities::stocks(world, site);
        (food.min(i32::MAX as u32) as i32, timber.min(i32::MAX as u32) as i32)
    } else { (ledger.food, ledger.materials) }
}

pub(crate) fn physical_settlement(world: &World, resident: u32) -> Option<u32> {
    let site = geo::original_settlement(resident)?;
    world.travel.settlements.get(&site).filter(|l| l.physical).map(|_| site)
}

fn spend(world: &mut World, site: u32, food: u32, timber: u32) -> bool {
    let Some(ledger) = world.travel.settlements.get(&site) else { return false; };
    if ledger.physical { return crate::world::farming::jobs::communities::withdraw(world, site, food, timber); }
    let ledger = world.travel.settlements.get_mut(&site).unwrap();
    if ledger.food < food as i32 || ledger.materials < timber as i32 { return false; }
    ledger.food -= food as i32;
    ledger.materials -= timber as i32;
    true
}

pub(crate) fn found_community(world: &mut World, source: u32, site: Settlement, founder: u32) {
    let charter = world.travel.settlements.get(&source).map(|l|l.charter_id(source));
    if let Some(ids) = crate::world::farming::jobs::communities::resident_ids(world,source)
        && let Some(old) = world.travel.settlements.get_mut(&source) {
        old.residents = ids.into_iter().collect();
        if old.founder.is_some_and(|id| !old.residents.contains(&id)) { old.founder = None; }
        if old.residents.is_empty() { old.base = 0; old.founder = None; }
    }
    let mut ledger = Ledger::new(site,day(world));
    ledger.physical = true;
    ledger.charter = charter;
    ledger.base = 0;
    ledger.founder = Some(founder);
    ledger.residents.insert(founder);
    ledger.food = 0;
    ledger.materials = 0;
    world.travel.settlements.insert(site.id,ledger);
}

pub(crate) fn moved_resident(world: &mut World, source: u32, target: u32) {
    for site in [source,target] {
        if let Some(ids) = crate::world::farming::jobs::communities::resident_ids(world,site)
            && let Some(ledger) = world.travel.settlements.get_mut(&site) {
            ledger.residents = ids.into_iter().collect();
            if ledger.founder.is_some_and(|id| !ledger.residents.contains(&id)) { ledger.founder = None; }
            if ledger.residents.is_empty() { ledger.base = 0; }
            if site == target { ledger.migrants = ledger.migrants.saturating_add(1); }
        }
    }
}

pub(crate) fn resident_died(world: &mut World, site: u32, id: u32) {
    if let Some(ledger) = world.travel.settlements.get_mut(&site).filter(|l|l.physical) {
        if ledger.residents.is_empty() { ledger.residents.extend(site..site + ledger.base); }
        ledger.residents.remove(&id);
        if ledger.founder == Some(id) { ledger.founder = None; }
        if ledger.residents.is_empty() { ledger.base = 0; }
    }
}

pub(super) fn valid_ownership(records: &BTreeMap<u32, Ledger>) -> bool {
    let mut residents = std::collections::BTreeSet::new();
    records.iter().all(|(&id, ledger)| {
        ledger.identities(id)
            .chain(ledger.journeys.iter().map(|journey| journey.resident))
            .all(|resident| residents.insert(resident))
            && ledger.journeys.iter().all(|journey| {
                journey
                    .route
                    .is_none_or(|(target, _)| target != id && records.contains_key(&target))
            })
    })
}

fn enabled(world: &World) -> bool {
    #[cfg(feature = "multiplayer")]
    if world.multiplayer.is_some() {
        return false;
    }
    geo::enabled(&world.sample_config()) && world.ca_view.is_none()
}

fn door(world: &World, plot: IVec3, key: u8) -> Vec3 {
    let scale = world.voxel_scale.meters();
    let offset = if key & 1 == 0 { -4.0 } else { 4.0 };
    let z = plot.z + (offset / scale) as i32;
    let y = world.terrain_height_at(plot.x, z) + 1;
    (IVec3::new(plot.x, y, z).as_vec3() + Vec3::new(0.5, 0.02 / scale, 0.5)) * scale
}

fn house(world: &World, lot: &Lot) -> Vec3 {
    let scale = world.voxel_scale.meters();
    (IVec3::from_array(lot.plot).as_vec3() + Vec3::new(0.5, 0.02 / scale, 0.5)) * scale
}

pub(crate) fn home(world: &World, id: u32) -> Option<Vec3> {
    if let Some(home) = crate::world::farming::jobs::communities::home(world,id) { return Some(home); }
    if !geo::resident_id(id) {
        return None;
    }
    if let Some((&site, _)) = world
        .travel
        .settlements
        .iter()
        .find(|(_, ledger)| ledger.founder == Some(id))
    {
        return geo::settlement_by_id(&world.sample_config(), site).map(|site| site.center);
    }
    world
        .travel
        .settlements
        .values()
        .flat_map(|site| site.lots.values())
        .find(|lot| lot.resident == Some(id))
        .map(|lot| house(world, lot))
}

pub(crate) fn present(world: &World, id: u32) -> bool {
    if !world.sample_config().civilization_enabled() { return false; }
    if let Some(alive) = crate::world::farming::jobs::communities::present(world, id) {
        return alive && (!world.travel.politics.mobilized(id) || world.travel.politics.physical_march(id));
    }
    if world.travel.politics.mobilized(id) {
        return false;
    }
    if !geo::resident_id(id) {
        return true;
    }
    if home(world, id).is_some() {
        return true;
    }
    geo::original_settlement(id).is_some_and(|site| {
        world
            .travel
            .settlements
            .get(&site)
            .is_none_or(|ledger| id - site < ledger.base)
    })
}

pub(crate) fn residents(world: &World, feet: Vec3) -> Vec<u32> {
    if !world.sample_config().civilization_enabled() { return Vec::new(); }
    let founders = world.travel.settlements.iter().filter_map(|(&id, ledger)| {
        let founder = ledger.founder?;
        let site = geo::settlement_by_id(&world.sample_config(), id)?;
        (site.center.distance_squared(feet) < 100.0 * 100.0).then_some(founder)
    });
    world
        .travel
        .settlements
        .values()
        .flat_map(|site| site.lots.values())
        .filter(|lot| house(world, lot).distance_squared(feet) < 100.0 * 100.0)
        .filter_map(|lot| lot.resident)
        .chain(founders)
        .collect()
}

pub(crate) fn goal(world: &World, id: u32, position: Vec3) -> Option<Vec3> {
    if !geo::resident_id(id) {
        return None;
    }
    let cfg = world.sample_config();
    for (&site_id, state) in &world.travel.settlements {
        if state.physical { continue; }
        if let Some((&key, lot)) = state.lots.iter().find(|(_, lot)| can_build(world, lot))
            && lot.remote_labor == 0
            && state.worker(world, site_id, day(world)) == Some(id)
        {
            let target = door(world, IVec3::from_array(lot.plot), key);
            if let Some(site) = geo::settlement_by_id(&cfg, site_id)
                && (position.x - site.center.x).abs() < 19.0
                && (4.0..15.0).contains(&(position.z - site.center.z).abs())
                && let Some(next) = geo::daily_destination(&cfg, id, position, false)
            {
                return Some(next);
            }
            return Some(target);
        }
    }
    home(world, id)
}

pub(crate) fn work_pose(
    world: &World,
    id: u32,
    position: Vec3,
) -> Option<crate::world::farming::jobs::WorkPose> {
    if !geo::resident_id(id) {
        return None;
    }
    for (&site, state) in &world.travel.settlements {
        if state.physical { continue; }
        let Some((&key, lot)) = state.lots.iter().find(|(_, lot)| can_build(world, lot)) else {
            continue;
        };
        if lot.cursor > 0
            && lot.remote_labor == 0
            && state.worker(world, site, day(world)) == Some(id)
            && position.distance_squared(door(world, IVec3::from_array(lot.plot), key)) <= 9.0
        {
            return Some(crate::world::farming::jobs::WorkPose::Place {
                target: house(world, lot),
                elapsed: (world.farming.clock % 120) as f32 / 120.0,
            });
        }
    }
    None
}

fn advance(world: &World, records: &mut BTreeMap<u32, Ledger>, now: u64, nearby: &[Settlement]) {
    let cfg = world.sample_config();
    let mut advanced = false;
    for (&id, ledger) in records.iter_mut() {
        ledger.physical |= crate::world::farming::jobs::communities::population(world, id).is_some();
        if ledger.physical {
            // Transfer/death commits update this view. Only hydrate old saves;
            // rebuilding every resident set every second scales with population.
            if ledger.residents.is_empty() && ledger.base > 0
                && let Some(ids) = crate::world::farming::jobs::communities::resident_ids(world,id) {
                ledger.residents = ids.into_iter().collect();
                if ledger.residents.is_empty() { ledger.base = 0; ledger.founder = None; }
            }
            continue;
        }
        let elapsed = now.saturating_sub(ledger.last_day).min(32);
        if elapsed == 0 {
            continue;
        }
        ledger.charter.get_or_insert_with(|| regional_charter(id));
        let Some(site) = geo::settlement_by_id(&cfg, id) else {
            continue;
        };
        advanced = true;
        let distant = !nearby.iter().any(|local| local.id == id);
        let through = ledger.last_day + elapsed;
        for date in ledger.last_day + 1..=through {
            let population = ledger.population() as i32;
            if population == 0 {
                break;
            }
            let weather = engine_core::hash::hash_u32(cfg.seed ^ id ^ date as u32) % 100;
            let crop_loss = if weather < (site.flood_risk * 28.0) as u32 {
                14
            } else if weather >= 83 {
                10
            } else {
                0
            };
            let shops = ledger
                .lots
                .values()
                .filter(|lot| lot.finished && lot.workshop)
                .count() as i32;
            ledger.food = (ledger.food
                + (site.harvest + population / 2 + shops * 2 - crop_loss).max(0)
                - population * 2)
                .clamp(-64, 64);
            ledger.materials = (ledger.materials + 2 + shops).min(64);
            if ledger.food < -4 && ledger.journeys.len() < MAX_JOURNEYS {
                let resident = ledger
                    .lots
                    .values_mut()
                    .rev()
                    .find_map(|lot| {
                        lot.resident
                            .filter(|&resident| available(world, resident))?;
                        lot.resident.take()
                    })
                    .or_else(|| {
                        if ledger.base == 0 || !available(world, id + ledger.base - 1) {
                            return None;
                        }
                        ledger.base -= 1;
                        Some(id + ledger.base)
                    });
                if let Some(resident) = resident {
                    ledger.journeys.push(Journey {
                        resident,
                        route: None,
                    });
                    ledger.food += 4;
                }
            } else if ledger.food >= 16 && ledger.next_resident < 128 {
                let resident = geo::extension_resident(id, ledger.next_resident);
                if let Some(lot) = ledger.vacancy() {
                    lot.resident = Some(resident);
                    ledger.next_resident += 1;
                    ledger.food -= 6;
                }
            }
            if distant && ledger.worker(world, id, date).is_some() {
                start_project(world, site, ledger, None);
                if let Some(lot) = ledger.lots.values_mut().find(|lot| can_build(world, lot)) {
                    lot.remote_labor =
                        (lot.remote_labor + REMOTE_LABOR_PER_DAY).min(MAX_REMOTE_LABOR);
                }
            }
        }
        ledger.last_day = through;
    }
    if advanced {
        advance_journeys(&cfg, records, now);
    }
}

fn advance_journeys(
    cfg: &engine_worldgen::WorldSampleConfig,
    records: &mut BTreeMap<u32, Ledger>,
    now: u64,
) {
    // Reserve a planning slot for each trip already in flight. Admission still
    // checks the real stock and housing on arrival.
    let mut openings: BTreeMap<_, _> = records
        .iter()
        .filter_map(|(&id, ledger)| {
            if ledger.physical { return None; }
            let homes = ledger
                .lots
                .values()
                .filter(|lot| lot.finished && !lot.workshop && lot.resident.is_none())
                .count();
            let affordable = ((ledger.food - 8).max(0) / 4) as usize;
            (homes != 0 && affordable != 0).then_some((id, homes.min(affordable)))
        })
        .collect();
    for (target, _) in records
        .values()
        .flat_map(|ledger| ledger.journeys.iter().filter_map(|journey| journey.route))
    {
        if let Some(room) = openings.get_mut(&target) {
            *room = room.saturating_sub(1);
        }
    }
    let sources: Vec<_> = records
        .iter()
        .filter_map(|(&id, ledger)| (!ledger.physical && !ledger.journeys.is_empty()).then_some(id))
        .collect();
    for id in sources {
        let Some(source) = geo::settlement_by_id(cfg, id) else {
            continue;
        };
        let ledger = records.get_mut(&id).unwrap();
        let charter = ledger.charter_id(id);
        let journeys = std::mem::take(&mut ledger.journeys);
        let mut candidates = Vec::new();
        if journeys.iter().any(|journey| journey.route.is_none()) {
            candidates.extend(openings.iter().filter_map(|(&target, &room)| {
                if target == id || room == 0 {
                    return None;
                }
                let site = geo::settlement_by_id(cfg, target)?;
                Some((source.center.distance(site.center), site))
            }));
            candidates.sort_by(|a, b| {
                let foreign = |site: Settlement| records[&site.id].charter_id(site.id) != charter;
                foreign(a.1)
                    .cmp(&foreign(b.1))
                    .then_with(|| a.0.total_cmp(&b.0))
                    .then(a.1.id.cmp(&b.1.id))
            });
        }
        let mut routes = candidates
            .into_iter()
            .filter(|&(_, site)| geo::linked(cfg, source, site))
            .peekable();
        let mut waiting = Vec::with_capacity(journeys.len());
        for mut journey in journeys {
            if let Some((target, arrival)) = journey.route {
                if now < arrival {
                    waiting.push(journey);
                    continue;
                }
                if let Some(ledger) = records.get_mut(&target)
                    && ledger.food >= 12
                    && let Some(lot) = ledger.vacancy()
                {
                    lot.resident = Some(journey.resident);
                    ledger.food -= 4;
                    ledger.migrants = ledger.migrants.saturating_add(1);
                    continue;
                }
                // Re-evaluate housing from the original route hub on the next day.
                journey.route = None;
            } else {
                if let Some(&(distance, target)) = routes.peek() {
                    let days = (distance / MIGRATION_METRES_PER_DAY).ceil().max(1.0) as u64;
                    journey.route = Some((target.id, now.saturating_add(days)));
                    let room = openings.get_mut(&target.id).unwrap();
                    *room -= 1;
                    if *room == 0 {
                        routes.next();
                    }
                }
            }
            waiting.push(journey);
        }
        records.get_mut(&id).unwrap().journeys = waiting;
    }
}

fn blueprint_batch(
    world: &World,
    site: Settlement,
    key: u8,
    lot: &Lot,
    limit: usize,
) -> (usize, Vec<(IVec3, B)>) {
    let cfg = world.sample_config();
    let scale = cfg.voxel_meters();
    let radius = cfg.blocks_for_span(3.0);
    let wall_height = cfg.blocks_for_span(3.0);
    let foundation = cfg.blocks_for_span(3.0);
    let origin = IVec3::from_array(lot.plot);
    let material = match site.history.rock {
        B::Limestone | B::Sandstone | B::Basalt => site.history.rock,
        _ => B::Cobblestone,
    };
    let mut out = Vec::with_capacity(limit);
    let end = lot.cursor + limit;
    let entrance = if key & 1 == 0 { -1 } else { 1 };
    let stair_run = cfg.blocks_for_span(2.0);
    let stair_half_width = cfg.blocks_for_span(1.0);
    let mut prefix = 0;
    for step in 1..=stair_run {
        let rise = (step + 1) / 2;
        let depth = (foundation - rise).max(0) as usize;
        let next = prefix + (2 * stair_half_width) as usize * depth;
        for index in lot.cursor.max(prefix)..end.min(next) {
            let offset = index - prefix;
            let x = (offset / depth) as i32 - stair_half_width;
            let y = (offset % depth) as i32 - foundation;
            out.push((
                origin + IVec3::new(x, y, entrance * (radius + step)),
                material,
            ));
        }
        prefix = next;
    }
    // Preserve the saved cursor order: stairs first, then the shell in y/z/x order.
    let width = (2 * radius + 1) as usize;
    let layer = width * width;
    let total = prefix + layer * (foundation + wall_height + 1) as usize;
    for index in lot.cursor.max(prefix)..end.min(total) {
        let offset = index - prefix;
        let x = (offset % width) as i32 - radius;
        let z = (offset / width % width) as i32 - radius;
        let y = (offset / layer) as i32 - foundation;
        let doorway = (x as f32 * scale).abs() < 1.0 && y < stair_run && z == entrance * radius;
        let edge = x.abs().max(z.abs()) == radius;
        let block = if y < 0 || (edge && !doorway && y < wall_height) {
            material
        } else if y == wall_height {
            B::Planks
        } else if lot.workshop && x == radius - 1 && y == 0 && z == 0 {
            B::Workbench
        } else {
            B::Air
        };
        out.push((origin + IVec3::new(x, y, z), block));
    }
    (total, out)
}

fn protected_plot(world: &World, plot: IVec3) -> bool {
    let cfg = world.sample_config();
    let shell = cfg.blocks_for_span(3.0);
    let stairs = cfg.blocks_for_span(2.0);
    let reach = IVec3::new(shell, shell, shell + stairs);
    crate::world::farming::base::overlaps(world, plot - reach, plot + reach)
}

fn can_build(world: &World, lot: &Lot) -> bool {
    !lot.finished && !lot.blocked && !protected_plot(world, IVec3::from_array(lot.plot))
}

fn start_project(world: &World, site: Settlement, state: &mut Ledger, viewer: Option<Vec3>) {
    if !state.lots.values().any(|lot| can_build(world, lot)) {
        if state.food < 10 || state.materials < 8 {
            return;
        }
        let spare_home = state.vacancy().is_some();
        let charter = CHARTERS[state.charter_id(site.id) as usize];
        let added_housing = state.lots.values().any(|lot| lot.finished && !lot.workshop);
        let workshop = state.population() >= 4
            && (!charter.housing_first || added_housing)
            && !state.lots.values().any(|lot| lot.workshop && !lot.blocked);
        if !workshop && spare_home {
            return;
        }
        for key in 0..EXTENSION_LOTS {
            if state.lots.contains_key(&key) {
                continue;
            }
            let Some(plot) = geo::extension_plot(&world.sample_config(), site, key) else {
                continue;
            };
            if protected_plot(world, plot) {
                continue;
            }
            if viewer
                .is_some_and(|feet| door(world, plot, key).distance_squared(feet) > 70.0 * 70.0)
            {
                continue;
            }
            state.lots.insert(
                key,
                Lot {
                    plot: plot.to_array(),
                    workshop,
                    cursor: 0,
                    finished: false,
                    blocked: false,
                    resident: None,
                    remote_labor: 0,
                },
            );
            state.materials -= 8;
            break;
        }
    }
}

fn build(world: &mut World, site: Settlement, state: &mut Ledger, feet: Vec3) {
    if state.physical { return; }
    let worker = state.worker(world, site.id, day(world));
    if worker.is_some() {
        start_project(world, site, state, Some(feet));
    }
    let Some((&key, lot)) = state.lots.iter_mut().find(|(_, lot)| can_build(world, lot)) else {
        return;
    };
    let remote = lot.remote_labor > 0;
    if !remote
        && worker
            .and_then(|id| vehicle::villager_actor(world, id))
            .is_none_or(|actor| {
                vehicle::seeking_safety(actor)
                    || actor
                        .pos
                        .distance_squared(door(world, IVec3::from_array(lot.plot), key))
                        > 3.0 * 3.0
            })
    {
        return;
    }
    let cfg = world.sample_config();
    let mut edits = Vec::new();
    let mut admitted = Vec::new();
    let batch = if remote {
        BUILD_BATCH.min(lot.remote_labor)
    } else {
        BUILD_BATCH
    };
    let (total, plan) = blueprint_batch(world, site, key, lot, batch);
    let scale = world.voxel_scale.meters();
    for (p, material) in plan {
        if !world.chunks.contains_key(&chunk_coord_from_world_block(p)) {
            break;
        }
        let actual = world.get(p);
        if actual == material || (p.y < lot.plot[1] && actual.is_solid()) {
            admitted.push((p, actual));
        } else {
            let min = p.as_vec3() * scale;
            let max = min + Vec3::splat(scale);
            let overlaps = |at: Vec3| {
                (at - Vec3::new(0.4, 0.0, 0.4)).cmplt(max).all()
                    && (at + Vec3::new(0.4, 1.9, 0.4)).cmpgt(min).all()
            };
            if remote
                && material.is_solid()
                && (overlaps(feet)
                    || world
                        .adventure
                        .enemies
                        .iter()
                        .any(|actor| actor.health > 0.0 && overlaps(actor.pos)))
            {
                break;
            }
            let generated = engine_worldgen::sample_block(&cfg, p);
            let removable =
                p.y < lot.plot[1] || matches!(actual, B::Air | B::Grass | B::Leaves | B::Bush);
            if actual != generated || !removable {
                lot.blocked = true;
                break;
            }
            admitted.push((p, material));
            edits.push((p, material));
        }
    }
    if !edits.is_empty() {
        world.set_many(&edits);
    }
    let completed = admitted
        .into_iter()
        .take_while(|&(p, material)| world.get(p) == material)
        .count();
    lot.cursor += completed;
    if remote {
        lot.remote_labor -= completed;
    }
    lot.finished = lot.cursor == total;
    if lot.finished || lot.blocked {
        lot.remote_labor = 0;
    }
}

fn survey_origin(n:u32) -> Vec3 {
    if n==0 {return Vec3::ZERO;}
    let ring=(((f64::from(n)+1.0).sqrt()-1.0)*0.5).ceil() as i64;
    let side=2*ring;
    let offset=(side+1)*(side+1)-1-i64::from(n);
    let (x,z)=match offset/side {
        0=>(ring-offset, -ring),
        1=>(-ring, -ring+offset-side),
        2=>(-ring+offset-2*side,ring),
        _=>(ring,ring-(offset-3*side)),
    };
    Vec3::new(x as f32*768.0,0.0,z as f32*768.0)
}

pub(crate) fn service(world: &mut World, feet: Vec3) {
    if !enabled(world) {
        return;
    }
    let cfg = world.sample_config();
    let now = day(world);
    // Historical admission follows a saved geography scan, never a camera visit.
    // Survey preparation is asynchronous; a pending survey cannot stop existing towns.
    let nearby = if geo::prepare_settlements_near(&cfg, feet, 110.0) {
        geo::settlements_near(&cfg,feet,110.0)
    } else {Vec::new()};
    let mut records = std::mem::take(&mut world.travel.settlements);
    let origin=survey_origin(world.travel.civilization_scan);
    if records.len()<MAX_SETTLEMENTS && geo::prepare_settlements_near(&cfg,origin,REGIONAL_DISCOVERY_RADIUS) {
        let mut region=geo::settlements_near(&cfg,origin,REGIONAL_DISCOVERY_RADIUS);
        region.sort_by_key(|site|site.id);
        for site in region
            .into_iter()
            .filter(|site| site.history.households > 0)
        {
            if records.len() >= MAX_SETTLEMENTS {
                break;
            }
            records
                .entry(site.id)
                .or_insert_with(|| Ledger::new(site, now));
        }
        world.travel.civilization_scan=world.travel.civilization_scan.wrapping_add(1);
    }
    advance(world, &mut records, now, &nearby);
    for site in nearby {
        if let Some(state) = records.get_mut(&site.id) {
            build(world, site, state, feet);
        }
    }
    world.travel.settlements = records;
    // Admission is bounded. An existing legacy ledger retains its old owner;
    // a newly initialized settlement is fenced before any physical work starts.
    let pending = world.travel.settlements.iter()
        .filter(|(_, ledger)| ledger.physical)
        .filter_map(|(&id, _)| (crate::world::farming::jobs::communities::population(world, id).is_none()).then_some(id))
        .next();
    if let Some(site) = pending.and_then(|id| geo::settlement_by_id(&cfg, id)) {
        crate::world::farming::jobs::communities::register(world, site);
    }
    politics::service(world, feet);
}

pub(crate) struct Summary {
    pub charter: &'static str,
    pub settlement: &'static str,
    pub priority: &'static str,
    pub households: u32,
    pub vacant: usize,
    pub arrivals: u32,
    pub in_transit: usize,
    pub awaiting_housing: usize,
    pub project: Option<&'static str>,
}

pub(crate) fn summary(world: &World, id: u32) -> Option<Summary> {
    if !enabled(world) { return None; }
    let site = crate::world::farming::jobs::communities::owner_site(world,id).or_else(||geo::original_settlement(id));
    let (site, state) = world
        .travel
        .settlements
        .iter()
        .find(|(_, ledger)| {
            ledger.founder == Some(id) || ledger.lots.values().any(|lot| lot.resident == Some(id))
        })
        .map(|(&id, ledger)| (id, ledger))
        .or_else(|| {
            let site = site?;
            Some((site, world.travel.settlements.get(&site)?))
        })?;
    let in_transit = state
        .journeys
        .iter()
        .filter(|trip| trip.route.is_some())
        .count();
    let population = crate::world::farming::jobs::communities::population(world, site)
        .map_or_else(|| state.population(), |n| n as u32);
    Some(Summary {
        charter: CHARTERS[state.charter_id(site) as usize].name,
        settlement: match population {
            0 => "Abandoned",
            1..=3 => "Outpost",
            8.. if state.lots.values().any(|lot| lot.finished && lot.workshop) => "Town",
            _ => "Village",
        },
        priority: if state.physical { "Resident livelihoods" } else if CHARTERS[state.charter_id(site) as usize].housing_first {
            "Housing first"
        } else {
            "Workshops first"
        },
        households: population,
        vacant: state
            .lots
            .values()
            .filter(|lot| lot.finished && !lot.workshop && lot.resident.is_none())
            .count(),
        arrivals: state.migrants,
        in_transit,
        awaiting_housing: state.journeys.len() - in_transit,
        project: state
            .lots
            .values()
            .find(|lot| !lot.finished && !lot.blocked)
            .map(|lot| {
                if lot.workshop {
                    "Workshop under construction"
                } else {
                    "Home under construction"
                }
            }),
    })
}

pub(crate) fn dev_factions(world: &World) -> String {
    let mut counts = [(0, 0); CHARTERS.len()];
    for (&id, state) in &world.travel.settlements {
        let (sites, households) = &mut counts[state.charter_id(id) as usize];
        *sites += 1;
        *households += state.population();
    }
    let rows: Vec<_> = CHARTERS
        .iter()
        .zip(counts)
        .filter_map(|(charter, (sites, households))| {
            (sites != 0).then(|| {
                format!(
                    "{} · {sites} holdings · {households} households",
                    charter.name
                )
            })
        })
        .collect();
    if rows.is_empty() {
        "No active settlement records".to_owned()
    } else {
        rows.join(" | ")
    }
}
