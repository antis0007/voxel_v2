use super::*;

const VIEW: Radius = Radius { horizontal: 8, vertical: 4 };

fn ellipsoid(radius: Radius, grow: i32) -> usize {
    let (h, v) = (radius.horizontal + grow, radius.vertical + grow);
    (-v..=v)
        .flat_map(|y| (-h..=h).flat_map(move |z| (-h..=h).map(move |x| IVec3::new(x, y, z))))
        .filter(|&o| radius.contains(o, grow))
        .count()
}

#[test]
fn the_first_view_loads_the_whole_ellipsoid_nearest_and_ahead_first() {
    for center in [IVec3::ZERO, IVec3::new(-70_000, -3, 1 << 20)] {
        let mut t = ChunkTracker::default();
        let plan = t.update(center, Vec3::X, VIEW);
        assert_eq!(plan.load.len(), ellipsoid(VIEW, 0));
        assert!(plan.unload.is_empty());
        assert_eq!(plan.load[0], center);
        let rank = |c: IVec3| plan.load.iter().position(|&l| l == center + c);
        assert!(rank(IVec3::new(3, 0, 0)) < rank(IVec3::new(-3, 0, 0)));
        assert!(rank(IVec3::new(-2, 0, 0)) < rank(IVec3::new(-6, 0, 0)));
    }
}

#[test]
fn a_one_chunk_step_loads_a_face_and_stepping_back_loads_nothing() {
    let mut t = ChunkTracker::default();
    let start = IVec3::new(-1, 0, -1);
    t.update(start, Vec3::Z, VIEW);
    let forward = t.update(start + IVec3::X, Vec3::Z, VIEW);
    assert!(!forward.load.is_empty() && forward.load.len() < ellipsoid(VIEW, 0) / 8, "{}", forward.load.len());
    assert!(forward.unload.is_empty(), "the trailing face stays within radius + 1");
    let back = t.update(start, Vec3::Z, VIEW);
    assert!(back.load.is_empty() && back.unload.is_empty());
}

#[test]
fn chunks_unload_only_past_the_hysteresis_band() {
    let mut t = ChunkTracker::default();
    t.update(IVec3::ZERO, Vec3::X, VIEW);
    let moved = IVec3::new(5, 0, 0);
    let plan = t.update(moved, Vec3::X, VIEW);
    assert!(!plan.unload.is_empty());
    assert!(plan.unload.iter().all(|c| !VIEW.contains(*c - moved, 1)));
    assert!(t.held() <= ellipsoid(VIEW, 1));
    t.forget(moved);
    assert_eq!(t.update(moved, Vec3::X, VIEW).load, vec![moved]);
}

#[test]
fn delivery_generates_pristine_reuses_current_cache_and_sends_the_rest() {
    assert_eq!(delivery(0, 0xabc, Some(4)), Delivery::Generate { hash: 0xabc });
    assert_eq!(delivery(7, 0, Some(7)), Delivery::Cached);
    assert_eq!(delivery(7, 0, Some(6)), Delivery::Send);
    assert_eq!(delivery(7, 0, None), Delivery::Send);
}

#[test]
fn the_cache_offer_lists_the_nearest_chunks_first_and_stays_bounded() {
    let index: HashMap<IVec3, u32> = [(IVec3::new(i32::MIN, 0, 0), 1), (IVec3::new(2, 0, 0), 3), (IVec3::new(-1, 0, 0), 2)].into();
    let offer = cache_offer(&index, IVec3::new(i32::MAX, 0, 0), 2);
    assert_eq!(offer, vec![(IVec3::new(2, 0, 0), 3), (IVec3::new(-1, 0, 0), 2)]);
}
