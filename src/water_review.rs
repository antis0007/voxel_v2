//! Opt-in, unsaved in-engine movement review. Uses the normal player update.
use crate::{player::{Input, Player, FlyMode}, world::{World, BlockId}};
use glam::{IVec3, Vec3};

#[derive(Default)]
pub struct WaterReview { elapsed: f32, reset_flight: bool, jump_max: f32, flew: bool, reported: u8, captured_jump: bool }
impl WaterReview {
    pub fn install(&mut self, world: &mut World, player: &mut Player) {
        let n = (1.0 / world.voxel_scale.meters()).round() as i32;
        let mut edits = Vec::new();
        for y in 95*n..108*n { for z in -5*n..6*n { for x in -5*n..6*n {
            let b = if y < 96*n || x == -5*n || x == 6*n-1 || z == -5*n || z == 6*n-1 {
                BlockId::StoneBricks
            } else if y < 100*n { BlockId::Water } else { BlockId::Air };
            edits.push((IVec3::new(x,y,z), b));
        } } }
        world.set_many(&edits);
        Self::reset(player, world.voxel_scale.meters());
    }
    fn reset(player: &mut Player, scale: f32) {
        *player = Player::new();
        player.pos = Vec3::new(0.5,97.0,0.5)/scale;
        player.precise_pos = player.pos.as_dvec3();
        player.fly_mode = FlyMode::Creative;
        player.pitch = 0.15;
    }
    pub fn input(&mut self, player: &mut Player, input: &mut Input, scale: f32, dt: f32) {
        self.elapsed += dt;
        if self.elapsed >= 6.0 && !self.reset_flight {
            Self::reset(player,scale);
            self.reset_flight = true;
        }
        input.clear();
        input.jump = (2.0..4.5).contains(&self.elapsed)
            || (6.5..6.58).contains(&self.elapsed)
            || (6.68..10.5).contains(&self.elapsed);
    }
    pub fn observe(&mut self, player: &Player, scale: f32) -> Option<&'static str> {
        let y = player.pos.y*scale;
        if self.elapsed < 6.0 { self.jump_max = self.jump_max.max(y); }
        else { self.flew |= player.flying && y > 101.0 && !player.swimming; }
        if self.elapsed < 6.0 && y > 100.2 && !self.captured_jump {
            self.captured_jump = true;
            return Some("jump-exit");
        }
        if self.elapsed >= 4.5 && self.reported == 0 {
            self.reported = 1;
            println!("[water-review] jump={} scale={scale} peak_feet_m={:.3} surface_m=100 flying={}",
                if self.jump_max>100.2 && !player.flying {"PASS"} else {"FAIL"},self.jump_max,player.flying);
            return None;
        }
        if self.elapsed >= 10.5 && self.reported == 1 {
            self.reported = 2;
            println!("[water-review] flight={} scale={scale} feet_m={y:.3} swimming={} flying={}",
                if self.flew {"PASS"} else {"FAIL"},player.swimming,player.flying);
            return Some("flight-exit");
        }
        None
    }
}
