//! Keyed random draws: a pure function of what is being decided.
//!
//! There is no generator state. A retry, a different batching schedule or a
//! different thread count asks the same question and gets the same answer.

/// Part of every key. Bump it when the draw function changes.
pub const DRAW_VERSION: u32 = 1;

const M0: u32 = 0xD251_1F53;
const M1: u32 = 0xCD9E_8D57;
const W0: u32 = 0x9E37_79B9;
const W1: u32 = 0xBB67_AE85;

/// Philox4x32-10 block function (Salmon and others, SC11).
pub const fn philox4x32(mut c: [u32; 4], mut k: [u32; 2]) -> [u32; 4] {
    let mut round = 0;
    while round < 10 {
        if round > 0 {
            k[0] = k[0].wrapping_add(W0);
            k[1] = k[1].wrapping_add(W1);
        }
        let p0 = M0 as u64 * c[0] as u64;
        let p1 = M1 as u64 * c[2] as u64;
        c = [(p1 >> 32) as u32 ^ c[1] ^ k[0], p1 as u32, (p0 >> 32) as u32 ^ c[3] ^ k[1], p0 as u32];
        round += 1;
    }
    c
}

/// SplitMix64 finalizer.
pub const fn mix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// One 64-bit draw for `(seed, purpose, entity, index)`.
///
/// `purpose` names the decision, `entity` who or what it concerns, `index`
/// the semantic occurrence (game day, event sequence). Never pass a counter
/// that a retry would advance.
pub const fn draw(seed: u64, purpose: u32, entity: u64, index: u64) -> u64 {
    let k = mix64(seed ^ mix64(((DRAW_VERSION as u64) << 32) | purpose as u64));
    let out = philox4x32(
        [entity as u32, (entity >> 32) as u32, index as u32, (index >> 32) as u32],
        [k as u32, (k >> 32) as u32],
    );
    out[0] as u64 | (out[1] as u64) << 32
}

/// Maps a draw onto `0..n` by multiply-high. Bias is below `n / 2^64`.
pub const fn below(x: u64, n: u64) -> u64 {
    ((x as u128 * n as u128) >> 64) as u64
}

/// True with probability `numer / denom`; `denom` must be nonzero.
pub const fn chance(x: u64, numer: u64, denom: u64) -> bool {
    below(x, denom) < numer
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known answers from the Random123 distribution (kat_vectors, philox4x32 10).
    #[test]
    fn philox_matches_published_vectors() {
        assert_eq!(philox4x32([0; 4], [0; 2]), [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8]);
        assert_eq!(philox4x32([u32::MAX; 4], [u32::MAX; 2]), [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd]);
        assert_eq!(
            philox4x32([0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344], [0xa409_3822, 0x299f_31d0]),
            [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1]
        );
    }

    #[test]
    fn every_key_part_changes_the_draw() {
        let base = draw(7, 1, 100, 412);
        assert_eq!(base, draw(7, 1, 100, 412));
        for other in [draw(8, 1, 100, 412), draw(7, 2, 100, 412), draw(7, 1, 101, 412), draw(7, 1, 100, 413),
            draw(7, 1, 100 | 1 << 40, 412), draw(7, 1, 100, 412 | 1 << 40)] {
            assert_ne!(base, other);
        }
    }

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut seen = [false; 6];
        for i in 0..200 {
            let v = below(draw(1, 3, 9, i), 6);
            assert!(v < 6);
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(below(u64::MAX, 1), 0);
        assert!(!chance(u64::MAX, 0, 10));
        assert!(chance(0, 1, 10));
    }

    #[test]
    fn draws_are_roughly_uniform() {
        let n = 40_000u64;
        let hits = (0..n).filter(|&i| chance(draw(99, 5, i, 0), 1, 4)).count() as u64;
        assert!((n / 4 - n / 50..=n / 4 + n / 50).contains(&hits), "{hits}");
    }
}
