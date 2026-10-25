//! Two starting designs, built through the same edit calls a player makes.
//! "Sentinel" is small and frugal: one level sensor, an 8-bit wake timer that
//! cannot wake faster than 125 ms, no SRAM. "Twin Watch" spends die area and
//! leakage on a second ADC, SRAM for the second channel's state, a 16-bit
//! timer that can wake every few ms, an ALU for the compares, and a watchdog
//! that restarts the core after a brownout (Sentinel hangs until reset).
use crate::design::{Design, Pin};
use crate::firmware::{Firmware, Task};
use crate::registry::Registry;
use crate::route::route_all;

pub const PROCESS: &str = "p8um";

fn pin(part: u16, port: &str) -> Pin {
    Pin { part, port: port.to_string() }
}

/// Join every pin to the first: one net.
fn join(d: &mut Design, registry: &Registry, pins: &[(u16, &str)]) {
    for w in pins.windows(2) {
        let _ = d.connect(registry, pin(w[0].0, w[0].1), pin(w[1].0, w[1].1));
    }
}

fn place(d: &mut Design, registry: &Registry, module: &str, x: u8, y: u8) -> u16 {
    d.place(registry, module, x, y, 0).unwrap_or(0)
}

pub fn sentinel(registry: &Registry) -> Design {
    let mut d = Design::new("Sentinel", PROCESS, 12, 10, 2);
    let supply = place(&mut d, registry, "supply_pad", 0, 0);
    let osc = place(&mut d, registry, "rc_osc", 3, 0);
    let core = place(&mut d, registry, "core8", 4, 3);
    let rom = place(&mut d, registry, "rom256", 9, 3);
    let timer = place(&mut d, registry, "timer8", 0, 5);
    let gpio = place(&mut d, registry, "gpio4", 0, 8);
    let adc = place(&mut d, registry, "adc8", 3, 8);
    join(&mut d, registry, &[(supply, "vdd"), (osc, "vdd"), (core, "vdd"), (rom, "vdd"), (timer, "vdd"), (gpio, "vdd"), (adc, "vdd")]);
    join(&mut d, registry, &[(osc, "clk"), (core, "clk")]);
    join(&mut d, registry, &[(supply, "por"), (core, "rst")]);
    join(&mut d, registry, &[(core, "imem"), (rom, "bus")]);
    join(&mut d, registry, &[(core, "pbus"), (timer, "bus"), (gpio, "bus"), (adc, "bus")]);
    d.firmware = Firmware { task: Task::FillBand, channels: 1, sample_ms: 250 };
    route_all(&mut d, registry);
    d
}

pub fn twin_watch(registry: &Registry) -> Design {
    let mut d = Design::new("Twin Watch", PROCESS, 14, 12, 2);
    let supply = place(&mut d, registry, "supply_pad", 0, 0);
    let osc = place(&mut d, registry, "rc_osc", 3, 0);
    let core = place(&mut d, registry, "core8", 5, 3);
    let rom = place(&mut d, registry, "rom256", 10, 2);
    let sram = place(&mut d, registry, "sram64", 10, 6);
    let alu = place(&mut d, registry, "alu8", 6, 7);
    let timer = place(&mut d, registry, "timer16", 0, 5);
    let gpio = place(&mut d, registry, "gpio4", 0, 10);
    let adc_a = place(&mut d, registry, "adc8", 3, 10);
    let adc_b = place(&mut d, registry, "adc8", 9, 10);
    let dog = place(&mut d, registry, "watchdog", 3, 3);
    join(&mut d, registry, &[
        (supply, "vdd"), (osc, "vdd"), (core, "vdd"), (rom, "vdd"), (sram, "vdd"), (alu, "vdd"),
        (timer, "vdd"), (gpio, "vdd"), (adc_a, "vdd"), (adc_b, "vdd"), (dog, "vdd"),
    ]);
    join(&mut d, registry, &[(osc, "clk"), (core, "clk"), (timer, "clk")]);
    join(&mut d, registry, &[(supply, "por"), (core, "rst")]);
    join(&mut d, registry, &[(core, "imem"), (rom, "bus")]);
    join(&mut d, registry, &[(core, "dmem"), (sram, "bus")]);
    join(&mut d, registry, &[(core, "alu"), (alu, "op")]);
    join(&mut d, registry, &[(core, "pbus"), (timer, "bus"), (gpio, "bus"), (adc_a, "bus"), (adc_b, "bus"), (dog, "bus")]);
    d.firmware = Firmware { task: Task::FillBand, channels: 2, sample_ms: 25 };
    route_all(&mut d, registry);
    d
}
