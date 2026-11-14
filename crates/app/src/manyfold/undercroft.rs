//! The listening chamber: visible build-up, solid cover, editable source.
use crate::{
    audio::{Cue, Event},
    mesh::models::Model,
    player::Player,
    world::{self, interaction::Prompt, BlockId, World, WorldType},
};
use engine_worldgen::manyfold::{undercroft as layout, weave::point};
use glam::{IVec3, Vec3};

fn enabled(w: &World) -> bool {
    w.world_type == WorldType::Manyfold
        && w.generator_version >= layout::VERSION
        && w.generator_version < engine_worldgen::manyfold::INFINITE_MAZE
}

fn source(w: &World) -> Option<IVec3> {
    if !enabled(w) {
        return None;
    }
    let cell = layout::heart(&w.sample_config());
    (super::weave::known(w, cell) == Some(BlockId::Crystal)).then_some(cell)
}

fn aimed(w: &World, eye: Vec3, forward: Vec3) -> Option<IVec3> {
    let cell = source(w)?;
    let scale = w.voxel_scale.meters();
    let (hit, _) = world::raycast_block(w, eye / scale, forward, 4.0 / scale)?;
    (hit == cell).then_some(cell)
}

pub fn focus(w: &World, eye: Vec3, forward: Vec3) -> Option<Prompt> {
    aimed(w, eye, forward)?;
    Some(Prompt {
        title: "Listening stone",
        action: "[F] Dampen resonance",
    })
}

pub fn interact(w: &mut World, p: &Player) -> bool {
    let Some(cell) = aimed(
        w,
        p.eye(w.voxel_scale) * w.voxel_scale.meters(),
        p.forward_dir(),
    ) else {
        return false;
    };
    if let Ok(commit) = world::effects::replace(w, cell, BlockId::Crystal, BlockId::CarvedStone) {
        let mut changed = Vec::with_capacity(1);
        commit.publish_voxel(w, &mut changed);
        w.adventure.dirty_voxels.extend(changed);
        w.expedition.say("The stone falls silent.");
        sound(w, cell, Cue::Ward, 0.4, 0.55);
    }
    true
}

fn sound(w: &mut World, cell: IVec3, cue: Cue, gain: f32, pitch: f32) {
    if w.adventure.sounds.len() < 64 {
        w.adventure.sounds.push(Event {
            cue,
            pos: Some((cell.as_vec3() + Vec3::splat(0.5)) * w.voxel_scale.meters()),
            gain,
            pitch,
        });
    }
}

/// Called only by solo/host authority. Cooldowns belong to each player, while
/// all visible inscriptions and the warning use the same chamber clock.
pub fn tick(w: &mut World, p: &mut Player, dt: f32) {
    p.manyfold_resonance_harm = (p.manyfold_resonance_harm - dt).max(0.0);
    p.manyfold_resonance_cue = (p.manyfold_resonance_cue - dt).max(0.0);
    if !w.rules.mode.interacts() || w.adventure.is_dead() {
        return;
    }
    let Some(cell) = source(w) else {
        return;
    };
    let scale = w.voxel_scale.meters();
    let eye = p.eye(w.voxel_scale) * scale;
    let target = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
    let delta = target - eye;
    let distance = delta.length();
    if !(0.01..10.0).contains(&distance) {
        return;
    }
    // Check every intervening cell, including residency. A carved passage can
    // carry the sound; solid construction and unloaded edits always stop it.
    let steps = (distance / (scale * 0.35)).ceil() as usize;
    for i in 1..steps {
        let at = ((eye + delta * (i as f32 / steps as f32)) / scale)
            .floor()
            .as_ivec3();
        if at == cell {
            break;
        }
        if super::weave::known(w, at).is_none_or(|b| !b.is_empty()) {
            return;
        }
    }
    let phase = w.adventure.manyfold_phase;
    if (4.5..6.5).contains(&phase) && p.manyfold_resonance_cue <= 0.0 {
        p.manyfold_resonance_cue = 8.0;
        sound(w, cell, Cue::Echo, 0.6, 0.55);
    }
    // Entering during a pulse never produces an unannounced hit. A player
    // must have received this cycle's warning at least two seconds earlier.
    let warned = p.manyfold_resonance_cue > 0.0 && p.manyfold_resonance_cue <= 6.0;
    if (6.5..7.0).contains(&phase) && warned && p.manyfold_resonance_harm <= 0.0 {
        p.manyfold_resonance_harm = 8.0;
        w.adventure
            .environment_damage(4.0, world::adventure::death::Cause::Wounds);
        sound(w, cell, Cue::Rift, 0.45, 0.6);
    }
}

pub fn draw(mesh: &mut Model, w: &World) {
    if source(w).is_none() {
        return;
    }
    let phase = w.adventure.manyfold_phase;
    let charge = ((phase - 4.5) / 2.0).clamp(0.0, 1.0);
    let glow = if phase < 7.0 {
        0.08 + charge * 0.85
    } else {
        0.04
    };
    let color = if phase >= 6.5 && phase < 7.0 {
        [0.75, 0.30, 0.20]
    } else {
        [0.24, 0.51, 0.58]
    };
    // Three incomplete scored circles converge on the throat. These are local
    // marks on the chamber floor, never a full-screen horror filter.
    for ring in 0..3 {
        let radius = 1.0 + ring as f32 * 0.7;
        for i in 0..32 {
            if i % 8 == 0 {
                continue;
            }
            let at = |j: f32| {
                let a = j * std::f32::consts::TAU / 32.0;
                point(0, a.cos() * radius, -4.86, -19.0 + a.sin() * radius)
            };
            mesh.beam(at(i as f32), at(i as f32 + 0.72), 0.035, color, glow);
        }
    }
}
