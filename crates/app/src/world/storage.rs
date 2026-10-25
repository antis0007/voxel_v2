//! Versioned local quicksave. Files are checksummed, bounded, and replaced only
//! after a complete write and sync; failed loads never replace the live world.
use super::{BlockId, CHUNK_SIZE, ChunkData, DayNightCycle, World, WorldType};
use crate::fluid::VoxelScalePreset;
use crate::player::{FlyMode, Player};
use anyhow::{Context, Result, ensure};
use glam::{IVec3, Vec3};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAGIC: &[u8; 8] = b"VXWSAV34";
#[cfg(test)]
#[path = "storage/environment_tests.rs"]
mod environment_tests;
#[cfg(test)]
#[path = "storage/chip_tests.rs"]
mod chip_tests;
/// Current writer version. Companion bytes follow the adventure block from 18;
/// the found-spell log (3 words) follows the companion from 19. Version 20
/// widens the resource table to 22 and the machine table to 21 kinds, and
/// the crafting grid to 3 × 3. Version 21 adds induction matrices,
/// transformers and assemblers (27 kinds) and two words per assembly.
/// Version 22 adds non-stackable power-kit item tags and permits matrix pools
/// on induction cells. The slot and factory record lengths are unchanged.
/// Version 23 adds finite Tollkeeper gate ownership and live/dormant encounter damage.
/// Version 24 adds a bounded Meridian orbital alignment booking.
/// Version 25 adds the ground receiver and finite cargo contract ledger.
/// Version 26 adds habitat sources, residents, production and separate mana/XP ledgers.
/// Version 28 adds radio equipment, finite knowledge and registered dish tracking.
/// Version 30 permits monotone receiver stages 3/4 for finite relay recovery.
/// Version 31 persists sparse soil, sediment, ecological stocks and due work.
/// Version 32 adds optional resonance expeditions and permanent calibration.
/// Version 33 stores lantern ownership and switch state in the expedition flags.
/// Version 34 appends the chip library and installed controllers.
pub(crate) const SAVE_VERSION: u8 = 34;
// V1-V9 used a single decimal digit. V10+ keeps the fixed eight-byte header
// and uses two digits; discovery and reclamation share this parser.
fn save_version(bytes: &[u8]) -> Result<u8> {
    let header = bytes.get(..8).context("Truncated save header")?;
    let version = if &header[..7] == b"VXWSAVE" && header[7].is_ascii_digit() {
        header[7] - b'0'
    } else if &header[..6] == b"VXWSAV" && header[6].is_ascii_digit() && header[7].is_ascii_digit()
    {
        (header[6] - b'0') * 10 + header[7] - b'0'
    } else {
        anyhow::bail!("Unsupported save format")
    };
    ensure!((1..=SAVE_VERSION).contains(&version), "Unsupported save version");
    Ok(version)
}
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CHUNKS: usize = 4096;
const VOXELS: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;

pub struct SavedSession {
    pub world: World,
    pub player: Player,
    pub day_night: DayNightCycle,
    pub recovery_notice: Option<String>,
}

pub fn quicksave_path() -> PathBuf {
    save_directory().join("quicksave.vxw")
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, &byte| {
        (hash ^ byte as u64).wrapping_mul(0x100000001b3)
    })
}

fn push(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Decoder<'_> {
    fn u32(&mut self) -> Result<u32> {
        let end = self.offset.checked_add(4).context("Save offset overflow")?;
        let value = self.bytes.get(self.offset..end).context("Truncated save")?;
        self.offset = end;
        Ok(u32::from_le_bytes(value.try_into()?))
    }
    fn float(&mut self) -> Result<f32> {
        let value = f32::from_bits(self.u32()?);
        ensure!(value.is_finite(), "Non-finite value in save");
        Ok(value)
    }
}

mod paths;
pub(crate) use paths::preference_read_path;
pub use paths::save_directory;
mod background;
mod library;
pub use background::{
    SaveService, autosave_enabled, autosave_seconds, set_autosave_enabled, set_autosave_seconds,
};
pub use library::{WorldEntry, WorldLibrary};

fn session_header(world: &World, player: &Player, day: &DayNightCycle) -> Result<Vec<u8>> {
    ensure!(world.ca_view.is_none(), "Experimental CPU region requires an exact checkpoint format; ordinary world save refused");
    ensure!(
        player.pos.abs().max_element() <= 1_000_000.0,
        "Player position exceeds supported save range"
    );
    ensure!((0.1..=10.0).contains(&player.scale), "Invalid player scale");
    ensure!(
        (1.0..=86400.0).contains(&day.day_length_seconds),
        "Invalid day length"
    );
    let mut bytes = MAGIC.to_vec();
    push(&mut bytes, world.generator_version);
    push(&mut bytes, world.seed);
    push(
        &mut bytes,
        match world.world_type {
            WorldType::Superflat => 0,
            WorldType::Hills => 1,
            WorldType::Ridged => 2,
        },
    );
    push(
        &mut bytes,
        match world.voxel_scale {
            VoxelScalePreset::OneMeter => 0,
            VoxelScalePreset::HalfMeter => 1,
            VoxelScalePreset::QuarterMeter => 2,
        },
    );
    for value in [
        player.pos.x,
        player.pos.y,
        player.pos.z,
        if player.precise_pos.as_vec3() == player.pos {
            (player.precise_pos.x - player.pos.x as f64) as f32
        } else {
            0.0
        },
        if player.precise_pos.as_vec3() == player.pos {
            (player.precise_pos.y - player.pos.y as f64) as f32
        } else {
            0.0
        },
        if player.precise_pos.as_vec3() == player.pos {
            (player.precise_pos.z - player.pos.z as f64) as f32
        } else {
            0.0
        },
        player.yaw,
        player.pitch,
        player.scale,
        player.crouch_t,
        day.time_of_day,
        day.day_length_seconds,
    ] {
        ensure!(value.is_finite(), "Cannot save non-finite state");
        push(&mut bytes, value.to_bits());
    }
    push(&mut bytes, u32::from(player.flying));
    push(
        &mut bytes,
        u32::from(matches!(player.fly_mode, FlyMode::Noclip)),
    );
    push(&mut bytes, day.day_index);
    push(&mut bytes, day.weather_seed);
    // Retain every weather control without relying on Rust struct layout.
    let c = day.cloud_controls;
    for value in [
        c.coverage,
        c.density,
        c.cumulus,
        c.stratus,
        c.cumulonimbus,
        c.base_height,
        c.layer_thickness,
        c.storm_thickness,
        c.anvil_spread,
        c.altocumulus,
        c.altostratus,
        c.mid_layer_height,
        c.cirrus,
        c.layer_separation,
        c.wind_shear,
        c.erosion,
        c.advection,
        c.noise_scale,
        c.wind_speed,
        c.wind_dir_angle,
        c.detail_strength,
        c.silver_lining,
    ] {
        ensure!(value.is_finite(), "Cannot save non-finite weather state");
        push(&mut bytes, value.to_bits());
    }
    Ok(bytes)
}
fn session_footer(world: &World, save_version: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    let resources = if save_version >= 20 {
        super::factory::RESOURCE_COUNT
    } else {
        super::factory::LEGACY_RESOURCE_COUNT
    };
    for &amount in &world.expedition.resources[..resources] {
        push(&mut bytes, amount);
    }
    push(&mut bytes, world.expedition.tool_tier);
    push(&mut bytes, u32::from(world.expedition.workshop_found));
    push(&mut bytes, u32::from(world.expedition.lamp)
        | (u32::from(world.expedition.lamp_disabled) << 1));
    world.factory.encode_format(
        &mut bytes,
        match save_version {
            21.. => 10,
            20 => 9,
            _ => 8,
        },
    );
    world.adventure.encode(&mut bytes);
    if save_version >= 18 {
        world.adventure.encode_companion(&mut bytes);
    }
    if save_version >= 19 {
        world.adventure.encode_spell_log(&mut bytes);
    }
    if save_version >= 23 { world.adventure.encode_boss_series(&mut bytes); }
    if save_version >= 29 { world.adventure.encode_wand_bank(&mut bytes); }
    if save_version >= 17 {
        world.undercrown.encode(&mut bytes);
    }
    world.sandbox.encode(&mut bytes);
    if save_version >= 24 { world.orbital.encode(&mut bytes, save_version); }
    if save_version >= 26 { world.habitats.encode_versioned(&mut bytes,save_version); }
    if save_version >= 31 { world.environment.encode(&mut bytes); }
    if save_version >= 32 { world.adventure.resonance.encode(&mut bytes); }
    world.inventory.encode_versioned(&mut bytes, save_version);
    push(&mut bytes, world.inventory.dropped.len() as u32);
    for drop in &world.inventory.dropped {
        for v in drop.pos.to_array() {
            push(&mut bytes, v.to_bits());
        }
        let (kind, payload) = drop.stack.item.encode_u32();
        for v in [kind, payload, drop.stack.count] {
            push(&mut bytes, v);
        }
    }
    if save_version >= 34 { world.factory.chips.encode(&mut bytes); }
    bytes
}
fn manifest(
    mut bytes: Vec<u8>,
    view: &super::regions::RegionView,
    footer: &[u8],
    commit_id: u64,
) -> Result<Vec<u8>> {
    ensure!(
        view.records.len() <= 1_000_000,
        "World index exceeds save limit"
    );
    let name = view.session_name().as_bytes();
    push(&mut bytes, name.len() as u32);
    bytes.extend_from_slice(name);
    push(&mut bytes, view.records.len() as u32);
    for c in view.records.keys() {
        ensure!(
            c.to_array().iter().all(|v| v.unsigned_abs() <= 32_000),
            "Chunk exceeds save range"
        );
        for v in c.to_array() {
            push(&mut bytes, v as u32);
        }
        let r = &view.records[c];
        bytes.extend_from_slice(&r.offset.to_le_bytes());
        push(&mut bytes, r.len);
        bytes.extend_from_slice(&r.checksum.to_le_bytes());
        bytes.extend_from_slice(&r.version.to_le_bytes());
        push(&mut bytes, r.resident_bytes);
        bytes.extend_from_slice(&r.generation.to_le_bytes());
    }
    bytes.extend_from_slice(footer);
    bytes.extend_from_slice(&commit_id.to_le_bytes());
    ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES - 8,
        "Save manifest exceeds size limit"
    );
    bytes.extend_from_slice(&checksum(&bytes).to_le_bytes());
    Ok(bytes)
}

fn load_exact(path: &Path, slot: &Path) -> Result<SavedSession> {
    let file = fs::File::open(path).context("Cannot open quicksave")?;
    ensure!(
        file.metadata()?.len() <= MAX_FILE_BYTES,
        "Save exceeds size limit"
    );
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() >= 16 && bytes.len() as u64 <= MAX_FILE_BYTES,
        "Invalid save size"
    );
    let end = bytes.len() - 8;
    ensure!(
        checksum(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?),
        "Save checksum mismatch"
    );
    let version = save_version(&bytes)?;
    let version_thirteen = version >= 13;
    let version_twelve = version_thirteen || &bytes[..8] == b"VXWSAV12";
    let version_eleven = version_twelve || &bytes[..8] == b"VXWSAV11";
    let version_ten = version_eleven || &bytes[..8] == b"VXWSAV10";
    let version_nine = version_ten || &bytes[..8] == b"VXWSAVE9";
    let version_eight = version_nine || &bytes[..8] == b"VXWSAVE8";
    let version_seven = version_eight || &bytes[..8] == b"VXWSAVE7";
    let version_six = version_seven || &bytes[..8] == b"VXWSAVE6";
    let version_five = version_six || &bytes[..8] == b"VXWSAVE5";
    let version_four = version_five || &bytes[..8] == b"VXWSAVE4";
    let version_three = version_four || &bytes[..8] == b"VXWSAVE3";
    let version_two = version_three || &bytes[..8] == b"VXWSAVE2";
    ensure!(
        version_two || &bytes[..8] == b"VXWSAVE1",
        "Unsupported save version"
    );
    let mut input = Decoder {
        bytes: &bytes[..end],
        offset: 8,
    };
    let generator_version = if version_two { input.u32()? } else { 1 };
    ensure!(
        (1..=super::CURRENT_GENERATOR_VERSION).contains(&generator_version),
        "Unsupported terrain generator version"
    );
    let seed = input.u32()?;
    let kind = match input.u32()? {
        0 => WorldType::Superflat,
        1 => WorldType::Hills,
        2 => WorldType::Ridged,
        _ => anyhow::bail!("Invalid terrain type"),
    };
    let scale = match input.u32()? {
        0 => VoxelScalePreset::OneMeter,
        1 => VoxelScalePreset::HalfMeter,
        2 => VoxelScalePreset::QuarterMeter,
        _ => anyhow::bail!("Invalid voxel scale"),
    };
    let mut world = World::with_seed(kind, scale, seed);
    world.generator_version = generator_version;
    let mut player = Player::new();
    player.pos = Vec3::new(input.float()?, input.float()?, input.float()?);
    player.precise_pos = player.pos.as_dvec3();
    if version_twelve {
        let residual = Vec3::new(input.float()?, input.float()?, input.float()?);
        ensure!(
            residual.abs().max_element() <= 0.125,
            "Invalid player coordinate residual"
        );
        player.precise_pos += residual.as_dvec3();
    }
    ensure!(
        player.pos.abs().max_element() <= 1_000_000.0,
        "Player position exceeds supported range"
    );
    player.yaw = input.float()?.rem_euclid(std::f32::consts::TAU);
    player.pitch = input.float()?.clamp(-1.54, 1.54);
    player.scale = input.float()?;
    ensure!((0.1..=10.0).contains(&player.scale), "Invalid player scale");
    player.crouch_t = input.float()?.clamp(0.0, 1.0);
    let time = input.float()?;
    let length = input.float()?;
    ensure!((1.0..=86400.0).contains(&length), "Invalid day length");
    let mut day_night = DayNightCycle::new(time, length);
    player.flying = input.u32()? != 0;
    player.fly_mode = if input.u32()? == 0 {
        FlyMode::Creative
    } else {
        FlyMode::Noclip
    };
    day_night.day_index = input.u32()?;
    day_night.weather_seed = input.u32()?;
    let c = &mut day_night.cloud_controls;
    for target in [
        &mut c.coverage,
        &mut c.density,
        &mut c.cumulus,
        &mut c.stratus,
        &mut c.cumulonimbus,
        &mut c.base_height,
        &mut c.layer_thickness,
        &mut c.storm_thickness,
        &mut c.anvil_spread,
        &mut c.altocumulus,
        &mut c.altostratus,
        &mut c.mid_layer_height,
        &mut c.cirrus,
        &mut c.layer_separation,
        &mut c.wind_shear,
        &mut c.erosion,
        &mut c.advection,
        &mut c.noise_scale,
        &mut c.wind_speed,
        &mut c.wind_dir_angle,
        &mut c.detail_strength,
        &mut c.silver_lining,
    ] {
        *target = input.float()?;
    }
    day_night.cloud_controls = super::sanitize_cloud_controls(day_night.cloud_controls);
    if version_three {
        let name_len = input.u32()? as usize;
        ensure!(
            (1..=96).contains(&name_len),
            "Invalid world region identity"
        );
        let end_name = input
            .offset
            .checked_add(name_len)
            .context("Region name overflow")?;
        let name = std::str::from_utf8(
            input
                .bytes
                .get(input.offset..end_name)
                .context("Truncated region identity")?,
        )?;
        ensure!(
            name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Invalid region directory name"
        );
        let root = slot
            .parent()
            .unwrap_or(Path::new("."))
            .join("regions")
            .join(name);
        input.offset = end_name;
        let count = input.u32()? as usize;
        ensure!(count <= 1_000_000, "Oversized region index");
        let mut records = std::collections::HashMap::new();
        for _ in 0..count {
            let coord = IVec3::new(
                input.u32()? as i32,
                input.u32()? as i32,
                input.u32()? as i32,
            );
            ensure!(
                coord.to_array().iter().all(|v| v.unsigned_abs() <= 32_000),
                "Invalid region coordinate"
            );
            let offset = input.u32()? as u64 | ((input.u32()? as u64) << 32);
            let len = input.u32()?;
            let checksum = input.u32()? as u64 | ((input.u32()? as u64) << 32);
            let version = input.u32()? as u64 | ((input.u32()? as u64) << 32);
            let resident_bytes = input.u32()?;
            let generation = if version_five {
                input.u32()? as u64 | ((input.u32()? as u64) << 32)
            } else {
                0
            };
            ensure!(
                (65536..=8 * 1024 * 1024).contains(&resident_bytes),
                "Invalid region residency size"
            );
            ensure!(
                (28..=1024 * 1024).contains(&len) && version < u64::MAX - 1,
                "Invalid region extent/revision"
            );
            ensure!(
                records
                    .insert(
                        coord,
                        super::regions::Record {
                            pin: None,
                            generation,
                            offset,
                            len,
                            checksum,
                            version,
                            resident_bytes,
                        }
                    )
                    .is_none(),
                "Duplicate region coordinate"
            );
        }
        if version_four {
            let resources = if version >= 20 {
                super::factory::RESOURCE_COUNT
            } else if version_seven {
                super::factory::LEGACY_RESOURCE_COUNT
            } else if version_six {
                14
            } else {
                9
            };
            for amount in &mut world.expedition.resources[..resources] {
                *amount = input.u32()?;
                ensure!(
                    *amount <= 64_000_000,
                    "Invalid expedition resource quantity"
                );
            }
            world.expedition.tool_tier = input.u32()?;
            ensure!(world.expedition.tool_tier <= 2, "Invalid tool tier");
            let found = input.u32()?;
            let lamp = input.u32()?;
            ensure!(found <= 1 && lamp <= if version >= 33 { 3 } else { 1 }, "Invalid expedition flags");
            world.expedition.workshop_found = found != 0;
            world.expedition.lamp = lamp & 1 != 0;
            world.expedition.lamp_disabled = lamp & 2 != 0;
        }
        if version_six {
            let size = super::factory::step(&world);
            world.factory = super::factory::Factory::decode(
                &mut || input.u32(),
                size,
                if version >= 21 {
                    10
                } else if version >= 20 {
                    9
                } else if version_eight {
                    8
                } else if version_seven {
                    7
                } else {
                    6
                },
            )?;
        }
        if version_seven {
            world.adventure = super::adventure::Adventure::decode(
                &mut || input.u32(),
                version_eleven,
                version >= 16,
            )?;
            if version >= 18 {
                world.adventure.companion =
                    super::adventure::Adventure::decode_companion(&mut || input.u32())?;
            }
            // Saves before 19 predate discovery: every spell stays found, so a
            // player never loses access to something they already owned.
            if version >= 19 {
                world.adventure.spell_log =
                    super::adventure::Adventure::decode_spell_log(&mut || input.u32())?;
            }
            if version >= 23 { world.adventure.decode_boss_series(&mut || input.u32())?; }
            // Before 29 the single wand lives only in the working copy; the first
            // tick mints it into the bank and the inventory.
            if version >= 29 { world.adventure.decode_wand_bank(&mut || input.u32())?; }
        }
        if version >= 17 {
            world.undercrown = super::undercrown::State::decode(&mut || input.u32())?;
        }
        if version_twelve {
            world.sandbox = crate::simulation::sandbox::Sandbox::decode(&mut || input.u32())?;
        }
        if version >= 24 {
            world.orbital = super::orbital::State::decode(&mut || input.u32(), version)?;
        }
        if version >= 26 {
            world.habitats = super::habitats::State::decode(&mut || input.u32(), version)?;
            world.adventure.restore_farm_mastery(world.habitats.mastered);
        }
        if version_thirteen {
            if version >= 31 {
                world.environment = super::environment::State::decode(&mut || input.u32(), world.voxel_scale)?;
            }
            if version >= 32 {
                world.adventure.resonance = super::adventure::resonance::State::decode(&mut || input.u32())?;
            }
            world.inventory = super::inventory::SurvivalInventory::decode_versioned(
                &mut || input.u32(),
                world.voxel_scale.meters(),
                version,
            )?;
            if version >= 16 {
                let count = input.u32()?;
                ensure!(count <= 1024, "Too many dropped stacks");
                for _ in 0..count {
                    let pos = Vec3::new(input.float()?, input.float()?, input.float()?);
                    let item = super::inventory::Item::decode_u32(input.u32()?, input.u32()?)?;
                    let count = input.u32()?;
                    ensure!(
                        count > 0 && count <= world.inventory.stack_limit(item),
                        "Invalid dropped stack"
                    );
                    world
                        .inventory
                        .dropped
                        .push(super::inventory::DroppedStack {
                            pos,
                            stack: super::inventory::ItemStack { item, count },
                        });
                }
            }
            if version >= 34 {
                world.factory.chips = super::factory::chips::Bay::decode(&mut || input.u32())?;
            }
            let bank = world.adventure.wand_bank.len();
            let inv = &world.inventory;
            ensure!(
                inv.hotbar.iter().chain(inv.pack.iter()).chain(inv.craft_grid.iter()).chain(std::iter::once(&inv.cursor)).flatten().map(|s| s.item)
                    .chain(inv.dropped.iter().map(|d| d.stack.item))
                    .all(|item| !matches!(item, super::inventory::Item::Wand { id, .. } if id as usize >= bank)),
                "Wand item without a wand record"
            );
            world.sync_from_inventory();
        } else {
            world.migrate_legacy_save_to_inventory();
        }
        if world.ensure_starter_wand() {
            // Pre-wand saves could load without any discoverable route to
            // spellcasting. Grant the same Lumen Twig a fresh world receives.
            world.expedition.say(
                "Lumen Twig attuned. Hold R to cast, T cycles spells, and E → Gear arranges the deck.",
            );
        }
        if version_nine {
            world.activity = super::activity::Activity::decode(&mut || input.u32(), version_ten)?;
        } else {
            // Old saves did not retain pending simulation. Resume stored chunks
            // through a bounded background scan instead of reading them at load.
            world.activity.request_recovery();
        }
        if version_five {
            input.u32()?;
            input.u32()?;
        }
        ensure!(input.offset == end, "Unexpected save manifest data");
        ensure!(
            root.is_dir() || records.is_empty(),
            "World regions directory is missing"
        );
        world.regions = super::regions::RegionStream::open(root, records);
        world.regions.view.ensure_owner()?;
        background::validate_extents(&world.regions.view)?;
        // Inventories and physical assemblies must belong to the same checkpoint.
        // Reject a missing assembly so recovery can try the previous save.
        for (p, node) in &world.factory.nodes {
            let cfg = world.sample_config();
            let size = super::factory::step(&world);
            ensure!(
                p.y >= cfg.min_world_y() && p.y + size <= cfg.max_world_y(),
                "Factory outside world bounds"
            );
            let chunk = world
                .regions
                .view
                .load(super::chunk_coord_from_world_block(*p))?
                .context("Factory assembly region missing")?;
            ensure!(
                super::factory::cells(*p, size)
                    .iter()
                    .all(
                        |c| chunk.chunk.blocks[ChunkData::local_index(c.x, c.y, c.z)]
                            == node.kind.block()
                    ),
                "Factory assembly disagrees with saved terrain"
            );
        }

        return Ok(SavedSession {
            world,
            player,
            day_night,
            recovery_notice: None,
        });
    }
    let count = input.u32()? as usize;
    ensure!(count <= MAX_CHUNKS, "Too many saved chunks");
    for _ in 0..count {
        let coord = IVec3::new(
            input.u32()? as i32,
            input.u32()? as i32,
            input.u32()? as i32,
        );
        ensure!(
            coord
                .to_array()
                .into_iter()
                .all(|v| (-32_000..=32_000).contains(&v)),
            "Invalid chunk coordinate"
        );
        ensure!(
            !world.chunks.contains_key(&coord),
            "Duplicate chunk in save"
        );
        let run_count = input.u32()? as usize;
        ensure!((1..=VOXELS).contains(&run_count), "Invalid run count");
        let mut blocks = vec![BlockId::Air; VOXELS];
        let mut offset = 0usize;
        for _ in 0..run_count {
            let length = input.u32()? as usize;
            let raw = input.u32()?;
            ensure!(length > 0 && length <= VOXELS - offset, "Invalid block run");
            let block = BlockId::from_u32(raw);
            ensure!(block.as_u32() == raw, "Unknown material in save");
            blocks[offset..offset + length].fill(block);
            offset += length;
        }
        ensure!(offset == VOXELS, "Incomplete chunk");
        let mut torches = HashSet::new();
        for (index, block) in blocks.iter().enumerate() {
            if block.is_light_source() {
                let i = index as i32;
                torches.insert(
                    coord * CHUNK_SIZE
                        + IVec3::new(
                            i % CHUNK_SIZE,
                            i / (CHUNK_SIZE * CHUNK_SIZE),
                            (i / CHUNK_SIZE) % CHUNK_SIZE,
                        ),
                );
            }
        }
        world.regions.mark_dirty(coord);
        world.chunks.insert(
            coord,
            Arc::new(ChunkData {
                modified_bounds: Some((IVec3::ZERO, IVec3::splat(CHUNK_SIZE))),
                structural_bounds: Some((IVec3::ZERO, IVec3::splat(CHUNK_SIZE))),
                blocks: blocks
                    .into_boxed_slice()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Invalid chunk length"))?,
                light: Box::new([0; VOXELS]),
                torches,
            }),
        );
    }
    if version_two {
        let count = input.u32()? as usize;
        ensure!(count <= 1_000_000, "Too many water levels");
        for _ in 0..count {
            let p = IVec3::new(
                input.u32()? as i32,
                input.u32()? as i32,
                input.u32()? as i32,
            );
            let volume = input.u32()?;
            ensure!((1..8).contains(&volume), "Invalid water volume");
            ensure!(
                p.to_array().iter().all(|v| v.unsigned_abs() <= 1_000_000),
                "Invalid water position"
            );
            ensure!(
                world.get(p) == BlockId::Water,
                "Water level has no water block"
            );
            ensure!(
                world.water_levels.insert(p, volume as u8).is_none(),
                "Duplicate water level"
            );
        }
    }
    world.migrate_legacy_save_to_inventory();
    world.activity.request_recovery();
    ensure!(input.offset == end, "Unexpected data after save");
    Ok(SavedSession {
        world,
        player,
        day_night,
        recovery_notice: None,
    })
}

/// Explicit opt-in runtime review uses the production checkpoint decoder.
pub(crate) fn load_review_checkpoint() -> Result<SavedSession> {
    let path = quicksave_path();
    load_exact(&path, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::material::BlockId;
    use crate::world::inventory::{ArmorTier, EquipSlot, FoodKind, Item, ItemStack};
    use crate::world::{DayNightCycle, WorldType};

    #[test]
    fn save_and_load_preserves_survival_inventory_and_migrates_v12() {
        let dir = std::env::temp_dir().join(format!("voxel_inv_save_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let save_path = dir.join("world.dat");

        let mut world = World::new(WorldType::Superflat, VoxelScalePreset::HalfMeter);
        world.inventory.hotbar[0] = Some(ItemStack::new(Item::Weapon(2), 1));
        world.inventory.hotbar[4] = Some(ItemStack::new(Item::Block(BlockId::Torch), 19));
        world.inventory.pack[3] = Some(ItemStack::new(Item::Resource(0), 42));
        world.inventory.pack[10] = Some(ItemStack::new(Item::Food(FoodKind::RoastedCap), 7));
        world.inventory.equipment[1] = Some(ItemStack::new(
            Item::Armor(EquipSlot::Chest, ArmorTier::Copper),
            1,
        ));
        world.inventory.craft_grid[0] = Some(ItemStack::new(Item::Block(BlockId::Planks), 3));
        world.inventory.pack[11] = Some(ItemStack {
            item: Item::Block(BlockId::Stone),
            count: 512,
        });
        world
            .inventory
            .dropped
            .push(super::super::inventory::DroppedStack {
                pos: Vec3::new(2.0, 4.0, -3.0),
                stack: ItemStack {
                    item: Item::Block(BlockId::Stone),
                    count: 400,
                },
            });
        world.sync_from_inventory();

        let player = Player::new();
        let day_night = DayNightCycle::default();

        // Write V13 manifest bytes using production header + region identity + footer + activity + commit_id
        let mut bytes = Vec::with_capacity(512);
        bytes.extend_from_slice(&session_header(&world, &player, &day_night).unwrap());
        let region_name = b"test-world";
        bytes.extend_from_slice(&(region_name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(region_name);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // 0 regions
        let mut footer = session_footer(&world, SAVE_VERSION);
        world.activity.snapshot().encode(&mut footer);
        bytes.extend_from_slice(&footer);
        bytes.extend_from_slice(&1u64.to_le_bytes()); // commit_id
        let crc = checksum(&bytes);
        bytes.extend_from_slice(&crc.to_le_bytes());
        std::fs::write(&save_path, &bytes).unwrap();

        let loaded = load_exact(&save_path, &save_path).unwrap();
        assert_eq!(loaded.world.inventory.pack[11].unwrap().count, 512);
        assert_eq!(loaded.world.inventory.dropped.len(), 1);
        assert_eq!(loaded.world.inventory.dropped[0].stack.count, 400);
        assert_eq!(
            loaded.world.inventory.dropped[0].pos,
            Vec3::new(2.0, 4.0, -3.0)
        );
        assert_eq!(
            loaded.world.inventory.hotbar[0],
            Some(ItemStack::new(Item::Weapon(2), 1))
        );
        assert_eq!(
            loaded.world.inventory.hotbar[4],
            Some(ItemStack::new(Item::Block(BlockId::Torch), 19))
        );
        assert_eq!(
            loaded.world.inventory.pack[3],
            Some(ItemStack::new(Item::Resource(0), 42))
        );
        assert_eq!(
            loaded.world.inventory.pack[10],
            Some(ItemStack::new(Item::Food(FoodKind::RoastedCap), 7))
        );
        assert_eq!(
            loaded.world.inventory.equipment[1],
            Some(ItemStack::new(
                Item::Armor(EquipSlot::Chest, ArmorTier::Copper),
                1
            ))
        );
        assert_eq!(
            loaded.world.inventory.craft_grid[0],
            Some(ItemStack::new(Item::Block(BlockId::Planks), 3))
        );
        assert!(loaded.world.undercrown.is_empty());

        // A synthetic v17 manifest proves the Undercrown state layout before
        // production starts writing version 17.
        let mut v17_world = World::new(WorldType::Superflat, VoxelScalePreset::HalfMeter);
        let record = crate::world::undercrown::ObjectId::new(2, 3, 4).unwrap();
        assert!(v17_world.undercrown.set(
            record,
            crate::world::undercrown::flags::DISCOVERED | crate::world::undercrown::flags::OPEN
        ));
        let mut v17_bytes = Vec::with_capacity(512);
        let mut v17_header = session_header(&v17_world, &player, &day_night).unwrap();
        v17_header[..8].copy_from_slice(b"VXWSAV17");
        v17_bytes.extend_from_slice(&v17_header);
        v17_bytes.extend_from_slice(&(region_name.len() as u32).to_le_bytes());
        v17_bytes.extend_from_slice(region_name);
        v17_bytes.extend_from_slice(&0u32.to_le_bytes());
        let mut v17_footer = session_footer(&v17_world, 17);
        v17_world.activity.snapshot().encode(&mut v17_footer);
        v17_bytes.extend_from_slice(&v17_footer);
        v17_bytes.extend_from_slice(&3u64.to_le_bytes());
        let v17_crc = checksum(&v17_bytes);
        v17_bytes.extend_from_slice(&v17_crc.to_le_bytes());
        let v17_path = dir.join("world_v17.dat");
        std::fs::write(&v17_path, &v17_bytes).unwrap();
        let loaded_v17 = load_exact(&v17_path, &v17_path).unwrap();
        assert_eq!(
            loaded_v17.world.undercrown.get(record),
            crate::world::undercrown::flags::DISCOVERED | crate::world::undercrown::flags::OPEN
        );

        // Now construct a legacy V12 save (without the V13 inventory section) and verify migration
        let mut v12_world = World::new(WorldType::Superflat, VoxelScalePreset::HalfMeter);
        v12_world.expedition.resources[0] = 18 * 64; // 18 whole Timber units (64 fixed-point units each)
        v12_world.factory.kits[1] = 2; // Smelter kits
        v12_world.adventure.rations = 4;
        v12_world.adventure.weapons |= 1 << (2 - 1); // Quarry hammer (tier 2)
        let mut v12_bytes = Vec::with_capacity(512);
        let mut v12_header = session_header(&v12_world, &player, &day_night).unwrap();
        v12_header[..8].copy_from_slice(b"VXWSAV12");
        v12_bytes.extend_from_slice(&v12_header);
        v12_bytes.extend_from_slice(&(region_name.len() as u32).to_le_bytes());
        v12_bytes.extend_from_slice(region_name);
        v12_bytes.extend_from_slice(&0u32.to_le_bytes()); // 0 regions
        for &amount in &v12_world.expedition.resources[..crate::world::factory::LEGACY_RESOURCE_COUNT] {
            push(&mut v12_bytes, amount);
        }
        push(&mut v12_bytes, v12_world.expedition.tool_tier);
        push(
            &mut v12_bytes,
            u32::from(v12_world.expedition.workshop_found),
        );
        push(&mut v12_bytes, u32::from(v12_world.expedition.lamp));
        v12_world.factory.encode_format(&mut v12_bytes, 8);
        v12_world.adventure.encode(&mut v12_bytes);
        v12_bytes.truncate(v12_bytes.len() - 24); // V12 adventure encoded without V15 wand loadout (6 u32s)
        v12_world.sandbox.encode(&mut v12_bytes);
        v12_world.activity.snapshot().encode(&mut v12_bytes);
        v12_bytes.extend_from_slice(&2u64.to_le_bytes()); // commit_id
        let v12_crc = checksum(&v12_bytes);
        v12_bytes.extend_from_slice(&v12_crc.to_le_bytes());
        let v12_path = dir.join("world_v12.dat");
        std::fs::write(&v12_path, &v12_bytes).unwrap();

        let migrated = load_exact(&v12_path, &v12_path).unwrap();
        assert_eq!(migrated.world.inventory.count_item(Item::Resource(0)), 18);
        assert_eq!(
            migrated
                .world
                .inventory
                .count_item(Item::Block(crate::world::factory::Kind::ALL[1].block())),
            2
        );
        assert_eq!(
            migrated
                .world
                .inventory
                .count_item(Item::Food(FoodKind::FungalRation)),
            4
        );
        assert_eq!(migrated.world.inventory.count_item(Item::Weapon(2)), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Builds a manifest the way the production writer does, but at an explicit
    /// footer version, so older layouts can be loaded through the real decoder.
    fn synth_save(world: &World, version: u8, dir: &Path, name: &str) -> PathBuf {
        let player = Player::new();
        let day_night = DayNightCycle::default();
        let mut bytes = Vec::with_capacity(512);
        let mut header = session_header(world, &player, &day_night).unwrap();
        header[..8].copy_from_slice(format!("VXWSAV{version:02}").as_bytes());
        bytes.extend_from_slice(&header);
        let region_name = b"test-world";
        bytes.extend_from_slice(&(region_name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(region_name);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let mut footer = session_footer(world, version);
        world.activity.snapshot().encode(&mut footer);
        bytes.extend_from_slice(&footer);
        bytes.extend_from_slice(&1u64.to_le_bytes());
        let crc = checksum(&bytes);
        bytes.extend_from_slice(&crc.to_le_bytes());
        let path = dir.join(name);
        std::fs::write(&path, &bytes).unwrap();
        path
    }

    #[test]
    fn found_spells_are_saved_from_v19_and_older_saves_keep_every_spell() {
        use crate::world::adventure::discovery::SpellLog;
        let dir = std::env::temp_dir().join(format!("voxel_spell_log_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut world = World::new(WorldType::Superflat, VoxelScalePreset::HalfMeter);
        world.adventure.begin_journey();
        world.adventure.spell_log.add(29);
        assert_ne!(world.adventure.spell_log, SpellLog::all());

        let current = synth_save(&world, 19, &dir, "world_v19.dat");
        let loaded = load_exact(&current, &current).unwrap();
        assert_eq!(loaded.world.adventure.spell_log, world.adventure.spell_log);
        assert!(loaded.world.adventure.spell_log.has(29));
        assert!(!loaded.world.adventure.spell_log.has(3));

        // A v18 file has no log block. It must still decode, and the player
        // must keep everything: discovery never takes spells away.
        let legacy = synth_save(&world, 18, &dir, "world_v18.dat");
        let old = load_exact(&legacy, &legacy).unwrap();
        assert_eq!(old.world.adventure.spell_log, SpellLog::all());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_20_keeps_batteries_coke_and_a_v19_layout_still_decodes() {
        use crate::world::factory::{self, Factory, Kind};
        let dir = std::env::temp_dir().join(format!("voxel_v20_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Resources and the 3 x 3 grid through the real loader.
        let mut world = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        world.adventure.begin_journey();
        world.expedition.resources[factory::COKE] = 5 * 64;
        world.reconcile_legacy_to_inventory();
        world.inventory.craft_grid[8] = Some(ItemStack::new(Item::Block(BlockId::Clay), 2));
        let current = synth_save(&world, SAVE_VERSION, &dir, "world_v20.dat");
        let loaded = load_exact(&current, &current).unwrap();
        assert_eq!(loaded.world.inventory.count_item(Item::Resource(factory::COKE as u8)), 5);
        assert_eq!(loaded.world.inventory.craft_grid[8].unwrap().count, 2);
        let legacy = synth_save(&world, 19, &dir, "world_v19.dat");
        assert!(load_exact(&legacy, &legacy).is_ok());
        let _ = std::fs::remove_dir_all(&dir);

        // Machines through the factory codec, current and legacy formats.
        let mut f = Factory::default();
        let mut battery = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        let cell = glam::IVec3::new(4, 40, 4);
        let node = factory::fixture_place(&mut battery, cell, Kind::Battery).unwrap();
        node.energy = 9000;
        node.mode = 3;
        f.nodes.insert(cell, node.clone());
        let mut bytes = Vec::new();
        f.encode(&mut bytes);
        let mut words = bytes.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap()));
        let back = Factory::decode(&mut || Ok(words.next().unwrap()), 1, factory::FACTORY_FORMAT).unwrap();
        let n = &back.nodes[&cell];
        assert_eq!((n.kind, n.energy, n.mode), (Kind::Battery, 9000, 3));

        let mut old = Factory::default();
        let mut furnace = factory::fixture_place(&mut battery, cell + glam::IVec3::X, Kind::Furnace)
            .unwrap()
            .clone();
        furnace.input[9] = 64;
        old.nodes.insert(cell, furnace);
        let mut bytes = Vec::new();
        old.encode_format(&mut bytes, 8);
        let mut words = bytes.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap()));
        let back = Factory::decode(&mut || Ok(words.next().unwrap()), 1, 8).unwrap();
        assert!(words.next().is_none());
        assert_eq!(back.nodes[&cell].input[9], 64);
    }

    #[test]
    fn save_version_17_is_accepted_and_magic_agrees() {
        assert_eq!(save_version(b"VXWSAV17").unwrap(), 17);
        assert_eq!(save_version(b"VXWSAV18").unwrap(), 18);
        assert_eq!(save_version(b"VXWSAV19").unwrap(), 19);
        assert_eq!(save_version(b"VXWSAV20").unwrap(), 20);
        assert_eq!(save_version(b"VXWSAV21").unwrap(), 21);
        assert_eq!(save_version(b"VXWSAV22").unwrap(), 22);
        assert_eq!(save_version(b"VXWSAV23").unwrap(), 23);
        assert_eq!(save_version(b"VXWSAV24").unwrap(), 24);
        let next = format!("VXWSAV{:02}", SAVE_VERSION + 1);
        assert!(save_version(next.as_bytes()).is_err(), "a future version must be rejected");
        let magic_version = (MAGIC[6] - b'0') * 10 + (MAGIC[7] - b'0');
        assert_eq!(magic_version, SAVE_VERSION);
    }

    #[test]
    fn v22_keeps_portable_charge_drops_matrix_pool_and_partial_assembler() {
        use super::super::factory::{self, Kind, portable::PowerKit};
        let dir = std::env::temp_dir().join(format!("voxel_v22_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        w.regions = super::super::regions::RegionStream::open(dir.join("regions/test-world"), Default::default());
        let item = Item::PowerKit(PowerKit { kind: Kind::Battery, energy: 54_321, upgrade: 2, mode: 1, limit: 23 });
        w.inventory.pack[0] = Some(ItemStack::new(item, 1));
        w.inventory.cursor = Some(ItemStack::new(item, 1));
        w.inventory.dropped.push(super::super::inventory::DroppedStack { pos: Vec3::new(3.0, 40.0, 3.0), stack: ItemStack::new(item, 1) });
        let p = glam::IVec3::new(4, 40, 4);
        let a = factory::fixture_place(&mut w, p, Kind::Assembler).unwrap();
        a.target = super::super::inventory::all_recipes().iter().position(|r|
            r.output.item == Item::Block(Kind::Generator.block())).unwrap() as u32 + 1;
        a.progress = 3; a.input[6] = 128; a.input[8] = 128; a.energy = 40;
        let c = factory::fixture_place(&mut w, p + glam::IVec3::X, Kind::InductionCell).unwrap();
        c.energy = 2 * factory::induction::CELL_CAPACITY;
        let (view, _) = w.capture_edits().unwrap().unwrap().persist().unwrap();
        let mut footer = session_footer(&w, SAVE_VERSION);
        w.activity.snapshot().encode(&mut footer);
        let bytes = manifest(session_header(&w, &Player::new(), &DayNightCycle::default()).unwrap(), &view, &footer, 1).unwrap();
        let path = dir.join("current.vxw");
        std::fs::write(&path, bytes).unwrap();
        let loaded = load_exact(&path, &path).unwrap().world;
        assert_eq!(loaded.inventory.pack[0], w.inventory.pack[0]);
        assert_eq!(loaded.inventory.cursor, w.inventory.cursor);
        assert_eq!(loaded.inventory.dropped[0].stack.item, item);
        assert_eq!(loaded.factory.nodes[&p].progress, 3);
        assert_eq!(loaded.factory.nodes[&(p + glam::IVec3::X)].energy, 2 * factory::induction::CELL_CAPACITY);
        // A real v21-layout file still loads through the production decoder.
        let legacy = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        let path = synth_save(&legacy, 21, &dir, "legacy21.vxw");
        assert!(load_exact(&path, &path).is_ok());
        drop(loaded); drop(w); drop(view);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn v29_keeps_every_wand_and_v28_wand_becomes_an_item() {
        use super::super::adventure::wand_items;
        let dir = std::env::temp_dir().join(format!("voxel_v29_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        w.inventory = super::super::inventory::SurvivalInventory::empty();
        w.adventure.wand = 2;
        w.adventure.loadout.set_slot(3, 4);
        w.adventure.sync_wands(&mut w.inventory, 0, Vec3::ZERO);
        let found = w.adventure.mint_wand(wand_items::generate(4242, 3)).unwrap();
        w.inventory.pack[4] = Some(ItemStack::new(found, 1));
        let stray = w.adventure.mint_wand(wand_items::generate(17, 1)).unwrap();
        w.inventory.dropped.push(super::super::inventory::DroppedStack { pos: Vec3::new(3.0, 40.0, 3.0), stack: ItemStack::new(stray, 1) });
        let path = synth_save(&w, SAVE_VERSION, &dir, "world_v29.dat");
        let loaded = load_exact(&path, &path).unwrap().world;
        assert_eq!(loaded.adventure.wand_bank, w.adventure.wand_bank);
        assert_eq!(loaded.adventure.active_wand, Some(0));
        assert_eq!(loaded.adventure.loadout.slot(3), 4);
        assert_eq!(loaded.inventory.pack[4], w.inventory.pack[4]);
        assert_eq!(loaded.inventory.dropped[0].stack.item, stray);
        // Holding a found wand with a bigger pool than the plain formula, at full mana.
        let mut rich = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        rich.inventory = super::super::inventory::SurvivalInventory::empty();
        let seed = (1..10_000).find(|&s| wand_items::stats_for(3, &wand_items::generate(s, 3).loadout, 0).mana_max > 150.0).unwrap();
        let item = rich.adventure.mint_wand(wand_items::generate(seed, 3)).unwrap();
        rich.inventory.hotbar[0] = Some(ItemStack::new(item, 1));
        rich.adventure.sync_wands(&mut rich.inventory, 0, Vec3::ZERO);
        rich.adventure.mana = rich.adventure.max_mana();
        assert!(rich.adventure.mana > 150.0);
        let path = synth_save(&rich, SAVE_VERSION, &dir, "world_v29_rich.dat");
        let back = load_exact(&path, &path).unwrap().world;
        assert_eq!(back.adventure.mana, rich.adventure.mana);
        assert_eq!(back.adventure.loadout.seed, seed);
        // A version 28 save knows one wand only; the first tick turns it into an item.
        let mut legacy = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        legacy.inventory = super::super::inventory::SurvivalInventory::empty();
        legacy.adventure.wand = 3;
        legacy.adventure.loadout.set_slot(5, 2);
        let path = synth_save(&legacy, 28, &dir, "world_v28.dat");
        let mut old = load_exact(&path, &path).unwrap().world;
        assert!(old.adventure.wand_bank.is_empty());
        assert_eq!(old.adventure.wand, 3);
        let mut inv = std::mem::take(&mut old.inventory);
        old.adventure.sync_wands(&mut inv, 0, Vec3::ZERO);
        assert_eq!(old.adventure.wand_bank.len(), 1);
        assert_eq!(old.adventure.wand_bank[0].tier, 3);
        assert_eq!(old.adventure.wand_bank[0].loadout.slot(5), 2);
        assert!(inv.hotbar.iter().chain(inv.pack.iter()).flatten().any(|s| matches!(s.item, Item::Wand { id: 0, .. })));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
