//! Native rune casting. A phrase bound to the held wand runs here, straight from its
//! compiled tree, instead of being lowered onto deck cards. It reuses the projectile and
//! status handlers, but never the deck evaluator, so a phrase cannot inherit twinning,
//! shuffle or a trigger the writer did not put in it.
//!
//! An event names a child phrase. The parent bolt carries a handle; the first accepted
//! hit (a pierced foe, a foe, or a surface) takes the child out of `State::pending` and
//! runs it once at that point, after the parent's own effect has landed. A bolt that
//! misses or expires drops its child unpaid work; the mana was paid at cast time.
//!
//! Bindings are transient: they live with the session, not the save.
use super::*;
use engine_content::runes::{Field, Phrase, Program};

/// Area forms resolved at an event point: radius before qualifiers, total height, and the
/// most foes one pulse may touch.
const EVENT_RADIUS: f32 = 1.5;
const EVENT_HEIGHT: f32 = 2.0;
const EVENT_TARGETS: usize = 4;
/// Damage of an elemental area pulse before the status reaction multiplies it.
const PULSE_DAMAGE: f32 = 6.0;

struct Bound {
    wand: Option<u16>,
    program: Program,
}

#[derive(Default)]
pub struct State {
    bound: Option<Bound>,
    /// Event children waiting on a bolt's first hit, by handle.
    pending: Vec<(u32, Phrase)>,
    next: u32,
}

impl State {
    pub(super) fn unbind(&mut self) {
        self.bound = None;
    }

    /// Canonical text of the bound phrase, for the wand page.
    pub(super) fn phrase(&self) -> Option<String> {
        self.bound.as_ref().map(|bound| bound.program.text.clone())
    }

    fn hold(&mut self, child: Phrase) -> u32 {
        self.next = self.next.wrapping_add(1).max(1);
        self.pending.push((self.next, child));
        self.next
    }
}

fn is_area(form: &engine_content::runes::Rune) -> bool {
    form.circle
}

/// Projectile forms carry a deck spell for flight and impact; area forms run at a point.
fn check_phrase(phrase: &Phrase, root: bool, can_wield: &dyn Fn(u32) -> bool) -> Result<(), String> {
    let table = &*runes::TABLE;
    let form = table.rune(phrase.form);
    let area = is_area(form);
    if area && root {
        return Err("Needs Circle".into());
    }
    if !area {
        let card = form.card.filter(|&card| card != 34).ok_or_else(|| format!("Not Native: {}", form.name))?;
        if !can_wield(card) {
            return Err(format!("Not Found: {}", spell_name(card)));
        }
    }
    let effect = phrase.effect.as_ref();
    let qualifiers = phrase.qualifiers.iter().chain(effect.into_iter().flat_map(|e| &e.qualifiers));
    for &id in qualifiers {
        let rune = table.rune(id);
        if rune.sustain {
            return Err("Sustain Needs Circle".into());
        }
        // Area pulses read only reach; bolts read the qualifier's card.
        let usable = if area { rune.reach != 1.0 } else { rune.card.is_some() };
        if !usable {
            return Err(format!("Not Native: {}", rune.name));
        }
        if let Some(card) = rune.card.filter(|_| !area)
            && !can_wield(card)
        {
            return Err(format!("Not Found: {}", spell_name(card)));
        }
    }
    if let Some(effect) = effect {
        let rune = table.rune(effect.rune);
        if effect.material.is_some() {
            return Err("Material Needs Circle".into());
        }
        let ok = match rune.field {
            Some(Field::Push) => area,
            Some(Field::Burn | Field::Chill | Field::Shock | Field::Venom | Field::Drench) => true,
            _ => false,
        };
        if !ok {
            return Err(format!("Not Native: {}", rune.name));
        }
        if let Some(card) = rune.card.filter(|_| !area)
            && !can_wield(card)
        {
            return Err(format!("Not Found: {}", spell_name(card)));
        }
    }
    if phrase.events.len() > 1 {
        return Err("One Event Per Form".into());
    }
    for (event, child) in &phrase.events {
        let event = table.rune(*event);
        if area || event.key != "ON_FIRST_HIT" {
            return Err(format!("Not Native: {}", event.name));
        }
        check_phrase(child, false, can_wield)?;
    }
    Ok(())
}

/// Why the native path cannot cast this program on this wand, if it cannot.
pub fn check(program: &Program, wand: u32, can_wield: &dyn Fn(u32) -> bool) -> Result<(), String> {
    runes::fits_wand(program, wand)?;
    check_phrase(&program.root, true, can_wield)
}

impl Adventure {
    /// Bind a phrase packed by `runes::pack_program` to the held wand, or clear the
    /// binding when the word is `u64::MAX`.
    pub fn bind_runes(&mut self, word: u64) -> Result<String, String> {
        if word == u64::MAX {
            self.rune_cast.unbind();
            return Ok("Phrase Released".into());
        }
        let program = runes::unpack_program(word)
            .and_then(|text| runes::TABLE.compile(&text).ok())
            .ok_or("Rune Phrase Invalid")?;
        check(&program, self.wand, &|card| self.can_wield(card))?;
        self.rune_cast.bound = Some(Bound { wand: self.active_wand, program });
        Ok("Phrase Bound".into())
    }

    /// Cast the bound phrase. False when nothing is bound to the held wand, so the deck
    /// casts instead.
    pub(super) fn cast_bound_runes(&mut self, world: &mut World, eye: Vec3, forward: Vec3) -> bool {
        let Some(bound) = &self.rune_cast.bound else { return false };
        if bound.wand != self.active_wand {
            self.rune_cast.unbind();
            return false;
        }
        let program = bound.program.clone();
        if let Err(reason) = check(&program, self.wand, &|card| self.can_wield(card)) {
            self.cast_cd = 0.35;
            world.expedition.say(reason);
            return true;
        }
        // One quote, paid once and in full before anything leaves the wand; event
        // children were priced into it by the compiler.
        let cost = program.mana * modifiers::mana_mul(self.site_modifier);
        if self.mana < cost {
            self.cast_cd = 0.12;
            self.mana_flash = 0.45;
            return true;
        }
        if self.projectiles.len() >= 96 {
            self.cast_cd = 0.12;
            return true;
        }
        self.mana -= cost;
        let forward = modifiers::wobble(self.site_modifier, forward, self.time);
        self.emit_phrase(world, &program.root, eye, forward);
        let form = runes::TABLE.rune(program.root.form).card.unwrap_or(0);
        self.last_cast = form;
        self.cast_pose = 0.4;
        self.cast_cd = 0.3 + 0.04 * program.slots as f32;
        sound(&mut self.sounds, super::spell_defs::spell_audio(form).0, Some(eye), 0.6);
        true
    }

    /// Run one phrase at `pos` facing `dir`: launch its bolt, or pulse its area there.
    fn emit_phrase(&mut self, world: &World, phrase: &Phrase, pos: Vec3, dir: Vec3) {
        let table = &*runes::TABLE;
        let form = table.rune(phrase.form);
        if is_area(form) {
            self.rune_pulse(world, phrase, pos);
            return;
        }
        let Some(spell) = form.card else { return };
        let salt = self.time.to_bits();
        let mut mods = wands::ShotMods::default();
        for &id in &phrase.qualifiers {
            if let Some(card) = table.rune(id).card {
                wands::paint_shot(card, &mut mods, salt);
            }
        }
        if let Some(effect) = &phrase.effect {
            for &id in &effect.qualifiers {
                if let Some(card) = table.rune(id).card {
                    wands::paint_shot(card, &mut mods, salt);
                }
            }
            if let Some(card) = table.rune(effect.rune).card {
                wands::paint_shot(card, &mut mods, salt);
            }
        }
        if let Some((_, child)) = phrase.events.first() {
            mods.rune = self.rune_cast.hold(child.clone());
        }
        self.emit_spell_spread(spell, pos, dir, None, mods, arcana::SCATTER_BASE_SPREAD, None);
    }

    /// A bolt carrying `handle` made its first accepted hit at `at`; run its child once.
    /// `normal` faces back out of what was hit; a child bolt leaves along `dir`.
    pub(super) fn rune_event(&mut self, world: &World, handle: u32, at: Vec3, normal: Vec3, dir: Vec3) {
        let Some(index) = self.rune_cast.pending.iter().position(|(h, _)| *h == handle) else { return };
        let (_, child) = self.rune_cast.pending.swap_remove(index);
        let normal = normal.try_normalize().unwrap_or(Vec3::Y);
        let dir = dir.try_normalize().unwrap_or(normal);
        self.emit_phrase(world, &child, at + normal * 0.1, dir);
    }

    /// Drop children whose bolts are gone without a hit.
    pub(super) fn tick_rune_cast(&mut self) {
        let mut pending = std::mem::take(&mut self.rune_cast.pending);
        pending.retain(|(handle, _)| self.projectiles.iter().any(|shot| shot.mods.rune == *handle));
        self.rune_cast.pending = pending;
    }

    /// An area form resolved at an event point: a flat disk (or ring band) of fixed
    /// height that touches at most `EVENT_TARGETS` foes in line of sight of the point.
    /// It leaves nothing behind.
    fn rune_pulse(&mut self, world: &World, phrase: &Phrase, at: Vec3) {
        let table = &*runes::TABLE;
        let Some(effect) = &phrase.effect else { return };
        let Some(field) = table.rune(effect.rune).field else { return };
        let reach = phrase
            .qualifiers
            .iter()
            .chain(&effect.qualifiers)
            .map(|&id| table.rune(id).reach)
            .product::<f32>();
        let radius = EVENT_RADIUS * reach;
        let ring = table.rune(phrase.form).key == "RING";
        let band = radius - 1.0;
        let flat = |p: Vec3| Vec3::new(p.x - at.x, 0.0, p.z - at.z).length();
        let mut targets: Vec<(f32, usize)> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, enemy)| enemy.health > 0.0)
            .filter_map(|(index, enemy)| {
                let centre = enemy.center();
                let d = flat(centre);
                ((centre.y - at.y).abs() <= EVENT_HEIGHT * 0.5
                    && d <= radius
                    && (!ring || d >= band)
                    && line(world, at, centre))
                .then_some((d, index))
            })
            .collect();
        targets.sort_by(|a, b| a.0.total_cmp(&b.0));
        targets.truncate(EVENT_TARGETS);
        let element = circles::element(field);
        for &(_, index) in &targets {
            let enemy = &mut self.enemies[index];
            let centre = enemy.center();
            match element {
                Some(element) => {
                    let reaction = enemy.status.apply(element, enemy.affixes);
                    enemy.health -= PULSE_DAMAGE * reaction.damage_mul;
                    enemy.stagger = enemy.stagger.max(reaction.stagger);
                }
                None => {
                    let out = Vec3::new(centre.x - at.x, 0.0, centre.z - at.z).normalize_or_zero();
                    enemy.knock = out * 8.0 + Vec3::Y * 2.0;
                }
            }
            enemy.flash = 0.2;
        }
        let tint = circles::color(field);
        self.effect(at, tint, radius);
        for i in 0..12 {
            let turn = i as f32 * std::f32::consts::TAU / 12.0;
            self.spawn_voxel_burst(at + Vec3::new(turn.cos(), 0.05, turn.sin()) * radius, tint, 2, 0.9, 0.035);
        }
        if let Some(card) = table.rune(effect.rune).card {
            let (cue, _) = super::spell_defs::spell_audio(card);
            sound(&mut self.sounds, cue, Some(at), 0.6);
        }
    }
}
