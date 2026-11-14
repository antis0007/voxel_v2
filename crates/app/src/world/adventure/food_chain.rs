//! Predators hunt prey when no player holds their attention, from
//! `assets/food_chain.ron`. Prey flees on its own (wildlife.rs reacts to any
//! hostile close by), so this pass only steers the predator and lands bites.
//! The rule a player can learn: a quiet predator is busy with something else,
//! and a herd that bolts means a hunter is near.
use super::{Enemy, World, burrowers::Fx, free, line};
use super::Cue;
use glam::Vec3;

#[derive(Clone, Debug, serde::Deserialize)]
struct Diet {
    prey: Vec<u32>,
    bite: f32,
    sense_m: f32,
}

const RON: &str = include_str!("../../../../../assets/food_chain.ron");
static TABLE: std::sync::LazyLock<Vec<(u32, Diet)>> = std::sync::LazyLock::new(|| {
    ron::from_str(RON).unwrap_or_else(|error| panic!("assets/food_chain.ron: {error}"))
});

/// Predators leave prey alone while the player is this close.
const PLAYER_CALM_M: f32 = 12.0;
const BITE_REACH_M: f32 = 1.6;
const BITE_EVERY_S: f32 = 1.1;
const CHASE_MPS: f32 = 3.2;

fn diet(kind: u32) -> Option<&'static Diet> {
    TABLE.iter().find(|(k, _)| *k == kind).map(|(_, d)| d)
}

/// One hunting step for every idle predator. Runs after the per-kind ticks
/// so a predator that noticed the player this frame keeps that focus.
pub(super) fn hunt(enemies: &mut [Enemy], world: &World, feet: Vec3, dt: f32, fx: &mut Vec<Fx>) {
    for p in 0..enemies.len() {
        let Some(diet) = diet(enemies[p].kind) else { continue };
        let hunter = &enemies[p];
        if hunter.health <= 0.0 || hunter.visible || hunter.provoked || (hunter.pos - feet).length() < PLAYER_CALM_M {
            continue;
        }
        #[cfg(feature = "multiplayer")]
        if world.multiplayer.as_ref().is_some_and(|c| !c.owns(hunter.pos)) {
            continue;
        }
        let from = hunter.pos;
        let reach_sq = diet.sense_m * diet.sense_m;
        let Some(q) = enemies
            .iter()
            .enumerate()
            .filter(|(i, e)| *i != p && e.health > 0.0 && diet.prey.contains(&e.kind))
            .map(|(i, e)| (i, e.pos.distance_squared(from)))
            .filter(|&(_, d)| d < reach_sq)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
        else {
            continue;
        };
        let target = enemies[q].center();
        let to = enemies[q].pos - from;
        let flat = Vec3::new(to.x, 0.0, to.z);
        let distance = flat.length();
        let hunter = &mut enemies[p];
        if distance > 0.01 {
            hunter.yaw = (-flat.x).atan2(-flat.z);
        }
        if distance > BITE_REACH_M {
            let step = flat / distance * CHASE_MPS * dt;
            let next = hunter.pos + step;
            if free(world, next, hunter.kind) {
                hunter.pos = next;
                hunter.phase += step.length();
            }
            continue;
        }
        if hunter.cooldown > 0.0 || !line(world, hunter.center(), target) {
            continue;
        }
        hunter.cooldown = BITE_EVERY_S;
        let bite = diet.bite;
        let prey = &mut enemies[q];
        // A bite from a beast is not the player's doing: keep the prey's
        // temper toward the player as it was.
        let calm = prey.provoked;
        prey.hurt(bite);
        prey.provoked = calm;
        fx.push(Fx::Sound { cue: Cue::Impact, at: target, gain: 0.6, pitch: 0.8 });
        fx.push(Fx::Burst { at: target, color: [0.55, 0.12, 0.1], count: 10 });
    }
}
