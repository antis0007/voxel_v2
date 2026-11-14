//! Portable use of the pump's finite free-water effect.
use super::*;
use crate::world::effects::{self, Failure};

pub const SPELL: u32 = 95;
pub const REACH: f32 = 12.0;
pub const LIMIT: u32 = 128; // 0.25 cubic metre, bounded by the source cell.

impl Adventure {
    /// False means selection, no-op or failed admission: do not pay for a cast.
    /// The selection stores coordinates only, never a detached water inventory.
    pub(super) fn cast_water_transfer(
        &mut self,
        world: &mut World,
        player: &Player,
        eye: Vec3,
        forward: Vec3,
    ) -> bool {
        self.cast_cd = 0.35;
        let Some((_, target)) = wands::aim_cell(world, eye, forward, REACH) else {
            world.expedition.say("Destination required");
            return false;
        };
        if self.water_source.is_none() || player.crouch_t > 0.5 {
            if effects::admit(world, target).is_err() {
                world.expedition.say("Water unavailable");
            } else if world.water_microlitres(target) >= effects::WATER_UNIT_UL {
                self.water_source = Some(target);
                world.expedition.say("Intake selected");
            } else {
                world.expedition.say("Water required");
            }
            return false;
        }
        let Some(source) = self.water_source else {
            return false;
        };
        let scale = world.voxel_scale.meters();
        let centre = (source.as_vec3() + Vec3::splat(0.5)) * scale;
        if (centre - eye).length() > REACH {
            self.water_source = None;
            world.expedition.say("Source out of reach");
            return false;
        }
        // Reserve both geometry notifications before changing either endpoint.
        if self.dirty_voxels.len() > 2046 {
            return false;
        }
        match effects::transfer_water(world, source, target, LIMIT) {
            Ok(receipt) => {
                receipt.publish_voxels(world, &mut self.dirty_voxels);
                self.spawn_voxel_burst(centre, spell_color(SPELL), 6, 0.7, 0.035);
                self.spawn_voxel_burst(
                    (target.as_vec3() + Vec3::splat(0.5)) * scale,
                    spell_color(SPELL),
                    6,
                    0.7,
                    0.035,
                );
                true
            }
            Err(Failure::NoChange) => {
                if world.water_microlitres(source) < effects::WATER_UNIT_UL {
                    self.water_source = None;
                    world.expedition.say("Source empty");
                } else if source != target {
                    world.expedition.say("Destination full");
                }
                false
            }
            Err(Failure::Deferred) => {
                world.expedition.say("Transfer waiting");
                false
            }
            Err(Failure::Rejected | Failure::Conflict) => {
                world.expedition.say("Transfer blocked");
                false
            }
        }
    }
}
