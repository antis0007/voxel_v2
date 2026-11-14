use super::*;
use glam::Vec2;

fn world() -> World {
    World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7)
}

/// A lake of partial cells across negative and positive chunks, some moving.
fn lake(w: &mut World) {
    for x in -40..40 {
        for z in -40..40 {
            let p = IVec3::new(x, 199, z);
            w.water_levels.insert(p, 1 + (x + z).rem_euclid(7) as u8);
            if x % 5 == 0 {
                w.water_levels.set_velocity(p, Vec2::new(0.5, -0.25));
            }
        }
    }
}

struct Joiner {
    w: World,
    p: Player,
    day: DayNightCycle,
    blocks: HashSet<IVec3>,
    chunks: HashSet<IVec3>,
}

impl Joiner {
    fn new() -> Self {
        Self { w: world(), p: Player::new(), day: DayNightCycle::default(), blocks: HashSet::new(), chunks: HashSet::new() }
    }

    fn apply(&mut self, bytes: &[u8]) -> Applied {
        self.blocks.clear();
        self.chunks.clear();
        apply(bytes, &mut self.w, &mut self.p, &mut self.day, &mut self.blocks, &mut self.chunks).unwrap()
    }
}

fn send(host: &World, mirror: &mut Mirror, ack: u64) -> Vec<u8> {
    capture(host, &Player::new(), &DayNightCycle::default(), ack, Vec::new(), mirror).unwrap()
}

fn same_water(a: &World, b: &World) -> bool {
    let cells = |w: &World| {
        let mut v: Vec<_> = w.water_levels.snapshot_all().iter().map(|(&p, &l)| (p.to_array(), l, w.water_levels.velocity(p).to_array())).collect();
        v.sort_by_key(|c| c.0);
        v
    };
    cells(a) == cells(b)
}

#[test]
fn a_quiet_world_sends_a_header_after_the_first_image() {
    let mut host = world();
    lake(&mut host);
    let mut mirror = Mirror::default();
    let mut joiner = Joiner::new();
    let first = send(&host, &mut mirror, 3);
    let applied = joiner.apply(&first);
    assert_eq!(applied.command_ack, 3);
    assert!(!applied.water_mismatch && same_water(&host, &joiner.w));
    let quiet = send(&host, &mut mirror, 3);
    let applied = joiner.apply(&quiet);
    assert!(!applied.water_mismatch && joiner.chunks.is_empty(), "a quiet image remeshed {} chunks", joiner.chunks.len());
    let old = gameplay::capture(&host, &Player::new(), &DayNightCycle::default(), 3, Vec::new()).unwrap();
    // A quiet frame is header-sized; the full image resent everything every 100 ms.
    assert!(quiet.len() * 20 < old.len(), "quiet {} B vs full image {} B (first {} B)", quiet.len(), old.len(), first.len());
}

#[test]
fn changed_and_drained_cells_follow_the_host() {
    let mut host = world();
    lake(&mut host);
    let mut mirror = Mirror::default();
    let mut joiner = Joiner::new();
    joiner.apply(&send(&host, &mut mirror, 0));
    let quiet = send(&host, &mut mirror, 0).len();
    let (moved, drained, added) = (IVec3::new(-33, 199, 7), IVec3::new(12, 199, -38), IVec3::new(-70, 205, -70));
    host.water_levels.insert(moved, 8);
    host.water_levels.set_velocity(moved, Vec2::new(-1.0, 2.0));
    host.water_levels.remove(&drained);
    host.water_levels.remove_velocity(&drained);
    host.water_levels.insert(added, 2);
    // A wave that only changes speed is still sent.
    host.water_levels.set_velocity(IVec3::new(5, 199, 5), Vec2::new(3.0, 0.0));
    let delta = send(&host, &mut mirror, 0);
    let applied = joiner.apply(&delta);
    assert!(!applied.water_mismatch && same_water(&host, &joiner.w));
    for p in [moved, drained, added] {
        assert!(joiner.chunks.contains(&engine_core::chunk_coord_from_world_block(p)), "{p} not remeshed");
    }
    assert!(delta.len() < quiet + 60, "four cells cost {} B over a {quiet} B quiet image", delta.len() - quiet);
}

#[test]
fn a_joiner_whose_water_drifts_asks_for_and_gets_a_full_list() {
    let mut host = world();
    lake(&mut host);
    let mut mirror = Mirror::default();
    let mut joiner = Joiner::new();
    joiner.apply(&send(&host, &mut mirror, 0));
    joiner.w.water_levels.insert(IVec3::new(100, 199, 100), 4);
    joiner.w.water_levels.remove(&IVec3::new(0, 199, 0));
    assert!(joiner.apply(&send(&host, &mut mirror, 0)).water_mismatch);
    mirror.resend_water();
    assert!(!joiner.apply(&send(&host, &mut mirror, 0)).water_mismatch);
    assert!(same_water(&host, &joiner.w));
}

#[test]
fn delta_images_preserve_locally_stepped_viewmodel_motion() {
    let host = world();
    let mut mirror = Mirror::default();
    let mut joiner = Joiner::new();
    joiner.p.viewmodel.mine = 0.75;
    joiner.apply(&send(&host, &mut mirror, 0));
    assert_eq!(joiner.p.viewmodel.mine, 0.75);
    joiner.apply(&send(&host, &mut mirror, 0));
    assert_eq!(joiner.p.viewmodel.mine, 0.75);
}

#[test]
fn unchanged_sections_are_skipped_until_a_refresh() {
    let host = world();
    let mut mirror = Mirror::default();
    let mut joiner = Joiner::new();
    joiner.apply(&send(&host, &mut mirror, 0));
    let quiet = send(&host, &mut mirror, 0);
    mirror.refresh();
    let refreshed = send(&host, &mut mirror, 0);
    assert!(refreshed.len() > quiet.len() * 4, "refresh {} B vs quiet {} B", refreshed.len(), quiet.len());
    joiner.apply(&refreshed);
}

#[test]
fn damaged_images_are_refused_before_anything_is_published() {
    let mut host = world();
    lake(&mut host);
    let mut mirror = Mirror::default();
    let bytes = send(&host, &mut mirror, 0);
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(&bytes[..]).read_to_end(&mut raw).unwrap();
    let pack = |raw: &[u8]| {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(raw).unwrap();
        e.finish().unwrap()
    };
    let mut joiner = Joiner::new();
    let mut try_apply = |raw: &[u8]| apply(&pack(raw), &mut joiner.w, &mut joiner.p, &mut joiner.day, &mut joiner.blocks, &mut joiner.chunks).is_err();
    assert!(try_apply(&raw[..raw.len() - 1]), "truncated water accepted");
    let mut wrong = raw.clone();
    wrong[0] = VERSION + 1;
    assert!(try_apply(&wrong), "wrong version accepted");
    let mut extra = raw.clone();
    extra.extend_from_slice(&[HEADER, 0]);
    assert!(try_apply(&extra), "out-of-order section accepted");
    assert!(joiner.w.water_levels.snapshot_all().iter().next().is_none(), "a refused image published water");
}

