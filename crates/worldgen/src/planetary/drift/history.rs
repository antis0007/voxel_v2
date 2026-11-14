//! Advected, time-binned deposits and the climate and events that formed them.
//! Eight legacy or twenty-four deep-time 30 Myr bins plus two recent beds. Lava and soil
//! stay separate, so an eruption buries soil instead of mixing through it.
use super::{Crust, DRIFT_MYR, Mesh, STEP_MYR};
use crate::planetary::{SEA, smooth, tectonics::Edge};
use crate::{BlockId, planet::noise_at};
use glam::DVec3;

const BEDS: usize = 24;

#[derive(Clone, Copy, Default)]
struct Bed {
    depth: f64,
    wet: f64,
    organic: f64,
    carbonate: f64,
    lava: f64,
    date: f64,
}

impl Bed {
    fn add(&mut self, other: Self, weight: f64) {
        self.depth += other.depth * weight;
        self.wet += other.wet * weight;
        self.organic += other.organic * weight;
        self.carbonate += other.carbonate * weight;
        self.lava += other.lava * weight;
        self.date += other.date * weight;
    }

    fn scale(&mut self, weight: f64) {
        *self = Self { depth: self.depth * weight, wet: self.wet * weight,
            organic: self.organic * weight, carbonate: self.carbonate * weight,
            lava: self.lava * weight, date: self.date * weight };
    }
}

#[derive(Clone, Copy)]
pub(in crate::planetary) struct Column {
    beds: [Bed; BEDS],
    recent: [Bed; 2],
    duration: u16,
}

impl Default for Column {
    fn default() -> Self { Self { beds: [Bed::default(); BEDS], recent: [Bed::default(); 2], duration: DRIFT_MYR } }
}

#[derive(Default)]
struct Removal { basement: f64, lava: f64, carbonate: f64 }

impl Column {
    /// 30 Myr deposit bins; a shortened development history keeps one.
    fn bins(&self) -> usize { usize::from(self.duration / 30).max(1) }

    fn layers(&self) -> impl DoubleEndedIterator<Item = &Bed> {
        self.beds[..self.bins()].iter().chain(self.recent.iter())
    }

    fn layers_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut Bed> {
        let bins = self.bins();
        self.beds[..bins].iter_mut().chain(self.recent.iter_mut())
    }

    fn add(&mut self, other: &Self, weight: f64) {
        self.duration = self.duration.max(other.duration);
        for (bed, incoming) in self.beds.iter_mut().zip(other.beds.iter()) { bed.add(*incoming, weight); }
        for (bed, incoming) in self.recent.iter_mut().zip(other.recent.iter()) { bed.add(*incoming, weight); }
    }

    fn scale(&mut self, weight: f64) { for bed in self.layers_mut() { bed.scale(weight); } }

    pub(in crate::planetary) fn deposit(&mut self, myr: impl Into<f64>, depth: f64, wet: f64,
        temperature: f64, carbonate: f64, lava: bool, ponded: bool)
    {
        let myr = myr.into();
        if depth <= 0.0 { return; }
        let latest = self.recent[1];
        if latest.depth > 1e-9 && ((latest.lava / latest.depth > 0.5) != lava
            || (f64::from(myr) - latest.date / latest.depth).abs() > 0.5)
        {
            let oldest = self.recent[0];
            if oldest.depth > 0.0 {
                let date = oldest.date / oldest.depth;
                let index = (((date - 1.0).max(0.0) / 30.0) as usize).min(self.bins() - 1);
                self.beds[index].add(oldest, 1.0);
            }
            self.recent = [latest, Bed::default()];
        }
        let organic = if ponded && !lava {
            smooth(0.52, 0.88, wet) * smooth(1.0, 18.0, temperature)
        } else { 0.0 };
        self.recent[1].add(Bed { depth, wet: wet * depth, organic: organic * depth,
            carbonate: carbonate * depth, lava: if lava { depth } else { 0.0 },
            date: f64::from(myr) * depth }, 1.0);
    }

    /// Erosion removes youngest deposits first; the remainder cuts basement.
    fn erode(&mut self, mut depth: f64) -> Removal {
        let mut removed = Removal::default();
        for bed in self.layers_mut().rev() {
            if depth <= 0.0 { break; }
            let cut = depth.min(bed.depth);
            if bed.depth > 0.0 {
                removed.lava += cut * bed.lava / bed.depth;
                removed.carbonate += cut * bed.carbonate / bed.depth;
                bed.scale((bed.depth - cut) / bed.depth);
            }
            depth -= cut;
        }
        removed.basement = depth;
        removed
    }

    // Columns contain many beds; hot read-only queries must not copy the column.
    fn mobile(&self, myr: impl Into<f64>) -> f64 {
        let myr = myr.into();
        let mut cover = 0.0;
        for bed in self.layers().rev().filter(|b| b.depth > 1e-9) {
            if bed.lava / bed.depth > 0.5 || f64::from(myr) - bed.date / bed.depth > 20.0 { break; }
            cover += bed.depth;
        }
        cover
    }

    pub(in crate::planetary) fn block(&self, depth: f64) -> Option<BlockId> {
        self.block_at(depth, self.duration)
    }

    pub(in crate::planetary) fn trim(&mut self, depth: f64) { self.erode(depth.max(0.0)); }

    pub(in crate::planetary) fn fit(&mut self, top: f64, fit: impl Fn(f64) -> f64) {
        let mut at = top;
        for bed in self.layers_mut().rev() {
            let next = at - bed.depth;
            if bed.depth > 0.0 { bed.scale((fit(at) - fit(next)).max(0.0) / bed.depth); }
            at = next;
        }
    }

    fn block_at(&self, mut depth: f64, myr: impl Into<f64>) -> Option<BlockId> {
        let myr = myr.into();
        if depth < 0.0 { return None; }
        let cover = depth;
        for bed in self.layers().rev() {
            if depth >= bed.depth { depth -= bed.depth; continue; }
            let age = f64::from(myr) - bed.date / bed.depth;
            return Some(if bed.lava / bed.depth > 0.50 { BlockId::Basalt }
                else if cover > 6.0 && age > 40.0 && bed.organic / bed.depth > 0.60 { BlockId::CoalOre }
                else if bed.carbonate / bed.depth > 0.58 && (cover > 4.0 || age > 30.0) { BlockId::Limestone }
                else if bed.wet / bed.depth > 0.64 && cover < 12.0 { BlockId::Clay }
                else if cover < 2.0 && age < 30.0 && bed.wet / bed.depth > 0.34 { BlockId::Dirt }
                else if age < 15.0 && cover < 4.0 { BlockId::Sand }
                else { BlockId::Sandstone });
        }
        None
    }

    fn sediment(&self) -> f64 { self.layers().map(|b| b.depth - b.lava).sum() }

    pub(super) fn thickness(&self) -> f64 { self.layers().map(|b| b.depth).sum() }
}

#[derive(Clone, Copy)]
pub(in crate::planetary) struct RockColumn {
    pub beds: Column,
    pub top: f64,
    pub lava_m: f64,
    pub carbonate: f64,
    pub weathered_myr: f64,
}

impl RockColumn {
    pub(in crate::planetary) fn block(&self, height: f64) -> BlockId {
        if self.lava_m > 0.0 && height > self.top - self.lava_m { return BlockId::Basalt; }
        self.beds.block(self.top - self.lava_m - height).unwrap_or(
            if self.carbonate > 0.58 { BlockId::Limestone } else { BlockId::Stone })
    }

    pub(in crate::planetary) fn resistance(&self, height: f64) -> f64 {
        if self.lava_m > 0.0 && height > self.top - self.lava_m { return 2.6; }
        let fresh = match self.block(height) {
            BlockId::Basalt => 2.6,
            BlockId::CoalOre | BlockId::Clay | BlockId::Dirt => 0.65,
            BlockId::Sandstone | BlockId::Sand => 1.05,
            BlockId::Limestone => 1.65,
            _ => 1.9,
        };
        fresh * (1.0 - 0.48 * smooth(15.0, 150.0, self.weathered_myr))
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Record {
    pub column: Column,
    pub weathered_myr: f64,
    pub locked: f64,
    pub magma: f64,
    pub quake_m: f64,
    pub eruption_m: f64,
    pub wet: f64,
    pub temperature: f64,
    pub winter_c: f64,
    pub basement_cut: f64,
}

impl Record {
    pub(super) fn remove(&mut self, cut: f64, basement_carbonate: f64) -> (f64, f64) {
        let removed = self.column.erode(cut);
        self.basement_cut += removed.basement;
        (removed.basement + removed.lava, removed.carbonate + removed.basement * basement_carbonate)
    }

    pub(super) fn deposit(&mut self, myr: impl Into<f64>, depth: f64, carbonate: f64, ponded: bool) {
        self.column.deposit(myr, depth, self.wet, self.temperature, carbonate, false, ponded);
    }

    /// Return newly mobilised rock, excluding reworked older sediment.
    pub(super) fn reshape(&mut self, myr: impl Into<f64>, cut: f64, deposit: f64,
        carbonate: f64, ponded: bool) -> f64
    {
        let (fresh, _) = self.remove(cut, carbonate);
        self.deposit(myr, deposit, carbonate, ponded);
        fresh
    }

    pub(super) fn mobile(&self, myr: impl Into<f64>) -> f64 { self.column.mobile(myr) }

    pub(super) fn resistance(&self, myr: impl Into<f64>, carbonate: f64) -> f64 {
        let myr = myr.into();
        self.resistance_with_mobile(myr, carbonate, self.mobile(myr))
    }

    // The supplied cover must be this unchanged column's mobile query at myr.
    pub(super) fn resistance_with_mobile(&self, myr: f64, carbonate: f64, mobile: f64) -> f64 {
        let fresh = match self.column.block_at(mobile + 0.05, myr) {
            Some(BlockId::Basalt) => 2.6,
            Some(BlockId::CoalOre | BlockId::Clay | BlockId::Dirt) => 0.65,
            Some(BlockId::Sandstone | BlockId::Sand) => 1.05,
            _ => 1.25 + 0.65 * carbonate,
        };
        fresh * (1.0 - 0.48 * smooth(15.0, 150.0, self.weathered_myr))
    }
}

pub(in crate::planetary) struct Field {
    pub(super) records: Vec<Record>,
    next: Vec<Record>,
}

impl Field {
    pub(super) fn with_duration(n: usize, duration: u16) -> Self {
        let record = Record { column: Column { duration, ..Column::default() }, ..Record::default() };
        Self { records: vec![record; n], next: vec![record; n] }
    }

    pub(super) fn seal(&mut self) { self.next = Vec::new(); }

    /// Fills the next generation of records from the current ones, one
    /// destination at a time, then `finish` scales each and `commit` swaps.
    pub(super) fn gather_into(&mut self, workers: usize, sources: impl Fn(usize) -> Vec<(usize, f64)> + Sync) {
        let duration = self.records.first().map_or(DRIFT_MYR, |r| r.column.duration);
        let blank = Record { column: Column { duration, ..Column::default() }, ..Record::default() };
        let records = &self.records;
        // Reusing the buffer avoids faulting in fresh pages every plate step.
        crate::planetary::fill_history(&mut self.next, workers, |to, new| {
            *new = blank;
            // Sources arrive in ascending parcel order: the serial scatter's
            // float summation order, independent of the worker count.
            for (from, area) in sources(to) {
                let old = &records[from];
                new.column.add(&old.column, area);
                new.weathered_myr += old.weathered_myr * area;
                new.locked += old.locked * area;
                new.magma += old.magma * area;
                new.quake_m += old.quake_m * area;
                new.eruption_m += old.eruption_m * area;
            }
        });
    }

    /// Install the finished generation as the current records.
    pub(super) fn commit(&mut self) { std::mem::swap(&mut self.records, &mut self.next); }

    /// Normalises gathered record `i` in place. `decay` then scales the
    /// weathering age, as the caller previously did after the copy back.
    pub(super) fn finish(&mut self, i: usize, area: f64, arriving_area: f64, elapsed_myr: f64, decay: f64) {
        let record = &mut self.next[i];
        record.column.scale(area.recip());
        let weight = arriving_area.max(1e-12).recip();
        record.weathered_myr = record.weathered_myr * weight + elapsed_myr;
        record.locked *= weight;
        record.magma *= weight;
        record.quake_m *= weight * 0.5;
        record.eruption_m *= weight * 0.5;
        record.weathered_myr *= decay;
    }

    pub(in crate::planetary) fn column(&self, mesh: &Mesh, dir: DVec3) -> (Column, f64) {
        let (ids, weights) = mesh.stencil(dir);
        let mut column = Column::default();
        let mut age = 0.0;
        for (i, weight) in ids.into_iter().zip(weights) {
            column.add(&self.records[i].column, weight);
            age += self.records[i].weathered_myr * weight;
        }
        (column, age)
    }

    pub(super) fn sediment_volume(&self, mesh: &Mesh) -> f64 {
        self.records.iter().zip(&mesh.area).map(|(r, a)| r.column.sediment() * a).sum()
    }

    pub(super) fn checksum(&self, mut hash: u64) -> u64 {
        for record in &self.records {
            for bed in record.column.layers() {
                for value in [bed.depth, bed.wet, bed.organic, bed.carbonate, bed.lava, bed.date] {
                    hash = (hash ^ value.to_bits()).wrapping_mul(0x100000001b3);
                }
            }
        }
        hash
    }
}

/// Aggregates many ruptures and eruptions into one geological step. Loading
/// and magma recharge are persistent; a date alone never creates an event.
pub(super) fn events(record: &mut Record, seed: u32, dir: DVec3, edge: &Edge,
    own: Crust, other: Crust, myr: u16) -> (f64, f64)
{
    let [compression, spreading, shear] = edge.stresses().map(|v| v.min(1.0));
    let key = seed ^ (edge.owner.min(edge.neighbour) as u32).wrapping_mul(0x9e37_79b9)
        ^ (edge.owner.max(edge.neighbour) as u32).wrapping_mul(0x85eb_ca6b);
    let along = (dir - edge.normal * dir.dot(edge.normal)).normalize_or_zero();
    let segment = noise_at(along * 31.0, key);
    record.locked += (compression + 0.28 * shear) * f64::from(STEP_MYR);
    // Locked faults creep after leaving an active margin.
    record.locked *= 1.0 - 0.12 * (1.0 - (compression + shear).min(1.0));
    let threshold = 4.5 + 5.0 * segment;
    let subducting = (1.0 - own.continental.min(other.continental))
        * smooth(12.0, 100.0, own.ocean_age_myr.max(other.ocean_age_myr));
    let upper = own.continental > other.continental
        || ((own.continental - other.continental).abs() < 0.05 && own.ocean_age_myr < other.ocean_age_myr);
    let mut slip = 0.0;
    if record.locked >= threshold && edge.boundary < 0.065 {
        let released = record.locked.min(threshold * 1.4);
        record.locked -= released;
        let polarity = if upper { 1.0 } else { -0.32 };
        let megathrust = subducting * compression;
        slip = released * (0.55 * compression + 2.0 * megathrust) * polarity
            * (1.0 - smooth(0.012, 0.065, edge.boundary));
        record.quake_m = slip;
        if slip > 0.5 { record.weathered_myr *= (-slip / 8.0).exp(); }
    }
    let arc = if upper { subducting * compression * smooth(0.008, 0.025, edge.boundary) } else { 0.0 };
    let melt = (arc + spreading * (1.0 - 0.65 * own.continental))
        * smooth(0.48, 0.80, noise_at(along * 67.0, key.wrapping_add(83)));
    record.magma = (record.magma + melt * f64::from(STEP_MYR)) * 0.97;
    let mut lava = 0.0;
    if record.magma > 3.2 + 3.0 * segment {
        lava = (record.magma * 3.0).min(22.0);
        record.magma *= 0.18;
        record.eruption_m = lava;
        record.weathered_myr = 0.0;
        record.column.deposit(myr, lava, 0.0, 0.0, 0.0, true, false);
    }
    (slip, lava)
}

/// Bounded hillslope transport moves real rock/soil into neighbouring lows.
pub(super) fn weather(mesh: &Mesh, records: &mut [Record], crust: &[Crust], height: &mut [f64],
    soil: &mut [f64], strength: f64, myr: impl Into<f64>) -> f64
{
    let myr = myr.into();
    let mut moved = vec![0.0; height.len()];
    let mut cuts = vec![0.0; height.len()];
    for i in 0..height.len() {
        if height[i] <= SEA { continue; }
        let old = smooth(8.0, 120.0, records[i].weathered_myr);
        let wet = records[i].wet;
        let resistance = records[i].resistance(myr, crust[i].carbonate);
        for &(j, distance) in &mesh.neighbours[i] {
            let drop = height[i] - height[j];
            if drop <= 0.0 { continue; }
            let grade = drop / distance;
            let repose = 0.72 - 0.25 * old;
            let creep = (0.08 + 0.85 * old) * wet * grade;
            let failure = (grade - repose).max(0.0) * distance * 0.10;
            let cut = ((creep + failure) * strength / resistance).min(drop * 0.08).min(2.5)
                / mesh.neighbours[i].len() as f64;
            cuts[i] += cut;
            moved[j] += cut * mesh.area[i];
        }
    }
    let mut rock = 0.0;
    for i in 0..height.len() {
        rock += records[i].reshape(myr, cuts[i], moved[i] / mesh.area[i], crust[i].carbonate, false) * mesh.area[i];
        soil[i] = records[i].mobile(myr);
        height[i] += moved[i] / mesh.area[i] - cuts[i];
    }
    rock
}

#[cfg(test)]
mod column_query_tests {
    use super::*;

    #[test]
    fn column_queries_preserve_material_and_age_boundaries() {
        for duration in [240, 720] {
            let empty = Column { duration, ..Column::default() };
            assert_eq!(empty.block_at(0.0, 30.0), None);
            assert_eq!(empty.mobile(30.0), 0.0);
            assert_eq!(empty.sediment(), 0.0);
            let mut soil = empty;
            soil.recent[1] = Bed { depth: 10.0, wet: 5.0, date: 100.0, ..Bed::default() };
            assert_eq!(soil.block_at(-0.01, 10.0), None);
            assert_eq!(soil.block_at(1.0, 10.0), Some(BlockId::Dirt));
            assert_eq!(soil.block_at(2.0, 10.0), Some(BlockId::Sand));
            assert_eq!(soil.block_at(4.0, 10.0), Some(BlockId::Sandstone));
            assert_eq!(soil.block_at(10.0, 10.0), None);
            assert_eq!(soil.mobile(30.0), 10.0);
            assert_eq!(soil.mobile(30.01), 0.0);
            assert_eq!(soil.sediment(), 10.0);
            soil.recent[1].wet = 8.0;
            assert_eq!(soil.block_at(1.0, 10.0), Some(BlockId::Clay));
            soil.recent[1].organic = 8.0;
            assert_eq!(soil.block_at(7.0, 60.0), Some(BlockId::CoalOre));
            soil.recent[1].organic = 0.0;
            soil.recent[1].carbonate = 8.0;
            assert_eq!(soil.block_at(3.0, 60.0), Some(BlockId::Limestone));
            soil.recent[1].lava = 10.0;
            assert_eq!(soil.block_at(1.0, 10.0), Some(BlockId::Basalt));
            assert_eq!(soil.mobile(10.0), 0.0);
            assert_eq!(soil.sediment(), 0.0);
        }
    }

    #[test]
    fn rock_queries_preserve_lava_cap_and_basement() {
        let rock = RockColumn { beds: Column::default(), top: 10.0, lava_m: 2.0,
            carbonate: 0.3, weathered_myr: 0.0 };
        assert_eq!(rock.block(9.0), BlockId::Basalt);
        assert_eq!(rock.block(8.0), BlockId::Stone);
        assert_eq!(rock.resistance(9.0).to_bits(), 2.6_f64.to_bits());
        assert_eq!(rock.resistance(8.0).to_bits(), 1.9_f64.to_bits());
        let limestone = RockColumn { carbonate: 0.7, ..rock };
        assert_eq!(limestone.block(8.0), BlockId::Limestone);
        assert_eq!(limestone.resistance(8.0).to_bits(), 1.65_f64.to_bits());
    }
}

#[cfg(test)]
mod resistance_reuse_tests {
    use super::*;

    // Frozen pre-reuse formula: do not call either resistance entry point.
    fn original_resistance(record: &Record, myr: f64, carbonate: f64) -> f64 {
        let fresh = match record.column.block_at(record.mobile(myr) + 0.05, myr) {
            Some(BlockId::Basalt) => 2.6,
            Some(BlockId::CoalOre | BlockId::Clay | BlockId::Dirt) => 0.65,
            Some(BlockId::Sandstone | BlockId::Sand) => 1.05,
            _ => 1.25 + 0.65 * carbonate,
        };
        fresh * (1.0 - 0.48 * smooth(15.0, 150.0, record.weathered_myr))
    }

    fn bed(depth: f64, date: f64, wet: f64, organic: f64, carbonate: f64, lava: f64) -> Bed {
        Bed { depth, date: date * depth, wet: wet * depth, organic: organic * depth,
            carbonate: carbonate * depth, lava: lava * depth }
    }

    #[test]
    fn supplied_mobile_resistance_matches_original_across_materials_and_age_boundaries() {
        let mut observed = Vec::new();
        for duration in [240, 720] {
            let empty = Column { duration, ..Column::default() };
            let mut cases = vec![("empty basement", empty)];
            for (name, wet, lava) in [("dry soil", 0.1, 0.0), ("wet soil", 0.8, 0.0),
                ("lava cap", 0.0, 1.0)] {
                let mut column = empty;
                column.recent[1] = bed(10.0, 10.0, wet, 0.0, 0.0, lava);
                cases.push((name, column));
            }
            // A thin immobile cap lets the 5 cm resistance query reach young
            // sand/dirt/clay without incorrectly treating the cap as mobile.
            for (name, wet) in [("sand under lava", 0.1), ("dirt under lava", 0.5),
                ("clay under lava", 0.8)] {
                let mut column = empty;
                column.recent[0] = bed(2.0, 10.0, wet, 0.0, 0.0, 0.0);
                column.recent[1] = bed(0.01, 10.0, 0.0, 0.0, 0.0, 1.0);
                cases.push((name, column));
            }
            for (name, cover, organic, carbonate) in [("buried coal", 8.0, 0.8, 0.0),
                ("buried carbonate", 5.0, 0.0, 0.8)] {
                let mut column = empty;
                column.beds[usize::from(duration / 30) - 1] = bed(10.0, 0.0, 0.2, organic, carbonate, 0.0);
                column.recent[1] = bed(cover, 50.0, 0.5, 0.0, 0.0, 0.0);
                cases.push((name, column));
            }
            let mut column = empty;
            column.beds[0] = bed(2.0, 0.0, 0.5, 0.0, 0.0, 0.0);
            column.recent[1] = bed(1e-10, 10.0, 0.5, 0.0, 0.0, 0.0);
            cases.push(("negligible cap and empty bins", column));
            for (name, column) in cases {
                for myr in [10.0, 24.999999, 25.0, 25.000001, 29.9, 30.0, 30.1,
                    39.9, 40.0, 40.1, 49.9, 50.0, 50.1, 69.9, 70.0, 70.1, 90.0] {
                    for weathered_myr in [0.0, 14.999999, 15.0, 15.000001, 80.0,
                        149.999999, 150.0, 150.000001] {
                        let record = Record { column, weathered_myr, ..Record::default() };
                        // Exact caller contract: fluvial supplied this query at
                        // the same date, and this column has not changed since.
                        let supplied_soil = record.mobile(myr);
                        let material = record.column.block_at(supplied_soil + 0.05, myr);
                        if !observed.contains(&material) { observed.push(material); }
                        for carbonate in [0.0, 0.58, 0.75, 1.0] {
                            let expected = original_resistance(&record, myr, carbonate).to_bits();
                            assert_eq!(record.resistance_with_mobile(myr, carbonate, supplied_soil).to_bits(), expected,
                                "cached: {name}, duration={duration}, myr={myr}, weathered={weathered_myr}, carbonate={carbonate}");
                            assert_eq!(record.resistance(myr, carbonate).to_bits(), expected,
                                "fallback: {name}, duration={duration}, myr={myr}, weathered={weathered_myr}, carbonate={carbonate}");
                        }
                    }
                }
            }
        }
        for expected in [None, Some(BlockId::Basalt), Some(BlockId::CoalOre), Some(BlockId::Clay),
            Some(BlockId::Dirt), Some(BlockId::Sand), Some(BlockId::Sandstone), Some(BlockId::Limestone)] {
            assert!(observed.contains(&expected), "fixture did not cover material {expected:?}");
        }
    }
}

mod snapshot;

#[cfg(test)]
impl Column {
    pub(in crate::planetary) fn assert_history_stencil_bits_eq(&self, other: &Self) {
        assert_eq!(self.duration, other.duration, "column duration");
        for (i, (a, b)) in self.beds.iter().chain(&self.recent)
            .zip(other.beds.iter().chain(&other.recent)).enumerate() {
            assert_eq!([a.depth, a.wet, a.organic, a.carbonate, a.lava, a.date].map(f64::to_bits),
                [b.depth, b.wet, b.organic, b.carbonate, b.lava, b.date].map(f64::to_bits),
                "stored bed {i}, including inactive/future bins");
        }
    }
}
