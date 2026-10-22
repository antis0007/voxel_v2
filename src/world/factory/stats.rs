//! Factorio-style production statistics: what the factory made and used,
//! per resource, over the last minute and the last hour, plus a power graph.
//! Counts are measured from machine buffers, so every recipe, fuel and
//! multiblock is covered without bookkeeping at each call site.
use super::RESOURCE_COUNT;
use std::collections::VecDeque;

pub const SECONDS: usize = 60;
pub const MINUTES: usize = 60;

#[derive(Clone, Copy, Default)]
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
