//! Quiet ground around the world spawn and the respawn point for the first
//! in-game day. Radius and duration are data in `assets/spawn_safety.ron`.
//! Runtime only: nothing here is saved.
use super::{Adventure, World, burrowers, night};
use glam::Vec3;

#[derive(Clone, Copy, Debug, serde::Deserialize)]
struct Rules {
    radius_m: f32,
    grace_s: f32,
}

const RULES_RON: &str = include_str!("../../../../../assets/spawn_safety.ron");
static RULES: std::sync::LazyLock<Rules> = std::sync::LazyLock::new(|| {
    ron::from_str(RULES_RON).unwrap_or_else(|error| panic!("assets/spawn_safety.ron: {error}"))
});

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    /// World spawn in metres, resolved once per session.
    anchor: Option<Vec3>,
}

impl State {
    /// True when `p` (metres) is inside quiet ground right now.
    pub(super) fn guards(&self, time: f32, respawn: Option<Vec3>, p: Vec3) -> bool {
        let rules = *RULES;
        if time >= rules.grace_s {
            return false;
        }
        let near = |a: Vec3| {
            let d = p - a;
            Vec3::new(d.x, 0.0, d.z).length() < rules.radius_m
        };
        self.anchor.is_some_and(near) || respawn.filter(|r| r.is_finite()).is_some_and(near)
    }
}

impl Adventure {
    /// True when `p` (metres) is quiet ground for wild hostiles right now.
    pub(super) fn safety_guards(&self, p: Vec3) -> bool {
        self.safety.guards(self.time, self.respawn_point, p)
    }
}

/// Resolves the anchor once, then removes transient hostiles standing inside.
/// Runs on the one-second spawn pass, after the spawners.
pub(super) fn sweep(adv: &mut Adventure, world: &World) {
    if adv.time >= RULES.grace_s {
        return;
    }
    if adv.safety.anchor.is_none() {
        let cfg = world.sample_config();
        let scale = world.voxel_scale.meters();
        adv.safety.anchor = Some(crate::world::suggested_spawn(&cfg).unwrap_or(Vec3::new(0.5, 0.0, 0.5)) * scale);
    }
    let (state, time, respawn) = (adv.safety.clone(), adv.time, adv.respawn_point);
    adv.enemies.retain(|e| {
        let transient = e.site.z == night::NIGHT_Z || burrowers::is_burrow_body(e);
        !(transient && state.guards(time, respawn, e.pos))
    });
}
