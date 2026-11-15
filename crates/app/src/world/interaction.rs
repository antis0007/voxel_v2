//! Shared diegetic interaction prompt. Reliquaries and world mechanisms both
//! use this shape so the HUD has one prompt path and one visual language.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Prompt {
    #[cfg_attr(feature="multiplayer", serde(deserialize_with="crate::multiplayer::text::string"))]

    pub title: crate::world::interaction::Text,
#[cfg_attr(feature="multiplayer", serde(deserialize_with="crate::multiplayer::text::string"))]
    pub action: crate::world::interaction::Text,
}

/// Owned wire decoding uses a bounded interner; source prompts remain static.
pub type Text = &'static str;

use super::{World, BlockId};
use crate::player::Player;
use glam::{IVec3, Vec3};
use std::collections::HashSet;

pub const REACH: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target { Block(IVec3), Mimic(IVec3, u32), Relic, Resident(u32), ConvoyDriver(u32), StrandedCargo, Traveller }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Taken,
    Block(IVec3),
    Inventory(u32),
    ThresholdGuide,
    Trade(u32),
}

pub fn gameplay_enabled(focused: bool, grabbed: bool, menu: bool, text: bool) -> bool {
    focused && grabbed && !menu && !text
}

pub fn block_interactable(b: BlockId) -> bool {
    b == BlockId::Workbench || b == BlockId::Bed || b == BlockId::Smelter
        || super::factory::Kind::from_block(b).is_some()
        || super::containers::is_cache(b) || super::smeltery::interactive(b)
        || super::block_data::opens_page(b) || engine_content::fittings::state(b).is_some()
        || super::dungeon_quest::is_door(b)
}

/// One camera query shared by the dedicated key and contextual secondary use.
/// First-hit blocks provide occlusion; free-standing objects additionally test sightlines.
pub fn target(world: &World, player: &Player) -> Option<Target> {
    let scale = world.voxel_scale.meters();
    let eye = player.eye(world.voxel_scale);
    let forward = player.forward_dir();
    let hit = super::raycast_block(world, eye, forward, REACH / scale);
    let mut candidates = Vec::with_capacity(3);
    if let Some((cell, front)) = hit.filter(|(p, _)| block_interactable(world.get(*p)) || super::mill::is_wheel(world, *p)) {
        // Distance to the entry face, rather than the voxel's corner or centre.
        let face = (cell.as_vec3() + front.as_vec3() + Vec3::ONE) * 0.5;
        candidates.push(((face - eye).dot(forward).max(0.0) * scale, Target::Block(cell)));
    }
    if let Some(distance) = world.adventure.focused_relic_distance(world, eye * scale, forward) {
        candidates.push((distance, Target::Relic));
    }
    let obstruction = hit.map_or(f32::INFINITY, |(cell, front)| {
        let face = (cell.as_vec3() + front.as_vec3() + Vec3::ONE) * 0.5;
        (face - eye).dot(forward) * scale
    });
    if let Some((distance, site, slot)) = super::adventure::encounters::focused_cache(world, eye * scale, forward, REACH)
        && distance < obstruction
    {
        candidates.push((distance, Target::Mimic(site, slot)));
    }
    if let Some((distance, traveller)) = super::travel::target_distance(world, eye * scale, forward) {
        let target = match traveller {
            super::adventure::wildlife::vehicle::Target::Villager(id) => Target::Resident(id),
            super::adventure::wildlife::vehicle::Target::Driver(id) => Target::ConvoyDriver(id),
            _ => Target::Traveller,
        };
        candidates.push((distance, target));
    }
    if let Some(distance) = super::farming::planet_freight::stranded_distance(world, eye * scale, forward, REACH) {
        candidates.push((distance, Target::StrandedCargo));
    }
    candidates.into_iter().min_by(|a,b| a.0.total_cmp(&b.0)).map(|(_, t)| t)
}

pub fn prompt(world: &World, player: &Player, slot: usize) -> Option<Prompt> {
    if let Some(prompt) = super::orbital::habitat::prompt(world, player) { return Some(prompt); }
    if let Some(prompt) = super::prospecting::prompt(world, player, slot) { return Some(prompt); }
    match target(world, player)? {
        Target::Block(cell) => {
            if super::mill::is_wheel(world, cell) {
                return Some(Prompt { title: "Spillway Mill", action: "[F] Read Mill Ledger" });
            }
            let b = world.get(cell);
            Some(Prompt { title: b.display_name(), action: if b.chest_tier().is_some() || super::containers::is_cache(b) {
                "[F] Open Chest"
            } else if matches!(b, BlockId::MechanicalButton | BlockId::MechanicalButtonOn) { "[F] Press" }
            else if b == BlockId::PressurePlateOn { "[F] Reset" }
            else if b == BlockId::PressurePlate { "Step to activate" }
            else if engine_content::fittings::state(b).is_some() { "[F] Toggle" }
            else if b == BlockId::ClaimStake && super::farming::base::enabled(world) {
                super::farming::base::stake_action(world, cell)
            }
            else if b == BlockId::Bed && super::spawn::bed_binds_spawn(world) { "[F] Set spawn" }
            else { "[F] Use" } })
        }
        Target::Relic => Some(world.adventure.interaction_prompt().unwrap_or(Prompt { title: "Shrine", action: "[F] Activate Shrine" })),
        Target::Mimic(..) => Some(Prompt { title: "Chest", action: "[F] Open Chest" }),
        Target::Resident(id) => Some(Prompt { title: super::travel::trades::name(id), action: "[F] Talk" }),
        Target::ConvoyDriver(id) => Some(Prompt { title: super::farming::planet_freight::driver_name(id)?, action: "[F] Talk" }),
        Target::StrandedCargo => Some(Prompt { title: "Stranded Cargo", action: "[F] Recover Cargo" }),
        Target::Traveller => Some(Prompt { title: "Interact", action: "[F] Talk / Ride" }),
    }
}

pub fn secondary_targets_world(world: &World, player: &Player) -> bool {
    matches!(target(world, player), Some(Target::Block(_) | Target::Mimic(..) | Target::Resident(_) | Target::ConvoyDriver(_) | Target::StrandedCargo | Target::Traveller))
}

/// Dispatch exactly one world action. This API deliberately has no equipment-use callback.
pub fn activate(world: &mut World, player: &Player, slot: usize, sneak: bool,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> Effect {
    if !world.rules.mode.interacts() || world.adventure.health <= 0.0 { return Effect::None; }
    if super::orbital::habitat::interact(world, player) { return Effect::Taken; }
    if sneak && super::prospecting::inspect(world, player, slot) { return Effect::Taken; }
    let scale = world.voxel_scale.meters();
    let eye = player.eye(world.voxel_scale) * scale;
    let forward = player.forward_dir();
    if sneak && let Some((cell, _)) = super::raycast_block(world, eye / scale, forward, REACH / scale)
        && world.get(cell).is_mirage()
    {
        let changed = super::mirage::reveal(world, cell);
        if !changed.is_empty() {
            for p in changed {
                blocks.insert(p);
                super::mark_chunk_and_neighbors_dirty(chunks, p);
            }
            world.adventure.spawn_voxel_burst((cell.as_vec3() + Vec3::splat(0.5)) * scale,
                [0.58, 0.37, 0.91], 18, 2.2, 0.035);
            return Effect::Taken;
        }
    }
    match target(world, player) {
        Some(Target::Block(cell)) => {
            if super::mill::is_wheel(world, cell) {
                let status = super::mill::command(world, player, "status");
                world.expedition.say_for(status, 9.0);
                return Effect::Taken;
            }
            if world.get(cell) == BlockId::ClaimStake && super::farming::base::enabled(world) {
                super::farming::base::use_stake(world, cell, sneak);
                return Effect::Taken;
            }
            if world.get(cell) == BlockId::Bed && super::spawn::bed_binds_spawn(world) {
                if super::spawn::bind_bed(world, player, cell) {
                    world.expedition.say("Spawn set");
                } else {
                    world.expedition.say("Bed obstructed");
                }
                return Effect::Taken;
            }
            if sneak && let Some((_, front)) = super::raycast_block(world, eye / scale, forward, REACH / scale)
                && let Some((p, side)) = super::factory::sides::side_toward(world, cell, front) {
                super::factory::action(world, player.pos, super::factory::Action::ToggleSide(p, side));
                return Effect::Taken;
            }
            if super::fittings::interact(world, cell, player, blocks, chunks) { return Effect::Taken; }
            return Effect::Block(cell);
        }
        Some(Target::Relic) => {
            super::adventure::reliquaries::interact(world, player);
            return Effect::Taken;
        }
        Some(Target::Mimic(site, id)) => {
            super::adventure::encounters::open_cache(world, site, id, player.pos * scale);
            return Effect::Taken;
        }
        Some(Target::Resident(id)) => return Effect::Trade(id),
        Some(Target::ConvoyDriver(id)) => {
            super::farming::planet_freight::talk(world, id);
            return Effect::Taken;
        }
        Some(Target::StrandedCargo) => {
            super::farming::planet_freight::recover(world);
            return Effect::Taken;
        }
        Some(Target::Traveller) => {
            let target = super::travel::target_distance(world, eye, forward).map(|(_, t)| t);
            return match super::travel::interact_target(world, target) {
                super::travel::Use::OpenTrade(id) => Effect::Trade(id),
                _ => Effect::Taken,
            };
        }
        None => {}
    }
    if super::adventure::chronicle::interact(world, player) { return Effect::Taken; }
    if super::orbital::cargo::console_focused(world, player) || super::orbital::receiver::focused(world, player) {
        return Effect::Inventory(8);
    }
    if super::orbital::express::interact(world, player)
        || super::adventure::reliquaries::interact(world, player)
        || crate::manyfold::use_weight(world, player, true)
        || super::portals::gates::interact(world, player).is_some() { return Effect::Taken; }
    let outcome = super::undercrown::interact(world, player);
    if outcome.used {
        for cell in outcome.changed {
            blocks.insert(cell);
            super::mark_chunk_and_neighbors_dirty(chunks, cell);
        }
        if let Some(message) = outcome.message { world.expedition.say(message); }
        return Effect::Taken;
    }
    if super::progression::study::guide_focused(world, player) {
        super::progression::thresholds::Milestone::ReadGuide.record(&mut world.progression);
        world.expedition.say("Field guide · Inventory → Recipes → Portals");
        return Effect::ThresholdGuide;
    }
    if let Some((cell, _)) = super::raycast_block(world, eye / scale, forward, REACH / scale) {
        if super::environment::inspect::clear(world, cell)
            || super::environment::cultivation::compost(world, cell, slot) { return Effect::Taken; }
    }
    if world.inventory.retrieve_nearby(player.pos * scale) {
        world.sync_from_inventory();
        return Effect::Taken;
    }
    Effect::None
}
