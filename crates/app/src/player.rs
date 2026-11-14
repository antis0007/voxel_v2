use glam::{DVec3, IVec3, Vec2, Vec3};
use winit::keyboard::KeyCode;

pub mod movement;
pub(crate) mod radial;
pub mod viewmodel;
mod climb;
mod checkpoint;
pub mod hazards;
pub mod observations;
pub mod glide;
pub mod traversal;
pub(crate) mod creative_flight;

use crate::keybinds::{self, Action, Binding};

/// The HUD reads the same bindings as input dispatch.
pub fn spell_binding_hint() -> String {
    format!("{} Cast / {} Next", keybinds::hint(Action::Cast), keybinds::hint(Action::NextSpell))
}

use crate::world::{
    AIR_ACCEL, BlockId, FLY_ACCEL, GRAVITY,
    GROUND_ACCEL, GROUND_FRICTION, JUMP_VELOCITY,
    SNEAK_SPEED, SPRINT_SPEED, WALK_SPEED, World, acceleration_blocks, hotbar_block,
    player_eye_height_blocks, resolve_penetration_local, speed_blocks, try_move_local,
    try_step_up_local,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FlyMode {
    Creative,
    Noclip,
}


#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FlightSettings {
    pub speed: f32,
    pub boost: f32,
    pub camera_direction: bool,
    pub land_on_descent: bool,
}
impl Default for FlightSettings {
    fn default() -> Self {
        Self { speed: 10.8, boost: 2.0, camera_direction: false, land_on_descent: true }
    }
}
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sprint: bool,
    pub sneak: bool,
    pub alt_down: bool,
    pub break_block: bool,
    pub place_block: bool,
    /// Keep short clicks until the next fixed interaction step.
    pub break_pressed: bool,
    pub place_pressed: bool,
    pub wheel: f32,
    pub ctrl_down: bool,
    pub pick_block: bool,
    pub interact: bool,
    pub inspect_equipment: bool,
    pub cast_spell: bool,
    /// Retain a quick press/release until gameplay consumes the input frame.
    pub cast_pressed: bool,
    pub cycle_spell: bool,
    pub dash: bool,
    /// Latched like `cast_pressed`; cleared after each fixed gameplay step.
    pub dash_pressed: bool,
    /// Hold-to-zoom; presentation only.
    #[serde(default)]
    pub zoom: bool,
    shift_keys: u8,
    control_keys: u8,
    alt_keys: u8,
    /// Bit `action * SLOTS + slot` per pressed binding, so releasing one of
    /// two keys on an action keeps it held.
    #[serde(default)]
    held: u32,
}

impl Input {
    /// Raw Shift state, whatever sneak is bound to.
    pub fn shift_down(&self) -> bool {
        self.shift_keys != 0
    }

    pub fn keyboard(&mut self, key: winit::keyboard::KeyCode, pressed: bool) {
        let modifier = match key {
            KeyCode::ShiftLeft => Some((&mut self.shift_keys, 1)),
            KeyCode::ShiftRight => Some((&mut self.shift_keys, 2)),
            KeyCode::ControlLeft => Some((&mut self.control_keys, 1)),
            KeyCode::ControlRight => Some((&mut self.control_keys, 2)),
            KeyCode::AltLeft => Some((&mut self.alt_keys, 1)),
            KeyCode::AltRight => Some((&mut self.alt_keys, 2)),
            _ => None,
        };
        if let Some((keys, bit)) = modifier {
            if pressed {
                *keys |= bit;
            } else {
                *keys &= !bit;
            }
        }
        // Ctrl and Alt stay raw modifiers for tool clicks whatever sprint is bound to.
        self.ctrl_down = self.control_keys != 0;
        self.alt_down = self.alt_keys != 0;
        self.binding(Binding::Key(key), pressed);
    }

    pub fn mouse(&mut self, button: winit::event::MouseButton, pressed: bool) {
        self.binding(Binding::Mouse(button), pressed);
    }

    pub fn pad(&mut self, button: keybinds::PadButton, pressed: bool) {
        self.binding(Binding::Pad(button), pressed);
    }

    fn binding(&mut self, binding: Binding, pressed: bool) {
        if pressed {
            keybinds::note_input(binding);
        }
        if let Some((action, slot)) = keybinds::lookup(binding) {
            self.apply(action, slot, pressed);
        }
    }

    fn action_held(&self, action: Action) -> bool {
        let mask = ((1u32 << keybinds::SLOTS) - 1) << (action as usize * keybinds::SLOTS);
        self.held & mask != 0
    }

    /// Applies one binding edge of an input-held action; other actions are dispatched by the window loop.
    pub fn apply(&mut self, action: Action, slot: usize, pressed: bool) {
        if !action.mouse_bindable() || slot >= keybinds::SLOTS {
            return;
        }
        let was = self.action_held(action);
        let bit = 1u32 << (action as usize * keybinds::SLOTS + slot);
        if pressed {
            self.held |= bit;
        } else {
            self.held &= !bit;
        }
        let on = self.action_held(action);
        match action {
            Action::Forward => self.forward = on,
            Action::Back => self.back = on,
            Action::Left => self.left = on,
            Action::Right => self.right = on,
            Action::Jump => self.jump = on,
            Action::Sneak => self.sneak = on,
            Action::Sprint => self.sprint = on,
            Action::Dash => self.press_dash(on),
            Action::Break => {
                self.break_pressed |= on && !self.break_block;
                self.break_block = on;
            }
            Action::Place => {
                self.place_pressed |= on && !self.place_block;
                self.place_block = on;
            }
            Action::Pick => self.pick_block |= on && !was,
            Action::Cast => {
                self.cast_pressed |= on && !self.cast_spell;
                self.cast_spell = on;
            }
            Action::Inspect => self.inspect_equipment = on,
            Action::Zoom => self.zoom = on,
            _ => {}
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Dash key or mouse button edge; a quick tap survives until the next step.
    pub fn press_dash(&mut self, pressed: bool) {
        self.dash_pressed |= pressed && !self.dash;
        self.dash = pressed;
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Player {
    pub pos: Vec3,
    /// Authoritative position; pos is the legacy rounded world-space view.
    pub precise_pos: DVec3,
    pub vel: Vec3,
    /// State in a foreign body's frame while its sphere of influence holds the player; never saved.
    #[serde(skip)]
    pub anchor: Option<crate::planet::influence::Anchor>,
    /// Star-system state in open space between the planets; never saved.
    #[serde(skip)]
    pub void: Option<crate::planet::void::Void>,
    /// Orbital flight state above home's atmosphere; never saved.
    #[serde(skip)]
    pub orbit: Option<crate::planet::orbit::Orbit>,
    pub yaw: f32,
    pub pitch: f32,
    /// Saved local upright frame; yaw and pitch remain player-controlled.
    #[serde(default)]
    pub gravity_frame: glam::Quat,
    /// Transient local cue; terrain edits, not this flag, own the fold state.
    #[serde(skip)]
    pub manyfold_crossing: u8,
    #[serde(skip)]
    pub map_crossings: observations::Crossings,
    #[serde(skip)]
    pub manyfold_feedback_cooldown: f32,
    #[serde(skip)]
    pub manyfold_resonance_harm: f32,
    #[serde(skip)]
    pub manyfold_resonance_cue: f32,
    pub on_ground: bool,
    pub sprinting: bool,
    pub selected_block: BlockId,
    pub scale: f32,

    pub flying: bool,
    pub fly_mode: FlyMode,
    #[serde(default)]
    pub flight: FlightSettings,
    /// Saved and replicated charge; loading in midair cannot refill it.
    #[serde(default)]
    pub levitation_seconds: f32,
    pub jump_was_down: bool,
    pub jump_tap_timer: f32,
    pub crouch_t: f32,
    /// Camera-only vertical settling after the body steps onto a voxel ledge, in meters.
    pub step_camera_offset: f32,
    /// Velocity of the critically damped ledge camera spring, in meters/second.
    step_camera_velocity_meters: f32,
    /// Physical camera bob driven by distance traveled while grounded.
    walk_phase: f32,
    walk_bob_meters: f32,
    pub swimming: bool,
    pub underwater: bool,
    /// Seconds spent with the head under water since the last breath.
    pub air_seconds: f32,
    /// Damage this step earned from the environment, not yet applied to health.
    pub pending_damage: f32,
    /// Transient ward for the next damaging landing.
    #[serde(skip)]
    pub(crate) fall_ward_seconds: f32,
    /// Seconds the head has spent unlit in the Lightless; the Hush closes in.
    #[serde(skip)]
    pub dark_seconds: f32,
    /// What earned `pending_damage`; transient, so checkpoints skip it.
    #[serde(skip)]
    pub pending_cause: Option<crate::world::adventure::death::Cause>,
    /// Downward speed captured on the transition into water. Adventure drains
    /// this once to emit a splash at the true voxel surface.
    pub water_impact: f32,
    /// Sticky surface-exit flag, drained by the next Adventure update.
    pub water_exit: bool,
    jump_buffer: f32,
    coyote_time: f32,
    /// Slide, dash, air jumps and the view kick they drive.
    pub movement: movement::Movement,
    pub viewmodel: viewmodel::State,
    /// Traversal unlocked by worn gear. Set from the inventory each fixed step
    /// (`simulation::fixed`); creative grants everything.
    pub gear: crate::world::inventory::Mobility,
    /// Traversal granted regardless of gear (tests and review drivers).
    pub gear_override: crate::world::inventory::Mobility,
    /// Glide frame: offered by worn gear, opened with a jump while falling.
    #[serde(skip)]
    pub glide: glide::Glide,
    #[serde(default)]
    pub traversal: traversal::Traversal,
}

/// Breath and drowning, in seconds and health per second.
const AIR_SECONDS: f32 = 12.0;
const DROWN_DAMAGE_PER_S: f32 = 4.0;
const AIR_RECOVER_PER_S: f32 = 12.0;

impl Player {
    /// Breath and drowning. Damage is only accumulated here, never applied, so the
    /// caller owns health on both the single-player and the host path.
    fn update_air(&mut self, dt: f32) {
        self.update_air_with_rate(dt, 1.0);
    }

    fn update_air_with_rate(&mut self, dt: f32, consumption: f32) {
        // Creative immunity follows mode even while walking or swimming.
        if self.underwater && !self.flying {
            // Capped so surfacing refills in at most one second.
            self.air_seconds = (self.air_seconds + dt * consumption).min(AIR_SECONDS);
            if self.air_seconds >= AIR_SECONDS {
                self.harm(DROWN_DAMAGE_PER_S * dt, crate::world::adventure::death::Cause::Drowned);
            }
        } else {
            self.air_seconds = (self.air_seconds - AIR_RECOVER_PER_S * dt).max(0.0);
        }
    }

    /// Hands the caller this step's environmental damage and clears it.
    pub fn take_pending_damage(&mut self) -> f32 {
        self.take_pending_harm().map_or(0.0, |(amount, _)| amount)
    }

    pub fn new() -> Self {
        Self {
            pos: Vec3::new(0.0, 6.0, 0.0),
            precise_pos: DVec3::new(0.0, 6.0, 0.0),
            vel: Vec3::ZERO,
            anchor: None,
            void: None,
            orbit: None,
            yaw: 0.0,
            pitch: 0.0,
            gravity_frame: glam::Quat::IDENTITY,
            manyfold_crossing: 0,
            map_crossings: Default::default(),
            manyfold_feedback_cooldown: 0.0,
            manyfold_resonance_harm: 0.0,
            manyfold_resonance_cue: 0.0,
            on_ground: false,
            sprinting: false,
            selected_block: hotbar_block(0).unwrap_or(BlockId::Grass),
            scale: 1.0,
            flying: false,
            fly_mode: FlyMode::Creative,
            flight: FlightSettings::default(),
            levitation_seconds: 0.0,
            jump_was_down: false,
            jump_tap_timer: 0.0,
            air_seconds: 0.0,
            pending_damage: 0.0,
            fall_ward_seconds: 0.0,
            dark_seconds: 0.0,
            pending_cause: None,
            crouch_t: 0.0,
            step_camera_offset: 0.0,
            step_camera_velocity_meters: 0.0,
            walk_phase: 0.0,
            walk_bob_meters: 0.0,
            swimming: false,
            underwater: false,
            water_impact: 0.0,
            water_exit: false,
            jump_buffer: 0.0,
            coyote_time: 0.0,
            movement: movement::Movement::new(),
            viewmodel: viewmodel::State::default(),
            gear: crate::world::inventory::Mobility::default(),
            gear_override: crate::world::inventory::Mobility::default(),
            glide: glide::Glide::default(),
            traversal: traversal::Traversal::default(),
        }
    }

    /// Gait phase in radians; footsteps fall twice per turn.
    pub fn walk_phase(&self) -> f32 {
        self.walk_phase
    }

    pub fn up_dir(&self) -> Vec3 { self.gravity_frame * Vec3::Y }

    pub fn bounds(&self, scale: crate::fluid::VoxelScalePreset) -> (Vec3, Vec3) {
        let r = crate::world::player_width_blocks(scale, self.scale) * 0.5;
        let height = crate::world::player_height_blocks(scale, self.scale)
            - 0.3 * self.scale * self.crouch_t / scale.meters();
        let a = self.pos + self.up_dir() * r;
        let b = self.pos + self.up_dir() * (height - r).max(r);
        (a.min(b) - Vec3::splat(r), a.max(b) + Vec3::splat(r))
    }

    pub fn forward_dir(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.gravity_frame * Vec3::new(sy * cp, sp, -cy * cp).normalize_or_zero()
    }

    /// Dominant axis of the look direction along the floor.
    pub fn look_axis(&self) -> glam::IVec3 {
        let up = self.up_dir();
        let forward = self.forward_dir();
        let tangent = forward - up * forward.dot(up);
        let axis = if tangent.length_squared() > 1e-4 { tangent } else { forward };
        let ax = axis.abs();
        if ax.y >= ax.x && ax.y >= ax.z {
            glam::IVec3::new(0, if axis.y < 0.0 { -1 } else { 1 }, 0)
        } else if ax.x >= ax.z {
            glam::IVec3::new(if axis.x < 0.0 { -1 } else { 1 }, 0, 0)
        } else {
            glam::IVec3::new(0, 0, if axis.z < 0.0 { -1 } else { 1 })
        }
    }

    pub fn flat_forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        self.gravity_frame * Vec3::new(sy, 0.0, -cy).normalize_or_zero()
    }

    pub fn right_dir(&self) -> Vec3 {
        let f = self.flat_forward();
        f.cross(self.up_dir()).normalize_or_zero()
    }

    pub fn precise_eye(&self, voxel_scale: crate::fluid::VoxelScalePreset) -> DVec3 {
        let position = if self.precise_pos.as_vec3() == self.pos {
            self.precise_pos
        } else {
            self.pos.as_dvec3()
        };
        let meters_per_voxel = voxel_scale.meters().max(1e-6);
        let height = player_eye_height_blocks(voxel_scale, self.scale)
            - 0.25 * self.scale * self.crouch_t / voxel_scale.meters().max(1e-6)
            + (self.step_camera_offset + self.walk_bob_meters) / meters_per_voxel;
        position + self.up_dir().as_dvec3() * height as f64
    }

    pub fn eye(&self, voxel_scale: crate::fluid::VoxelScalePreset) -> Vec3 {
        let base_eye = player_eye_height_blocks(voxel_scale, self.scale);
        let meters_per_voxel = voxel_scale.meters().max(1e-6);
        let crouch_drop = 0.25 * self.scale / voxel_scale.meters().max(1e-6);
        let current_eye = base_eye - crouch_drop * self.crouch_t;
        self.pos
            + self.up_dir()
                * (current_eye
                    + (self.step_camera_offset + self.walk_bob_meters) / meters_per_voxel)
    }
}

pub struct FpsCounter {
    acc: f32,
    frames: u32,
    fps: f32,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            acc: 0.0,
            frames: 0,
            fps: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.acc += dt;
        self.frames += 1;
        if self.acc >= 0.25 {
            self.fps = self.frames as f32 / self.acc.max(1e-6);
            self.acc = 0.0;
            self.frames = 0;
        }
    }

    pub fn fps_rounded(&self) -> u32 {
        self.fps.round().max(0.0) as u32
    }
}

fn update_flight_toggle(player: &mut Player, input: &Input, dt: f32) {
    if player.jump_tap_timer > 0.0 {
        player.jump_tap_timer = (player.jump_tap_timer - dt).max(0.0);
    }

    let jump_pressed_now = input.jump && !player.jump_was_down;
    if jump_pressed_now {
        if player.jump_tap_timer > 0.0 {
            player.flying = !player.flying;
            player.jump_tap_timer = 0.0;
            if player.flying {
                player.vel -= player.up_dir() * player.vel.dot(player.up_dir());
            }
        } else {
            player.jump_tap_timer = 0.25;
        }
    }

    player.jump_was_down = input.jump;
}

pub fn update_player(player: &mut Player, input: &mut Input, world: &World, dt: f32) {
    if !player.pos.is_finite() || !dt.is_finite() || dt <= 0.0 {
        return;
    }
    player.fall_ward_seconds = (player.fall_ward_seconds - dt).max(0.0);
    player.glide.drift_m += glam::DVec2::new(f64::from(player.glide.wind.x), f64::from(player.glide.wind.z)) * f64::from(dt);
    player.sprinting = input.sprint && input.forward && !input.sneak && !player.flying;
    // Existing teleports write pos. Reconcile once, then integrate only locally.
    if player.precise_pos.as_vec3() != player.pos {
        player.precise_pos = player.pos.as_dvec3();
        player.anchor = None;
    }
    let cfg = world.sample_config();
    if !cfg.is_planet() {
        player.anchor = None;
    }
    if cfg.is_hell() {
        limit_hell_ascent(player, cfg);
    }
    if !cfg.is_hell() && ((world.world_type == crate::world::WorldType::Manyfold
        && !engine_worldgen::manyfold::flat_maze(world.generator_version))
        || crate::world::orbital::habitat::acceleration(world, player.pos * world.voxel_scale.meters()).is_some())
    {
        radial::update(player, input, world, dt);
        return;
    }
    player.gravity_frame = glam::Quat::IDENTITY;
    if cfg.is_planet()
        && (crate::planet::void::step(player, input, world, dt)
            || crate::planet::orbit::step(player, input, world, dt)
            || crate::planet::influence::step(player, input, world, dt))
    {
        return;
    }
    let origin = (player.precise_pos / 32.0).floor().as_ivec3() * 32;
    player.pos = (player.precise_pos - origin.as_dvec3()).as_vec3();
    update_player_local(player, input, world, dt, origin);
    player.precise_pos = origin.as_dvec3() + player.pos.as_dvec3();
    if cfg.is_planet() {
        crate::planet::cross_face(player, world.voxel_scale.meters() as f64);
        if !player.flying && !player.on_ground && !player.swimming {
            let radius = engine_core::cube_sphere::atlas::RADIUS_M as f32;
            let altitude = (player.precise_pos.y * world.voxel_scale.meters() as f64) as f32;
            // Real 9.81 at ground level made every airborne frame on a planet
            // float: jumps tuned for GRAVITY rose almost three times higher
            // than on flat worlds. Keep movement gravity near the ground and
            // hand over to the home body's 9.81 profile (flight HUD, orbits)
            // between 300 m and 3 km.
            let lift = altitude.max(0.0);
            let t = ((lift - 300.0) / 2_700.0).clamp(0.0, 1.0);
            let surface = GRAVITY + (9.81 - GRAVITY) * t * t * (3.0 - 2.0 * t);
            let gravity = surface * (radius / (radius + lift)).powi(2);
            player.vel.y += acceleration_blocks(world.voxel_scale, GRAVITY - gravity) * dt;
        }
    }
    if cfg.is_hell() {
        limit_hell_ascent(player, cfg);
    }
    player.pos = player.precise_pos.as_vec3();
}

fn limit_hell_ascent(player: &mut Player, cfg: engine_voxel::WorldSampleConfig) {
    // Beyond the outermost orbit, keep pilots in a bounded navigation volume.
    let max_y = 55_000.0 / f64::from(cfg.voxel_meters());
    if player.precise_pos.y >= max_y {
        player.precise_pos.y = max_y;
        player.pos.y = max_y as f32;
        player.vel.y = player.vel.y.min(0.0);
    }
}
fn update_player_local(
    player: &mut Player,
    input: &mut Input,
    world: &World,
    dt: f32,
    origin: IVec3,
) {
    if dt <= 0.0 {
        return;
    }
    // Recover safely from a body that is already inside geometry (newly solid
    // foliage, a save from an older tree, or a streaming edit). This is a
    // no-op in the common case and runs before movement can make it worse.
    if !(player.flying && matches!(player.fly_mode, FlyMode::Noclip))
        && resolve_penetration_local(
            origin,
            world,
            player.scale,
            player.crouch_t,
            &mut player.pos,
        )
    {
        player.vel.y = player.vel.y.max(0.0);
        player.on_ground = false;
    }
    // The collision body stays on the exact voxel surface. The view follows
    // its ledge displacement with an analytic critically damped spring in
    // meters, independent of voxel size and frame rate.
    let spring_omega = 20.0;
    let offset = player.step_camera_offset;
    let velocity = player.step_camera_velocity_meters;
    let c = velocity + spring_omega * offset;
    let decay = (-spring_omega * dt).exp();
    player.step_camera_offset = (offset + c * dt) * decay;
    player.step_camera_velocity_meters = (velocity - spring_omega * c * dt) * decay;

    let immersed = |point: Vec3| {
        let cell = point.floor().as_ivec3();
        point.y - (cell.y as f32) < world.water_fill(cell + origin)
    };
    let body_scale = player.scale / world.voxel_scale.meters();
    let was_swimming = player.swimming;
    let was_underwater = player.underwater;
    player.underwater = immersed(player.eye(world.voxel_scale));
    if world.adventure.creative {
        player.air_seconds = 0.0;
    } else {
        if world.adventure.discoveries & (1 << 9) != 0 {
            player.update_air_with_rate(dt, 0.5);
        } else {
            player.update_air(dt);
        }
    }
    hazards::contact(player, world, origin, dt);
    hazards::darkness(player, world, origin, dt);
    player.swimming = !player.flying && immersed(player.pos + Vec3::Y * (0.65 * body_scale));
    let horizontal_speed_m =
        Vec2::new(player.vel.x, player.vel.z).length() * world.voxel_scale.meters();
    let walking =
        player.on_ground && !player.flying && !player.swimming && horizontal_speed_m > 0.16;
    if walking {
        player.walk_phase =
            (player.walk_phase + dt * horizontal_speed_m * 2.3).rem_euclid(std::f32::consts::TAU);
    }
    let stride_weight = if walking {
        (horizontal_speed_m / 1.8).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let target_walk_bob = 0.018
        * stride_weight
        * (player.walk_phase * 2.0).sin()
        * crate::comfort::camera_motion();
    player.walk_bob_meters +=
        (target_walk_bob - player.walk_bob_meters) * (1.0 - (-dt * 14.0).exp());
    let fresh_jump = input.jump && !player.jump_was_down;
    let fresh_sneak = input.sneak && !player.movement.sneak_was_down;
    player.movement.sneak_was_down = input.sneak;
    // Air jump: a fresh press in the air once the double-tap flight window has
    // closed, so a quick double tap still toggles flight. The press then does
    // not open a new flight window (no accidental flight on a triple tap).
    let air_jump = fresh_jump
        && !player.flying
        && !player.on_ground
        && player.coyote_time <= 0.0
        && player.jump_tap_timer - dt <= 0.0
        && player.gear.thrusters
        && player.movement.air_jump_left()
        && !player.movement.wallrunning
        && !player.swimming;
    // Flight is a movement mode, including while immersed. Swimming must not
    // consume/reset the double-tap before the mode switch can see it.
    // This is the current actor's mode, including per-player server grants.
    if !world.rules.mode.interacts() {
        // Spectators stay airborne; the caller pins noclip each step.
    } else if world.adventure.creative {
        update_flight_toggle(player, input, dt);
    } else {
        player.flying = false;
        player.jump_tap_timer = 0.0;
        player.jump_was_down = input.jump;
    }
    let levitation = world.adventure.discoveries & (1 << 8) != 0;
    if !levitation {
        player.levitation_seconds = 0.0;
    } else if player.on_ground && !player.swimming {
        player.levitation_seconds = (player.levitation_seconds + dt * 0.75).min(1.5);
    }
    if air_jump {
        player.jump_tap_timer = 0.0;
    }
    let walking_mode = !player.flying && !player.swimming;
    player.movement.settle(walking_mode, dt);
    player.swimming = !player.flying && immersed(player.pos + Vec3::Y * (0.65 * body_scale));
    if player.swimming && !was_swimming {
        // Capture the incoming vertical speed before the swimming branch damps
        // it. A deliberate walk-in still gets a minimum contact splash.
        player.water_impact = player.water_impact.max((-player.vel.y).max(0.35));
        // Water breaks the fall.
        player.movement.fall_speed = 0.0;
    }
    if was_underwater && !player.underwater {
        player.water_exit = true;
    }
    player.jump_buffer = if fresh_jump {
        0.12
    } else {
        (player.jump_buffer - dt).max(0.0)
    };
    player.coyote_time = if player.on_ground {
        0.09
    } else {
        (player.coyote_time - dt).max(0.0)
    };
    let target_crouch = if input.sneak && !player.flying && !player.swimming {
        1.0
    } else {
        0.0
    };
    let next_crouch =
        player.crouch_t + (target_crouch - player.crouch_t) * (1.0 - (-dt * 15.0).exp());
    let (min, max) =
        crate::world::player_aabb(player.pos, world.voxel_scale, player.scale, next_crouch);
    if next_crouch >= player.crouch_t
        || !crate::world::collides_world_local(world, min, max, origin)
    {
        player.crouch_t = next_crouch;
    }

    if player.swimming {
        // A fresh jump press gives a decisive surface-breaking kick. Holding
        // jump then keeps the swimmer rising until their body clears the water.
        if fresh_jump {
            player.vel.y = player.vel.y.max(speed_blocks(world.voxel_scale, 8.5));
        }
        let mut wish = Vec3::ZERO;
        if input.forward {
            wish += player.forward_dir();
        }
        if input.back {
            wish -= player.flat_forward();
        }
        if input.right {
            wish += player.right_dir();
        }
        if input.left {
            wish -= player.right_dir();
        }
        if input.jump {
            wish.y += 1.0;
        }
        if input.sneak {
            wish.y -= 1.0;
        }
        let speed = speed_blocks(world.voxel_scale, if input.sprint { 6.0 } else { 4.0 });
        let target = wish.normalize_or_zero() * speed;
        player.vel = player.vel.lerp(target, 1.0 - (-8.0 * dt).exp());
        // Water drag and buoyancy prevent an idle swimmer from sinking like a
        // stone, while sneak remains an intentional dive control.
        player.vel.y +=
            acceleration_blocks(world.voxel_scale, if input.sneak { -4.0 } else { 3.0 }) * dt;
        player.vel.y = player.vel.y.clamp(
            -speed_blocks(world.voxel_scale, 5.0),
            speed_blocks(world.voxel_scale, 9.0),
        );
        // Preserve upward momentum through the surface instead of damping the
        // kick back toward a diagonally-normalized swim speed each frame.
        if input.jump && !input.sneak {
            player.vel.y = player.vel.y.max(speed_blocks(world.voxel_scale, 8.0));
        }
        player.on_ground = false;
        try_move_local(
            origin,
            world,
            player.scale,
            player.crouch_t,
            &mut player.pos,
            player.vel * dt,
            &mut player.on_ground,
            false,
        );
        player.jump_buffer = 0.0;
        return;
    }

    if player.flying {
        let fly_accel = FLY_ACCEL;
        creative_flight::migrate(&mut player.flight);
        let altitude = (player.pos.y + origin.y as f32) * world.voxel_scale.meters();
        let planetary = world.sample_config().is_planet();
        let creative_planet = world.rules.mode.flight() && planetary;
        let cruise_altitude = if planetary { altitude } else { 0.0 };
        let meters_per_second = creative_flight::speed(&player.flight, input, cruise_altitude);
        let move_speed = speed_blocks(world.voxel_scale, meters_per_second);

        let mut wish = Vec3::ZERO;
        let forward = if !player.flight.camera_direction && matches!(player.fly_mode, FlyMode::Creative) {
            player.flat_forward()
        } else {
            player.forward_dir()
        };
        if input.forward {
            wish += forward;
        }
        if input.back {
            wish -= forward;
        }
        if input.right {
            wish += player.right_dir();
        }
        if input.left {
            wish -= player.right_dir();
        }
        if input.jump {
            wish += Vec3::Y;
        }
        if input.sneak {
            wish -= Vec3::Y;
        }
        wish = wish.normalize_or_zero();

        let target = if planetary {
            let point_m = (origin.as_dvec3() + player.pos.as_dvec3()) * f64::from(world.voxel_scale.meters());
            crate::planet::cruise_velocity(point_m, wish * move_speed)
        } else { wish * move_speed };
        player.vel = player.vel.lerp(target, (fly_accel * dt).min(1.0));

        if creative_planet && altitude < crate::altitude::LOAD_STOP_M as f32 {
            let scale = world.voxel_scale.meters();
            player.vel = creative_flight::approach(player.vel * scale, Vec3::Y,
                altitude - engine_voxel::WORLD_MAX_Y_METERS, &player.flight, input) / scale;
        }

        let near_structure = creative_planet && matches!(player.fly_mode, FlyMode::Creative)
            && altitude > engine_voxel::WORLD_MAX_Y_METERS + 8.0
            && creative_flight::structure_on_path(world, origin.as_dvec3() + player.pos.as_dvec3(),
                origin.as_dvec3() + (player.pos + player.vel * dt).as_dvec3());
        if near_structure {
            // Dock at local building speed instead of repeatedly sweeping a
            // kilometre-per-second cruise command into a station's hull.
            let docking_speed = creative_flight::speed(&player.flight, input, 0.0);
            player.vel = player.vel.clamp_length_max(speed_blocks(world.voxel_scale, docking_speed));
        }
        if planetary && altitude > engine_voxel::WORLD_MAX_Y_METERS + 8.0 && !near_structure {
            player.pos += player.vel * dt;
            player.on_ground = false;
            return;
        }

        match player.fly_mode {
            FlyMode::Creative => {
                player.on_ground = false;
                let horiz_delta = Vec3::new(player.vel.x * dt, 0.0, player.vel.z * dt);
                try_move_local(
                    origin,
                    world,
                    player.scale,
                    player.crouch_t,
                    &mut player.pos,
                    horiz_delta,
                    &mut player.on_ground,
                    false,
                );
                try_move_local(
                    origin,
                    world,
                    player.scale,
                    player.crouch_t,
                    &mut player.pos,
                    Vec3::new(0.0, player.vel.y * dt, 0.0),
                    &mut player.on_ground,
                    false,
                );
                if player.on_ground && input.sneak && player.flight.land_on_descent {
                    player.flying = false;
                    player.vel.y = 0.0;
                }
            }
            FlyMode::Noclip => {
                player.pos += player.vel * dt;
                player.on_ground = false;
            }
        }
        return;
    }

    if traversal::octo_step(player, input, world, dt, origin, fresh_jump) {
        player.jump_buffer = 0.0;
        return;
    }

    // A fresh jump while falling opens the frame (after any air jump); another closes it.
    if !player.glide.available || player.on_ground || player.swimming {
        player.glide.deployed = false;
    } else if fresh_jump && !air_jump && player.coyote_time <= 0.0 && (player.glide.deployed || player.vel.y < 0.0) {
        player.glide.deployed = !player.glide.deployed;
    }
    if player.glide.deployed {
        let m = world.voxel_scale.meters();
        let vel = glide::step(player.vel * m, glide::glide_air(player.glide.wind), player.yaw, player.pitch, GRAVITY, dt);
        player.vel = vel / m;
        // Landing damage follows the sink rate, not the fall that preceded opening.
        player.movement.fall_speed = (-vel.y).max(0.0);
        player.on_ground = false;
        for delta in [Vec3::new(player.vel.x, 0.0, player.vel.z) * dt, Vec3::Y * (player.vel.y * dt)] {
            let before = player.pos;
            try_move_local(origin, world, player.scale, player.crouch_t, &mut player.pos, delta, &mut player.on_ground, false);
            // A blocked axis loses its speed; the frame does not slide along walls at full speed.
            let moved = player.pos - before;
            if delta.x.abs() > 1e-6 && (moved.x / delta.x) < 0.5 { player.vel.x = 0.0; }
            if delta.z.abs() > 1e-6 && (moved.z / delta.z) < 0.5 { player.vel.z = 0.0; }
            if delta.y.abs() > 1e-6 && (moved.y / delta.y) < 0.5 { player.vel.y = 0.0; }
        }
        if player.on_ground {
            player.glide.deployed = false;
            if player.movement.fall_speed >= movement::HARD_LANDING_SPEED {
                player.movement.raise(movement::EVENT_LAND);
            }
            if !world.adventure.creative && !player.traversal.slime_boots {
                player.harm(hazards::fall_damage(player.movement.fall_speed), crate::world::adventure::death::Cause::Fall);
            }
            traversal::bounce(player, input, world);
            player.movement.fall_speed = 0.0;
        }
        player.jump_buffer = 0.0;
        return;
    }

    let walk_speed = speed_blocks(world.voxel_scale, WALK_SPEED);
    let sprint_speed = speed_blocks(world.voxel_scale, SPRINT_SPEED);
    let sneak_speed = speed_blocks(world.voxel_scale, SNEAK_SPEED);
    let ground_accel = GROUND_ACCEL;
    let air_accel = AIR_ACCEL;
    let gravity = acceleration_blocks(world.voxel_scale, GRAVITY);
    let jump_velocity =
        speed_blocks(world.voxel_scale, JUMP_VELOCITY) * world.progression.effects.jump_factor();
    let underfoot = world.get(
        (player.pos - Vec3::Y * (0.05 / world.voxel_scale.meters()))
            .floor()
            .as_ivec3()
            + origin,
    );
    let grip = if underfoot == BlockId::Ice { 0.08 } else { 1.0 };
    let terrain_speed = if underfoot == BlockId::Mud { 0.62 } else { 1.0 };
    let move_speed = (if input.sneak {
        sneak_speed
    } else if input.sprint && input.forward {
        sprint_speed
    } else {
        walk_speed
    }) * terrain_speed
        * world.progression.effects.speed_factor();

    let mut wish = Vec3::ZERO;
    if input.forward {
        wish += player.flat_forward();
    }
    if input.back {
        wish -= player.flat_forward();
    }
    if input.right {
        wish += player.right_dir();
    }
    if input.left {
        wish -= player.right_dir();
    }
    wish = wish.normalize_or_zero();

    let accel = if player.on_ground {
        ground_accel * grip
    } else {
        air_accel
    };
    let target = wish * move_speed;
    let horiz = Vec2::new(player.vel.x, player.vel.z);
    let desired = Vec2::new(target.x, target.z);
    let speed_of = |m: f32| speed_blocks(world.voxel_scale, m);
    let look_flat = player.flat_forward();
    let side = player.right_dir();
    let gear = player.gear;
    let (body_scale, crouch_t) = (player.scale, player.crouch_t);
    // Bunny hop (Quake): a jump queued for the landing frame skips that
    // frame's ground friction, so timed hops keep their speed. Boots only.
    let hop = gear.wallrun && player.jump_buffer > 0.0;
    let mv = &mut player.movement;
    mv.tick(player.on_ground, dt);

    // Slide: sneak pressed while grounded and fast. It replaces steering with a
    // low-friction glide that turns slowly toward the held direction.
    let speed_now = horiz.length();
    if !mv.sliding
        && fresh_sneak
        && player.on_ground
        && mv.slide_cooldown <= 0.0
        && !mv.dashing()
        && speed_now >= speed_of(movement::SLIDE_MIN_SPEED)
    {
        mv.start_slide();
        let boosted = (speed_now + speed_of(movement::SLIDE_BOOST))
            .min(speed_of(movement::SLIDE_MAX_SPEED).max(speed_now));
        player.vel.x = horiz.x / speed_now * boosted;
        player.vel.z = horiz.y / speed_now * boosted;
    } else if mv.sliding
        && (!input.sneak
            || mv.slide_time >= movement::SLIDE_MAX_TIME
            || speed_now < speed_of(movement::SLIDE_END_SPEED))
    {
        mv.end_slide();
    }

    // Dash: a short burst along the held direction (look direction when none),
    // on the ground or once in the air. An air dash holds height while it runs.
    if input.dash_pressed && gear.thrusters && mv.can_dash() {
        mv.end_wallrun();
        let dir = if wish.length_squared() > 1e-6 { wish } else { look_flat };
        let dir = Vec2::new(dir.x, dir.z).normalize_or_zero();
        let along = horiz.dot(dir).max(0.0);
        let speed = speed_of(movement::DASH_SPEED).max(along);
        mv.start_dash(!player.on_ground);
        player.vel.x = dir.x * speed;
        player.vel.z = dir.y * speed;
        if mv.air_dash {
            player.vel.y = 0.0;
        }
    }

    // Wall run (Titanfall): boots, airborne, forward held, fast and not
    // dropping hard, with a solid wall within reach on the left or right.
    let probe = movement::WALL_PROBE / world.voxel_scale.meters();
    let wall_at = |pos: Vec3, dir: Vec3| {
        let (lo, hi) = crate::world::player_aabb(pos, world.voxel_scale, body_scale, crouch_t);
        let shift = dir * probe;
        // Mid-body only: a knee-high step or a lintel is not a wall.
        let lift = (hi.y - lo.y) * 0.25;
        crate::world::collides_world_local(
            world,
            lo + shift + Vec3::Y * lift,
            hi + shift - Vec3::Y * lift,
            origin,
        )
    };
    let speed_now = Vec2::new(player.vel.x, player.vel.z).length();
    if mv.wallrunning {
        let n = mv.wall_normal;
        let still_wall = wall_at(player.pos, Vec3::new(-n.x, 0.0, -n.y));
        if !still_wall
            || !input.forward
            || mv.wallrun_time >= movement::WALLRUN_MAX_TIME
            || speed_now < speed_of(movement::WALLRUN_END_SPEED)
        {
            mv.end_wallrun();
        }
    } else if gear.wallrun
        && !player.on_ground
        && input.forward
        && mv.wallrun_cooldown <= 0.0
        && !mv.dashing()
        && speed_now >= speed_of(movement::WALLRUN_MIN_SPEED)
        && player.vel.y > -speed_of(movement::WALLRUN_MAX_FALL)
    {
        for sign in [1.0f32, -1.0] {
            let dir = side * sign;
            if wall_at(player.pos, dir) {
                mv.start_wallrun(Vec2::new(-dir.x, -dir.z));
                player.vel.y = player.vel.y.max(speed_of(movement::WALLRUN_LIFT));
                break;
            }
        }
    }
    // Lean away from the wall; ease back upright otherwise.
    let roll_target = if mv.wallrunning {
        let n = Vec3::new(mv.wall_normal.x, 0.0, mv.wall_normal.y);
        -n.dot(side) * movement::WALLRUN_ROLL
    } else {
        0.0
    };
    mv.ease_roll(roll_target, dt);

    if mv.dashing() {
        // Velocity is held for the whole burst.
    } else if mv.wallrunning {
        // Run along the wall: velocity follows the wall tangent nearest the
        // look direction and climbs toward run speed. Never slower on entry.
        let n = mv.wall_normal;
        let tangent = Vec2::new(-n.y, n.x);
        let look = Vec2::new(look_flat.x, look_flat.z);
        let tangent = if tangent.dot(look) < 0.0 { -tangent } else { tangent };
        let h = Vec2::new(player.vel.x, player.vel.z);
        let along = h.dot(tangent).max(0.0);
        let target = speed_of(movement::WALLRUN_SPEED).max(along);
        let speed = along + (target - along) * (movement::WALLRUN_ACCEL * dt).min(1.0);
        // A small pull toward the wall keeps contact on curved voxel faces.
        let h = tangent * speed - n * speed_of(0.5);
        player.vel.x = h.x;
        player.vel.z = h.y;
    } else if mv.sliding {
        let friction = movement::SLIDE_FRICTION / grip.max(0.08).sqrt();
        let damp = (-friction * dt).exp();
        let mut h = Vec2::new(player.vel.x, player.vel.z) * damp;
        let speed = h.length();
        if desired.length_squared() > 1e-6 && speed > 1e-6 {
            let turned = h.lerp(desired.normalize() * speed, (movement::SLIDE_STEER * dt).min(1.0));
            h = turned.normalize_or_zero() * speed;
        }
        player.vel.x = h.x;
        player.vel.z = h.y;
    } else if hop && player.on_ground {
        // Landing frame of a queued hop: no friction, no decay this step.
    } else if player.on_ground && wish.length_squared() < 1e-6 {
        let damp = (-GROUND_FRICTION * grip * dt).exp();
        player.vel.x = horiz.x * damp;
        player.vel.z = horiz.y * damp;
    } else {
        // Above the current move speed (after a dash or slide-jump) speed is
        // carried and bleeds off; at or below it this is the old steering lerp.
        let decay = acceleration_blocks(
            world.voxel_scale,
            if player.on_ground {
                movement::GROUND_MOMENTUM_DECAY * grip
            } else {
                movement::AIR_MOMENTUM_DECAY
            },
        );
        let speed = horiz.length();
        let new_horiz = if gear.wallrun
            && !player.on_ground
            && speed > move_speed
            && desired.length_squared() > 1e-6
        {
            // Quake / Source air strafing: no decay, and acceleration only
            // along the unclaimed part of the wish direction.
            movement::air_accelerate(
                horiz,
                desired.normalize(),
                speed_of(movement::AIR_STRAFE_WISH),
                movement::AIR_STRAFE_ACCEL,
                dt,
            )
        } else {
            movement::carry_momentum(horiz, desired, (accel * dt).min(1.0), move_speed, decay, dt)
        };
        player.vel.x = new_horiz.x;
        player.vel.z = new_horiz.y;
    }
    let cap = speed_of(if player.traversal.sling || player.traversal.slime_boots {
        movement::MOMENTUM_SPEED_CAP
    } else { movement::speed_cap(gear) });
    let h = Vec2::new(player.vel.x, player.vel.z);
    if h.length() > cap {
        let h = h * (cap / h.length());
        player.vel.x = h.x;
        player.vel.z = h.y;
    }

    let was_on_ground = player.on_ground;
    player.on_ground = false;

    // A slide may run off a ledge and climb voxel steps; sneaking does neither.
    let sneaking = input.sneak && !mv.sliding;
    let sneak_edge_check = was_on_ground && sneaking;

    if mv.wallrunning && fresh_jump {
        // Wall jump: up, away from the wall, and keep the run speed. Refunds
        // the air jump so wall-to-wall chains stay possible (Titanfall).
        let n = mv.wall_normal;
        let h = Vec2::new(player.vel.x, player.vel.z) + n * speed_of(movement::WALLJUMP_PUSH);
        player.vel.x = h.x;
        player.vel.z = h.y;
        player.vel.y = jump_velocity;
        player.jump_buffer = 0.0;
        mv.end_wallrun();
        mv.air_jumps_used = 0;
        mv.raise(movement::EVENT_WALLJUMP);
    } else if (input.jump && was_on_ground) || (player.jump_buffer > 0.0 && player.coyote_time > 0.0) {
        player.vel.y = jump_velocity;
        player.jump_buffer = 0.0;
        player.coyote_time = 0.0;
        if mv.sliding {
            // Slide-jump: keep the slide speed and add a forward kick.
            let h = Vec2::new(player.vel.x, player.vel.z);
            let h = h + h.normalize_or_zero() * speed_of(movement::SLIDE_JUMP_FORWARD);
            player.vel.x = h.x;
            player.vel.z = h.y;
            mv.end_slide();
        }
    } else if air_jump && mv.take_air_jump() {
        player.vel.y = jump_velocity * movement::AIR_JUMP_FACTOR;
        player.jump_buffer = 0.0;
        // An air jump re-aims carried speed toward the held direction.
        if desired.length_squared() > 1e-6 {
            let h = Vec2::new(player.vel.x, player.vel.z);
            let speed = h.length().max(move_speed);
            let h = desired.normalize() * speed;
            player.vel.x = h.x;
            player.vel.z = h.y;
        }
    }

    // Bound long falls in both physical speed and collision traversal work.
    if !mv.air_dash {
        let g = gravity * mv.wallrun_gravity();
        player.vel.y = (player.vel.y - g * dt).max(-speed_blocks(world.voxel_scale, 60.0));
    }
    // Claimed Prism Hart perk: hold Jump for a short lift, release to coast.
    // Charge only returns on solid ground, so repeated taps cannot hover forever.
    if levitation && input.jump && !was_on_ground && !player.swimming
        && !mv.air_dash && player.levitation_seconds > 0.0
    {
        let burn = dt.min(player.levitation_seconds);
        player.levitation_seconds = (player.levitation_seconds - burn).max(0.0);
        let ceiling = speed_blocks(world.voxel_scale, 4.5);
        // Taper thrust near cruise speed; never snap a faster jump down to it.
        let lift = (ceiling - player.vel.y).max(0.0)
            .min(acceleration_blocks(world.voxel_scale, GRAVITY + 18.0) * burn);
        player.vel.y += lift;
        // Braking a fall counts; a token burst at impact is not immunity.
        mv.fall_speed = (-player.vel.y * world.voxel_scale.meters()).max(0.0);
    }
    if climb::touching(world, player.pos, origin, player.scale, player.crouch_t) {
        let grip = climb::Grip { up: input.jump || input.forward, hold: input.sneak };
        player.vel.y = climb::vertical(player.vel.y, grip, world.voxel_scale);
        player.movement.fall_speed = 0.0;
    }

    if !player.swimming && !climb::touching(world, player.pos, origin, player.scale, player.crouch_t) {
        // Strong wind shoves the body; a tornado's updraft lifts it off the ground.
        let m = world.voxel_scale.meters();
        let push = glide::wind_push(player.vel * m, player.glide.wind, player.on_ground);
        player.vel += push * (dt / m);
        if push.y > 0.0 && player.vel.y > 0.0 {
            player.on_ground = false;
        }
    }
    traversal::propel(player, input, world, dt);

    let horiz_delta = Vec3::new(player.vel.x * dt, 0.0, player.vel.z * dt);
    let old_pos = player.pos;
    try_move_local(
        origin,
        world,
        player.scale,
        player.crouch_t,
        &mut player.pos,
        horiz_delta,
        &mut player.on_ground,
        sneak_edge_check,
    );

    let actual = player.pos - old_pos;
    let blocked =
        (actual.x - horiz_delta.x).abs() > 0.0001 || (actual.z - horiz_delta.z).abs() > 0.0001;
    let mut stepped_up_this_frame = false;
    if was_on_ground && !sneaking && blocked {
        let mut stepped = old_pos;
        if try_step_up_local(
            origin,
            world,
            player.scale,
            player.crouch_t,
            &mut stepped,
            horiz_delta,
        ) {
            player.step_camera_offset += (old_pos.y - stepped.y) * world.voxel_scale.meters();
            player.pos = stepped;
            stepped_up_this_frame = true;
        }
    }

    let old_y = player.pos.y;
    try_move_local(
        origin,
        world,
        player.scale,
        player.crouch_t,
        &mut player.pos,
        Vec3::new(0.0, player.vel.y * dt, 0.0),
        &mut player.on_ground,
        false,
    );

    // Match the upward step spring when walking down a small ledge. Larger
    // drops keep their normal fall response instead of being camera-smoothed.
    let descended = old_y - player.pos.y;
    if was_on_ground
        && !stepped_up_this_frame
        && player.on_ground
        && descended > 1e-4
        && descended * world.voxel_scale.meters() <= 0.65
    {
        player.step_camera_offset += descended * world.voxel_scale.meters();
    }

    // Landing cue after a long fall (metres per second, any voxel scale).
    let falling_m = (-player.vel.y * world.voxel_scale.meters()).max(0.0);
    if player.on_ground && !was_on_ground {
        if player.movement.fall_speed >= movement::HARD_LANDING_SPEED {
            player.movement.raise(movement::EVENT_LAND);
        }
        // Shallow water that does not float the body still breaks the fall.
        if !world.adventure.creative && !player.flying && !player.traversal.slime_boots && !immersed(player.pos + Vec3::Y * 0.01) {
            player.harm(
                hazards::fall_damage(player.movement.fall_speed),
                crate::world::adventure::death::Cause::Fall,
            );
        }
        // Bounce after blocked-axis velocity has been cleared below.
    } else if player.flying || player.swimming {
        // A dive in flight or a sink through water is not a fall.
        player.movement.fall_speed = 0.0;
    } else if !player.on_ground {
        player.movement.fall_speed = player.movement.fall_speed.max(falling_m);
    }

    if (player.on_ground && player.vel.y < 0.0)
        || (player.vel.y > 0.0 && (player.pos.y - old_y).abs() < 1e-6)
    {
        player.vel.y = 0.0;
    }
    if player.on_ground && !was_on_ground {
        if !immersed(player.pos + Vec3::Y * 0.01) {
            traversal::bounce(player, input, world);
        }
        player.movement.fall_speed = 0.0;
    }
}
