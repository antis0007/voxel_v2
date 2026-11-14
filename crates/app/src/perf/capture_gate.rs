//! Avoid repeated filesystem probes after a capture is already requested.
pub(crate) fn needed(last: Option<usize>, shot: usize, exists: impl FnOnce() -> bool) -> bool {
    last != Some(shot) && !exists()
}
#[cfg(test)]
mod tests {
    use super::needed;
    use std::cell::Cell;
    #[test]
    fn requested_shot_never_probes_filesystem() {
        assert!(!needed(Some(7),7,||panic!("unnecessary filesystem probe")));
    }
    #[test]
    fn new_shot_requests_only_missing_file() {
        assert!(needed(Some(1),2,||false));
        assert!(!needed(Some(1),2,||true));
        assert!(needed(None,0,||false));
        assert!(!needed(None,0,||true));
    }
    #[test]
    fn existing_file_can_be_removed_before_request() {
        let last=None;
        assert!(!needed(last,0,||true));
        assert!(needed(last,0,||false));
    }
    #[test]
    fn one_probe_per_requested_shot_across_frames() {
        let queries=Cell::new(0);let mut last=None;let mut requests=0;
        for shot in (0..3).flat_map(|shot|std::iter::repeat_n(shot,1500)) {
            if needed(last,shot,||{queries.set(queries.get()+1);false}) {
                last=Some(shot);requests+=1;
            }
        }
        assert_eq!(queries.get(),3);assert_eq!(requests,3);
    }
}
