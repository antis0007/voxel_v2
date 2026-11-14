//! Settlement projects use the existing paid, path-following builder.
use super::{B, IVec3, Vec3, World};
use serde::{Deserialize, Serialize};
pub(crate) mod check;
mod fixture;
pub(crate) mod geometry;
pub(crate) mod panel;
#[cfg(test)]
mod tests;
mod verify;
mod walk;
mod roof_check;
mod crew_check;
mod market_fixture;
mod market_check;
pub(crate) mod layout;
pub(crate) mod frontier;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Roof {
    Gable,
    Terrace,
    Courtyard,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tradition {
    pub name: String,
    pub wall: B,
    pub frame: B,
    pub roof: B,
    pub form: Roof,
}

pub fn traditions() -> Result<Vec<Tradition>, String> {
    let rows: Vec<Tradition> = ron::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/city_traditions.ron"
    )))
    .map_err(|e| e.to_string())?;
    if rows.len() != 3
        || rows.iter().any(|r| {
            [r.wall, r.frame, r.roof]
                .iter()
                .any(|b| !b.is_solid() || *b as u32 > u8::MAX as u32)
        })
    {
        return Err("Invalid architecture materials".into());
    }
    Ok(rows)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lot {
    pub origin: [i32; 3],
    pub tradition: usize,
    pub complexity: u8,
    pub turn: bool,
    pub layer: i32,
    pub occupied: bool,
    #[serde(default)]
    pub market_cover: bool,
    #[serde(default)]
    pub frontage: bool,
    #[serde(default)]
    pub variant: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub roof_access: bool,
}

fn is_false(value: &bool) -> bool { !*value }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub frontier: Option<frontier::State>,
    #[serde(default)]
    pub layout: Option<layout::Plan>,
    pub lots: Vec<Lot>,
    pub housing_demand: u32,
    pub residents: u32,
    pub paused: bool,
    pub completed: u32,
    pub origin: [i32; 3],
    #[serde(skip)]
    pub status: String,
}

impl State {
    pub(super) fn valid(&self) -> bool {
        self.frontier.as_ref().is_none_or(frontier::State::valid)
            && self.layout.as_ref().is_none_or(layout::Plan::valid)
            && self.lots.len() <= 8
            && self.housing_demand <= 64
            && self.residents <= 256
            && self.origin.iter().all(|n| n.unsigned_abs() < 1 << 27)
            && self.lots.iter().all(|l| {
                l.tradition < 3
                    && (1..=3).contains(&l.complexity)
                    && (0..=80).contains(&l.layer)
                    && l.origin.iter().all(|n| n.unsigned_abs() < 1 << 27)
            })
            && self.layout.as_ref().filter(|p| p.design.is_some()).is_none_or(|p| {
                let expected = p.parcels[0].lot(IVec3::from(self.origin), true);
                self.frontier.is_none() && self.lots.len() == 1 && self.lots.first().is_some_and(|l|
                    l.origin == expected.origin && l.tradition == expected.tradition
                    && l.complexity == expected.complexity && l.turn == expected.turn
                    && l.variant == expected.variant && l.roof_access == expected.roof_access
                    && l.frontage && !l.market_cover)
            })
    }
}

pub(crate) fn install(world: &mut World, player: &mut crate::player::Player) -> Result<(), String> {
    fixture::install(world, player)
}

pub(super) fn service(world: &mut World) {
    let Some(mut city) = world.farming.jobs.quarter.take() else {
        return;
    };
    advance(world, &mut city);
    world.farming.jobs.quarter = Some(city);
}

fn advance(world: &mut World, city: &mut State) {
    if city.frontier.is_some() {frontier::advance(world, city);return;}
    if city.paused {
        city.status = "Projects Paused".into();
        return;
    }
    if super::crew::saved_workers(world).is_empty() {
        city.status = "No Builder".into();
        return;
    }
    if !world.farming.jobs.build.is_empty() || super::crew::cargo(&world.farming.jobs) > 0 {
        city.status = super::status(world);
        return;
    }
    if city.housing_demand == 0 {
        city.status = "Housing Supplied".into();
        return;
    }
    let Some(lot) = city.lots.iter_mut().find(|l| !l.occupied) else {
        city.status = "No Vacant Parcels".into();
        return;
    };
    let rows = match traditions() {
        Ok(rows) => rows,
        Err(e) => {
            city.status = e;
            return;
        }
    };
    let plan = geometry::plan_for_scale(lot, &rows[lot.tradition], world.voxel_scale);
    if let Some(&(first, _)) = plan.first() {
        let first = geometry::at(lot, first);
        let (min, max) = plan.iter().fold((first, first), |(min, max), &(p, _)| {
            let cell = geometry::at(lot, p);
            (min.min(cell), max.max(cell))
        });
        if crate::world::farming::base::overlaps(world, min, max) {
            city.status = "Protected Base".into();
            return;
        }
    }
    let highest = plan.iter().map(|(p, _)| p.y).max().unwrap_or(0);
    if lot.layer > highest {
        // Occupancy is committed only after every expected cell exists.
        if plan
            .iter()
            .all(|(p, b)| super::known(world, geometry::at(lot, *p)) == Some(*b))
        {
            lot.occupied = true;
            city.completed += 1;
            // A completed review building does not spawn four physical residents.
            if !city.layout.as_ref().is_some_and(|p| p.design.is_some()) {
                city.residents += 4;
            }
            city.housing_demand = city.housing_demand.saturating_sub(4);
            city.status = if city.layout.as_ref().is_some_and(|p| p.design.is_some()) {
                "Home Complete"
            } else { "Home Occupied" }.into();
            println!(
                "[city-quarter] home-occupied completed={} residents={}",
                city.completed, city.residents
            );
        } else {
            city.status = "Structure Changed".into();
        }
        return;
    }
    let cells: Vec<_> = plan.into_iter().filter(|(p, _)| p.y == lot.layer).collect();
    if cells.iter().any(|(p, b)| {
        super::known(world, geometry::at(lot, *p)).is_none_or(|old| old != B::Air && old != *b)
    }) {
        city.status = "Parcel Obstructed".into();
        return;
    }
    let mut orders = Vec::new();
    for (p, b) in cells {
        let cell = geometry::at(lot, p);
        if super::known(world, cell) != Some(b) {
            orders.push(super::construction::Order::new(cell, b));
        }
    }
    orders.truncate(super::construction::LIMIT);
    let added = orders.len();
    world.farming.jobs.build.extend(orders);
    world.farming.jobs.reset_task();
    if added == 0 {
        lot.layer += 1;
    }
    city.status = format!("Layer {} ┬╖ {} Blocks", lot.layer, added);
    println!(
        "[city-quarter] layer={} blocks={added} style={}",
        lot.layer, rows[lot.tradition].name
    );
}
