use super::*;
use engine_core::hash::hash_u32;

fn h(a: u32, b: u32) -> u32 {
    hash_u32(a.wrapping_mul(0x9e37_79b9) ^ hash_u32(b))
}

/// Server cells in a 4x2x4-brick block at negative coordinates; 0 is air.
struct Scene {
    server: HashMap<IVec3, u32>,
    client: HashMap<IVec3, u32>,
    stream: CellStream,
    delta_bytes: usize,
    brick_bytes: usize,
    changes: usize,
}

const ORIGIN: IVec3 = IVec3::new(-40, -16, -70_000);

impl Scene {
    fn new() -> Self {
        Self {
            server: HashMap::new(),
            client: HashMap::new(),
            stream: CellStream::default(),
            delta_bytes: 0,
            brick_bytes: 0,
            changes: 0,
        }
    }

    fn set(&mut self, cell: IVec3, value: u32) {
        self.server.insert(cell, value);
        self.stream.changed(cell, value);
        self.changes += 1;
    }

    fn flush(&mut self, center: IVec3, budget: usize) -> Flush {
        let mut out = Vec::new();
        let flush = self.stream.flush(center, budget, &mut out);
        self.delta_bytes += out.len();
        for (cell, value) in read_flush(&out).unwrap() {
            self.client.insert(cell, value);
        }
        for &brick in &flush.resend {
            let mut cells = [0u32; BRICK_CELLS];
            for (i, c) in cells.iter_mut().enumerate() {
                *c = self.server.get(&cell_at(brick, i as u16)).copied().unwrap_or(0);
            }
            let mut bytes = Vec::new();
            write_brick(&cells, &mut bytes);
            self.brick_bytes += bytes.len();
            for (i, v) in read_brick(&bytes).unwrap().iter().enumerate() {
                self.client.insert(cell_at(brick, i as u16), *v);
            }
        }
        flush
    }

    fn agree(&self) -> bool {
        let keys = self.server.keys().chain(self.client.keys());
        keys.into_iter().all(|k| self.server.get(k).copied().unwrap_or(0) == self.client.get(k).copied().unwrap_or(0))
    }
}

#[test]
fn a_burst_of_flow_converges_within_budget_and_costs_less_than_raw_cells() {
    let mut s = Scene::new();
    for tick in 0..120u32 {
        // Water sloshing through the block: 400 changes a tick, many repeats.
        for n in 0..400u32 {
            let r = h(tick, n);
            let cell = ORIGIN + IVec3::new((r % 32) as i32, ((r >> 5) % 16) as i32, ((r >> 9) % 32) as i32);
            s.set(cell, 1 + (r >> 14) % 8);
        }
        assert!(s.flush(ORIGIN, 300).cells <= 300);
    }
    for _ in 0..(QUIET as usize + 2) {
        s.flush(ORIGIN, 300);
    }
    assert!(s.agree());
    assert_eq!(s.stream.pending_bricks(), 0);
    // Raw cells would be 12 bytes of position and 4 of value each. Measured: 79 KB of
    // deltas and 9 KB of brick resends against 768 KB raw.
    let raw = s.changes * 16;
    assert!((s.delta_bytes + s.brick_bytes) * 6 < raw, "{} + {} vs {raw}", s.delta_bytes, s.brick_bytes);
}

#[test]
fn nearest_bricks_go_first_and_far_ones_wait() {
    let mut s = Scene::new();
    let near = IVec3::new(-3, 0, -3);
    let far = near + IVec3::new(200, 0, 0);
    for i in 0..10 {
        s.set(near + IVec3::new(0, i, 0), 5);
        s.set(far + IVec3::new(0, i, 0), 5);
    }
    let first = s.flush(near, 10);
    assert_eq!(first.cells, 10);
    assert_eq!(s.client.get(&(near + IVec3::Y * 9)), Some(&5));
    assert_eq!(s.client.get(&far), None);
    s.flush(near, 10);
    assert!(s.agree());
}

#[test]
fn an_overfull_brick_is_resent_whole_once_quiet() {
    let mut s = Scene::new();
    for i in 0..(STALE_CELLS as i32 + 1) {
        s.set(ORIGIN + IVec3::new(i % 8, (i / 8) % 8, i / 64), 3);
    }
    let resent: Vec<Flush> = (0..QUIET + 1).map(|_| s.flush(ORIGIN, 1_000)).collect();
    assert!(resent[..QUIET as usize - 1].iter().all(|f| f.cells == 0 && f.resend.is_empty()));
    assert_eq!(resent.iter().map(|f| f.resend.len()).sum::<usize>(), 1);
    assert!(s.agree());
}

#[test]
fn a_brick_that_never_quiets_is_still_resent() {
    let mut s = Scene::new();
    let mut resends = 0;
    for tick in 0..(MAX_STALE as i32 + 5) {
        for i in 0..(STALE_CELLS as i32 + 1) {
            s.set(ORIGIN + IVec3::new(i % 8, (i / 8) % 8, i / 64), (tick % 7) as u32);
        }
        resends += s.flush(ORIGIN, 1_000).resend.len();
    }
    assert!(resends >= 1);
}

#[test]
fn hostile_flushes_and_bricks_are_refused() {
    let mut bad = Vec::new();
    [0i64, 0, 0].iter().for_each(|&c| put_i64(&mut bad, c));
    put_u64(&mut bad, 1);
    [0i64, 0, 0].iter().for_each(|&c| put_i64(&mut bad, c));
    put_u64(&mut bad, 1);
    put_u64(&mut bad, 600);
    put_u64(&mut bad, 1);
    assert!(read_flush(&bad).is_err());
    let mut brick = Vec::new();
    write_brick(&[7; BRICK_CELLS], &mut brick);
    assert!(brick.len() <= 6, "{}", brick.len());
    assert_eq!(read_brick(&brick), Ok([7; BRICK_CELLS]));
    let last = brick.len() - 1;
    brick[last] = 0x7f;
    assert!(read_brick(&brick).is_err());
}

#[test]
fn cells_map_to_bricks_by_floor_division() {
    for cell in [IVec3::new(-1, -8, -9), IVec3::new(7, 8, -70_001), IVec3::ZERO] {
        assert_eq!(cell_at(brick_of(cell), index_in_brick(cell)), cell);
    }
    assert_eq!(brick_of(IVec3::new(-1, -8, -9)), IVec3::new(-1, -1, -2));
}
