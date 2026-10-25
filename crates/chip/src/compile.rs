//! Design checks (floorplan, ports, functional reachability, firmware fit,
//! extracted wires) and the characterisation they unlock. Every failure says
//! what is wrong and, where one exists, offers one concrete fix.
use crate::characterize::{Report, characterize};
use crate::design::{Design, Net, Part};
use crate::extract::extract;
use crate::registry::{Dir, ModuleSpec, PortSpec, Registry, Role, Signal};

/// Bumped whenever checks or characterisation change results. Revisions
/// record it; a released revision keeps the model it was released with.
pub const COMPILER_VERSION: u32 = 2;
/// Bytes of register file inside the core, usable as task RAM without SRAM.
pub const CORE_REGISTER_BYTES: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fix {
    UsePreset,
    RepairRoutes,
    Add(Role),
    Connect { part: u16, port: String },
    MoveToEdge(u16),
    AddLayer,
    SampleAtLeast(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub fix: Option<Fix>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Compiled {
    pub diagnostics: Vec<Diagnostic>,
    pub report: Option<Report>,
}

impl Compiled {
    pub fn ok(&self) -> bool {
        self.report.is_some() && self.diagnostics.iter().all(|d| d.severity != Severity::Error)
    }
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error)
    }
}

pub(crate) fn err(out: &mut Vec<Diagnostic>, message: String, fix: Option<Fix>) {
    out.push(Diagnostic { severity: Severity::Error, message, fix });
}
fn warn(out: &mut Vec<Diagnostic>, message: String, fix: Option<Fix>) {
    out.push(Diagnostic { severity: Severity::Warning, message, fix });
}

/// A pin resolved to its part, module and port.
pub type Resolved<'a> = (&'a Part, &'a ModuleSpec, &'a PortSpec);

pub fn resolve<'a>(design: &'a Design, registry: &'a Registry, net: &'a Net) -> Vec<Resolved<'a>> {
    net.pins
        .iter()
        .filter_map(|pin| {
            let part = design.part(pin.part)?;
            let spec = registry.module(&part.module)?;
            Some((part, spec, spec.port(&pin.port)?))
        })
        .collect()
}

/// Net of a port, if it joins at least one other pin.
pub fn net_at<'a>(design: &'a Design, part: u16, port: &str) -> Option<&'a Net> {
    design.nets.iter().find(|n| n.pins.len() >= 2 && n.pins.iter().any(|p| p.part == part && p.port == port))
}

pub fn label(spec: &ModuleSpec, part: &Part) -> String {
    format!("{} #{}", spec.name, part.id)
}

pub fn compile(design: &Design, registry: &Registry) -> Compiled {
    let mut d = Vec::new();
    let Some(process) = registry.process(&design.process) else {
        err(&mut d, format!("Unknown Process: {}", design.process), Some(Fix::UsePreset));
        return Compiled { diagnostics: d, report: None };
    };
    if design.layers == 0 || design.layers > process.max_layers {
        err(&mut d, format!("Layers {}: Process Allows 1–{}", design.layers, process.max_layers), None);
    }
    floorplan(design, registry, &mut d);
    nets(design, registry, &mut d);
    crate::loops::cycles(design, registry, &mut d);
    let core = function(design, registry, &mut d);

    let wires = extract(design, registry);
    if !wires.unrouted.is_empty() {
        err(&mut d, format!("Unrouted: {} Nets", wires.unrouted.len()), Some(Fix::RepairRoutes));
    }
    for &id in &wires.opens {
        err(&mut d, format!("Open: {}", net_name(design, id)), Some(Fix::RepairRoutes));
    }
    for &(a, b) in &wires.shorts {
        err(&mut d, format!("Short: {} × {}", net_name(design, a), net_name(design, b)), Some(Fix::RepairRoutes));
    }
    for (id, why) in &wires.illegal {
        err(&mut d, format!("Bad Wire: {} · {why}", net_name(design, *id)), Some(Fix::RepairRoutes));
    }
    if design.layers == 1 && !wires.unrouted.is_empty() {
        warn(&mut d, "Congested: 1 Layer".into(), Some(Fix::AddLayer));
    }

    let has_errors = d.iter().any(|x| x.severity == Severity::Error);
    let report = match core {
        Some(core) if !has_errors => characterize(design, registry, process, core, &mut d),
        _ => None,
    };
    Compiled { diagnostics: d, report }
}

fn net_name(design: &Design, id: u16) -> String {
    design.nets.iter().find(|n| n.id == id).map_or_else(|| format!("#{id}"), |n| n.name.clone())
}

fn floorplan(design: &Design, registry: &Registry, d: &mut Vec<Diagnostic>) {
    for (i, part) in design.parts.iter().enumerate() {
        let Some(spec) = registry.module(&part.module) else {
            err(d, format!("Unknown Module: {}", part.module), None);
            continue;
        };
        let Some((x, y, w, h)) = design.rect(registry, part) else { continue };
        if u16::from(x) + u16::from(w) > u16::from(design.width) || u16::from(y) + u16::from(h) > u16::from(design.height) {
            err(d, format!("Off Die: {}", label(spec, part)), None);
        }
        if spec.edge && !(x == 0 || y == 0 || x + w >= design.width || y + h >= design.height) {
            err(d, format!("Pads Off Edge: {}", label(spec, part)), Some(Fix::MoveToEdge(part.id)));
        }
        for other in &design.parts[..i] {
            if let Some((ox, oy, ow, oh)) = design.rect(registry, other)
                && x < ox + ow && ox < x + w && y < oy + oh && oy < y + h
            {
                err(d, format!("Overlap: {} × #{}", label(spec, part), other.id), None);
            }
        }
    }
}

fn nets(design: &Design, registry: &Registry, d: &mut Vec<Diagnostic>) {
    for net in &design.nets {
        let pins = resolve(design, registry, net);
        if pins.len() != net.pins.len() {
            err(d, format!("Unknown Port: {}", net.name), Some(Fix::UsePreset));
            continue;
        }
        let Some(&(_, _, first)) = pins.first() else { continue };
        if let Some((p, s, port)) = pins.iter().find(|(_, _, q)| q.signal != first.signal) {
            err(d, format!("Type Mismatch: {} {} {} on {} Net", label(s, p), port.name.to_uppercase(), port.signal.label(), first.signal.label()), None);
            continue;
        }
        if let Some((p, s, port)) = pins.iter().find(|(_, _, q)| q.width != first.width) {
            err(d, format!("Width Mismatch: {} {} {} b, Net {} b", label(s, p), port.name.to_uppercase(), port.width, first.width), None);
        }
        let drivers = pins.iter().filter(|(_, _, q)| q.dir == Dir::Drive).count();
        if drivers == 0 {
            err(d, format!("No Driver: {}", net.name), None);
        } else if drivers > 1 {
            err(d, format!("{drivers} Drivers: {}", net.name), None);
        }
    }
    for part in &design.parts {
        let Some(spec) = registry.module(&part.module) else { continue };
        for port in &spec.ports {
            let joined = net_at(design, part.id, &port.name).is_some();
            let required = !port.optional && (port.dir == Dir::Sink || spec.role == Role::Core);
            if required && !joined {
                let what = match port.signal {
                    Signal::Power => "Unpowered",
                    Signal::Clock => "No Clock",
                    Signal::Reset => "No Reset",
                    _ => "Unconnected",
                };
                err(d, format!("{what}: {} {}", label(spec, part), port.name.to_uppercase()), Some(Fix::Connect { part: part.id, port: port.name.clone() }));
            } else if !joined && port.dir == Dir::Drive && !port.optional {
                warn(d, format!("Unused Output: {} {}", label(spec, part), port.name.to_uppercase()), None);
            }
        }
    }
}

/// Functional reachability from the core and firmware fit. Returns the core.
fn function<'a>(design: &'a Design, registry: &'a Registry, d: &mut Vec<Diagnostic>) -> Option<&'a Part> {
    let role = |r: Role| -> Vec<&Part> {
        design.parts.iter().filter(|p| registry.module(&p.module).is_some_and(|s| s.role == r)).collect()
    };
    let cores = role(Role::Core);
    if role(Role::Supply).is_empty() {
        err(d, "No Supply Pads".into(), Some(Fix::Add(Role::Supply)));
    }
    if role(Role::Clock).is_empty() {
        err(d, "No Clock Source".into(), Some(Fix::Add(Role::Clock)));
    }
    let core = match cores.as_slice() {
        [] => {
            err(d, "No Core".into(), Some(Fix::Add(Role::Core)));
            return None;
        }
        [one] => *one,
        _ => {
            err(d, "Cores: 1 Per Die".into(), None);
            return None;
        }
    };
    let on_bus = |port: &str, r: Role| -> Vec<&ModuleSpec> {
        net_at(design, core.id, port)
            .map(|n| resolve(design, registry, n).into_iter().filter(|(_, s, _)| s.role == r).map(|(_, s, _)| s).collect())
            .unwrap_or_default()
    };
    let roms = on_bus("imem", Role::Rom);
    if roms.is_empty() {
        err(d, "No Program ROM".into(), Some(Fix::Add(Role::Rom)));
    }
    let mix = design.firmware.mix();
    let rom: u32 = roms.iter().map(|s| s.bytes).sum();
    if !roms.is_empty() && mix.rom_bytes > rom {
        err(d, format!("ROM {} / {rom} B", mix.rom_bytes), Some(Fix::Add(Role::Rom)));
    }
    let ram: u32 = CORE_REGISTER_BYTES + on_bus("dmem", Role::Sram).iter().map(|s| s.bytes).sum::<u32>();
    if mix.ram_bytes > ram {
        err(d, format!("RAM {} / {ram} B", mix.ram_bytes), Some(Fix::Add(Role::Sram)));
    }
    let adcs: u32 = on_bus("pbus", Role::Adc).iter().map(|s| s.channels).sum();
    if u32::from(design.firmware.channels) > adcs {
        err(d, format!("Sensors {} / {adcs}", design.firmware.channels), Some(Fix::Add(Role::Adc)));
    }
    if on_bus("pbus", Role::Gpio).is_empty() {
        err(d, "No GPIO".into(), Some(Fix::Add(Role::Gpio)));
    }
    let alus_on_bus = on_bus("alu", Role::Alu).len();
    let alus = role(Role::Alu).len();
    if alus > alus_on_bus {
        warn(d, format!("ALU Unused: {} Off ALU Bus", alus - alus_on_bus), None);
    }
    if on_bus("pbus", Role::Watchdog).is_empty() {
        warn(d, "No Watchdog: Brownout Hangs Core".into(), Some(Fix::Add(Role::Watchdog)));
    }
    if role(Role::Timer).len() > on_bus("pbus", Role::Timer).len() {
        warn(d, "Timer Off Bus".into(), None);
    }
    Some(core)
}
