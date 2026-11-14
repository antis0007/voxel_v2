//! Local mining presentation over the same rules used by host authority.
use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct Feedback {
    pub target: Option<(IVec3, BlockId)>,
    pub progress: Option<f32>,
    /// Worked and needed seconds of the local break; paces the tool arc and dig sound.
    pub timing: Option<(f32, f32)>,
    pub requires: Option<u8>,
    pub working: bool,
}

impl Session {
    /// Reads host progress or advances joiner-only presentation progress.
    pub fn mining_feedback(
        &mut self,
        world: &World,
        player: &Player,
        input: &Input,
        can_act: bool,
        dt: f32,
    ) -> Feedback {
        let eye = player.precise_eye(world.voxel_scale) + player.forward_dir().as_dvec3() * 0.0005;
        let hit = world::raycast_block_precise(
            world,
            eye,
            player.forward_dir(),
            6.0 / world.voxel_scale.meters(),
        );
        let target = hit.map(|(cell, _)| (cell, world.get(cell)));
        let held = world
            .inventory
            .hotbar
            .get(self.hand)
            .copied()
            .flatten()
            .map(|stack| stack.item);
        let working = can_act
            && !self.creative
            && !player.flying
            && !input.alt_down
            && (input.break_block || input.break_pressed)
            && world.adventure.swing() <= 0.0
            && !world.adventure.is_dead();
        let tool = world::mining::tool_for_world(world, held);
        let mined = target.map(|(_, block)| world::mining::survival_break(block, tool));
        let progress = if matches!(&self.role, Role::Client { .. }) {
            self.mining_feedback.tick(
                target.filter(|_| working),
                working,
                mined.map_or(0.0, |rule| rule.seconds),
                dt.clamp(0.0, 0.1),
            );
            &self.mining_feedback
        } else {
            &self.authority.local_mining
        };
        Feedback {
            target,
            progress: progress.fraction().filter(|_| working),
            timing: progress.timing().filter(|_| working),
            requires: mined
                .filter(|rule| !rule.drops)
                .map(|rule| rule.required_tier),
            working,
        }
    }

    /// Viewmodel motion is presentational; health and combat stay authoritative.
    pub fn local_viewmodel(world: &World, player: &mut Player, input: &Input, dt: f32) {
        crate::player::viewmodel::tick(player, world, input, dt);
    }
}
