//! A caster sustains one field. Fields are transient and never own terrain.
use super::*;
use crate::world::progression::study;

pub(crate) const AEGIS: u32 = 33;
pub(crate) const PLASMA: u32 = 11;
pub(super) const AEGIS_RADIUS: f32 = 3.0;
pub(super) const PLASMA_REACH: f32 = 8.0;
const TETHER: f32 = 12.0;
const PLASMA_SPEED: f32 = 3.0;

fn plasma_target(world: &World, eye: Vec3, forward: Vec3) -> Vec3 {
    let end = eye + forward * PLASMA_REACH;
    trace::solid(world, eye, end)
        .map_or(end, |(at, _)| eye + forward * ((at - eye).dot(forward) - 0.12).max(0.0))
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
enum Field {
    Aegis(Vec3),
    Plasma(spells::PlasmaCross),
}

impl Field {
    fn center(&self) -> Vec3 {
        match self {
            Self::Aegis(center) => *center,
            Self::Plasma(cross) => cross.pos,
        }
    }
}

#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) struct Channel {
    field: Field,
    wand: Option<u16>,
    deck: wands::Loadout,
    upkeep: f32,
}

impl Channel {
    pub(super) fn upkeep(&self) -> f32 {
        self.upkeep
    }

    pub(super) fn plasma_mut(&mut self) -> Option<&mut spells::PlasmaCross> {
        match &mut self.field {
            Field::Plasma(cross) => Some(cross),
            Field::Aegis(_) => None,
        }
    }
}

pub(crate) fn requirements(spell: u32) -> Option<(f32, study::Lesson)> {
    match spell {
        AEGIS => Some((8.0, study::Lesson::Wards)),
        PLASMA => Some((16.0, study::Lesson::Celestial)),
        _ => None,
    }
}

pub(crate) fn binding_upkeep(upkeep: f32, mode: u32) -> f32 {
    upkeep * super::enchanting::binding(mode).map_or(1.0, |binding| binding.upkeep)
}

impl Adventure {
    /// Returns true when this cast belongs to the channel path, even if refused.
    pub(super) fn start_concentration(
        &mut self, world: &mut World, eval: &wands::CastBlockEval, eye: Vec3, forward: Vec3,
    ) -> bool {
        let Some((spell, (upkeep, lesson))) = eval.actions.iter().flat_map(|action| {
            [Some(action.spell), action.payload, action.follow].into_iter().flatten()
        }).find_map(|spell| requirements(spell).map(|spec| (spell, spec))) else { return false };
        if !wands::is_grimoire(self.loadout.mode) {
            self.cast_cd = 0.35;
            world.expedition.say("Concentration requires a grimoire");
            return true;
        }
        let action = &eval.actions[0];
        if eval.actions.len() != 1 || action.spell != spell || action.payload.is_some()
            || action.follow.is_some() || !eval.modifier_slots.is_empty()
        {
            self.cast_cd = 0.35;
            world.expedition.say("Concentration · Use a single card");
            return true;
        }
        if study::available(world) && !lesson.learned(world.progression.lessons, world.progression.horticulture) {
            self.cast_cd = 0.35;
            world.expedition.say(format!("Study {} at a bookcase", lesson.title()));
            return true;
        }
        if self.hurt > 0.0 {
            return true;
        }
        let mana_mul = modifiers::mana_mul(self.site_modifier);
        let upkeep = binding_upkeep(upkeep, self.loadout.mode);
        let cost = inscriptions::cost(spell, self.discoveries) * mana_mul;
        if self.mana < cost {
            self.cast_cd = 0.2;
            self.mana_flash = 0.45;
            return true;
        }
        let field = if spell == AEGIS {
            Field::Aegis(eye)
        } else {
            let pos = plasma_target(world, eye, forward);
            Field::Plasma(spells::PlasmaCross { pos, normal: -forward, life: 1.0, pulse: 0.0 })
        };
        self.mana -= cost;
        // One maintained spell: a channel ends a sustained circle.
        if self.circles.release_sustained() {
            world.expedition.say("Circle Released");
        }
        self.last_cast = spell;
        self.cast_pose = 0.4;
        self.concentration = Some(Channel {
            field, wand: self.active_wand, deck: self.loadout, upkeep: upkeep * mana_mul,
        });
        sound(&mut self.sounds, super::spell_defs::spell_audio(spell).0, Some(field.center()), 0.7);
        true
    }

    pub(super) fn sustain_concentration(
        &mut self, world: &World, eye: Vec3, forward: Vec3, dt: f32, antimagic: bool,
    ) {
        let Some(channel) = self.concentration.as_mut() else { return };
        let cost = channel.upkeep * dt;
        let center = channel.field.center();
        if antimagic || self.hurt > 0.0 || !wands::is_grimoire(self.loadout.mode) || self.active_wand != channel.wand
            || self.loadout != channel.deck || self.mana < cost
            || center.distance_squared(eye) > TETHER * TETHER || !line(world, eye, center)
        {
            self.concentration = None;
            self.cast_cd = self.cast_cd.max(0.35);
        } else {
            self.mana -= cost;
            if let Some(cross) = channel.plasma_mut() {
                let target = plasma_target(world, eye, forward);
                let next = cross.pos + (target - cross.pos).clamp_length_max(PLASMA_SPEED * dt);
                if line(world, cross.pos, next) && line(world, eye, next) {
                    cross.pos = next;
                }
            }
        }
    }

    pub(super) fn sustained_plasma(&self) -> Option<&spells::PlasmaCross> {
        match &self.concentration.as_ref()?.field {
            Field::Plasma(cross) => Some(cross),
            Field::Aegis(_) => None,
        }
    }

    fn aegis_center(&self) -> Option<Vec3> {
        match self.concentration.as_ref()?.field {
            Field::Aegis(center) => Some(center),
            Field::Plasma(_) => None,
        }
    }

    /// The shell blocks an inbound segment. Shots originating inside can leave.
    pub(super) fn aegis_contact(&self, from: Vec3, to: Vec3) -> Option<Vec3> {
        let center = self.aegis_center()?;
        let offset = from - center;
        let delta = to - from;
        let c = offset.length_squared() - AEGIS_RADIUS * AEGIS_RADIUS;
        let a = delta.length_squared();
        let b = offset.dot(delta);
        let discriminant = b * b - a * c;
        if c < 0.0 || a == 0.0 || b >= 0.0 || discriminant < 0.0 {
            return None;
        }
        let t = (-b - discriminant.sqrt()) / a;
        (0.0..=1.0).contains(&t).then(|| from + delta * t)
    }

    pub(super) fn aegis_model(&self, model: &mut Model) {
        if let Some(center) = self.aegis_center() {
            model.glow_orb(center, AEGIS_RADIUS, spell_color(AEGIS), 0.65, 0.12);
            tactics::ring(model, center, AEGIS_RADIUS, spell_color(AEGIS), 1.2);
        }
    }
}
