//! Persistent, finite-input cultivation. A source owns one resident and its
//! independent product cycle. Combat is never approximated as production.
use super::{BlockId as B, World, adventure};
use crate::mesh::models::Model;
use glam::{IVec3, Quat, Vec3};
mod persistence;
mod automation;
pub(crate) use engine_worldgen::habitats_region as region;
pub(crate) mod review;

const MAX_SOURCES: usize = 64;
const MAX_RELEASES: usize = 256;
const CAP: u32 = 65_536;
const STEP: f64 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Family {
    Nursery,
    Spider,
    Matron,
}
impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Self::Nursery => "Root nursery",
            Self::Spider => "Cistern spider brood",
            Self::Matron => "Matron nursery",
        }
    }
    pub fn kind(self) -> u32 {
        match self {
            Self::Nursery => 1,
            Self::Spider => 0,
            Self::Matron => 15,
        }
    }
    fn growth(self) -> u32 {
        match self {
            Self::Nursery => 120,
            Self::Spider => 160,
            Self::Matron => 720,
        }
    }
    fn feed_cost(self) -> u32 {
        if self == Self::Matron { 256 } else { 64 }
    }
    fn mana_cost(self) -> u32 {
        if self == Self::Matron { 32 } else { 0 }
    }
    fn xp(self) -> u32 {
        if self == Self::Matron { 80 } else { 8 }
    }
    fn resource(self) -> usize {
        if self == Self::Matron { 13 } else { 4 }
    }
}

#[derive(Clone)]
pub struct Resident {
    pub pos: Vec3,
    pub health: f32,
    pub pods: u32,
    pub spores: u32,
    pub work: f64,
}
impl Resident {
    fn new(pos: Vec3, kind: u32) -> Self {
        Self {
            pos,
            health: adventure::habitat_health(kind),
            pods: if kind == 15 { 2 } else { 0 },
            spores: if kind == 15 { 12 } else { 0 },
            work: 0.0,
        }
    }
}
/// Nests and the mobs they spawn are drawn (and fade out) within this many metres.
pub const DRAW_M: f32 = 90.0;
pub const BEACON_RANGE: f32 = 6.0;
pub const MEND_MANA: u32 = 2;
pub const MEND_HEALTH: f32 = 4.0;
pub const HARVEST_MANA: u32 = 4;
pub const HARVEST_DAMAGE: f32 = 8.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub enum Beacon {
    #[default]
    Off,
    Mending,
    Harvest,
}

impl Beacon {
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Off => [0.55, 0.74, 0.82],
            Self::Mending => [0.34, 0.95, 0.65],
            Self::Harvest => [0.93, 0.41, 0.69],
        }
    }
}

#[derive(Clone)]
pub struct Source {
    pub id: u32,
    pub parent: u32,
    pub family: Family,
    /// None means the unique source is being carried; no production in a pack.
    pub pos: Option<Vec3>,
    pub resident: Option<Resident>,
    pub feed: u32,
    pub charge: u32,
    pub development: u32,
    pub product_clock: u32,
    pub products: u32,
    pub mana: u32,
    pub xp: u32,
    pub receiver: bool,
    pub automatic: bool,
    pub emitter: bool,
    pub beacon: Beacon,
    pub reserve: bool,
    pub emit_ticks: u32,
    pub births: u32,
    pub harvested: u32,
    pub feed_used: u32,
    pub mana_made: u32,
    pub mana_used: u32,
    pub xp_made: u32,
    pub deaths: u32,
    pub blocker: &'static str,
}
impl Source {
    fn breeding_reserve(&self) -> u32 {
        self.family.mana_cost()
    }

    pub fn usable_mana(&self) -> u32 {
        self.mana + self.charge.saturating_sub(self.breeding_reserve())
    }

    pub(super) fn spend_mana(&mut self, cost: u32) {
        debug_assert!(self.usable_mana() >= cost);
        let stored = self.mana.min(cost);
        self.mana -= stored;
        self.charge -= cost - stored;
        self.mana_used = self.mana_used.saturating_add(cost);
    }

    fn new(id: u32, family: Family, pos: Option<Vec3>) -> Self {
        Self {
            id,
            parent: 0,
            family,
            pos,
            resident: None,
            feed: 0,
            charge: 0,
            development: 0,
            product_clock: 0,
            products: 0,
            mana: 0,
            xp: 0,
            receiver: false,
            automatic: false,
            emitter: false,
            beacon: Beacon::Off,
            reserve: true,
            emit_ticks: 0,
            births: 0,
            harvested: 0,
            feed_used: 0,
            mana_made: 0,
            mana_used: 0,
            xp_made: 0,
            deaths: 0,
            blocker: "Inspecting local conditions",
        }
    }
}
#[derive(Clone)]
struct Release {
    pos: Vec3,
    mana: u32,
    xp: u32,
}
#[derive(Default)]
pub struct State {
    pub(crate) shots: Vec<FarmShot>,
    seeded: bool,
    next_id: u32,
    clock: f64,
    pub sources: Vec<Source>,
    releases: Vec<Release>,
    pub xp: u64,
    pub lantern: bool,
    pub xp_spent: u32,
    pub mastered: u32,
}
#[derive(Clone)]
pub(crate) struct FarmShot {
    pub owner: u32,
    pub pos: Vec3,
    pub velocity: Vec3,
    pub life: f32,
}
#[derive(Default, Clone)]
pub struct View {
    pub sources: Vec<(Source, f32)>,
    pub xp: u64,
    pub lantern: bool,
    pub lantern_near: bool,
    pub loose_xp: u64,
    pub mastered: u32,
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub enum Action {
    Feed(u32),
    Charge(u32),
    Collect(u32),
    Harvest(u32),
    Receiver(u32),
    Automatic(u32),
    Carry(u32),
    Place(u32),
    Propagate(u32),
    Destroy(u32),
    ClaimLantern,
    ImprintMoss,
    Emitter(u32),
    Reserve(u32),
    SetBeacon(u32, Beacon),
}

fn cell(world: &World, p: Vec3) -> IVec3 {
    (p / world.voxel_scale.meters()).floor().as_ivec3()
}
fn conditions(world: &World, source: &Source, p: Vec3) -> &'static str {
    if !world.solid(cell(world, p - Vec3::Y * 0.1)) {
        return "Supporting bed missing";
    }
    if world.get(cell(world, p + Vec3::Y * 0.2)) == B::Water {
        return "Source flooded; needs a dry opening";
    }
    if source.family != Family::Spider
        && !matches!(
            world.get(cell(world, p - Vec3::Y * 0.1)),
            B::FungalSoil | B::Dirt | B::Mud
        )
    {
        return "Needs a rooted soil bed";
    }
    if source.family == Family::Spider && !world.solid(cell(world, p + Vec3::Y * 2.1)) {
        return "Dry overhead anchor missing (2 m above bed)";
    }
    let wet = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
        .iter()
        .any(|d| world.get(cell(world, p + *d * 2.0 - Vec3::Y * 0.25)) == B::Water);
    if !wet {
        return "Needs water 2 m beside the bed";
    }
    if !adventure::habitat_space(world, p, source.family.kind()) {
        return "Emergence opening blocked";
    }
    "Ready"
}

pub fn view(world: &World, player: Vec3) -> View {
    let mut sources: Vec<_> = world
        .habitats
        .sources
        .iter()
        .map(|s| (s.clone(), s.pos.map_or(0.0, |p| p.distance(player))))
        .collect();
    sources.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.id.cmp(&b.0.id)));
    View {
        sources,
        xp: world.habitats.xp,
        lantern: world.habitats.lantern,
        lantern_near: world.generator_version >= 27
            && world.world_type != super::WorldType::Superflat
            && player.distance(region::lantern(&world.sample_config())) <= 7.0,
        loose_xp: world.habitats.releases.iter().map(|r| r.xp as u64).sum(),
        mastered: world.habitats.mastered,
    }
}

/// Called with the same simulation delta as adventure; accumulates fractional
/// ticks without frame-dependent rounding. Does not consult render residency.
pub(crate) fn tick(world: &mut World, player: Vec3, dt: f32) {
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }
    let mut state = std::mem::take(&mut world.habitats);
    if !state.seeded
        && world.generator_version >= 27
        && world.world_type != super::WorldType::Superflat
    {
        state.seeded = true;
        for (i, family) in [Family::Nursery, Family::Spider, Family::Matron]
            .into_iter()
            .enumerate()
        {
            let mut s = Source::new(i as u32 + 1, family, Some(region::sources(&world.sample_config())[i]));
            s.feed = family.feed_cost() * 2;
            // A finite, visible starting charge; later Matrons need supplied mana.
            s.charge = family.mana_cost();
            state.sources.push(s);
        }
        state.next_id = 4;
        if let Some(p) = engine_worldgen::fernwake::nursery(&world.sample_config()) {
            let mut source = Source::new(4, Family::Nursery, Some(p));
            source.feed = Family::Nursery.feed_cost() * 2;
            state.sources.push(source);
            state.next_id = 5;
        }
    }
    state.clock += dt as f64;
    while state.clock >= STEP {
        state.clock -= STEP;
        for source in &mut state.sources {
            source.emit_ticks = source.emit_ticks.saturating_sub(1);
            let Some(p) = source.pos else {
                source.blocker = "Carried; development paused";
                continue;
            };
            // Ports use finite stock; separate crates may supply and receive.
            if source.receiver {
                automation::supply_mana(world,source,p);
                let scale = world.voxel_scale.meters();
                let half = super::factory::step(world) as f32 * 0.5;
                let mut ports: Vec<_> = world
                    .factory
                    .nodes
                    .iter()
                    .filter(|(_, n)| n.kind == super::factory::Kind::Storage && n.enabled)
                    .map(|(q, _)| {
                        (
                            *q,
                            ((q.as_vec3() + Vec3::splat(half)) * scale).distance_squared(p),
                        )
                    })
                    .filter(|(_, d)| *d <= 2.5 * 2.5)
                    .collect();
                ports.sort_by(|(a,da),(b,db)|da.total_cmp(db).then((a.x,a.y,a.z).cmp(&(b.x,b.y,b.z))));
                for (q,_) in ports {
                    world.factory.habitat_exchange(
                        q,
                        source.family.resource(),
                        &mut source.feed,
                        &mut source.harvested,
                        source.family.feed_cost()*2,
                    );
                }
            }
            // Already-produced goods can be harvested even when new production
            // is starved, flooded or blocked. Only the new work needs inputs.
            if source.automatic
                && source.receiver
                && source.products >= 64
                && source.xp <= CAP - 2
                && source.harvested <= CAP - 64
            {
                harvest(source);
                world.factory.stats.made(source.family.resource(), 64);
            }
            source.blocker = conditions(world, source, p);
            if source.blocker != "Ready" {
                continue;
            }
            if source.resident.is_none() {
                if source.automatic {
                    let moved = source
                        .family
                        .mana_cost()
                        .saturating_sub(source.charge)
                        .min(source.mana);
                    source.charge += moved;
                    source.mana -= moved;
                }
                if source.feed < source.family.feed_cost() {
                    source.blocker = "Needs fungal biomass";
                    continue;
                }
                if source.charge < source.family.mana_cost() {
                    source.blocker = "Needs 32 supplied mana";
                    continue;
                }
                source.development += 1;
                source.blocker = "Developing; inputs reserved until emergence";
                if source.development >= source.family.growth() {
                    let feed = source.family.feed_cost();
                    source.feed -= feed;
                    source.feed_used = source.feed_used.saturating_add(feed);
                    world.factory.stats.used(13, feed);
                    source.charge -= source.family.mana_cost();
                    source.mana_used = source.mana_used.saturating_add(source.family.mana_cost());
                    source.development = 0;
                    source.product_clock = 0;
                    source.resident = Some(Resident::new(p, source.family.kind()));
                    source.births = source.births.saturating_add(1);
                }
            } else {
                if source.products > CAP - 64
                    || source.mana > CAP - 8
                    || source.harvested > CAP - 64
                {
                    source.blocker = "Output full";
                    continue;
                }
                if source.feed < 64 {
                    source.blocker = "Resident alive; needs biomass for products";
                    continue;
                }
                // Production belongs to a resident at its working bed, not a
                // second invisible creature while the real one escapes or fights.
                if source
                    .resident
                    .as_ref()
                    .is_some_and(|r| r.pos.distance(p) > 3.0)
                {
                    source.blocker = "Resident away from working bed";
                    continue;
                }
                source.product_clock += 1;
                source.blocker = "Producing; 1 biomass per 20 seconds";
                if source.product_clock >= 80 {
                    source.product_clock = 0;
                    source.feed -= 64;
                    source.feed_used = source.feed_used.saturating_add(64);
                    world.factory.stats.used(13, 64);
                    source.products += 64;
                    // Nutrient-fed magical growth; fictional mana, no electrical units.
                    if source.family != Family::Spider {
                        source.mana += 8;
                        source.mana_made = source.mana_made.saturating_add(8);
                    }
                }
            }
        }
        // Deterministic nearest receiver, one owner and finite capacity. Loose
        // resources remain where released when storage is blocked.
        for release in &mut state.releases {
            let receiver = state
                .sources
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.receiver && ((release.mana>0 && s.mana<CAP) || (release.xp>0 && s.xp<CAP))
                        && s.pos.is_some_and(|p| p.distance(release.pos) <= 6.0)
                })
                .min_by(|(_, a), (_, b)| {
                    a.pos
                        .unwrap()
                        .distance_squared(release.pos)
                        .total_cmp(&b.pos.unwrap().distance_squared(release.pos))
                        .then(a.id.cmp(&b.id))
                })
                .map(|(i, _)| i);
            if let Some(i) = receiver {
                let s = &mut state.sources[i];
                let mana = release.mana.min(CAP - s.mana);
                let xp = release.xp.min(CAP - s.xp);
                s.mana += mana;
                s.xp += xp;
                release.mana -= mana;
                release.xp -= xp;
            }
            if state.lantern && release.pos.distance(player) <= 6.0 {
                state.xp += release.xp as u64;
                release.xp = 0;
                let mana = (world.adventure.max_mana() - world.adventure.mana)
                    .max(0.0)
                    .floor() as u32;
                let mana = release.mana.min(mana);
                world.adventure.mana += mana as f32;
                release.mana -= mana;
            }
        }
        state.releases.retain(|r| r.mana > 0 || r.xp > 0);
    }
    world.habitats = state;
}

fn harvest(source: &mut Source) {
    source.products -= 64;
    // A new product event is independent of the body's eventual death reward.
    source.xp += 2;
    source.xp_made = source.xp_made.saturating_add(2);
    source.harvested = source.harvested.saturating_add(64);
}

impl State {
    /// Sends a flower's pulse to the nearest installed receiver with room.
    pub(super) fn store_flower_mana(&mut self, pos: Vec3, radius: f32, amount: u32) -> u32 {
        let Some(receiver) = self.sources.iter_mut()
            .filter(|s| s.receiver && s.mana < CAP)
            .filter_map(|s| Some((s.pos?.distance_squared(pos), s)))
            .filter(|(distance, _)| *distance <= radius * radius)
            .min_by(|(da, a), (db, b)| da.total_cmp(db).then(a.id.cmp(&b.id)))
            .map(|(_, s)| s)
        else {
            return 0;
        };
        let stored = amount.min(CAP - receiver.mana);
        receiver.mana += stored;
        receiver.mana_made = receiver.mana_made.saturating_add(stored);
        stored
    }

    /// Takes `amount` from the nearest receiver within `radius` holding that much; all or nothing.
    pub(super) fn draw_flower_mana(&mut self, pos: Vec3, radius: f32, amount: u32) -> bool {
        let Some(receiver) = self.sources.iter_mut()
            .filter(|s| s.receiver && s.mana >= amount)
            .filter_map(|s| Some((s.pos?.distance_squared(pos), s)))
            .filter(|(distance, _)| *distance <= radius * radius)
            .min_by(|(da, a), (db, b)| da.total_cmp(db).then(a.id.cmp(&b.id)))
            .map(|(_, s)| s)
        else {
            return false;
        };
        receiver.mana -= amount;
        receiver.mana_used = receiver.mana_used.saturating_add(amount);
        true
    }

    pub(crate) fn release(&mut self, pos: Vec3, mana: u32, xp: u32) {
        // At most one packet per source/site location; coalescing never discards rewards.
        if let Some(r) = self.releases.iter_mut().find(|r| r.pos == pos) {
            let room = u32::MAX - r.xp;
            let moved = xp.min(room);
            r.xp += moved;
            self.xp += (xp - moved) as u64;
            r.mana = r.mana.saturating_add(mana);
        } else if self.releases.len() >= MAX_RELEASES {
            // Bounded loose geometry must never suppress automated XP. The
            // shared ledger receives overflow without requiring player presence.
            self.xp += xp as u64;
        } else {
            self.releases.push(Release { pos, mana, xp });
        }
    }
    pub(crate) fn death(&mut self, id: u32, pos: Vec3) -> Option<(usize, u32)> {
        if let Some(s) = self.sources.iter_mut().find(|s| s.id == id) {
            let resident = s.resident.take()?;
            let xp = s.family.xp();
            let resource = if s.family == Family::Matron { 13 } else { 18 };
            s.xp_made = s.xp_made.saturating_add(xp);
            s.deaths = s.deaths.saturating_add(1);
            if s.family == Family::Matron && s.deaths >= 3 && s.receiver && !s.reserve
                && (s.emitter || s.beacon == Beacon::Harvest)
            {
                self.mastered |= 1 << s.family as u32;
            }
            s.product_clock = 0;
            // Body essence is distinct from the earlier product event.
            self.release(pos, 0, xp);
            return Some((resource, 64 + resident.pods * 64));
        }
        None
    }

    pub fn models(&self, model: &mut Model, viewer: Vec3, cfg: &super::WorldSampleConfig) {
        let previous_material = model.set_surface_kind(58.0);
        for s in &self.sources {
            let Some(p) = s.pos else { continue };
            let coverage = crate::mesh::models::range_fade(p.distance(viewer), DRAW_M);
            if coverage <= 0.0 {
                continue;
            }
            let color = match s.family {
                Family::Nursery => [0.42, 0.58, 0.32],
                Family::Spider => [0.74, 0.82, 0.86],
                Family::Matron => [0.52, 0.36, 0.52],
            };
            let mark = model.mark();
            let size = if s.family == Family::Matron {
                0.95
            } else {
                0.35
            };
            let growth = s.development as f32 / s.family.growth() as f32;
            model.cuboid(
                p + Vec3::Y * size * 0.5,
                Vec3::new(size, size * (0.5 + growth * 0.5), size),
                Quat::IDENTITY,
                color,
                0.0,
            );
            for x in [-1.0, 1.0] {
                model.beam(
                    p + Vec3::new(x * size, 0.05, 0.0),
                    p + Vec3::Y * size,
                    0.05,
                    color,
                    0.0,
                );
            }
            if s.family == Family::Spider {
                model.beam(p + Vec3::Y * 0.4, p + Vec3::Y * 2.0, 0.05, color, 0.0);
                model.beam(p + Vec3::Y * 0.2, p + Vec3::X * 2.0, 0.05, color, 0.0);
            }
            if s.receiver {
                model.material_box(
                    p + Vec3::new(-0.7, 0.3, 0.0),
                    Vec3::splat(0.35),
                    Quat::IDENTITY,
                    s.beacon.color(),
                    crate::mesh::models::AUTHORED_SURFACE,
                    if s.beacon != Beacon::Off && s.usable_mana() > 0 { 0.65 } else { 0.0 },
                );
            }
            model.bake_model_uv_since(mark, p, 58.0);
            model.fade_since(mark, coverage);
        }
        for r in self
            .releases
            .iter()
            .filter(|r| r.pos.distance(viewer) < 64.0)
        {
            model.cuboid(
                r.pos + Vec3::Y * 0.2,
                Vec3::splat(0.1),
                Quat::IDENTITY,
                [0.9, 0.72, 0.42],
                0.3,
            );
        }
        let lantern = region::lantern(cfg);
        if self.seeded && !self.lantern && viewer.distance(lantern) < 90.0 {
            model.cuboid(
                lantern + Vec3::Y * 1.3,
                Vec3::new(0.3, 0.5, 0.3),
                Quat::IDENTITY,
                [0.9, 0.72, 0.42],
                0.3,
            );
        }
        model.set_surface_kind(previous_material);
    }
}

pub fn action(world: &mut World, player: Vec3, action: Action) {
    if matches!(action, Action::ClaimLantern) {
        if view(world, player).lantern_near && !world.habitats.lantern {
            world.habitats.lantern = true;
            world.expedition.say("Witness Lantern recovered. Collects loose XP and mana within 6 m. Habitat receivers work while you are away.");
        }
        return;
    }
    if matches!(action, Action::ImprintMoss) {
        if world.habitats.xp >= 64 && world.adventure.tool_mods & 8 == 0 {
            world.habitats.xp -= 64;
            world.habitats.xp_spent = world.habitats.xp_spent.saturating_add(64);
            world.adventure.tool_mods |= 8;
            world.expedition.say(
                "Moss binding imprinted: melee hits restore health. Spell knowledge is unchanged.",
            );
        }
        return;
    }
    let id = match action {
        Action::Feed(id)
        | Action::Charge(id)
        | Action::Collect(id)
        | Action::Harvest(id)
        | Action::Receiver(id)
        | Action::Automatic(id)
        | Action::Carry(id)
        | Action::Place(id)
        | Action::Propagate(id)
        | Action::Destroy(id) => id,
        Action::Emitter(id) | Action::Reserve(id) | Action::SetBeacon(id, _) => id,
        _ => return,
    };
    let Some(index) = world.habitats.sources.iter().position(|s| s.id == id) else {
        return;
    };
    if world.habitats.sources[index]
        .pos
        .is_some_and(|p| p.distance(player) > 7.0)
    {
        world
            .expedition
            .say("Move within 7 m of the habitat source.");
        return;
    }
    let carried = world.habitats.sources.iter().any(|s| s.pos.is_none());
    let scale = world.voxel_scale.meters();
    let placement = Vec3::new(
        player.x.floor() + 0.5,
        (player.y / scale).floor() * scale,
        player.z.floor() + 0.5,
    );
    let occupied = world
        .habitats
        .sources
        .iter()
        .any(|s| s.id != id && s.pos.is_some_and(|p| p.distance(placement) < 1.0));
    let can_split = world.habitats.sources.len() < MAX_SOURCES && !carried;
    let s = &mut world.habitats.sources[index];
    match action {
        Action::Feed(_) => {
            let n = 256.min(CAP - s.feed).min(world.expedition.resources[13]);
            world.expedition.resources[13] -= n;
            s.feed += n;
        }
        Action::Charge(_) => {
            let n = 32
                .min(CAP - s.charge)
                .min(world.adventure.mana.max(0.0).floor() as u32);
            world.adventure.mana -= n as f32;
            s.charge += n;
        }
        Action::Collect(_) => {
            let n = (world.adventure.max_mana() - world.adventure.mana)
                .max(0.0)
                .floor() as u32;
            let n = n.min(s.mana);
            s.mana -= n;
            world.adventure.mana += n as f32;
            world.habitats.xp += s.xp as u64;
            s.xp = 0;
            let room =
                64_000_000u32.saturating_sub(world.expedition.resources[s.family.resource()]);
            let n = s.harvested.min(room);
            world.expedition.resources[s.family.resource()] += n;
            s.harvested -= n;
        }
        Action::Harvest(_) if s.products >= 64 && s.xp <= CAP - 2 && s.harvested <= CAP - 64 => {
            harvest(s);
            world.factory.stats.made(s.family.resource(), 64);
        }
        Action::Receiver(_)
            if s.receiver
                && !s.automatic
                && !s.emitter
                && s.beacon == Beacon::Off
                && s.mana == 0
                && s.xp == 0
                && world.expedition.resources[12] <= 64_000_000 - 64
                && world.expedition.resources[5] <= 64_000_000 - 64 =>
        {
            s.receiver = false;
            world.expedition.add(12, 64);
            world.expedition.add(5, 64);
        }
        Action::Receiver(_) if !s.receiver && world.expedition.pay(&[(12, 64), (5, 64)]) => {
            s.receiver = true
        }
        Action::Automatic(_) if s.receiver => s.automatic = !s.automatic,
        Action::Emitter(_) if s.receiver => {
            s.emitter = !s.emitter;
            if s.emitter {
                s.beacon = Beacon::Off;
            }
        }
        Action::SetBeacon(_, mode) if s.receiver && s.beacon != mode => {
            s.beacon = mode;
            if mode != Beacon::Off {
                s.emitter = false;
            }
        }
        Action::Reserve(_) => s.reserve = !s.reserve,
        Action::Carry(_) if !carried && s.resident.is_none() => {
            s.pos = None;
            s.parent = 0;
        }
        Action::Place(_) if s.pos.is_none() && !occupied => {
            // Place at the player's feet, one source per metre cell.
            s.pos = Some(placement);
        }
        Action::Propagate(_)
            if can_split
                && s.products >= 64
                && s.resident.is_some()
                && world.habitats.next_id < u32::MAX
                && world.expedition.pay(&[(13, 256)]) =>
        {
            s.products -= 64;
            let family = s.family;
            let id = world.habitats.next_id;
            world.habitats.next_id += 1;
            world.habitats.sources.push(Source::new(id, family, None));
        }
        Action::Destroy(_)
            if s.resident.is_none()
                && s.feed == 0
                && s.charge == 0
                && s.products == 0
                && s.mana == 0
                && s.xp == 0
                && s.harvested == 0
                && !s.receiver =>
        {
            world.habitats.sources.remove(index);
        }
        _ => world.expedition.say(
            "Action unavailable: inspect the source's conditions, buffers and resident first.",
        ),
    }
}

pub(crate) fn brood_target(world: &World, owner: u32, home: Vec3) -> Option<Vec3> {
    if world.habitats.sources.len() >= MAX_SOURCES
        || world
            .habitats
            .sources
            .iter()
            .filter(|s| s.parent == owner)
            .count()
            >= 4
    {
        return None;
    }
    for d in [
        Vec3::X * 4.0,
        Vec3::NEG_X * 4.0,
        Vec3::Z * 4.0,
        Vec3::NEG_Z * 4.0,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(-2.0, 0.0, -2.0),
    ] {
        let p = home + d;
        if world
            .habitats
            .sources
            .iter()
            .any(|s| s.pos.is_some_and(|q| q.distance(p) < 1.0))
        {
            continue;
        }
        let probe = Source::new(1, Family::Nursery, Some(p));
        if conditions(world, &probe, p) == "Ready" {
            return Some(p);
        }
    }
    None
}
pub(crate) fn plant_brood(world: &mut World, owner: u32, p: Vec3) -> bool {
    if world.habitats.sources.len() >= MAX_SOURCES
        || world.habitats.next_id == u32::MAX
        || world
            .habitats
            .sources
            .iter()
            .filter(|s| s.parent == owner)
            .count()
            >= 4
        || world
            .habitats
            .sources
            .iter()
            .any(|s| s.pos.is_some_and(|q| q.distance(p) < 1.0))
    {
        return false;
    }
    let mut pod = Source::new(world.habitats.next_id, Family::Nursery, Some(p));
    // No growth recipe bypass: a landed pod still needs its real habitat.
    if !world.solid(cell(world, p - Vec3::Y * 0.1)) || !adventure::habitat_space(world, p, 1) {
        return false;
    }
    pod.feed = 64;
    pod.parent = owner;
    world.habitats.next_id += 1;
    world.habitats.sources.push(pod);
    true
}
