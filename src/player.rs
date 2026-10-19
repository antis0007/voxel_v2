use glam::{Vec2, Vec3, DVec3, IVec3};

use crate::world::{
    AIR_ACCEL, BlockId, CREATIVE_FLY_SPEED, CREATIVE_FLY_SPRINT_SPEED, FLY_ACCEL, GRAVITY,
    GROUND_ACCEL, GROUND_FRICTION, JUMP_VELOCITY, NOCLIP_FLY_FAST_SPEED, NOCLIP_FLY_SPEED,
    SNEAK_SPEED, SPRINT_SPEED, WALK_SPEED, World, acceleration_blocks, hotbar_block,
    player_eye_height_blocks, speed_blocks, try_move_local, try_step_up_local,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlyMode {
    Creative,
    Noclip,
}

impl FlyMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Creative => "Creative",
            Self::Noclip => "Noclip",
        }
    }
}

#[derive(Default, Clone)]
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
    pub wheel: f32,
    pub ctrl_down: bool,
    pub pick_block: bool,
    pub interact: bool,
    pub cast_spell: bool,
    pub cycle_spell: bool,
    shift_keys: u8,
    control_keys: u8,
    alt_keys: u8,
}

impl Input {
    pub fn keyboard(&mut self, key: winit::keyboard::KeyCode, pressed: bool) {
        use winit::keyboard::KeyCode;
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
        self.sneak = self.shift_keys != 0;
        self.ctrl_down = self.control_keys != 0;
        self.sprint = self.ctrl_down;
        self.alt_down = self.alt_keys != 0;
        match key {
            KeyCode::KeyW => self.forward = pressed,
            KeyCode::KeyS => self.back = pressed,
            KeyCode::KeyA => self.left = pressed,
            KeyCode::KeyD => self.right = pressed,
            KeyCode::Space => self.jump = pressed,
            KeyCode::KeyR => self.cast_spell = pressed,
            _ => {}
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

pub struct Player {
    pub pos: Vec3,
    /// Authoritative position; pos is the legacy rounded world-space view.
    pub precise_pos: DVec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub sprinting: bool,
    pub selected_block: BlockId,
    pub scale: f32,

    pub flying: bool,
    pub fly_mode: FlyMode,
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
    jump_buffer: f32,
    coyote_time: f32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            pos: Vec3::new(0.0, 6.0, 0.0),
            precise_pos: DVec3::new(0.0, 6.0, 0.0),
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            sprinting: false,
            selected_block: hotbar_block(0).unwrap_or(BlockId::Grass),
            scale: 1.0,
            flying: false,
            fly_mode: FlyMode::Creative,
            jump_was_down: false,
            jump_tap_timer: 0.0,
            crouch_t: 0.0,
            step_camera_offset: 0.0,
            step_camera_velocity_meters: 0.0,
            walk_phase: 0.0,
            walk_bob_meters: 0.0,
            swimming: false,
            underwater: false,
            jump_buffer: 0.0,
            coyote_time: 0.0,
        }
    }

    pub fn forward_dir(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(sy * cp, sp, -cy * cp).normalize_or_zero()
    }

    pub fn flat_forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(sy, 0.0, -cy).normalize_or_zero()
    }

    pub fn right_dir(&self) -> Vec3 {
        let f = self.flat_forward();
        Vec3::new(-f.z, 0.0, f.x).normalize_or_zero()
    }

    pub fn precise_eye(&self, voxel_scale: crate::fluid::VoxelScalePreset) -> DVec3 {
        let position = if self.precise_pos.as_vec3() == self.pos { self.precise_pos } else { self.pos.as_dvec3() };
        let meters_per_voxel = voxel_scale.meters().max(1e-6);
        let height = player_eye_height_blocks(voxel_scale,self.scale)
            - 0.25*self.scale*self.crouch_t/voxel_scale.meters().max(1e-6)
            + (self.step_camera_offset + self.walk_bob_meters) / meters_per_voxel;
        position + DVec3::Y * height as f64
    }

    pub fn eye(&self, voxel_scale: crate::fluid::VoxelScalePreset) -> Vec3 {
        let base_eye = player_eye_height_blocks(voxel_scale, self.scale);
        let meters_per_voxel = voxel_scale.meters().max(1e-6);
        let crouch_drop = 0.25 * self.scale / voxel_scale.meters().max(1e-6);
        let current_eye = base_eye - crouch_drop * self.crouch_t;
        self.pos + Vec3::Y * (current_eye
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
                player.vel.y = 0.0;
            }
        } else {
            player.jump_tap_timer = 0.25;
        }
    }

    player.jump_was_down = input.jump;
}

pub fn update_player(player: &mut Player, input: &mut Input, world: &World, dt: f32) {
    if !player.pos.is_finite() || !dt.is_finite() || dt <= 0.0 { return; }
    player.sprinting = input.sprint && input.forward && !input.sneak && !player.flying;
    // Existing teleports write pos. Reconcile once, then integrate only locally.
    if player.precise_pos.as_vec3() != player.pos { player.precise_pos = player.pos.as_dvec3(); }
    let origin = (player.precise_pos / 32.0).floor().as_ivec3() * 32;
    player.pos = (player.precise_pos - origin.as_dvec3()).as_vec3();
    update_player_local(player,input,world,dt,origin);
    player.precise_pos = origin.as_dvec3() + player.pos.as_dvec3();
    player.pos = player.precise_pos.as_vec3();
}
fn update_player_local(player: &mut Player, input: &mut Input, world: &World, dt: f32, origin: IVec3) {
    if dt <= 0.0 {
        return;
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
        point.y - (cell.y as f32) < world.water_volume(cell + origin) as f32 / 8.0
    };
    let body_scale = player.scale / world.voxel_scale.meters();
    player.underwater = immersed(player.eye(world.voxel_scale));
    player.swimming = !player.flying && immersed(player.pos + Vec3::Y * (0.65 * body_scale));
    let horizontal_speed_m = Vec2::new(player.vel.x, player.vel.z).length() * world.voxel_scale.meters();
    let walking = player.on_ground && !player.flying && !player.swimming && horizontal_speed_m > 0.16;
    if walking {
        player.walk_phase = (player.walk_phase + dt * horizontal_speed_m * 2.3)
            .rem_euclid(std::f32::consts::TAU);
    }
    let stride_weight = if walking {
        (horizontal_speed_m / 1.8).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let target_walk_bob = 0.018 * stride_weight * (player.walk_phase * 2.0).sin();
    player.walk_bob_meters += (target_walk_bob - player.walk_bob_meters) * (1.0 - (-dt * 14.0).exp());
    let fresh_jump = input.jump && !player.jump_was_down;
    // Flight is a movement mode, including while immersed. Swimming must not
    // consume/reset the double-tap before the mode switch can see it.
    update_flight_toggle(player, input, dt);
    player.swimming = !player.flying && immersed(player.pos + Vec3::Y * (0.65 * body_scale));
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
    if next_crouch >= player.crouch_t || !crate::world::collides_world_local(world, min, max, origin) {
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
        player.vel.y += acceleration_blocks(world.voxel_scale, if input.sneak { -4.0 } else { 3.0 }) * dt;
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
        let (base_speed, fast_speed) = match player.fly_mode {
            FlyMode::Creative => (CREATIVE_FLY_SPEED, CREATIVE_FLY_SPRINT_SPEED),
            FlyMode::Noclip => (NOCLIP_FLY_SPEED, NOCLIP_FLY_FAST_SPEED),
        };
        let move_speed = speed_blocks(
            world.voxel_scale,
            if input.sprint { fast_speed } else { base_speed },
        );

        let mut wish = Vec3::ZERO;
        let forward = if matches!(player.fly_mode, FlyMode::Creative) {
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

        let target = wish * move_speed;
        player.vel = player.vel.lerp(target, (fly_accel * dt).min(1.0));

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
                if player.on_ground && input.sneak {
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

    let walk_speed = speed_blocks(world.voxel_scale, WALK_SPEED);
    let sprint_speed = speed_blocks(world.voxel_scale, SPRINT_SPEED);
    let sneak_speed = speed_blocks(world.voxel_scale, SNEAK_SPEED);
    let ground_accel = GROUND_ACCEL;
    let air_accel = AIR_ACCEL;
    let gravity = acceleration_blocks(world.voxel_scale, GRAVITY);
    let jump_velocity = speed_blocks(world.voxel_scale, JUMP_VELOCITY);
    let underfoot = world.get(
        (player.pos - Vec3::Y * (0.05 / world.voxel_scale.meters()))
            .floor()
            .as_ivec3() + origin,
    );
    let grip = if underfoot == BlockId::Ice { 0.08 } else { 1.0 };
    let terrain_speed = if underfoot == BlockId::Mud { 0.62 } else { 1.0 };
    let move_speed = (if input.sneak {
        sneak_speed
    } else if input.sprint && input.forward {
        sprint_speed
    } else {
        walk_speed
    }) * terrain_speed;

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
    let new_horiz = horiz.lerp(desired, (accel * dt).min(1.0));

    if player.on_ground && wish.length_squared() < 1e-6 {
        let damp = (-GROUND_FRICTION * grip * dt).exp();
        player.vel.x = horiz.x * damp;
        player.vel.z = horiz.y * damp;
    } else {
        player.vel.x = new_horiz.x;
        player.vel.z = new_horiz.y;
    }

    let was_on_ground = player.on_ground;
    player.on_ground = false;

    let sneak_edge_check = was_on_ground && input.sneak;

    if (input.jump && was_on_ground) || (player.jump_buffer > 0.0 && player.coyote_time > 0.0) {
        player.vel.y = jump_velocity;
        player.jump_buffer = 0.0;
        player.coyote_time = 0.0;
    }

    // Bound long falls in both physical speed and collision traversal work.
    player.vel.y = (player.vel.y - gravity * dt).max(-speed_blocks(world.voxel_scale, 60.0));

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
    if was_on_ground && !input.sneak && blocked {
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

    if (player.on_ground && player.vel.y < 0.0)
        || (player.vel.y > 0.0 && (player.pos.y - old_y).abs() < 1e-6)
    {
        player.vel.y = 0.0;
    }
}
