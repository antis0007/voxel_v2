//! One active world's treaties and supplied expeditions.
use super::*;
use crate::world::inventory::Item;
use crate::world::farming::jobs::communities::transit;

const SECOND: u64 = (1.0 / engine_core::time::PLAYER_STEP_SECONDS + 0.5) as u64;
const FOOD: Item = crate::world::farming::pantry::BIOMASS;
const TIMBER: Item = Item::Block(B::Planks);
const SUPPLY: u8 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Relation {
    Pact,
    War,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Campaign {
    pub resident: u32,
    target: u32,
    attacker: u8,
    defender: u8,
    steps: u16,
    length: u16,
    supply: u8,
    pub health: f32,
    next_siege: u64,
    breaches: u8,
    #[serde(default)]
    withdrawing: bool,
    #[serde(default)]
    blocked_at: Option<u16>,
    #[serde(default)]
    travel_owned: bool,
}

impl Campaign {
    pub(crate) fn hostile_to_player(self) -> bool {
        self.attacker != PLAYER_CHARTER && self.defender == PLAYER_CHARTER
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct State {
    #[serde(skip)] people:BTreeMap<u32,(bool,bool)>,
    #[serde(skip)] indexed:bool,
    relations: BTreeMap<(u8, u8), Relation>,
    campaigns: BTreeMap<u32, Campaign>,
    last_counter: u64,
    #[serde(default)]
    grievances: BTreeMap<(u8, u8), u16>,
    #[serde(default)]
    diplomacy_cursor: u32,
}

fn pair(a: u8, b: u8) -> (u8, u8) {
    (a.min(b), a.max(b))
}

impl State {
    pub(crate) fn physical_march(&self, id: u32) -> bool {
        if self.indexed {return self.people.get(&id).is_some_and(|p|p.0);}
        self.campaigns.values().any(|c|c.resident==id && c.travel_owned)
    }
    pub(crate) fn hostile_march(&self,id:u32)->bool {
        if self.indexed {return self.people.get(&id).is_some_and(|p|p.1);}
        self.campaigns.values().any(|c|c.resident==id && c.travel_owned && c.hostile_to_player() && !c.withdrawing)
    }
    pub(crate) fn mobilized(&self, id: u32) -> bool {
        if self.indexed {return self.people.contains_key(&id);}
        self.campaigns.values().any(|force| force.resident == id)
    }

    pub(crate) fn valid(&self, records: &BTreeMap<u32, Ledger>) -> bool {
        let mut people = std::collections::BTreeSet::new();
        self.grievances.iter().all(|(&(a,b),&n)| a < b && b <= PLAYER_CHARTER && n <= 100)
            && self.relations
            .keys()
            .all(|&(a, b)| a < b && b <= PLAYER_CHARTER)
            && self.campaigns.len() <= MAX_SETTLEMENTS
            && self.campaigns.iter().all(|(&source, force)| {
                source != force.target
                    && records.contains_key(&force.target)
                    && records
                        .get(&source)
                        .is_some_and(|ledger| owns(source, ledger, force.resident))
                    && people.insert(force.resident)
                    && force.attacker <= PLAYER_CHARTER
                    && force.defender <= PLAYER_CHARTER
                    && force.attacker != force.defender
                    && (1..=512).contains(&force.length)
                    && force.steps <= force.length
                    && force.supply <= SUPPLY
                    && (0.0..=20.0).contains(&force.health)
                    && force.breaches <= 3
                    && force.blocked_at.is_none_or(|step| step <= force.length)
            })
    }
}

fn owns(site: u32, ledger: &Ledger, id: u32) -> bool {
    ledger.identities(site).any(|person| person == id)
}

pub(crate) fn at_war(world: &World, a: u32, b: u32) -> bool {
    let (Some(a),Some(b)) = (world.travel.settlements.get(&a).map(|l|l.charter_id(a)),
        world.travel.settlements.get(&b).map(|l|l.charter_id(b))) else { return false; };
    world.travel.politics.relations.get(&pair(a,b)) == Some(&Relation::War)
}

pub(crate) fn raid_recorded(world: &mut World, source: u32, target: u32) {
    let (Some(a),Some(b)) = (world.travel.settlements.get(&source).map(|l|l.charter_id(source)),
        world.travel.settlements.get(&target).map(|l|l.charter_id(target))) else { return; };
    if a == b { return; }
    let key = pair(a,b);
    let grievances = world.travel.politics.grievances.entry(key).or_default();
    *grievances = grievances.saturating_add(1).min(100);
    // Incidents create a grievance. War requires a later, provisioned political decision.
}

pub(crate) fn trade_completed(world: &mut World, source: u32, target: u32) {
    let (Some(a),Some(b)) = (world.travel.settlements.get(&source).map(|l|l.charter_id(source)),
        world.travel.settlements.get(&target).map(|l|l.charter_id(target))) else { return; };
    if a == b { return; }
    if let Some(grievances) = world.travel.politics.grievances.get_mut(&pair(a,b)) {
        *grievances = grievances.saturating_sub(1);
    }
}

fn detach(site: u32, ledger: &mut Ledger, id: u32) -> bool {
    if ledger.founder == Some(id) {
        ledger.founder = None;
        return true;
    }
    if let Some(lot) = ledger
        .lots
        .values_mut()
        .find(|lot| lot.resident == Some(id))
    {
        lot.resident = None;
        return true;
    }
    if ledger.base > 0 && id == site + ledger.base - 1 {
        ledger.base -= 1;
        return true;
    }
    false
}

fn nearest(world: &World, feet: Vec3) -> Option<Settlement> {
    geo::settlements_near(&world.sample_config(), feet, 22.0)
        .into_iter()
        .min_by(|a, b| {
            a.center
                .distance_squared(feet)
                .total_cmp(&b.center.distance_squared(feet))
                .then(a.id.cmp(&b.id))
        })
}

fn found(world: &mut World, site: Settlement) -> &'static str {
    let Some(id) = world.travel.following else {
        return "A recruited resident must accompany the founder";
    };
    if !geo::resident_id(id)
        || world.travel.politics.mobilized(id)
        || vehicle::villager_feet(world, id)
            .is_none_or(|at| at.distance_squared(site.center) > 22.0 * 22.0)
    {
        return "A registered resident must reach the outpost site";
    }
    if site.history.households != 0
        || world.travel.settlements.contains_key(&site.id)
        || world.travel.settlements.len() >= MAX_SETTLEMENTS
        || !(0..EXTENSION_LOTS)
            .any(|slot| geo::extension_plot(&world.sample_config(), site, slot).is_some())
    {
        return "An abandoned site with buildable land is required";
    }
    if world.inventory.count_item(FOOD) < 12 || world.inventory.count_item(TIMBER) < 8 {
        return "Founding requires 12 biomass and 8 planks";
    }
    let Some((&source, _)) = world
        .travel
        .settlements
        .iter()
        .find(|(source, ledger)| owns(**source, ledger, id))
    else {
        return "Register the resident's home settlement first";
    };
    if !detach(
        source,
        world.travel.settlements.get_mut(&source).unwrap(),
        id,
    ) {
        return "Recruit an extension household or the last original household";
    }
    world.inventory.remove_item(FOOD, 12);
    world.inventory.remove_item(TIMBER, 8);
    world.travel.following = None;
    world.travel.follow_owner = None;
    world.travel.homes.remove(&id);
    let mut ledger = Ledger::new(site, day(world));
    ledger.charter = Some(PLAYER_CHARTER);
    ledger.founder = Some(id);
    ledger.food = 12;
    ledger.materials = 8;
    world.travel.settlements.insert(site.id, ledger);
    "Outpost founded · first household and building stock registered"
}

fn mobilize(world: &mut World, source: Settlement, target: Settlement) -> &'static str {
    let Some(home) = world.travel.settlements.get(&source.id) else {
        return "Unknown source holding";
    };
    let Some(enemy) = world.travel.settlements.get(&target.id) else {
        return "Unknown target holding";
    };
    let attacker = home.charter_id(source.id);
    let defender = enemy.charter_id(target.id);
    if world
        .travel
        .politics
        .relations
        .get(&pair(attacker, defender))
        != Some(&Relation::War)
        || !geo::linked(&world.sample_config(), source, target)
    {
        return "A reachable holding at war is required";
    }
    let (food, materials) = stocks(world, source.id, home);
    let population = crate::world::farming::jobs::communities::population(world, source.id)
        .map_or_else(|| home.population(), |n| n as u32);
    if world.travel.politics.campaigns.contains_key(&source.id)
        || population < 2
        || food < i32::from(SUPPLY) + 8
        || materials < 4
    {
        return "Mobilization requires two households, 24 food and 4 materials";
    }
    let Some(resident) = home.identities(source.id)
        .find(|&resident| {
            available(world, resident) && world.travel.away.get(&resident) != Some(&day(world))
                && crate::world::farming::jobs::communities::expedition_ready(world, resident)
        })
    else {
        return "No available household for mobilization";
    };
    let health = vehicle::villager_actor(world, resident).map(|a| a.health)
        .or_else(|| crate::world::farming::jobs::crew::body(world, resident).map(vehicle::ResidentBody::health))
        .unwrap_or(20.0).clamp(0.0, 20.0);
    let physical = home.physical;
    if health <= 0.0 || if physical {
        !transit::muster(world,source.id,target.id,resident,SUPPLY,None)
    } else { !spend(world, source.id, u32::from(SUPPLY), 4) } { return "Settlement supplies unavailable"; }
    world.travel.politics.campaigns.insert(
        source.id,
        Campaign {
            resident,
            target: target.id,
            attacker,
            defender,
            steps: 0,
            length: source
                .center
                .distance(target.center)
                .ceil()
                .clamp(1.0, 512.0) as u16,
            supply: if physical {0} else {SUPPLY},
            health,
            next_siege: 0,
            breaches: 0,
            withdrawing: false,
            blocked_at: None,
            travel_owned: physical,
        },
    );
    world.travel.politics.people.insert(resident,(physical,physical && attacker!=PLAYER_CHARTER && defender==PLAYER_CHARTER));
    "Expedition mustered · one household and supplies committed"
}

pub(crate) fn command(world: &mut World, feet: Vec3, args: &str) -> String {
    if !enabled(world) || !world.rules.mode.interacts() {
        return "Realm orders require an active solo Planet".into();
    }
    let cfg = world.sample_config();
    if args.trim() == "reinforce" {
        let Some((&source, _)) = world
            .travel
            .politics
            .campaigns
            .iter()
            .find(|(source, force)| {
                force.attacker == PLAYER_CHARTER
                    && force.health > 0.0
                    && force.supply < SUPPLY
                    && position(&cfg, **source, **force, force.steps)
                        .is_some_and(|at| at.distance_squared(feet) < 6.0 * 6.0)
                    && vehicle::campaign_present(world, force.resident)
            })
        else {
            return "A nearby supplied expedition with room in its pack is required".into();
        };
        let force = world.travel.politics.campaigns.get_mut(&source).unwrap();
        let amount =
            u32::from((SUPPLY - force.supply).min(8)).min(world.inventory.count_item(FOOD));
        if amount == 0 || !world.inventory.remove_item(FOOD, amount) {
            return "No supplies transferred".into();
        }
        force.supply += amount as u8;
        return format!("{amount} rations delivered");
    }
    let Some(site) = nearest(world, feet) else {
        return "No settlement site in reach".into();
    };
    let (order, argument) = args.trim().split_once(' ').unwrap_or((args.trim(), ""));
    if order == "found" {
        return found(world, site).into();
    }
    let Some(ledger) = world.travel.settlements.get(&site.id) else {
        return "Settlement not registered".into();
    };
    let owner = ledger.charter_id(site.id);
    match order {
        "inspect" | "" => {
            let linked: Vec<_> = world.travel.settlements.keys().filter_map(|&id| geo::settlement_by_id(&cfg, id))
                .filter(|target| target.id != site.id && geo::linked(&cfg, site, *target))
                .map(|target| format!("{} ({})", target.id, CHARTERS[world.travel.settlements[&target.id].charter_id(target.id) as usize].name)).collect();
            let relation = match world.travel.politics.relations.get(&pair(owner, PLAYER_CHARTER)) {
                Some(Relation::Pact) => "Pact", Some(Relation::War) => "War", None if owner == PLAYER_CHARTER => "Held", None => "Neutral",
            };
            let expedition = world.travel.politics.campaigns.get(&site.id).map_or_else(String::new, |force| {
                format!(" · Expedition {} · {}/{} m · {} supply · {} health · {} siege progress{}",
                    force.target, force.steps, force.length, force.supply, force.health, force.breaches,
                    if force.blocked_at.is_some() { " · Road blocked" } else if force.withdrawing { " · Returning" } else { "" })
            });
            let (food, materials) = stocks(world, site.id, ledger);
            format!("{} · {} · {relation} · food {} · materials {} · routes {}{expedition}", site.id, CHARTERS[owner as usize].name, food, materials, linked.join(", "))
        }
        "war" | "pact" | "peace" if owner != PLAYER_CHARTER => {
            let key = pair(owner, PLAYER_CHARTER);
            if order == "war" && world.travel.politics.relations.get(&key) == Some(&Relation::Pact) {
                return "End the pact before declaring war".into();
            }
            if order == "pact" && world.travel.politics.relations.get(&key) == Some(&Relation::War) {
                return "Make peace before agreeing a pact".into();
            }
            match order {
                "war" => { world.travel.politics.relations.insert(key, Relation::War); "War declared" }
                "pact" => { world.travel.politics.relations.insert(key, Relation::Pact); "Non-aggression pact" }
                _ => { world.travel.politics.relations.remove(&key); "Neutral relations" }
            }.into()
        }
        "march" if owner == PLAYER_CHARTER => {
            let Some(target) = argument.trim().parse().ok().and_then(|id| geo::settlement_by_id(&cfg, id)) else {
                return "March requires a linked settlement ID".into();
            };
            mobilize(world, site, target).into()
        }
        "supply" if owner == PLAYER_CHARTER => {
            let physical = ledger.physical;
            let (food, materials) = stocks(world, site.id, ledger);
            let (item, stock) = match argument.trim() {
                "food" => (if physical { crate::world::farming::jobs::communities::MEAL } else { FOOD }, food),
                "timber" => (TIMBER, materials),
                _ => return "Supply requires food or timber".into(),
            };
            let amount = (64 - stock).clamp(0, 8) as u32;
            let amount = amount.min(world.inventory.count_item(item));
            if amount == 0 || !world.inventory.remove_item(item, amount) { return "No supplies transferred".into(); }
            if physical {
                let stored = crate::world::farming::jobs::communities::deposit(world, site.id, feet, item, amount);
                if stored < amount { super::super::give(world, feet, item, amount - stored); }
                return format!("{stored} supplies delivered");
            }
            let ledger = world.travel.settlements.get_mut(&site.id).unwrap();
            if item == FOOD { ledger.food += amount as i32; } else { ledger.materials += amount as i32; }
            format!("{amount} supplies transferred")
        }
        "withdraw" if owner == PLAYER_CHARTER => {
            let Some(force) = world.travel.politics.campaigns.get_mut(&site.id) else { return "No expedition from this holding".into() };
            force.withdrawing = true;
            "Expedition recalled".into()
        }
        _ => "Realm orders: inspect, found, pact, peace, war, supply, march <settlement ID>, reinforce, withdraw".into(),
    }
}

fn position(
    cfg: &engine_worldgen::WorldSampleConfig,
    source: u32,
    force: Campaign,
    steps: u16,
) -> Option<Vec3> {
    let from = geo::settlement_by_id(cfg, source)?.center;
    let to = geo::settlement_by_id(cfg, force.target)?.center;
    let point = from.lerp(to, f32::from(steps) / f32::from(force.length));
    let scale = cfg.voxel_meters();
    let cell = (point / scale).floor().as_ivec3();
    let y = (engine_worldgen::terrain_height(cfg, cell.x, cell.z) + 1) as f32 * scale;
    Some(Vec3::new(point.x, y + 0.02, point.z))
}

pub(crate) fn actors(world: &World, viewer: Vec3) -> Vec<(Campaign, Vec3)> {
    if !enabled(world) { return Vec::new(); }
    let cfg = world.sample_config();
    world
        .travel
        .politics
        .campaigns
        .iter()
        .filter_map(|(&source, &force)| {
            if force.travel_owned { return None; }
            let from = geo::settlement_by_id(&cfg, source)?.center;
            let to = geo::settlement_by_id(&cfg, force.target)?.center;
            let approximate = from.lerp(to, f32::from(force.steps) / f32::from(force.length));
            if approximate.distance_squared(viewer) > 72.0 * 72.0 || force.health <= 0.0 {
                return None;
            }
            Some((force, position(&cfg, source, force, force.steps)?))
        })
        .collect()
}

pub(crate) fn wound(world: &mut World, resident: u32, health: f32) {
    if let Some(force) = world
        .travel
        .politics
        .campaigns
        .values_mut()
        .find(|force| force.resident == resident)
    {
        force.health = force.health.min(health.max(0.0));
    }
}

fn physical_campaign(world: &mut World, source: u32, force: &mut Campaign) -> bool {
    if !force.travel_owned {
        let Some(at)=position(&world.sample_config(),source,*force,force.steps) else {return true;};
        if !transit::muster(world,source,force.target,force.resident,force.supply,Some(at)) {return true;}
        force.supply=0; // Ownership moved into the saved individual's pack.
        force.travel_owned=true;
        crate::world::farming::jobs::communities::set_health(world,force.resident,force.health);
    }
    let Some((arrived,returning,food))=transit::campaign_state(world,force.resident) else {return false;};
    force.health=vehicle::villager_actor(world,force.resident).map(|a|a.health)
        .or_else(||crate::world::farming::jobs::crew::body(world,force.resident).map(|b|b.health())).unwrap_or(0.0);
    if force.health<=0.0 {return false;}
    force.withdrawing |= food==0
        || world.travel.settlements.get(&source).is_none_or(|s|s.charter_id(source)!=force.attacker)
        || world.travel.settlements.get(&force.target).is_none_or(|s|s.charter_id(force.target)!=force.defender)
        || world.travel.politics.relations.get(&pair(force.attacker,force.defender))!=Some(&Relation::War);
    if force.withdrawing && !returning {transit::recall(world,source,force.resident);}
    if returning || force.withdrawing || !arrived || world.farming.clock<force.next_siege {return true;}
    force.next_siege=world.farming.clock.saturating_add(8*SECOND);
    match transit::siege(world,source,force.resident,force.target) {
        transit::Siege::Breach=>{
            force.breaches=force.breaches.saturating_add(1).min(3);
            if force.breaches==3 {
                if let Some(target)=world.travel.settlements.get_mut(&force.target) {target.charter=Some(force.attacker);}
                force.withdrawing=true;
                transit::recall(world,source,force.resident);
            }
        }
        transit::Siege::Returning=>force.withdrawing=true,
        _=>{},
    }
    true
}

pub(super) fn service(world: &mut World, _viewer: Vec3) {
    if !enabled(world) { return; }
    let cfg = world.sample_config();
    let clock = world.farming.clock;
    let mut campaigns = std::mem::take(&mut world.travel.politics.campaigns);
    campaigns.retain(|&source, force| {
        if force.travel_owned || world.travel.settlements.get(&source).is_some_and(|l|l.physical) {
            return physical_campaign(world,source,force);
        }
        let Some(home) = world.travel.settlements.get(&source) else {
            return false;
        };
        let Some(target) = world.travel.settlements.get(&force.target) else {
            return false;
        };
        if force.health <= 0.0 {
            if home.physical {
                let at = position(&cfg, source, *force, force.steps).unwrap_or(Vec3::ZERO);
                crate::world::farming::jobs::worker_fell(world, force.resident, at);
            } else {
                detach(source, world.travel.settlements.get_mut(&source).unwrap(), force.resident);
            }
            return false;
        }
        force.withdrawing |= home.charter_id(source) != force.attacker
            || target.charter_id(force.target) != force.defender
            || world
                .travel
                .politics
                .relations
                .get(&pair(force.attacker, force.defender))
                != Some(&Relation::War)
            || force.supply == 0;
        if force.withdrawing && force.steps == 0 {
            if home.physical {
                let at = position(&cfg, source, *force, 0).unwrap_or(Vec3::splat(f32::MAX));
                let stored = crate::world::farming::jobs::communities::deposit(world, source,
                    at, crate::world::farming::jobs::communities::MEAL, u32::from(force.supply));
                force.supply -= stored as u8;
                if force.supply != 0 { return true; }
                crate::world::farming::jobs::communities::set_health(world, force.resident, force.health);
                return false;
            }
            let home = world.travel.settlements.get_mut(&source).unwrap();
            home.food = (home.food + i32::from(force.supply)).min(64);
            return false;
        }
        if force.withdrawing || force.steps < force.length {
            let next = if force.withdrawing {
                force.steps - 1
            } else {
                force.steps + 1
            };
            // Legacy campaigns also validate every crossing, independently of the viewer.
            let Some(at) = position(&cfg, source, *force, next) else { return false; };
            let Some(previous)=position(&cfg,source,*force,force.steps) else {return false;};
            if !crate::world::farming::jobs::activity::prepare_corridor(world,previous,at)
                || !vehicle::campaign_crossing(world,previous,at) {
                force.blocked_at = Some(next);
                return true;
            }
            force.blocked_at = None;
            if !force.withdrawing && next % 60 == 0 {
                force.supply -= 1;
            }
            force.steps = next;
            if next == force.length {
                force.next_siege = clock.saturating_add(8 * SECOND);
            }
            return true;
        }
        if clock < force.next_siege {
            return true;
        }
        force.next_siege = clock.saturating_add(8 * SECOND);
        force.supply -= 1;
        let defenders = crate::world::farming::jobs::communities::population(world, force.target)
            .map_or_else(|| world.travel.settlements[&force.target].population() as usize, |n| n);
        if defenders > 0 && spend(world, force.target, 0, 4) {
            force.health = (force.health - 2.0).max(0.0);
            return true;
        }
        force.breaches += 1;
        if force.breaches < 3 {
            return true;
        }
        world.travel.settlements.get_mut(&force.target).unwrap().charter = Some(force.attacker);
        force.withdrawing = true;
        true
    });
    world.travel.politics.campaigns = campaigns;
    world.travel.politics.people=world.travel.politics.campaigns.values()
        .map(|c|(c.resident,(c.travel_owned,c.travel_owned && c.hostile_to_player() && !c.withdrawing))).collect();
    world.travel.politics.indexed=true;

    let today = day(world);
    world.travel.politics.last_counter = today;
    // One source and a bounded local neighborhood per service. A day boundary
    // must not trigger an all-pairs scan over every retained settlement.
    use std::ops::Bound::{Excluded, Unbounded};
    let cursor = world.travel.politics.diplomacy_cursor;
    let Some((&id,_)) = world.travel.settlements.range((Excluded(cursor),Unbounded)).next()
        .or_else(||world.travel.settlements.first_key_value()) else { return; };
    world.travel.politics.diplomacy_cursor = id;
    let Some(source) = geo::settlement_by_id(&cfg,id) else { return; };
    if !geo::prepare_settlements_near(&cfg,source.center,512.0) { return; }
    let sites: Vec<_> = geo::settlements_near(&cfg,source.center,512.0).into_iter()
        .filter(|site|site.id != id && world.travel.settlements.contains_key(&site.id)).take(16).collect();
        let ledger = &world.travel.settlements[&source.id];
        let owner = ledger.charter_id(source.id);
        let (food, materials) = stocks(world, source.id, ledger);
        if owner == PLAYER_CHARTER || food < 24 || materials < 4 {
            return;
        }
        if let Some(&target) = sites.iter().find(|target| {
            let defender = world.travel.settlements[&target.id].charter_id(target.id);
            world.travel.politics.relations.get(&pair(owner, defender)) == Some(&Relation::War)
                && geo::linked(&cfg, source, **target)
        }) {
            mobilize(world, source, target);
            return;
        }
        // War is a provisioned response to accumulated incidents, not a random
        // declaration or an automatic consequence of a single hungry thief.
        if ledger.population() < 4
            || world.travel.politics.campaigns.contains_key(&source.id)
            || food < ledger.population() as i32*4+i32::from(SUPPLY)
        {
            return;
        }
        if let Some(&target) = sites.iter().find(|target| {
            let rival = &world.travel.settlements[&target.id];
            let defender = rival.charter_id(target.id);
            defender != owner
                && defender != PLAYER_CHARTER
                && world.travel.politics.grievances.get(&pair(owner,defender)).copied().unwrap_or(0)>=6
                && !world
                    .travel
                    .politics
                    .relations
                    .contains_key(&pair(owner, defender))
                && geo::linked(&cfg, source, **target)
        }) {
            let key = pair(
                owner,
                world.travel.settlements[&target.id].charter_id(target.id),
            );
            world.travel.politics.relations.insert(key, Relation::War);
            mobilize(world, source, target);
            if !world.travel.politics.campaigns.contains_key(&source.id) {
                world.travel.politics.relations.remove(&key);
            }
        }
}
