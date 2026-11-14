//! Factorio-style production statistics: what the factory made and used,
//! per resource, over the last minute and the last hour, plus a power graph.
//! Counts are measured from machine buffers, so every recipe, fuel and
//! multiblock is covered without bookkeeping at each call site.
use super::RESOURCE_COUNT;
use std::collections::VecDeque;

pub const SECONDS: usize = 60;
pub const MINUTES: usize = 60;

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Sample {
    /// Resource sub-units (1/64 item) made and used.
    pub made: [u32; RESOURCE_COUNT],
    pub used: [u32; RESOURCE_COUNT],
    /// Energy units generated and drawn over the sample.
    pub generated: u64,
    pub drawn: u64,
    /// Gross storage transactions, excluding transformer buffers.
    pub charged: u64,
    pub discharged: u64,
    /// Cable energy delivered to consumer buffers, not consumption.
    pub delivered: u64,
    /// Energy entering converter buffers; ideal capacity abstraction.
    pub converted: u64,
    /// Machine-kits finished by assemblers.
    pub kits: u32,
}

impl Sample {
    fn subtract(&mut self, o: &Sample) {
        for i in 0..RESOURCE_COUNT { self.made[i] -= o.made[i]; self.used[i] -= o.used[i]; }
        self.generated -= o.generated; self.drawn -= o.drawn;
        self.charged -= o.charged; self.discharged -= o.discharged;
        self.delivered -= o.delivered; self.converted -= o.converted; self.kits -= o.kits;
    }
    fn add(&mut self, o: &Sample) {
        for i in 0..RESOURCE_COUNT {
            self.made[i] += o.made[i];
            self.used[i] += o.used[i];
        }
        self.generated += o.generated;
        self.drawn += o.drawn;
        self.charged += o.charged;
        self.discharged += o.discharged;
        self.delivered += o.delivered;
        self.converted += o.converted;
        self.kits += o.kits;
    }
}

#[derive(Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Production {
    current: Sample,
    ticks: u32,
    pub seconds: VecDeque<Sample>,
    hour_total: Sample,
    hour_seconds: VecDeque<Sample>,
}

impl Production {
    pub fn made(&mut self, i: usize, n: u32) {
        self.current.made[i] += n;
    }
    pub fn used(&mut self, i: usize, n: u32) {
        self.current.used[i] += n;
    }
    pub fn power(&mut self, generated: u32, drawn: u32) {
        self.current.generated += generated as u64;
        self.current.drawn += drawn as u64;
    }
    pub fn kits(&mut self, n: u32) {
        self.current.kits += n;
    }
    pub fn delivery(&mut self, n: u32) { self.current.delivered += n as u64; }
    pub fn storage(&mut self, n: u32, charging: bool, converter: bool) {
        if converter {
            if charging { self.current.converted += n as u64; }
        } else if charging { self.current.charged += n as u64; }
        else { self.current.discharged += n as u64; }
    }
    /// Close one 0.25 s tick. Four make a second; sixty seconds a minute.
    pub fn tick(&mut self) {
        self.ticks += 1;
        if self.ticks < 4 {
            return;
        }
        self.ticks = 0;
        let second = std::mem::take(&mut self.current);
        self.hour_total.add(&second);
        self.hour_seconds.push_back(second);
        if self.hour_seconds.len() > MINUTES * SECONDS {
            self.hour_total.subtract(&self.hour_seconds.pop_front().unwrap());
        }
        self.seconds.push_back(second);
        if self.seconds.len() > SECONDS {
            self.seconds.pop_front();
        }
    }
    /// Totals over the last minute (`hour == false`) or hour, and the span
    /// in seconds they cover so far.
    pub fn window(&self, hour: bool) -> (Sample, f32) {
        let mut total = Sample::default();
        if hour {
            (self.hour_total, (self.hour_seconds.len() as f32).max(1.0))
        } else {
            for s in &self.seconds {
                total.add(s);
            }
            (total, (self.seconds.len() as f32).max(1.0))
        }
    }
}

/// One base's history: a minute of seconds and an hour of minutes, so many
/// bases cost about 36 KB each instead of an hour of seconds.
#[derive(Default)]
pub struct BaseHistory {
    current: Sample,
    seconds: VecDeque<Sample>,
    minutes: VecDeque<Sample>,
    minute: Sample,
    minute_len: u32,
}

impl BaseHistory {
    fn close_second(&mut self) {
        let second = std::mem::take(&mut self.current);
        self.minute.add(&second);
        self.minute_len += 1;
        if self.minute_len as usize == SECONDS {
            self.minutes.push_back(std::mem::take(&mut self.minute));
            self.minute_len = 0;
            if self.minutes.len() > MINUTES {
                self.minutes.pop_front();
            }
        }
        self.seconds.push_back(second);
        if self.seconds.len() > SECONDS {
            self.seconds.pop_front();
        }
    }

    /// Totals over the last minute or hour, and the seconds they cover.
    pub fn window(&self, hour: bool) -> (Sample, f32) {
        let mut total = Sample::default();
        if hour {
            for m in &self.minutes {
                total.add(m);
            }
            total.add(&self.minute);
            (total, ((self.minutes.len() * SECONDS) as f32 + self.minute_len as f32).max(1.0))
        } else {
            for s in &self.seconds {
                total.add(s);
            }
            (total, (self.seconds.len() as f32).max(1.0))
        }
    }

    /// Generation and draw per second over the last minute, oldest first.
    pub fn power_history(&self) -> Vec<(u64, u64)> {
        self.seconds.iter().map(|s| (s.generated, s.drawn)).collect()
    }

    /// Folds a merged base's history into this one, newest buckets aligned.
    fn absorb(&mut self, other: BaseHistory) {
        self.current.add(&other.current);
        let fold = |into: &mut VecDeque<Sample>, from: VecDeque<Sample>| {
            let skip = into.len().saturating_sub(from.len());
            let offset = from.len().saturating_sub(into.len());
            for (mine, theirs) in into.iter_mut().skip(skip).zip(from.iter().skip(offset)) {
                mine.add(theirs);
            }
        };
        fold(&mut self.seconds, other.seconds);
        fold(&mut self.minutes, other.minutes);
        self.minute.add(&other.minute);
    }
}

/// Per-node machine buffers before a factory tick: input, output, assembler stock.
pub type Before = (glam::IVec3, [u32; RESOURCE_COUNT], [u32; RESOURCE_COUNT], u32);

/// Production and power per factory base, attributed from each machine's
/// position. Runtime only: history restarts on load like the global graph.
#[derive(Default)]
pub struct ByBase {
    owner: crate::fast_hash::FastMap<glam::IVec3, u32>,
    pub bases: crate::fast_hash::FastMap<u32, BaseHistory>,
    ticks: u32,
}

impl ByBase {
    pub fn get(&self, id: u32) -> Option<&BaseHistory> {
        self.bases.get(&id)
    }

    /// Stored energy of every powered consumer, taken before machines run.
    pub fn energy_mark(&self, nodes: &crate::fast_hash::FastMap<glam::IVec3, super::Node>) -> Vec<(glam::IVec3, u32)> {
        if self.owner.is_empty() {
            return Vec::new();
        }
        nodes.iter()
            .filter(|(_, n)| n.kind.powered() && !n.kind.generator())
            .map(|(&p, n)| (p, n.energy))
            .collect()
    }

    /// Attributes one 0.25 s factory tick to the bases that own each machine.
    pub fn record(
        &mut self,
        nodes: &crate::fast_hash::FastMap<glam::IVec3, super::Node>,
        before: &[Before],
        energy: &[(glam::IVec3, u32)],
    ) {
        if self.owner.is_empty() {
            return;
        }
        fn at<'a>(
            owner: &crate::fast_hash::FastMap<glam::IVec3, u32>,
            bases: &'a mut crate::fast_hash::FastMap<u32, BaseHistory>,
            p: &glam::IVec3,
        ) -> Option<&'a mut Sample> {
            owner.get(p).map(|id| &mut bases.entry(*id).or_default().current)
        }
        let (owner, bases) = (&self.owner, &mut self.bases);
        for (p, input, output, stock) in before {
            let Some(n) = nodes.get(p) else { continue };
            let Some(s) = at(owner, bases, p) else { continue };
            for i in 0..RESOURCE_COUNT {
                s.used[i] += input[i].saturating_sub(n.input[i]);
                s.made[i] += n.output[i].saturating_sub(output[i]);
            }
            if n.stock > *stock && matches!(super::assembler::stock_item(n),
                Some(crate::world::inventory::Item::Block(b)) if super::Kind::from_block(b).is_some())
            {
                s.kits += n.stock - stock;
            }
        }
        for (p, n) in nodes.iter().filter(|(_, n)| n.kind.generator() && n.flow > 0) {
            if let Some(s) = at(owner, bases, p) {
                s.generated += n.flow as u64;
            }
        }
        for (p, was) in energy {
            if let (Some(n), Some(s)) = (nodes.get(p), at(owner, bases, p)) {
                s.drawn += u64::from(was.saturating_sub(n.energy));
            }
        }
    }

    /// Closes a tick. Once a second it regroups bases (cheap when nothing moved),
    /// rebuilds machine ownership and carries merged bases' history forward.
    pub fn tick(
        &mut self,
        nodes: &crate::fast_hash::FastMap<glam::IVec3, super::Node>,
        bases: &mut super::bases::Bases,
        cells_per_metre: i32,
    ) {
        self.ticks += 1;
        if self.ticks < 4 {
            return;
        }
        self.ticks = 0;
        for history in self.bases.values_mut() {
            history.close_second();
        }
        let cells = super::overview::base_cells(nodes);
        bases.refresh(&cells, super::overview::BASE_LINK_M * cells_per_metre as f32);
        self.owner.clear();
        for base in &bases.list {
            for &p in &base.members {
                self.owner.insert(p, base.id);
            }
        }
        let live: std::collections::HashSet<u32> = bases.list.iter().map(|b| b.id).collect();
        let gone: Vec<u32> = self.bases.keys().copied().filter(|id| !live.contains(id)).collect();
        for id in gone {
            let Some(history) = self.bases.remove(&id) else { continue };
            let to = bases.resolve(id);
            if to != id && live.contains(&to) {
                self.bases.entry(to).or_default().absorb(history);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_roll_into_minutes() {
        let mut p = Production::default();
        for _ in 0..4 * 61 {
            p.made(8, 64);
            p.tick();
        }
        assert_eq!(p.seconds.len(), 60);
        assert_eq!(p.hour_seconds.len(), 61);
        let (hour, span) = p.window(true);
        assert_eq!(hour.made[8], 64 * 4 * 61);
        assert_eq!(span, 61.0);
    }

    #[test]
    fn hour_is_a_sliding_3600_seconds_and_drops_the_oldest_transactions() {
        let mut p = Production::default();
        p.power(123, 7);
        for _ in 0..4 * 3600 { p.tick(); }
        assert_eq!(p.window(true).0.generated, 123);
        for _ in 0..4 { p.tick(); }
        let (hour, span) = p.window(true);
        assert_eq!(span, 3600.0);
        assert_eq!(hour.generated, 0);
        assert_eq!(hour.drawn, 0);
        assert_eq!(p.hour_seconds.len(), 3600);
    }
}
