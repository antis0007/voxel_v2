//! Bounded paid service parcels retain their terrain and street provenance.
use crate::world::{World, farming::jobs};
use jobs::{known, construction::Order};
use jobs::quarter::{geometry as quarter_geometry, layout::Parcel, Roof, Tradition};
use jobs::quarter::frontier::{Project, State as Site};
use jobs::quarter::frontier::colony::housing::Home;
use engine_content::BlockId as B;
use glam::IVec3;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[path = "plan_content.rs"] mod content;
#[path = "plan_geometry.rs"] mod geometry;
#[path = "plan_survey.rs"] mod survey;
#[path = "plan_terrain.rs"] mod terrain;
#[path = "plan_validation.rs"] mod validation;
#[cfg(test)] #[path = "plan_tests.rs"] mod tests;
#[cfg(test)] pub(crate) use tests::fixture;
pub(crate) use geometry::{home_project, hub_project};
pub(crate) use survey::{available, hub_available, select};
pub(crate) use validation::valid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Plan {
    pub content_version: u32,
    pub provenance: u64,
    pub candidate: usize,
    pub base_projects: usize,
    pub primary: [i32; 3],
    pub hub: [i32; 3],
    pub origin: [i32; 3],
    pub turn: bool,
    pub complexity: u8,
    pub capacity: u8,
    pub parcel: Parcel,
    pub primary_access: Vec<[i32; 3]>,
    pub home_access: Vec<[i32; 3]>,
    pub anchor_project: usize,
    pub anchor_order: usize,
}

pub(crate) fn candidate_count() -> usize { content::read().map_or(0, |c| c.candidates.len()) }
fn point(p: [i32; 3]) -> [i32; 2] { [p[0], p[2]] }
fn shifted(primary: [i32; 3], p: [i32; 2]) -> [i32; 2] {
    [primary[0] + p[0], primary[2] + p[1]]
}
fn bounded(p: [i32; 3]) -> bool { p.into_iter().all(|n| n.unsigned_abs() < 1 << 26) }
fn near(a: [i32; 3], b: [i32; 3]) -> bool {
    (IVec3::from(a) - IVec3::from(b)).as_vec3().length_squared() <= jobs::RANGE.powi(2)
}
fn step(a: [i32; 3], b: [i32; 3]) -> bool {
    let d = (IVec3::from(a) - IVec3::from(b)).abs();
    d.x + d.z == 1 && d.y <= 1
}
fn route(start: [i32; 2], stops: &[[i32; 2]]) -> Result<Vec<[i32; 2]>, String> {
    let mut p = start;
    let mut out = Vec::new();
    for &stop in stops {
        if p[0] != stop[0] && p[1] != stop[1] { return Err("Service Route Bend Invalid".into()); }
        while p != stop {
            if out.len() == 64 { return Err("Service Route Budget".into()); }
            p[0] += (stop[0] - p[0]).signum();
            p[1] += (stop[1] - p[1]).signum();
            out.push(p);
        }
    }
    if out.iter().copied().collect::<BTreeSet<_>>().len() != out.len() {
        return Err("Service Route Loop".into());
    }
    Ok(out)
}
fn primary_route(anchor: [i32; 3], approach: [i32; 2]) -> Result<Vec<[i32; 2]>, String> {
    let out = route(point(anchor), &[[approach[0], anchor[2]], approach])?;
    if out.is_empty() || out.len() > 8 { return Err("Paid Street Out of Reach".into()); }
    Ok(out)
}
fn parcel(c: &content::Content, spec: &content::Candidate, primary: [i32; 3]) -> Parcel {
    Parcel { origin: shifted(primary, spec.origin), turn: spec.turn, tradition: 0,
        complexity: c.complexity, variant: 0, roof_access: false }
}
fn anchor(s: &Site, plan: &Plan) -> Option<Order> {
    s.projects.get(plan.anchor_project)?.cells.get(plan.anchor_order).copied()
}
fn same(a: &Project, b: &Project) -> bool {
    a.name == b.name && a.cells == b.cells && a.home == b.home
        && a.food_reserve == b.food_reserve && a.meals_eaten == b.meals_eaten
}
