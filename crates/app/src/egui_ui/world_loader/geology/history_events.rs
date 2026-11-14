//! Timestamp window for retained history events.
use std::ops::Range;

pub const WINDOW_MYR: f64 = 4.0;

/// Includes the next snapshot, whose events precede its endpoint.
pub fn range<T>(frames: &[T], myr: f64, timestamp: impl Fn(&T) -> f64) -> Range<usize> {
    let start = frames.partition_point(|f| timestamp(f) < myr - WINDOW_MYR);
    let end = (frames.partition_point(|f| timestamp(f) <= myr) + 1).min(frames.len());
    start.min(end)..end
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window(times: &[u16], myr: f64) -> &[u16] {
        &times[range(times, myr, |t| f64::from(*t))]
    }
    #[test]
    fn one_myr_frames_keep_older_live_events() {
        assert_eq!(window(&[5, 6, 7, 8, 9, 10, 11, 12], 10.5), &[7, 8, 9, 10, 11]);
    }
    #[test]
    fn five_myr_frames_keep_original_endpoints() {
        assert_eq!(window(&[0, 5, 10, 15], 7.5), &[5, 10]);
    }
    #[test]
    fn endpoint_events_are_included_once() {
        assert_eq!(window(&[0, 1, 2, 3, 4, 5, 6], 4.0), &[0, 1, 2, 3, 4, 5]);
        assert_eq!(window(&[0], 0.0), &[0]);
    }
    #[test]
    fn empty_or_expired_history_has_no_events() {
        assert!(window(&[], 2.0).is_empty());
        assert!(window(&[0, 1, 2], 7.0).is_empty());
    }
    #[test]
    fn late_published_events_remain_available_before_the_endpoint() {
        assert_eq!(window(&[0, 5], 1.25), &[0, 5]);
    }
}
