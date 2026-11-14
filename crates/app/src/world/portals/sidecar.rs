//! Gate links persist beside the world, not in the save body, so the save
//! version is unchanged. A missing or damaged file means no links.
use super::gates::{Gates, Link};
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub const FILE: &str = "gates.ron";
const VERSION: u32 = 1;
const MAX_LINKS: usize = super::MAX_PORTALS / 4;

#[derive(serde::Serialize, serde::Deserialize)]
struct Stored {
    version: u32,
    links: Vec<Link>,
    #[serde(default)]
    hell: Vec<crate::world::hell_gate::built::Threshold>,
    #[serde(default)]
    circles: crate::world::adventure::fieldcraft::CircleRoute,
}

pub fn encode(gates: &Gates) -> Vec<u8> {
    let stored = Stored { version: VERSION, links: gates.links.clone(), hell: gates.hell.clone(), circles: gates.circles };
    ron::to_string(&stored).unwrap_or_default().into_bytes()
}

pub fn decode(bytes: &[u8]) -> Result<Vec<Link>> {
    Ok(decode_stored(bytes)?.links)
}

fn decode_stored(bytes: &[u8]) -> Result<Stored> {
    ensure!(bytes.len() <= 262_144, "Gate file exceeds size limit");
    let stored: Stored = ron::from_str(std::str::from_utf8(bytes)?).context("Unreadable gate file")?;
    ensure!(stored.version == VERSION, "Unsupported gate file version");
    ensure!(stored.links.len() <= MAX_LINKS, "Too many gate links");
    ensure!(stored.hell.len() <= crate::world::hell_gate::built::MAX_GATES, "Too many Hell portals");
    // Physical bounds are checked on load with the saved voxel scale.
    Ok(stored)
}

pub fn save(world_dir: &Path, bytes: &[u8]) -> Result<()> {
    let path = world_dir.join(FILE);
    if bytes.is_empty() || decode_stored(bytes).is_ok_and(|s| {
        s.links.is_empty() && s.hell.is_empty() && s.circles.is_empty()
    }) {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    let temporary = path.with_extension("ron.tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, &path)?;
    Ok(())
}

/// Links whose rings are no longer intact are dropped by the first audits.
pub fn load(world: &mut crate::world::World, world_dir: &Path) {
    let stored = match std::fs::read(world_dir.join(FILE)) {
        Ok(bytes) => decode_stored(&bytes).ok().or_else(|| {
            eprintln!("[gates] ignored invalid {}", FILE);
            None
        }),
        Err(_) => None,
    };
    let (links, hell, circles) = stored.map_or_else(Default::default, |s| (s.links, s.hell, s.circles));
    world.portals.links = links;
    world.portals.circles = circles.validated();
    world.portals.hell = hell.into_iter().filter(|g| g.validate(f64::from(world.voxel_scale.meters())).is_ok()).collect();
    world.portals.rebuild(world.voxel_scale.meters());
}
