//! Optional expeditions layered over the persistent world. Only unbanked
//! resonance and boons are lost on death; factories and discoveries survive.
use super::*;

pub const BOONS: [(&str, &str); 6] = [
    ("Copper edge", "+8% melee damage per stack."),
    ("Prism catalyst", "+20% direct spell-hit damage on named elemental reactions per stack."),
    ("Cinder echo", "Guardian kills pulse for 4 damage per stack within 3 m. Walls block the pulse."),
    ("Moss covenant", "Guardian kills restore 2 health per stack."),
    ("Living dynamo", "+1.5 mana regeneration per second per stack."),
    ("Reclaimer", "+1 unbanked salvage per guardian kill per stack."),
];

#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    pub active: bool,
    pub kills: u32,
    pub salvage: u32,
    pub combat_seconds: f32,
    pub boons: [u32; 6],
    pub pending: u32,
    pub chosen: u32,
    pub completed: u32,
    pub calibration: u32,
}

impl State {
    pub fn stacks(&self, id: usize) -> f32 {
        if self.active { self.boons[id] as f32 } else { 0.0 }
    }
    pub fn pressure(&self) -> u32 {
        if self.active { (self.kills / 8 + (self.combat_seconds / 90.0) as u32).min(5) } else { 0 }
    }
    pub fn incoming(&self) -> f32 {
        if self.active { (1.0 + self.pressure() as f32 * 0.12) * (1.0 - self.calibration as f32 * 0.02) } else { 1.0 }
    }
    pub fn offers(&self) -> Vec<usize> {
        let start = (self.chosen as usize * 2 + self.completed as usize) % BOONS.len();
        (0..BOONS.len()).map(|i| (start + i) % BOONS.len())
            .filter(|&i| self.boons[i] < 5).take(3).collect()
    }
    pub fn end(&mut self) {
        *self = Self { completed: self.completed, calibration: self.calibration, ..Self::default() };
    }
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        for value in [self.active as u32, self.kills, self.salvage, self.combat_seconds.to_bits(),
            self.pending, self.chosen, self.completed, self.calibration].into_iter().chain(self.boons) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    pub fn decode(read: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<Self> {
        let active = read()?;
        let mut state = Self { active: active == 1, kills: read()?, salvage: read()?,
            combat_seconds: f32::from_bits(read()?), pending: read()?, chosen: read()?,
            completed: read()?, calibration: read()?, ..Self::default() };
        for stack in &mut state.boons { *stack = read()?; }
        anyhow::ensure!(active <= 1 && state.kills <= 10_000 && state.salvage <= 60_000
            && state.combat_seconds.is_finite() && (0.0..=450.0).contains(&state.combat_seconds)
            && state.pending <= 30 && state.chosen <= 30 && state.completed <= 1_000_000
            && state.calibration <= 5 && state.boons.iter().all(|&n| n <= 5)
            && state.boons.iter().sum::<u32>() == state.chosen
            && state.pending + state.chosen <= 30, "Invalid resonance expedition");
        anyhow::ensure!(state.active || (state.kills == 0 && state.salvage == 0
            && state.combat_seconds == 0.0 && state.pending == 0 && state.chosen == 0),
            "Inactive resonance expedition has unbanked state");
        anyhow::ensure!(!state.active || (state.pending + state.chosen == (1 + state.kills / 3).min(30)
            && (state.kills..=state.kills * 6).contains(&state.salvage)),
            "Invalid resonance reward ledger");
        Ok(state)
    }
}

#[derive(Clone, Copy)]
pub enum Action { Begin, Bank, Choose(usize), Calibrate }

pub fn action(world: &mut World, at_bench: bool, action: Action) {
    if let Action::Choose(id) = action {
        let run = &mut world.adventure.resonance;
        if run.active && run.pending > 0 && run.offers().contains(&id) {
            run.boons[id] += 1;
            run.pending -= 1;
            run.chosen += 1;
            world.expedition.say(format!("{} attuned, stack {}.", BOONS[id].0, run.boons[id]));
        }
        return;
    }
    if !at_bench {
        world.expedition.say("Resonance expeditions begin and bank at a workbench.");
        return;
    }
    let run = &mut world.adventure.resonance;
    match action {
        Action::Begin if !run.active => {
            if world.expedition.pay(&[(5, 64), (16, 64)]) {
                run.active = true;
                run.pending = 1;
                world.expedition.say("Resonance kindled. Choose a boon in Forge. Bank salvage here before falling.");
            } else {
                world.expedition.say("Kindling requires 1 resonant crystal and 1 induction coil.");
            }
        }
        Action::Bank if run.active => {
            let salvage = run.salvage;
            world.expedition.add(15, salvage * 64);
            world.expedition.add(16, salvage * 32);
            world.expedition.add(17, (salvage / 8) * 64);
            if run.kills > 0 { run.completed = (run.completed + 1).min(1_000_000); }
            run.end();
            world.expedition.say(format!("Banked {salvage} salvage: steel, coil parts and {} cores. Boons released.", salvage / 8));
        }
        Action::Calibrate if !run.active && run.calibration < 5 => {
            let cost = (run.calibration + 1) * 64;
            if world.expedition.pay(&[(12, cost * 2), (16, cost), (17, cost)]) {
                run.calibration += 1;
                world.expedition.say("Resonator calibrated: another 2% damage reduction during expeditions.");
            } else {
                world.expedition.say("Calibration needs circuits, induction coils and arcane cores. See Forge for quantities.");
            }
        }
        _ => {}
    }
}

impl Adventure {
    pub(super) fn resonance_tick(&mut self, feet: Vec3, dt: f32) {
        if self.resonance.active && self.enemies.iter().any(|e| e.health > 0.0
            && e.habitat.is_none() && (e.center() - feet).length_squared() < 24.0 * 24.0) {
            self.resonance.combat_seconds = (self.resonance.combat_seconds + dt).min(450.0);
        }
    }
    pub(super) fn resonance_kills(&mut self, world: &mut World, kills: &[Vec3]) {
        if !self.resonance.active || self.health <= 0.0 { return; }
        for &at in kills {
            let run = &mut self.resonance;
            if run.kills == 10_000 { break; }
            run.kills += 1;
            run.salvage = (run.salvage + 1 + run.boons[5]).min(60_000);
            if run.kills.is_multiple_of(3) && run.pending + run.chosen < 30 {
                run.pending += 1;
                world.expedition.say("A resonance boon is ready. Choose one in Forge, or return to bank your salvage.");
            }
            let healing = run.stacks(3) * 2.0;
            let pulse = run.stacks(2) * 4.0;
            self.health = (self.health + healing).min(self.max_health());
            if healing > 0.0 { self.spawn_voxel_burst(at, PALETTE_OLIVE, 10, 1.8, 0.04); }
            if pulse > 0.0 {
                // Only natural guardians participate: factory habitat stock
                // cannot mint expedition rewards or be damaged by echo chains.
                for enemy in &mut self.enemies {
                    if enemy.health > 0.0 && enemy.habitat.is_none()
                        && (enemy.center() - at).length_squared() < 9.0
                        && line(world, at, enemy.center()) {
                        enemy.hurt(pulse * enemy.affixes.damage_taken());
                        enemy.flash = 0.16;
                    }
                }
                self.blast_visual(at, 3.0, Vec3::Y, PALETTE_COPPER);
                self.spawn_voxel_burst(at, PALETTE_COPPER, 18, 3.2, 0.045);
                sound(&mut self.sounds, Cue::Burst, Some(at), 0.45);
            }
        }
    }
}
