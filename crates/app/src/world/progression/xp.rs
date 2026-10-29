//! Vanilla experience curve: points to the next level grow in three linear bands.

/// Points needed to go from `level` to `level + 1`.
pub fn to_next(level: u32) -> u32 {
    match level {
        0..=15 => 2 * level + 7,
        16..=30 => 5 * level - 38,
        _ => 9u32.saturating_mul(level).saturating_sub(158),
    }
}

/// Level reached by `total` points and the points into that level.
pub fn level_of(total: u32) -> (u32, u32) {
    let (mut level, mut left) = (0u32, total);
    while level < 10_000 && left >= to_next(level) {
        left -= to_next(level);
        level += 1;
    }
    (level, left)
}

/// Fraction of the current level filled, for the HUD bar.
pub fn progress(total: u32) -> f32 {
    let (level, into) = level_of(total);
    into as f32 / to_next(level) as f32
}

/// XP granted for a defeated hostile.
pub fn for_kill(menace: u8, boss: bool) -> u32 {
    if boss { 50 } else { 5 + 3 * u32::from(menace) }
}
