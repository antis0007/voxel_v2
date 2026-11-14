//! Priority flood and stream-power erosion on a closed, connected sphere.
use super::{epochs, topology::Mesh};
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

#[derive(Clone, Copy, PartialEq)]
struct Key(f64, usize);
impl Eq for Key {}
impl PartialOrd for Key { fn partial_cmp(&self, b: &Self) -> Option<Ordering> { Some(self.cmp(b)) } }
impl Ord for Key {
    fn cmp(&self, b: &Self) -> Ordering { self.0.total_cmp(&b.0).then_with(|| self.1.cmp(&b.1)) }
}

pub struct Flow {
    pub receiver: Vec<usize>,
    pub spill: Vec<f64>,
    pub discharge: Vec<f64>,
    pub order: Vec<usize>,
}

#[derive(Default)]
struct FloodScratch {
    seen: Vec<bool>,
    heap: BinaryHeap<Reverse<Key>>,
}

impl Flow {
    fn new(count: usize) -> Self {
        Self { receiver: vec![0; count], spill: vec![0.0; count],
            discharge: vec![0.0; count], order: Vec::with_capacity(count) }
    }
}

fn outlets(height: &[f64], sea: f64) -> Vec<Key> {
    let mut ocean: Vec<_> = height.iter().enumerate().filter(|(_, h)| **h <= sea)
        .map(|(i, &h)| Key(h, i)).collect();
    ocean.sort_unstable();
    ocean
}

pub fn route(mesh: &Mesh, height: &[f64], rain: &[f64], sea: f64) -> Flow {
    route_landscape(mesh, height, rain, sea, false)
}

pub(super) fn route_landscape(mesh: &Mesh, height: &[f64], rain: &[f64], sea: f64, distributed: bool) -> Flow {
    let mut flow = Flow::new(height.len());
    let runoff: Vec<_> = mesh.area.iter().zip(rain).map(|(a, r)| a * r.max(0.0)).collect();
    route_into(mesh, height, &runoff, sea, &outlets(height, sea), &mut flow, &mut FloodScratch::default());
    if distributed {
        spread_hillslope_runoff(mesh, height, &runoff, &mut flow);
    }
    flow
}

/// Share shallow hillslope runoff among downhill exits, then deliver
/// concentrated channel flow along its principal receiver.
fn spread_hillslope_runoff(mesh: &Mesh, height: &[f64], runoff: &[f64], flow: &mut Flow) {
    flow.discharge.copy_from_slice(runoff);
    for &i in flow.order.iter().rev() {
        let r = flow.receiver[i];
        if r == i { continue; }
        let q = flow.discharge[i];
        let diffuse = (1.0 - super::smooth(70_000.0, super::alluvium::SOURCE_AREA, q))
            * (1.0 - super::smooth(0.1, 0.75, flow.spill[i] - height[i]));
        if diffuse <= 0.0 {
            flow.discharge[r] += q;
            continue;
        }
        let weight = |j: usize, distance: f64| {
            ((flow.spill[i] - flow.spill[j]) / distance).max(0.0).powi(2)
        };
        let total: f64 = mesh.neighbours[i].iter().map(|&(j, d)| weight(j, d)).sum();
        if total <= 0.0 {
            flow.discharge[r] += q;
            continue;
        }
        flow.discharge[r] += q * (1.0 - diffuse);
        for &(j, d) in &mesh.neighbours[i] {
            flow.discharge[j] += q * diffuse * weight(j, d) / total;
        }
    }
}

fn route_into(mesh: &Mesh, height: &[f64], runoff: &[f64], sea: f64, ocean: &[Key],
    flow: &mut Flow, scratch: &mut FloodScratch) {
    let Flow { receiver, spill, discharge, order } = flow;
    let FloodScratch { seen, heap } = scratch;
    seen.resize(height.len(), false);
    seen.fill(false);
    heap.clear();
    order.clear();
    spill.copy_from_slice(height);
    discharge.copy_from_slice(runoff);
    for (i, receiver) in receiver.iter_mut().enumerate() { *receiver = i; }

    // Every sea outlet precedes every land vertex. Visiting them in heap
    // order up front preserves ties while avoiding repeated ocean heap work.
    for &Key(_, i) in ocean { seen[i] = true; }
    for &Key(level, i) in ocean {
        order.push(i);
        for &(j, _) in &mesh.neighbours[i] {
            if seen[j] { continue; }
            seen[j] = true;
            receiver[j] = i;
            heap.push(Reverse(Key(height[j].max(level), j)));
        }
    }
    // A completely dry recipe still has a well-defined terminal depression.
    if ocean.is_empty() && !height.is_empty() {
        let i = (0..height.len()).min_by(|&a, &b| height[a].total_cmp(&height[b]).then(a.cmp(&b))).unwrap();
        seen[i] = true;
        heap.push(Reverse(Key(height[i], i)));
    }
    while let Some(Reverse(Key(level, i))) = heap.pop() {
        order.push(i);
        for &(j, _) in &mesh.neighbours[i] {
            if seen[j] { continue; }
            seen[j] = true;
            spill[j] = height[j].max(level);
            receiver[j] = i;
            heap.push(Reverse(Key(spill[j], j)));
        }
    }
    // A strictly lower spill level was popped earlier. On flats retain the
    // original flood parent, which is earlier even at an equal spill level.
    for &i in order.iter().skip(ocean.len()) {
        if height[i] <= sea || receiver[i] == i { continue; }
        let mut slope = 0.0;
        for &(j, distance) in &mesh.neighbours[i] {
            let drop = (spill[i] - spill[j]) / distance;
            if drop > slope { receiver[i] = j; slope = drop; }
        }
    }
    for &i in order.iter().rev() {
        if receiver[i] != i { discharge[receiver[i]] += discharge[i]; }
    }
}

/// Incise shallow, well-fed spillways before using the flood surface as water
/// elevation. A cut must reach lower ground within a short downstream path.
pub(super) fn breach_shallow_basins(
    mesh: &Mesh,
    height: &mut [f64],
    flow: &Flow,
    erodibility: &[f64],
    sediment: &mut [f64],
    sea: f64,
) -> usize {
    let none = height.len();
    let mut outlet = vec![none; height.len()];
    let mut floor = height.to_vec();
    let mut area = vec![0.0; height.len()];
    for &i in &flow.order {
        let r = flow.receiver[i];
        if r == i || height[i] <= sea || flow.spill[i] - height[i] <= 0.05 {
            continue;
        }
        let rim = if outlet[r] != none && flow.spill[r] == flow.spill[i] { outlet[r] } else { r };
        outlet[i] = rim;
        floor[rim] = floor[rim].min(height[i]);
        area[rim] += mesh.area[i];
    }
    let mut bed = height.to_vec();
    let mut breaches = 0;
    for &rim in &flow.order {
        let depth = height[rim] - floor[rim];
        if !(0.75..=8.0).contains(&depth) || area[rim] == 0.0
            || flow.discharge[rim] < area[rim] * 0.16
        {
            continue;
        }
        let target = (floor[rim] + 0.2).max(sea + 0.1);
        let mut path = [rim; 4];
        let mut next = rim;
        let mut connected = false;
        let mut count = 0;
        for i in &mut path {
            let cut = height[next] - target;
            if cut <= 0.0 {
                connected = true;
                break;
            }
            if cut > 5.5 * erodibility[next] || next == flow.receiver[next] {
                break;
            }
            *i = next;
            count += 1;
            next = flow.receiver[next];
        }
        if connected && count > 0 {
            for &i in &path[..count] {
                bed[i] = bed[i].min(target);
            }
            breaches += 1;
        }
    }
    if breaches > 0 {
        for i in 0..height.len() {
            if let Some(load) = sediment.get_mut(i) {
                *load += (height[i] - bed[i]) * mesh.area[i];
            }
            height[i] = bed[i];
        }
    }
    breaches
}

pub fn erode(
    mesh: &Mesh, height: &mut [f64], rain: &[f64], erodibility: &[f64],
    sea: f64, strength: f64, talus: bool, sediment: &mut [f64], coupled: bool,
    mut history: Option<epochs::History<'_>>,
) -> Option<Vec<f64>> {
    let mut moved = if talus { vec![0.0; height.len()] } else { Vec::new() };
    // Erosion keeps land above sea level and never moves ocean vertices.
    let ocean = outlets(height, sea);
    let runoff: Vec<_> = mesh.area.iter().zip(rain).map(|(a, r)| a * r.max(0.0)).collect();
    let mut flow = Flow::new(height.len());
    let mut scratch = FloodScratch::default();
    let mut transport = coupled.then(|| Transport::new(height.len()));
    for pass in 0..18 {
        if let (Some(history), Some(transport)) = (&mut history, &mut transport) {
            history.set_runoff(mesh, height, sea, pass);
            route_into(mesh, height, &history.runoff, sea, &ocean, &mut flow, &mut scratch);
            if pass % epochs::PASSES_PER_EPOCH == 0 && history.reshape(mesh, height, &flow,
                erodibility, sea, pass / epochs::PASSES_PER_EPOCH, &mut transport.source)
            {
                for (load, feed) in sediment.iter_mut().zip(&transport.source) { *load += feed; }
                history.set_runoff(mesh, height, sea, pass);
                route_into(mesh, height, &history.runoff, sea, &ocean, &mut flow, &mut scratch);
            }
        } else {
            route_into(mesh, height, &runoff, sea, &ocean, &mut flow, &mut scratch);
        }
        for &i in flow.order.iter().skip(ocean.len()) {
            let r = flow.receiver[i];
            if r == i || height[i] <= sea || height[i] < flow.spill[i] - 0.5 { continue; }
            let distance = (mesh.directions[i] - mesh.directions[r]).length() * engine_core::cube_sphere::atlas::RADIUS_M;
            let mut k = strength * erodibility[i] * flow.discharge[i].sqrt() / distance * 0.018;
            if let Some(history) = &history {
                let cover = history.cover[i];
                let scour = (flow.discharge[i] / mesh.area[i] / 4.0).clamp(0.0, 1.0);
                k *= (0.55 + 0.45 * scour) / (1.0 + 0.22 * cover * (1.0 - 0.75 * scour));
            }
            let mut next = ((height[i] + k * height[r]) / (1.0 + k)).max(sea + 0.1);
            if coupled { next = next.min(height[i]); }
            if !sediment.is_empty() || coupled {
                let volume = (height[i] - next).max(0.0) * mesh.area[i];
                if let Some(sediment) = sediment.get_mut(i) { *sediment += volume; }
                if let Some(transport) = &mut transport { transport.source[i] += volume; }
            }
            if let Some(history) = &mut history {
                history.cover[i] = (history.cover[i] - (height[i] - next)).max(0.0);
            }
            height[i] = next;
        }
        if let Some(transport) = &mut transport {
            if pass % epochs::PASSES_PER_EPOCH == 1 && let Some(history) = &mut history {
                history.flood_cut(mesh, height, &flow, erodibility, sea, &mut transport.source, sediment);
            }
            transport.settle(mesh, height, &flow, sea, ocean.len());
        }
        if talus && pass % 2 == 0 {
            relax_slopes(mesh, height, erodibility, sea + 2.0, strength, &mut moved);
        }
    }
    if let Some(history) = &history { history.report(); }
    transport.map(Transport::finish)
}

struct Transport {
    source: Vec<f64>,
    incoming: Vec<f64>,
    coast: Vec<f64>,
    eroded: f64,
    deposited: f64,
}

/// Erode the assembled terrain and keep the mobile bed separate from rock.
/// Routing is rebuilt after each exchange so the next event follows the
/// watershed that the previous event actually left behind.
pub(super) fn evolve_watersheds(
    mesh: &Mesh, height: &mut [f64], rain: &[f64], erodibility: &[f64],
    sea: f64, strength: f64, sediment: &mut [f64], distributed: bool,
    beds: Option<&[super::strata::Column]>,
) -> Vec<f64> {
    let ocean = outlets(height, sea);
    let runoff: Vec<_> = mesh.area.iter().zip(rain).map(|(&area, &r)| area * r.max(0.0)).collect();
    let mut rock = height.to_vec();
    let mut cover = vec![0.0; height.len()];
    let mut load = vec![0.0_f64; height.len()];
    let mut floodplain = vec![0.0; height.len()];
    let mut export = vec![0.0; height.len()];
    let mut flow = Flow::new(height.len());
    let mut scratch = FloodScratch::default();
    route_into(mesh, height, &runoff, sea, &ocean, &mut flow, &mut scratch);
    let original = flow.receiver.clone();
    let mut eroded = 0.0;
    let mut overbank = 0.0;
    let mut basin_store = 0.0;
    for event in 0..if distributed { 24 } else { 14 } {
        if distributed {
            spread_hillslope_runoff(mesh, height, &runoff, &mut flow);
        }
        let wet_event = if distributed { [0.78, 1.0, 1.33, 0.89][event % 4] } else { 1.0 };
        load.fill(0.0);
        floodplain.fill(0.0);
        for &i in flow.order[ocean.len()..].iter().rev() {
            let r = flow.receiver[i];
            let area = mesh.area[i];
            if r == i {
                // A landlocked terminal basin stores incoming material.
                let deposit = load[i].min(area * 2.5);
                cover[i] += deposit / area;
                basin_store += load[i] - deposit;
                continue;
            }
            let distance = mesh.directions[i].distance(mesh.directions[r])
                * engine_core::cube_sphere::atlas::RADIUS_M;
            let grade = ((flow.spill[i] - flow.spill[r]) / distance).max(0.0);
            let depth = (flow.spill[i] - height[i] - 0.5).max(0.0);
            let q = flow.discharge[i];
            let power = ((q * wet_event / 320_000.0).sqrt() * grade - 0.007).max(0.0);
            let flowing = 1.0 / (1.0 + depth * depth);
            let scour = (3.0 * strength * power * flowing).min(cover[i]);
            cover[i] -= scour;
            let exposed = (-cover[i] / 1.8).exp();
            let resistance = beds.map_or(1.0, |columns| columns[i].resistance(rock[i]));
            let rock_cut = (15.0 * strength * power * erodibility[i] * exposed * flowing / resistance)
                .min((height[i] - height[r]).max(0.0) * 0.38)
                .min((rock[i] - sea - 0.20).max(0.0))
                .min(7.0);
            rock[i] -= rock_cut;
            let fresh = rock_cut * area;
            eroded += fresh;
            sediment[i] += fresh;
            let moving = load[i] + (scour + rock_cut) * area;
            let trap = depth / (depth + 3.0);
            let low_grade = 1.0 - super::smooth(0.0015, 0.019, grade);
            let deposit = (moving * (0.055 * low_grade + 0.63 * trap)).min(area * 1.4);
            cover[i] += deposit / area;
            let mut leaving = moving - deposit;

            // Only the low, connected banks receive overbank sediment.
            // High valley sides and other active channels stay outside the
            // floodplain even when they are near the river in plan view.
            let bankfull = super::smooth(500_000.0, 7_500_000.0, q * wet_event) * low_grade * flowing;
            if bankfull > 0.0 && leaving > 0.0 {
                let water_rise = 1.2 + 5.8 * bankfull;
                let neighbours = &mesh.neighbours[i];
                let bank_weight = |j: usize| {
                    let rise = height[j] - height[i];
                    if height[j] <= sea || j == r || rise < 0.0
                        || flow.receiver[j] != i || flow.discharge[j] >= 0.6 * q
                    {
                        return 0.0;
                    }
                    1.0 - super::smooth(0.25, water_rise, rise)
                };
                let total: f64 = neighbours.iter().map(|&(j, _)| bank_weight(j)).sum();
                if total > 0.0 {
                    let available = leaving * 0.26 * bankfull;
                    for &(j, _) in neighbours {
                        let volume = (available * bank_weight(j) / total).min(mesh.area[j] * 0.32);
                        floodplain[j] += volume;
                        leaving -= volume;
                        overbank += volume;
                    }
                }
            }
            if height[r] <= sea { export[i] += leaving; } else { load[r] += leaving; }
        }
        for i in 0..height.len() {
            cover[i] += floodplain[i] / mesh.area[i];
            height[i] = rock[i] + cover[i];
        }
        route_into(mesh, height, &runoff, sea, &ocean, &mut flow, &mut scratch);
    }
    let retained: f64 = cover.iter().zip(&mesh.area).map(|(h, a)| h * a).sum();
    let to_coast: f64 = export.iter().sum();
    let residual = eroded - retained - to_coast - basin_store;
    let changed = original.iter().zip(&flow.receiver).filter(|(a, b)| a != b).count();
    if distributed {
        spread_hillslope_runoff(mesh, height, &runoff, &mut flow);
    }
    let received: f64 = flow.receiver.iter().enumerate().filter(|(i, r)| *i == **r)
        .map(|(i, _)| flow.discharge[i]).sum();
    let water_residual = runoff.iter().sum::<f64>() - received;
    eprintln!("[worldgen] connected_watersheds rerouted_nodes={changed} bedrock_m3={eroded:.1} floodplain_m3={overbank:.1} retained_m3={retained:.1} outlet_m3={to_coast:.1} terminal_stock_m3={basin_store:.1} sediment_residual_m3={residual:.6} runoff_residual={water_residual:.6}");
    export
}

impl Transport {
    fn new(count: usize) -> Self {
        Self {
            source: vec![0.0; count], incoming: vec![0.0; count], coast: vec![0.0; count],
            eroded: 0.0, deposited: 0.0,
        }
    }

    fn settle(&mut self, mesh: &Mesh, height: &mut [f64], flow: &Flow, sea: f64, ocean_count: usize) {
        for &i in flow.order[ocean_count..].iter().rev() {
            let fresh = std::mem::take(&mut self.source[i]);
            let incoming = std::mem::take(&mut self.incoming[i]);
            if fresh == 0.0 && incoming == 0.0 { continue; }
            self.eroded += fresh;
            let r = flow.receiver[i];
            let capacity = (495.0 - height[i]).clamp(0.0, 1.5) * mesh.area[i];
            if r == i {
                // A closed basin settles at the same bounded rate as other
                // cells. Carry any remaining stock into the next route.
                let deposit = (incoming + fresh).min(capacity);
                height[i] += deposit / mesh.area[i];
                self.deposited += deposit;
                self.incoming[i] = incoming + fresh - deposit;
                continue;
            }
            let distance = mesh.directions[i].distance(mesh.directions[r])
                * engine_core::cube_sphere::atlas::RADIUS_M;
            let slope = ((flow.spill[i] - flow.spill[r]) / distance).max(0.0);
            let depth = (flow.spill[i] - height[i] - 0.5).max(0.0);
            // A backward-Euler relaxation of suspended load over an edge.
            // Larger discharge and slope keep grains moving. Drowned
            // reaches settle more of their incoming load into the bed.
            let travel = (320.0 + 0.9 * flow.discharge[i].sqrt())
                * (0.65 + (slope * 80.0).min(2.0)) / (1.0 + depth / 3.0);
            let potential = incoming * distance / (distance + travel);
            let deposit = potential.min(capacity);
            height[i] += deposit / mesh.area[i];
            self.deposited += deposit;
            let leaving = fresh + (incoming - deposit);
            if height[r] <= sea {
                // Keep provenance at the river mouth, before merging into
                // the ocean vertex that may receive several distinct rivers.
                self.coast[i] += leaving;
            } else {
                self.incoming[r] += leaving;
            }
        }
    }

    fn finish(self) -> Vec<f64> {
        let export: f64 = self.coast.iter().sum();
        let unsettled: f64 = self.incoming.iter().sum();
        let residual = self.eroded - self.deposited - export - unsettled;
        eprintln!("[worldgen] sediment_transport eroded_m3={:.3} deposited_m3={:.3} exported_m3={export:.3} unsettled_m3={unsettled:.3} residual_m3={residual:.6}",
            self.eroded, self.deposited);
        self.coast
    }
}

/// Deliver eroded material down the final receiver graph. Lakes retain a
/// fraction based on depth, so large lake-fed rivers can carry little sediment.
pub fn deliver_sediment(flow: &Flow, height: &[f64], sediment: &mut [f64]) {
    for &i in flow.order.iter().rev() {
        let r = flow.receiver[i];
        if r != i {
            let entering_lake = (flow.spill[r] - height[r] - 0.75).max(0.0);
            let from_lake = flow.spill[i] > height[i] + 0.75;
            let delivered = if from_lake { sediment[i] } else { sediment[i] / (1.0 + entering_lake) };
            sediment[r] += delivered;
        }
    }
}

/// Move loose rock downhill without changing its volume. Apply transfers
/// together so no vertex sees a partially updated slope in the same pass.
fn relax_slopes(mesh: &Mesh, height: &mut [f64], erodibility: &[f64], floor: f64, strength: f64, moved: &mut [f64]) {
    moved.fill(0.0);
    let rate = (0.18 * strength).clamp(0.0, 0.35);
    for i in 0..height.len() {
        if height[i] <= floor { continue; }
        // Resistant provinces retain steeper sides at watershed scale.
        let repose = 0.32 - 0.19 * ((erodibility[i] - 0.4) / 0.95).clamp(0.0, 1.0);
        let mut excess = 0.0;
        let mut receiver = i;
        for &(j, distance) in &mesh.neighbours[i] {
            if height[j] <= floor { continue; }
            let drop = height[i] - height[j] - repose * distance;
            if drop > excess { excess = drop; receiver = j; }
        }
        if receiver == i { continue; }
        let a = mesh.area[i];
        let b = mesh.area[receiver];
        let volume = (rate * excess / (a.recip() + b.recip())).min((height[i] - floor) * a);
        moved[i] -= volume / a;
        moved[receiver] += volume / b;
    }
    for (h, delta) in height.iter_mut().zip(moved.iter()) { *h += delta; }
}
