//! First-order characterisation of a design that passed its checks.
//!
//! Timing (STA-lite): every core bus is a register-to-register path,
//! `t = t_clk→q(core) + 2·t_wire(bus) + t_access(slave) + t_setup(core)`
//! (address out, data back over the same routed net), ALU paths add chained
//! combinational ALUs, and the clock net's own wire delay counts as skew.
//! `t_wire(L, V) = a·L + b·L² + V·t_via` for routed length L and V vias.
//! `f = min(f_osc, f_core, 1 / (margin · t_critical))`: the oscillator is
//! trimmed to the timing-safe frequency.
//!
//! Work: a wake runs the firmware mix; ALU instructions take one core CPI on
//! an ALU that is on the core's ALU bus, the slow microcoded CPI otherwise.
//! Conversions run in parallel across ADC modules. With a timer on the
//! peripheral bus the core sleeps between wakes; without one it polls.
//!
//! Power: `P_idle = Σ leakage + C_clk·V²·f`; a wake costs core run power for
//! its handler time, plus per-instruction, per-access and per-conversion
//! energy, plus `½·C_bus·V²·(width/2)` per bus access. These are models
//! for gameplay, not transistor-level signoff.
use crate::compile::{Diagnostic, Fix, Severity, net_at, resolve};
use crate::design::{Design, Net, Part};
use crate::firmware::Task;
use crate::registry::{Dir, ModuleSpec, Process, Registry, Role};
use serde::{Deserialize, Serialize};

/// The world simulation's fixed step, which quantises response times.
pub const TICK_HZ: u32 = 120;
/// Power-on reset hold after supply returns, ms.
pub const BOOT_MS: f32 = 10.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub die_sites: u32,
    pub used_sites: u32,
    pub die_mm2: f32,
    pub wire_sites: u32,
    pub vias: u32,
    pub critical_ns: f32,
    pub critical_path: String,
    pub max_clock_hz: f32,
    pub clock_hz: f32,
    pub cycles_per_wake: u32,
    pub instructions_per_s: f32,
    pub handler_us: f32,
    pub busy_us: f32,
    pub max_wake_hz: f32,
    pub channels: u8,
    pub outputs: u8,
    pub rom_bytes: u32,
    pub ram_bytes: u32,
    pub sleeps: bool,
    pub wake_ms: f32,
    pub worst_response_ms: f32,
    pub idle_w: f32,
    pub wake_uj: f32,
    pub average_w: f32,
    pub model: Model,
}

/// What a manufactured part needs at runtime: small, copied into each
/// revision when it is released and never recomputed for installed parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Model {
    pub task: Task,
    pub channels: u8,
    pub wake_ticks: u32,
    /// Ticks from a wake to the output changing (conversion + handler).
    pub response_ticks: u32,
    pub boot_ticks: u32,
    /// A watchdog restarts the core after a brownout; without one it hangs.
    #[serde(default = "recovers_by_default")]
    pub recovers: bool,
    /// Continuous draw, nW (includes core run power when it never sleeps).
    pub idle_nw: u64,
    /// Energy per wake, nJ.
    pub wake_nj: u64,
}

/// Revisions released before the watchdog existed restarted after a brownout.
fn recovers_by_default() -> bool {
    true
}

fn wire_ns(p: &Process, net: &Net) -> f32 {
    let l = net.length() as f32;
    p.wire_ns_per_site * l + p.wire_ns_per_site2 * l * l + p.via_ns * net.vias() as f32
}

fn wire_pf(p: &Process, net: &Net) -> f32 {
    p.wire_pf_per_site * net.length() as f32
}

/// Worst combinational delay from an ALU input to the end of its carry chain.
fn alu_chain(design: &Design, registry: &Registry, p: &Process, part: &Part, depth: u32) -> f32 {
    let Some(spec) = registry.module(&part.module) else { return 0.0 };
    let mut worst = 0.0f32;
    if depth < 16
        && let Some(net) = net_at(design, part.id, "carry_out")
    {
        for (q, s, port) in resolve(design, registry, net) {
            if port.dir == Dir::Sink && s.combinational {
                worst = worst.max(wire_ns(p, net) + alu_chain(design, registry, p, q, depth + 1));
            }
        }
    }
    spec.delay_ns + worst
}

pub fn characterize(design: &Design, registry: &Registry, p: &Process, core: &Part, d: &mut Vec<Diagnostic>) -> Option<Report> {
    let core_spec = registry.module(&core.module)?;
    let fw = design.firmware;
    let mix = fw.mix();
    let slaves = |port: &str| -> Vec<(&Part, &ModuleSpec)> {
        net_at(design, core.id, port)
            .map(|n| resolve(design, registry, n).into_iter().filter(|(_, _, q)| q.dir == Dir::Sink).map(|(a, s, _)| (a, s)).collect())
            .unwrap_or_default()
    };

    // Timing.
    let mut critical = (0.0f32, String::from("none"));
    for port in ["imem", "dmem", "pbus", "alu"] {
        let Some(net) = net_at(design, core.id, port) else { continue };
        for (part, spec) in slaves(port) {
            let access = if spec.combinational { alu_chain(design, registry, p, part, 0) } else { spec.delay_ns };
            let t = core_spec.delay_ns + 2.0 * wire_ns(p, net) + access + core_spec.setup_ns;
            if t > critical.0 {
                critical = (t, format!("CPU {} → {} #{} · {} Sites · {} Vias", port.to_uppercase(), spec.name, part.id, net.length(), net.vias()));
            }
        }
    }
    let clock_net = net_at(design, core.id, "clk");
    let skew = clock_net.map_or(0.0, |n| wire_ns(p, n));
    let critical_ns = critical.0 + skew;
    let max_clock_hz = 1.0e9 / (p.margin * critical_ns.max(1.0));
    let osc_hz = clock_net
        .and_then(|n| resolve(design, registry, n).into_iter().find(|(_, s, q)| s.role == Role::Clock && q.dir == Dir::Drive))
        .map_or(0.0, |(_, s, _)| s.hz as f32);
    let clock_hz = osc_hz.min(core_spec.hz as f32).min(max_clock_hz);
    if clock_hz <= 0.0 {
        d.push(Diagnostic { severity: Severity::Error, message: "Clock Undriven".into(), fix: Some(Fix::UsePreset) });
        return None;
    }

    // Work per wake.
    let has_alu = slaves("alu").iter().any(|(_, s)| s.role == Role::Alu);
    let alu_cpi = if has_alu { core_spec.cpi } else { core_spec.soft_alu_cpi.max(core_spec.cpi) };
    let periph = slaves("pbus");
    let watchdog = periph.iter().any(|(_, s)| s.role == Role::Watchdog);
    // Each wake kicks the watchdog: a bus write and a reload.
    let control = mix.control + if watchdog { 2 } else { 0 };
    let instructions = mix.io + mix.alu + control;
    let cycles = core_spec.cpi * (mix.io + control) + alu_cpi * mix.alu;
    let handler_us = cycles as f32 / clock_hz * 1.0e6;
    let adcs: Vec<&ModuleSpec> = periph.iter().filter(|(_, s)| s.role == Role::Adc).map(|(_, s)| *s).collect();
    let channels = adcs.iter().map(|s| s.channels).sum::<u32>().min(255) as u8;
    let outputs = periph.iter().filter(|(_, s)| s.role == Role::Gpio).map(|(_, s)| s.channels).sum::<u32>().min(255) as u8;
    let conv_us = adcs.iter().map(|s| s.time_us).fold(0.0, f32::max);
    let rounds = mix.conversions.div_ceil(adcs.len().max(1) as u32);
    let busy_us = handler_us + conv_us * rounds as f32;
    let max_wake_hz = 1.0e6 / busy_us.max(1.0e-3);
    let timer = periph.iter().filter(|(_, s)| s.role == Role::Timer).map(|(_, s)| s.time_us).fold(f32::INFINITY, f32::min);
    let sleeps = timer.is_finite();
    let wake_ms = if sleeps { fw.sample_ms as f32 } else { busy_us / 1000.0 };
    if sleeps && (fw.sample_ms as f32) * 1000.0 < timer {
        let min = (timer / 1000.0).ceil() as u32;
        d.push(Diagnostic { severity: Severity::Error, message: format!("Timer Min {min} ms"), fix: Some(Fix::SampleAtLeast(min)) });
        return None;
    }
    if sleeps && (fw.sample_ms as f32) * 1000.0 < busy_us {
        let min = (busy_us / 1000.0).ceil() as u32;
        d.push(Diagnostic { severity: Severity::Error, message: format!("Wake {busy_us:.0} µs > {} ms", fw.sample_ms), fix: Some(Fix::SampleAtLeast(min)) });
        return None;
    }

    // Power.
    let v2 = p.vdd * p.vdd;
    let leak_w: f32 = design.parts.iter().filter_map(|q| registry.module(&q.module)).map(|s| s.idle_uw).sum::<f32>() * 1.0e-6;
    let clock_w = clock_net.map_or(0.0, |n| wire_pf(p, n) * 1.0e-12 * v2 * clock_hz);
    let run_w = core_spec.run_uw * 1.0e-6;
    let rom_nj = slaves("imem").iter().map(|(_, s)| s.event_nj).fold(0.0, f32::max);
    let bus_nj = |port: &str| net_at(design, core.id, port).map_or(0.0, |n| 0.5 * wire_pf(p, n) * 1.0e-3 * v2 * 4.0);
    let adc_nj = adcs.iter().map(|s| s.event_nj).fold(0.0, f32::max);
    // A polling core's run power is already in its continuous draw.
    let wake_nj = if sleeps { handler_us * 1.0e3 * run_w } else { 0.0 }
        + instructions as f32 * (core_spec.event_nj + rom_nj + bus_nj("imem"))
        + mix.io as f32 * bus_nj("pbus")
        + mix.conversions as f32 * adc_nj;
    let idle_w = leak_w + clock_w + if sleeps { 0.0 } else { run_w };
    let average_w = idle_w + wake_nj * 1.0e-9 / (wake_ms / 1000.0).max(1.0e-6);

    let tick_ms = 1000.0 / TICK_HZ as f32;
    let model = Model {
        task: fw.task,
        channels: channels.min(fw.channels),
        wake_ticks: ((wake_ms / tick_ms).round() as u32).max(1),
        response_ticks: ((busy_us / 1000.0 / tick_ms).ceil() as u32).max(1),
        boot_ticks: ((BOOT_MS / tick_ms).ceil() as u32).max(1),
        recovers: watchdog,
        // A polling core wakes every busy period, far faster than the world
        // step: its whole average draw is continuous.
        idle_nw: ((if sleeps { idle_w } else { average_w }) * 1.0e9) as u64,
        wake_nj: if sleeps { wake_nj as u64 } else { 0 },
    };
    let used: u32 = design.parts.iter().filter_map(|q| registry.module(&q.module)).map(|s| s.area()).sum();
    let die_sites = u32::from(design.width) * u32::from(design.height);
    let side_mm = p.site_um / 1000.0;
    Some(Report {
        die_sites,
        used_sites: used,
        die_mm2: die_sites as f32 * side_mm * side_mm,
        wire_sites: design.nets.iter().map(Net::length).sum(),
        vias: design.nets.iter().map(Net::vias).sum(),
        critical_ns,
        critical_path: critical.1,
        max_clock_hz,
        clock_hz,
        cycles_per_wake: cycles,
        instructions_per_s: clock_hz * instructions as f32 / cycles.max(1) as f32,
        handler_us,
        busy_us,
        max_wake_hz,
        channels,
        outputs,
        rom_bytes: slaves("imem").iter().map(|(_, s)| s.bytes).sum(),
        ram_bytes: crate::compile::CORE_REGISTER_BYTES + slaves("dmem").iter().map(|(_, s)| s.bytes).sum::<u32>(),
        sleeps,
        wake_ms,
        worst_response_ms: wake_ms + busy_us / 1000.0,
        idle_w,
        wake_uj: wake_nj / 1000.0,
        average_w,
        model,
    })
}

