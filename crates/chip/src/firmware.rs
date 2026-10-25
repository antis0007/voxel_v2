//! Firmware tasks a player selects instead of hand-writing an instruction set.
//! Each task states its instruction mix, ROM and RAM needs, and its decision.
//! The mix is what the compiler times; the decision is what the world runs.
use serde::{Deserialize, Serialize};

/// A sensor channel that is wired to nothing in the world.
pub const OPEN: u16 = u16::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Task {
    /// Pump until full: drive the output on when any watched level falls
    /// below the lower threshold, off once every watched level reaches the
    /// upper one, and hold in between (hysteresis).
    FillBand,
    /// The same band on crate fill: run a processor until its output crates
    /// are full.
    StockBand,
    /// Inverse band on the machine's own heat: off above the upper
    /// threshold, on again once every reading is below the lower one.
    HeatGuard,
}

/// What a task's sensor channels measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sensor {
    TankLevel,
    CrateLevel,
    CoreHeat,
}

impl Task {
    pub const ALL: [Task; 3] = [Task::FillBand, Task::StockBand, Task::HeatGuard];
    pub fn sensor(self) -> Sensor {
        match self {
            Task::FillBand => Sensor::TankLevel,
            Task::StockBand => Sensor::CrateLevel,
            Task::HeatGuard => Sensor::CoreHeat,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Task::FillBand => "Tank Fill",
            Task::StockBand => "Crate Fill",
            Task::HeatGuard => "Heat Guard",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Firmware {
    pub task: Task,
    /// Level sensors the task reads each wake.
    pub channels: u8,
    /// Wake period requested from the timer (ignored without one: the core
    /// then polls continuously).
    pub sample_ms: u32,
}

impl Default for Firmware {
    fn default() -> Self {
        Self { task: Task::FillBand, channels: 1, sample_ms: 250 }
    }
}

/// Per-wake work of a task, in instructions by kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mix {
    /// Peripheral-bus accesses (ADC start/read, GPIO write).
    pub io: u32,
    /// Compares and logic: one cycle on an ALU, slow in core microcode.
    pub alu: u32,
    /// Fetch-dominated control flow and state stores.
    pub control: u32,
    pub conversions: u32,
    pub rom_bytes: u32,
    pub ram_bytes: u32,
}

impl Firmware {
    pub fn mix(&self) -> Mix {
        let ch = u32::from(self.channels.max(1));
        match self.task {
            Task::FillBand | Task::StockBand => Mix {
                io: 2 * ch + 1,
                alu: 3 * ch,
                control: 6 + ch,
                conversions: ch,
                rom_bytes: 20 + 12 * ch,
                // Output latch plus the last reading of each channel.
                ram_bytes: 1 + 2 * ch,
            },
            Task::HeatGuard => Mix {
                io: 2 * ch + 1,
                alu: 2 * ch,
                control: 6 + ch,
                conversions: ch,
                rom_bytes: 24 + 10 * ch,
                ram_bytes: 1 + 2 * ch,
            },
        }
    }

    pub fn label(&self) -> &'static str {
        self.task.label()
    }
}

/// The task's next output given the previous one. Levels and thresholds are
/// in permille of tank capacity; `OPEN` channels are ignored, and a task with
/// no connected channel fails safe (off).
pub fn decide(task: Task, prev: bool, levels: &[u16], lower: u16, upper: u16) -> bool {
    match task {
        Task::FillBand | Task::StockBand => {
            let (mut any, mut low, mut full) = (false, false, true);
            for &l in levels.iter().filter(|&&l| l != OPEN) {
                any = true;
                low |= l < lower;
                full &= l >= upper;
            }
            if !any {
                false
            } else if low {
                true
            } else if full {
                false
            } else {
                prev
            }
        }
        Task::HeatGuard => {
            let (mut any, mut hot, mut cool) = (false, false, true);
            for &l in levels.iter().filter(|&&l| l != OPEN) {
                any = true;
                hot |= l > upper;
                cool &= l < lower;
            }
            if !any || hot {
                false
            } else if cool {
                true
            } else {
                prev
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_band_has_hysteresis_and_fails_safe() {
        let t = Task::FillBand;
        assert!(decide(t, false, &[100], 300, 900));
        assert!(decide(t, true, &[500], 300, 900), "holds on inside the band");
        assert!(!decide(t, false, &[500], 300, 900), "holds off inside the band");
        assert!(!decide(t, true, &[900], 300, 900));
        assert!(decide(t, false, &[950, 100], 300, 900), "any low channel starts it");
        assert!(decide(t, true, &[950, 600], 300, 900), "all channels must reach upper");
        assert!(!decide(t, true, &[OPEN], 300, 900), "no sensor: off");
        assert!(!decide(t, true, &[950, OPEN], 300, 900));
        let h = Task::HeatGuard;
        assert!(!decide(h, true, &[760], 500, 750), "too hot: off");
        assert!(!decide(h, false, &[600], 500, 750), "cooling: stays off");
        assert!(decide(h, false, &[400], 500, 750), "cool: on");
        assert!(decide(h, true, &[700], 500, 750), "inside the band: holds on");
        assert!(!decide(h, true, &[OPEN], 500, 750), "no sensor: off");
    }
}
