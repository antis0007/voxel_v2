//! Per-worker bounded caches. No locks are held while evaluating dependencies.
use super::{BushProfile, ColumnSample, TreeProfile, WorldSampleConfig};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Default)]
struct Cache {
    cfg: Option<WorldSampleConfig>,
    columns: HashMap<(i32, i32), ColumnSample>,
    trees: HashMap<(i32, i32), Option<TreeProfile>>,
    bushes: HashMap<(i32, i32), Option<BushProfile>>,
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
cached!(
    column,
    columns,
    ColumnSample,
    super::sample_column_uncached,
    8192
);
cached!(
    tree,
    trees,
    Option<TreeProfile>,
    super::tree_profile_uncached,
    1024
);
cached!(
    bush,
    bushes,
    Option<BushProfile>,
    super::bush_profile_uncached,
    1024
);
