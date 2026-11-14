//! Multi-rate surface history. Plate intervals are versioned;
//! sediment evolves every 250 kyr and reservoir crossings have exact dates.
//! Repeated, identical cycles are integrated analytically, not year by year.
use super::{Drift, HistoryBudget, HistoryEvent, Plates, history, cadence::Clock};
use crate::planetary::{SEA, coastal_erosion, hydrology, smooth, tectonics::Edge};
use crate::planet::noise_at;
use engine_core::cube_sphere::atlas;
use glam::DVec3;

const SURFACE_STEPS: usize = 20;
const MAX_REROUTES: usize = 3;
const ROUTE_REFRESH_STEPS: usize = 5;
const EVENTS_PER_FRAME: usize = 4;

#[derive(Default)]
struct Cycles { volume: f64, count: usize, first: f64, last: f64 }

/// Exact solution of dV/dt = recharge - leak*V, with a threshold/reset.
/// A changed boundary can inherit an already overfull reservoir; release
/// that excess too. Cycle batching preserves volume, residual and dates.
fn cycles(value: &mut f64, rate: f64, leak: f64, threshold: f64, reset: f64, dt: f64) -> Cycles {
    let advance = |v: f64, t: f64| v * (-leak * t).exp() + rate * -(-leak * t).exp_m1() / leak;
    let crossing = |v: f64| {
        if v >= threshold { 0.0 }
        else if rate <= leak * threshold { f64::INFINITY }
        else { -(-leak * (threshold - v) / (rate - leak * v)).ln_1p() / leak }
    };
    let first = crossing(*value);
    if first > dt {
        *value = advance(*value, dt);
        return Cycles::default();
    }
    let residual = threshold * reset;
    let period = crossing(residual);
    let count = 1 + ((dt - first) / period).floor() as usize;
    let last = if count == 1 { first } else { first + (count - 1) as f64 * period };
    let volume = (*value).max(threshold) - residual + (count - 1) as f64 * (threshold - residual);
    *value = advance(residual, dt - last);
    Cycles { volume, count, first, last }
}

struct Source {
    i: usize, owner: usize, neighbour: usize, normal: DVec3,
    loading: f64, slip: f64, melt: f64, quake_threshold: f64, magma_threshold: f64,
}

fn sources(field: &Drift, plates: &Plates, edges: &[Edge]) -> Vec<Source> {
    let mut sources = Vec::new();
    for (i, &dir) in field.mesh.directions.iter().enumerate() {
        let edge = &edges[i];
        let plume = field.deep.as_ref().and_then(|d| d.melt.get(i)).copied().unwrap_or(0.0);
        if edge.boundary >= 0.065 && plume <= 1e-9 { continue; }
        let [compression, spreading, shear] = edge.stresses().map(|v| v.min(1.0));
        let own = field.crust[i];
        let other = field.at((dir + edge.normal * (edge.boundary + 0.065)).normalize());
        let subducting = (1.0 - own.continental.min(other.continental))
            * smooth(12.0, 100.0, own.ocean_age_myr.max(other.ocean_age_myr));
        let upper = own.continental > other.continental
            || ((own.continental - other.continental).abs() < 0.05 && own.ocean_age_myr < other.ocean_age_myr);
        let key = plates.config().seed ^ (edge.owner.min(edge.neighbour) as u32).wrapping_mul(0x9e37_79b9)
            ^ (edge.owner.max(edge.neighbour) as u32).wrapping_mul(0x85eb_ca6b);
        let along = (dir - edge.normal * dir.dot(edge.normal)).normalize_or_zero();
        let segment = noise_at(along * 31.0, key);
        let arc = if upper { subducting * compression * smooth(0.008, 0.025, edge.boundary) } else { 0.0 };
        sources.push(Source { i, owner: edge.owner, neighbour: edge.neighbour, normal: edge.normal,
            loading: compression + 0.28 * shear,
            slip: (0.55 * compression + 2.0 * subducting * compression)
                * if upper { 1.0 } else { -0.32 } * (1.0 - smooth(0.012, 0.065, edge.boundary)),
            melt: (arc + spreading * (1.0 - 0.65 * own.continental))
                * smooth(0.48, 0.80, noise_at(along * 67.0, key.wrapping_add(83))) + plume,
            quake_threshold: 0.0012 + 0.0048 * segment,
            magma_threshold: 0.012 + 0.068 * segment,
        });
    }
    sources
}

// One strongest event per time stratum, alternating fault/eruption. This
// bounds preview memory without quantising the event's actual occurrence.
fn retain(events: &mut [Option<HistoryEvent>], event: HistoryEvent, start: f64, interval: f64) {
    let bin = (((event.myr - start) / interval * (EVENTS_PER_FRAME / 2) as f64) as usize)
        .min(EVENTS_PER_FRAME / 2 - 1) * 2 + usize::from(event.eruption);
    if events[bin].is_none_or(|old| event.metres.abs() > old.metres.abs()) { events[bin] = Some(event); }
}

pub(super) fn evolve(field: &mut Drift, plates: &mut Plates, edges: &[Edge], mut height: Vec<f64>,
    weather: &[[f64; 4]], end: f64, runoff: &mut Vec<f64>, budget: &mut HistoryBudget) -> Vec<HistoryEvent>
{
    let mesh = field.mesh;
    let clock = Clock::for_generator(plates.config().generator_version);
    let surface_steps = clock.surface_steps;
    let interval = f64::from(clock.plate_myr);
    // All surface operators share the same marine datum for this interval.
    // Relief is accumulated as a difference in this same coordinate system.
    let datum_shift = SEA - field.sea_m;
    if datum_shift != 0.0 { height.iter_mut().for_each(|h| *h += datum_shift); }
    let mut original = height.clone();
    // This is the last read of the post-advance edge snapshot. Source values
    // remain local to this interval; relax_motion below invalidates the edges.
    let sources = sources(field, plates, edges);
    let records = &mut field.history.as_mut().expect("resolved history columns").records;
    let rain: Vec<_> = weather.iter().map(|w| w[0]).collect();
    for (r, &[_rain, wet, annual, winter]) in records.iter_mut().zip(weather) {
        r.wet = wet;
        r.temperature = annual;
        r.winter_c = winter;
        r.basement_cut = 0.0;
    }
    let mut soil: Vec<_> = field.crust.iter().map(|c| c.sediment_m).collect();
    let mut flow = hydrology::route(mesh, &height, &rain, SEA);
    let mut load = vec![0.0; height.len()];
    let mut carbonate = vec![0.0; height.len()];
    let mut momentum = vec![DVec3::ZERO; height.len()];
    let mut weathering = Weathering::new(height.len());
    let erodibility: Vec<_> = field.crust.iter().map(|c| 0.65 + 0.35 * c.strain.min(2.0)).collect();
    let mut torque = vec![(DVec3::ZERO, 0.0); plates.count()];
    let mut selected = vec![None; EVENTS_PER_FRAME];
    let dt = interval / surface_steps as f64;
    let start = end - interval;
    let strength = plates.config().planet_settings().map_or(1.0, |s| 0.25 + s.unit(3) * 1.5);
    let fraction = strength / SURFACE_STEPS as f64;
    let mut reroutes = 0;
    let mut displaced = false;
    let eroded_before = budget.eroded_m3;
    let mut glacial_rock_m3 = 0.0;
    for step in 0..surface_steps {
        let now = start + step as f64 * dt;
        // Same factors as evaluating each exponential per record.
        let (relax, leak) = ((-dt / 0.5).exp(), (-0.002 * dt).exp());
        for record in records.iter_mut() {
            record.weathered_myr += dt;
            record.quake_m *= relax;
            record.eruption_m *= relax;
            record.locked *= leak;
            record.magma *= leak;
        }
        for s in &sources {
            let r = &mut records[s.i];
            let quake = cycles(&mut r.locked, s.loading, 0.024, s.quake_threshold, 0.0, dt);
            let eruption = cycles(&mut r.magma, s.melt, 0.006, s.magma_threshold, 0.18, dt);
            let slip = quake.volume * s.slip;
            let lava = eruption.volume * 3.0;
            height[s.i] += slip + lava;
            field.crust[s.i].slip_m += slip;
            r.quake_m += slip;
            r.eruption_m += lava;
            r.weathered_myr *= (-slip.max(0.0) / 8.0).exp();
            let dir = mesh.directions[s.i];
            if quake.count > 0 && slip.abs() > 1e-8 {
                retain(&mut selected, HistoryEvent { myr: now + quake.last, direction: dir.as_vec3().to_array(),
                    metres: (slip / quake.count as f64) as f32, eruption: false }, start, interval);
                let force = dir.cross(s.normal) * slip * mesh.area[s.i];
                torque[s.owner].0 += force; torque[s.neighbour].0 -= force;
                torque[s.owner].1 += mesh.area[s.i]; torque[s.neighbour].1 += mesh.area[s.i];
            }
            if eruption.count > 0 {
                // Preserve the last cap and the mass-weighted date of earlier
                // identical eruptions, rather than dating all lava at step end.
                let last_lava = (s.magma_threshold * 0.82 * 3.0).min(lava);
                if eruption.count > 1 {
                    let period = (eruption.last - eruption.first) / (eruption.count - 1) as f64;
                    r.column.deposit(now + (eruption.first + eruption.last - period) * 0.5,
                        lava - last_lava, 0.0, 0.0, 0.0, true, false);
                }
                r.column.deposit(now + eruption.last, if eruption.count == 1 { lava } else { last_lava },
                    0.0, 0.0, 0.0, true, false);
                r.weathered_myr = dt - eruption.last;
                retain(&mut selected, HistoryEvent { myr: now + eruption.last, direction: dir.as_vec3().to_array(),
                    metres: last_lava as f32, eruption: true }, start, interval);
            }
            soil[s.i] = r.mobile(now + dt);
        }
        // Reuse the DAG; refresh its spill levels after each bed update. Only
        // changed catchment connections need another expensive priority flood.
        if displaced && reroutes < MAX_REROUTES && step % ROUTE_REFRESH_STEPS == 0 {
            flow = hydrology::route(mesh, &height, &rain, SEA);
            reroutes += 1;
            displaced = false;
        }
        let track_displacement = reroutes < MAX_REROUTES
            && (step / ROUTE_REFRESH_STEPS + 1) * ROUTE_REFRESH_STEPS < surface_steps;
        for &i in &flow.order {
            let r = flow.receiver[i];
            flow.spill[i] = if r == i { height[i] } else { height[i].max(flow.spill[r]) };
        }
        load.fill(0.0); carbonate.fill(0.0); momentum.fill(DVec3::ZERO);
        for &i in flow.order.iter().rev() {
            let receiver = flow.receiver[i];
            let area = mesh.area[i];
            if height[i] <= SEA || receiver == i {
                let deposited = load[i] / area;
                records[i].deposit(now + dt, deposited, carbonate[i] / load[i].max(1e-12), height[i] > SEA - 0.5);
                height[i] += deposited;
                soil[i] = records[i].mobile(now + dt);
                continue;
            }
            let delta = mesh.directions[receiver] - mesh.directions[i];
            let distance = delta.length() * atlas::RADIUS_M;
            let direction = delta.normalize_or_zero();
            let grade = ((flow.spill[i] - flow.spill[receiver]) / distance).max(0.0);
            let pond = (flow.spill[i] - height[i]).max(0.0);
            let power = ((flow.discharge[i] / 320_000.0).sqrt() * grade - 0.007).max(0.0) / (1.0 + pond * pond);
            let drop = (height[i] - height[receiver]).max(0.0) * 0.2;
            let stripping_capacity = 2.0 * power * fraction;
            let rock_time = if crate::planetary::fine_history(plates.config().generator_version) {
                crate::planetary::erosion_clock::exposed_time(soil[i], stripping_capacity)
            } else { 1.0 };
            let scour = stripping_capacity.min(soil[i]).min(drop);
            soil[i] -= scour;
            let exposed = (-soil[i] / 0.75).exp();
            let resistance = records[i].resistance(now + dt, field.crust[i].carbonate);
            // Sediment tools abrade only exposed bed. Keep over-capacity load:
            // deposition and cover, not a flux clamp, control its effect.
            let tools = (load[i] / (area * fraction.max(1e-12))).sqrt();
            let damage = 1.0 + 0.4 * field.crust[i].strain.min(2.0);
            let cut = (fraction * exposed * damage / resistance * (4.2 * power + 1.8 * tools * power.sqrt()) * rock_time)
                .min((drop - scour).max(0.0)).min((height[i] - soil[i] - scour - SEA).max(0.0)).min(0.3);
            let (fresh, carbon) = records[i].remove(scour + cut, field.crust[i].carbonate);
            budget.eroded_m3 += fresh * area;
            let moving = load[i] + (scour + cut) * area;
            let mixture = ((carbonate[i] + carbon * area) / moving.max(1e-12)).clamp(0.0, 1.0);
            let alignment = if load[i] > 1e-9 { momentum[i].normalize_or_zero().dot(direction).max(0.0) } else { 1.0 };
            let reach = (250.0 + 1600.0 * (flow.discharge[i] / 320_000.0).sqrt() * smooth(0.001, 0.03, grade))
                * (0.35 + 0.65 * alignment);
            let deposited = moving * -(-distance / reach).exp_m1();
            records[i].deposit(now + dt, deposited / area, mixture, pond > 0.15 || grade < 0.004);
            height[i] += deposited / area - scour - cut;
            soil[i] = records[i].mobile(now + dt);
            let outgoing = moving - deposited;
            load[receiver] += outgoing;
            carbonate[receiver] += outgoing * mixture;
            momentum[receiver] += direction * outgoing;
            // Only a future route refresh can observe this flag. Once true,
            // additional pure neighbor scans cannot change that decision.
            if track_displacement && !displaced {
                displaced = height[i] < height[receiver] || mesh.neighbours[i].iter().any(|&(j, d)| {
                    j != receiver && (height[i] - height[j]) / d > grade * 1.4 + 0.002
                });
            }
        }
        budget.eroded_m3 += weathering.advance(mesh, records, &field.crust, &mut height, &mut soil, fraction, now + dt);
        if let Some(ice) = &mut field.ice {
            let eroded = ice.advance(mesh, &flow, records, &field.crust, &mut height, &mut soil, fraction, dt, now + dt);
            budget.eroded_m3 += eroded;
            glacial_rock_m3 += eroded;
            let sea = ice.sea(mesh, &height);
            let shift = field.sea_m - sea;
            // Height and its initial reference share the changing marine
            // coordinate frame. A water-level change must not create rock.
            if shift != 0.0 {
                for (h, old) in height.iter_mut().zip(&mut original) { *h += shift; *old += shift; }
            }
            field.sea_m = sea;
            displaced |= shift.abs() > 0.02 || eroded > 0.0;
        }
        // Coastal exchange needs the evolving shoreline, but not a new
        // global drainage solve. Four finite-volume sweeps per plate step.
        if step % clock.coast_steps == clock.coast_steps - 1 {
            let coast = coastal_erosion::transport_recorded(mesh, &mut height, &mut soil, &erodibility, strength * clock.coast_fraction(), 1,
                |i, cut, deposit| {
                    budget.eroded_m3 += records[i].reshape(now + dt, cut, deposit, field.crust[i].carbonate, false) * mesh.area[i];
                });
            budget.alongshore_m3 += coast.alongshore_m3;
            for (i, r) in records.iter().enumerate() { soil[i] = r.mobile(now + dt); }
        }
    }
    for (i, crust) in field.crust.iter_mut().enumerate() {
        let removed = (records[i].basement_cut * 0.001 * crust.continental).min(crust.continental_km);
        crust.continental_km -= removed;
        budget.continental_eroded_km3 += removed * mesh.area[i] / 1e6;
        crust.continental = (crust.continental_km / 28.0).clamp(0.0, 1.0);
        crust.relief_m += height[i] - original[i];
        crust.sediment_m = soil[i];
    }
    plates.relax_motion(&torque);
    runoff.clone_from(&hydrology::route(mesh, &height, &rain, SEA).discharge);
    if field.ice.is_some() {
        eprintln!("[erosion-epoch] myr={end:.2} surface_steps={surface_steps} rock_removed_m3={:.3} glacial_rock_m3={glacial_rock_m3:.3} sea_m={:.4}",
            budget.eroded_m3 - eroded_before, field.sea_m);
    }
    let mut events: Vec<_> = selected.into_iter().flatten().collect();
    events.sort_by(|a, b| a.myr.total_cmp(&b.myr));
    events
}

/// Scratch is reused across all short surface steps. Remove every source
/// before depositing any arrival, retaining the source's carbonate fraction.
struct Weathering { moved: Vec<f64>, carbonate: Vec<f64>, cuts: Vec<f64>, edges: Vec<(usize, f64)> }

impl Weathering {
    fn new(n: usize) -> Self {
        Self { moved: vec![0.0; n], carbonate: vec![0.0; n], cuts: vec![0.0; n], edges: Vec::with_capacity(8) }
    }

    fn advance(&mut self, mesh: &super::Mesh, records: &mut [history::Record], crust: &[super::Crust],
        height: &mut [f64], soil: &mut [f64], strength: f64, myr: f64) -> f64
    {
        self.moved.fill(0.0); self.carbonate.fill(0.0); self.cuts.fill(0.0);
        let mut rock = 0.0;
        for i in 0..height.len() {
            if height[i] <= SEA { continue; }
            self.edges.clear();
            let old = smooth(8.0, 120.0, records[i].weathered_myr);
            let wet = records[i].wet;
            // Fluvial refreshed soil at this myr. Earlier weathering sources
            // changed only their own columns; arrivals are still scratch-only.
            let resistance = records[i].resistance_with_mobile(myr, crust[i].carbonate, soil[i]);
            for &(j, distance) in &mesh.neighbours[i] {
                let drop = height[i] - height[j];
                if drop <= 0.0 { continue; }
                let grade = drop / distance;
                let creep = (0.08 + 0.85 * old) * wet * grade;
                let failure = (grade - (0.72 - 0.25 * old)).max(0.0) * distance * 0.10;
                let cut = ((creep + failure) * strength / resistance).min(drop * 0.08).min(2.5)
                    / mesh.neighbours[i].len() as f64;
                self.cuts[i] += cut;
                self.edges.push((j, cut * mesh.area[i]));
            }
            let (fresh, carbon) = records[i].remove(self.cuts[i], crust[i].carbonate);
            rock += fresh * mesh.area[i];
            let mixture = carbon / self.cuts[i].max(1e-12);
            for &(j, volume) in &self.edges {
                self.moved[j] += volume;
                self.carbonate[j] += volume * mixture;
            }
        }
        for i in 0..height.len() {
            // Fluvial updated every soil value at this same myr immediately
            // before this pass. Untouched columns retain that exact query.
            if self.cuts[i] != 0.0 || self.moved[i] != 0.0 {
                records[i].deposit(myr, self.moved[i] / mesh.area[i], self.carbonate[i] / self.moved[i].max(1e-12), false);
                soil[i] = records[i].mobile(myr);
            }
            height[i] += self.moved[i] / mesh.area[i] - self.cuts[i];
        }
        rock
    }
}

#[cfg(test)]
mod weathering_query_tests {
    use super::*;

    #[test]
    fn weathering_soil_queries_remain_exact_through_age_boundaries() {
        let mesh = crate::planetary::topology::Mesh::with_side(4);
        let n = mesh.directions.len();
        for duration in [240, 720] {
            let mut records = history::Field::with_duration(n, duration).records;
            let crust = vec![super::super::Crust { carbonate: 0.25, ..Default::default() }; n];
            let mut height: Vec<_> = (0..n).map(|i| SEA + if i % 3 == 0 { -3.0 } else { (i % 11) as f64 * 0.2 }).collect();
            for (i, record) in records.iter_mut().enumerate() {
                record.wet = 0.6;
                record.temperature = 12.0;
                record.column.deposit(10.0, 0.5 + (i % 4) as f64, 0.6, 12.0, 0.25, i % 5 == 0, false);
            }
            let mut weathering = Weathering::new(n);
            for myr in [29.9, 30.0, 30.1] {
                // Match the caller contract: fluvial has refreshed every soil
                // query at this exact date before weathering starts.
                let mut soil: Vec<_> = records.iter().map(|r| r.mobile(myr)).collect();
                weathering.advance(&mesh, &mut records, &crust, &mut height, &mut soil, 0.05, myr);
                for (i, record) in records.iter().enumerate() {
                    assert_eq!(soil[i].to_bits(), record.mobile(myr).to_bits(), "duration={duration}, myr={myr}, node={i}");
                }
            }
        }
    }
    #[test]
    fn submerged_untouched_columns_retain_exact_soil_queries() {
        let mesh = crate::planetary::topology::Mesh::with_side(4);
        let n = mesh.directions.len();
        for duration in [240, 720] {
            let mut records = history::Field::with_duration(n, duration).records;
            let crust = vec![super::super::Crust::default(); n];
            let mut height = vec![SEA - 3.0; n];
            for (i, record) in records.iter_mut().enumerate() {
                record.column.deposit(10.0, 1.0, 0.6, 12.0, 0.25, i % 2 == 0, false);
            }
            let mut weathering = Weathering::new(n);
            for myr in [29.9, 30.0, 30.1] {
                let mut soil: Vec<_> = records.iter().map(|r| r.mobile(myr)).collect();
                weathering.advance(&mesh, &mut records, &crust, &mut height, &mut soil, 0.05, myr);
                assert!(weathering.cuts.iter().all(|&v| v == 0.0));
                assert!(weathering.moved.iter().all(|&v| v == 0.0));
                for (i, record) in records.iter().enumerate() {
                    assert_eq!(soil[i].to_bits(), record.mobile(myr).to_bits());
                    assert_eq!(height[i].to_bits(), (SEA - 3.0).to_bits());
                }
            }
        }
    }
}
