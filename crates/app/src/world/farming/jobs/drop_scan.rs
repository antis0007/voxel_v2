//! Rotate bounded drop discovery independently of task retries.
const BUDGET: usize = 256;

pub fn window(cursor: &mut usize, len: usize)
    -> std::iter::Chain<std::ops::Range<usize>, std::ops::Range<usize>> {
    let start = if len == 0 { 0 } else { *cursor % len };
    let count = len.min(BUDGET);
    let first = count.min(len - start);
    let end = start + first;
    let remaining = count - first;
    *cursor = if remaining > 0 { remaining } else if end == len { 0 } else { end };
    (start..end).chain(0..remaining)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn visits_timber_after_rejected_prefix() {
        let mut cursor = 0;
        assert_eq!(window(&mut cursor, 300).filter(|i| *i >= 256).count(), 0);
        assert_eq!(window(&mut cursor, 300).filter(|i| *i >= 256).count(), 44);
    }
    #[test]
    fn covers_static_pools_with_bounded_unique_windows() {
        for len in [0usize, 1, 255, 256, 257, 511, 512, 1024] {
            let mut cursor = 0;
            let mut seen = BTreeSet::new();
            for _ in 0..len.div_ceil(BUDGET).max(1) {
                let batch: Vec<_> = window(&mut cursor, len).collect();
                assert!(batch.len() <= BUDGET);
                assert_eq!(batch.iter().copied().collect::<BTreeSet<_>>().len(), batch.len());
                assert!(batch.iter().all(|i| *i < len));
                seen.extend(batch);
            }
            assert_eq!(seen.len(), len);
        }
    }
    #[test]
    fn empty_and_shrinking_pools_normalize_cursor() {
        let mut cursor = usize::MAX;
        assert_eq!(window(&mut cursor, 3).count(), 3);
        assert!(cursor < 3);
        assert_eq!(window(&mut cursor, 0).count(), 0);
        assert_eq!(cursor, 0);
        assert_eq!(window(&mut cursor, 1).collect::<Vec<_>>(), [0]);
    }
    #[test]
    fn failed_selection_advances_workers_independently() {
        let mut lead = 0;
        let mut helper = 0;
        for expected in [0, 256, 512, 768, 0] {
            assert_eq!(window(&mut lead, 1024).next(), Some(expected));
        }
        assert_eq!(window(&mut helper, 1024).next(), Some(0));
        assert_eq!(window(&mut lead, 1024).next(), Some(256));
    }
    #[test]
    fn saved_cursor_resumes_without_restarting_prefix() {
        let mut cursor = 0;
        let _ = window(&mut cursor, 1024);
        let mut restored: usize = cursor.to_string().parse().unwrap();
        assert_eq!(window(&mut restored, 1024).next(), Some(256));
    }
    #[test]
    fn wrapping_does_not_overflow_large_lengths() {
        let mut cursor = usize::MAX - 2;
        let batch: Vec<_> = window(&mut cursor, usize::MAX).collect();
        assert_eq!(batch.len(), BUDGET);
        assert_eq!(&batch[..3], &[usize::MAX - 2, usize::MAX - 1, 0]);
        assert_eq!(cursor, BUDGET - 2);
    }
}
