//! Recognition requires inspecting an unchanged physical inscription previously
//! recorded in this world's personal archive. Similar geometry is insufficient.
use super::{Knowledge, Patch, Saved, pages::Page};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Serialize, Deserialize)]
pub struct Landmark {
    layer: u32,
    text: Vec<String>,
}

pub(super) fn valid(saved: &Saved) -> bool {
    saved.landmarks.len() <= 4096
        && saved.landmarks.iter().all(|(cell, mark)| {
            (mark.layer as usize) < saved.layers.len()
                && mark.text.len() <= 16
                && mark.text.iter().map(String::len).sum::<usize>() <= 512
                && cell.iter().all(|v| (-4_000_000..=4_000_000).contains(v))
        })
        && saved.aliases.len() <= super::MAX_LAYERS
        && saved.aliases.keys().all(|&from| {
            let mut seen = BTreeSet::new();
            let mut at = from;
            loop {
                if at as usize >= saved.layers.len() || !seen.insert(at) {
                    return false;
                }
                match saved.aliases.get(&at) {
                    Some(&next) => at = next,
                    None => return true,
                }
            }
        })
}

impl Knowledge {
    pub(super) fn canonical(&self, mut id: u32) -> u32 {
        for _ in 0..super::MAX_LAYERS {
            let Some(&next) = self.saved.aliases.get(&id) else {
                break;
            };
            id = next;
        }
        id
    }

    pub fn inspect_record(&mut self, cell: IVec3, text: &[String]) {
        if !self.enabled
            || self.loading.is_some()
            || text.is_empty()
            || text.iter().all(|s| s.trim().is_empty())
            || text.len() > 16
            || text.iter().map(String::len).sum::<usize>() > 512
        {
            return;
        }
        let Some(active) = self.active else {
            return;
        };
        let target = self
            .saved
            .landmarks
            .get(&cell.to_array())
            .filter(|mark| mark.text == text)
            .map(|mark| self.canonical(mark.layer));
        if let Some(target) =
            target.filter(|&target| target != active && self.canonical(active) != target)
        {
            let old = &self.saved.layers[target as usize];
            let current = &self.saved.layers[active as usize];
            // Reading a sign from another floor does not merge the floors.
            if old.report.is_none() && old.accepts(self.feet, Vec3::from_array(current.up)) {
                self.saved.aliases.insert(active, target);
                self.saved.scope = old.scope;
                self.active = Some(target);
                self.selected = None;
                self.last_step = None;
                self.recent.clear();
                self.route_plan = None;
                self.dirty = true;
            }
        }
        let active = self.active.unwrap_or(active);
        if self.saved.landmarks.len() < 4096 || self.saved.landmarks.contains_key(&cell.to_array())
        {
            self.saved.landmarks.insert(
                cell.to_array(),
                Landmark {
                    layer: active,
                    text: text.to_vec(),
                },
            );
            self.dirty = true;
        }
    }

    pub(super) fn merge_recognized(&mut self) {
        // Four small COW pages per maintenance turn; aliases persist so a
        // checkpoint during reconciliation resumes without losing either chart.
        let candidates: Vec<_> = self
            .saved
            .patches
            .pages
            .keys()
            .filter(|(layer, _)| self.saved.aliases.contains_key(layer))
            .take(4)
            .copied()
            .collect();
        for source in candidates {
            let target = (self.canonical(source.0), source.1);
            if self.saved.cold.contains_key(&target) {
                self.pager.request(target);
                continue;
            }
            let Some(page) = self.saved.patches.pages.get(&source).cloned() else {
                continue;
            };
            let mut merged: Page = self
                .saved
                .patches
                .pages
                .get(&target)
                .map(|p| (**p).clone())
                .unwrap_or_default();
            for (&cell, incoming) in page.iter() {
                let old_key = (self.saved.layers[source.0 as usize].scope, cell);
                if let Some(layers) = self.locations.get_mut(&old_key) {
                    layers.remove(&source.0);
                }
                if self.locations.get(&old_key).is_some_and(BTreeSet::is_empty) {
                    self.locations.remove(&old_key);
                }
                if self.footsteps.get(&old_key) == Some(&source.0) {
                    self.footsteps.remove(&old_key);
                }
                let out = merged.entry(cell).or_insert_with(Patch::default);
                out.travelled |= incoming.travelled;
                for face in 0..6 {
                    if incoming.order[face] > out.order[face]
                        || (incoming.order[face] == out.order[face]
                            && out.faces[face].is_none()
                            && incoming.faces[face].is_some())
                    {
                        out.faces[face] = incoming.faces[face];
                        out.order[face] = incoming.order[face];
                    }
                }
                let scope = self.saved.layers[target.0 as usize].scope;
                self.locations
                    .entry((scope, cell))
                    .or_default()
                    .insert(target.0);
                if out.travelled {
                    self.footsteps.insert((scope, cell), target.0);
                }
            }
            self.saved.patches.remove_page(source);
            self.saved.patches.insert_page(target, merged);
            self.dirty = true;
        }
        for (&key, _) in self
            .saved
            .cold
            .iter()
            .filter(|((layer, _), _)| self.saved.aliases.contains_key(layer))
            .take(4)
        {
            self.pager.request(key);
        }
    }
}
