//! Design checks (floorplan, ports, functional reachability, firmware fit,
//! extracted wires) and the characterisation they unlock. Every failure says
//! what is wrong and, where one exists, offers one concrete fix.
use crate::characterize::{Report, characterize};
use crate::design::{Design, Net, Part};
use crate::extract::extract;
use crate::registry::{Dir, ModuleSpec, PortSpec, Registry, Role, Signal};

/// Bumped whenever checks or characterisation change results. Revisions
/// record it; a released revision keeps the model it was released with.
pub const COMPILER_VERSION: u32 = 1;
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
        err(&mut d, format!("Unknown process {}", design.process), Some(Fix::UsePreset));
        return Compiled { diagnostics: d, report: None };
    };
    if design.layers == 0 || design.layers > process.max_layers {
        err(&mut d, format!("{} supports 1–{} metal layers", process.name, process.max_layers), None);
    }
    floorplan(design, registry, &mut d);
    nets(design, registry, &mut d);
    crate::loops::cycles(design, registry, &mut d);
    let core = function(design, registry, &mut d);

    let wires = extract(design, registry);
    if !wires.unrouted.is_empty() {
        err(&mut d, format!("{} net(s) have no wire yet", wires.unrouted.len()), Some(Fix::RepairRoutes));
    }
    for &id in &wires.opens {
        err(&mut d, format!("Net {} does not reach all of its pins", net_name(design, id)), Some(Fix::RepairRoutes));
    }
    for &(a, b) in &wires.shorts {
        err(&mut d, format!("Short: nets {} and {} share a site on one layer", net_name(design, a), net_name(design, b)), Some(Fix::RepairRoutes));
    }
    for (id, why) in &wires.illegal {
        err(&mut d, format!("Wire of net {} {why}", net_name(design, *id)), Some(Fix::RepairRoutes));
    }
    if design.layers == 1 && !wires.unrouted.is_empty() {
        warn(&mut d, "Routing is congested on a single layer; a second metal layer usually frees it".into(), Some(Fix::AddLayer));
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
            err(d, format!("Module {} is not in the verified library", part.module), None);
            continue;
        };
        let Some((x, y, w, h)) = design.rect(registry, part) else { continue };
        if u16::from(x) + u16::from(w) > u16::from(design.width) || u16::from(y) + u16::from(h) > u16::from(design.height) {
            err(d, format!("{} leaves the die", label(spec, part)), None);
        }
        if spec.edge && !(x == 0 || y == 0 || x + w >= design.width || y + h >= design.height) {
            err(d, format!("{} has pads: it must touch the die edge", label(spec, part)), Some(Fix::MoveToEdge(part.id)));
        }
        for other in &design.parts[..i] {
            if let Some((ox, oy, ow, oh)) = design.rect(registry, other)
                && x < ox + ow && ox < x + w && y < oy + oh && oy < y + h
            {
                err(d, format!("{} overlaps module #{}", label(spec, part), other.id), None);
            }
        }
    }
}

fn nets(design: &Design, registry: &Registry, d: &mut Vec<Diagnostic>) {
    for net in &design.nets {
        let pins = resolve(design, registry, net);
        if pins.len() != net.pins.len() {
            err(d, format!("Net {} names a port that does not exist", net.name), Some(Fix::UsePreset));
            continue;
        }
        let Some(&(_, _, first)) = pins.first() else { continue };
        if let Some((p, s, port)) = pins.iter().find(|(_, _, q)| q.signal != first.signal) {
            err(d, format!("Net {}: {}.{} is {:?}, the net carries {:?}", net.name, label(s, p), port.name, port.signal, first.signal), None);
            continue;
        }
        if let Some((p, s, port)) = pins.iter().find(|(_, _, q)| q.width != first.width) {
            err(d, format!("Net {}: {}.{} is {} bits wide, the net {}", net.name, label(s, p), port.name, port.width, first.width), None);
        }
        let drivers = pins.iter().filter(|(_, _, q)| q.dir == Dir::Drive).count();
        if drivers == 0 {
            err(d, format!("Net {} ({:?}) has no driver", net.name, first.signal), None);
        } else if drivers > 1 {
            err(d, format!("Net {} has {drivers} drivers fighting", net.name), None);
        }
    }
    for part in &design.parts {
        let Some(spec) = registry.module(&part.module) else { continue };
        for port in &spec.ports {
            let joined = net_at(design, part.id, &port.name).is_some();
            let required = !port.optional && (port.dir == Dir::Sink || spec.role == Role::Core);
            if required && !joined {
                let what = match port.signal {
                    Signal::Power => "is unpowered",
                    Signal::Clock => "has no clock",
                    Signal::Reset => "has no reset",
                    _ => "is not connected",
                };
                err(d, format!("{} port {} {what}", label(spec, part), port.name), Some(Fix::Connect { part: part.id, port: port.name.clone() }));
            } else if !joined && port.dir == Dir::Drive && !port.optional {
                warn(d, format!("{}.{} drives nothing", label(spec, part), port.name), None);
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
        err(d, "No supply pads: nothing on the die is powered".into(), Some(Fix::Add(Role::Supply)));
    }
    if role(Role::Clock).is_empty() {
        err(d, "No clock source".into(), Some(Fix::Add(Role::Clock)));
    }
    let core = match cores.as_slice() {
        [] => {
            err(d, "No control core: nothing runs the firmware".into(), Some(Fix::Add(Role::Core)));
            return None;
        }
        [one] => *one,
        _ => {
            err(d, "This compiler supports one control core per die".into(), None);
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
        err(d, "No program memory on the core's instruction bus: the core cannot execute".into(), Some(Fix::Add(Role::Rom)));
    }
    let mix = design.firmware.mix();
    let rom: u32 = roms.iter().map(|s| s.bytes).sum();
    if !roms.is_empty() && mix.rom_bytes > rom {
        err(d, format!("Firmware needs {} B of ROM, the instruction bus reaches {rom} B", mix.rom_bytes), Some(Fix::Add(Role::Rom)));
    }
    let ram: u32 = CORE_REGISTER_BYTES + on_bus("dmem", Role::Sram).iter().map(|s| s.bytes).sum::<u32>();
    if mix.ram_bytes > ram {
        err(d, format!("{} needs {} B of RAM, the core has {ram} B: add SRAM on the data bus", design.firmware.label(), mix.ram_bytes), Some(Fix::Add(Role::Sram)));
    }
    let adcs: u32 = on_bus("pbus", Role::Adc).iter().map(|s| s.channels).sum();
    if u32::from(design.firmware.channels) > adcs {
        err(d, format!("Firmware reads {} sensor(s), the peripheral bus reaches {adcs} ADC channel(s)", design.firmware.channels), Some(Fix::Add(Role::Adc)));
    }
    if on_bus("pbus", Role::Gpio).is_empty() {
        err(d, "No GPIO on the peripheral bus: the controller cannot drive the machine".into(), Some(Fix::Add(Role::Gpio)));
    }
    let alus_on_bus = on_bus("alu", Role::Alu).len();
    let alus = role(Role::Alu).len();
    if alus > alus_on_bus {
        warn(d, format!("{} ALU(s) are not on the core's ALU bus: they cost area and leakage but add no throughput", alus - alus_on_bus), None);
    }
    if role(Role::Timer).len() > on_bus("pbus", Role::Timer).len() {
        warn(d, "A timer is not on the peripheral bus: the core cannot sleep on it".into(), None);
    }
    Some(core)
}
