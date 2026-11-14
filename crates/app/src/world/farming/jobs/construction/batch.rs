//! Stable nearest candidates with bounded storage and lazy eligibility checks.
pub(super) fn nearest<const N: usize, T: Copy>(
    rows: impl IntoIterator<Item = T>,
    mut distance: impl FnMut(T) -> f32,
    mut eligible: impl FnMut(T) -> bool,
) -> Vec<T> {
    let mut selected: [Option<(f32, T)>; N] = [None; N];
    let mut len = 0;
    for row in rows {
        let key = distance(row);
        let index = selected[..len]
            .iter()
            .position(|entry| entry.is_some_and(|(other, _)| key.total_cmp(&other).is_lt()))
            .unwrap_or(len);
        // Equal distances stay in input order, as in the previous stable sort.
        if index >= N || !eligible(row) {
            continue;
        }
        let next_len = (len + 1).min(N);
        for slot in (index + 1..next_len).rev() {
            selected[slot] = selected[slot - 1];
        }
        selected[index] = Some((key, row));
        len = next_len;
    }
    selected[..len].iter().filter_map(|entry| entry.map(|(_, row)| row)).collect()
}

#[cfg(test)]
mod tests {
    use super::nearest;
    use std::cell::Cell;

    fn oracle<const N: usize>(rows: &[(u32, f32, bool)]) -> Vec<(u32, f32, bool)> {
        let mut result: Vec<_> = rows.iter().copied().filter(|row| row.2).collect();
        result.sort_by(|a, b| a.1.total_cmp(&b.1));
        result.truncate(N);
        result
    }

    #[test]
    fn stable_ties_keep_queue_order() {
        let rows = [(9, 4.0, true), (2, 1.0, true), (7, 1.0, true), (1, 1.0, true)];
        assert_eq!(nearest::<2, _>(rows, |r| r.1, |r| r.2), vec![rows[1], rows[2]]);
    }

    #[test]
    fn rejected_near_cells_do_not_displace_ready_cells() {
        let rows = [(0, 9.0, true), (1, 1.0, false), (2, 4.0, true), (3, 0.0, false)];
        assert_eq!(nearest::<2, _>(rows, |r| r.1, |r| r.2), oracle::<2>(&rows));
    }

    #[test]
    fn empty_capacity_never_checks_readiness() {
        assert!(nearest::<0, _>(0..256, |n| n as f32, |_| panic!("no capacity")).is_empty());
        assert!(nearest::<7, _>(std::iter::empty::<u32>(), |n| n as f32, |_| true).is_empty());
    }

    #[test]
    fn full_batch_skips_farther_and_equal_readiness_checks() {
        let checks = Cell::new(0);
        let selected = nearest::<7, _>(0..256, |n| (n / 2) as f32, |_| {
            checks.set(checks.get() + 1);
            true
        });
        assert_eq!(selected, (0..7).collect::<Vec<_>>());
        assert_eq!(checks.get(), 7);
    }

    #[test]
    fn descending_distances_still_check_every_better_candidate() {
        let checks = Cell::new(0);
        let selected = nearest::<7, _>((0..256).rev(), |n| n as f32, |_| {
            checks.set(checks.get() + 1);
            true
        });
        assert_eq!(selected, (0..7).collect::<Vec<_>>());
        assert_eq!(checks.get(), 256);
    }

    #[test]
    fn every_short_readiness_mask_matches_stable_sort() {
        for mask in 0..1024u32 {
            let rows: Vec<_> = (0..10).map(|i| (i, ((i * 7) % 5) as f32, mask & (1 << i) != 0)).collect();
            assert_eq!(nearest::<7, _>(rows.iter().copied(), |r| r.1, |r| r.2), oracle::<7>(&rows));
        }
    }

    #[test]
    fn queued_build_shapes_match_stable_sort() {
        let mut state = 0x915a_b42du32;
        for count in 0..=256 {
            for _ in 0..16 {
                let rows: Vec<_> = (0..count).map(|id| {
                    state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                    let x = (state & 31) as i32 - 16;
                    let z = ((state >> 5) & 31) as i32 - 16;
                    (id, (x * x + z * z) as f32, state >> 27 != 0)
                }).collect();
                assert_eq!(nearest::<7, _>(rows.iter().copied(), |r| r.1, |r| r.2), oracle::<7>(&rows));
            }
        }
    }
}
