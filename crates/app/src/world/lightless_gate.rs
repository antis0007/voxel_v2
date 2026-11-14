//! Realm gates. Lightless shafts: a placed shaft block under deep rock leads
//! down to the Lightless; the generated shaft in its arrival chamber leads
//! back. A placed gilded gate leads to the Gilded Cave the same way. Travel
//! itself runs through `hall_gate::Travel`, which owns checkpoint ordering.
use super::{BlockId, World, WorldType};
use crate::player::Player;
use anyhow::{Result, ensure};
use engine_voxel::RealmKind;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

pub(super) const FILE: &str = "lightless-chart.ron";

/// Chart file beside both worlds, one per realm.
pub(super) fn file(kind: RealmKind) -> &'static str {
    match kind {
        RealmKind::Lightless => FILE,
        RealmKind::Gilded => "gilded-chart.ron",
        RealmKind::Highfold => "highfold-chart.ron",
    }
}

/// The realm a gate block leads to.
pub fn gate_realm(block: BlockId) -> Option<RealmKind> {
    match block {
        BlockId::LightlessShaft => Some(RealmKind::Lightless),
        BlockId::GildedGate => Some(RealmKind::Gilded),
        _ => None,
    }
}

/// The realm of the gate block under the player's feet.
pub fn realm_under(w: &World, player: &Player) -> Option<RealmKind> {
    let below = (player.pos - player.up_dir() * 0.1 / w.voxel_scale.meters()).floor().as_ivec3();
    gate_realm(w.get(below))
}
/// Rock that must lie over the head before a shaft opens.
pub const DEPTH_M: f32 = 24.0;
/// Share of the cells over the head that must be solid.
const COVER: f32 = 0.75;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shaft {
    Descend,
    Ascend,
}

/// Worlds a shaft can be sunk in: the ordinary surface worlds.
fn parent_kind(kind: WorldType) -> bool {
    matches!(kind, WorldType::Hills | WorldType::Ridged | WorldType::Superflat | WorldType::Planet)
}

pub fn standing(w: &World, player: &Player) -> Option<Shaft> {
    let kind = realm_under(w, player)?;
    if w.world_type.realm() == Some(kind) {
        Some(Shaft::Ascend)
    } else if parent_kind(w.world_type) {
        Some(Shaft::Descend)
    } else {
        None
    }
}

/// True when at least `DEPTH_M` of mostly solid rock lies over the head,
/// measured along the body's up axis so planet charts agree.
pub fn buried(w: &World, player: &Player) -> bool {
    let scale = w.voxel_scale.meters();
    let up = player.up_dir();
    let head = player.pos + up * (2.2 / scale);
    let steps = (DEPTH_M / scale).ceil() as i32;
    let solid = (0..steps)
        .filter(|&i| w.get((head + up * i as f32).floor().as_ivec3()).is_solid())
        .count();
    solid as f32 >= steps as f32 * COVER
}

pub fn usable(w: &World, player: &Player) -> bool {
    match standing(w, player) {
        Some(Shaft::Descend) => (realm_under(w, player) != Some(RealmKind::Lightless) || buried(w, player))
            && w.adventure.health > 0.0,
        Some(Shaft::Ascend) => w.adventure.health > 0.0,
        None => false,
    }
}

pub fn prompt(w: &World, player: &Player, network: bool) -> Option<super::interaction::Prompt> {
    let shaft = standing(w, player)?;
    let lightless = realm_under(w, player) == Some(RealmKind::Lightless);
    let action = if network {
        "Realm crossing requires local play"
    } else {
        match shaft {
            Shaft::Descend if lightless && !buried(w, player) => "Too Shallow: 24 m Rock Required",
            Shaft::Descend if lightless => "F  Descend",
            Shaft::Ascend if lightless => "F  Ascend",
            Shaft::Descend => "F  Enter",
            Shaft::Ascend => "F  Return",
        }
    };
    let title = if lightless { "Lightless Shaft" } else { "Gilded Gate" };
    Some(super::interaction::Prompt { title, action })
}

fn id_ok(s: &str) -> bool {
    s.len() <= 96 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Written beside both worlds. `ret` is the parent shaft stand in voxels.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Link {
    pub parent: String,
    pub lightless: String,
    pub seed: u32,
    pub generator: u32,
    pub ret: [f32; 3],
}

impl Link {
    pub fn new(source: &World) -> Self {
        Link { parent: String::new(), lightless: String::new(), seed: source.seed, generator: source.generator_version, ret: [0.0; 3] }
    }
    pub fn slot(&self, kind: WorldType) -> &str {
        if kind.realm().is_some() { &self.lightless } else { &self.parent }
    }
    pub fn slot_mut(&mut self, kind: WorldType) -> &mut String {
        if kind.realm().is_some() { &mut self.lightless } else { &mut self.parent }
    }
    pub fn read(root: &Path, kind: RealmKind) -> Result<Option<Self>> {
        let path = root.join(file(kind));
        if !path.exists() {
            return Ok(None);
        }
        ensure!(fs::metadata(&path)?.len() <= 4096, "Shaft link is oversized");
        let link: Link = ron::from_str(&fs::read_to_string(&path)?)?;
        ensure!(link.ret.iter().all(|v| v.is_finite() && v.abs() <= 4.0e6), "Shaft link position out of range");
        Ok(Some(link))
    }
    pub fn validate(&self, w: &World) -> Result<()> {
        ensure!(self.seed == w.seed && self.generator == w.generator_version, "Shaft world mismatch");
        let here = self.slot(w.world_type);
        ensure!(here.is_empty() || (id_ok(here) && here == w.regions.view.session_name()), "Shaft realm mismatch");
        Ok(())
    }
    pub fn publish(&self, root: &Path, kind: RealmKind) -> Result<()> {
        ensure!(id_ok(&self.parent) && id_ok(&self.lightless), "Shaft link names are invalid");
        let path = root.join(file(kind));
        if let Some(old) = Self::read(root, kind)? {
            ensure!(old.seed == self.seed && old.generator == self.generator, "Shaft link changed");
            for (was, now) in [(&old.parent, &self.parent), (&old.lightless, &self.lightless)] {
                ensure!(was.is_empty() || was == now, "Shaft link changed");
            }
            if &old == self {
                return Ok(());
            }
        }
        fs::create_dir_all(root)?;
        let stem = file(kind).trim_end_matches(".ron");
        let tmp = root.join(format!("{stem}-{}-{}.tmp", std::process::id(), super::regions::checkpoint_id()));
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(ron::to_string(self)?.as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(tmp, path)?;
        Ok(())
    }
}

/// Arrival feet in voxels for a crossing from `source`.
pub(super) fn arrival(source: &World, link: &Link, shaft: Shaft, kind: RealmKind) -> Vec3 {
    match shaft {
        Shaft::Descend => {
            let cfg = engine_voxel::WorldSampleConfig {
                world_type: WorldType::Realm(kind),
                ..source.sample_config()
            };
            match kind {
                RealmKind::Lightless => engine_worldgen::lightless::spawn(&cfg),
                RealmKind::Gilded => engine_worldgen::gilded::spawn(&cfg),
                RealmKind::Highfold => engine_worldgen::highfold::spawn(&cfg),
            }
        }
        Shaft::Ascend => Vec3::from_array(link.ret),
    }
}

/// The destination world kind for a crossing.
pub(super) fn destination(shaft: Shaft, kind: RealmKind) -> Option<WorldType> {
    match shaft {
        Shaft::Descend => Some(WorldType::Realm(kind)),
        // The parent kind comes from its saved checkpoint.
        Shaft::Ascend => None,
    }
}
