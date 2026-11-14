//! Conservative crust transport and surface evolution during plate motion.
use super::{Crust, Drift, HistoryBudget, Progress, Plates, Mesh, SEA, DRIFT_MYR,
    STEP_MYR, SNAPSHOT_MYR, RIFTED_SIDE, MOTION_CHANGE_AGE_MYR};
use crate::planetary::{coastal_erosion, hydrology, tectonics::{Edge, ProvinceNoise, smooth}};
use engine_core::{cube_sphere::atlas, planet_frame::PlanetAxis};
use glam::DVec3;
use std::sync::OnceLock;
use super::history;
use crate::planet_climate::ClimateBasis;

#[path = "weather_samples.rs"]
mod weather_samples;
use weather_samples::WeatherSamples;

const CONTINENT_KM: f64 = 28.0;
const SOURCE_PLATES: usize = 18;

#[derive(Clone, Default)]
struct Transfer {
    area: f64,
    continental: f64,
    sediment: f64,
    basement: f64,
    buried: f64,
    age: f64,
    strain: f64,
    slip: f64,
    carbonate: f64,
    ancestry: [f64; SOURCE_PLATES],
}

pub(super) fn solve(plates: &mut Plates, progress: &Progress) -> Drift {
    static MESH: OnceLock<Mesh> = OnceLock::new();
    let mesh = MESH.get_or_init(|| Mesh::with_side(RIFTED_SIDE));
    // Reserve one core. Ordered batches keep every result identical to a
    // single worker, so the count only changes wall time.
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get().saturating_sub(1).clamp(1, 12));
    let crust = mesh.directions.iter().map(|&dir| {
        let continental = plates.initial_crust(dir);
        Crust {
            continental,
            continental_km: continental * CONTINENT_KM,
            ocean_age_myr: 100.0,
            lineage: plates.initial_lineage(dir),
            carbonate: plates.at(dir).carbonate,
            ..Crust::default()
        }
    }).collect();
    let historical = crate::planetary::geological_history(plates.config().generator_version);
    let resolved = crate::planetary::resolved_history(plates.config().generator_version);
    let clock = super::cadence::Clock::for_generator(plates.config().generator_version);
    let step_myr = clock.plate_myr;
    let deep = crate::planetary::deep_time(plates.config().generator_version);
    let duration = crate::planetary::history_duration_myr(&plates.config());
    let mut field = Drift { mesh, crust, history: historical.then(|| history::Field::with_duration(mesh.directions.len(), duration)), sea_m: SEA,
        ice: crate::planetary::landscape_cycles(plates.config().generator_version).then(|| super::ice::Ice::new(mesh.directions.len())),
        deep: deep.then(|| super::deep_time::Evolution::new(plates.config().seed)) };
    let warped: Vec<_> = mesh.directions.iter().map(|&dir| plates.warp(dir)).collect();
    let mut transfer = vec![Transfer::default(); mesh.directions.len()];
    let mut runoff = vec![0.0; mesh.directions.len()];
    let mut budget = HistoryBudget::default();
    let weather_samples = WeatherSamples::new(mesh);
    // Seed, Earthlike axis and world-space directions stay fixed for this solve.
    // Land/interior and every evolving climate/height adjustment remain live.
    let climate_basis: Vec<_> = mesh.directions.iter().map(|&dir|
        ClimateBasis::new(plates.config().seed, &PlanetAxis::EARTHLIKE, dir)).collect();
    // Fixed world-space node coordinates and seed, not plate-relative samples.
    // These terms remain valid through splitting, drift and motion relaxation.
    let province_noise = resolved.then(|| mesh.directions.iter()
        .map(|&dir| plates.province_noise(dir)).collect::<Vec<_>>());
    let mut events = Vec::new();
    let initial_continent = continent_volume(&field);
    let mut split_plates = Vec::new();
    let faulted = plates.config().generator_version == crate::planetary::LAYERED_CANYON_GENERATOR;
    if historical { surface_step(&mut field, plates, &weather_samples, &climate_basis, &warped, 0, &mut runoff, &mut budget, workers); }
    publish(&field, plates, progress, &warped, &runoff, 0, budget, initial_continent);
    let mut published_myr = 0;
    for myr in (step_myr..=duration).step_by(usize::from(step_myr)) {
        if !historical && myr == DRIFT_MYR - MOTION_CHANGE_AGE_MYR + STEP_MYR { plates.reorganize(); }
        if myr % SNAPSHOT_MYR == 0 && plates.count() < if deep { 40 } else { 24 } {
            split(plates, &field, &warped, &mut split_plates);
        }
        if deep {
            plates.evolve_continents(mesh, &field.crust, &warped, f64::from(step_myr));
            if myr % 120 == 0 { split_plates.clear(); }
        }
        // Classify each material parcel before advancing the plate sites.
        let owners: Vec<_> = crate::planetary::map_history(warped.len(), workers,
            |i| plates.warped_edge(warped[i]).owner);
        let inverse = plates.advance(super::motion_radians_per_myr(plates.config().generator_version) * f64::from(step_myr));
        // Resolved histories keep plate sites and spins unchanged until the
        // final relax_motion in episodes::evolve. Cache only that phase; the
        // legacy unresolved path relaxes earlier and keeps its direct queries.
        let step_edges = resolved.then(|| crate::planetary::map_history(warped.len(), workers,
            |i| plates.warped_edge(warped[i])));
        // Each parcel spreads over the stencil of its backtraced position.
        // Gather per destination with sources in ascending parcel order, the
        // same float summation order as a serial scatter, so any worker
        // count gives identical bits.
        let n = mesh.directions.len();
        let stencils = crate::planetary::map_history(n, workers,
            |i| mesh.stencil(inverse[owners[i]].conjugate() * mesh.directions[i]));
        let mut start = vec![0usize; n + 1];
        for (ids, _) in &stencils { for &j in ids { start[j + 1] += 1; } }
        for j in 0..n { start[j + 1] += start[j]; }
        let mut cursor = start.clone();
        let mut sources = vec![(0usize, 0usize); start[n]];
        for (i, (ids, _)) in stencils.iter().enumerate() {
            for (k, &j) in ids.iter().enumerate() {
                sources[cursor[j]] = (i, k);
                cursor[j] += 1;
            }
        }
        let incoming = |j: usize| sources[start[j]..start[j + 1]].iter()
            .map(|&(i, k)| (i, mesh.area[i] * stencils[i].1[k]));
        {
            let crust = &field.crust;
            let buried: Vec<f64> = field.history.as_ref().map_or_else(|| vec![0.0; n], |h| crate::planetary::map_history(n, workers,
                |i| (h.records[i].column.thickness() - crust[i].sediment_m).max(0.0)));
            crate::planetary::fill_history(&mut transfer, workers, |j, next| {
                *next = Transfer::default();
                for (i, area) in incoming(j) {
                    let old = &crust[i];
                    next.area += area;
                    next.continental += old.continental_km * area;
                    next.sediment += old.sediment_m * area;
                    next.basement += (old.relief_m - old.sediment_m) * area;
                    next.buried += buried[i] * area;
                    next.age += old.ocean_age_myr * area;
                    next.strain += old.strain * area;
                    next.slip += old.slip_m * area;
                    next.carbonate += old.carbonate * old.continental_km * area;
                    if old.lineage > 0 {
                        next.ancestry[usize::from(old.lineage - 1)] += old.continental_km * area;
                    }
                }
            });
        }
        if let Some(history) = &mut field.history { history.gather_into(workers, |j| incoming(j).collect()); }
        for (i, next) in transfer.iter().enumerate() {
            let area = mesh.area[i];
            let coverage = next.area / area;
            let old_area = next.area.max(1e-12);
            let uncached;
            let edge = if let Some(edges) = &step_edges { &edges[i] } else {
                uncached = plates.warped_edge(warped[i]);
                &uncached
            };
            let [collision, spreading, shear] = edge.stresses().map(|v| v.min(1.0));
            // Diverging parcels expose fresh basalt. Compression thickens
            // the continental column by depositing its transported volume.
            let continental_km = next.continental / area;
            let continental = (continental_km / CONTINENT_KM).clamp(0.0, 1.0);
            let fresh = (1.0 - coverage).clamp(0.0, 1.0);
            let age = ((next.age / old_area + f64::from(step_myr)) * (1.0 - fresh)).min(240.0);
            let strain = (next.strain / old_area + (-coverage.max(0.05).ln()).max(0.0)
                * smooth(0.02, 0.3, spreading)).min(8.0);
            let thickening = (continental_km - CONTINENT_KM).max(0.0);
            let cooling = (1.0 - continental) * 3.5 * ((next.age / old_area).sqrt() - age.sqrt());
            let inherited = next.basement / old_area
                + if historical { next.buried / area - next.buried / old_area } else { 0.0 };
            let deformation = if step_myr == STEP_MYR {
                cooling + collision * continental * 3.0 + thickening.min(20.0) * 0.12
                    + shear * continental * 0.5
            } else {
                cooling + (collision * continental * 3.0 + thickening.min(20.0) * 0.12
                    + shear * continental * 0.5) * clock.relative_plate_step()
            };
            let mut basement = if historical {
                // Bound additional uplift, never discard already deposited lava.
                inherited + deformation.clamp((-85.0 - inherited).min(0.0), (220.0 - inherited).max(0.0))
            } else { (inherited + deformation).clamp(-85.0, 180.0) };
            let inherited_slip = next.slip / old_area;
            let pulse = if faulted { plates.fault_episode(mesh.directions[i], edge, myr) }
                else { 0.0 };
            let basement_after = if historical { basement } else { (basement + pulse).clamp(-85.0, 180.0) };
            let slip_m = inherited_slip + basement_after - basement;
            basement = basement_after;
            let lineage = next.ancestry.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))
                .filter(|(_, volume)| **volume > 1e-9).map_or(0, |(id, _)| id as u16 + 1);
            field.crust[i] = Crust {
                continental, continental_km, ocean_age_myr: age, strain, lineage, slip_m,
                sediment_m: next.sediment / area,
                relief_m: basement + next.sediment / area,
                carbonate: if next.continental > 0.0 { next.carbonate / next.continental } else { 0.0 },
            };
            if let Some(history) = &mut field.history {
                history.finish(i, area, next.area, if resolved { 0.0 } else { f64::from(STEP_MYR) },
                    (-deformation.max(0.0) / 8.0).exp());
            }
        }
        if let Some(history) = &mut field.history { history.commit(); }
        if let Some(history) = &mut field.history.as_mut().filter(|_| !resolved) {
            let mut torque = vec![(DVec3::ZERO, 0.0); plates.count()];
            // Sample both sides before changing any column, so traversal
            // order cannot change a subduction polarity or event size.
            let before = field.crust.clone();
            for (i, &dir) in mesh.directions.iter().enumerate() {
                let edge = plates.warped_edge(warped[i]);
                let across = (dir + edge.normal * (edge.boundary + 0.065)).normalize();
                let (ids, weights) = mesh.stencil(across);
                let mut other = Crust::default();
                for (j, weight) in ids.into_iter().zip(weights) {
                    other.continental += before[j].continental * weight;
                    other.ocean_age_myr += before[j].ocean_age_myr * weight;
                }
                let (slip, lava) = history::events(&mut history.records[i], plates.config().seed,
                    dir, &edge, before[i], other, myr);
                field.crust[i].relief_m += slip + lava;
                field.crust[i].slip_m += slip;
                field.crust[i].sediment_m = history.records[i].mobile(myr);
                if slip.abs() > 0.0 {
                    let force = dir.cross(edge.normal) * slip * mesh.area[i];
                    torque[edge.owner].0 += force;
                    torque[edge.neighbour].0 -= force;
                    torque[edge.owner].1 += mesh.area[i];
                    torque[edge.neighbour].1 += mesh.area[i];
                }
            }
            plates.relax_motion(&torque);
        }
        if deep {
            super::deep_time::advance(&mut field, plates,
                step_edges.as_deref().expect("deep-time history is resolved"), myr, f64::from(step_myr), &mut budget);
        }
        if resolved {
            let interval_events = surface_step_resolved(&mut field, plates, &weather_samples, &climate_basis,
                step_edges.as_deref().expect("resolved phase edges"),
                province_noise.as_deref().expect("resolved province noise"), myr, &mut runoff, &mut budget, workers);
            if deep { events.extend(interval_events); } else { events = interval_events; }
        } else {
            surface_step(&mut field, plates, &weather_samples, &climate_basis, &warped, myr, &mut runoff, &mut budget, workers);
        }
        // Motion has now relaxed. Never use the pre-relax edges for previews
        // or the next interval's ownership, splitting or continental cycle.
        drop(step_edges);
        if (resolved && (!deep || myr % 3 == 0)) || (!resolved && myr % SNAPSHOT_MYR == 0) {
            let mut frame = make_frame(&field, plates, &warped, &runoff, myr, budget, initial_continent);
            frame.motion = interval_motion(plates, myr - published_myr);
            published_myr = myr;
            if deep {
                events.sort_by(|a, b| b.metres.abs().total_cmp(&a.metres.abs()).then_with(|| a.myr.total_cmp(&b.myr)));
                events.truncate(4);
                events.sort_by(|a, b| a.myr.total_cmp(&b.myr));
            }
            frame.events = std::mem::take(&mut events);
            progress.publish_drift(frame);
        }
    }
    if deep {
        // Runtime water uses the fixed playable datum. Translate the completed
        // terrain by the final ice-driven sea offset, preserving every shore
        // and bed thickness instead of flooding the last history frame again.
        let datum = SEA - field.sea_m;
        for crust in &mut field.crust { crust.relief_m += datum; }
        field.sea_m = SEA;
    }
    if let Some(history) = &mut field.history { history.seal(); }
    field
}

fn continent_volume(field: &Drift) -> f64 {
    field.crust.iter().zip(&field.mesh.area).map(|(c, area)| c.continental_km * area / 1e6).sum()
}

fn split(plates: &mut Plates, field: &Drift, warped: &[DVec3], split_plates: &mut Vec<usize>) {
    let candidate = field.crust.iter().zip(warped).filter_map(|(crust, &q)| {
        let edge = plates.warped_edge(q);
        let spreading = edge.stresses()[1];
        let thermal = plates.breakup_axis(q);
        let score = crust.strain * crust.continental * spreading
            + thermal.map_or(0.0, |_| crust.continental * (0.20 + 0.12 * crust.strain));
        (score > 0.16 && (crust.strain > 0.55 || thermal.is_some()) && !split_plates.contains(&edge.owner))
            .then_some((score, edge.owner, thermal.unwrap_or(edge.normal)))
    }).max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
    if let Some((_, owner, across)) = candidate {
        let daughter = plates.count();
        plates.split_strained(owner, across);
        split_plates.extend([owner, daughter]);
    }
}

fn surface_step(field: &mut Drift, plates: &Plates, weather_samples: &WeatherSamples, climate_basis: &[ClimateBasis], warped: &[DVec3], myr: u16,
    runoff: &mut Vec<f64>, budget: &mut HistoryBudget, workers: usize)
{
    let mesh = field.mesh;
    let mut height: Vec<_> = mesh.directions.iter().zip(warped).map(|(&dir, &q)| {
        plates.evolved_at(dir, &plates.warped_edge(q), |d| field.at(d)).height
    }).collect();
    let original = height.clone();
    let settings = plates.config().planet_settings();
    let historical = field.history.is_some();
    let (weather, sea) = weather_at(field, plates, weather_samples, climate_basis, &height, f64::from(myr), workers);
    field.sea_m = sea;
    let rain: Vec<_> = weather.iter().map(|w| w[0]).collect();
    if let Some(history) = &mut field.history {
        for (record, climate) in history.records.iter_mut().zip(&weather) {
            record.wet = climate[1];
            record.temperature = climate[2];
            record.winter_c = climate[3];
        }
    }
    let flow = hydrology::route(mesh, &height, &rain, sea);
    runoff.clone_from(&flow.discharge);
    let mut load = vec![0.0; height.len()];
    let mut carbonate_load = if historical { vec![0.0; height.len()] } else { Vec::new() };
    let mut soil: Vec<_> = field.crust.iter().map(|c| c.sediment_m).collect();
    let strength = if myr == 0 { 0.0 } else { settings.map_or(1.0, |s| 0.25 + s.unit(3) * 1.5) };
    for &i in flow.order.iter().rev() {
        let receiver = flow.receiver[i];
        let area = mesh.area[i];
        if height[i] <= sea || receiver == i {
            soil[i] += load[i] / area;
            height[i] += load[i] / area;
            if let Some(history) = &mut field.history {
                let carbonate = carbonate_load[i] / load[i].max(1e-12);
                history.records[i].deposit(myr, load[i] / area, carbonate,
                    height[i] > sea - 0.5 && flow.spill[i] - height[i] < 2.5);
                soil[i] = history.records[i].mobile(myr);
            }
            continue;
        }
        let distance = mesh.directions[i].distance(mesh.directions[receiver]) * atlas::RADIUS_M;
        let grade = ((flow.spill[i] - flow.spill[receiver]) / distance).max(0.0);
        let pond = (flow.spill[i] - height[i]).max(0.0);
        let power = ((flow.discharge[i] / 320_000.0).sqrt() * grade - 0.007).max(0.0)
            / (1.0 + pond * pond);
        let available_drop = (height[i] - height[receiver]).max(0.0) * 0.30;
        let scour = (2.0 * power * strength).min(soil[i]).min(available_drop);
        soil[i] -= scour;
        let damage = 1.0 + 0.4 * field.crust[i].strain.min(2.0);
        let resistance = field.history.as_ref().map_or(1.0, |h| h.records[i].resistance(myr, field.crust[i].carbonate));
        let cut = (12.0 * strength * power * damage * (-soil[i] / 1.8).exp() / resistance)
            .min((available_drop - scour).max(0.0))
            .min((height[i] - scour - soil[i] - sea).max(0.0)).min(3.0);
        if !historical { budget.eroded_m3 += cut * area; }
        let moving = load[i] + (scour + cut) * area;
        let transport_length = 250.0 + 1600.0 * (flow.discharge[i] / 320_000.0).sqrt()
            * smooth(0.001, 0.03, grade);
        let deposit = moving * (1.0 - (-distance / transport_length).exp());
        soil[i] += deposit / area;
        height[i] += deposit / area - scour - cut;
        if let Some(history) = &mut field.history {
            let (fresh, carbonate) = history.records[i].remove(scour + cut, field.crust[i].carbonate);
            budget.eroded_m3 += fresh * area;
            let carried = carbonate_load[i] + carbonate * area;
            let fraction = (carried / moving.max(1e-12)).clamp(0.0, 1.0);
            history.records[i].deposit(myr, deposit / area, fraction, pond > 0.15 || grade < 0.004);
            carbonate_load[receiver] += fraction * (moving - deposit);
            soil[i] = history.records[i].mobile(myr);
        }
        load[receiver] += moving - deposit;
    }
    let erodibility: Vec<_> = field.crust.iter().map(|c| 0.65 + 0.35 * c.strain.min(2.0)).collect();
    let datum_shift = SEA - sea;
    // Coastal transport and weathering operate relative to the current shore.
    if datum_shift != 0.0 { height.iter_mut().for_each(|h| *h += datum_shift); }
    let coast = if let Some(history) = &mut field.history {
        let coast = coastal_erosion::transport_recorded(mesh, &mut height, &mut soil, &erodibility, strength, 1,
            |i, cut, deposit| {
                budget.eroded_m3 += history.records[i].reshape(myr, cut, deposit, field.crust[i].carbonate, false) * mesh.area[i];
            });
        for (i, record) in history.records.iter().enumerate() { soil[i] = record.mobile(myr); }
        budget.eroded_m3 += history::weather(mesh, &mut history.records, &field.crust,
            &mut height, &mut soil, strength, myr);
        coast
    } else {
        let coast = coastal_erosion::transport(mesh, &mut height, &mut soil, &erodibility, strength, 1);
        budget.eroded_m3 += coast.eroded_m3;
        coast
    };
    budget.alongshore_m3 += coast.alongshore_m3;
    if datum_shift != 0.0 { height.iter_mut().for_each(|h| *h -= datum_shift); }
    for (i, crust) in field.crust.iter_mut().enumerate() {
        let rock_removed = field.history.as_ref().map_or_else(
            || (original[i] - crust.sediment_m - (height[i] - soil[i])).max(0.0),
            |h| h.records[i].basement_cut);
        let removed = (rock_removed * 0.001 * crust.continental).min(crust.continental_km);
        crust.continental_km -= removed;
        budget.continental_eroded_km3 += removed * mesh.area[i] / 1e6;
        crust.relief_m += height[i] - original[i];
        crust.sediment_m = soil[i];
        crust.continental = (crust.continental_km / CONTINENT_KM).clamp(0.0, 1.0);
    }
    if historical && myr % SNAPSHOT_MYR == 0 {
        // The displayed rivers must belong to the published, eroded bed.
        runoff.clone_from(&hydrology::route(mesh, &height, &rain, sea).discharge);
    }
}

fn publish(field: &Drift, plates: &Plates, progress: &Progress, warped: &[DVec3],
    runoff: &[f64], myr: u16, budget: HistoryBudget, initial_continent: f64)
{
    progress.publish_drift(make_frame(field, plates, warped, runoff, myr, budget, initial_continent));
}

/// Per-plate rotation over the last `interval_myr`, composed from the recorded
/// Euler stages; plates born inside the interval follow their parent before it.
/// Lets playback carry crust between frames instead of dissolving one into the next.
fn interval_motion(plates: &Plates, interval_myr: u16) -> Vec<[f32; 3]> {
    if !crate::planetary::resolved_history(plates.config().generator_version) || interval_myr == 0 {
        return Vec::new();
    }
    (0..plates.count()).map(|plate| {
        plates.carried_rotation(plate, f64::from(interval_myr)).to_scaled_axis().as_vec3().to_array()
    }).collect()
}

fn make_frame(field: &Drift, plates: &Plates, warped: &[DVec3],
    runoff: &[f64], myr: u16, mut budget: HistoryBudget, initial_continent: f64) -> super::DriftFrame
{
    budget.plates = plates.count();
    budget.continental_km3 = continent_volume(field);
    budget.continental_error_km3 = initial_continent + budget.continental_created_km3 - budget.continental_km3 - budget.continental_eroded_km3;
    budget.sediment_m3 = field.history.as_ref().map_or_else(
        || field.crust.iter().zip(&field.mesh.area).map(|(c, a)| c.sediment_m * a).sum(),
        |h| h.sediment_volume(field.mesh));
    budget.sediment_error_m3 = budget.eroded_m3 - budget.sediment_m3;
    let mut frame = field.frame(plates, myr, warped);
    if crate::planetary::fine_history(plates.config().generator_version) {
        for (texel, i) in frame.texels.iter_mut().zip(super::cadence::frame_indices(field.mesh)) {
            texel.runoff_m2 = runoff[i] as f32;
        }
    } else {
        for (texel, &i) in frame.texels.iter_mut().zip(field.mesh.face_vertices()) {
            texel.runoff_m2 = runoff[i] as f32;
        }
    }
    frame.budget = Some(budget);
    frame
}

fn weather_at(field: &mut Drift, plates: &Plates, weather_samples: &WeatherSamples, climate_basis: &[ClimateBasis], height: &[f64],
    myr: f64, workers: usize) -> (Vec<[f64; 4]>, f64) {
    let mesh = field.mesh;
    let cfg = plates.config();
    let settings = cfg.planet_settings();
    let profile = crate::planet_profile::profile(cfg.seed);
    let historical = field.history.is_some();
    let energy = crate::planetary::landscape_cycles(cfg.generator_version)
        .then(|| super::climate_cycle::solve(mesh, height, cfg.seed, myr,
            crate::planetary::history_duration_myr(&cfg), field.deep.as_ref().map(|d| d.forcing())));
    let cooling = if energy.is_some() { 0.0 } else if historical {
        let epochs = [8.0, 0.0, 12.0, 2.0, 6.0, 0.0, 0.0];
        let phase = f64::from(myr) / 40.0;
        let age = (phase.floor() as usize).min(5);
        epochs[age] + (epochs[age + 1] - epochs[age]) * smooth(0.0, 1.0, phase - age as f64)
    } else {
        [8.0, 0.0, 12.0, 2.0, 6.0, 0.0][((myr - 1.0).max(0.0) as usize / 40).min(5)]
    };
    // Compact eustatic approximation for the game's vertical scale. Cooling
    // stores water on land; warm epochs return to the present-day datum.
    let sea = if let Some(ice) = &field.ice { ice.sea(mesh, height) }
    else if crate::planetary::changing_seas(cfg.generator_version) {
        SEA - 6.0 * smooth(0.0, 12.0, cooling)
    } else { SEA };
    // Each vertex reads only the completed height/climate snapshot. The helper
    // joins in index order before any ice or deep-climate state is written.
    let weather = crate::planetary::map_history(mesh.directions.len(), workers, |i| {
        let dir = mesh.directions[i];
        let mut climate = climate_basis[i].at_land(((height[i] - SEA) / 200.0).clamp(0.0, 1.0));
        if let Some(s) = settings {
            climate.wet = ((climate.wet - f64::from(profile.rainfall - 0.5) * 0.3)
                * s.unit(5) * 2.0).clamp(0.0, 1.0);
            let warming = (s.unit(4) - 0.5) * 40.0;
            climate.annual_c += warming;
            climate.winter_c += warming;
        }
        let rise = height[i] - weather_samples.sample(height, i, 0);
        let mut wet = (climate.wet + (rise / 320.0).clamp(-0.3, 0.3)).clamp(0.0, 1.0);
        if historical {
            // Continuous climate epochs; drifting shores and ranges determine
            // maritime recharge and rain shadows within each epoch.
            let mut marine = 0.0;
            let mut barrier = height[i];
            for (sample, weight) in [(1, 0.45), (2, 0.35), (3, 0.20)] {
                let upwind = weather_samples.sample(height, i, sample);
                marine += weight * (1.0 - smooth(sea - 4.0, sea + 12.0, upwind));
                barrier = barrier.max(upwind);
            }
            wet = (wet * 0.74 + marine * 0.34 - ((barrier - height[i]) / 550.0).min(0.30)).clamp(0.0, 1.0);
        }
        let anomaly = energy.as_ref().map_or(-cooling, |e| e.anomaly(dir.y));
        let lapse = if energy.is_some() {
            crate::planet_climate::LAPSE_C_PER_M * (height[i] - sea).max(0.0)
        } else { 0.0 };
        let annual = climate.annual_c + anomaly - lapse;
        [wet * (0.25 + 0.75 * smooth(-6.0, 3.0, annual)), wet, annual, climate.winter_c + anomaly - lapse]
    });
    if let (Some(ice), Some(energy)) = (&mut field.ice, &energy) {
        for (i, &[_rain, wet, annual, _winter]) in weather.iter().enumerate() {
            let reference = annual - energy.anomaly(mesh.directions[i].y);
            let amount = |temperature: f64| 32.0 * wet * smooth(-1.0, 13.0, -temperature);
            ice.baseline[i] = if height[i] > SEA { amount(reference) } else { 0.0 };
            ice.target[i] = if height[i] > SEA { (amount(annual) - ice.baseline[i]).max(0.0) } else { 0.0 };
        }
    }
    if let Some(deep) = &mut field.deep { deep.climate = energy; }
    (weather, sea)
}

fn surface_step_resolved(field: &mut Drift, plates: &mut Plates, weather_samples: &WeatherSamples, climate_basis: &[ClimateBasis],
    edges: &[Edge], noise: &[ProvinceNoise], myr: u16,
    runoff: &mut Vec<f64>, budget: &mut HistoryBudget, workers: usize) -> Vec<super::HistoryEvent>
{
    let height = resolved_height(field, plates, edges, noise, workers);
    let weather_time = if crate::planetary::landscape_cycles(plates.config().generator_version) { f64::from(myr) }
        else { f64::from(myr) - f64::from(super::cadence::Clock::for_generator(plates.config().generator_version).plate_myr) * 0.5 };
    let (weather, sea) = weather_at(field, plates, weather_samples, climate_basis, &height, weather_time, workers);
    field.sea_m = sea;
    super::episodes::evolve(field, plates, edges, height, &weather, f64::from(myr), runoff, budget)
}

// This phase has no mutation or global reduction: only ordered vertex results.
fn resolved_height(field: &Drift, plates: &Plates, edges: &[Edge], noise: &[ProvinceNoise],
    workers: usize) -> Vec<f64>
{
    crate::planetary::map_history(field.mesh.directions.len(), workers, |i| {
        let dir = field.mesh.directions[i];
        plates.evolved_at_with_noise(dir, &edges[i], &noise[i], |d| field.at(d)).height
    })
}

#[cfg(test)]
#[path = "parallel_maps_tests.rs"]
mod parallel_maps_tests;
