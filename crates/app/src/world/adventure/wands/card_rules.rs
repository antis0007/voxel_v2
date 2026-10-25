//! Shared effect numbers used by gameplay and modifier card readouts.
//! Values moved unchanged from the evaluator and impact handlers.
pub const RAPID_MANA_REFUND: f32 = 20.0;
pub const WOUNDED_THRESHOLD: f32 = 0.5;
pub const TIMER_SECONDS: f32 = 0.6;
pub const CRITICAL_PERIOD: u32 = 4;
pub const CRITICAL_MULTIPLIER: f32 = 2.5;
pub const UNSTABLE_PERIOD: u32 = 5;
pub const RECKLESS_HEALTH: f32 = 4.0;
pub const BLOOD_PRICE_FRACTION: f32 = 0.5;
pub const BLOOD_PRICE_RESERVE: f32 = 4.0;
pub const SIPHON_HEALTH: f32 = 4.0;
pub const BLOOD_SIPHON_FRACTION: f32 = 0.25;
pub const MANA_LEECH: f32 = 4.0;
pub const SHRAPNEL_RADIUS: f32 = 2.0;
pub const SHRAPNEL_DAMAGE: f32 = 11.0;
pub const FORK_ANGLES: [f32; 3] = [-0.9, 0.0, 0.9];
pub const FORK_DAMAGE: f32 = 0.45;
pub const EXPLOSIVE_CRATER: f32 = 1.6;
pub const PROXIMITY_RANGE: f32 = 3.0;
pub const UNDERTOW_RADIUS: f32 = 4.5;
pub const SOAKED_MULTIPLIER: f32 = 2.0;
pub const GRAVITY_ACCELERATION: f32 = 14.0;
pub const MAX_DAMAGE_MULTIPLIER: f32 = 2.6;
pub const RESONANT_CRATER: f32 = 2.0;
pub const DRILL_REACH: f32 = 6.8;
pub const TIME_SNAG_DURATION: f32 = 3.5;
pub const GRAPPLE_REACH: f32 = 32.0;
pub const VENOM_NOVA_RADIUS: f32 = 3.4;
pub const ICE_WALL_WIDTH: f32 = 3.0;
pub const ICE_WALL_HEIGHT: f32 = 2.5;
pub const BOUNCE_ORB_RICOCHETS: u8 = 4;
pub const DIG_BOLT_BUDGET: u8 = 12;
pub const MEND_BASE: f32 = 30.0;
pub const MEND_PER_RANK: f32 = 5.0;

pub fn mend_health(rank: u32) -> f32 { MEND_BASE + rank as f32 * MEND_PER_RANK }
pub fn ward_duration(discoveries: u32) -> f32 {
    if discoveries & (1 << 9) != 0 { 9.0 } else { 6.0 }
}
pub fn ward_hits(discoveries: u32) -> u32 {
    if discoveries & (1 << 10) != 0 { 4 } else { 3 }
}
