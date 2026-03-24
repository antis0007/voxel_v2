use glam::{Vec2, Vec3};

use crate::world::{
    AIR_ACCEL, BlockId, CREATIVE_FLY_SPEED, CREATIVE_FLY_SPRINT_SPEED, FLY_ACCEL, GRAVITY,
    GROUND_ACCEL, GROUND_FRICTION, JUMP_VELOCITY, NOCLIP_FLY_FAST_SPEED, NOCLIP_FLY_SPEED,
    SNEAK_SPEED, SPRINT_SPEED, WALK_SPEED, World, acceleration_blocks, hotbar_block,
    player_eye_height_blocks, speed_blocks, try_move, try_step_up,
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
}

impl Input {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

pub struct Player {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub selected_block: BlockId,
    pub scale: f32,

    pub flying: bool,
    pub fly_mode: FlyMode,
    pub jump_was_down: bool,
    pub jump_tap_timer: f32,
    pub crouch_t: f32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            pos: Vec3::new(0.0, 6.0, 0.0),
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            selected_block: hotbar_block(0).unwrap_or(BlockId::Grass),
            scale: 1.0,
            flying: false,
            fly_mode: FlyMode::Creative,
            jump_was_down: false,
            jump_tap_timer: 0.0,
            crouch_t: 0.0,
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

    pub fn eye(&self, voxel_scale: crate::fluid::VoxelScalePreset) -> Vec3 {
        let base_eye = player_eye_height_blocks(voxel_scale, self.scale);
        let crouch_drop = 0.25 * self.scale / voxel_scale.meters().max(1e-6);
        let current_eye = base_eye - crouch_drop * self.crouch_t;
        self.pos + Vec3::new(0.0, current_eye, 0.0)
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

pub fn update_player(
    player: &mut Player,
    input: &mut Input,
    world: &World,
    dt: f32,
    shift_lock: bool,
) {
    if dt <= 0.0 {
        return;
    }

    let target_crouch = if input.sneak { 1.0 } else { 0.0 };
    player.crouch_t += (target_crouch - player.crouch_t) * (1.0 - (-dt * 15.0).exp());

    update_flight_toggle(player, input, dt);

    if player.flying {
        let fly_accel = acceleration_blocks(world.voxel_scale, FLY_ACCEL);
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
        let descend = if shift_lock {
            input.alt_down
        } else {
            input.sneak
        };
        if descend {
            wish -= Vec3::Y;
        }
        wish = wish.normalize_or_zero();

        let target = wish * move_speed;
        player.vel = player.vel.lerp(target, (fly_accel * dt).min(1.0));

        match player.fly_mode {
            FlyMode::Creative => {
                player.on_ground = false;
                let horiz_delta = Vec3::new(player.vel.x * dt, 0.0, player.vel.z * dt);
                try_move(
                    world,
                    player.scale,
                    player.crouch_t,
                    &mut player.pos,
                    horiz_delta,
                    &mut player.on_ground,
                    false,
                );
                try_move(
                    world,
                    player.scale,
                    player.crouch_t,
                    &mut player.pos,
                    Vec3::new(0.0, player.vel.y * dt, 0.0),
                    &mut player.on_ground,
                    false,
                );
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
    let ground_accel = acceleration_blocks(world.voxel_scale, GROUND_ACCEL);
    let air_accel = acceleration_blocks(world.voxel_scale, AIR_ACCEL);
    let gravity = acceleration_blocks(world.voxel_scale, GRAVITY);
    let jump_velocity = speed_blocks(world.voxel_scale, JUMP_VELOCITY);
    let foot_probe = speed_blocks(world.voxel_scale, 0.05);
    let move_speed = if input.sprint {
        sprint_speed
    } else if input.sneak {
        sneak_speed
    } else {
        walk_speed
    };

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
        ground_accel
    } else {
        air_accel
    };
    let target = wish * move_speed;
    let horiz = Vec2::new(player.vel.x, player.vel.z);
    let desired = Vec2::new(target.x, target.z);
    let new_horiz = horiz.lerp(desired, (accel * dt).min(1.0));

    if player.on_ground && wish.length_squared() < 1e-6 {
        let damp = (1.0 - GROUND_FRICTION * dt).max(0.0);
        player.vel.x = new_horiz.x * damp;
        player.vel.z = new_horiz.y * damp;
    } else {
        player.vel.x = new_horiz.x;
        player.vel.z = new_horiz.y;
    }

    let was_on_ground = player.on_ground;
    player.on_ground = false;

    let sneak_edge_check = was_on_ground && input.sneak;

    let below = (player.pos - Vec3::Y * foot_probe).floor().as_ivec3();
    if input.jump && world.get(below) != BlockId::Air && player.vel.y.abs() < 0.001 {
        player.vel.y = jump_velocity;
    }

    player.vel.y -= gravity * dt;

    let horiz_delta = Vec3::new(player.vel.x * dt, 0.0, player.vel.z * dt);
    let old_pos = player.pos;
    try_move(
        world,
        player.scale,
        player.crouch_t,
        &mut player.pos,
        horiz_delta,
        &mut player.on_ground,
        sneak_edge_check,
    );

    if (player.pos - old_pos).length_squared() < 1e-8 {
        let _ = try_step_up(
            world,
            player.scale,
            player.crouch_t,
            &mut player.pos,
            horiz_delta,
        );
    }

    try_move(
        world,
        player.scale,
        player.crouch_t,
        &mut player.pos,
        Vec3::new(0.0, player.vel.y * dt, 0.0),
        &mut player.on_ground,
        false,
    );

    if player.on_ground && player.vel.y < 0.0 {
        player.vel.y = 0.0;
    }
}
