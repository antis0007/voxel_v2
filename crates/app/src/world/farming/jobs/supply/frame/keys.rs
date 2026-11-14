type Key=([i32;3],u16);

#[derive(Clone, Debug)]
pub(super) struct Keys(Vec<Key>);

impl Keys {
    pub(super) fn new(keys: impl IntoIterator<Item=Key>) -> Self {
        let mut keys:Vec<_>=keys.into_iter().collect();
        keys.sort_unstable();keys.dedup();Self(keys)
    }
    pub(super) fn contains(&self, key: Key) -> bool { self.0.binary_search(&key).is_ok() }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]
    fn project_membership_matches_linear_scan_with_logarithmic_queries() {
        let orders:Vec<_>=(0..593).rev().map(|i|([i%29,18+i/29,i%7],(i%4) as u16)).collect();
        let indexed=Keys::new(orders.iter().copied());
        for key in orders.iter().copied().chain(orders.iter().map(|&(at,b)|(at,b+8))) {
            let mut comparisons=0;
            let found=indexed.0.binary_search_by(|candidate| {comparisons+=1;candidate.cmp(&key)}).is_ok();
            assert_eq!(found,orders.contains(&key));
            assert_eq!(indexed.contains(key),found);
            assert!(comparisons<=11,"{comparisons} comparisons for 593 cells");
        }
    }
    #[test]
    fn exact_block_identity_and_negative_coordinates_are_retained() {
        let keys=Keys::new([([-4,18,-9],3),([-4,18,-9],3),([1,-18,0],7)]);
        assert!(keys.contains(([-4,18,-9],3)));
        assert!(!keys.contains(([-4,18,-9],7)));
        assert!(keys.contains(([1,-18,0],7)));
        assert!(!Keys::new([]).contains(([0;3],0)));
    }
}
