//! Autonomous settlements reuse the same paid work executor as the player's crew.
//! There is one saved owner per worker, and one ordered wake-up per due person.
use super::{crew, production, vehicle, IVec3, ItemStack, State as Jobs, Vec3, World, PULSE};
use crate::world::inventory::{FoodKind, Item};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod founding;
mod market;
mod development;
mod custody;
mod roads;
pub(crate) mod transit;
pub(crate) use market::{exchange, market_ready, trade_stock};

const MAX_SITES: usize = crate::world::travel::civilization::MAX_SETTLEMENTS;
// Most wakes are now activity boundaries. Navigation has its own expansion budget.
const EVENTS_PER_TICK: usize = 64;
const FOUNDING: u32 = u32::MAX;
const COUNCIL: u32 = u32::MAX - 1;
pub(crate) const MEAL: Item = Item::Food(FoodKind::Bread);
const TIMBER: Item = Item::Block(super::B::Planks);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Home {
    pub destination: [i32; 3],
    pub entry: [i32; 3],
}

impl Home {
    pub(super) fn valid(&self) -> bool {
        self.destination.iter().chain(&self.entry).all(|n| n.unsigned_abs() < 1 << 28)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Person {
    home: Home,
    #[serde(default)] enlisted: bool,
    #[serde(default)] died_at: Option<u64>,
    #[serde(default)] experience: [u32; 4],
    #[serde(default)] grievance: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Community {
    jobs: Box<Jobs>,
    people: BTreeMap<u32, Person>,
    station: [i32; 3],
    fields: Vec<[i32; 3]>,
    /// 0: uninitialized; 1: finite initial stock committed; 2: operational.
    stage: u8,
    wakes: BTreeMap<u32, u64>,
    #[serde(default)] blocked: Option<String>,
    #[serde(default)] development: development::State,
    #[serde(default)] next_plan: u64,
    /// Supplies brought by a founder. None denotes a generated historical town.
    #[serde(default)] starter: Option<Vec<ItemStack>>,
    #[serde(default)] roads: roads::State,
    #[serde(skip)] work_signature: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct State {
    sites: BTreeMap<u32, Community>,
    #[serde(skip)] owners: BTreeMap<u32, u32>,
    #[serde(skip)] agenda: BTreeSet<(u64, u32, u32)>,
    #[serde(skip)] planning: BTreeSet<(u64, u32, u32)>,
    #[serde(skip)] indexed: bool,
    /// The player's crew remains queryable while a community owns the executor.
    #[serde(skip)] borrowed: Option<Box<Jobs>>,
    /// Reused empty executor storage, avoiding allocations on every wake-up.
    #[serde(skip)] spare: Option<Box<Jobs>>,
    #[serde(skip)] events: u64,
    #[serde(skip)] oldest_delay: u64,
}

impl State {
    pub(in crate::world::farming) fn used(&self) -> bool { !self.sites.is_empty() }

    pub(in crate::world::farming) fn restore(&mut self) {
        self.owners.clear();
        self.agenda.clear();
        self.planning.clear();
        for (&site, community) in &self.sites {
            for &person in community.people.keys() { self.owners.insert(person, site); }
            for (&person, &due) in &community.wakes { self.agenda.insert((due, site, person)); }
            if community.stage == 2 { self.planning.insert((community.next_plan, site, COUNCIL)); }
        }
        self.indexed = true;
    }

    pub(in crate::world::farming) fn valid(&self, local: &Jobs, now: u64) -> bool {
        let mut identities = BTreeSet::new();
        self.sites.len() <= MAX_SITES && self.sites.iter().all(|(&site, c)| {
            engine_worldgen::civilization::original_settlement(site) == Some(site)
                && c.stage <= 2 && c.jobs.valid() && c.jobs.quarter.is_none()
                && c.jobs.stockpile.is_some() && !c.people.is_empty() && c.people.len() <= crew::CAPACITY
                && c.fields.len() <= 24 && c.fields.iter().chain([&c.station])
                    .all(|p| p.iter().all(|n| n.unsigned_abs() < 1 << 28))
                && c.blocked.as_ref().is_none_or(|s| s.len() <= 96)
                && c.development.valid()
                && c.roads.valid()
                && c.people.iter().all(|(&id, p)| {
                    identities.insert(id) && engine_worldgen::civilization::resident_id(id) && p.home.valid()
                        && p.died_at.is_none_or(|at| at <= now)
                        && if p.died_at.is_some() {
                            !p.enlisted && crew::view(&c.jobs, id).is_none() && crew::view(local, id).is_none()
                        } else if p.enlisted {
                            crew::view(&c.jobs, id).is_none() && crew::view(local, id).is_some()
                        } else {
                            crew::view(&c.jobs, id).is_some() && crew::view(local, id).is_none()
                        }
                })
                && crew::ids(&c.jobs).all(|id| c.people.get(&id).is_some_and(|p| !p.enlisted && p.died_at.is_none())
                    && crew::mission(&c.jobs,id).is_none_or(|m|m.source==site))
                && c.wakes.len() <= crew::CAPACITY
                && c.starter.as_ref().is_none_or(|p| p.len() <= 12 && p.iter().all(|s| (1..=256).contains(&s.count)))
                && (if c.stage < 2 { c.wakes.contains_key(&FOUNDING) && c.wakes.len() == 1 }
                    else { crew::ids(&c.jobs).all(|id| c.wakes.contains_key(&id)) })
                && c.wakes.keys().all(|&id| if c.stage < 2 { id == FOUNDING }
                    else { c.people.get(&id).is_some_and(|p| !p.enlisted && p.died_at.is_none()) })
        })
    }

    /// Migration has no world access and cannot publish work or create actors.
    pub(in crate::world::farming) fn migrate_legacy_commitments(&mut self, clock: u64) {
        for city in self.sites.values_mut() {
            for id in crew::migrate_legacy_commitments(&mut city.jobs, clock) {
                if city.stage == 2 {
                    city.wakes.insert(id, clock);
                }
            }
        }
        self.restore();
    }

    pub(in crate::world::farming) fn checkpoint(&self, actors: &[crate::world::adventure::Enemy], now: u64) -> Self {
        let mut saved = self.clone();
        for c in saved.sites.values_mut() { crew::checkpoint(&mut c.jobs, actors, now); }
        saved
    }

    fn schedule(&mut self, site: u32, person: u32, due: u64) {
        let Some(c) = self.sites.get_mut(&site) else { return; };
        if person == COUNCIL {
            self.planning.remove(&(c.next_plan, site, COUNCIL));
            c.next_plan = due;
            self.planning.insert((due, site, COUNCIL));
            return;
        }
        if let Some(previous) = c.wakes.insert(person, due) { self.agenda.remove(&(previous, site, person)); }
        self.agenda.insert((due, site, person));
    }

    fn cancel(&mut self, site: u32, person: u32) {
        if person == COUNCIL {
            if let Some(c) = self.sites.get(&site) { self.planning.remove(&(c.next_plan, site, COUNCIL)); }
            return;
        }
        if let Some(due) = self.sites.get_mut(&site).and_then(|c| c.wakes.remove(&person)) {
            self.agenda.remove(&(due, site, person));
        }
    }
}

pub(crate) fn register(w: &mut World, site: engine_worldgen::civilization::Settlement) {
    if !super::enabled(w) || w.farming.communities.sites.contains_key(&site.id)
        || w.farming.communities.sites.len() >= MAX_SITES || site.history.households == 0 { return; }
    if !w.farming.communities.indexed { w.farming.communities.restore(); }
    let community = founding::plan(w, site);
    for &id in community.people.keys() { w.farming.communities.owners.insert(id, site.id); }
    w.farming.communities.sites.insert(site.id, community);
    w.farming.communities.schedule(site.id, FOUNDING, w.farming.clock);
}

pub(super) fn state_for(w: &World, id: u32) -> Option<&Jobs> {
    let state = &w.farming.communities;
    if let Some(local) = &state.borrowed && crew::view(local, id).is_some() { return Some(local); }
    let site = *state.owners.get(&id)?;
    let c = state.sites.get(&site)?;
    crew::view(&c.jobs, id).is_some().then_some(&c.jobs)
}

pub(super) fn restore_index(w:&mut World) {
    if !w.farming.communities.indexed {w.farming.communities.restore();}
}

pub(super) fn all_jobs(w: &World) -> impl Iterator<Item = &Jobs> {
    w.farming.communities.sites.values().map(|c| &*c.jobs)
        .chain(w.farming.communities.borrowed.as_deref())
}

pub(crate) fn present(w: &World, id: u32) -> Option<bool> {
    let site = *w.farming.communities.owners.get(&id)?;
    Some(w.farming.communities.sites.get(&site)?.people.get(&id)?.died_at.is_none())
}

pub(crate) fn population(w: &World, site: u32) -> Option<usize> {
    Some(w.farming.communities.sites.get(&site)?.people.values().filter(|p| p.died_at.is_none()).count())
}

pub(crate) fn resident_ids(w: &World, site: u32) -> Option<Vec<u32>> {
    Some(w.farming.communities.sites.get(&site)?.people.iter()
        .filter(|(_,p)| p.died_at.is_none()).map(|(&id,_)| id).collect())
}

pub(crate) fn home(w: &World, id: u32) -> Option<Vec3> {
    let site = *w.farming.communities.owners.get(&id)?;
    let p = w.farming.communities.sites.get(&site)?.people.get(&id)?;
    p.died_at.is_none().then(|| super::metres(p.home.destination.into(),w.voxel_scale.meters()))
}

pub(crate) fn owner_site(w: &World, id: u32) -> Option<u32> {
    w.farming.communities.owners.get(&id).copied()
        .or_else(|| crate::world::travel::civilization::physical_settlement(w, id))
}

pub(super) fn interrupt_activity(w: &mut World, id: u32) {
    let at = w.farming.activities.invalidated_at(id).unwrap_or(w.farming.clock);
    interrupt_activity_at(w,id,at);
}

pub(super) fn interrupt_activity_at(w: &mut World, id: u32, at: u64) -> bool {
    let now=w.farming.clock;
    let mut handled = crew::interrupt_activity(&mut w.farming.jobs,id,at)
        || w.farming.communities.borrowed.as_mut().is_some_and(|j|crew::interrupt_activity(j,id,at));
    let Some(site) = w.farming.communities.owners.get(&id).copied() else { return handled; };
    if !handled && let Some(c) = w.farming.communities.sites.get_mut(&site) {
        handled = crew::interrupt_activity(&mut c.jobs,id,at);
    }
    if w.farming.communities.sites.get(&site).is_some_and(|c| c.stage == 2
        && c.people.get(&id).is_some_and(|p|!p.enlisted && p.died_at.is_none())) {
        w.farming.communities.schedule(site,id,now);
    }
    handled
}

pub(crate) fn expedition_ready(w: &World, id: u32) -> bool {
    let Some(j) = super::resident_state(w, id) else { return true; };
    let ready = crew::view(j, id).is_none_or(|p| p.carry.is_none() && !p.batch && !p.mission && p.shopping.is_none())
        && crew::body(w, id).is_some();
    ready && market_ready(w, id).unwrap_or(true)
}

fn restaff(j: &mut Jobs, travel: &crate::world::travel::State) {
    let available = |id: u32| travel.following != Some(id)
        && !travel.guard_posts.contains_key(&id) && !travel.politics.mobilized(id)
        && !crew::view(j, id).is_some_and(|worker| worker.mission);
    if j.production.as_ref().is_none_or(|p| p.workers.iter().copied()
        .eq(crew::ids(j).filter(|&id| available(id)))) { return; }
    let workers: Vec<_> = crew::ids(j).filter(|&id| available(id)).collect();
    if workers.is_empty() { return; }
    if let Some(p) = &mut j.production {
        p.workers = workers;
        for (i, bill) in p.bills.iter_mut().enumerate() {
            if !p.workers.contains(&bill.worker) { bill.worker = p.workers[i % p.workers.len()]; }
        }
    }
}

/// Temporarily select the existing executor; its source pool stays shared.
fn with_site<R>(w: &mut World, site: u32, f: impl FnOnce(&mut World, &mut Community) -> R) -> Option<R> {
    if w.farming.communities.borrowed.is_some() { return None; }
    let mut city = w.farming.communities.sites.remove(&site)?;
    let spare = w.farming.communities.spare.take().unwrap_or_default();
    let mut local = std::mem::replace(&mut city.jobs, spare);
    std::mem::swap(&mut w.farming.jobs, &mut *local);
    std::mem::swap(&mut local.sources, &mut w.farming.jobs.sources);
    w.farming.communities.borrowed = Some(local);
    let result = f(w, &mut city);
    let mut local = w.farming.communities.borrowed.take().expect("community executor owner");
    std::mem::swap(&mut local.sources, &mut w.farming.jobs.sources);
    std::mem::swap(&mut w.farming.jobs, &mut *local);
    w.farming.communities.spare = Some(std::mem::replace(&mut city.jobs, local));
    w.farming.communities.sites.insert(site, city);
    Some(result)
}

pub(super) fn service(w: &mut World, player: Vec3) {
    if !super::enabled(w) { return; }
    if !w.farming.communities.indexed { w.farming.communities.restore(); }
    let now = w.farming.clock;
    for event in 0..=EVENTS_PER_TICK {
        // At most one expensive council per fixed step. Agent completions have
        // a separate lane, so a planning backlog cannot starve movement or meals.
        let queue=if event==0 {&w.farming.communities.planning} else {&w.farming.communities.agenda};
        let Some((due,site,person))=queue.first().copied().filter(|e|e.0<=now) else {
            if event==0 {continue;} else {break;}
        };
        w.farming.communities.cancel(site, person);
        w.farming.communities.events = w.farming.communities.events.saturating_add(1);
        w.farming.communities.oldest_delay = w.farming.communities.oldest_delay.max(now.saturating_sub(due));
        let next = with_site(w, site, |w, city| {
            if person == FOUNDING {
                if founding::advance(w, city) {
                    return crew::ids(&w.farming.jobs).enumerate()
                        .map(|(i, id)| (id, now.saturating_add(i as u64 + 1)))
                        .chain([(COUNCIL, now.saturating_add(120 + u64::from(site % 480)))])
                        .collect::<Vec<_>>();
                }
                return vec![(FOUNDING, now.saturating_add(120))];
            }
            if person == COUNCIL {
                restaff(&mut w.farming.jobs, &w.travel);
                development::plan(w, site, city);
                city.roads.plan(w);
                use std::hash::{Hash,Hasher};
                let mut hash=std::collections::hash_map::DefaultHasher::new();
                w.farming.jobs.build.len().hash(&mut hash);
                w.farming.jobs.mine.len().hash(&mut hash);
                if let Some(pile)=w.farming.jobs.stockpile {
                    for item in w.block_data.chest(pile.into()).into_iter().flatten() {
                        item.item.hash(&mut hash);item.count.hash(&mut hash);
                    }
                }
                if let Some(p)=&w.farming.jobs.production {
                    p.bills.len().hash(&mut hash);
                    for bed in &p.beds {
                        let cell=crate::world::chunk_coord_from_world_block(bed.at.into());
                        w.regions.source_version(cell).hash(&mut hash);
                    }
                }
                let signature=hash.finish();
                let mut events=vec![(COUNCIL, now.saturating_add(development::INTERVAL))];
                if signature!=city.work_signature {
                    let waiting:Vec<_>=crew::ids(&w.farming.jobs).filter(|&id|crew::view(&w.farming.jobs,id)
                        .is_some_and(|p| !p.navigation_wait && (p.task.is_none() || p.activity.is_some_and(|a|a.waiting())))).collect();
                    for id in waiting {
                        crew::interrupt_activity(&mut w.farming.jobs,id,now);
                        events.push((id,now.saturating_add(1)));
                    }
                    city.work_signature=signature;
                }
                return events;
            }
            if city.people.get(&person).is_none_or(|p| p.enlisted || p.died_at.is_some()) { return Vec::new(); }
            restaff(&mut w.farming.jobs, &w.travel);
            let before = crew::view(&w.farming.jobs, person)
                .map(|p| (p.routine.productive, development::discipline(p.task), p.pos,
                    matches!(p.task, Some(super::Task::Build(o)) if o.paving())));
            crew::service_person(w, player, person);
            if let (Some((before, skill, previous, paving)), Some(after), Some(p)) =
                (before, crew::view(&w.farming.jobs, person), city.people.get_mut(&person)) {
                if let (Some(a),Some(b),Some(pile)) = (previous,after.pos,w.farming.jobs.stockpile) {
                    city.roads.observe(w,pile.into(),a.into(),b.into());
                }
                if paving && after.routine.productive > before { city.roads.built = city.roads.built.saturating_add(1); }
                if let Some(skill) = skill {
                    p.experience[skill] = p.experience[skill].saturating_add(after.routine.productive.saturating_sub(before));
                }
            }
            if crew::view(&w.farming.jobs, person).is_none() {
                if let Some(p) = city.people.get_mut(&person) { p.died_at = Some(now); }
                restaff(&mut w.farming.jobs, &w.travel);
                return Vec::new();
            }
            vec![(person, crew::next_wake(&w.farming.jobs, person, now))]
        }).unwrap_or_default();
        for (person, due) in next { w.farming.communities.schedule(site, person, due); }
        if person == COUNCIL { transit::policy(w, site); }
        else if person < COUNCIL { transit::arrive(w, site, person); }
        // Edits performed while an executor owned the town must win over the
        // deadline that executor computed before returning it to the registry.
        super::activity::flush(w);
    }
}

/// Recruiting changes custody only after existing cargo and paid batches finish.
pub(super) fn enlist(w: &mut World, id: u32) -> Result<bool, &'static str> {
    let Some(&site) = w.farming.communities.owners.get(&id) else { return Ok(false); };
    let Some(c) = w.farming.communities.sites.get(&site) else { return Ok(false); };
    if c.people.get(&id).is_none_or(|p| p.died_at.is_some()) { return Err("Resident unavailable"); }
    if c.stage < 2 { return Err("Stores Preparing"); }
    let Some(worker) = crew::view(&c.jobs, id) else { return Ok(false); };
    if worker.carry.is_some() || worker.batch || worker.shopping.is_some() || worker.mission { return Err("Delivery Active"); }
    if crew::ids(&w.farming.jobs).count() >= crew::CAPACITY { return Err("Construction crew full"); }
    let actor = w.adventure.enemies.iter()
        .find(|actor| vehicle::villager_id(actor) == Some(id) && actor.health > 0.0);
    let now = w.farming.clock;
    let c = w.farming.communities.sites.get_mut(&site).unwrap();
    if let Some(actor) = actor { crew::checkpoint(&mut c.jobs, std::slice::from_ref(actor), now); }
    let mut worker = crew::take_worker(&mut c.jobs, id).ok_or("Resident unavailable")?;
    worker.reset();
    crew::put_worker(&mut w.farming.jobs, worker);
    restaff(&mut c.jobs, &w.travel);
    c.people.get_mut(&id).unwrap().enlisted = true;
    w.farming.communities.cancel(site, id);
    Ok(true)
}

pub(super) fn return_home(w: &mut World, id: u32) -> bool {
    let Some(&site) = w.farming.communities.owners.get(&id) else { return false; };
    let Some(c) = w.farming.communities.sites.get_mut(&site) else { return false; };
    if !c.people.get(&id).is_some_and(|p| p.enlisted && p.died_at.is_none()) { return false; }
    let Some(mut worker) = crew::take_worker(&mut w.farming.jobs, id) else { return false; };
    worker.reset();
    crew::put_worker(&mut c.jobs, worker);
    restaff(&mut c.jobs, &w.travel);
    c.people.get_mut(&id).unwrap().enlisted = false;
    if c.stage == 2 { w.farming.communities.schedule(site, id, w.farming.clock.saturating_add(1)); }
    true
}

pub(super) fn fell(w: &mut World, id: u32, feet: Vec3) -> bool {
    let Some(&site) = w.farming.communities.owners.get(&id) else { return false; };
    if state_for(w, id).is_some() {
        with_site(w, site, |w, _| { super::worker_fell(w, id, feet); restaff(&mut w.farming.jobs, &w.travel); });
    }
    mark_dead(w, id);
    true
}

pub(super) fn mark_dead(w: &mut World, id: u32) {
    let Some(&site) = w.farming.communities.owners.get(&id) else { return; };
    if let Some(p) = w.farming.communities.sites.get_mut(&site).and_then(|c| c.people.get_mut(&id)) {
        p.died_at = Some(w.farming.clock);
        p.enlisted = false;
    }
    crate::world::travel::civilization::resident_died(w,site,id);
    w.farming.communities.cancel(site, id);
}

pub(super) fn remember(w: &mut World, actor: &crate::world::adventure::Enemy) {
    let Some(id) = vehicle::villager_id(actor) else { return; };
    let Some(&site) = w.farming.communities.owners.get(&id) else { return; };
    if let Some(c) = w.farming.communities.sites.get_mut(&site) {
        crew::checkpoint(&mut c.jobs, std::slice::from_ref(actor), w.farming.clock);
    }
}

pub(crate) fn set_health(w: &mut World, id: u32, health: f32) {
    interrupt_activity(w,id);
    let Some(&site) = w.farming.communities.owners.get(&id) else { return; };
    if let Some(c) = w.farming.communities.sites.get_mut(&site) { crew::set_health(&mut c.jobs, id, health); }
}

fn stockpile(w: &World, site: u32) -> Option<IVec3> {
    let c = w.farming.communities.sites.get(&site)?;
    (c.stage == 2).then_some(c.jobs.stockpile?).map(IVec3::from)
}

pub(crate) fn stocks(w: &World, site: u32) -> (u32, u32) {
    let Some(pile) = stockpile(w, site) else { return (0, 0); };
    let stock = w.block_data.chest(pile);
    (production::count(&stock, MEAL), production::count(&stock, TIMBER))
}

pub(crate) fn withdraw(w: &mut World, site: u32, food: u32, timber: u32) -> bool {
    let Some(pile) = stockpile(w, site) else { return false; };
    if !super::residency::prepare_cells(w, vec![pile]) { return false; }
    if !super::resident(w, pile) || !super::known(w, pile).is_some_and(|b| b.chest_tier().is_some())
        || !w.block_data.can_store_chest(pile) { return false; }
    let mut slots = w.block_data.chest(pile);
    if production::count(&slots, MEAL) < food || production::count(&slots, TIMBER) < timber { return false; }
    for (item, mut need) in [(MEAL, food), (TIMBER, timber)] {
        for slot in &mut slots {
            if let Some(s) = slot.as_mut().filter(|s| s.item == item) {
                let n = need.min(s.count); need -= n; s.count -= n;
                if s.count == 0 { *slot = None; }
            }
        }
    }
    w.block_data.set_chest(pile, slots);
    true
}

pub(crate) fn deposit(w: &mut World, site: u32, at: Vec3, item: Item, count: u32) -> u32 {
    if count == 0 { return 0; }
    let Some(pile) = stockpile(w, site) else { return 0; };
    if at.distance_squared(super::Task::Store(pile).point(w.voxel_scale.meters())) > 9.0 { return 0; }
    if !super::residency::prepare_cells(w, vec![pile]) { return 0; }
    if !super::resident(w, pile) || !super::known(w, pile).is_some_and(|b| b.chest_tier().is_some()) { return 0; }
    let mut stack = ItemStack::new(item, count);
    crate::world::block_data::store_hauled(w, pile, &mut stack);
    count - stack.count
}

pub(crate) fn stock_summary(w: &World, id: u32) -> Option<String> {
    let site = owner_site(w, id)?;
    let c = w.farming.communities.sites.get(&site)?;
    if c.stage < 2 { return Some(c.blocked.as_deref().unwrap_or("Preparing Stores").into()); }
    let stock = w.block_data.chest(c.jobs.stockpile?.into());
    Some(format!("Bread: {} · Wheat: {} · Planks: {}",
        production::count(&stock, MEAL), production::count(&stock, Item::Block(super::B::WheatCrop)),
        production::count(&stock, TIMBER)))
}

pub(crate) fn report(w: &World) -> serde_json::Value {
    let s = &w.farming.communities;
    let people: usize = s.sites.values().map(|c| c.people.len()).sum();
    let dead = s.sites.values().flat_map(|c| c.people.values()).filter(|p| p.died_at.is_some()).count();
    let settlements: Vec<_> = s.sites.iter().map(|(&id, c)| {
        let (bread, planks) = stocks(w, id);
        serde_json::json!({"id": id, "stage": c.stage, "stockpile": c.jobs.stockpile,
            "station": c.station, "bread": bread, "planks": planks,
            "workers": crew::ids(&c.jobs).collect::<Vec<_>>(),
            "enlisted": c.people.iter().filter(|(_, p)| p.enlisted).map(|(&id, _)| id).collect::<Vec<_>>(),
            "dead": c.people.iter().filter(|(_, p)| p.died_at.is_some()).map(|(&id, _)| id).collect::<Vec<_>>(),
            "next_due": c.wakes.values().min(), "blocked": c.blocked,
            "development": c.development.report(),
            "road_cells_built": c.roads.built,
            "production": c.jobs.production.as_ref().map(|p|serde_json::json!({
                "made": p.made, "planted": p.counts.planted, "harvested": p.counts.harvested,
                "batches": p.counts.batches, "beds": p.beds.len(), "bills": p.bills.len()})),
            "experience": c.people.iter().map(|(&id,p)| (id,p.experience)).collect::<Vec<_>>(),
            "travellers": crew::ids(&c.jobs).filter_map(|id| crew::mission(&c.jobs,id).map(|m|
                serde_json::json!({"id": id, "target": m.target, "purpose": m.purpose,
                    "cargo": m.cargo, "returning": m.returning, "arrived": m.arrived}))).collect::<Vec<_>>()})
    }).collect();
    serde_json::json!({"sites": s.sites.len(), "people": people, "dead": dead,
        "workers": s.sites.values().map(|c| crew::ids(&c.jobs).count()).sum::<usize>(),
        "activities":super::activity::report(w),
        "scheduled_events": s.agenda.len()+s.planning.len(),
        "planning_events":s.planning.len(),
        "due_events": s.agenda.iter().take_while(|e| e.0 <= w.farming.clock).count(),
        "events_processed": s.events, "events_per_tick": EVENTS_PER_TICK,
        "oldest_delay_ticks": s.oldest_delay,
        "settlements": settlements,
        "preparing_sites": s.sites.values().filter(|c| c.stage < 2).count()})
}
