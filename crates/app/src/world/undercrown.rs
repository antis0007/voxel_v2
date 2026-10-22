//! Deterministic Undercrown content layer.
//!
//! The plan is a pure function of the world configuration, exactly like the
//! existing dungeon and site generators. This module owns the content contract
//! and the inert plumbing; authored rooms are added by later milestones.
//!
//! Version discipline:
//! - `FIRST_VERSION` is the first generator that may contain Undercrown content.
//! - A v18 world samples exactly as it did before this module was added.
//! - v19 and later author the connected rooms in `plan::Plan::for_config`.
mod interact;
mod plan;
mod state;

pub use interact::Outcome;
#[allow(unused_imports)]
pub use plan::{
    Action, Damage, District, Link, LinkKind, Plan, Prop, PropKind, RoomKind, RoomSpec, Style,
};
#[allow(unused_imports)]
pub use state::{KNOWN_FLAGS, MAX_RECORDS, ObjectId, State, flags};

use super::{BlockId, WorldSampleConfig};
use crate::player::Player;
use glam::{IVec3, Vec3};
use std::cell::RefCell;
use std::rc::Rc;

/// First generator version allowed to contribute Undercrown geometry.
pub const FIRST_VERSION: u32 = 19;

thread_local! {
    static PLAN_CACHE: RefCell<Option<(WorldSampleConfig, Rc<Plan>)>> = const { RefCell::new(None) };
}

/// Cached plan for the current configuration. One world is active at a time,
/// matching the existing site/dungeon caches.
pub(crate) fn plan(cfg: &WorldSampleConfig) -> Rc<Plan> {
    PLAN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((key, plan)) = cache.as_ref()
            && key == cfg
        {
            return plan.clone();
        }
        let built = Rc::new(Plan::for_config(cfg));
        *cache = Some((*cfg, built.clone()));
        built
    })
}

/// Block contributed by the Undercrown, if any. Call this before random sites
/// and caverns so authored space wins inside its own footprint.
pub(crate) fn block(cfg: &WorldSampleConfig, p: IVec3, height: i32) -> Option<BlockId> {
    if cfg.generator_version < FIRST_VERSION {
        return None;
    }
    let plan = plan(cfg);
    if plan.is_empty() {
        return None;
    }
    let m = p.as_vec3() * cfg.voxel_meters();
    plan.sample(cfg, m, height)
}

/// True when random generated space must stay out of this point.
pub(crate) fn reserved(cfg: &WorldSampleConfig, m: Vec3) -> bool {
    cfg.generator_version >= FIRST_VERSION && plan(cfg).reserved(m)
}

/// Human-readable location name for the journal and surface naming.
pub(crate) fn location(cfg: &WorldSampleConfig, m: Vec3) -> Option<&'static str> {
    if cfg.generator_version < FIRST_VERSION {
        return None;
    }
    plan(cfg).location(m)
}

/// Shared prompt focus. Reliquaries take priority: the caller only asks for a
/// mechanism prompt when no relic is focused.
pub(crate) fn focus(
    world: &super::World,
    eye: Vec3,
    forward: Vec3,
) -> Option<super::interaction::Prompt> {
    if world.generator_version < FIRST_VERSION {
        return None;
    }
    interact::focus_with_plan(
        &plan(&world.sample_config()),
        &world.undercrown,
        world,
        eye,
        forward,
    )
    .map(|(_, prompt)| prompt)
}

/// Runs the focused mechanism and applies any voxel edits. State is committed
/// only after the edit set has been applied, so a save cannot claim a repair
/// whose geometry was never written.
pub(crate) fn interact(world: &mut super::World, player: &Player) -> Outcome {
    if world.generator_version < FIRST_VERSION {
        return Outcome::default();
    }
    let cfg = world.sample_config();
    let plan = plan(&cfg);
    if plan.is_empty() {
        return Outcome::default();
    }
    let mut state = std::mem::take(&mut world.undercrown);
    let outcome = interact::interact_with_plan(&plan, &mut state, world, player);
    world.undercrown = state;
    outcome
}
