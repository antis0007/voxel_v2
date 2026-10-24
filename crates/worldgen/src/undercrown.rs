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
mod plan;
mod state;
pub mod v2;

#[allow(unused_imports)]
pub use plan::{
    Action, Damage, District, Link, LinkKind, Plan, Prop, PropKind, RoomKind, RoomSpec, Style,
};
#[allow(unused_imports)]
pub use state::{KNOWN_FLAGS, MAX_RECORDS, ObjectId, State, flags};

use super::{BlockId, WorldSampleConfig};
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
pub fn plan(cfg: &WorldSampleConfig) -> Rc<Plan> {
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
pub fn block(cfg: &WorldSampleConfig, p: IVec3, height: i32) -> Option<BlockId> {
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
pub fn reserved(cfg: &WorldSampleConfig, m: Vec3) -> bool {
    cfg.generator_version >= FIRST_VERSION && plan(cfg).reserved(m)
}

/// True when a box touches the reserved footprint.
pub fn overlaps(cfg: &WorldSampleConfig, lo: Vec3, hi: Vec3) -> bool {
    cfg.generator_version >= FIRST_VERSION && plan(cfg).overlaps(lo, hi)
}

/// Human-readable location name for the journal and surface naming.
pub fn location(cfg: &WorldSampleConfig, m: Vec3) -> Option<&'static str> {
    if cfg.generator_version < FIRST_VERSION {
        return None;
    }
    plan(cfg).location(m)
}
