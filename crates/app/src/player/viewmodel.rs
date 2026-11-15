//! Presentation springs driven by fixed-step player and equipment state.
use crate::mesh::models::fingers::{self, Fingers};
use glam::Vec2;

pub mod clips;

const HIT_STOP_SECONDS: f32 = 0.05;

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spring {
    pub value: Vec2,
    velocity: Vec2,
}

impl Spring {
    pub fn step(&mut self, target: Vec2, rate: f32, dt: f32) {
        if !target.is_finite() || !dt.is_finite() || dt <= 0.0 { return; }
        let delta = self.value - target;
        let impulse = self.velocity + delta * rate;
        let decay = (-rate * dt).exp();
        self.value = target + (delta + impulse * dt) * decay;
        self.velocity = (self.velocity - impulse * rate * dt) * decay;
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub sway: Spring,
    pub aim: f32,
    pub inspect: f32,
    pub interact: f32,
    pub equip: f32,
    pub gait: f32,
    pub speed: f32,
    pub hit: f32,
    pub fidget: f32,
    /// Seconds into the mining chop clip; 0 at rest.
    #[serde(default)]
    pub mine: f32,
    pub sky: f32,
    pub tier: u32,
    pub seed: u32,
    previous_angles: Vec2,
    previous_health: f32,
    idle: f32,
    interaction_time: f32,
    #[serde(skip)]
    mine_clock: f32,
    /// Break progress (worked, needed) seconds; the tool arc locks to it so the cell breaks on a strike.
    #[serde(skip)]
    pub mine_timing: Option<(f32, f32)>,
    /// Seconds the drawn swing trails the simulated one: hit-stop pauses the clip, then it catches up.
    #[serde(skip)]
    pub swing_lag: f32,
    /// One-shot clip on the held item and its time in clip seconds.
    #[serde(skip)]
    pub clip: Option<(clips::Name, f32)>,
    /// Eased finger openness of the main hand (see `mesh::models::fingers`).
    #[serde(skip)]
    pub fingers: Fingers,
    #[serde(skip)]
    breath: f32,
    #[serde(skip)]
    equip_time: f32,
    #[serde(skip)]
    held_seen: Option<crate::world::inventory::Item>,
    #[serde(skip)]
    hit_stop: f32,
    #[serde(skip)]
    landed_seen: u32,
    sky_clock: f32,
    sky_target: f32,
    initialized: bool,
}

impl Default for State {
    fn default() -> Self {
        Self { sway: Spring::default(), aim: 0.0, inspect: 0.0, interact: 0.0,
            equip: 0.0, gait: 0.0, speed: 0.0, hit: 0.0, fidget: 0.0, mine: 0.0, sky: 1.0,
            tier: 0, seed: 0, previous_angles: Vec2::ZERO, previous_health: 100.0,
            idle: 0.0, interaction_time: 0.0, mine_clock: 0.0, mine_timing: None, swing_lag: 0.0, clip: None, fingers: Fingers::CLOSED, breath: 0.0, equip_time: 0.0, held_seen: None, hit_stop: 0.0, landed_seen: 0, sky_clock: 0.0, sky_target: 1.0, initialized: false }
    }
}

impl State {
    pub(super) fn checkpoint_valid(&self) -> bool {
        self.sway.value.is_finite() && self.sway.velocity.is_finite()
            && self.previous_angles.is_finite()
            && [self.aim, self.inspect, self.interact, self.equip, self.gait, self.speed,
                self.hit, self.fidget, self.mine, self.sky, self.previous_health, self.idle,
                self.interaction_time, self.mine_clock, self.sky_clock, self.sky_target]
                .into_iter().all(f32::is_finite)
    }
}

impl State {
    /// Held-item pose: idle breathing under the active clip, or a benchmark review clip.
    pub fn pose(&self) -> clips::Pose {
        let idle = clips::Name::Idle.clip().sample(self.breath);
        clips::review().or(self.clip).map_or(idle, |(name, t)| idle.then(name.clip().sample(t)))
    }
}

fn approach(value: &mut f32, target: f32, rate: f32, dt: f32) {
    *value += (target - *value) * (1.0 - (-rate * dt).exp());
}

pub fn tick(player: &mut super::Player, world: &crate::world::World,
    input: &super::Input, dt: f32) {
    if !dt.is_finite() || dt <= 0.0 { return; }
    let scale = world.voxel_scale.meters();
    let velocity = player.vel * scale;
    let speed = if player.on_ground { velocity.xz().length() } else { 0.0 };
    // Body inertia: the held kit trails strafes, rises in a fall and dips on a jump.
    let inertia = (Vec2::new(-velocity.dot(player.right_dir()) * 0.0025, -velocity.y * 0.0022))
        .clamp(Vec2::splat(-0.02), Vec2::splat(0.02));
    let angles = Vec2::new(player.yaw, player.pitch);
    if !angles.is_finite() || !speed.is_finite() { return; }
    let eye = player.eye(world.voxel_scale).floor().as_ivec3();
    let held = crate::mesh::models::held_item::held(world, player);
    let sway_rate = crate::mesh::models::held_item::sway_rate(held);
    let adv = &world.adventure;
    let grip_item = if adv.swing() > 0.0 { crate::mesh::models::held_item::swung(world, held) } else { held };
    let grip_target = fingers::review_override()
        .unwrap_or_else(|| fingers::preset(crate::mesh::models::held_item::grip(grip_item)));
    let state = &mut player.viewmodel;
    if !state.initialized {
        state.previous_angles = angles;
        state.previous_health = adv.health;
        state.held_seen = held;
        state.equip_time = clips::Name::Equip.clip().duration;
        state.fingers = grip_target;
        state.initialized = true;
    }
    if state.held_seen != held {
        state.held_seen = held;
        state.equip_time = 0.0;
    }
    state.equip_time = (state.equip_time + dt).min(clips::Name::Equip.clip().duration);
    let mut turn = angles - state.previous_angles;
    turn.x = (turn.x + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    state.previous_angles = angles;
    let target = (-turn / dt * 0.007).clamp(Vec2::splat(-0.035), Vec2::splat(0.035))
        + if inertia.is_finite() { inertia } else { Vec2::ZERO };
    state.sway.step(target * crate::comfort::camera_motion(), sway_rate, dt);
    approach(&mut state.speed, speed.min(14.0), 10.0, dt);
    state.fingers.approach(grip_target, fingers::PRESETS.ease_rate, dt);
    state.gait = (state.gait + speed * dt * 1.8).rem_euclid(std::f32::consts::TAU);
    let wand_held = adv.wand > 0 && adv.swing() <= 0.0;
    if adv.wand > 0 {
        if state.seed != adv.loadout.seed && state.tier != 0 { state.equip = 0.0; }
        state.tier = adv.wand;
        state.seed = adv.loadout.seed;
    }
    approach(&mut state.equip, u8::from(wand_held) as f32, 12.0, dt);
    approach(&mut state.aim, u8::from(wand_held && input.cast_spell) as f32, 11.0, dt);
    approach(&mut state.inspect,
        u8::from(wand_held && input.inspect_equipment && speed < 0.3 && !adv.casting()) as f32, 8.0, dt);
    let use_clip = clips::Name::Use.clip();
    if input.interact { state.interaction_time = use_clip.duration; }
    state.interaction_time = (state.interaction_time - dt).max(0.0);
    approach(&mut state.interact, u8::from(state.interaction_time > 0.0) as f32, 14.0, dt);
    let chop = clips::Name::Chop.clip();
    if (input.break_block || input.break_pressed) && adv.swing() <= 0.0 && !adv.casting() {
        state.mine_clock = match state.mine_timing {
            Some((elapsed, needed)) => crate::world::mining::stroke_phase(elapsed, needed),
            None => (state.mine_clock + dt / chop.duration).rem_euclid(1.0),
        };
        state.mine = state.mine_clock * chop.duration;
    } else {
        state.mine_clock = 0.0;
        // A released stroke plays out instead of snapping back to rest.
        state.mine = if state.mine > 0.0 && state.mine + dt < chop.duration { state.mine + dt } else { 0.0 };
    }
    // Hit-stop: the drawn blade holds at contact for a beat, then catches up with the swing.
    if adv.melee_landed != state.landed_seen {
        state.landed_seen = adv.melee_landed;
        state.hit_stop = HIT_STOP_SECONDS;
    }
    if state.hit_stop > 0.0 { state.hit_stop -= dt; state.swing_lag += dt; }
    else { state.swing_lag *= (-14.0 * dt).exp(); }
    if adv.swing() <= 0.0 { state.swing_lag = 0.0; }
    state.breath = (state.breath + dt).rem_euclid(clips::Name::Idle.clip().duration);
    let equip_clip = clips::Name::Equip.clip();
    state.clip = if adv.swing() > 0.0 {
        let stroke = clips::Name::stroke(adv.backswing(), adv.finisher());
        let elapsed = ((1.0 - adv.swing()) * adv.swing_duration() - state.swing_lag).max(0.0);
        Some((stroke, elapsed * stroke.clip().duration / adv.swing_duration()))
    } else if state.mine > 0.0 {
        Some((clips::Name::Chop, state.mine))
    } else if state.interaction_time > 0.0 {
        Some((clips::Name::Use, use_clip.duration - state.interaction_time))
    } else if state.equip_time < equip_clip.duration {
        Some((clips::Name::Equip, state.equip_time))
    } else { None };
    let loss = (state.previous_health - adv.health).max(0.0);
    state.previous_health = adv.health;
    state.hit = (state.hit * (-12.0 * dt).exp() + loss * 0.025).min(1.0);
    state.idle = if speed > 0.2 || adv.casting() || input.inspect_equipment { 0.0 }
        else { state.idle + dt };
    let fidget_time = (state.idle - 16.0).rem_euclid(32.0);
    state.fidget = if state.idle > 16.0 && fidget_time < 1.8 {
        (fidget_time / 1.8 * std::f32::consts::PI).sin().powi(2)
    } else { 0.0 };
    // A bounded roof query prevents full outdoor ambient on a hand in a cave.
    state.sky_clock -= dt;
    if state.sky_clock <= 0.0 {
        state.sky_clock = 0.125;
        let reach = (24.0 / scale).ceil().clamp(1.0, 96.0) as i32;
        let mut column = world.column_reader(eye.x, eye.z);
        let covered = (1..=reach).any(|dy| column(eye.y + dy).is_opaque());
        state.sky_target = if covered { 0.06 } else { 1.0 };
    }
    approach(&mut state.sky, state.sky_target, 6.0, dt);
}

use glam::Vec3Swizzles;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewmodel_spring_is_partition_independent_and_finite() {
        let target = Vec2::new(0.03, -0.02);
        let mut a = Spring::default();
        let mut b = Spring::default();
        for _ in 0..60 { a.step(target, 16.0, 1.0/60.0); }
        for _ in 0..240 { b.step(target, 16.0, 1.0/240.0); }
        assert!(a.value.distance(b.value) < 1e-6);
        let before = a.value;
        a.step(Vec2::splat(f32::NAN), 16.0, 0.01);
        a.step(target, 16.0, f32::NAN);
        assert_eq!(a.value, before);
    }

    #[test]
    fn mining_animates_immediately_and_settles_on_release() {
        let world = crate::world::World::new(
            crate::world::WorldType::Superflat,
            crate::fluid::VoxelScalePreset::OneMeter,
        );
        let mut player = crate::player::Player::new();
        let mut input = crate::player::Input { break_block: true, ..Default::default() };
        tick(&mut player, &world, &input, 1.0 / 120.0);
        // The chop clip starts on the first tick.
        assert!(player.viewmodel.mine != 0.0);
        input.break_block = false;
        for _ in 0..120 {
            tick(&mut player, &world, &input, 1.0 / 120.0);
        }
        assert!(player.viewmodel.mine.abs() < 1e-5);
    }
}
