use super::*;
use crate::wire::Reader;

const HULL: u32 = 5;

fn hull_view(revision: u32) -> View {
    View { frame: HULL, center: IVec3::new(-3, 0, 2), revision }
}

#[test]
fn a_world_flush_costs_one_byte_more_than_before_frames() {
    let mut s = CellStream::default();
    s.changed(IVec3::new(-9, 0, 17), 4);
    let mut out = Vec::new();
    s.flush(IVec3::new(-9, 0, 17), 100, &mut out);
    // Before frames: home (-2, 0, 2), one brick at offset 0, one cell at index 7 + 8 * 0 + 64 * 1.
    let before = [3, 0, 4, 1, 0, 0, 0, 1, 71, 4];
    assert_eq!(out[0], 0);
    assert_eq!(&out[1..], &before);
    out.clear();
    s.flush(IVec3::ZERO, 100, &mut out);
    assert_eq!(out, [0, 0, 0, 0, 0]);
}

#[test]
fn cells_round_trip_across_two_frames() {
    let mut s = CellStream::default();
    s.baseline_sent(HULL, 3);
    let mut reader = CellReader::default();
    reader.install(&AssemblyBaseline { id: HULL, revision: 3, bounds: [IVec3::splat(-1), IVec3::ONE], bricks: Vec::new() });
    // The same local cell on the hull and in the world must stay apart.
    let shared = IVec3::new(-5, 1, 9);
    s.changed(shared, 1);
    s.changed_in(HULL, shared, 2);
    s.changed_in(HULL, IVec3::new(7, -8, 0), 3);
    let mut out = Vec::new();
    let flush = s.flush_frames(&[View::world(IVec3::new(100, 0, 0)), hull_view(4)], 100, &mut out);
    assert_eq!(flush.cells, 3);
    let mut got = reader.read(&out).unwrap();
    got.cells.sort_by_key(|&(f, c, _)| (f, c.x, c.y, c.z));
    assert_eq!(got.cells, vec![(0, shared, 1), (HULL, shared, 2), (HULL, IVec3::new(7, -8, 0), 3)]);
    assert!(got.resend.is_empty());
    assert_eq!(reader.revision(HULL), Some(4));
    assert_eq!(s.pending_bricks(), 0);
}

#[test]
fn a_frame_without_a_view_or_sent_baseline_stays_pending() {
    let mut s = CellStream::default();
    s.changed_in(HULL, IVec3::ZERO, 2);
    let mut out = Vec::new();
    assert_eq!(s.flush_frames(&[View::world(IVec3::ZERO), hull_view(1)], 100, &mut out).cells, 0);
    assert_eq!(out, [0, 0, 0, 0, 0]);
    s.changed_in(HULL, IVec3::ONE, 2);
    s.baseline_sent(HULL, 1);
    assert_eq!(s.pending_bricks(), 0, "the baseline covers earlier changes");
}

#[test]
fn a_flush_for_a_missing_baseline_is_refused_and_requests_a_resend() {
    let mut s = CellStream::default();
    s.baseline_sent(HULL, 0);
    s.changed(IVec3::ZERO, 9);
    s.changed_in(HULL, IVec3::ZERO, 2);
    let mut out = Vec::new();
    s.flush_frames(&[View::world(IVec3::ZERO), hull_view(1)], 100, &mut out);
    let mut reader = CellReader::default();
    let got = reader.read(&out).unwrap();
    assert_eq!(got.cells, vec![(0, IVec3::ZERO, 9)]);
    assert_eq!(got.resend, vec![HULL]);
    assert_eq!(reader.revision(HULL), None);
    assert!(read_flush(&out).is_err());
}

#[test]
fn a_skipped_revision_is_refused_until_a_new_baseline() {
    let mut s = CellStream::default();
    s.baseline_sent(HULL, 3);
    let base = AssemblyBaseline { id: HULL, revision: 3, bounds: [IVec3::ZERO; 2], bricks: Vec::new() };
    let mut reader = CellReader::default();
    reader.install(&base);
    let mut flushes = Vec::new();
    for (n, revision) in [4u32, 9, 10].into_iter().enumerate() {
        s.changed_in(HULL, IVec3::new(n as i32, 0, 0), revision);
        let mut out = Vec::new();
        s.flush_frames(&[hull_view(revision)], 100, &mut out);
        flushes.push(out);
    }
    // The flush to revision 4 is lost.
    let got = reader.read(&flushes[1]).unwrap();
    assert_eq!((got.cells.len(), got.resend), (0, vec![HULL]));
    assert_eq!(reader.read(&flushes[2]).unwrap().resend, vec![HULL]);
    reader.install(&AssemblyBaseline { revision: 9, ..base });
    let got = reader.read(&flushes[2]).unwrap();
    assert_eq!((got.cells, got.resend), (vec![(HULL, IVec3::new(2, 0, 0), 10)], vec![]));
    assert_eq!(reader.revision(HULL), Some(10));
}

#[test]
fn a_revision_bump_alone_still_reaches_the_client() {
    let mut s = CellStream::default();
    s.baseline_sent(HULL, 2);
    let mut out = Vec::new();
    s.flush_frames(&[hull_view(2)], 100, &mut out);
    assert!(out.is_empty(), "nothing new for the hull");
    s.flush_frames(&[hull_view(6)], 100, &mut out);
    let mut reader = CellReader::default();
    reader.install(&AssemblyBaseline { id: HULL, revision: 2, bounds: [IVec3::ZERO; 2], bricks: Vec::new() });
    assert_eq!(reader.read(&out), Ok(Received::default()));
    assert_eq!(reader.revision(HULL), Some(6));
}

#[test]
fn malformed_runs_are_refused_and_change_nothing() {
    let mut reader = CellReader::default();
    reader.install(&AssemblyBaseline { id: HULL, revision: 1, bounds: [IVec3::ZERO; 2], bricks: Vec::new() });
    let mut bytes = Vec::new();
    for _ in 0..2 {
        put_u64(&mut bytes, u64::from(HULL));
        bytes.extend([1, 1, 0, 0, 0, 0]);
    }
    assert!(reader.read(&bytes).is_err(), "repeated frame");
    assert!(reader.read(&[HULL as u8, 1, 1]).is_err(), "short run");
    assert!(reader.read(&[HULL as u8, 0xff, 0xff, 0xff, 0xff, 0x0f, 1, 0, 0, 0, 0]).is_err(), "revision overflow");
    assert_eq!(reader.revision(HULL), Some(1));
}

fn baseline() -> AssemblyBaseline {
    let mut deck = Box::new([0u32; BRICK_CELLS]);
    deck[..64].fill(12);
    let mut mast = Box::new([0u32; BRICK_CELLS]);
    mast.iter_mut().enumerate().for_each(|(i, c)| *c = (i % 3) as u32);
    AssemblyBaseline {
        id: 0xffff_ffff,
        revision: 41,
        bounds: [IVec3::new(-4, -1, -6), IVec3::new(3, 5, 2)],
        bricks: vec![(IVec3::new(-4, -1, -6), deck), (IVec3::new(0, 5, 2), mast)],
    }
}

#[test]
fn assembly_baselines_round_trip() {
    let b = baseline();
    let mut out = Vec::new();
    b.encode(&mut out);
    assert_eq!(AssemblyBaseline::decode(&out), Ok(b));
    let mut r = Reader::new(&out);
    assert_eq!(r.u32(), Ok(u32::MAX));
}

#[test]
fn hostile_baselines_are_refused() {
    let mut zero = Vec::new();
    AssemblyBaseline { id: 0, ..baseline() }.encode(&mut zero);
    assert!(AssemblyBaseline::decode(&zero).is_err());
    let mut tight = Vec::new();
    AssemblyBaseline { bounds: [IVec3::new(-4, -1, -6), IVec3::new(-4, -1, -6)], ..baseline() }.encode(&mut tight);
    assert_eq!(AssemblyBaseline::decode(&tight).map(|b| b.bricks.len()), Ok(1), "out-of-bounds bricks are not written");
    let mut out = Vec::new();
    baseline().encode(&mut out);
    out.push(0);
    assert!(AssemblyBaseline::decode(&out).is_err());
    let mut huge = Vec::new();
    [1u64, 1].iter().for_each(|&v| put_u64(&mut huge, v));
    (0..3).for_each(|_| put_i64(&mut huge, 0));
    (0..3).for_each(|_| put_u64(&mut huge, 0));
    put_u64(&mut huge, u64::MAX);
    assert!(AssemblyBaseline::decode(&huge).is_err());
    let mut far = Vec::new();
    AssemblyBaseline { bounds: [IVec3::splat(read::LIMIT - 1), IVec3::splat(read::LIMIT)], bricks: Vec::new(), ..baseline() }.encode(&mut far);
    assert!(AssemblyBaseline::decode(&far).is_ok());
    let mut over = Vec::new();
    AssemblyBaseline { bounds: [IVec3::splat(read::LIMIT), IVec3::splat(i32::MAX)], bricks: Vec::new(), ..baseline() }.encode(&mut over);
    assert!(AssemblyBaseline::decode(&over).is_err());
}
