//! Read-only founding progress from resident meals and physical project cells.
use super::{B, IVec3, World, known};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct View {
    pub project: String,
    pub built: usize,
    pub total: usize,
    pub unloaded: usize,
    pub meal_served: bool,
    pub paused: bool,
    pub housing: Option<Housing>,
    #[serde(skip)]
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Housing {
    pub housed: usize,
    pub residents: usize,
    pub demand: usize,
    pub completed: usize,
    pub projects: usize,
}

impl View {
    pub fn complete(&self) -> bool {
        if let Some(housing) = &self.housing {
            return self.unloaded == 0 && housing.demand == 0 && housing.projects > 0
                && housing.completed == housing.projects && self.meal_served;
        }
        self.total > 0 && self.built == self.total && self.unloaded == 0 && self.meal_served
    }
}

pub(crate) fn view(w: &World) -> Option<View> {
    if !w.sample_config().civilization_enabled() { return None; }
    let jobs = &w.farming.jobs;
    let production = jobs.production.as_ref()?;
    let mut result = View {
        project: "Work Shelter".into(), built: 0, total: 0, unloaded: 0,
        meal_served: production.counts.eaten > 0,
        paused: jobs.quarter.as_ref().is_some_and(|q| q.paused),
        housing: None, status: String::new(),
    };
    if let Some(city) = &jobs.quarter {
        result.status = city.status.clone();
        if let Some(site) = &city.frontier && !site.project_layers.is_empty() {
            let complete: Vec<_> = site.projects.iter().map(|project| {
                let mut complete = true;
                for order in &project.cells {
                    result.total += 1;
                    match known(w, order.cell()) {
                        Some(actual) if actual == order.block() => result.built += 1,
                        Some(_) => complete = false,
                        None => { complete = false; result.unloaded += 1; }
                    }
                }
                complete
            }).collect();
            let residents: std::collections::BTreeSet<_> = super::crew::saved_workers(w)
                .into_iter().map(|(id, _)| id).collect();
            let housed = site.households.iter()
                .filter(|h| complete.get(h.project) == Some(&true))
                .flat_map(|h| &h.members)
                .filter(|id| residents.contains(*id))
                .collect::<std::collections::BTreeSet<_>>().len();
            result.project = "Construction".into();
            result.housing = Some(Housing { housed, residents: residents.len(),
                demand: residents.len().saturating_sub(housed),
                completed: complete.into_iter().filter(|done| *done).count(), projects: site.projects.len() });
            return Some(result);
        }
    }
    let mut count = |cell, block| {
        result.total += 1;
        match known(w, cell) {
            Some(actual) => result.built += usize::from(actual == block),
            None => result.unloaded += 1,
        }
    };
    if let Some(site) = jobs.quarter.as_ref().and_then(|q| q.frontier.as_ref()) {
        let first = site.projects.first()?;
        for order in &first.cells { count(order.cell(), order.block()); }
        result.project.clone_from(&first.name);
    } else if jobs.quarter.is_none() {
        let base = IVec3::from(production.bills.first()?.station) - IVec3::Y - IVec3::Z;
        for cell in super::production::scene::shelter_cells(base) { count(cell, B::Planks); }
    } else {
        return None;
    }
    Some(result)
}

pub(crate) fn is_settlement(w: &World) -> bool {
    w.farming.jobs.production.is_some() || w.farming.jobs.quarter.is_some()
}
