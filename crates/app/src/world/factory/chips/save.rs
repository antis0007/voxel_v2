//! Save 34 block: the library, installed parts and the bay clock as
//! length-prefixed RON text packed into little-endian words.
use super::{Bay, Installed, Key, session};
use anyhow::{Result, ensure};
use engine_chip::library::Library;
use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;
use std::sync::Arc;

const MAX_TEXT: usize = 1 << 24;

#[derive(Serialize, Deserialize)]
pub(super) struct Saved {
    library: Library,
    installed: Vec<(Key, Installed)>,
    now: u64,
}

impl Bay {
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        let saved = Saved {
            library: (*self.library).clone(),
            installed: self.installed.iter().map(|(k, c)| (*k, c.clone())).collect(),
            now: self.now,
        };
        // Plain data always serialises; an empty library still decodes.
        let text = ron::to_string(&saved)
            .or_else(|_| ron::to_string(&Saved { library: Library::default(), installed: Vec::new(), now: self.now }))
            .unwrap_or_default()
            .into_bytes();
        bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
        for chunk in text.chunks(4) {
            let mut w = [0u8; 4];
            w[..chunk.len()].copy_from_slice(chunk);
            bytes.extend_from_slice(&w);
        }
    }

    pub fn decode(next: &mut impl FnMut() -> Result<u32>) -> Result<Self> {
        let len = next()? as usize;
        ensure!(len <= MAX_TEXT, "Chip library too large");
        let mut text = Vec::with_capacity(len.next_multiple_of(4));
        for _ in 0..len.div_ceil(4) {
            text.extend_from_slice(&next()?.to_le_bytes());
        }
        text.truncate(len);
        let saved: Saved = ron::from_str(std::str::from_utf8(&text)?)?;
        saved.library.validate().map_err(anyhow::Error::msg)?;
        let mut serials: Vec<u32> = saved.library.stock.iter().map(|p| p.serial).collect();
        for (_, c) in &saved.installed {
            ensure!(saved.library.serial_issued(c.package.serial), "Installed chip with an unissued serial");
            ensure!(saved.library.revision(c.package.revision).is_some(), "Installed chip without a revision");
            serials.push(c.package.serial);
        }
        let count = serials.len();
        serials.sort();
        serials.dedup();
        ensure!(serials.len() == count, "Chip package present twice");
        let mut library = saved.library;
        library.session = session();
        let mut bay = Self { library: Arc::new(library), installed: saved.installed.into_iter().collect(), now: saved.now, queue: BinaryHeap::new() };
        let keys: Vec<Key> = bay.installed.keys().copied().collect();
        for k in keys {
            bay.schedule(k);
        }
        Ok(bay)
    }

}
