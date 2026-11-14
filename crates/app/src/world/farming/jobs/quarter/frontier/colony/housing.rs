//! Existing residents occupy completed houses with clear entrance corridors.
use super::{jobs, known, IVec3, World};
use super::super::State;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

mod allocation;
mod negative;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Home {
    pub capacity: u8,
    pub entry: [i32; 3],
    pub destination: [i32; 3],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Household {
    pub project: usize,
    pub members: Vec<u32>,
}

pub(in super::super::super) fn valid(s: &State) -> bool {
    let mut projects = BTreeSet::new();
    let mut people = BTreeSet::new();
    let overrides: BTreeSet<_> = s.housing_overrides.iter().copied().collect();
    s.housing_overrides.len() <= jobs::crew::CAPACITY && overrides.len() == s.housing_overrides.len()
    && s.projects.iter().all(|p| p.home.as_ref().is_none_or(|h| {
        let a = IVec3::from(h.entry);
        let b = IVec3::from(h.destination);
        (1..=8).contains(&h.capacity)
            && h.entry.iter().chain(&h.destination).all(|n| n.unsigned_abs() < 1 << 27)
            && a.y == b.y && (a - b).abs().element_sum() == 2
            && (a.x == b.x || a.z == b.z)
    })) && s.households.len() <= s.projects.len()
        && s.households.iter().all(|h| {
            projects.insert(h.project) && !h.members.is_empty()
                && s.projects.get(h.project).and_then(|p| p.home.as_ref())
                    .is_some_and(|home| h.members.len() <= usize::from(home.capacity))
                && h.members.iter().all(|id| !overrides.contains(id) && people.insert(*id))
        })
}

pub(super) fn usable(w: &World, s: &State, index: usize) -> bool {
    let Some(p) = s.projects.get(index) else { return false };
    let Some(home) = &p.home else { return false };
    let high = p.cells.iter().map(|o| p.work_layer(*o)).max().unwrap_or(0);
    if !s.project_layers.get(index).is_some_and(|layer| *layer > high + 1)
        || !p.cells.iter().all(|o| known(w, o.cell()) == Some(o.block())) {
        return false;
    }
    let v = w.voxel_scale.meters();
    let entry = IVec3::from(home.entry);
    let destination = IVec3::from(home.destination);
    if !jobs::stand(w, entry, v) || !jobs::stand(w, destination, v) { return false; }
    let mut query = jobs::navigation::Query::new(w, v);
    jobs::clear_level_walk(&mut query, jobs::metres(entry, v), jobs::metres(destination, v), v)
}

fn residents(w: &World) -> Vec<u32> {
    let mut ids: Vec<_> = w.farming.jobs.production.as_ref().into_iter()
        .flat_map(|p| p.workers.iter().copied())
        .filter(|id| jobs::crew::view(&w.farming.jobs, *id).is_some()).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub(in super::super) fn service(w: &mut World, s: &mut State) {
    let mut ids = residents(w);
    let v = w.voxel_scale.meters();
    let mut overrides: BTreeSet<_> = s.housing_overrides.iter().copied().collect();
    for id in &ids {
        let assigned = s.households.iter().find(|h| h.members.contains(id))
            .and_then(|h| s.projects.get(h.project)).and_then(|p| p.home.as_ref());
        let moved = assigned.is_some_and(|home|
            w.travel.homes.get(id) != Some(&jobs::metres(home.destination.into(), v)));
        if moved || w.travel.following == Some(*id) || w.travel.guard_posts.contains_key(id) {
            if overrides.len() < jobs::crew::CAPACITY { overrides.insert(*id); }
        }
    }
    s.housing_overrides = overrides.into_iter().collect();
    // A full override ledger disables adoption rather than forgetting commands.
    ids.retain(|id| s.housing_overrides.len() < jobs::crew::CAPACITY
        && !s.housing_overrides.contains(id));
    let capacities: Vec<_> = s.projects.iter().enumerate().map(|(i, p)| {
        if usable(w, s, i) { p.home.as_ref().map_or(0, |h| h.capacity) } else { 0 }
    }).collect();
    let old: Vec<_> = s.households.iter().map(|h| (h.project, h.members.clone())).collect();
    let next = allocation::assign(&ids, &capacities, &old);
    for h in &s.households {
        let Some(home) = s.projects.get(h.project).and_then(|p| p.home.as_ref()) else { continue };
        let target = jobs::metres(home.destination.into(), v);
        for id in &h.members {
            if !s.housing_overrides.contains(id) && w.travel.following != Some(*id)
                && !w.travel.guard_posts.contains_key(id)
                && !next.iter().any(|(project, members)| *project == h.project && members.contains(id))
                && w.travel.homes.get(id) == Some(&target) {
                w.travel.homes.remove(id);
            }
        }
    }
    s.households = next.into_iter().map(|(project, members)| Household { project, members }).collect();
    for h in &s.households {
        if let Some(home) = s.projects[h.project].home.as_ref() {
            let target = jobs::metres(home.destination.into(), v);
            for id in &h.members { w.travel.homes.insert(*id, target); }
        }
    }
}

/// Verify occupancy against physical houses after the final project commits.
pub(in super::super) fn check(w: &mut World) -> anyhow::Result<usize> {
    use anyhow::{Context, ensure};
    let mut city = w.farming.jobs.quarter.take().context("Missing colony")?;
    let result = (|| {
        let s = city.frontier.as_mut().context("Missing colony projects")?;
        service(w, s);
        ensure!(valid(s), "Invalid household ledger");
        let ids: Vec<_> = residents(w).into_iter().filter(|id|
            s.housing_overrides.len() < jobs::crew::CAPACITY && !s.housing_overrides.contains(id)).collect();
        let capacity: usize = s.projects.iter().enumerate().filter(|(i, _)| usable(w, s, *i))
            .filter_map(|(_, p)| p.home.as_ref()).map(|h| usize::from(h.capacity)).sum();
        if s.phase == s.projects.len() {
            let planned: usize = s.projects.iter().filter_map(|p| p.home.as_ref())
                .map(|h| usize::from(h.capacity)).sum();
            ensure!(capacity == planned, "Completed colony has inaccessible housing");
        }
        let count: usize = s.households.iter().map(|h| h.members.len()).sum();
        ensure!(count == ids.len().min(capacity), "Household capacity not allocated");
        for h in &s.households {
            ensure!(usable(w, s, h.project), "Occupied house is incomplete or obstructed");
            let home = s.projects[h.project].home.as_ref().context("Missing home metadata")?;
            let target = jobs::metres(home.destination.into(), w.voxel_scale.meters());
            ensure!(h.members.iter().all(|id| ids.contains(id) && w.travel.homes.get(id) == Some(&target)),
                "Household resident or destination mismatch");
        }
        let encoded = ron::to_string(s)?;
        let restored: State = ron::from_str(&encoded)?;
        ensure!(valid(&restored) && restored.households == s.households
            && restored.housing_overrides == s.housing_overrides, "Household reload mismatch");
        let previous = s.households.clone();
        service(w, s);
        ensure!(previous == s.households, "Households changed without a world change");
        negative::check(w, s)?;
        Ok(count)
    })();
    w.farming.jobs.quarter = Some(city);
    result
}
