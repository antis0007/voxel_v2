//! Gameplay images as the sections that changed since the last one sent.
//!
//! Reliable ordered fragments carry one immutable terrain/gameplay frame.
//! The next frame waits for its publication acknowledgement; reconnect starts
//! a new `Mirror` with a complete first image. Sections are resent when their
//! bytes change or after `refresh`. Water goes as changed and drained cells.
use super::*;
use crate::world::adventure::Adventure;
use engine_netcode::wire::{Reader, put_u64};
use std::io::Write;
mod water;
mod industry;
#[cfg(test)]
mod industry_tests;
pub(super) mod terrain;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod assembler_tests;
#[cfg(test)]
mod furnace_tests;
#[cfg(test)]
mod crusher_tests;
#[cfg(test)]
mod chest_cache_tests;
#[cfg(test)]
mod turbine_tests;
#[cfg(test)]
mod boiler_tests;

const VERSION: u8 = 12;
const HEADER: u8 = 0;
const BODY: u8 = 1;
const ADVENTURE: u8 = 2;
const INVENTORY: u8 = 3;
const EXPEDITION: u8 = 4;
const FACTORY: u8 = 5;
const PARTS: u8 = 6;
const PART_COUNT: usize = 11;
const WATER: u8 = PARTS + PART_COUNT as u8;
const WATER_FULL: u8 = WATER + 1;
const RESERVOIRS: u8 = WATER_FULL + 1;
const FACTORY_SELECTION: u8 = RESERVOIRS + 1;
const RIGID: u8 = FACTORY_SELECTION + 1;
const HELL_GATES: u8 = RIGID + 1;
const PROGRESSION: u8 = HELL_GATES + 1;
const TRAVEL_STATE: u8 = PROGRESSION + 1;
const TERRAIN: u8 = TRAVEL_STATE + 1;
const INDUSTRY: u8 = TERRAIN + 1;
/// Joiner to host: its water disagrees with the last digest; send a full list.
pub const RESEND_WATER: u8 = 24;
/// Longest a section goes without a resend, and the shortest gap between full water lists.
pub const REFRESH: std::time::Duration = std::time::Duration::from_secs(2);
/// Reliable ordered transport does not need a complete gameplay resend at 0.5 Hz.
pub const FULL_REFRESH: std::time::Duration = std::time::Duration::from_secs(30);

/// Shared factory state changes on the automation clock. Reuse one encoded
/// image across peers; focus and bench are private to the mounted player.
#[derive(Default)]
pub struct FactoryImage {
    text: String,
    digest: u64,
    due: Option<std::time::Instant>,
    storage_revision: Option<u64>,
    terrain_revision: Option<u64>,
    gates: Vec<world::hell_gate::built::Threshold>,
    gate_text: String,
    gate_digest: u64,
    industry: std::sync::Arc<industry::Image>,
}

impl FactoryImage {
    pub fn invalidate(&mut self) { self.due = None; }
    pub fn refresh(&mut self, w: &mut World, now: std::time::Instant) -> Result<()> {
        if self.gate_text.is_empty() || self.gates != w.portals.hell {
            self.gate_text = ron::to_string(&w.portals.hell)?;
            self.gate_digest = fnv(self.gate_text.as_bytes());
            self.gates.clone_from(&w.portals.hell);
        }
        // Chest and machine inventories must describe the same transfer.
        if self.due.is_some_and(|due| now < due)
            && self.storage_revision == Some(w.block_data.storage_revision)
            && self.terrain_revision == Some(w.revision()) { return Ok(()); }
        let focus = w.factory.focus.take();
        let bench = w.factory.bench.take();
        let text = ron::to_string(&w.factory);
        w.factory.focus = focus;
        w.factory.bench = bench;
        self.text = text?;
        self.digest = fnv(self.text.as_bytes());
        self.industry = std::sync::Arc::new(industry::Image::capture(&w.industry));
        self.storage_revision = Some(w.block_data.storage_revision);
        self.terrain_revision = Some(w.revision());
        self.due = Some(now + std::time::Duration::from_millis(250));
        Ok(())
    }
}

/// Last encoded sections, including any frame awaiting publication.
#[derive(Default)]
pub struct Mirror {
    sent: [Option<u64>; TRAVEL_STATE as usize + 1],
    water: water::Sent,
    industry: industry::Sent,
    bulk_due: Option<std::time::Instant>,
    inventory: Option<(world::inventory::SurvivalInventory, String, u64)>,
}

impl Mirror {
    /// Resends every section except water with the next image.
    pub fn refresh(&mut self) {
        self.sent.fill(None);
        self.bulk_due = None;
        self.industry = industry::Sent::default();
    }

    pub fn resend_water(&mut self) {
        self.water.reset();
    }
}

/// Builds the next image for one joiner and records it as sent.
#[cfg(test)]
fn capture(w: &World, body: &Player, day: &DayNightCycle, command_ack: u64, others: Vec<&Adventure>, mirror: &mut Mirror) -> Result<Vec<u8>> {
    let text = ron::to_string(&w.factory)?;
    let digest = fnv(text.as_bytes());
    let gate_text = ron::to_string(&w.portals.hell)?;
    let gate_digest = fnv(gate_text.as_bytes());
    capture_shared(w, body, day, command_ack, others, mirror, &FactoryImage {
        text, digest, gate_text, gate_digest,
        industry: std::sync::Arc::new(industry::Image::capture(&w.industry)), ..Default::default()
    })
}

pub fn capture_shared(w: &World, body: &Player, day: &DayNightCycle, command_ack: u64, others: Vec<&Adventure>, mirror: &mut Mirror, factory: &FactoryImage) -> Result<Vec<u8>> {
    capture_frame(w, body, day, command_ack, others, mirror, factory, 0, &terrain::Frame::default())
}

pub fn capture_frame(w: &World, body: &Player, day: &DayNightCycle, command_ack: u64, others: Vec<&Adventure>, mirror: &mut Mirror, factory: &FactoryImage, sequence: u64, terrain: &terrain::Frame) -> Result<Vec<u8>> {
    let started = std::time::Instant::now();
    let mut out = vec![VERSION];
    put_u64(&mut out, command_ack);
    put_u64(&mut out, sequence);
    section(&mut out, HEADER, ron::to_string(&(day, w.manyfold_weight, w.rules.encode()))?.as_bytes());
    if mirror.inventory.as_ref().is_none_or(|(previous, _, _)| previous != &w.inventory) {
        let text = ron::to_string(&w.inventory)?;
        let digest = fnv(text.as_bytes());
        mirror.inventory = Some((w.inventory.clone(), text, digest));
    }
    let (_, inventory, inventory_digest) = mirror.inventory.as_ref().expect("inventory cached");
    let texts = [
        // The joiner owns its viewmodel; do not resend animated state it ignores.
        ron::to_string(&(body.scale, crate::player::viewmodel::State::default()))?,
        ron::to_string(&(&w.adventure, &others))?,
    ];
    let expedition = ron::to_string(&w.expedition)?;
    let mut parts = vec![Vec::new(); PART_COUNT];
    let now = std::time::Instant::now();
    let bulk_due = mirror.bulk_due.is_none_or(|due| now >= due);
    w.factory.chips.encode(&mut parts[0]);
    w.undercrown.encode(&mut parts[1]);
    if bulk_due {
        w.sandbox.encode_thermal_image(&mut parts[2]);
        w.orbital.encode(&mut parts[3], world::storage::SAVE_VERSION);
        w.habitats.encode_versioned(&mut parts[4], world::storage::SAVE_VERSION);
        w.activity.snapshot().encode(&mut parts[6]);
        mirror.bulk_due = Some(now + std::time::Duration::from_secs(1));
    }
    // Keep exact water alongside the coarse water update to avoid stale film heights.
    w.environment.encode(&mut parts[5]);
    parts[7] = world::block_data::sidecar::encode(&w.block_data);
    world::containers::encode(&w.caches, &mut parts[8]);
    parts[9] = world::farming::sidecar::encode(w);
    parts[10] = world::travel::sidecar::encode(&w.travel);
    let bodies = texts.iter().map(String::as_bytes)
        .chain([inventory.as_bytes(), expedition.as_bytes()])
        .chain(std::iter::once(factory.text.as_bytes()))
        .chain(parts.iter().map(Vec::as_slice));
    for (tag, body) in (BODY..).zip(bodies) {
        if !bulk_due && matches!(tag, t if t == PARTS+2 || t == PARTS+3 || t == PARTS+4 || t == PARTS+6) {
            continue;
        }
        let digest = match tag {
            FACTORY => factory.digest,
            INVENTORY => *inventory_digest,
            _ => fnv(body),
        };
        let sent = &mut mirror.sent[usize::from(tag)];
        if *sent != Some(digest) {
            section(&mut out, tag, body);
            *sent = Some(digest);
        }
    }
    let mut water = Vec::new();
    let full = mirror.water.write(&w.water_levels, &mut water);
    section(&mut out, if full { WATER_FULL } else { WATER }, &water);
    section(&mut out, RESERVOIRS, ron::to_string(&(body.traversal.fuel_seconds, body.traversal.charge_fe))?.as_bytes());
    let selection = ron::to_string(&(w.factory.focus, w.factory.bench))?;
    let digest = fnv(selection.as_bytes());
    if mirror.sent[FACTORY_SELECTION as usize] != Some(digest) {
        section(&mut out, FACTORY_SELECTION, selection.as_bytes());
        mirror.sent[FACTORY_SELECTION as usize] = Some(digest);
    }
    let mut rigid = Vec::new();
    w.sandbox.encode_rigid_image(&mut rigid, body.pos, 96.0 / w.voxel_scale.meters());
    let digest = fnv(&rigid);
    if mirror.sent[RIGID as usize] != Some(digest) {
        section(&mut out, RIGID, &rigid);
        mirror.sent[RIGID as usize] = Some(digest);
    }
    if mirror.sent[HELL_GATES as usize] != Some(factory.gate_digest) {
        section(&mut out, HELL_GATES, factory.gate_text.as_bytes());
        mirror.sent[HELL_GATES as usize] = Some(factory.gate_digest);
    }
    let progression = world::progression::sidecar::encode(&w.progression, &w.inventory);
    let digest = fnv(&progression);
    if mirror.sent[PROGRESSION as usize] != Some(digest) {
        section(&mut out, PROGRESSION, &progression);
        mirror.sent[PROGRESSION as usize] = Some(digest);
    }
    let travel = ron::to_string(&(
        w.travel.riding, w.travel.cast, w.travel.last_colour, w.travel.following,
    ))?;
    let digest = fnv(travel.as_bytes());
    if mirror.sent[TRAVEL_STATE as usize] != Some(digest) {
        section(&mut out, TRAVEL_STATE, travel.as_bytes());
        mirror.sent[TRAVEL_STATE as usize] = Some(digest);
    }
    section(&mut out, TERRAIN, &terrain.encode()?);
    if let Some(bytes) = mirror.industry.write(&factory.industry) {
        section(&mut out, INDUSTRY, &bytes);
    }
    ensure!(out.len() <= gameplay::MAX_IMAGE, "gameplay image exceeds budget");
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&out)?;
    let bytes = encoder.finish()?;
    ensure!(bytes.len() <= engine_net::transfer::MAX_TRANSFER, "compressed image exceeds budget");
    if std::env::var_os("VOXEL_NETWORK_PROFILE").is_some() {
        eprintln!("[image-timing] capture_ms={:.2} raw={} compressed={}", started.elapsed().as_secs_f64()*1000.0, out.len(), bytes.len());
    }
    Ok(bytes)
}

pub struct Applied {
    pub command_ack: u64,
    /// The joiner's water no longer matches the host's; send `RESEND_WATER`.
    pub water_mismatch: bool,
}

type Publish = Box<dyn FnOnce(&mut World, &mut Player, &mut DayNightCycle, &mut HashSet<IVec3>, &mut HashSet<IVec3>) -> Applied>;

pub struct Prepared {
    pub sequence: u64,
    pub command_ack: u64,
    pub terrain: terrain::Frame,
    pub publish: Publish,
}

/// Applies an image that contains no terrain transaction.
pub fn apply(bytes: &[u8], w: &mut World, p: &mut Player, day: &mut DayNightCycle, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> Result<Applied> {
    let prepared = prepare(bytes, w)?;
    ensure!(prepared.terrain.is_empty(), "terrain image requires staged publication");
    Ok((prepared.publish)(w, p, day, blocks, chunks))
}

/// Validates an entire image before changing the visible world.
pub fn prepare(bytes: &[u8], w: &World) -> Result<Prepared> {
    ensure!(bytes.len() <= gameplay::MAX_IMAGE, "oversized gameplay image");
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(bytes).take(gameplay::MAX_IMAGE as u64 + 1).read_to_end(&mut raw)?;
    ensure!(raw.len() <= gameplay::MAX_IMAGE, "expanded gameplay image exceeds budget");
    let mut r = Reader::new(&raw);
    ensure!(wire(r.u8())? == VERSION, "gameplay image version mismatch");
    let command_ack = wire(r.u64())?;
    let sequence = wire(r.u64())?;
    let mut bodies: [Option<&[u8]>; INDUSTRY as usize + 1] = Default::default();
    let mut last = None;
    while !r.done() {
        let tag = wire(r.u8())?;
        ensure!(tag <= INDUSTRY && last.is_none_or(|l| tag > l), "gameplay sections out of order");
        let len = usize::try_from(wire(r.u64())?)?;
        bodies[usize::from(tag)] = Some(wire(r.take(len))?);
        last = Some(tag);
    }
    let body = |tag: u8| bodies[usize::from(tag)];
    let header = body(HEADER).context("gameplay image has no header")?;
    let (next_day, weight, rules): (DayNightCycle, crate::manyfold::Weight, Vec<u8>) = ron::de::from_bytes(header)?;
    let rules = world::rules::Rules::decode(&rules)?;
    ensure!(weight.valid(), "Invalid Manyfold weight");
    let shown: Option<(f32, crate::player::viewmodel::State)> = body(BODY).map(ron::de::from_bytes).transpose()?;
    let reservoirs: Option<(f32,f32)> = body(RESERVOIRS).map(ron::de::from_bytes).transpose()?;
    if let Some((fuel,charge)) = reservoirs {
        ensure!((0.0..=crate::player::traversal::BURN_SECONDS).contains(&fuel)
            && (0.0..=crate::player::traversal::CHARGE_CAPACITY).contains(&charge), "Invalid traversal reservoirs");
    }
    let water = match (body(WATER), body(WATER_FULL)) {
        (Some(b), None) => water::Update::read(b, false)?,
        (None, Some(b)) => water::Update::read(b, true)?,
        _ => anyhow::bail!("gameplay image needs one water section"),
    };
    let adventure: Option<(Adventure, Vec<Adventure>)> = body(ADVENTURE).map(ron::de::from_bytes).transpose()?;
    if let Some((a, others)) = &adventure {
        ensure!(others.len() <= 16 && a.enemies.len() <= 512 && a.projectiles.len() <= 4096, "gameplay entity budget exceeded");
    }
    let inventory: Option<world::inventory::SurvivalInventory> = body(INVENTORY).map(ron::de::from_bytes).transpose()?;
    ensure!(inventory.as_ref().is_none_or(|i| i.dropped.len() <= 1024), "gameplay entity budget exceeded");
    let expedition: Option<world::expedition::Expedition> = body(EXPEDITION).map(ron::de::from_bytes).transpose()?;
    let factory: Option<world::factory::Factory> = body(FACTORY).map(ron::de::from_bytes).transpose()?;
    let selection: Option<(Option<IVec3>, Option<IVec3>)> = body(FACTORY_SELECTION).map(ron::de::from_bytes).transpose()?;
    ensure!(factory.as_ref().is_none_or(|f| f.nodes.len() <= 8192), "gameplay entity budget exceeded");
    let part = |i: u8| body(PARTS + i);
    let chips = part(0).map(|b| words(b, |r| world::factory::chips::Bay::decode(&mut || r()))).transpose()?;
    let undercrown = part(1).map(|b| words(b, |r| world::undercrown::State::decode(&mut || r()))).transpose()?;
    let sandbox = part(2).map(|b| words(b, |r| crate::simulation::sandbox::Sandbox::decode(&mut || r()))).transpose()?;
    let rigid = body(RIGID).map(|b| words(b, |r| crate::simulation::sandbox::Sandbox::decode(&mut || r()))).transpose()?;
    let orbital = part(3).map(|b| words(b, |r| world::orbital::State::decode(&mut || r(), world::storage::SAVE_VERSION))).transpose()?;
    let habitats = part(4).map(|b| words(b, |r| world::habitats::State::decode(&mut || r(), world::storage::SAVE_VERSION))).transpose()?;
    let environment = part(5).map(|b| words(b, |r| world::environment::State::decode(&mut || r(), w.voxel_scale))).transpose()?;
    let activity = part(6).map(|b| words(b, |r| world::activity::Activity::decode(&mut || r(), true))).transpose()?;
    let storage = part(7).map(world::block_data::sidecar::decode).transpose()?;
    let caches = part(8).map(|b| words(b, |r| world::containers::decode(&mut || r(), inventory.as_ref().unwrap_or(&w.inventory)))).transpose()?;
    let farming = part(9).map(world::farming::sidecar::decode).transpose()?.map(|(state, _)| state);
    let travel = part(10).map(world::travel::sidecar::decode).transpose()?;
    let gates: Option<Vec<world::hell_gate::built::Threshold>> = body(HELL_GATES).map(ron::de::from_bytes).transpose()?;
    let progression = body(PROGRESSION).map(world::progression::sidecar::decode).transpose()?;
    let travel_state: Option<(Option<world::travel::Ride>, Option<world::travel::fishing::Cast>, Option<u32>, Option<u32>)> =
        body(TRAVEL_STATE).map(ron::de::from_bytes).transpose()?;
    if let Some(gates) = &gates {
        realm::validate_gates(gates, w.voxel_scale)?;
    }

    let industry = body(INDUSTRY).map(|b| industry::read(b, &w.industry)).transpose()?;
    let terrain = terrain::Frame::decode(body(TERRAIN).context("gameplay image has no terrain frame")?)?;
    let publish: Publish = Box::new(move |w, p, day, blocks, chunks| {
    if let Some((a, others)) = adventure {
        // The displayed clock runs between images. Do not rewind an ongoing
        // animation when a slightly older authoritative image finishes arriving.
        // Bound extrapolation and handle the adventure clock's 10000s wrap.
        let visual_lead = (w.adventure.time - a.time + 5000.0).rem_euclid(10000.0) - 5000.0;
        let mut previous = std::mem::replace(&mut w.adventure, a);
        w.adventure.location = std::mem::take(&mut previous.location);
        for other in others {
            w.adventure.merge_multiplayer_visuals(other);
        }
        w.adventure.advance_multiplayer_visuals(visual_lead.clamp(0.0, 0.25));
        w.adventure.smooth_multiplayer_actors(&previous);
    }
    if let Some(i) = inventory {
        w.inventory = i;
    }
    if let Some(mut progression) = progression {
        for effect in &mut progression.effects.list {
            if let Some(previous) = w.progression.effects.list.iter().find(|previous|
                previous.kind == effect.kind && previous.level == effect.level && previous.seconds >= effect.seconds)
            {
                effect.total = previous.total;
            }
        }
        w.progression = progression;
    }
    world::progression::study::restore_spells(w);
    w.rules = rules;
    if let Some(mut state) = storage {
        state.storage_revision = w.block_data.storage_revision.wrapping_add(1);
        w.block_data = state;
    }
    if let Some(caches) = caches { w.caches = caches; }
    if let Some(farming) = farming { w.farming = farming; }
    if let Some(mut travel) = travel {
        travel.riding = w.travel.riding;
        travel.cast = w.travel.cast;
        travel.last_colour = w.travel.last_colour;
        travel.following = w.travel.following;
        w.travel = travel;
    }
    if let Some((ride, cast, last_colour, following)) = travel_state {
        w.travel.riding = ride;
        w.travel.cast = cast;
        w.travel.last_colour = last_colour;
        w.travel.following = following;
    }
    if let Some(gates) = gates { w.portals.hell = gates; }
    if let Some(e) = expedition {
        w.expedition = e;
    }
    if let Some(f) = factory {
        // Chips travel in their own section.
        let old = std::mem::replace(&mut w.factory, f);
        w.factory.chips = old.chips;
        w.factory.focus = old.focus;
        w.factory.bench = old.bench;
    }
    if let Some((focus, bench)) = selection {
        w.factory.focus = focus;
        w.factory.bench = bench;
    }
    if let Some(v) = chips {
        w.factory.chips = v;
    }
    if let Some(v) = undercrown {
        w.undercrown = v;
    }
    if let Some(mut v) = sandbox {
        v.bodies = std::mem::take(&mut w.sandbox.bodies);
        w.sandbox = v;
    }
    if let Some(v) = rigid { w.sandbox.bodies = v.bodies; }
    if let Some(v) = orbital {
        w.orbital = v;
    }
    if let Some(v) = habitats {
        w.habitats = v;
    }
    if let Some(v) = environment {
        // Coarse levels may be unchanged when an exact-volume film moves.
        for (key, _) in v.water_deficit.iter().chain(w.environment.water_deficit.iter()) {
            if v.water_deficit.get(key) != w.environment.water_deficit.get(key) {
                let p = IVec3::new(key.0, key.1, key.2);
                blocks.insert(p);
                world::mark_chunk_and_neighbors_dirty(chunks, p);
            }
        }
        w.environment = v;
    }
    if let Some(v) = industry { w.industry = v; }
    if let Some(v) = activity {
        w.activity = v;
    }
    let digest = water.digest;
    water.publish(&mut w.water_levels, blocks, chunks);
    // Movement and viewmodel state stay with the joiner, which steps both locally.
    if let Some((scale, _viewmodel)) = shown {
        p.scale = scale;
    }
    if let Some((fuel,charge)) = reservoirs {
        p.traversal.fuel_seconds=fuel;
        p.traversal.charge_fe=charge;
    }
    *day = next_day;
    w.manyfold_weight = weight;
    w.sync_from_inventory();
    Applied { command_ack, water_mismatch: water::digest(&w.water_levels) != digest }
    });
    Ok(Prepared { sequence, command_ack, terrain, publish })
}

fn section(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(tag);
    put_u64(out, body.len() as u64);
    out.extend_from_slice(body);
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

fn words<T>(bytes: &[u8], f: impl FnOnce(&mut dyn FnMut() -> Result<u32>) -> Result<T>) -> Result<T> {
    let mut offset = 0;
    let mut next = || {
        let slice = bytes.get(offset..offset + 4).context("truncated gameplay component")?;
        offset += 4;
        Ok(u32::from_le_bytes(slice.try_into()?))
    };
    let value = f(&mut next)?;
    ensure!(offset == bytes.len(), "trailing gameplay component data");
    Ok(value)
}

fn wire<T>(r: Result<T, engine_netcode::wire::WireError>) -> Result<T> {
    r.map_err(|e| anyhow::anyhow!(e.0))
}
