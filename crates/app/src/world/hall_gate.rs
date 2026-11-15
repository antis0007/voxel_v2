//! Charted crossing between the Hall, its parent realm, the Nether and the Manyfold.
use super::{World, WorldType, storage::{self, SaveService}};
use crate::player::Player;
use anyhow::{Context, Result, ensure};
use engine_worldgen::hall::{self, Chart};
use glam::{DVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

const FILE: &str = "hall-chart.ron";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    parent: String,
    hall: String,
    nether: String,
    manyfold: String,
    seed: u32,
    generator: u32,
}

fn id_ok(s: &str) -> bool {
    s.len() <= 96 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl Link {
    fn slot(&self, kind: WorldType) -> &str {
        match kind {
            WorldType::Hall => &self.hall,
            WorldType::Nether => &self.nether,
            WorldType::Manyfold => &self.manyfold,
            _ => &self.parent,
        }
    }
    fn slot_mut(&mut self, kind: WorldType) -> &mut String {
        match kind {
            WorldType::Hall => &mut self.hall,
            WorldType::Nether => &mut self.nether,
            WorldType::Manyfold => &mut self.manyfold,
            _ => &mut self.parent,
        }
    }
    fn validate(&self, w: &World) -> Result<()> {
        ensure!(self.seed == w.seed && self.generator == w.generator_version, "Chart world mismatch");
        let here = self.slot(w.world_type);
        ensure!(id_ok(here) && !here.is_empty() && here == w.regions.view.session_name(), "Chart realm mismatch");
        Ok(())
    }
    fn publish(&self, root: &Path) -> Result<()> {
        let path = root.join(FILE);
        if path.exists() {
            ensure!(fs::metadata(&path)?.len() <= 4096, "Chart link is oversized");
            let old: Link = ron::from_str(&fs::read_to_string(&path)?)?;
            ensure!(old.seed == self.seed && old.generator == self.generator, "Chart link changed");
            for (was, now) in [(&old.parent, &self.parent), (&old.hall, &self.hall), (&old.nether, &self.nether), (&old.manyfold, &self.manyfold)] {
                ensure!(was.is_empty() || was == now, "Chart link changed");
            }
            if &old == self { return Ok(()); }
        }
        fs::create_dir_all(root)?;
        let tmp = root.join(format!("hall-chart-{}-{}.tmp", std::process::id(), super::regions::checkpoint_id()));
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(ron::to_string(self)?.as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(tmp, path)?;
        Ok(())
    }
}

/// The link file a crossing publishes beside both worlds.
enum Chartlink {
    Hall(Link),
    Shaft(super::lightless_gate::Link, engine_voxel::RealmKind),
}

impl Chartlink {
    fn publish(&self, root: &Path) -> Result<()> {
        match self {
            Chartlink::Hall(link) => link.publish(root),
            Chartlink::Shaft(link, kind) => link.publish(root, *kind),
        }
    }
}

struct Destination {
    world: World,
    link: Chartlink,
    saves: SaveService,
    previous: Vec3,
    previous_precise: DVec3,
}

#[derive(Default)]
pub struct Travel { requested: bool, destination: Option<Destination> }

fn charts_open(w: &World) -> bool {
    let down = |site| w.adventure.records.get(&site).is_some_and(|r| r & 1 != 0);
    down(hall::FURNACE_SITE) && down(hall::ASTRONOMER_SITE)
}

fn seal(w: &World) -> &'static str {
    let down = |site| w.adventure.records.get(&site).is_some_and(|r| r & 1 != 0);
    match u8::from(down(hall::FURNACE_SITE)) + u8::from(down(hall::ASTRONOMER_SITE)) {
        0 => "Wardens 0/2",
        1 => "Wardens 1/2",
        _ => "F  Cross",
    }
}

pub fn standing(w: &World, player: &Player) -> Option<Chart> {
    hall::standing(&w.sample_config(), player.pos * w.voxel_scale.meters())
}

pub fn usable(w: &World, player: &Player) -> bool {
    if super::lightless_gate::standing(w, player).is_some() {
        return super::lightless_gate::usable(w, player);
    }
    match standing(w, player) {
        Some(Chart::Nether | Chart::Manyfold) => charts_open(w) && w.adventure.health > 0.0,
        Some(_) => w.adventure.health > 0.0,
        None => false,
    }
}

pub fn prompt(w: &World, player: &Player, network: bool) -> Option<super::interaction::Prompt> {
    if let Some(prompt) = super::lightless_gate::prompt(w, player, network) {
        return Some(prompt);
    }
    let chart = standing(w, player)?;
    let open = !matches!(chart, Chart::Nether | Chart::Manyfold) || charts_open(w);
    let action = if network { "Realm crossing requires local play" }
        else if !open { seal(w) }
        else { match chart { Chart::Hall => "F  Enter", Chart::Hills => "F  Return", Chart::Nether => "F  Nether", Chart::Manyfold => "F  Manyfold" } };
    let title = match chart {
        Chart::Hall => "Hall of the Crimson King",
        Chart::Hills => "Hills",
        Chart::Nether => "Nether",
        Chart::Manyfold => "Manyfold",
    };
    Some(super::interaction::Prompt { title, action })
}

fn exchange_actor(a: &mut World, b: &mut World) {
    a.adventure.swap_realm_shared(&mut b.adventure);
    std::mem::swap(&mut a.adventure, &mut b.adventure);
    std::mem::swap(&mut a.inventory.dropped, &mut b.inventory.dropped);
    std::mem::swap(&mut a.inventory, &mut b.inventory);
    std::mem::swap(&mut a.expedition, &mut b.expedition);
    std::mem::swap(&mut a.progression, &mut b.progression);
    a.inventory.container = None;
    b.inventory.container = None;
    a.factory.focus = None;
    b.factory.focus = None;
    a.factory.bench = None;
    b.factory.bench = None;
}

fn fresh(kind: WorldType, source: &World) -> World {
    let mut target = World::with_seed(kind, source.voxel_scale, source.seed);
    target.generator_version = source.generator_version;
    target.set_rules(source.rules);
    target
}

impl Travel {
    pub fn busy(&self) -> bool { self.requested || self.destination.is_some() }
    pub fn saving_destination(&self) -> bool { self.destination.is_some() }
    pub fn request(&mut self, saves: &mut SaveService) {
        if !self.busy() { self.requested = true; saves.request(true); }
    }

    pub fn poll(&mut self, source: &mut World, player: &mut Player, day: &super::DayNightCycle, saves: &SaveService,
        prepare: impl FnOnce(&mut World, &Player, Vec3) -> Result<()>) -> Result<Option<World>> {
        if let Some(mut dest) = self.destination.take() {
            dest.saves.tick(Some(&mut dest.world), player, day);
            if dest.saves.failed {
                exchange_actor(source, &mut dest.world);
                player.pos = dest.previous;
                player.precise_pos = dest.previous_precise;
                player.vel = Vec3::ZERO;
                anyhow::bail!("Destination checkpoint failed; chart crossing cancelled");
            }
            if dest.saves.busy() { self.destination = Some(dest); return Ok(None); }
            if let Err(e) = dest.link.publish(&dest.world.regions.view.root).and_then(|_| dest.link.publish(&source.regions.view.root)) {
                exchange_actor(source, &mut dest.world);
                player.pos = dest.previous;
                player.precise_pos = dest.previous_precise;
                return Err(e);
            }
            return Ok(Some(dest.world));
        }
        if !self.requested || saves.busy() { return Ok(None); }
        self.requested = false;
        ensure!(!saves.failed, "Source checkpoint failed; chart crossing cancelled");
        if let Some(shaft) = super::lightless_gate::standing(source, player) {
            return self.begin_shaft(source, player, shaft, prepare);
        }
        let chart = standing(source, player).context("Outside a charted arch")?;
        if matches!(chart, Chart::Nether | Chart::Manyfold) { ensure!(charts_open(source), "Chart sealed"); }
        let previous = player.pos;
        let previous_precise = player.precise_pos;
        let arrival = hall::arrival_voxels(&source.sample_config(), chart);
        player.pos = arrival;
        player.precise_pos = arrival.as_dvec3();
        player.vel = Vec3::ZERO;
        let built = (|| -> Result<(World, Link)> {
            let path = source.regions.view.root.join(FILE);
            let mut link = if path.exists() {
                ensure!(fs::metadata(&path)?.len() <= 4096, "Chart link is oversized");
                let link: Link = ron::from_str(&fs::read_to_string(&path)?)?;
                link.validate(source)?;
                link
            } else {
                Link { parent: String::new(), hall: String::new(), nether: String::new(), manyfold: String::new(), seed: source.seed, generator: source.generator_version }
            };
            {
                let name = source.regions.view.session_name().to_owned();
                let here = link.slot_mut(source.world_type);
                if here.is_empty() { *here = name; }
            }
            let kind = match chart { Chart::Hall => WorldType::Hall, Chart::Hills => WorldType::Hills, Chart::Nether => WorldType::Nether, Chart::Manyfold => WorldType::Manyfold };
            let named = link.slot(kind).to_owned();
            let mut target = if named.is_empty() { fresh(kind, source) } else {
                let saved = storage::load_named(&named)?.context("Charted realm checkpoint missing")?;
                ensure!(saved.world.regions.view.session_name() == named, "Charted realm identity mismatch");
                ensure!(saved.world.seed == source.seed && saved.world.generator_version == source.generator_version && saved.world.voxel_scale == source.voxel_scale, "Charted realm configuration mismatch");
                saved.world
            };
            if link.slot(kind).is_empty() {
                let name = target.regions.view.session_name().to_owned();
                *link.slot_mut(kind) = name;
            }
            let feet = player.pos * target.voxel_scale.meters();
            if kind == WorldType::Manyfold {
                let cell = player.pos.floor().as_ivec3();
                let b = target.get(cell);
                ensure!(!b.is_solid() && b != super::BlockId::Lava, "Chart destination obstructed");
            } else {
                clear_stand(&target, player, feet)?;
            }
            if chart == Chart::Nether {
                let gate = hall::canonical_gate(&target.sample_config());
                ensure!(super::hell_gate::intact(&target, gate, engine_worldgen::hell_gate::floor(&target.sample_config(), gate)), "The far gate has been broken");
            }
            super::spawn::record_arrival(&mut target, feet, true);
            Ok((target, link))
        })();
        let (target, link) = match built {
            Ok((target, link)) => (target, Chartlink::Hall(link)),
            Err(e) => {
                player.pos = previous;
                player.precise_pos = previous_precise;
                player.vel = Vec3::ZERO;
                return Err(e);
            }
        };
        self.finish(source, player, target, link, previous, previous_precise, arrival, prepare)
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(&mut self, source: &mut World, player: &mut Player, mut target: World, link: Chartlink,
        previous: Vec3, previous_precise: DVec3, arrival: Vec3,
        prepare: impl FnOnce(&mut World, &Player, Vec3) -> Result<()>) -> Result<Option<World>> {
        if let Err(error) = prepare(&mut target, player, arrival) {
            player.pos = previous;
            player.precise_pos = previous_precise;
            return Err(error);
        }
        exchange_actor(source, &mut target);
        let mut target_saves = SaveService::new();
        target_saves.request(true);
        self.destination = Some(Destination { world: target, link, saves: target_saves, previous, previous_precise });
        Ok(None)
    }

    /// Down a placed shaft into the Lightless, or back up the arrival shaft.
    fn begin_shaft(&mut self, source: &mut World, player: &mut Player, shaft: super::lightless_gate::Shaft,
        prepare: impl FnOnce(&mut World, &Player, Vec3) -> Result<()>) -> Result<Option<World>> {
        use super::lightless_gate::{self as gate, Shaft};
        ensure!(gate::usable(source, player), "Shaft sealed");
        let kind = gate::realm_under(source, player).context("No realm gate underfoot")?;
        let previous = player.pos;
        let previous_precise = player.precise_pos;
        let built = (|| -> Result<(World, gate::Link, Vec3)> {
            let mut link = gate::Link::read(&source.regions.view.root, kind)?.unwrap_or_else(|| gate::Link::new(source));
            link.validate(source)?;
            {
                let name = source.regions.view.session_name().to_owned();
                let here = link.slot_mut(source.world_type);
                if here.is_empty() { *here = name; }
            }
            if shaft == Shaft::Descend {
                // The last shaft used is the way home.
                link.ret = previous.to_array();
            }
            let far_kind = gate::destination(shaft, kind);
            let named = match far_kind { Some(kind) => link.slot(kind).to_owned(), None => link.parent.clone() };
            let mut target = if named.is_empty() {
                fresh(far_kind.context("Shaft has no parent world")?, source)
            } else {
                let saved = storage::load_named(&named)?.context("Shaft realm checkpoint missing")?;
                ensure!(saved.world.regions.view.session_name() == named, "Shaft realm identity mismatch");
                ensure!(saved.world.seed == source.seed && saved.world.generator_version == source.generator_version
                    && saved.world.voxel_scale == source.voxel_scale, "Shaft realm configuration mismatch");
                saved.world
            };
            if link.slot(target.world_type).is_empty() {
                let name = target.regions.view.session_name().to_owned();
                *link.slot_mut(target.world_type) = name;
            }
            let arrival = gate::arrival(source, &link, shaft, kind);
            player.pos = arrival;
            player.precise_pos = arrival.as_dvec3();
            player.vel = Vec3::ZERO;
            let feet = arrival * target.voxel_scale.meters();
            clear_stand(&target, player, feet)?;
            super::spawn::record_arrival(&mut target, feet, true);
            Ok((target, link, arrival))
        })();
        let (target, link, arrival) = match built {
            Ok(triple) => triple,
            Err(e) => {
                player.pos = previous;
                player.precise_pos = previous_precise;
                player.vel = Vec3::ZERO;
                return Err(e);
            }
        };
        self.finish(source, player, target, Chartlink::Shaft(link, kind), previous, previous_precise, arrival, prepare)
    }
}

/// The body fits at `feet` (metres) with solid footing and no liquid.
pub(super) fn clear_stand(target: &World, player: &Player, feet: Vec3) -> Result<()> {
    let scale = target.voxel_scale.meters();
    let radius = (0.45 * player.scale).max(0.45);
    let height = (2.0 * player.scale).max(2.0);
    let lo = ((feet + Vec3::new(-radius, 0.05, -radius)) / scale).floor().as_ivec3();
    let hi = ((feet + Vec3::new(radius, height, radius)) / scale).floor().as_ivec3();
    ensure!((hi - lo).max_element() <= 64, "Body exceeds arch clearance");
    for z in lo.z..=hi.z { for y in lo.y..=hi.y { for x in lo.x..=hi.x {
        let b = target.get(glam::IVec3::new(x, y, z));
        ensure!(!b.is_solid() && b != super::BlockId::Lava && b != super::BlockId::Water,
            "Chart destination obstructed by {b:?} at ({x}, {y}, {z})");
    } } }
    let support = ((feet - Vec3::Y * 0.1) / scale).floor().as_ivec3();
    ensure!(target.get(support).is_solid(), "Chart destination has no footing");
    Ok(())
}
