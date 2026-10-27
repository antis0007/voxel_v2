//! Presentation springs driven by fixed-step player and equipment state.
use glam::Vec2;

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
    pub sky: f32,
    pub tier: u32,
    pub seed: u32,
    previous_angles: Vec2,
    previous_health: f32,
    idle: f32,
    interaction_time: f32,
    sky_clock: f32,
    sky_target: f32,
    initialized: bool,
}

impl Default for State {
    fn default() -> Self {
        Self { sway: Spring::default(), aim: 0.0, inspect: 0.0, interact: 0.0,
            equip: 0.0, gait: 0.0, speed: 0.0, hit: 0.0, fidget: 0.0, sky: 1.0,
            tier: 0, seed: 0, previous_angles: Vec2::ZERO, previous_health: 100.0,
            idle: 0.0, interaction_time: 0.0, sky_clock: 0.0, sky_target: 1.0, initialized: false }
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
    let state = &mut player.viewmodel;
    let adv = &world.adventure;
    if !state.initialized {
        state.previous_angles = angles;
        state.previous_health = adv.health;
        state.initialized = true;
    }
    let mut turn = angles - state.previous_angles;
    turn.x = (turn.x + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    state.previous_angles = angles;
    let target = (-turn / dt * 0.007).clamp(Vec2::splat(-0.035), Vec2::splat(0.035))
        + if inertia.is_finite() { inertia } else { Vec2::ZERO };
    state.sway.step(target, 16.0, dt);
    approach(&mut state.speed, speed.min(14.0), 10.0, dt);
    state.gait = (state.gait + speed * dt * 1.8).rem_euclid(std::f32::consts::TAU);
    let held = adv.wand > 0 && adv.swing() <= 0.0;
    if adv.wand > 0 {
        if state.seed != adv.loadout.seed && state.tier != 0 { state.equip = 0.0; }
        state.tier = adv.wand;
        state.seed = adv.loadout.seed;
    }
    approach(&mut state.equip, u8::from(held) as f32, 12.0, dt);
    approach(&mut state.aim, u8::from(held && input.cast_spell) as f32, 11.0, dt);
    approach(&mut state.inspect,
        u8::from(held && input.inspect_equipment && speed < 0.3 && !adv.casting()) as f32, 8.0, dt);
    if input.interact { state.interaction_time = 0.30; }
    state.interaction_time = (state.interaction_time - dt).max(0.0);
    approach(&mut state.interact, u8::from(state.interaction_time > 0.0) as f32, 14.0, dt);
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
        let covered = (1..=reach).any(|y| world.get(eye + glam::IVec3::Y*y).is_opaque());
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
}
