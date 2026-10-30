//! Small immutable pages make checkpoint snapshots cheap. The wire format stays
//! a flat map so VXOBS001 archives can be migrated without discarding knowledge.
use super::{MAX_PATCHES, Patch};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, Visitor},
    ser::SerializeMap,
};
use std::{collections::BTreeMap, fmt, sync::Arc};

pub type Key = (u32, [i32; 3]);
pub type Page = BTreeMap<[i32; 3], Patch>;
pub const SIDE: i32 = 8;

#[derive(Default, Clone)]
pub struct Pages {
    pub pages: BTreeMap<Key, Arc<Page>>,
    count: usize,
}

fn page((layer, cell): Key) -> Key {
    (layer, cell.map(|v| v.div_euclid(SIDE)))
}

impl Pages {
    pub fn remove_page(&mut self, key: Key) {
        if let Some(page) = self.pages.remove(&key) {
            self.count -= page.len();
        }
    }
    pub fn insert_page(&mut self, key: Key, page: Page) {
        self.remove_page(key);
        self.count += page.len();
        self.pages.insert(key, Arc::new(page));
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn get(&self, key: &Key) -> Option<&Patch> {
        self.pages.get(&page(*key))?.get(&key.1)
    }
    pub fn contains_key(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }
    pub fn get_mut(&mut self, key: &Key) -> Option<&mut Patch> {
        let p = self.pages.get_mut(&page(*key))?;
        Arc::make_mut(p).get_mut(&key.1)
    }
    pub fn insert(&mut self, key: Key, patch: Patch) {
        let p = self.pages.entry(page(key)).or_default();
        if Arc::make_mut(p).insert(key.1, patch).is_none() {
            self.count += 1;
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (Key, &Patch)> {
        self.pages
            .iter()
            .flat_map(|(&(layer, _), p)| p.iter().map(move |(&cell, v)| ((layer, cell), v)))
    }
    pub fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.iter().map(|(k, _)| k)
    }
    pub fn migrate(&mut self) {
        for page in self.pages.values_mut() {
            for patch in Arc::make_mut(page).values_mut() {
                patch.migrate();
            }
        }
    }
}

impl Serialize for Pages {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.count))?;
        for (key, value) in self.iter() {
            map.serialize_entry(&key, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Pages {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ReadPages;
        impl<'de> Visitor<'de> for ReadPages {
            type Value = Pages;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded observation map")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Pages, M::Error> {
                let mut pages = Pages::default();
                while let Some((key, patch)) = map.next_entry::<Key, Patch>()? {
                    if pages.len() >= MAX_PATCHES || pages.contains_key(&key) {
                        return Err(serde::de::Error::custom(
                            "too many or duplicate observations",
                        ));
                    }
                    pages.insert(key, patch);
                }
                Ok(pages)
            }
        }
        deserializer.deserialize_map(ReadPages)
    }
}
