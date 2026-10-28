//! Per-world rules, stored as a checksummed sidecar in the world's own folder.
//!
//! A sidecar keeps the save format unchanged: worlds without one load with
//! `Rules::default()`, which matches how every world behaved before rules.
use anyhow::{Context, Result, bail, ensure};
use std::path::Path;

pub const FILE: &str = "rules.vxg";
const MAGIC: &[u8; 8] = b"VXRUL001";
/// Magic, mode, flags, protection radius, checksum.
const LEN: usize = 8 + 1 + 1 + 2 + 8;
pub const MAX_SPAWN_PROTECTION_M: u16 = 256;
const KEEP_INVENTORY: u8 = 1;
const MOB_SPAWNING: u8 = 1 << 1;
const DAYLIGHT_CYCLE: u8 = 1 << 2;
const PVP: u8 = 1 << 3;
const KNOWN_FLAGS: u8 = KEEP_INVENTORY | MOB_SPAWNING | DAYLIGHT_CYCLE | PVP;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameMode {
    /// Gathered blocks only; no creative tools, catalogue or flight.
    Survival,
    /// Worlds from before rules existed load here so their tools stay.
    #[default]
    Creative,
}

impl GameMode {
    pub const ALL: [Self; 2] = [Self::Survival, Self::Creative];

    pub fn label(self) -> &'static str {
        match self {
            Self::Survival => "Survival",
            Self::Creative => "Creative",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Survival => "Creative Tools: Off · Flight: Off",
            Self::Creative => "Creative Tools: On · Flight: On",
        }
    }

    /// Shape tools, the creative panel and the item catalogue.
    pub fn creative_tools(self) -> bool {
        self == Self::Creative
    }

    pub fn flight(self) -> bool {
        self == Self::Creative
    }

    fn code(self) -> u8 {
        match self {
            Self::Survival => 0,
            Self::Creative => 1,
        }
    }

    fn from_code(code: u8) -> Result<Self> {
        Ok(match code {
            0 => Self::Survival,
            1 => Self::Creative,
            _ => bail!("Unknown game mode {code}"),
        })
    }
}

/// The rules one world plays by. Network play reads `pvp` and
/// `spawn_protection_m` from the host world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub mode: GameMode,
    /// A death keeps every carried stack instead of dropping it.
    pub keep_inventory: bool,
    /// Natural hostile and wildlife spawns. Awakened bosses ignore it.
    pub mob_spawning: bool,
    /// False freezes the time of day.
    pub daylight_cycle: bool,
    /// Players may damage each other.
    pub pvp: bool,
    /// Only the owner edits blocks within this horizontal radius of the world spawn.
    pub spawn_protection_m: u16,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            mode: GameMode::Creative,
            keep_inventory: false,
            mob_spawning: true,
            daylight_cycle: true,
            pvp: true,
            spawn_protection_m: 0,
        }
    }
}

impl Rules {
    /// A new world's rules: the chosen mode, everything else at its default.
    pub fn new_world(mode: GameMode) -> Self {
        Self { mode, ..Self::default() }
    }

    /// True when `feet_m` lies within the protected radius around `spawn_m`.
    pub fn protects(&self, spawn_m: glam::Vec3, feet_m: glam::Vec3) -> bool {
        let r = f32::from(self.spawn_protection_m);
        r > 0.0 && glam::Vec2::new(feet_m.x - spawn_m.x, feet_m.z - spawn_m.z).length_squared() < r * r
    }

    pub fn encode(&self) -> Vec<u8> {
        let flags = [
            (self.keep_inventory, KEEP_INVENTORY),
            (self.mob_spawning, MOB_SPAWNING),
            (self.daylight_cycle, DAYLIGHT_CYCLE),
            (self.pvp, PVP),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(0, |acc, (_, bit)| acc | bit);
        let mut out = Vec::with_capacity(LEN);
        out.extend_from_slice(MAGIC);
        out.push(self.mode.code());
        out.push(flags);
        out.extend_from_slice(&self.spawn_protection_m.min(MAX_SPAWN_PROTECTION_M).to_le_bytes());
        let sum = checksum(&out);
        out.extend_from_slice(&sum.to_le_bytes());
        out
    }

    /// Rejects any file that is not exactly one well-formed record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() == LEN, "Rules file length {} is not {LEN}", bytes.len());
        ensure!(bytes.starts_with(MAGIC), "Unsupported rules file");
        let (body, sum) = bytes.split_at(LEN - 8);
        let sum = u64::from_le_bytes(sum.try_into().context("Rules checksum")?);
        ensure!(checksum(body) == sum, "Rules file checksum mismatch");
        let mode = GameMode::from_code(body[8])?;
        let flags = body[9];
        ensure!(flags & !KNOWN_FLAGS == 0, "Unknown rules flags {flags:#04x}");
        let spawn_protection_m = u16::from_le_bytes([body[10], body[11]]);
        ensure!(spawn_protection_m <= MAX_SPAWN_PROTECTION_M, "Spawn protection out of range");
        Ok(Self {
            mode,
            keep_inventory: flags & KEEP_INVENTORY != 0,
            mob_spawning: flags & MOB_SPAWNING != 0,
            daylight_cycle: flags & DAYLIGHT_CYCLE != 0,
            pvp: flags & PVP != 0,
            spawn_protection_m,
        })
    }

    /// The rules in `world_dir`. A missing file is `Ok(Rules::default())`.
    pub fn load(world_dir: &Path) -> Result<Self> {
        let path = world_dir.join(FILE);
        match std::fs::read(&path) {
            Ok(bytes) => Self::decode(&bytes).with_context(|| path.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("Cannot read {}", path.display())),
        }
    }

    /// Replaces the file through a synced temporary so a crash keeps the old one.
    pub fn save(&self, world_dir: &Path) -> Result<()> {
        use std::io::Write;
        let path = world_dir.join(FILE);
        let tmp = world_dir.join(format!("{FILE}.tmp"));
        let mut file = std::fs::File::create(&tmp)
            .with_context(|| format!("Cannot create {}", tmp.display()))?;
        file.write_all(&self.encode())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, &path).with_context(|| format!("Cannot publish {}", path.display()))
    }
}

impl super::World {
    /// Applies `rules` and writes them if the world folder exists yet.
    ///
    /// Every checkpoint also writes them, so a failed write here is retried there.
    pub fn set_rules(&mut self, rules: Rules) {
        self.rules = rules;
        let dir = self.regions.view.root.clone();
        if dir.is_dir()
            && let Err(e) = rules.save(&dir)
        {
            eprintln!("[rules] {e:#}");
        }
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
