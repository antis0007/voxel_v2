//! One-entry per-thread memo for pure functions of a world column.
//!
//! `generate_padded` walks y innermost, so a column-only function called once
//! per voxel repeats the same `(cfg, x, z)` 34 times in a row. The memo keeps
//! the last answer per thread; it never changes a result because every wrapped
//! function is a pure function of its key.
use engine_voxel::WorldSampleConfig;
use std::cell::Cell;

type Key = (WorldSampleConfig, i32, i32);

/// Last key and value for one column-only function on this thread.
pub(crate) struct ColumnMemo<T: Copy + 'static> {
    last: Cell<Option<(Key, T)>>,
}

impl<T: Copy + 'static> ColumnMemo<T> {
    pub(crate) const fn new() -> Self {
        Self { last: Cell::new(None) }
    }

    /// Returns `compute()` for this column, reusing the previous call's value
    /// when the key repeats.
    pub(crate) fn get(&self, cfg: &WorldSampleConfig, x: i32, z: i32, compute: impl FnOnce() -> T) -> T {
        let key = (*cfg, x, z);
        if let Some((last, value)) = self.last.get()
            && last == key
        {
            return value;
        }
        let value = compute();
        self.last.set(Some((key, value)));
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_voxel::WorldType;

    fn cfg(seed: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            planet: None,
            generator_version: 1,
            seed,
            world_type: WorldType::Hills,
            voxel_scale: engine_core::VoxelScalePreset::OneMeter,
        }
    }

    #[test]
    fn repeats_reuse_and_any_key_change_recomputes() {
        let memo = ColumnMemo::<i32>::new();
        let calls = Cell::new(0);
        let f = |v| { calls.set(calls.get() + 1); v };
        assert_eq!(memo.get(&cfg(1), 3, -4, || f(10)), 10);
        assert_eq!(memo.get(&cfg(1), 3, -4, || f(99)), 10);
        assert_eq!(calls.get(), 1);
        assert_eq!(memo.get(&cfg(1), 3, -5, || f(11)), 11);
        assert_eq!(memo.get(&cfg(2), 3, -5, || f(12)), 12);
        assert_eq!(memo.get(&cfg(2), i32::MIN, i32::MAX, || f(13)), 13);
        assert_eq!(calls.get(), 4);
    }
}
