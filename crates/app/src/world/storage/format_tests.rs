use super::*;

#[test]
fn save_magic_preserves_both_historical_header_shapes() {
    assert_eq!(save_magic(1), *b"VXWSAVE1");
    assert_eq!(save_magic(9), *b"VXWSAVE9");
    assert_eq!(save_magic(10), *b"VXWSAV10");
    assert_eq!(save_magic(70), *b"VXWSAV70");
    assert_eq!(save_magic(71), *b"VXWSAV71");
    assert_eq!(save_magic(99), *b"VXWSAV99");
    for version in 1..=SAVE_VERSION {
        assert_eq!(save_version(&save_magic(version)).unwrap(), version);
    }
    assert!(save_version(&save_magic(SAVE_VERSION + 1)).is_err());
    assert_eq!(*MAGIC, save_magic(SAVE_VERSION));
}

#[test]
fn session_identity_labels_the_requested_version_without_fixture_rewriting() {
    let world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17);
    for version in [9, 10, 17, 49, 68, 70, SAVE_VERSION] {
        let header =
            session_identity(&world, &Player::new(), &DayNightCycle::default(), version).unwrap();
        assert_eq!(save_version(&header).unwrap(), version);
        assert_eq!(&header[..8], &save_magic(version));
    }
}

#[test]
fn production_header_round_trips_embedded_residents_travel_and_activity() {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 71);
    world.farming.clock = 1234;
    world.farming.nursery_stock = 7;
    world.travel.day = 17;
    world.activity.tick = 791;
    world.activity.recovery = true;
    let player = Player::new();
    let day = DayNightCycle::default();
    let mut footer = session_footer(&world, SAVE_VERSION);
    world.activity.snapshot().encode(&mut footer);
    // Exactly the production header -> footer -> activity -> manifest path.
    // In particular, never rewrite the header to make the reader accept it.
    let bytes = manifest(
        session_header(&world, &player, &day).unwrap(),
        &world.regions.view,
        &footer,
        1,
    )
    .unwrap();
    assert_eq!(&bytes[..8], MAGIC);
    let slot = std::env::temp_dir()
        .join(format!("voxel-format-{}", std::process::id()))
        .join("quicksave.vxw");
    let loaded = load_bytes(&bytes, &slot).unwrap();
    assert_eq!(loaded.world.farming.clock, 1234);
    assert_eq!(loaded.world.farming.nursery_stock, 7);
    assert_eq!(loaded.world.travel.day, 17);
    assert_eq!(loaded.world.activity.tick, 791);
    assert!(loaded.world.activity.recovery);

    // Reproduce the release defect in memory. A valid checksum cannot make a
    // v71 footer into a v70 layout, and normal legacy decoding must not guess.
    let mut mislabeled = bytes;
    mislabeled[..8].copy_from_slice(&save_magic(70));
    let end = mislabeled.len() - 8;
    let hash = checksum(&mislabeled[..end]);
    mislabeled[end..].copy_from_slice(&hash.to_le_bytes());
    let error = load_bytes(&mislabeled, &slot)
        .err()
        .expect("mislabeled layout is rejected");
    assert!(
        error
            .to_string()
            .contains("Invalid simulation recovery flag"),
        "{error:#}"
    );
}
