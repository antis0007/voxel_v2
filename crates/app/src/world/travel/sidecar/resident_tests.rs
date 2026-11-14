use super::*;

#[test]
fn authored_and_constructor_homes_and_guards_round_trip() {
    for id in (0..64).chain(0x6000_0000..0x6000_000a).chain([1024, (1 << 28) + 1]) {
        let mut state = State::default();
        state.homes.insert(id, Vec3::new(8.5, 18.0, -3.5));
        state.guard_posts.insert(id, Vec3::new(-1.0, 18.0, 4.5));
        state.used.insert((id, 0), 2);
        let loaded = decode(&encode(&state)).expect("supported resident identity");
        assert_eq!(loaded.homes, state.homes, "resident {id}");
        assert_eq!(loaded.guard_posts, state.guard_posts);
        assert_eq!(loaded.used, state.used);
    }
}

#[test]
fn unknown_ids_and_invalid_home_positions_still_fail() {
    for id in [64, 1023, 0x6000_000a, u32::MAX] {
        let mut state = State::default();
        state.homes.insert(id, Vec3::ZERO);
        assert!(decode(&encode(&state)).is_err(), "unknown identity {id}");
        state.homes.clear();
        state.guard_posts.insert(id, Vec3::ZERO);
        assert!(decode(&encode(&state)).is_err(), "unknown guard {id}");
    }
    for pos in [Vec3::splat(f32::NAN), Vec3::splat(f32::INFINITY), Vec3::splat(50_000_000.0)] {
        let mut state = State::default();
        state.homes.insert(11, pos);
        assert!(decode(&encode(&state)).is_err());
    }
}

#[test]
fn too_many_homes_and_guards_still_fail() {
    let mut state = State::default();
    for id in 0..64 { state.homes.insert(id, Vec3::ZERO); }
    state.homes.insert(0x6000_0000, Vec3::ZERO);
    assert!(decode(&encode(&state)).is_err());
    state.homes.clear();
    for id in 0..=MAX_GUARDS as u32 { state.guard_posts.insert(id, Vec3::ZERO); }
    assert!(decode(&encode(&state)).is_err());
}
