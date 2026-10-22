//! Per-worker bounded caches. No locks are held while evaluating dependencies.
use super::{BushProfile, ColumnSample, TreeProfile, WorldSampleConfig};
use std::cell::RefCell;
use crate::fast_hash::FastMap;

#[derive(Default)]
struct Cache {
    cfg: Option<WorldSampleConfig>,
    columns: FastMap<(i32, i32), ColumnSample>,
    trees: FastMap<(i32, i32), Option<TreeProfile>>,
    bushes: FastMap<(i32, i32), Option<BushProfile>>,
}
thread_local! { static CACHE: RefCell<Cache> = RefCell::new(Cache::default()); }

macro_rules! cached {
    ($name:ident, $field:ident, $value:ty, $calculate:path, $limit:expr) => {
        pub(super) fn $name(cfg: &WorldSampleConfig, x: i32, z: i32) -> $value {
            let found = CACHE.with(|cache| {
                let mut c = cache.borrow_mut();
                if c.cfg != Some(*cfg) {
                    *c = Cache {
                        cfg: Some(*cfg),
                        ..Default::default()
                    };
                }
                c.$field.get(&(x, z)).copied()
            });
            if let Some(value) = found {
                return value;
            }
            let value = $calculate(cfg, x, z);
            CACHE.with(|cache| {
                let mut c = cache.borrow_mut();
                if c.$field.len() >= $limit {
                    c.$field.clear();
                }
                c.$field.insert((x, z), value);
            });
            value
        }
    };
}
// A 34x34 padded chunk consumes 1156 columns. The previous 8192-entry cap
// covered about seven chunks per worker and cleared wholesale, which forced
// repeated climate/erosion work while a worker walked a terrain frontier.
cached!(
    column,
    columns,
    ColumnSample,
    super::sample_column_uncached,
    32_768
);
cached!(
    tree,
    trees,
    Option<TreeProfile>,
    super::tree_profile_uncached,
    4096
);
cached!(
    bush,
    bushes,
    Option<BushProfile>,
    super::bush_profile_uncached,
    4096
);
