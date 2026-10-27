use super::*;

fn sample() -> Waypoints {
    let mut w = Waypoints::default();
    w.add(IVec3::new(-120, 64, 3_000_000));
    w.add(IVec3::new(5, -7, 9));
    w.list[1].name = "Forge".into();
    w.cycle_colour(1);
    w.set_death(IVec3::new(1, 2, 3));
    w
}

#[test]
fn map_waypoint_sidecar_round_trips() {
    let w = sample();
    assert_eq!(decode_waypoints(&encode_waypoints(&w)).ok(), Some(w));
    let empty = Waypoints::default();
    assert_eq!(decode_waypoints(&encode_waypoints(&empty)).ok(), Some(empty));
}

#[test]
fn map_waypoint_sidecar_rejects_corruption() {
    let bytes = encode_waypoints(&sample());
    for i in [0, 9, bytes.len() / 2, bytes.len() - 1] {
        let mut bad = bytes.clone();
        bad[i] ^= 0x41;
        assert!(decode_waypoints(&bad).is_err(), "flip at {i}");
    }
    assert!(decode_waypoints(&bytes[..bytes.len() - 3]).is_err());
    assert!(decode_waypoints(&[]).is_err());
    assert!(decode_tiles(&bytes).is_err(), "wrong magic");
}

/// Well-formed checksums over bad payloads: the bounds, not the hash, reject them.
#[test]
fn map_waypoint_sidecar_bounds_hold_with_valid_checksum() {
    let resealed = |edit: &dyn Fn(&mut Vec<u8>)| {
        let bytes = encode_waypoints(&sample());
        let mut body = bytes[..bytes.len() - 8].to_vec();
        edit(&mut body);
        seal(body)
    };
    // Count far past the cap.
    assert!(decode_waypoints(&resealed(&|b| b[8..12].copy_from_slice(&u32::MAX.to_le_bytes()))).is_err());
    // Colour index outside the palette (first entry's colour byte).
    assert!(decode_waypoints(&resealed(&|b| b[24] = 200)).is_err());
    // Name length past the limit.
    assert!(decode_waypoints(&resealed(&|b| b[26] = 25)).is_err());
    // One more entry claimed than stored.
    assert!(decode_waypoints(&resealed(&|b| b[8..12].copy_from_slice(&4u32.to_le_bytes()))).is_err());
    // Trailing garbage.
    assert!(decode_waypoints(&resealed(&|b| b.push(0))).is_err());
}

#[test]
fn map_tile_sidecar_round_trips_and_rejects_corruption() {
    let mut a = vec![Color32::TRANSPARENT; COLUMNS];
    a[0] = Color32::from_rgb(10, 20, 30);
    a[COLUMNS - 1] = Color32::from_rgb(200, 100, 50);
    let b = vec![Color32::from_rgb(1, 2, 3); COLUMNS];
    let tiles = [(IVec2::new(-3, 7), a.clone()), (IVec2::new(4, -9), b.clone())];
    let bytes = encode_tiles(tiles.iter().map(|(k, c)| (*k, c.as_slice())));
    let back = decode_tiles(&bytes).expect("decode");
    assert_eq!(back, tiles.to_vec());
    let mut bad = bytes.clone();
    bad[20] ^= 1;
    assert!(decode_tiles(&bad).is_err());
    let mut short = bytes[..bytes.len() - 8 - 100].to_vec();
    short = seal(short);
    assert!(decode_tiles(&short).is_err());
}

#[test]
fn map_prefs_sidecar_round_trips() {
    for p in [Prefs::default(), Prefs { minimap: false, coordinates: true }, Prefs { minimap: true, coordinates: false }] {
        assert_eq!(decode_prefs(&encode_prefs(p)).ok(), Some(p));
    }
    assert!(decode_prefs(b"VXMMP001").is_err());
}
