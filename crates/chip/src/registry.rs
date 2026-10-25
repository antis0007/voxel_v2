//! Verified macro modules and the process they are characterised on, loaded
//! from `assets/chip_modules.ron`. A module is a functional block with typed
//! ports; nothing a player draws becomes logic unless it is one of these.
use serde::{Deserialize, Serialize};

const SOURCE: &str = include_str!("../../../assets/chip_modules.ron");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Signal {
    Power,
    Clock,
    Reset,
    MemBus,
    PeriphBus,
    AluBus,
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    Drive,
    Sink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Supply,
    Clock,
    Core,
    Alu,
    Rom,
    Sram,
    Timer,
    Gpio,
    Adc,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Port")]
pub struct PortSpec {
    pub name: String,
    pub signal: Signal,
    pub dir: Dir,
    pub width: u8,
    /// Site inside the unrotated footprint.
    pub at: (u8, u8),
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}
fn is_zero_f(v: &f32) -> bool {
    *v == 0.0
}
fn is_zero(v: &u32) -> bool {
    *v == 0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Module")]
pub struct ModuleSpec {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub size: (u8, u8),
    /// Pads: the footprint must touch the die edge.
    #[serde(default, skip_serializing_if = "is_false")]
    pub edge: bool,
    /// No register cut: the output follows the inputs after `delay_ns`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub combinational: bool,
    /// Clock-to-output (sequential) or input-to-output (combinational), ns.
    pub delay_ns: f32,
    #[serde(default, skip_serializing_if = "is_zero_f")]
    pub setup_ns: f32,
    /// Leakage and bias with the core clock gated, µW.
    pub idle_uw: f32,
    /// Extra power while the core executes, µW.
    #[serde(default, skip_serializing_if = "is_zero_f")]
    pub run_uw: f32,
    /// Energy of one use (instruction, access, conversion), nJ.
    #[serde(default, skip_serializing_if = "is_zero_f")]
    pub event_nj: f32,
    /// ADC conversion time or the timer's shortest wake period, µs.
    #[serde(default, skip_serializing_if = "is_zero_f")]
    pub time_us: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bytes: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub channels: u32,
    /// Oscillator frequency, or the highest clock a core is rated for.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub hz: u32,
    /// Core: cycles per instruction, and per ALU instruction without an ALU.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cpi: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub soft_alu_cpi: u32,
    pub ports: Vec<PortSpec>,
}

impl ModuleSpec {
    pub fn port(&self, name: &str) -> Option<&PortSpec> {
        self.ports.iter().find(|p| p.name == name)
    }
    pub fn area(&self) -> u32 {
        u32::from(self.size.0) * u32::from(self.size.1)
    }
}

/// Interconnect and margin figures of one manufacturing process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Process {
    pub id: String,
    pub name: String,
    /// Physical pitch of one placement site.
    pub site_um: f32,
    pub vdd: f32,
    /// Linear (driver) and quadratic (distributed RC) wire delay per site.
    pub wire_ns_per_site: f32,
    pub wire_ns_per_site2: f32,
    pub via_ns: f32,
    pub wire_pf_per_site: f32,
    /// Clock period over the critical path.
    pub margin: f32,
    pub max_layers: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "ChipTable")]
pub struct Registry {
    pub processes: Vec<Process>,
    pub modules: Vec<ModuleSpec>,
}

impl Registry {
    pub fn parse(source: &str) -> Result<Self, String> {
        let table: Self = ron::from_str(source).map_err(|e| e.to_string())?;
        table.validate()?;
        Ok(table)
    }

    /// The shipped table. It is validated by a test, so a failure here is a
    /// build-time content error.
    pub fn builtin() -> Self {
        Self::parse(SOURCE).unwrap_or_else(|e| panic!("assets/chip_modules.ron: {e}"))
    }

    pub fn module(&self, id: &str) -> Option<&ModuleSpec> {
        self.modules.iter().find(|m| m.id == id)
    }

    pub fn process(&self, id: &str) -> Option<&Process> {
        self.processes.iter().find(|p| p.id == id)
    }

    fn validate(&self) -> Result<(), String> {
        for (i, m) in self.modules.iter().enumerate() {
            if self.modules[..i].iter().any(|o| o.id == m.id) {
                return Err(format!("duplicate module id {}", m.id));
            }
            if m.size.0 == 0 || m.size.1 == 0 {
                return Err(format!("{}: empty footprint", m.id));
            }
            for (j, p) in m.ports.iter().enumerate() {
                if p.at.0 >= m.size.0 || p.at.1 >= m.size.1 {
                    return Err(format!("{}.{}: port outside footprint", m.id, p.name));
                }
                if m.ports[..j].iter().any(|o| o.name == p.name || o.at == p.at) {
                    return Err(format!("{}.{}: duplicate port name or site", m.id, p.name));
                }
                if p.width == 0 {
                    return Err(format!("{}.{}: zero width", m.id, p.name));
                }
            }
            if m.role == Role::Core && (m.cpi == 0 || m.hz == 0) {
                return Err(format!("{}: core needs cpi and hz", m.id));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chip_modules_ron_round_trips() {
        let table = Registry::builtin();
        let text = ron::ser::to_string_pretty(&table, Default::default()).unwrap();
        assert_eq!(Registry::parse(&text).unwrap(), table);
        assert!(table.module("core8").is_some() && table.process("p8um").is_some());
    }

    #[test]
    fn invalid_ports_are_rejected() {
        let mut table = Registry::builtin();
        table.modules[0].ports[0].at = (9, 9);
        assert!(table.validate().unwrap_err().contains("outside footprint"));
    }
}
