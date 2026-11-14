//! Opt-in rendered gameplay driver. Uses ordinary movement/attack commands;
//! damage, collision, death and respawn are still resolved by the host.
use super::*;

#[derive(Default)]
pub(super) struct Review {
    began: Option<Instant>,
    next_log: f32,
    respawn_at: Option<Instant>,
}

impl Session {
    pub fn rendered_review_input(&mut self, world: &World, player: &mut Player, input: &mut Input) {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !*ENABLED.get_or_init(|| std::env::args().any(|a| a == "--multiplayer-rendered-duel"))
            || !self.ready() || self.hold { return; }
        let t = self.review.began.get_or_insert_with(Instant::now).elapsed().as_secs_f32();
        *input = Input::default();
        if world.adventure.is_dead() {
            if self.review.respawn_at.is_none_or(|at| at.elapsed().as_secs_f32() > 2.0) {
                let _ = self.route_ui(crate::egui_ui::UiAction::Respawn);
                self.review.respawn_at = Some(Instant::now());
                eprintln!("[rendered-duel] respawn requested");
            }
        } else {
            self.review.respawn_at = None;
            if let Some(remote) = self.remotes.values().min_by(|a,b| {
                Vec3::from_array(a.pose.position).distance_squared(player.pos)
                    .total_cmp(&Vec3::from_array(b.pose.position).distance_squared(player.pos))
            }) {
                let target = Vec3::from_array(remote.pose.position) + Vec3::Y * (0.9 / world.voxel_scale.meters());
                let delta = target - player.eye(world.voxel_scale);
                let aim = delta.normalize_or_zero();
                player.yaw = aim.x.atan2(-aim.z);
                player.pitch = aim.y.asin().clamp(-1.5, 1.5);
                let distance = delta.length() * world.voxel_scale.meters();
                input.forward = distance > 1.4;
                input.sprint = distance > 3.0;
                input.jump = distance > 2.0 && t.fract() < 0.12;
                input.left = distance < 2.5 && (t % 6.0) < 3.0;
                input.right = distance < 2.5 && (t % 6.0) >= 3.0;
                input.break_block = true;
            }
        }
        if t >= self.review.next_log {
            self.review.next_log = t + 1.0;
            let remote_swing = self.remotes.values().map(|r|r.pose.swing).fold(0.0_f32,f32::max);
            eprintln!("[rendered-duel] t={t:.1} pos={:?} health={:.1} swing={:.3} peers={} attack={} moving={} remote_swing={remote_swing:.3}",
                player.pos, world.adventure.health, world.adventure.swing(), self.remotes.len(), input.break_block,
                input.forward || input.left || input.right);
        }
    }
}
