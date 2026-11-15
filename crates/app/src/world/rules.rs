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
    /// Gathered blocks only; traversal comes from earned abilities.
    Survival,
    /// Worlds from before rules existed load here so their tools stay.
    #[default]
    Creative,
    /// Noclip flight with no interaction; mobs never see the player.
    Spectator,
    /// Survival without breaking or placing blocks.
    Adventure,
}

impl GameMode {
    /// Modes offered for a new world. Spectator and Adventure come from `/gamemode`.
    pub const ALL: [Self; 2] = [Self::Survival, Self::Creative];
    pub const EVERY: [Self; 4] = [Self::Survival, Self::Creative, Self::Spectator, Self::Adventure];

    pub fn label(self) -> &'static str {
        match self {
            Self::Survival => "Survival",
            Self::Creative => "Creative",
            Self::Spectator => "Spectator",
            Self::Adventure => "Adventure",
        }
    }

    /// Accepts the label, its first letter or the classic number, any case.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word.to_ascii_lowercase().as_str() {
            "survival" | "s" | "0" => Self::Survival,
            "creative" | "c" | "1" => Self::Creative,
            "adventure" | "a" | "2" => Self::Adventure,
            "spectator" | "sp" | "3" => Self::Spectator,
            _ => return None,
        })
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Survival => "Creative Tools: Off · Levitation: Earned",
            Self::Creative => "Creative Tools: On · Flight: On",
            Self::Spectator => "Interaction: Off · Flight: Noclip",
            Self::Adventure => "Block Edits: Off",
        }
    }

    /// Shape tools, the creative panel and the item catalogue.
    pub fn creative_tools(self) -> bool {
        self == Self::Creative
    }

    pub fn flight(self) -> bool {
        matches!(self, Self::Creative | Self::Spectator)
    }

    /// Casting, melee, use and block edits.
    pub fn interacts(self) -> bool {
        self != Self::Spectator
    }

    pub fn edits_blocks(self) -> bool {
        matches!(self, Self::Survival | Self::Creative)
    }

    /// Creatures can acquire the player as a hostile target.
    pub fn visible_to_mobs(self) -> bool {
        matches!(self, Self::Survival | Self::Adventure)
    }

    fn code(self) -> u8 {
        match self {
            Self::Survival => 0,
            Self::Creative => 1,
            Self::Spectator => 2,
            Self::Adventure => 3,
        }
    }

    fn from_code(code: u8) -> Result<Self> {
        Ok(match code {
            0 => Self::Survival,
            1 => Self::Creative,
            2 => Self::Spectator,
            3 => Self::Adventure,
            _ => bail!("Unknown game mode {code}"),
        })
    }
}

/// The rules one world plays by. Network play reads `pvp` and
/// `spawn_protection_m` from the host world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub stacks: super::inventory::stacks::Balance,
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
    /// Horizontal radius around x 0, z 0 that players stay inside; 0 is off.
    /// Session-only: the sidecar never stores it, so a reload clears it.
    pub border_m: u32,
    /// Gameplay systems this world plays with; stored in `systems::FILE`.
    pub systems: super::systems::Systems,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            stacks: super::inventory::stacks::Balance::Balanced,
            mode: GameMode::Creative,
            keep_inventory: false,
            mob_spawning: true,
            daylight_cycle: true,
            pvp: true,
            spawn_protection_m: 0,
            border_m: 0,
            systems: super::systems::Systems::default(),
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

    /// Metres from `feet_m` to the border, or `None` with no border.
    pub fn border_distance(&self, feet_m: glam::Vec3) -> Option<f32> {
        (self.border_m > 0).then(|| self.border_m as f32 - glam::Vec2::new(feet_m.x, feet_m.z).length())
    }

    /// `feet_m` pulled back inside the border, or `None` when already inside.
    pub fn border_clamp(&self, feet_m: glam::Vec3) -> Option<glam::Vec3> {
        let r = self.border_m as f32;
        let flat = glam::Vec2::new(feet_m.x, feet_m.z);
        if self.border_m == 0 || flat.length_squared() <= r * r {
            return None;
        }
        let inside = flat.normalize_or_zero() * r;
        Some(glam::Vec3::new(inside.x, feet_m.y, inside.y))
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
        out.push(self.stacks.code() as u8);
        let sum = checksum(&out);
        out.extend_from_slice(&sum.to_le_bytes());
        out
    }

    /// Rejects any file that is not exactly one well-formed record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() == LEN || bytes.len() == LEN + 1, "Invalid rules file length");
        ensure!(bytes.starts_with(MAGIC), "Unsupported rules file");
        let (body, sum) = bytes.split_at(bytes.len() - 8);
        let sum = u64::from_le_bytes(sum.try_into().context("Rules checksum")?);
        ensure!(checksum(body) == sum, "Rules file checksum mismatch");
        let mode = GameMode::from_code(body[8])?;
        let flags = body[9];
        ensure!(flags & !KNOWN_FLAGS == 0, "Unknown rules flags {flags:#04x}");
        let spawn_protection_m = u16::from_le_bytes([body[10], body[11]]);
        ensure!(spawn_protection_m <= MAX_SPAWN_PROTECTION_M, "Spawn protection out of range");
        Ok(Self {
            stacks: if body.len() > 12 { super::inventory::stacks::Balance::decode(u32::from(body[12]))? } else { super::inventory::stacks::Balance::Balanced },
            mode,
            keep_inventory: flags & KEEP_INVENTORY != 0,
            mob_spawning: flags & MOB_SPAWNING != 0,
            daylight_cycle: flags & DAYLIGHT_CYCLE != 0,
            pvp: flags & PVP != 0,
            spawn_protection_m,
            border_m: 0,
            systems: super::systems::Systems::default(),
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
        .map(|rules| Self { systems: Self::load_systems(world_dir), ..rules })
    }

    /// A damaged systems sidecar plays with every system rather than block the world.
    fn load_systems(world_dir: &Path) -> super::systems::Systems {
        super::systems::Systems::load(world_dir).unwrap_or_else(|e| {
            eprintln!("[systems] Using every system: {e:#}");
            super::systems::Systems::default()
        })
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
        std::fs::rename(&tmp, &path).with_context(|| format!("Cannot publish {}", path.display()))?;
        self.systems.save(world_dir)
    }
}

impl super::World {
    /// Applies `rules` and writes them if the world folder exists yet.
    ///
    /// Every checkpoint also writes them, so a failed write here is retried there.
    pub fn set_rules(&mut self, rules: Rules) {
        self.rules = rules;
        rules.systems.publish();
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
