use super::*;
use crate::world::factory::chips::{Action, Bay, registry};
use crate::world::factory::{Kind, fixture_place};
use engine_chip::presets::{sentinel, twin_watch};

fn fixture(world: &World, version: u8, directory: &Path) -> PathBuf {
    let mut bytes = session_header(world, &Player::new(), &DayNightCycle::default()).unwrap();
    bytes[..8].copy_from_slice(format!("VXWSAV{version:02}").as_bytes());
    let name = b"chip-test";
    push(&mut bytes, name.len() as u32);
    bytes.extend_from_slice(name);
    push(&mut bytes, 0);
    bytes.extend_from_slice(&session_footer(world, version));
    world.activity.snapshot().encode(&mut bytes);
    bytes.extend_from_slice(&1u64.to_le_bytes());
    let crc = checksum(&bytes);
    bytes.extend_from_slice(&crc.to_le_bytes());
    let path = directory.join(format!("world{version}.vxw"));
    fs::write(&path, bytes).unwrap();
    path
}

fn directory(tag: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let d = std::env::temp_dir().join(format!("voxel_chip_{tag}_{}_{}", std::process::id(), nonce));
    fs::create_dir_all(d.join("regions/chip-test")).unwrap();
    d
}

/// A world whose chip bay holds two released projects, one stocked part and
/// one part installed (and run) at `pump`. The machines live in a scratch
/// world: the save fixture stores no regions, so the saved world has none.
fn world_with_chips() -> (World, IVec3) {
    let mut scratch = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
    let pump = IVec3::new(-40, 10, -7);
    fixture_place(&mut scratch, pump, Kind::Pump);
    let f = &mut scratch.factory;
    for design in [sentinel(registry()), twin_watch(registry())] {
        let name = design.name.clone();
        f.chips.action(&f.nodes, Action::NewProject(name, design), |_| true);
    }
    for project in [1, 2] {
        let design = f.chips.library.project(project).unwrap().draft.clone();
        f.chips.action(&f.nodes, Action::Release { project, design, generation: 0 }, |_| true);
        f.chips.action(&f.nodes, Action::Prototype(project), |_| true);
    }
    f.chips.action(&f.nodes, Action::Install(pump, 2), |_| true);
    f.chips.action(&f.nodes, Action::Thresholds(pump, 250, 800), |_| true);
    for _ in 0..90 {
        f.nodes.get_mut(&pump).unwrap().energy = 64;
        f.tick(1.0 / 120.0, 1);
    }
    let mut world = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
    world.factory.chips = std::mem::take(&mut scratch.factory.chips);
    (world, pump)
}

#[test]
fn save_chip_library_and_installed_state_round_trip() {
    let dir = directory("round_trip");
    let (world, pump) = world_with_chips();
    let before = world.factory.chips.diagnostics(pump).unwrap();
    assert_eq!(before.state.lower, 250);
    assert!(before.state.wakes > 0);
    let path = fixture(&world, SAVE_VERSION, &dir);
    let loaded = load_exact(&path, &path).unwrap();
    let chips = &loaded.world.factory.chips;
    // Draft edit counters are session-only; everything else is identical.
    assert_eq!(chips.library.revisions, world.factory.chips.library.revisions);
    assert_eq!(chips.library.stock, world.factory.chips.library.stock);
    assert_eq!(chips.diagnostics(pump).unwrap().state, before.state);
    assert_eq!(chips.now(), world.factory.chips.now());
    assert_eq!(chips.library.stock.len(), 1, "one part stocked, one installed: no copies");
    let mut a = Vec::new();
    let mut b = Vec::new();
    world.factory.chips.encode(&mut a);
    chips.encode(&mut b);
    assert_eq!(a, b);
}

#[test]
fn save_v33_loads_without_a_chip_library() {
    let dir = directory("legacy");
    let (world, pump) = world_with_chips();
    let path = fixture(&world, 33, &dir);
    let loaded = load_exact(&path, &path).unwrap();
    assert!(world.factory.chips.diagnostics(pump).is_some());
    assert_eq!(loaded.world.factory.chips.installed_count(), 0);
    assert!(loaded.world.factory.chips.library.projects.is_empty());
}

#[test]
fn save_rejects_a_chip_both_installed_and_stocked() {
    let (world, _) = world_with_chips();
    let mut bytes = Vec::new();
    world.factory.chips.encode(&mut bytes);
    let len = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let text = String::from_utf8(bytes[4..4 + len].to_vec()).unwrap();
    // Stock holds serial 1; claim the installed part is serial 1 as well.
    let at = text.find("installed:").unwrap();
    let forged = format!("{}{}", &text[..at], text[at..].replacen("serial:2", "serial:1", 1));
    assert_ne!(forged, text);
    let mut words = vec![forged.len() as u32];
    for chunk in forged.as_bytes().chunks(4) {
        let mut w = [0u8; 4];
        w[..chunk.len()].copy_from_slice(chunk);
        words.push(u32::from_le_bytes(w));
    }
    let mut it = words.into_iter();
    let err = Bay::decode(&mut || it.next().ok_or_else(|| anyhow::anyhow!("eof"))).err().unwrap();
    assert!(err.to_string().contains("twice"), "{err}");
}
