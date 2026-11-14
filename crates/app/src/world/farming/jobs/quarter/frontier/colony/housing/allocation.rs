//! Retain valid occupants before filling vacancies in stable ID order.
use std::collections::{BTreeMap, BTreeSet};

pub fn assign(ids: &[u32], capacities: &[u8], old: &[(usize, Vec<u32>)]) -> Vec<(usize, Vec<u32>)> {
    let mut remaining: BTreeSet<_> = ids.iter().copied().collect();
    let mut homes = BTreeMap::<usize, Vec<u32>>::new();
    let mut ordered = old.to_vec();
    ordered.sort_by_key(|(project, _)| *project);
    for (project, mut members) in ordered {
        let cap = capacities.get(project).copied().unwrap_or(0) as usize;
        members.sort_unstable();
        for id in members {
            let home = homes.entry(project).or_default();
            if home.len() < cap && remaining.remove(&id) { home.push(id); }
        }
    }
    for (project, &capacity) in capacities.iter().enumerate() {
        let home = homes.entry(project).or_default();
        while home.len() < capacity as usize {
            let Some(id) = remaining.pop_first() else { break };
            home.push(id);
        }
        home.sort_unstable();
    }
    homes.into_iter().filter(|(_, members)| !members.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn household_allocation_is_order_independent_and_capacity_bounded() {
        let expected = vec![(0, vec![1, 2]), (2, vec![3])];
        assert_eq!(assign(&[4, 2, 1, 3, 1], &[2, 0, 1], &[]), expected);
        assert_eq!(assign(&[1, 2, 3, 4], &[2, 0, 1], &[]), expected);
    }
    #[test]
    fn household_allocation_retains_occupants_and_rehomes_invalidated_members() {
        let old = vec![(1, vec![2, 3]), (0, vec![1])];
        assert_eq!(assign(&[1, 2, 3, 4], &[2, 2], &old), vec![(0, vec![1, 4]), (1, vec![2, 3])]);
        assert_eq!(assign(&[1, 2, 3, 4], &[0, 3], &old), vec![(1, vec![1, 2, 3])]);
    }
    #[test]
    fn household_allocation_never_creates_people_or_uses_unready_homes() {
        assert!(assign(&[1, 2], &[0, 0], &[(0, vec![1])]).is_empty());
        assert_eq!(assign(&[2], &[4], &[(0, vec![1, 2, 9])]), vec![(0, vec![2])]);
        assert!(assign(&[], &[4], &[]).is_empty());
    }
}
