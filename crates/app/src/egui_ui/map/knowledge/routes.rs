//! Historical directed traversal. No live topology or destination lookup.
use super::navigation::Travel;
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
    #[serde(default)]
    travel: Option<Travel>,
    #[serde(default)]
    metres: Option<f32>,
}

impl Route {
    pub fn valid(&self, layers: usize) -> bool {
        (self.from as usize) < layers
            && (self.to as usize) < layers
            && self.passages > 0
            && self
                .metres
                .is_none_or(|m| m.is_finite() && (0.0..=512.0).contains(&m))
            && [self.entry, self.exit].iter().all(|p| {
                Vec3::from_array(*p).is_finite()
                    && Vec3::from_array(*p).abs().max_element() < 1_000_000.0
            })
    }
    fn travel(&self) -> Travel {
        // Older archives did not distinguish walking from airborne transitions.
        self.travel.unwrap_or(if self.opening == 0 {
            Travel::Traverse
        } else {
            Travel::Portal
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
            self.navigation = Default::default();
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
                self.navigation = Default::default();
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
            self.navigation = Default::default();
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
                    Travel::Portal,
                    None,
                );
            }
        }
    }

    pub(super) fn record_route(
        &mut self,
        c: Crossing,
        to: u32,
        travel: Travel,
        metres: Option<f32>,
    ) {
        let from = self.canonical(c.from);
        let to = self.canonical(to);
        if from == to && travel != Travel::Portal {
            return;
        }
        let existing = self.saved.routes.iter().position(|r| {
            self.canonical(r.from) == from
                && self.canonical(r.to) == to
                && r.opening == c.opening
                && r.travel() == travel
                && Vec3::from_array(r.entry).distance_squared(c.entry) <= 1.0
                && Vec3::from_array(r.exit).distance_squared(c.exit) <= 1.0
        });
        if let Some(index) = existing {
            let route = &mut self.saved.routes[index];
            route.passages = route.passages.saturating_add(1);
            route.revision = c.revision;
            // Keep the entrance anchors stable: repeated nearby crossings must
            // not drift this record until it absorbs a different entrance.
            route.travel = Some(travel);
            route.metres = metres;
        } else if self.saved.routes.len() < MAX_ROUTES {
            self.saved.routes.push(Route {
                from,
                to,
                opening: c.opening,
                revision: c.revision,
                entry: c.entry.to_array(),
                exit: c.exit.to_array(),
                passages: 1,
                travel: Some(travel),
                metres,
            });
        }
        self.dirty = true;
        self.route_plan = None;
    }

    pub(super) fn routes_ui(&mut self, ui: &mut egui::Ui) {
        if ui
            .checkbox(&mut self.walk_routes_only, "Walked only")
            .changed()
        {
            self.route_plan = None;
        }
        let current = self.selected.or(self.active);
        if let (Some(from), Some(to)) = (self.active, current) {
            if from != to && ui.small_button("Recorded route").clicked() {
                self.route_plan = Some(self.find_route(from, to));
            }
        }
        if let Some(plan) = &self.route_plan {
            if plan.is_empty() {
                ui.label("No known route");
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("route-steps")
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for &index in plan {
                            let route = &self.saved.routes[index];
                            ui.label(format!(
                                "{} → {} · {}",
                                self.chart_name(route.from),
                                self.chart_name(route.to),
                                route.travel().label()
                            ));
                        }
                    });
            }
        }
        let routes: Vec<_> = self
            .saved
            .routes
            .iter()
            .filter(|r| {
                (!self.walk_routes_only || r.travel() == Travel::Walk)
                    && (Some(self.canonical(r.from)) == current
                        || Some(self.canonical(r.to)) == current)
            })
            .take(64)
            .map(|r| {
                (
                    self.canonical(r.from),
                    self.canonical(r.to),
                    r.travel(),
                    r.passages,
                    r.metres,
                )
            })
            .collect();
        egui::ScrollArea::vertical()
            .id_salt("known-routes")
            .max_height(180.0)
            .show(ui, |ui| {
                for (from, to, travel, count, metres) in routes {
                    let label = format!("{} → {}", self.chart_name(from), self.chart_name(to));
                    if ui
                        .small_button(label)
                        .on_hover_text(format!(
                            "{} · {} crossings{}",
                            travel.label(),
                            count,
                            metres.map_or(String::new(), |m| format!(" · ~{m:.1} m"))
                        ))
                        .clicked()
                    {
                        self.selected = Some(to);
                    }
                }
            });
    }

    fn find_route(&self, from: u32, to: u32) -> Vec<usize> {
        use std::collections::{BTreeMap, VecDeque};
        let from = self.canonical(from);
        let to = self.canonical(to);
        let mut edges: BTreeMap<u32, Vec<(u32, usize)>> = BTreeMap::new();
        for (index, route) in self.saved.routes.iter().enumerate() {
            if self.walk_routes_only && route.travel() != Travel::Walk {
                continue;
            }
            edges
                .entry(self.canonical(route.from))
                .or_default()
                .push((self.canonical(route.to), index));
        }
        let mut previous = BTreeMap::from([(from, (from, usize::MAX))]);
        let mut queue = VecDeque::from([from]);
        while let Some(node) = queue.pop_front() {
            if node == to {
                let mut path = Vec::new();
                let mut at = to;
                while at != from {
                    if path.len() >= 128 {
                        return Vec::new();
                    }
                    let (parent, edge) = previous[&at];
                    path.push(edge);
                    at = parent;
                }
                path.reverse();
                return path;
            }
            if let Some(next) = edges.get(&node) {
                for &(next, edge) in next {
                    if let std::collections::btree_map::Entry::Vacant(slot) = previous.entry(next) {
                        slot.insert((node, edge));
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
            .filter(|r| {
                (!self.walk_routes_only || r.travel() == Travel::Walk)
                    && (self.canonical(r.from) == id || self.canonical(r.to) == id)
            })
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
