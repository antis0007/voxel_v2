//! Paid small-house parcels share the district street router.
use super::{Point, Profile, IVec3};
use super::super::frontier::{Project, State as Site, scene};
use crate::world::{World, farming::jobs};
use engine_content::BlockId as B;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
mod survey;
mod planning;
mod validation;
mod incremental;
mod local;
pub(crate) use survey::revalidate;
pub(crate) use planning::generate;
pub(crate) use incremental::poll;
pub(crate) const RETRY: u64 = 240 * jobs::PULSE;
pub(crate) const GRID: i32 = 24;
pub(crate) const ROUTES: u32 = 24;
pub(crate) const PROJECT_LIMIT: usize = 16;
pub(crate) const CANDIDATES: u32 = 4 * (2 * GRID as u32 + 1).pow(2);
pub(crate) const CANDIDATES_PER_POLL: u32 = 64;
pub(crate) const ROUTES_PER_POLL: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfillPlan {
    pub schema: u8,
    pub seed: u32,
    pub profile: u8,
    pub base: [i32; 3],
    pub stockpile: [i32; 3],
    pub parcels: Vec<InfillParcel>,
    pub surveyed: bool,
    pub retry_at: u64,
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<incremental::Search>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfillParcel {
    pub origin: Point,
    pub turn: bool,
    pub stairs_right: bool,
    pub footprint: [i32; 4],
    pub entrance: Point,
    pub access: Vec<Point>,
    /// Stable project and order indices reference already paid frontage.
    pub anchor: [usize; 2],
    pub provenance: u32,
    pub project: Option<usize>,
}

impl InfillPlan {
    pub(crate) fn new(base: IVec3, stockpile: [i32; 3], seed: u32, profile: u8) -> Self {
        Self { schema: 1, seed, profile, base: base.to_array(), stockpile,
            parcels: Vec::new(), surveyed: false, retry_at: 0, attempts: 0, search: None }
    }
    pub(crate) fn valid(&self, s: &Site) -> bool { self.valid_prefix(s,s.projects.len()) }
    pub(crate) fn valid_prefix(&self, s: &Site, count: usize) -> bool { validation::valid(self,s,count) }
    pub(crate) fn pending(&self) -> Option<&InfillParcel> {
        self.parcels.iter().find(|p|p.project.is_none())
    }
    pub(crate) fn searching(&self) -> bool { self.search.is_some() }
    pub(crate) fn restart(&mut self) {
        self.search=None; self.surveyed=false; self.retry_at=0;
    }
}

impl InfillParcel {
    pub(crate) fn building(&self, base: IVec3) -> Result<Project, String> {
        scene::infill([self.origin[0],1,self.origin[1]],self.turn,self.stairs_right,Vec::new(),base)
    }
    pub(crate) fn build(&self, base: IVec3) -> Result<Project, String> {
        let building = self.building(base)?;
        let path = self.access.iter().take(self.access.len().saturating_sub(1))
            .filter(|p|!building.cells.iter().any(|o|o.cell() == at(base,**p)))
            .map(|p|[p[0],1,p[1]]).collect();
        scene::infill([self.origin[0],1,self.origin[1]],self.turn,self.stairs_right,path,base)
    }
}

pub(crate) fn same(a: &Project, b: &Project) -> bool {
    a.name == b.name && a.cells == b.cells && a.food_reserve == b.food_reserve
        && a.meals_eaten == b.meals_eaten && a.home == b.home
}
fn at(base: IVec3, p: Point) -> IVec3 { base + IVec3::new(p[0],1,p[1]) }
fn point(base: IVec3, p: IVec3) -> Point { [p.x-base.x,p.z-base.z] }
fn contains(b: [i32;4], [x,z]: Point) -> bool { x>=b[0] && x<=b[2] && z>=b[1] && z<=b[3] }
fn bounded(p: Point) -> bool { p.into_iter().all(|v|v.unsigned_abs()<=GRID as u32) }
fn provenance(seed: u32, origin: Point, turn: bool, right: bool) -> u32 {
    engine_core::hash::hash_u32(seed ^ (origin[0] as u32).wrapping_mul(7919)
        ^ (origin[1] as u32).wrapping_mul(104729) ^ u32::from(turn)*19 ^ u32::from(right)*43)
}
fn shape(p: &Project, base: IVec3) -> Option<([i32;4],Point)> {
    let low = p.cells.iter().map(|o|o.cell()).reduce(IVec3::min)?-base;
    let high = p.cells.iter().map(|o|o.cell()).reduce(IVec3::max)?-base;
    Some(([low.x,low.z,high.x,high.z],point(base,IVec3::from(p.home.as_ref()?.entry))))
}
fn compatible(a: &InfillParcel, b: &InfillParcel) -> bool {
    !super::overlaps(a.footprint,b.footprint)
        && !a.access.iter().take(a.access.len().saturating_sub(1)).any(|p|contains(b.footprint,*p))
        && !b.access.iter().take(b.access.len().saturating_sub(1)).any(|p|contains(a.footprint,*p))
        && !a.access.iter().skip(1).take(a.access.len().saturating_sub(2)).any(|p|
            b.access.iter().take(b.access.len().saturating_sub(1)).any(|q|q==p))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scans {
    pub candidates: u32,
    pub cells: u64,
    pub path_searches: u32,
    pub path_node_budget: u64,
}
