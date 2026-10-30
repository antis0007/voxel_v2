//! Historical directed traversal. No live topology or destination lookup.
use super::{Knowledge, Layer, MAX_LAYERS};
use egui::{Color32, Pos2, Stroke, emath::Rot2};
use glam::Vec3;
use serde::{Deserialize, Serialize};
pub const MAX_ROUTES: usize = 4096;

pub struct Crossing {
    pub from: u32,
    pub opening: u64,
    pub revision: u64,
    pub entry: Vec3,
    pub exit: Vec3,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Route {
    from: u32,
    to: u32,
    entry: [f32; 3],
    exit: [f32; 3],
    opening: u64,
    revision: u64,
    passages: u32,
}

impl Route {
    pub fn valid(&self, layers: usize) -> bool {
        (self.from as usize) < layers
            && (self.to as usize) < layers
            && self.passages > 0
            && [self.entry, self.exit].iter().all(|p| {
                Vec3::from_array(*p).is_finite()
                    && Vec3::from_array(*p).abs().max_element() < 1_000_000.0
            })
    }
}

impl Knowledge {
    pub fn chart_name(&self, id: u32) -> String {
        let id = self.canonical(id);
        self.saved
            .names
            .get(&id)
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| format!("Level {}", id + 1))
    }

    fn arrival_chart(&mut self, point: Vec3, up: Vec3) -> Option<u32> {
        if self.saved.layers.len() >= MAX_LAYERS {
            return None;
        }
        let reference = if up.dot(Vec3::Y).abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::NEG_Z
        };
        let forward = (reference - up * reference.dot(up)).normalize_or_zero();
        let layer = Layer {
            scope: self.saved.scope,
            origin: point.to_array(),
            up: up.to_array(),
            right: forward.cross(up).normalize_or_zero().to_array(),
            forward: forward.to_array(),
            report: None,
        };
        if !layer.valid() {
            return None;
        }
        let id = self.saved.layers.len() as u32;
        self.saved.layers.push(layer);
        self.dirty = true;
        Some(id)
    }

    pub fn crossings(
        &mut self,
        events: &crate::player::observations::Crossings,
        root: &std::path::Path,
    ) {
        self.finish_load(false);
        if self.loading.is_some() || !self.enabled {
            return;
        }
        if self.observer_epoch != events.epoch {
            self.observer_epoch = events.epoch;
            self.last_crossing = 0;
        }
        for event in &events.events {
            if event.sequence <= self.last_crossing {
                continue;
            }
            if event.sequence > self.last_crossing.saturating_add(1) {
                self.active = None;
            }
            self.last_crossing = event.sequence;
            if event.root.as_path() != root {
                continue;
            }
            let from = self
                .active
                .filter(|&i| self.saved.layers[i as usize].accepts(event.entry, event.source_up))
                .or_else(|| self.arrival_chart(event.entry, event.source_up));
            self.saved.scope = self
                .saved
                .layers
                .iter()
                .map(|l| l.scope)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            self.active = self.arrival_chart(event.exit, event.up);
            self.selected = None;
            self.recent.clear();
            self.last_step = None;
            self.last_position = Some(event.exit);
            self.pending_arrival = false;
            if let (Some(from), Some(to)) = (from, self.active) {
                self.record_route(
                    Crossing {
                        from,
                        opening: event.opening,
                        revision: event.revision,
                        entry: event.entry,
                        exit: event.exit,
                    },
                    to,
                );
            }
        }
    }

    pub(super) fn record_route(&mut self, c: Crossing, to: u32) {
        if let Some(route) = self
            .saved
            .routes
            .iter_mut()
            .find(|r| r.from == c.from && r.to == to && r.opening == c.opening)
        {
            route.passages = route.passages.saturating_add(1);
            route.revision = c.revision;
        } else if self.saved.routes.len() < MAX_ROUTES {
            self.saved.routes.push(Route {
                from: c.from,
                to,
                opening: c.opening,
                revision: c.revision,
                entry: c.entry.to_array(),
                exit: c.exit.to_array(),
                passages: 1,
            });
        }
        self.dirty = true;
    }

    pub(super) fn record_connector(&mut self, from: u32, to: u32, point: Vec3) {
        // A transition between walking charts establishes only the direction
        // actually taken. No invented reverse stair, ladder or return portal.
        if self.saved.layers[from as usize].scope == self.saved.layers[to as usize].scope {
            self.record_route(
                Crossing {
                    from,
                    opening: 0,
                    revision: 0,
                    entry: point,
                    exit: point,
                },
                to,
            );
        }
    }

    pub(super) fn routes_ui(&mut self, ui: &mut egui::Ui) {
        let current = self.selected.or(self.active);
        if let (Some(from), Some(to)) = (self.active, current) {
            if from != to && ui.small_button("Known route").clicked() {
                self.route_plan = Some(self.find_route(from, to));
            }
        }
        if let Some(plan) = &self.route_plan {
            if plan.is_empty() {
                ui.label("No known route");
            } else {
                let label = plan
                    .iter()
                    .map(|&id| self.chart_name(id))
                    .collect::<Vec<_>>()
                    .join(" → ");
                ui.label(label).on_hover_text("Last crossed connections");
            }
        }
        let routes: Vec<_> = self
            .saved
            .routes
            .iter()
            .filter(|r| {
                Some(self.canonical(r.from)) == current || Some(self.canonical(r.to)) == current
            })
            .take(64)
            .map(|r| {
                (
                    self.canonical(r.from),
                    self.canonical(r.to),
                    r.opening == 0,
                    r.passages,
                )
            })
            .collect();
        egui::ScrollArea::vertical()
            .id_salt("known-routes")
            .max_height(180.0)
            .show(ui, |ui| {
                for (from, to, walked, count) in routes {
                    let label = format!("{} → {}", self.chart_name(from), self.chart_name(to));
                    if ui
                        .small_button(label)
                        .on_hover_text(format!(
                            "{} · {} crossings",
                            if walked { "Walked" } else { "Last crossed" },
                            count
                        ))
                        .clicked()
                    {
                        self.selected = Some(to);
                    }
                }
            });
    }

    fn find_route(&self, from: u32, to: u32) -> Vec<u32> {
        use std::collections::{BTreeMap, VecDeque};
        let mut edges: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for route in &self.saved.routes {
            edges
                .entry(self.canonical(route.from))
                .or_default()
                .push(self.canonical(route.to));
        }
        let mut previous = BTreeMap::from([(from, from)]);
        let mut queue = VecDeque::from([from]);
        while let Some(node) = queue.pop_front() {
            if node == to {
                let mut path = vec![to];
                let mut at = to;
                while at != from {
                    if path.len() >= 128 {
                        return Vec::new();
                    }
                    at = previous[&at];
                    path.push(at);
                }
                path.reverse();
                return path;
            }
            if let Some(next) = edges.get(&node) {
                for &next in next {
                    if let std::collections::btree_map::Entry::Vacant(slot) = previous.entry(next) {
                        slot.insert(node);
                        queue.push_back(next);
                    }
                }
            }
        }
        Vec::new()
    }

    pub(super) fn draw_routes(
        &self,
        p: &egui::Painter,
        id: u32,
        centre: Pos2,
        origin: egui::Vec2,
        zoom: f32,
        rot: Rot2,
    ) {
        let layer = &self.saved.layers[id as usize];
        for route in self
            .saved
            .routes
            .iter()
            .filter(|r| self.canonical(r.from) == id || self.canonical(r.to) == id)
            .take(128)
        {
            let outgoing = self.canonical(route.from) == id;
            let point = Vec3::from_array(if outgoing { route.entry } else { route.exit });
            let at = centre + rot * (layer.project(point) - origin) * zoom;
            if !p.clip_rect().contains(at) {
                continue;
            }
            let colour = if outgoing {
                Color32::from_rgb(203, 168, 91)
            } else {
                Color32::from_rgb(125, 167, 182)
            };
            p.circle_stroke(at, 4.0, Stroke::new(1.5, colour));
            if outgoing {
                p.line_segment([at, at + egui::vec2(0.0, -7.0)], Stroke::new(1.5, colour));
            }
        }
    }
}
