//! Player statistics, advancements and the clock and map items.
//! Persists in `sidecar::FILE` beside the world, not in the save body.
pub mod advancements;
pub mod sidecar;
use super::World;
use super::inventory::Item;
use crate::audio::{Cue, Event};
use crate::player::Player;
use glam::Vec3;

pub const CLOCK: Item = Item::Tool(40);
pub const MAP: Item = Item::Tool(41);

/// Seconds an unlock toast stays on screen.
const TOAST_SECONDS: f32 = 4.0;
/// Larger per-step moves are teleports or respawns and are not counted.
const MAX_STEP_METRES: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Stat {
    Mined,
    Placed,
    /// Centimetres on the ground.
    Walked,
    /// Centimetres in flight.
    Flown,
    /// Centimetres in water.
    Swum,
    Kills,
    Deaths,
    Crafted,
    /// Milliseconds in the world.
    PlayTime,
    Studies,
    Research,
    /// Bit set of `assets/discoveries.ron` places visited; read through `State::get`.
    Places,
}

impl Stat {
    pub const ALL: [Stat; 12] = [
        Stat::Mined,
        Stat::Placed,
        Stat::Walked,
        Stat::Flown,
        Stat::Swum,
        Stat::Kills,
        Stat::Deaths,
        Stat::Crafted,
        Stat::PlayTime,
        Stat::Studies,
        Stat::Research,
        Stat::Places,
    ];
    pub const COUNT: usize = Self::ALL.len();

    pub fn label(self) -> &'static str {
        match self {
            Stat::Mined => "Blocks Mined",
            Stat::Placed => "Blocks Placed",
            Stat::Walked => "Distance Walked",
            Stat::Flown => "Distance Flown",
            Stat::Swum => "Distance Swum",
            Stat::Kills => "Mobs Killed",
            Stat::Deaths => "Deaths",
            Stat::Crafted => "Items Crafted",
            Stat::PlayTime => "Play Time",
            Stat::Studies => "Lessons Learned",
            Stat::Research => "Research Batches",
            Stat::Places => "Places Discovered",
        }
    }

    /// The value with its unit, as the statistics panel shows it.
    pub fn format(self, value: u64) -> String {
        match self {
            Stat::Walked | Stat::Flown | Stat::Swum if value >= 100_000 => {
                format!("{:.2} km", value as f64 / 100_000.0)
            }
            Stat::Walked | Stat::Flown | Stat::Swum => format!("{} m", value / 100),
            Stat::PlayTime => {
                let minutes = value / 60_000;
                format!("{} h {} min", minutes / 60, minutes % 60)
            }
            _ => grouped(value),
        }
    }
}

/// `12345` as `12,345`.
pub fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub stats: [u64; Stat::COUNT],
    /// Bit `i` set when `advancements::table()[i]` is unlocked.
    pub unlocked: u64,
    /// Sub-unit remainders carried between steps: centimetres and milliseconds.
    carry: [f32; Stat::COUNT],
    last_pos: Option<Vec3>,
    was_dead: bool,
    /// Advancement indices waiting for a toast, oldest first.
    toasts: Vec<usize>,
    toast_left: f32,
}

/// UI snapshot published each frame.
#[derive(Clone, Debug, Default)]
pub struct View {
    pub stats: [u64; Stat::COUNT],
    pub unlocked: u64,
    pub level: u32,
    pub toast: Option<&'static str>,
    /// Time of day in `0..1` while a clock is held.
    pub clock: Option<f32>,
}

impl State {
    /// Counters and unlocks only, for UI progress queries.
    pub fn from_view(view: &View) -> Self {
        Self { stats: view.stats, unlocked: view.unlocked, ..Self::default() }
    }

    pub fn get(&self, stat: Stat) -> u64 {
        let v = self.stats[stat as usize];
        if stat == Stat::Places { u64::from(v.count_ones()) } else { v }
    }

    pub fn add(&mut self, stat: Stat, amount: u64) {
        let v = &mut self.stats[stat as usize];
        *v = v.saturating_add(amount);
    }

    /// Adds a fractional amount, keeping the remainder for the next call.
    fn add_fraction(&mut self, stat: Stat, amount: f32) {
        let carry = &mut self.carry[stat as usize];
        *carry += amount.max(0.0);
        let whole = carry.floor();
        *carry -= whole;
        self.add(stat, whole as u64);
    }

    pub fn is_unlocked(&self, index: usize) -> bool {
        index < 64 && self.unlocked & (1 << index) != 0
    }

    pub fn view(&self, level: u32) -> View {
        let toast = self.toasts.first().and_then(|&i| advancements::table().get(i)).map(|a| a.name.as_str());
        View { stats: self.stats, unlocked: self.unlocked, level, toast, clock: None }
    }

    /// Unlocks every advancement whose trigger is met; returns how many were new.
    pub fn check(&mut self, level: u32) -> usize {
        let mut new = 0;
        for (i, row) in advancements::table().iter().enumerate().take(64) {
            if self.is_unlocked(i) || !row.trigger.met(self, level) {
                continue;
            }
            self.unlocked |= 1 << i;
            if self.toasts.is_empty() {
                self.toast_left = TOAST_SECONDS;
            }
            self.toasts.push(i);
            new += 1;
        }
        new
    }

    fn tick_toast(&mut self, dt: f32) {
        if self.toasts.is_empty() {
            return;
        }
        self.toast_left -= dt;
        if self.toast_left <= 0.0 {
            self.toasts.remove(0);
            self.toast_left = TOAST_SECONDS;
        }
    }
}

/// Per fixed step, after progression: play time, distance, deaths, crafts and unlocks.
pub fn step(world: &mut World, player: &Player, dt: f32) {
    let metres = world.voxel_scale.meters();
    let crafted = std::mem::take(&mut world.inventory.crafted);
    let dead = world.adventure.health <= 0.0;
    let level = world.progression.level();
    let learned = super::progression::study::Lesson::ALL.into_iter()
        .filter(|lesson| lesson.learned(world.progression.lessons, world.progression.horticulture))
        .count() as u64;
    let s = &mut world.stats;
    // Existing saves already have permanent lessons, even before this counter existed.
    s.stats[Stat::Studies as usize] = learned;
    s.add_fraction(Stat::PlayTime, dt.max(0.0) * 1000.0);
    s.add(Stat::Crafted, u64::from(crafted));
    if dead && !s.was_dead {
        s.add(Stat::Deaths, 1);
    }
    s.was_dead = dead;
    let pos = player.pos * metres;
    if let Some(last) = s.last_pos.replace(pos) {
        let d = pos - last;
        let moved = if player.swimming || player.flying { d.length() } else { Vec3::new(d.x, 0.0, d.z).length() };
        let stat = if player.swimming {
            Some(Stat::Swum)
        } else if player.flying {
            Some(Stat::Flown)
        } else if player.on_ground {
            Some(Stat::Walked)
        } else {
            None
        };
        if let Some(stat) = stat.filter(|_| !dead && moved < MAX_STEP_METRES) {
            s.add_fraction(stat, moved * 100.0);
        }
    }
    // The location readout names the surroundings; each new name is a discovery.
    if let Some(bit) = world.adventure.location.reading()
        .and_then(|r| discoveries().iter().position(|name| name == r.place.name))
        .filter(|&i| i < 64)
    {
        s.stats[Stat::Places as usize] |= 1u64 << bit;
    }
    s.tick_toast(dt);
    if s.check(level) > 0 && world.adventure.sounds.len() < 32 {
        world.adventure.sounds.push(Event { cue: Cue::Treasure, pos: None, gain: 0.4, pitch: 1.5 });
    }
}

const DISCOVERIES_RON: &str = include_str!("../../../../assets/discoveries.ron");

/// Place names that count toward `Stat::Places`, in bit order. Append only:
/// a saved bit set refers to these positions.
static DISCOVERIES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
    ron::from_str(DISCOVERIES_RON).unwrap_or_else(|error| {
        eprintln!("[stats] assets/discoveries.ron: {error}");
        Vec::new()
    })
});

pub fn discoveries() -> &'static [String] {
    &DISCOVERIES
}
