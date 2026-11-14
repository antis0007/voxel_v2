//! Volume and gold accounting for the quarry → sluice → furnace line.
use super::{CAPACITY, DIRS, GOLD_CONCENTRATE, Kind, Node, blocks::Material, industry, sides};
use crate::world::World;
use glam::IVec3;

pub const VOLUME_PER_M3: u32 = 64;
pub const PAYDIRT_GRADE: u32 = 500;
pub const SLUICE_MODE: u32 = 1;
pub const FEED_CAPACITY: u32 = VOLUME_PER_M3 * 8;
pub const SLUICE_TICKS: u32 = 32;
pub const SLUICE_POWER: u32 = 12;
const WATER_PER_VOLUME: u32 = 2;

/// Volume is in 1/64 m³. Each gold quantum is four concentrate units before recovery.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Feed {
    pub volume: u32,
    pub gold: u32,
}

impl Feed {
    pub fn from_grade(grade: u32, volume: u32) -> Self {
        Self {
            volume,
            gold: (grade / 128) * volume,
        }
    }

    pub fn grade(self) -> u32 {
        (self.gold * 128).checked_div(self.volume).unwrap_or(0)
    }

    pub fn pan_recovery(self) -> u32 {
        self.gold * 2
    }

    pub fn sluice_recovery(self) -> u32 {
        self.gold * 3
    }

    pub fn fits(self, incoming: Self) -> bool {
        self.volume + incoming.volume <= FEED_CAPACITY
    }

    pub fn add(&mut self, other: Self) {
        self.volume += other.volume;
        self.gold += other.gold;
    }

    fn take(&mut self, limit: u32) -> Self {
        let volume = self.volume.min(limit);
        let gold = (self.gold * volume).checked_div(self.volume).unwrap_or(0);
        self.volume -= volume;
        self.gold -= gold;
        Self { volume, gold }
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Buffer {
    pub enabled: bool,
    pub feed: Feed,
    pub batch: Feed,
}

impl Buffer {
    pub fn has_feed(self) -> bool {
        self.feed.volume > 0 || self.batch.volume > 0
    }

    pub(super) fn cancel_batch(&mut self) {
        self.feed.add(std::mem::take(&mut self.batch));
    }
}

pub fn cell_volume(size: i32) -> u32 {
    VOLUME_PER_M3 / (size * size * size) as u32
}

/// An explicit flush recovers the physical fill and abandons its gold budget.
pub(super) fn flush(n: &mut Node, size: i32) -> bool {
    if !n.placer.has_feed() {
        return true;
    }
    let sand = &mut n.blocks.output[Material::Sand.index()];
    let blocks = (n.placer.feed.volume + n.placer.batch.volume) / cell_volume(size);
    if *sand > CAPACITY - blocks {
        return false;
    }
    *sand += blocks;
    n.placer.feed = Feed::default();
    n.placer.batch = Feed::default();
    if sluice(n) {
        n.progress = 0;
    }
    true
}

pub fn sluice(n: &Node) -> bool {
    n.kind == Kind::Washer && n.mode == SLUICE_MODE
}

pub fn sluice_ticks(batch: Feed) -> u32 {
    (SLUICE_TICKS * batch.volume).div_ceil(VOLUME_PER_M3).max(1)
}

pub(super) fn valid_progress(n: &Node) -> bool {
    sluice(n) && n.placer.batch.volume > 0 && n.progress < sluice_ticks(n.placer.batch)
}

pub(super) fn process(n: &mut Node, size: i32) -> Option<u32> {
    if !sluice(n) {
        return None;
    }
    let volume = cell_volume(size);
    if n.placer.batch.volume == 0 {
        let paydirt = &mut n.blocks.input[Material::Paydirt.index()];
        let needed = VOLUME_PER_M3.saturating_sub(n.placer.feed.volume) / volume;
        let moved = (*paydirt).min(needed);
        *paydirt -= moved;
        n.placer.feed.add(Feed::from_grade(PAYDIRT_GRADE, moved * volume));
        n.placer.batch = n.placer.feed.take(VOLUME_PER_M3);
    }
    let batch = n.placer.batch;
    if batch.volume == 0 {
        n.status = "Needs input";
        return Some(0);
    }
    let sand = batch.volume / volume;
    let water = batch.volume * WATER_PER_VOLUME;
    let gold = batch.sluice_recovery();
    if n.blocks.output[Material::Sand.index()] > CAPACITY - sand
        || n.output[GOLD_CONCENTRATE] > CAPACITY - gold
    {
        n.status = "Output blocked";
        return Some(0);
    }
    if n.water < water {
        n.status = "Needs water";
        return Some(0);
    }
    let ticks = sluice_ticks(batch);
    let steps = (1 + n.upgrade).min(ticks - n.progress);
    let cost = steps * SLUICE_POWER;
    if n.energy < cost {
        n.status = "Needs cable / power";
        return Some(0);
    }
    n.energy -= cost;
    n.progress += steps;
    industry::produce(n, steps);
    n.status = "Processing";
    if n.progress == ticks {
        n.water -= water;
        n.output[GOLD_CONCENTRATE] += gold;
        n.blocks.output[Material::Sand.index()] += sand;
        n.placer.batch = Feed::default();
        n.progress = 0;
    }
    Some(cost)
}

/// Direct neighbours use the same side roles as the item bus. Concentrate also
/// remains available to ordinary pipes, storage and manual transfer.
pub(super) fn route(world: &mut World, p: IVec3, n: &mut Node, size: i32) {
    for (face, dir) in DIRS.into_iter().enumerate() {
        if !matches!(sides::role(n, face), sides::Role::Both | sides::Role::Out) {
            continue;
        }
        let q = p + dir * size;
        let incoming = world.factory.input_reserved(q, GOLD_CONCENTRATE);
        let Some(dest) = world.factory.nodes.get_mut(&q) else {
            continue;
        };
        if !matches!(
            sides::role(dest, face ^ 1),
            sides::Role::Both | sides::Role::In
        ) {
            continue;
        }
        if n.kind == Kind::Quarry && sluice(dest) {
            let occupied = dest.placer.feed.volume + dest.placer.batch.volume;
            dest.placer
                .feed
                .add(n.placer.feed.take(FEED_CAPACITY - occupied));
        } else if n.kind == Kind::Washer
            && matches!(dest.kind, Kind::Furnace | Kind::SmelteryController)
            && dest.accepts(GOLD_CONCENTRATE)
        {
            let room = CAPACITY.saturating_sub(dest.input[GOLD_CONCENTRATE] + incoming);
            let moved = n.output[GOLD_CONCENTRATE].min(room);
            n.output[GOLD_CONCENTRATE] -= moved;
            dest.input[GOLD_CONCENTRATE] += moved;
        }
    }
}

pub(super) fn encode(n: &Node, put: &mut impl FnMut(u32)) {
    if matches!(n.kind, Kind::Quarry | Kind::Washer) {
        put(u32::from(n.placer.enabled));
        for stock in [n.placer.feed, n.placer.batch] {
            put(stock.volume);
            put(stock.gold);
        }
    }
}

pub(super) fn decode(
    n: &mut Node,
    size: i32,
    read: &mut impl FnMut() -> anyhow::Result<u32>,
) -> anyhow::Result<()> {
    if !matches!(n.kind, Kind::Quarry | Kind::Washer) {
        return Ok(());
    }
    let enabled = read()?;
    anyhow::ensure!(
        enabled == 0 || (enabled == 1 && n.kind == Kind::Quarry),
        "Invalid placer mode"
    );
    n.placer.enabled = enabled == 1;
    for stock in [&mut n.placer.feed, &mut n.placer.batch] {
        stock.volume = read()?;
        stock.gold = read()?;
        anyhow::ensure!(
            stock.volume <= FEED_CAPACITY
                && stock.volume.is_multiple_of(cell_volume(size))
                && stock.gold <= stock.volume * 7,
            "Invalid placer stock"
        );
    }
    anyhow::ensure!(
        n.placer.feed.volume + n.placer.batch.volume <= FEED_CAPACITY
            && n.placer.batch.volume <= VOLUME_PER_M3
            && (n.placer.batch.volume == 0 || sluice(n)),
        "Invalid placer batch"
    );
    Ok(())
}
