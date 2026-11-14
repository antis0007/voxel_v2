//! Ritual circles: a rune phrase with a circle form, cast slowly on the ground.
//!
//! The caster stands at the centre while the ritual runs (two seconds per rune), paying
//! the phrase's mana over that time. Mature mana flowers inside the circle pay first.
//! PLACE builds the footprint as the ritual advances. Other effects pulse once when it
//! completes, or keep pulsing for upkeep when the phrase carries SUSTAIN. One circle per
//! caster. Circles are transient: they are not saved and never own terrain.
//!
//! A SUSTAIN circle is the caster's one maintained spell, under the same rule as Aegis and
//! Plasma: it needs a grimoire and the Wards lesson, it ends any channel when it starts,
//! a channel ends it, its upkeep takes the binding's discount once, and recovery pauses
//! while it is held (from the start of its ritual).
use super::*;
use crate::world::farming;
use engine_content::runes::Field;
use status::Element;

/// Radius in metres before qualifiers.
const RADIUS: f32 = 3.0;
const SECONDS_PER_RUNE: f32 = 2.0;
/// Seconds between field pulses.
const PULSE: f32 = 0.5;
/// A sustained circle ends when its caster is farther than this, in metres.
const TETHER: f32 = 12.0;
/// A ring's band, and the slack allowed around the caster's feet, in metres.
const BAND: f32 = 1.0;
const MAX_CELLS: usize = 400;

struct Circle {
    centre: Vec3,
    radius: f32,
    /// A ring acts on its band only; a disk on its whole interior.
    ring: bool,
    field: Field,
    sustain: bool,
    block: Option<crate::world::BlockId>,
    /// Footprint cells in sweep order, for PLACE.
    cells: Vec<IVec3>,
    placed: usize,
    ritual: f32,
    ritual_total: f32,
    /// Mana per second while the ritual runs, then while the field is held.
    cast_rate: f32,
    upkeep: f32,
    pulse: f32,
    /// The phrase's runes in written order, carved round the rim as the ritual runs.
    runes: Vec<engine_content::runes::RuneId>,
}

#[derive(Default)]
pub struct State {
    circle: Option<Circle>,
    /// A PURGE circle holds the caster this tick.
    pub purging: bool,
}

impl State {
    /// The caster maintains a SUSTAIN circle (ritual or held), which pauses recovery.
    pub(super) fn sustaining(&self) -> bool {
        self.circle.as_ref().is_some_and(|circle| circle.sustain)
    }

    /// Upkeep of a held SUSTAIN circle, for the HUD.
    pub(super) fn held_upkeep(&self) -> Option<f32> {
        self.circle.as_ref().filter(|circle| circle.sustain).map(|circle| circle.upkeep)
    }

    /// End a maintained circle because another maintained spell took over. True if one ended.
    pub(super) fn release_sustained(&mut self) -> bool {
        let held = self.sustaining();
        if held {
            self.circle = None;
            self.purging = false;
        }
        held
    }
}

/// Seconds a phrase takes to cast as a ritual, and its circle's radius in metres.
pub fn preview(program: &engine_content::runes::Program) -> (f32, f32) {
    let table = &*runes::TABLE;
    let root = &program.root;
    let effect = root.effect.iter().flat_map(|effect| &effect.qualifiers);
    let reach = root.qualifiers.iter().chain(effect).map(|&id| table.rune(id).reach).product::<f32>();
    (program.slots as f32 * SECONDS_PER_RUNE, RADIUS * reach)
}

pub(super) fn color(field: Field) -> [f32; 3] {
    match field {
        Field::Burn => Element::Fire.color(),
        Field::Chill => Element::Frost.color(),
        Field::Shock => Element::Shock.color(),
        Field::Venom => Element::Toxic.color(),
        Field::Drench => Element::Wet.color(),
        Field::Push => [0.82, 0.86, 0.9],
        Field::Purge => [0.62, 0.9, 0.78],
        Field::Place => [0.8, 0.74, 0.6],
    }
}

pub(super) fn element(field: Field) -> Option<Element> {
    Some(match field {
        Field::Burn => Element::Fire,
        Field::Chill => Element::Frost,
        Field::Shock => Element::Shock,
        Field::Venom => Element::Toxic,
        Field::Drench => Element::Wet,
        Field::Push | Field::Purge | Field::Place => return None,
    })
}

fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    Vec3::new(a.x - b.x, 0.0, a.z - b.z).length()
}

impl Circle {
    fn holds(&self, pos: Vec3) -> bool {
        let d = flat_distance(pos, self.centre);
        (pos.y - self.centre.y).abs() <= 3.0
            && d <= self.radius
            && (!self.ring || d >= self.radius - BAND)
    }

    fn holds_caster(&self, feet: Vec3) -> bool {
        (feet.y - self.centre.y).abs() <= 3.0 && flat_distance(feet, self.centre) <= self.radius + BAND
    }
}

/// Mana per second from mature mana flowers standing inside the circle.
fn flower_supply(world: &World, centre: Vec3, radius: f32) -> f32 {
    let scale = world.voxel_scale.meters();
    world
        .farming
        .plots
        .iter()
        .filter_map(|(&(x, y, z), plot)| {
            let crop = farming::table::get(plot.crop)?;
            let Some(farming::table::Aura::Mana { per_second, .. }) = crop.aura else { return None };
            let at = (IVec3::new(x, y, z).as_vec3() + Vec3::splat(0.5)) * scale;
            (plot.stage >= crop.mature()
                && (at.y - centre.y).abs() <= 3.0
                && flat_distance(at, centre) <= radius)
                .then_some(per_second)
        })
        .sum()
}

fn footprint(centre: Vec3, radius: f32, ring: bool, scale: f32) -> Vec<IVec3> {
    let reach = (radius / scale).ceil() as i32;
    let origin = (centre / scale).floor().as_ivec3();
    let band = BAND.max(scale);
    let mut cells = Vec::new();
    for dz in -reach..=reach {
        for dx in -reach..=reach {
            let cell = origin + IVec3::new(dx, 0, dz);
            let d = flat_distance((cell.as_vec3() + Vec3::splat(0.5)) * scale, centre);
            if d <= radius && (!ring || d >= radius - band) {
                cells.push(cell);
            }
        }
    }
    // Sweep round the centre so the ring closes as the ritual advances.
    cells.sort_by(|a, b| {
        let angle = |c: &IVec3| ((c.z - origin.z) as f32).atan2((c.x - origin.x) as f32);
        angle(a).total_cmp(&angle(b)).then(a.x.cmp(&b.x)).then(a.z.cmp(&b.z))
    });
    cells
}

impl Adventure {
    /// Begin a ritual at the caster's feet from a word packed by `runes::pack_phrase`.
    /// Returns the ritual's length in seconds.
    pub fn draw_circle(&mut self, world: &World, feet: Vec3, word: u64) -> Result<f32, String> {
        let table = &*runes::TABLE;
        let program = runes::unpack_phrase(word)
            .and_then(|text| table.compile(&text).ok())
            .ok_or("Rune Phrase Invalid")?;
        if !program.circle {
            return Err("No Circle Form".into());
        }
        runes::fits_wand(&program, self.wand)?;
        let sustain = program.sustained();
        if sustain {
            if !wands::is_grimoire(self.loadout.mode) {
                return Err("Concentration requires a grimoire".into());
            }
            let lesson = crate::world::progression::study::Lesson::Wards;
            if crate::world::progression::study::available(world)
                && !lesson.learned(world.progression.lessons, world.progression.horticulture)
            {
                return Err(format!("Study {} at a bookcase", lesson.title()));
            }
        }
        let root = &program.root;
        let effect = root.effect.as_ref().ok_or("No Circle Effect")?;
        let field = table.rune(effect.rune).field.ok_or("No Circle Effect")?;
        let (ritual_total, radius) = preview(&program);
        let ring = table.rune(root.form).key == "RING";
        let scale = world.voxel_scale.meters();
        let block = effect.material.and_then(|id| table.rune(id).block);
        let cells = if field == Field::Place {
            if block.is_none() {
                return Err("No Material".into());
            }
            let cells = footprint(feet, radius, ring, scale);
            if cells.len() > MAX_CELLS {
                return Err(format!("Circle Cells: {}/{MAX_CELLS}", cells.len()));
            }
            cells
        } else {
            Vec::new()
        };
        // Everything is checked; only now does a new maintained spell replace the old one.
        if sustain {
            self.concentration = None;
        }
        let upkeep = concentration::binding_upkeep(program.upkeep, self.loadout.mode);
        self.circles.circle = Some(Circle {
            centre: feet,
            radius,
            ring,
            field,
            sustain,
            block,
            cells,
            placed: 0,
            ritual: ritual_total,
            ritual_total,
            cast_rate: program.mana / ritual_total,
            upkeep,
            pulse: 0.0,
            runes: table.written(&program.root),
        });
        Ok(ritual_total)
    }

    /// Take `rate * dt` mana, flowers in the circle first. False when the caster runs dry.
    fn pay_circle(&mut self, rate: f32, supply: f32, dt: f32) -> bool {
        let owed = (rate - supply).max(0.0) * dt;
        if self.mana < owed {
            return false;
        }
        self.mana -= owed;
        true
    }

    pub(super) fn tick_circles(&mut self, world: &mut World, feet: Vec3, dt: f32) {
        self.circles.purging = false;
        let Some(mut circle) = self.circles.circle.take() else { return };
        world.goals.note(crate::world::goals::Event::CircleDrawn);
        let supply = flower_supply(world, circle.centre, circle.radius) + hedrons::supply(world, circle.centre, element(circle.field));
        let tint = color(circle.field);
        if circle.ritual > 0.0 {
            if !circle.holds_caster(feet) {
                world.expedition.say("Ritual Broken");
                return;
            }
            if !self.pay_circle(circle.cast_rate, supply, dt) {
                world.expedition.say("Ritual Broken: No Mana");
                return;
            }
            circle.ritual -= dt;
            let done = 1.0 - (circle.ritual / circle.ritual_total).max(0.0);
            if let Some(block) = circle.block {
                let target = (done * circle.cells.len() as f32).ceil() as usize;
                while circle.placed < target.min(circle.cells.len()) {
                    let cell = circle.cells[circle.placed];
                    circle.placed += 1;
                    if world.get(cell) == crate::world::BlockId::Air {
                        world.set(cell, block);
                        world.activity.wake(cell);
                        self.mark_voxel_dirty(world, cell);
                    }
                }
            }
            circle.pulse -= dt;
            if circle.pulse <= 0.0 {
                circle.pulse = PULSE;
                // A spark walks the rim once per ritual.
                let turn = done * std::f32::consts::TAU;
                let rim = circle.centre + Vec3::new(turn.cos(), 0.1, turn.sin()) * circle.radius;
                self.spawn_voxel_burst(rim, tint, 6, 1.2, 0.04);
            }
            if circle.ritual > 0.0 {
                self.circles.circle = Some(circle);
                return;
            }
            self.effect(circle.centre, tint, circle.radius);
            if circle.field == Field::Place {
                return;
            }
            self.pulse_circle(&circle, feet);
            if circle.sustain {
                circle.pulse = PULSE;
                self.circles.circle = Some(circle);
            }
            return;
        }
        if feet.distance(circle.centre) > TETHER {
            world.expedition.say("Circle Released");
            return;
        }
        if !self.pay_circle(circle.upkeep, supply, dt) {
            world.expedition.say("Circle Released: No Mana");
            return;
        }
        circle.pulse -= dt;
        if circle.pulse <= 0.0 {
            circle.pulse = PULSE;
            self.pulse_circle(&circle, feet);
        } else if circle.field == Field::Purge {
            self.circles.purging = circle.holds(feet);
        }
        self.circles.circle = Some(circle);
    }

    fn pulse_circle(&mut self, circle: &Circle, feet: Vec3) {
        let tint = color(circle.field);
        for i in 0..8 {
            let turn = i as f32 * std::f32::consts::TAU / 8.0 + self.time;
            let rim = circle.centre + Vec3::new(turn.cos(), 0.1, turn.sin()) * circle.radius;
            self.spawn_voxel_burst(rim, tint, 2, 0.8, 0.04);
        }
        if circle.field == Field::Purge {
            self.circles.purging = circle.holds(feet);
            return;
        }
        let element = element(circle.field);
        for enemy in &mut self.enemies {
            let at = enemy.center();
            if !circle.holds(at) {
                continue;
            }
            match element {
                Some(element) => {
                    enemy.status.apply(element, enemy.affixes);
                }
                None => {
                    let out = Vec3::new(at.x - circle.centre.x, 0.0, at.z - circle.centre.z).normalize_or_zero();
                    enemy.knock = out * 8.0 + Vec3::Y * 2.0;
                }
            }
            enemy.flash = 0.2;
        }
    }
}

/// Height of a carved rim glyph, in metres.
const GLYPH_HEIGHT: f32 = 0.8;
/// Plain carving colour without rune goggles.
const PLAIN_GLYPH: [f32; 3] = [0.78, 0.74, 0.66];

impl Adventure {
    /// The circle's runes carved flat round the inside of its rim, each one appearing as
    /// the ritual reaches it. Through rune goggles each shows its element colour.
    pub(super) fn circle_model(&self, model: &mut Model) {
        let Some(circle) = &self.circles.circle else { return };
        let table = &*runes::TABLE;
        let count = circle.runes.len();
        if count == 0 {
            return;
        }
        let done = if circle.ritual > 0.0 { 1.0 - circle.ritual / circle.ritual_total } else { 1.0 };
        let lit = ((done * count as f32).ceil() as usize).min(count);
        let unit = GLYPH_HEIGHT / engine_content::runes::GLYPH_GRID.1;
        let (half_w, half_h) = (engine_content::runes::GLYPH_GRID.0 * 0.5, engine_content::runes::GLYPH_GRID.1 * 0.5);
        let rim = (circle.radius - GLYPH_HEIGHT * 0.75).max(GLYPH_HEIGHT);
        for (index, &id) in circle.runes.iter().take(lit).enumerate() {
            let turn = index as f32 / count as f32 * std::f32::consts::TAU;
            let out = Vec3::new(turn.cos(), 0.0, turn.sin());
            let along = Vec3::new(-turn.sin(), 0.0, turn.cos());
            let base = circle.centre + out * rim + Vec3::Y * 0.04;
            // Glyph tops point outward, so the phrase reads from the centre.
            let at = |x: f32, y: f32| base + along * (x - half_w) * unit + out * (half_h - y) * unit;
            let color = if self.rune_sight {
                table.color(id).map(|byte| byte as f32 / 255.0)
            } else {
                PLAIN_GLYPH
            };
            for (x0, y0, x1, y1) in table.strokes(id) {
                model.beam(at(x0, y0), at(x1, y1), 0.022, color, 0.9);
            }
        }
    }
}
