//! Versioned local quicksave. Files are checksummed, bounded, and replaced only
//! after a complete write and sync; failed loads never replace the live world.
use super::{BlockId, CHUNK_SIZE, ChunkData, DayNightCycle, World, WorldType};
use crate::fluid::VoxelScalePreset;
use crate::player::{FlyMode, Player};
use engine_core::cube_sphere::atlas;
use anyhow::{Context, Result, ensure};
use glam::{IVec3, Vec3};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// The header and footer must use the same version owner. A separately written
// magic string can label new sections as an old layout and shift every reader.
#[cfg(test)]
const MAGIC: &[u8; 8] = &save_magic(SAVE_VERSION);

const fn save_magic(version: u8) -> [u8; 8] {
    assert!(version > 0 && version < 100, "Save version must fit its eight-byte header");
    if version < 10 {
        [b'V', b'X', b'W', b'S', b'A', b'V', b'E', b'0' + version]
    } else {
        [b'V', b'X', b'W', b'S', b'A', b'V', b'0' + version / 10, b'0' + version % 10]
    }
}

#[cfg(test)]
#[path = "storage/format_tests.rs"]
mod format_tests;
#[cfg(test)]
#[path = "storage/environment_tests.rs"]
mod environment_tests;
#[cfg(test)]
#[path = "storage/chip_tests.rs"]
mod chip_tests;
#[cfg(test)]
#[path = "storage/wildlife_tests.rs"]
mod wildlife_tests;
#[cfg(test)]
#[path = "storage/difficulty_tests.rs"]
mod difficulty_tests;
#[cfg(test)]
#[path = "storage/recovery_tests.rs"]
mod recovery_tests;
#[cfg(test)]
#[path = "storage/range_tests.rs"]
mod range_tests;
#[cfg(test)]
#[path = "storage/steam_tests.rs"]
mod steam_tests;
#[cfg(test)]
#[path = "storage/boiler_tests.rs"]
mod boiler_tests;
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
/// Version 35 retains lab stock, manufacturing batches and powered wafer clocks.
/// Version 36 persists production orders and their bounded sand feed bins.
/// Version 37 appends passive wildlife identities, health and positions.
/// Version 38 adds persistent Ossuary Engine mechanisms, stock and salvage.
/// Version 39 persists orbital freight momentum, cargo and elastic tether modes.
/// Version 40 appends the world difficulty word; older saves load as Normal.
/// Version 41 retains whole-block machine buffers and recovery checkpoints.
/// Version 43 appends smeltery component and metal material IDs; record layout is unchanged.
/// Version 42 stores the offhand and persistent cache slot inventories.
/// Version 45 adds inventory slot protection and retained quantities.
/// Version 46 retains the Manyfold counterweight latch (older saves are unlatched).
/// Version 47 appends one face-role word per machine. Older saves are both on every face.
/// Version 48 appends bamboo, wheat, carrot, beetroot and crop shoot blocks and four foods (bread,
/// root stew, cooked fish, harvest cake). Record layout is unchanged; older saves load as before.
/// Version 49 retains the wearer's burner fuel and induction-pack charge.
/// Version 50 appends four potions (healing, resistance, leaping, haste) and four status effects
/// (resistance, absorption, haste, jump boost). Record layout is unchanged.
/// Version 51 appends an optional, validated planet recipe to the session footer.
/// Version 52 retains industrial air, deposition and machine gas/waste stocks.
/// Version 53 retains voxel habitat identity, construction, gas and airlocks.
/// Version 54 retains general station cargo, dispatch clocks and security robots.
/// Version 58 expands the factory block buffers for quarry output.
/// Version 59 adds the standalone quarry and its excavation direction.
/// Version 60 checkpoints block inventories with the world and factory buffers.
/// Version 61 adds paydirt and gold to the factory block buffers.
/// Version 62 appends 36 flora blocks (kelp through iron fern). Record layout is unchanged.
/// Version 63 adds gold ingots to the resource buffers.
/// Version 64 adds five distinct ore minerals to the factory block buffers.
/// Version 66 adds feathers to the resource buffers (factory format 20).
/// Version 67 adds oxygen and configured machine vents (factory format 21).
/// Version 68 checkpoints the metered dam and mill with voxel edits and inventory.
/// It also retains Planet flight motion and foreign-body anchors.
/// Version 69 retains the planet's civilization choice. Older planets remain living worlds.
/// Version 70 appends the crop harvester block. Record layout is unchanged.
/// Version 71 commits residents, activities and travel with their physical inventories.
/// Version 72 appends the dungeon Locked Door and Great Door blocks. Record layout is unchanged.
/// Version 73 retains the planet's chosen history length. Older planets keep the generator's own.
/// Version 74 appends Lightless stone, foxfire moss and the Lightless shaft, and world type code 7
/// (the Lightless realm). Record layout is unchanged; older saves load as before.
/// Version 75 appends the fourteen Gilded Cave blocks (gilded gate through sunstone) and world
/// type code 8 (the Gilded Cave realm). Record layout is unchanged; older saves load as before.
pub(crate) const SAVE_VERSION: u8 = 75;
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
/// Offset of the region identity, shared by the library and region collector.
fn identity_offset(prefix: &[u8], version: u8) -> Result<usize> {
    let word = |offset: usize| -> Result<u32> {
        Ok(u32::from_le_bytes(prefix.get(offset..offset + 4).context("Truncated world header")?.try_into()?))
    };
    let kind = word(16)?;
    let mut offset = if version >= 12 { 176 } else { 164 };
    if kind == 5 { offset += 40; }
    if version >= 49 { offset += 8; }
    if version >= 55 && kind == 3 && engine_voxel::WorldSampleConfig::is_hell_generator(word(8)?) {
        offset += if word(88)? == 0 { 4 } else { 44 };
    }
    if version >= 68 && kind == 4 {
        offset += planet_actor::HEADER_BYTES;
        if word(planet_actor::ANCHOR_TAG_OFFSET)? != 0 {
            offset += planet_actor::ANCHOR_BYTES;
        }
    }
    Ok(offset)
}

impl<'a> Decoder<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.offset.checked_add(len).context("Save offset overflow")?;
        let value = self.bytes.get(self.offset..end).context("Truncated save")?;
        self.offset = end;
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
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
pub(crate) mod session;
mod planet_actor;
mod library;
pub use background::{
    SaveService, autosave_enabled, autosave_seconds, set_autosave_enabled, set_autosave_seconds,
};
pub use library::{WorldEntry, WorldLibrary, load_named, name_world};

fn session_header(world: &World, player: &Player, day: &DayNightCycle) -> Result<Vec<u8>> {
    ensure!(world.ca_view.is_none(), "Experimental CPU region requires an exact checkpoint format; ordinary world save refused");
    session_identity(world, player, day, SAVE_VERSION)
}

/// Largest player coordinate a save accepts, in voxels.
/// Planets span six faces of `PITCH_M` each, so their range grows with finer voxels.
fn max_player_coordinate(kind: WorldType, scale: VoxelScalePreset) -> f32 {
    match kind {
        WorldType::Planet => (6.0 * atlas::PITCH_M / f64::from(scale.meters())) as f32,
        WorldType::Nether => (6.0 * atlas::PITCH_M / f64::from(scale.meters())).max(1_000_000.0) as f32,
        _ => 1_000_000.0,
    }
}

fn session_identity(world: &World, player: &Player, day: &DayNightCycle, version: u8) -> Result<Vec<u8>> {
    if let Some(settings) = world.planet_settings {
        ensure!(settings.valid() && (world.sample_config().hell_mirror()
            || world.world_type == WorldType::Planet
                && world.generator_version >= engine_voxel::CUSTOM_PLANET_VERSION), "Invalid planet recipe");
    }
    ensure!(
        player.pos.abs().max_element() <= max_player_coordinate(world.world_type, world.voxel_scale),
        "Player position exceeds supported save range"
    );
    ensure!((0.1..=10.0).contains(&player.scale), "Invalid player scale");
    ensure!(
        (1.0..=86400.0).contains(&day.day_length_seconds),
        "Invalid day length"
    );
    let mut bytes = save_magic(version).to_vec();
    push(&mut bytes, world.generator_version);
    push(&mut bytes, world.seed);
    push(
        &mut bytes,
        u32::from(world.world_type.code()),
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
    super::orbital::habitat::persistence::validate(&world.orbital.habitats)?;
    // Terrain tag 5 is new: its actor extension does not alter older world layouts.
    if world.world_type == WorldType::Manyfold {
        ensure!(player.gravity_frame.is_finite() && (player.gravity_frame.length_squared() - 1.0).abs() < 0.001, "Invalid upright frame");
        for v in player.gravity_frame.to_array() { push(&mut bytes, v.to_bits()); }
        ensure!(world.manyfold_weight.valid(), "Invalid Manyfold weight");
        for v in world.manyfold_weight.position.to_array().into_iter().chain(world.manyfold_weight.velocity.to_array()) {
            push(&mut bytes, v.to_bits());
        }
    }
    push(&mut bytes, u32::from(player.flying));
    push(
        &mut bytes,
        u32::from(matches!(player.fly_mode, FlyMode::Noclip)),
    );
    push(&mut bytes, day.day_index);
    push(&mut bytes, day.weather_seed);
    if world.sample_config().is_hell() {
        let anchor = player.anchor;
        if let Some(a) = anchor {
            ensure!(crate::planet::influence::valid_hell_anchor(world, a), "Invalid Hell moon anchor");
        }
        push(&mut bytes, anchor.map_or(0, |a| u32::from(a.body.0)));
        if let Some(a) = anchor {
            for value in a.pos.to_array().into_iter().chain(a.vel.to_array()).chain(a.upright.to_array()) {
                push(&mut bytes, (value as f32).to_bits());
            }
        }
    }
    if version >= 68 && world.world_type == WorldType::Planet {
        planet_actor::encode(player, &mut bytes)?;
    }
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
    ensure!(player.traversal.checkpoint_valid(), "Invalid traversal reservoirs");
    for value in [player.traversal.fuel_seconds, player.traversal.charge_fe] {
        push(&mut bytes, value.to_bits());
    }
    Ok(bytes)
}
fn session_footer(world: &World, save_version: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    let resources = super::factory::resources_in_save(save_version);
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
            67.. => super::factory::FACTORY_FORMAT,
            66 => 20,
            64..=65 => 19,
            63 => 18,
            61..=62 => 17,
            59..=60 => 16,
            58 => 15,
            52.. => 14,
            47.. => 13,
            44.. => 12,
            41.. => 11,
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
        world.adventure.encode_spell_log(&mut bytes, save_version >= 56);
    }
    if save_version >= 23 { world.adventure.encode_boss_series(&mut bytes); }
    if save_version >= 38 { world.adventure.encode_engines(&mut bytes); }
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
    if save_version >= 44 { super::tool_workshop::encode(&world.adventure.tool_bank, &mut bytes).expect("validated tool records"); }
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
    if save_version >= 37 { world.adventure.wildlife.encode(&world.adventure.enemies, &mut bytes); }
    if save_version >= 40 { push(&mut bytes, world.adventure.difficulty.code()); }
    if save_version >= 41 {
        for point in [world.adventure.respawn_point, world.adventure.last_death_pos] {
            push(&mut bytes, u32::from(point.is_some()));
            if let Some(point) = point {
                for value in point.to_array() {
                    push(&mut bytes, value.to_bits());
                }
            }
        }
    }
    if save_version >= 42 { super::containers::encode(&world.caches, &mut bytes); }
    if save_version >= 46 { push(&mut bytes, u32::from(world.manyfold_weight.opened)); }
    if save_version >= 51 {
        push(&mut bytes, u32::from(world.planet_settings.is_some()));
        if let Some(settings) = world.planet_settings {
            for value in settings.values { push(&mut bytes, u32::from(value)); }
            if save_version >= 69 { push(&mut bytes, settings.civilization.code()); }
            if save_version >= 73 { push(&mut bytes, u32::from(settings.history_myr)); }
        }
    }
    if save_version >= 52 { world.industry.encode_versioned(&mut bytes, save_version >= 67); }
    if save_version >= 53 { super::orbital::habitat::persistence::encode(&world.orbital.habitats, &mut bytes); }
    if save_version >= 57 {
        let spawn = world.adventure.spawn_binding;
        push(&mut bytes, u32::from(spawn.bed.is_some()) | (u32::from(spawn.arrival.is_some()) << 1));
        if let Some(bed) = spawn.bed {
            for value in bed.to_array() { push(&mut bytes, value as u32); }
        }
        if let Some(arrival) = spawn.arrival {
            for value in arrival.to_array() { push(&mut bytes, value.to_bits()); }
        }
    }
    if save_version >= 60 {
        let block_data = super::block_data::sidecar::encode(&world.block_data);
        push(&mut bytes, block_data.len() as u32);
        bytes.extend_from_slice(&block_data);
    }
    if save_version >= 68 {
        let mill = world.mill.map(|state| ron::to_string(&state).expect("serializable mill"));
        push(&mut bytes, mill.as_ref().map_or(0, |text| text.len() as u32));
        if let Some(text) = mill { bytes.extend_from_slice(text.as_bytes()); }
    }
    if save_version >= 71 {
        for state in [super::farming::sidecar::encode(world), super::travel::sidecar::encode(&world.travel)] {
            push(&mut bytes, state.len() as u32);
            bytes.extend_from_slice(&state);
        }
    }
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
    load_bytes(&bytes, slot)
}

fn load_bytes(bytes: &[u8], slot: &Path) -> Result<SavedSession> {
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
        (1..=super::CURRENT_GENERATOR_VERSION).contains(&generator_version)
            || (engine_voxel::COASTAL_PLANET_GENERATOR..=engine_voxel::MOBILE_RIVERBED_GENERATOR).contains(&generator_version)
            || generator_version == engine_voxel::CUSTOM_PLANET_VERSION
            || generator_version == 137
            || generator_version == engine_voxel::SEAMLESS_PLANET_GENERATOR
            || generator_version == engine_voxel::HELL_ECOLOGY_GENERATOR
            || generator_version == engine_voxel::HELL_RUINS_GENERATOR
            || generator_version == engine_voxel::HELL_SURVEYED_RUINS_GENERATOR
            || generator_version == engine_voxel::HELL_MIRROR_GENERATOR
            || generator_version == engine_voxel::HELL_BIOME_GENERATOR
            || (engine_voxel::PLANET_FREIGHT_GENERATOR..=engine_voxel::PLANET_FREIGHT_DETOUR_GENERATOR).contains(&generator_version)
            || generator_version == engine_voxel::PLANET_GENERATOR_VERSION
            || (engine_voxel::WATERSHED_EROSION_GENERATOR..=engine_voxel::BRANCHING_WATERSHEDS_GENERATOR).contains(&generator_version)
            || generator_version == engine_voxel::LANDSCAPE_CYCLES_GENERATOR
            || generator_version == engine_voxel::DEEP_TIME_GENERATOR
            || generator_version == engine_voxel::FRACTAL_COAST_GENERATOR
            || generator_version == engine_voxel::PROCESS_COAST_GENERATOR
            || generator_version == engine_voxel::CLEAN_RIVERBED_GENERATOR
            || generator_version == engine_voxel::GROUND_BREAK_GENERATOR
            || generator_version == engine_voxel::LIVING_SURFACE_GENERATOR
            || generator_version == engine_voxel::LOWLAND_RIVER_GENERATOR
            || generator_version == engine_voxel::RIFT_VALLEY_GENERATOR
            || generator_version == engine_voxel::DIKE_SWARM_GENERATOR
            || generator_version == engine_voxel::CARVED_SURFACE_GENERATOR
            || generator_version == engine_voxel::ROCK_RELIEF_GENERATOR
            || generator_version == engine_voxel::FLOODPLAIN_MEANDER_GENERATOR
            || generator_version == engine_worldgen::planetary::reference_relief::VERSION
            || generator_version == engine_worldgen::manyfold::maze::TWISTED
            // First tree-detail planet release stays readable as defaults advance.
            || generator_version == 137,
        "Unsupported terrain generator version"
    );
    let seed = input.u32()?;
    let kind = u8::try_from(input.u32()?).ok().and_then(WorldType::from_code).context("Invalid terrain type")?;
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
        player.pos.abs().max_element() <= max_player_coordinate(kind, scale),
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
    if kind == WorldType::Manyfold {
        player.gravity_frame = glam::Quat::from_array([input.float()?, input.float()?, input.float()?, input.float()?]);
        ensure!((player.gravity_frame.length_squared() - 1.0).abs() < 0.001, "Invalid upright frame");
        world.manyfold_weight.position = Vec3::new(input.float()?, input.float()?, input.float()?);
        world.manyfold_weight.velocity = Vec3::new(input.float()?, input.float()?, input.float()?);
        ensure!(world.manyfold_weight.valid(), "Invalid Manyfold weight");
    }
    player.flying = input.u32()? != 0;
    player.fly_mode = if input.u32()? == 0 {
        FlyMode::Creative
    } else {
        FlyMode::Noclip
    };
    day_night.day_index = input.u32()?;
    day_night.weather_seed = input.u32()?;
    world.orbital.planet_tick = crate::planet::tick_of(&day_night);
    if version >= 55 && world.sample_config().is_hell() {
        let body = input.u32()?;
        if body != 0 {
            let body = engine_space::BodyId(u16::try_from(body)?);
            let pos = glam::DVec3::new(f64::from(input.float()?), f64::from(input.float()?), f64::from(input.float()?));
            let vel = glam::DVec3::new(f64::from(input.float()?), f64::from(input.float()?), f64::from(input.float()?));
            let upright = glam::DQuat::from_xyzw(f64::from(input.float()?), f64::from(input.float()?),
                f64::from(input.float()?), f64::from(input.float()?));
            let a = crate::planet::influence::Anchor { body, pos, vel, upright };
            ensure!(crate::planet::influence::valid_hell_anchor(&world, a), "Invalid Hell moon anchor");
            crate::planet::influence::restore_hell_anchor(&world, &mut player, a);
        }
    }
    if version >= 68 && kind == WorldType::Planet {
        planet_actor::decode(&mut input, &mut player)?;
    }
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
    if version >= 49 {
        player.traversal.fuel_seconds = input.float()?;
        player.traversal.charge_fe = input.float()?;
        ensure!(player.traversal.checkpoint_valid(), "Invalid saved traversal reservoirs");
    }
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
                super::factory::resources_in_save(version)
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
                if version >= 67 {
                    super::factory::FACTORY_FORMAT
                } else if version >= 66 {
                    20
                } else if version >= 64 {
                    19
                } else if version >= 63 {
                    18
                } else if version >= 61 {
                    17
                } else if version >= 59 {
                    16
                } else if version >= 58 {
                    15
                } else if version >= 52 {
                    14
                } else if version >= 47 {
                    13
                } else if version >= 44 {
                    12
                } else if version >= 41 {
                    11
                } else if version >= 21 {
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
                    super::adventure::Adventure::decode_spell_log(&mut || input.u32(), version >= 56)?;
            }
            if version >= 23 { world.adventure.decode_boss_series(&mut || input.u32())?; }
            if version >= 38 { world.adventure.decode_engines(&mut || input.u32())?; }
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
            world.orbital.planet_tick = crate::planet::tick_of(&day_night);
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
            if version >= 44 { world.adventure.tool_bank = super::tool_workshop::decode(&mut || input.u32())?; }
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
            if version >= 37 {
                world.adventure.wildlife = super::adventure::wildlife::State::decode(&mut || input.u32())?;
            }
            if version >= 40 {
                let code = input.u32()?;
                world.adventure.set_difficulty(super::adventure::difficulty::Difficulty::from_code(code));
            }
            if version >= 41 {
                let adventure = &mut world.adventure;
                for target in [&mut adventure.respawn_point, &mut adventure.last_death_pos] {
                    let present = input.u32()?;
                    ensure!(present <= 1, "Invalid recovery checkpoint flag");
                    *target = if present == 1 {
                        let point = Vec3::new(input.float()?, input.float()?, input.float()?);
                        ensure!(point.abs().max_element() <= 1_000_000.0, "Recovery checkpoint out of range");
                        Some(point)
                    } else {
                        None
                    };
                }
            }
            if version >= 42 { world.caches = super::containers::decode(&mut || input.u32(), &world.inventory)?; }
            if version >= 46 {
                let opened = input.u32()?;
                ensure!(opened <= 1, "Invalid counterweight latch");
                world.manyfold_weight.opened = opened != 0;
            }
            if version >= 51 {
                let present = input.u32()?;
                ensure!(present <= 1, "Invalid planet recipe flag");
                if present != 0 {
                    ensure!((kind == WorldType::Planet && generator_version >= engine_voxel::CUSTOM_PLANET_VERSION) || (kind == WorldType::Nether && matches!(generator_version, engine_voxel::HELL_MIRROR_GENERATOR | engine_voxel::HELL_BIOME_GENERATOR)), "Planet recipe on incompatible world");
                    let mut settings = engine_voxel::PlanetSettings::default();
                    for value in &mut settings.values {
                        let raw = input.u32()?;
                        ensure!(raw <= 100, "Planet recipe dial out of range");
                        *value = raw as u8;
                    }
                    if version >= 69 {
                        settings.civilization = engine_voxel::CivilizationMode::from_code(input.u32()?)
                            .context("Invalid civilization mode")?;
                    }
                    if version >= 73 {
                        settings.history_myr = u16::try_from(input.u32()?).ok()
                            .filter(|myr| engine_voxel::PlanetSettings::valid_history(*myr))
                            .context("Planet history length out of range")?;
                    }
                    world.planet_settings = Some(settings);
                }
            }
            if version >= 52 { world.industry = super::industry::State::decode_versioned(&mut || input.u32(), version >= 67)?; }
            if version >= 53 { world.orbital.habitats = super::orbital::habitat::persistence::decode(&mut || input.u32())?; }
            if version >= 57 {
                let flags = input.u32()?;
                ensure!(flags <= 3, "Invalid Hell spawn flags");
                if flags & 1 != 0 {
                    let bed = IVec3::new(input.u32()? as i32, input.u32()? as i32, input.u32()? as i32);
                    ensure!(bed.to_array().iter().all(|v| v.unsigned_abs() <= 1_000_000), "Hell bed out of range");
                    world.adventure.spawn_binding.bed = Some(bed);
                }
                if flags & 2 != 0 {
                    let arrival = Vec3::new(input.float()?, input.float()?, input.float()?);
                    ensure!(arrival.abs().max_element() <= 1_000_000.0, "Hell arrival out of range");
                    world.adventure.spawn_binding.arrival = Some(arrival);
                }
            }
            if version >= 60 {
                let length = input.u32()? as usize;
                world.block_data = super::block_data::sidecar::decode(input.take(length)?)?;
            }
            if version >= 68 {
                let length = input.u32()? as usize;
                ensure!(length <= 512, "Oversized mill record");
                if length != 0 {
                    let state: super::mill::Mill = ron::de::from_bytes(input.take(length)?)?;
                    ensure!(state.valid(&world), "Invalid mill record");
                    world.mill = Some(state);
                }
            }
            if version >= 71 {
                let length = input.u32()? as usize;
                ensure!(length <= 48 * 1024 * 1024, "Oversized resident checkpoint");
                let (farming, herd) = super::farming::sidecar::decode(input.take(length)?)?;
                let clock = farming.clock;
                world.farming = farming;
                super::farming::sidecar::apply_herd(&mut world, herd, clock)?;
                let length = input.u32()? as usize;
                ensure!(length <= 8 * 1024 * 1024, "Oversized travel checkpoint");
                world.travel = super::travel::sidecar::decode(input.take(length)?)?;
            }
            let bank = world.adventure.wand_bank.len();
            let inv = &world.inventory;
            ensure!(
                inv.hotbar.iter().chain(inv.pack.iter()).chain(inv.craft_grid.iter()).chain(std::iter::once(&inv.cursor)).chain(std::iter::once(&inv.offhand)).flatten().map(|s| s.item)
                    .chain(world.caches.iter().flat_map(|c| c.slots.iter().flatten().map(|s| s.item)))
                    .chain(world.orbital.habitats.cargo.stations.iter().filter_map(|s| s.shipment.as_ref())
                        .flat_map(|q| q.items.iter().map(|s| s.item)))
                    .chain(inv.dropped.iter().map(|d| d.stack.item))
                    .all(|item| !matches!(item, super::inventory::Item::Wand { id, .. } if id as usize >= bank)),
                "Wand item without a wand record"
            );
            // The rules sidecar is loaded below; preserve the saved preset until then.
            world.rules.stacks = world.inventory.balance;
            world.sync_from_inventory();
        } else {
            world.migrate_legacy_save_to_inventory();
        }
        if world.ensure_starter_wand() {
            // Pre-wand saves could load without any discoverable route to
            // spellcasting. Grant the same Lumen Twig a fresh world receives.
            world.expedition.say("Lumen Twig Added");
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
        // A damaged rules sidecar must not block opening the world it sits in.
        world.rules = super::rules::Rules::load(&root).unwrap_or_else(|e| {
            eprintln!("[rules] Using default rules: {e:#}");
            super::rules::Rules::default()
        });
        world.rules.systems.publish();
        if version < 71 { super::farming::sidecar::load(&mut world, &root); }
        if version < 60 {
            super::block_data::sidecar::load(&mut world.block_data, &root);
        }
        if world.inventory.balance != world.rules.stacks { world.inventory.migrate_balance(world.rules.stacks); }
        if version < 71 { super::travel::sidecar::load(&mut world.travel, &root); }
        super::progression::sidecar::load(&mut world, &root);
        super::stats::sidecar::load(&mut world, &root);
        super::goals::sidecar::load(&mut world.goals, &root);
        super::portals::sidecar::load(&mut world, &root);
        super::lightless_life::load(&mut world, &root);
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
        v17_header.truncate(v17_header.len() - 8);
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
        v12_header.truncate(v12_header.len() - 8);
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
    pub(super) fn synth_save(world: &World, version: u8, dir: &Path, name: &str) -> PathBuf {
        let player = Player::new();
        let day_night = DayNightCycle::default();
        let mut bytes = Vec::with_capacity(512);
        let mut header = session_identity(world, &player, &day_night, version).unwrap();
        if version < 49 { header.truncate(header.len() - 8); }
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
    fn version_66_keeps_feathers_and_a_v65_layout_still_decodes() {
        use crate::world::factory;
        let dir = std::env::temp_dir().join(format!("voxel_v66_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let world = |feathers: u32| {
            let mut w = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
            w.adventure.begin_journey();
            w.expedition.resources[factory::GOLD_INGOT] = 2 * 64;
            w.expedition.resources[factory::FEATHERS] = feathers * 64;
            w.reconcile_legacy_to_inventory();
            w
        };
        let current = synth_save(&world(3), SAVE_VERSION, &dir, "world_v66.dat");
        let loaded = load_exact(&current, &current).unwrap();
        assert_eq!(loaded.world.inventory.count_item(Item::Resource(factory::FEATHERS as u8)), 3);
        let legacy = synth_save(&world(0), 65, &dir, "world_v65.dat");
        let old = load_exact(&legacy, &legacy).unwrap();
        assert_eq!(old.world.inventory.count_item(Item::Resource(factory::GOLD_INGOT as u8)), 2);
        assert_eq!(old.world.inventory.count_item(Item::Resource(factory::FEATHERS as u8)), 0);
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
    #[test]
    fn crop_items_round_trip_at_v48_and_a_v47_save_keeps_its_food() {
        use super::super::inventory::FoodKind;
        let dir = std::env::temp_dir().join(format!("voxel_v48_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        w.inventory = super::super::inventory::SurvivalInventory::empty();
        w.inventory.pack[0] = Some(ItemStack::new(Item::Food(FoodKind::Bread), 5));
        w.inventory.pack[1] = Some(ItemStack::new(Item::Block(BlockId::WheatCrop), 3));
        w.inventory.pack[2] = Some(ItemStack::new(Item::Food(FoodKind::HarvestCake), 2));
        w.inventory.pack[3] = Some(ItemStack::new(Item::Block(BlockId::CropShoot), 1));
        let path = synth_save(&w, SAVE_VERSION, &dir, "world_v48.dat");
        let back = load_exact(&path, &path).unwrap().world;
        assert_eq!(back.inventory.pack[..4], w.inventory.pack[..4]);
        let mut old = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        old.inventory = super::super::inventory::SurvivalInventory::empty();
        old.inventory.pack[0] = Some(ItemStack::new(Item::Food(FoodKind::RawFish), 4));
        old.inventory.pack[1] = Some(ItemStack::new(Item::Block(BlockId::BambooShoot), 2));
        let path = synth_save(&old, 47, &dir, "world_v47.dat");
        let back = load_exact(&path, &path).unwrap().world;
        assert_eq!(back.inventory.pack[..2], old.inventory.pack[..2]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
#[path = "storage/geology_cache_tests.rs"]
mod geology_cache_tests;


#[cfg(test)]
#[path = "storage/relief_cache_tests.rs"]
mod relief_cache_tests;
