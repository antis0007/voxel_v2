//! Finite, recorded initial conditions. Regional scores never become inventory.
use super::*;
use crate::world::{effects, farming::{self, Plot}, BlockId as B};

pub(super) fn plan(w: &World, site: engine_worldgen::civilization::Settlement) -> Community {
    let scale = w.voxel_scale.meters();
    let cfg = w.sample_config();
    let pile = ((site.center + Vec3::new(0.0, 0.0, 2.0)) / scale).floor().as_ivec3();
    let station = pile + IVec3::X;
    let mut jobs = Jobs { stockpile: Some(pile.to_array()), priorities: Some([1, 2, 2]), ..Jobs::default() };
    let mut people = BTreeMap::new();
    for id in site.id..site.id + site.history.households {
        let position = engine_worldgen::households::home(&cfg, id).unwrap_or(site.center);
        let destination = (position / scale).floor().as_ivec3();
        let door = Vec3::new(position.x, position.y, site.center.z + (position.z - site.center.z).signum() * 3.0);
        let home = Home { destination: destination.to_array(), entry: (door / scale).floor().as_ivec3().to_array() };
        jobs.homes.insert(id, home.clone());
        // Discovery can follow recruitment or a death. Keep that custody instead
        // of creating a second worker or reviving the day's recorded casualty.
        let enlisted = crew::view(&w.farming.jobs, id).is_some();
        let died_at = (!enlisted && w.travel.away.get(&id) == Some(&crate::world::travel::day(w)))
            .then_some(w.farming.clock);
        if !enlisted && died_at.is_none() {
            crew::put_worker(&mut jobs, crew::new_worker(id, position, w.farming.clock));
        }
        people.insert(id, Person { home, enlisted, died_at, experience: [0; 4], grievance: 0 });
    }
    crew::checkpoint(&mut jobs, &w.adventure.enemies, w.farming.clock);
    let mut fields = Vec::new();
    if matches!(site.history.livelihood, engine_worldgen::civilization::Livelihood::Floodplain
        | engine_worldgen::civilization::Livelihood::Pasture) {
        for x in [-21.5, -20.0, 20.0, 21.5] { for z in [-8.5, -6.5, 6.5, 8.5] {
            let cell = ((site.center + Vec3::new(x, 0.0, z)) / scale).floor().as_ivec3();
            let y = engine_worldgen::terrain_height(&cfg, cell.x, cell.z) + 1;
            fields.push(IVec3::new(cell.x, y, cell.z).to_array());
        }}
    }
    fields.sort_unstable(); fields.dedup();
    Community { jobs: Box::new(jobs), people, station: station.to_array(), fields,
        stage: 0, wakes: BTreeMap::new(), blocked: None,
        development: development::State::default(), next_plan: 0, starter: None, roads: roads::State::default(), work_signature:0 }
}

fn publish(w: &mut World, at: IVec3, expected: B, block: B) -> bool {
    let Ok(commit) = effects::replace(w, at, expected, block) else { return false; };
    let mut changed = Vec::new();
    commit.publish_voxel(w, &mut changed);
    w.adventure.dirty_voxels.extend(changed);
    true
}

pub(super) fn advance(w: &mut World, city: &mut Community) -> bool {
    if city.stage == 2 { return true; }
    let pile = IVec3::from(w.farming.jobs.stockpile.expect("community stockpile"));
    let station = IVec3::from(city.station);
    let mut cells = vec![pile, pile - IVec3::Y, station, station - IVec3::Y];
    for p in &city.fields {
        let at = IVec3::from(*p);
        for dy in -2..=2 { cells.push(at + IVec3::Y * dy); }
    }
    if !super::super::residency::prepare_cells(w, cells) {
        city.blocked = Some("Terrain Pending".into());
        return false;
    }
    if farming::base::overlaps(w, pile.min(station), pile.max(station)) {
        city.blocked = Some("Protected Site".into());
        return false;
    }
    if city.stage == 0 {
        if w.get(pile) != B::Air || w.get(station) != B::Air
            || !w.get(pile - IVec3::Y).is_solid() || !w.get(station - IVec3::Y).is_solid()
            || !w.block_data.can_store_chest(pile) {
            city.blocked = Some("Site Occupied".into());
            return false;
        }
        if city.starter.is_some() && !crew::ids(&w.farming.jobs).any(|id|
            crew::view(&w.farming.jobs,id).and_then(|p|p.pos).is_some_and(|p|
                Vec3::from(p).distance_squared(super::super::Task::Store(pile).point(w.voxel_scale.meters())) <= 9.0)) {
            city.blocked = Some("Founder Absent".into());
            return false;
        }
        // These goods are the settlement's explicit initial endowment, not
        // retroactive income. A new foundation instead owns its traveller's pack.
        let mut stock = vec![None; crate::world::block_data::CHEST_SLOTS];
        let population = city.people.len() as u32;
        let mut supplies = city.starter.clone().unwrap_or_else(|| vec![
            ItemStack::new(MEAL,population * 2), ItemStack::new(Item::Block(B::WheatCrop),population * 6),
            ItemStack::new(TIMBER,16)]);
        if city.starter.is_some() && !custody::pack_take(&mut supplies,ItemStack::new(Item::Block(B::Chest),1)) {
            city.blocked = Some("Chest Missing".into());
            return false;
        }
        if !supplies.into_iter().filter(|s|s.count>0).all(|s|custody::credit(w,&mut stock,s)) { return false; }
        if !publish(w, pile, B::Air, B::Chest) { return false; }
        w.block_data.set_chest(pile, stock);
        if city.starter.is_some() { city.starter = Some(Vec::new()); }
        city.stage = 1;
    }
    if w.get(pile).chest_tier().is_none() {
        city.blocked = Some("Stockpile Missing".into());
        return false;
    }
    if w.get(station) != B::Workbench {
        let mut stock = w.block_data.chest(pile);
        if w.get(station) != B::Air {
            city.blocked = Some("Workbench Obstructed".into());
            return false;
        }
        if city.starter.is_some() && !custody::debit(&mut stock,ItemStack::new(Item::Block(B::Workbench),1)) {
            city.blocked = Some("Workbench Missing".into());
            return false;
        }
        if !publish(w, station, B::Air, B::Workbench) { return false; }
        if city.starter.is_some() { w.block_data.set_chest(pile,stock); }
    }
    let mut beds = Vec::new();
    if let Some((crop, row)) = farming::table::by_seed(B::WheatCrop) {
        for position in &city.fields {
            let base = IVec3::from(*position);
            let at = (-2..=2).map(|dy| base + IVec3::Y * dy).find(|&p| w.get(p) == B::WheatCrop);
            let Some(at) = at else {
                // Empty arable land is a sowing designation, not a free crop.
                // The ordinary worker must collect a seed and plant it here.
                if let Some(bed) = (-2..=2).map(|dy| production::Bed {
                    at: (base + IVec3::Y * dy).to_array(), crop: B::WheatCrop,
                }).find(|b| b.open(w)) { beds.push(bed); }
                continue;
            };
            if w.farming.plots.len() >= farming::MAX_PLOTS { break; }
            let key = (at.x, at.y, at.z);
            // Register the crop that physically exists; adoption never awards grain.
            if w.farming.plots.contains_key(&key) || farming::base::overlaps(w, at, at) { continue; }
            w.farming.plots.insert(key, Plot { crop, planted: w.farming.clock, rolled: 0, stage: row.mature() });
            beds.push(production::Bed { at: at.to_array(), crop: B::WheatCrop });
        }
    }
    // Retain the production plan even if every living resident is currently
    // enlisted. No work runs until an actual worker returns to this owner.
    let workers: Vec<_> = city.people.iter().filter(|(_, p)| p.died_at.is_none()).map(|(&id, _)| id).collect();
    if workers.is_empty() {
        city.stage = 2;
        city.blocked = None;
        return true;
    }
    let cook = workers[0];
    w.farming.jobs.production = Some(production::State {
        workers, beds,
        bills: vec![
            production::Bill { recipe: "Bread".into(), target: 4, per_resident: 2,
                station: city.station, worker: cook, once: false },
            production::Bill { recipe: "Oak planks".into(), target: 32, per_resident: 0,
                station: city.station, worker: cook, once: false },
        ],
        meal: MEAL, meal_ticks: 24_000, speed: 1, next_meal: BTreeMap::new(),
        served: BTreeMap::new(), counts: production::Counters::default(),
        made: BTreeMap::new(), woodlot: None,
    });
    city.stage = 2;
    city.starter = None;
    city.blocked = None;
    true
}
