use super::records::RECORD_CEILING;
use super::*;

const UNTOUCHED: u32 = reliquaries::PHYSICAL;
const DEFEATED_LEAD: u32 = reliquaries::PHYSICAL | 1;

fn full_table() -> Adventure {
    let mut a = Adventure::default();
    a.wand = 1;
    a.spell = 0;
    a.loadout.slots[0] = 0;
    for x in 0..MAX_RECORDS as i32 {
        a.records.insert(IVec3::new(x, 0, 0), if x % 2 == 0 { DEFEATED_LEAD } else { UNTOUCHED });
    }
    a
}

#[test]
fn adventure_new_site_past_the_record_cap_evicts_only_untouched_far_records() {
    let mut a = full_table();
    let fresh = IVec3::new(-2, 0, 0);
    let nearby = [fresh, IVec3::new(MAX_RECORDS as i32 - 1, 0, 0)];
    let last_untouched = MAX_RECORDS as i32 - 3;
    a.enemies.push(Enemy::new(IVec3::new(last_untouched, 0, 0), 1, 0, Vec3::ZERO, 0));
    assert!(a.admit_record(fresh, &nearby));
    *a.records.entry(fresh).or_default() |= UNTOUCHED;
    assert_eq!(a.records.len(), MAX_RECORDS);
    // The farthest untouched record without a live guardian or a nearby site.
    assert!(!a.records.contains_key(&IVec3::new(MAX_RECORDS as i32 - 5, 0, 0)));
    for kept in [MAX_RECORDS as i32 - 1, last_untouched] {
        assert_eq!(a.records[&IVec3::new(kept, 0, 0)], UNTOUCHED);
    }
    assert_eq!(a.records.values().filter(|m| **m == DEFEATED_LEAD).count(), MAX_RECORDS / 2);
}

#[test]
fn adventure_records_with_history_grow_to_a_soft_cap_that_still_loads() {
    let mut a = full_table();
    for record in a.records.values_mut() {
        *record = DEFEATED_LEAD;
    }
    let mut x = -1;
    while a.admit_record(IVec3::new(x, 0, 0), &[]) {
        a.records.insert(IVec3::new(x, 0, 0), DEFEATED_LEAD);
        x -= 1;
    }
    assert_eq!(a.records.len(), RECORD_CEILING);
    // At the ceiling an untouched record is still recycled for a new site.
    a.records.insert(IVec3::new(-1, 0, 0), UNTOUCHED);
    assert!(a.admit_record(IVec3::new(x, 0, 0), &[]));
    assert_eq!(a.records.len(), RECORD_CEILING - 1);
    assert!(!a.records.contains_key(&IVec3::new(-1, 0, 0)));
    assert!(a.records.values().all(|m| *m == DEFEATED_LEAD));

    let mut bytes = Vec::new();
    a.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    let mut read = || words.next().ok_or_else(|| anyhow::anyhow!("short"));
    let back = Adventure::decode(&mut read, true, true).expect("records under the ceiling must load");
    assert_eq!(back.records, a.records);
}
