//! Personal, observed 3D patches. No world, chunk, generator or networking reads.
//! The renderer consumes these owned snapshots, never the live terrain.
use super::sidecar;
use egui::{Color32, Mesh, Pos2, Rect, Shape, emath::Rot2};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::Instant,
};

const MAX_PATCHES: usize = 262_144;
const MAX_LAYERS: usize = 2048;
const MAX_BYTES: u64 = 48 * 1024 * 1024;
mod navigation;
mod pages;
mod paging;
mod recognition;
mod reports;
mod routes;
pub(super) use navigation::Motion;
use pages::Pages;
use routes::Route;

#[derive(Clone, Serialize, Deserialize)]
pub struct Layer {
    #[serde(default)]
    scope: u64,
    pub origin: [f32; 3],
    pub up: [f32; 3],
    pub right: [f32; 3],
    pub forward: [f32; 3],
    #[serde(default)]
    report: Option<reports::Provenance>,
}

impl Layer {
    pub fn project(&self, p: Vec3) -> egui::Vec2 {
        let d = p - Vec3::from_array(self.origin);
        egui::vec2(
            d.dot(Vec3::from_array(self.right)),
            -d.dot(Vec3::from_array(self.forward)),
        )
    }
    pub fn accepts(&self, p: Vec3, up: Vec3) -> bool {
        up.dot(Vec3::from_array(self.up)) > 0.8
            && (p - Vec3::from_array(self.origin))
                .dot(Vec3::from_array(self.up))
                .abs()
                < 1.5
    }
    fn valid(&self) -> bool {
        let up = Vec3::from_array(self.up);
        let right = Vec3::from_array(self.right);
        let forward = Vec3::from_array(self.forward);
        self.report.as_ref().is_none_or(reports::Provenance::valid)
            && Vec3::from_array(self.origin).is_finite()
            && Vec3::from_array(self.origin).abs().max_element() < 1_000_000.0
            && [up, right, forward]
                .iter()
                .all(|v| v.is_finite() && (v.length_squared() - 1.0).abs() < 0.001)
            && up.dot(right).abs() < 0.001
            && up.dot(forward).abs() < 0.001
            && right.dot(forward).abs() < 0.001
    }
}

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct Patch {
    /// None is a travelled position without a visual survey.
    pub colour: Option<[u8; 3]>,
    pub travelled: bool,
    #[serde(default)]
    pub normal: [i8; 3],
    /// Each face retains its own appearance; contact does not imply visibility.
    #[serde(default)]
    pub faces: [Option<[u8; 3]>; 6],
    #[serde(default)]
    pub order: [u64; 6],
}

impl Patch {
    fn face(normal: [i8; 3]) -> Option<usize> {
        let axis = normal.iter().position(|v| *v != 0)?;
        (normal.iter().filter(|v| **v != 0).count() == 1 && normal[axis].unsigned_abs() == 1)
            .then_some(axis * 2 + usize::from(normal[axis] > 0))
    }
    fn migrate(&mut self) {
        if let (Some(c), Some(face)) = (self.colour.take(), Self::face(self.normal)) {
            self.faces[face].get_or_insert(c);
        }
        self.normal = [0; 3];
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    sequence: u64,
    #[serde(default)]
    scope: u64,
    layers: Vec<Layer>,
    patches: Pages,
    #[serde(default)]
    cold: BTreeMap<pages::Key, paging::Cold>,
    #[serde(default)]
    routes: Vec<Route>,
    #[serde(default)]
    names: BTreeMap<u32, String>,
    #[serde(default)]
    landmarks: BTreeMap<[i32; 3], recognition::Landmark>,
    #[serde(default)]
    aliases: BTreeMap<u32, u32>,
    #[serde(default)]
    pins: BTreeSet<([i32; 3], u32)>,
}

type Footsteps = BTreeMap<(u64, [i32; 3]), u32>;
type Locations = BTreeMap<(u64, [i32; 3]), BTreeSet<u32>>;
struct Loaded {
    saved: Saved,
    footsteps: Footsteps,
    locations: Locations,
    cold_layers: Locations,
}
impl Loaded {
    fn new(saved: Saved) -> Self {
        let mut footsteps = Footsteps::new();
        let mut locations = Locations::new();
        for ((layer, cell), patch) in saved.patches.iter() {
            let chart = &saved.layers[layer as usize];
            if chart.report.is_some() {
                continue;
            }
            let key = (chart.scope, cell);
            locations.entry(key).or_default().insert(layer);
            if patch.travelled {
                footsteps.insert(key, layer);
            }
        }
        let mut cold_layers = Locations::new();
        for &(layer, coord) in saved.cold.keys() {
            cold_layers
                .entry((saved.layers[layer as usize].scope, coord))
                .or_default()
                .insert(layer);
        }
        Self {
            saved,
            footsteps,
            locations,
            cold_layers,
        }
    }
}

pub struct Knowledge {
    saved: Saved,
    pub active: Option<u32>,
    pub selected: Option<u32>,
    pub legacy: bool,
    pub enabled: bool,
    pub creative: bool,
    pub feet: Vec3,
    pub yaw: f32,
    recent: BTreeMap<(u32, [i32; 3]), Instant>,
    dirty: bool,
    load_failed: bool,
    save_failed: bool,
    path: Option<PathBuf>,
    writer: Option<std::thread::JoinHandle<std::io::Result<()>>>,
    saved_at: Instant,
    last_step: Option<(u32, [i32; 3])>,
    footsteps: Footsteps,
    locations: Locations,
    loading: Option<std::thread::JoinHandle<Result<Loaded, String>>>,
    last_position: Option<Vec3>,
    pending_arrival: bool,
    last_crossing: u64,
    observer_epoch: u64,
    scale: f32,
    route_view: bool,
    route_plan: Option<Vec<usize>>,
    walk_routes_only: bool,
    exchange: reports::Exchange,
    pager: paging::Pager,
    navigation: navigation::Navigation,
}

impl Default for Knowledge {
    fn default() -> Self {
        Self {
            saved: Saved::default(),
            active: None,
            selected: None,
            legacy: false,
            enabled: false,
            creative: false,
            feet: Vec3::ZERO,
            yaw: 0.0,
            recent: BTreeMap::new(),
            dirty: false,
            load_failed: false,
            save_failed: false,
            path: None,
            writer: None,
            saved_at: Instant::now(),
            last_step: None,
            footsteps: BTreeMap::new(),
            locations: BTreeMap::new(),
            loading: None,
            last_position: None,
            pending_arrival: false,
            last_crossing: 0,
            observer_epoch: 0,
            scale: 1.0,
            route_view: false,
            route_plan: None,
            walk_routes_only: false,
            exchange: Default::default(),
            pager: Default::default(),
            navigation: Default::default(),
        }
    }
}

impl Knowledge {
    pub fn unknown_arrival(&mut self) {
        self.pending_arrival = true;
    }
    pub fn open(&mut self, root: &Path, creative: bool) {
        self.flush();
        let receipt = (self.observer_epoch, self.last_crossing);
        *self = Self::default();
        (self.observer_epoch, self.last_crossing) = receipt;
        self.creative = creative;
        // Local owner only. A copied world never turns a teammate's survey into ours.
        let owner = std::env::args()
            .find_map(|a| a.strip_prefix("--multiplayer-profile=").map(str::to_owned))
            .unwrap_or_else(|| {
                format!(
                    "{}:{}",
                    std::env::var("COMPUTERNAME").unwrap_or_default(),
                    std::env::var("USERNAME").unwrap_or_default()
                )
            });
        let hash = owner.trim().bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        let policy = if creative { "creative" } else { "survival" };
        let path = root.join(format!("observations-{hash:016x}-{policy}.ron"));
        self.pager.open(&path);
        let read_path = path.clone();
        self.loading = std::thread::Builder::new()
            .name("map-observation-load".into())
            .spawn(move || {
                use std::io::Read;
                let file = match std::fs::File::open(&read_path) {
                    Ok(f) => f,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Ok(Loaded::new(Saved::default()));
                    }
                    Err(e) => return Err(e.to_string()),
                };
                {
                    let mut text = String::new();
                    file.take(MAX_BYTES + 1)
                        .read_to_string(&mut text)
                        .map_err(|e| e.to_string())?;
                    if text.len() as u64 > MAX_BYTES {
                        return Err("Observation archive exceeds limit".into());
                    }
                    let read = text
                        .strip_prefix("VXOBS004\n")
                        .or_else(|| text.strip_prefix("VXOBS003\n"))
                        .or_else(|| text.strip_prefix("VXOBS002\n"))
                        .or_else(|| text.strip_prefix("VXOBS001\n"))
                        .and_then(|body| ron::from_str::<Saved>(body).ok());
                    match read {
                        Some(mut saved)
                            if saved.layers.len() <= MAX_LAYERS
                                && saved.patches.len() <= MAX_PATCHES
                                && saved.cold.len() <= paging::MAX_COLD
                                && saved.cold.iter().all(|(&(layer, cell), cold)| {
                                    (layer as usize) < saved.layers.len()
                                        && cell.iter().all(|v| (-500_000..=500_000).contains(v))
                                        && cold.count > 0
                                        && cold.count <= 512
                                        && !saved.patches.pages.contains_key(&(layer, cell))
                                })
                                && saved.layers.iter().all(Layer::valid)
                                && saved.patches.keys().all(|(layer, cell)| {
                                    (layer as usize) < saved.layers.len()
                                        && cell.iter().all(|v| (-4_000_000..=4_000_000).contains(v))
                                })
                                && saved.routes.len() <= routes::MAX_ROUTES
                                && saved.routes.iter().all(|r| r.valid(saved.layers.len()))
                                && recognition::valid(&saved)
                                && saved.pins.len() <= 4096
                                && saved.pins.iter().all(|(cell, layer)| {
                                    (*layer as usize) < saved.layers.len()
                                        && saved.layers[*layer as usize].report.is_none()
                                        && cell.iter().all(|v| (-4_000_000..=4_000_000).contains(v))
                                })
                                && saved.names.len() <= MAX_LAYERS
                                && saved.names.iter().all(|(id, name)| {
                                    (*id as usize) < saved.layers.len() && name.len() <= 96
                                }) =>
                        {
                            saved.patches.migrate();
                            return Ok(Loaded::new(saved));
                        }
                        _ => return Err("Invalid observation archive".into()),
                    }
                }
            })
            .ok();
        // Never overwrite an unread archive if its reader could not start.
        self.path = self.loading.is_some().then_some(path);
        self.load_failed = self.loading.is_none();
    }

    fn finish_load(&mut self, wait: bool) {
        if !wait && self.loading.as_ref().is_some_and(|job| !job.is_finished()) {
            return;
        }
        if let Some(job) = self.loading.take() {
            match job.join() {
                Ok(Ok(loaded)) => {
                    self.footsteps = loaded.footsteps;
                    self.locations = loaded.locations;
                    self.pager.cold_layers = loaded.cold_layers;
                    self.saved = loaded.saved;
                }
                _ => {
                    eprintln!("[map] Observation archive unreadable; preserving file");
                    self.path = None;
                    self.load_failed = true;
                }
            }
        }
    }

    pub fn locate(&mut self, feet: Vec3, up: Vec3, forward: Vec3, scale: f32, motion: Motion) {
        if !feet.is_finite()
            || !up.is_finite()
            || !forward.is_finite()
            || !scale.is_finite()
            || scale <= 0.0
            || (up.length_squared() - 1.0).abs() > 0.01
        {
            self.active = None;
            self.navigation = Default::default();
            return;
        }
        self.scale = scale;
        self.feet = feet;
        self.finish_load(false);
        if !self.enabled || self.loading.is_some() {
            self.active = None;
            self.navigation = Default::default();
            return;
        }
        // A discontinuous arrival does not establish its identity. Keep old
        // charts available manually, but start an independent local chart.
        if self.pending_arrival
            || self
                .last_position
                .is_some_and(|p| p.distance_squared(feet) > 64.0)
        {
            self.pending_arrival = false;
            self.saved.scope = self
                .saved
                .layers
                .iter()
                .map(|l| l.scope)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            self.active = None;
            self.selected = None;
            self.last_step = None;
            self.recent.clear();
            self.navigation = Default::default();
            self.dirty = true;
        }
        self.last_position = Some(feet);
        let cell = (feet / scale).floor().as_ivec3().to_array();
        // Return to an actually travelled patch before considering a nearby plane.
        let visited = self
            .footsteps
            .get(&(self.saved.scope, cell))
            .copied()
            .filter(|&id| {
                self.saved.layers[id as usize].scope == self.saved.scope
                    && self.saved.layers[id as usize].accepts(feet, up)
            });
        let existing = visited
            .map(|id| self.canonical(id))
            .or_else(|| {
                self.active
                    .filter(|&id| self.saved.layers[id as usize].accepts(feet, up))
            })
            .or_else(|| {
                self.saved
                    .layers
                    .iter()
                    .enumerate()
                    .position(|(id, l)| {
                        !self.saved.aliases.contains_key(&(id as u32))
                            && l.scope == self.saved.scope
                            && l.accepts(feet, up)
                    })
                    .map(|i| i as u32)
            });
        self.active = if existing.is_some() {
            existing
        } else if motion.grounded && self.saved.layers.len() < MAX_LAYERS {
            let reference = if up.dot(Vec3::Y).abs() < 0.9 {
                Vec3::Y
            } else {
                Vec3::NEG_Z
            };
            let f = (reference - up * reference.dot(up)).normalize();
            let right = f.cross(up).normalize();
            let id = self.saved.layers.len() as u32;
            self.saved.layers.push(Layer {
                scope: self.saved.scope,
                origin: feet.to_array(),
                up: up.to_array(),
                right: right.to_array(),
                forward: f.to_array(),
                report: None,
            });
            self.dirty = true;
            Some(id)
        } else {
            None
        };
        if let Some(leg) = self.navigation.observe(navigation::Sample {
            at: Instant::now(),
            point: feet,
            up,
            scope: self.saved.scope,
            layer: self.active,
            motion,
        }) {
            self.record_route(
                routes::Crossing {
                    from: leg.from,
                    opening: 0,
                    revision: 0,
                    entry: leg.entry,
                    exit: leg.exit,
                },
                leg.to,
                leg.travel,
                Some(leg.metres),
            );
        }
        if let Some(layer) = self.active {
            let l = &self.saved.layers[layer as usize];
            self.yaw = forward
                .dot(Vec3::from_array(l.right))
                .atan2(forward.dot(Vec3::from_array(l.forward)));
            if self.request_cell(layer, cell) {
                return;
            }
            if motion.grounded
                && self.last_step != Some((layer, cell))
                && (self.saved.patches.len() < MAX_PATCHES
                    || self.saved.patches.contains_key(&(layer, cell)))
            {
                let mut patch = self
                    .saved
                    .patches
                    .get(&(layer, cell))
                    .copied()
                    .unwrap_or_default();
                patch.travelled = true;
                self.saved.patches.insert((layer, cell), patch);
                self.last_step = Some((layer, cell));
                self.footsteps.insert((self.saved.scope, cell), layer);
                self.locations
                    .entry((self.saved.scope, cell))
                    .or_default()
                    .insert(layer);
                self.dirty = true;
            }
        }
    }

    /// Exact visible voxel appearance from the adapter, not a whole chunk/room.
    pub fn observe(&mut self, cell: IVec3, colour: [u8; 3], normal: IVec3) {
        let Some(active) = self.active else { return };
        let normal = normal.to_array().map(|v| v.clamp(-1, 1) as i8);
        let Some(face) = Patch::face(normal) else {
            return;
        };
        let up = Vec3::from_array(self.saved.layers[active as usize].up);
        let point =
            (cell.as_vec3() + Vec3::splat(0.5) + Vec3::from_array(normal.map(f32::from)) * 0.5)
                * self.scale;
        // Floors seen above/below belong to their own observed chart. Do not
        // infer a connector or a visit merely because that floor was visible.
        let floor = Vec3::from_array(normal.map(f32::from)).dot(up) > 0.6;
        let layer = if floor {
            if let Some(i) = self.saved.layers.iter().enumerate().position(|(id, l)| {
                !self.saved.aliases.contains_key(&(id as u32))
                    && l.scope == self.saved.scope
                    && l.accepts(point, up)
            }) {
                i as u32
            } else if self.saved.layers.len() < MAX_LAYERS {
                let mut l = self.saved.layers[active as usize].clone();
                l.origin = point.to_array();
                let id = self.saved.layers.len() as u32;
                self.saved.layers.push(l);
                self.dirty = true;
                id
            } else {
                return;
            }
        } else {
            // Tall walls cannot silently populate a different floor. Preserve
            // only the portion supported by the current walking plane.
            let height =
                (point - Vec3::from_array(self.saved.layers[active as usize].origin)).dot(up);
            if !(-1.5..=3.5).contains(&height) {
                return;
            }
            active
        };
        let key = (layer, cell.to_array());
        if self.request_cell(layer, cell.to_array()) {
            return;
        }
        if self.saved.patches.len() >= MAX_PATCHES && !self.saved.patches.contains_key(&key) {
            return;
        }
        let mut patch = self.saved.patches.get(&key).copied().unwrap_or_default();
        // Re-observing the same colour is newer evidence too. Otherwise a chart
        // merge could replace a just-seen wall with an older differing memory.
        patch.faces[face] = Some(colour);
        self.saved.sequence = self.saved.sequence.saturating_add(1);
        patch.order[face] = self.saved.sequence;
        self.saved.patches.insert(key, patch);
        self.dirty = true;
        self.recent.insert(key, Instant::now());
        self.locations
            .entry((self.saved.scope, cell.to_array()))
            .or_default()
            .insert(layer);
    }

    pub fn observed_empty(&mut self, cell: IVec3) {
        let Some(layers) = self.locations.get(&(self.saved.scope, cell.to_array())) else {
            return;
        };
        for &layer in layers {
            if self.request_cell(layer, cell.to_array()) {
                continue;
            }
            if let Some(patch) = self.saved.patches.get_mut(&(layer, cell.to_array())) {
                patch.faces = [None; 6];
                self.saved.sequence = self.saved.sequence.saturating_add(1);
                patch.order = [self.saved.sequence; 6];
                self.dirty = true;
            }
        }
    }

    pub fn remembers(&self, cell: IVec3) -> bool {
        if self
            .locations
            .contains_key(&(self.saved.scope, cell.to_array()))
        {
            return true;
        }
        self.pager
            .request_known_cell(self.saved.scope, cell.to_array())
    }

    pub fn layer(&self, minimap: bool) -> Option<&Layer> {
        let id = if minimap {
            self.active
        } else {
            self.selected.or(self.active)
        }?;
        self.saved.layers.get(id as usize)
    }
    pub fn project(&self, p: Vec3, minimap: bool) -> egui::Vec2 {
        self.layer(minimap)
            .map_or(egui::Vec2::ZERO, |l| l.project(p))
    }
    pub fn marker_on_layer(&self, cell: IVec3, scale: f32, minimap: bool) -> bool {
        let id = if minimap {
            self.active
        } else {
            self.selected.or(self.active)
        };
        self.layer(minimap).is_some_and(|l| {
            l.report.is_none()
                && l.accepts(cell.as_vec3() * scale, Vec3::from_array(l.up))
                && (l.scope == 0
                    || id.is_some_and(|id| {
                        self.saved
                            .pins
                            .range((cell.to_array(), 0)..=(cell.to_array(), u32::MAX))
                            .any(|(_, chart)| self.canonical(*chart) == self.canonical(id))
                    }))
        })
    }
    /// A stored global coordinate is not recognition evidence after a fold.
    /// Remember only annotations explicitly made on this personal chart.
    pub fn mark(&mut self, cell: IVec3, here: bool) {
        let Some(id) = (if here {
            self.active
        } else {
            self.selected.or(self.active)
        }) else {
            return;
        };
        if self.saved.pins.len() < 4096 && self.saved.layers[id as usize].report.is_none() {
            self.dirty |= self
                .saved
                .pins
                .insert((cell.to_array(), self.canonical(id)));
        }
    }
    pub fn can_measure_from_player(&self) -> bool {
        self.layer(false)
            .zip(self.layer(true))
            .is_some_and(|(shown, here)| shown.report.is_none() && shown.scope == here.scope)
    }
    pub fn unproject(&self, point: egui::Vec2, scale: f32) -> Option<IVec3> {
        let l = self.layer(false)?;
        if l.report.is_some() {
            return None;
        }
        Some(
            ((Vec3::from_array(l.origin) + Vec3::from_array(l.right) * point.x
                - Vec3::from_array(l.forward) * point.y)
                / scale)
                .floor()
                .as_ivec3(),
        )
    }
    pub fn picker(&mut self, ui: &mut egui::Ui) {
        let before = self.selected;
        ui.checkbox(&mut self.legacy, "Legacy survey");
        if self.legacy {
            return;
        }
        if self.saved.scope != 0 {
            ui.label("Unlocalized");
        }
        egui::ComboBox::from_id_salt("observed-level")
            .selected_text(
                self.selected
                    .map_or("Current level".into(), |id| self.chart_name(id)),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.selected, None, "Current level");
                for id in 0..self.saved.layers.len() as u32 {
                    if self.saved.aliases.contains_key(&id) {
                        continue;
                    }
                    let name = self.chart_name(id);
                    ui.selectable_value(&mut self.selected, Some(id), name);
                }
            });
        if let Some(id) = self.selected.or(self.active) {
            let name = self.saved.names.entry(id).or_default();
            if ui
                .add(
                    egui::TextEdit::singleline(name)
                        .hint_text("Level name")
                        .char_limit(24),
                )
                .changed()
            {
                self.dirty = true;
            }
        }
        ui.checkbox(&mut self.route_view, "Routes");
        if self.route_view {
            self.routes_ui(ui);
        }
        self.exchange_ui(ui);
        if self.saved.patches.len() >= MAX_PATCHES {
            ui.label("Survey full");
        }
        if self.load_failed {
            ui.label("Archive unreadable · Session only");
        } else if self.save_failed {
            ui.label("Survey save failed");
            if ui.small_button("Retry save").clicked() && self.writer.is_none() {
                self.save();
            }
        }
        if self.pager.error {
            ui.label("Archive unavailable");
            if ui.small_button("Retry").clicked() {
                self.pager.retry();
            }
        }
        if self.selected != before {
            self.route_plan = None;
        }
    }

    pub fn draw(
        &self,
        p: &egui::Painter,
        centre: Pos2,
        origin: egui::Vec2,
        zoom: f32,
        rot: Rot2,
        voxel_m: f32,
        minimap: bool,
    ) {
        let Some(id) = (if minimap {
            self.active
        } else {
            self.selected.or(self.active)
        }) else {
            return;
        };
        let layer = &self.saved.layers[id as usize];
        let mut mesh = Mesh::default();
        let now = Instant::now();
        let mut drawn = 0;
        let mut inspected = 0;
        'pages: for (&(_, coord), page) in self
            .saved
            .patches
            .pages
            .range((id, [i32::MIN; 3])..=(id, [i32::MAX; 3]))
        {
            let page_center = (Vec3::from_array(coord.map(|v| v as f32)) + Vec3::splat(0.5))
                * pages::SIDE as f32
                * voxel_m;
            let page_at = centre + rot * (layer.project(page_center) - origin) * zoom;
            let radius = pages::SIDE as f32 * voxel_m * zoom * 0.87 + 2.0;
            if !p.clip_rect().expand(radius).contains(page_at) {
                continue;
            }
            self.pager.touch((id, coord));
            for (&cell, patch) in page.iter() {
                inspected += 1;
                if inspected > 32_768 {
                    break 'pages;
                }
                let point = (Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5)) * voxel_m;
                let delta = layer.project(point) - origin;
                let at = centre + rot * delta * zoom;
                if !p.clip_rect().expand(voxel_m * zoom).contains(at) {
                    continue;
                }
                // Prefer the observed upward face, then a wall face. Never flatten
                // an observed ceiling into the floor beneath it.
                let up = Vec3::from_array(layer.up);
                let colour = patch
                    .faces
                    .iter()
                    .enumerate()
                    .filter_map(|(face, c)| {
                        let mut normal = Vec3::ZERO;
                        normal[face / 2] = if face % 2 == 0 { -1.0 } else { 1.0 };
                        let facing = normal.dot(up);
                        c.filter(|_| facing >= -0.3).map(|c| (facing, c))
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, c)| c);
                if let Some(c) = colour {
                    // This is explicitly recent observation, never claimed current visibility.
                    let recent = self
                        .recent
                        .get(&(id, cell))
                        .is_some_and(|t| now.duration_since(*t).as_secs_f32() < 0.3);
                    let factor = if recent { 0.9 } else { 0.55 };
                    let colour = Color32::from_rgb(
                        (c[0] as f32 * factor) as u8,
                        (c[1] as f32 * factor) as u8,
                        (c[2] as f32 * factor) as u8,
                    );
                    mesh.add_colored_rect(
                        Rect::from_center_size(at, egui::Vec2::splat((voxel_m * zoom).max(1.0))),
                        colour,
                    );
                }
                if patch.travelled {
                    mesh.add_colored_rect(
                        Rect::from_center_size(
                            at,
                            egui::Vec2::splat((voxel_m * zoom * 0.22).clamp(1.0, 2.0)),
                        ),
                        Color32::from_rgb(180, 155, 107),
                    );
                }
                drawn += 1;
                if drawn == 16_384 {
                    break 'pages;
                }
            }
        }
        p.add(Shape::mesh(mesh));
        let mut requested = 0;
        for (&key, _) in self
            .saved
            .cold
            .range((id, [i32::MIN; 3])..=(id, [i32::MAX; 3]))
        {
            let point = (Vec3::from_array(key.1.map(|v| v as f32)) + Vec3::splat(0.5))
                * pages::SIDE as f32
                * voxel_m;
            let at = centre + rot * (layer.project(point) - origin) * zoom;
            let radius = pages::SIDE as f32 * voxel_m * zoom * 0.87 + 2.0;
            if p.clip_rect().expand(radius).contains(at) {
                self.pager.touch(key);
                self.pager.request(key);
                requested += 1;
                if requested == 32 {
                    break;
                }
            }
        }
        self.draw_routes(p, id, centre, origin, zoom, rot);
    }

    pub fn maintenance(&mut self) {
        self.finish_load(false);
        self.paging(false);
        self.merge_recognized();
        self.recent.retain(|_, t| t.elapsed().as_secs_f32() < 0.5);
        if self.writer.as_ref().is_some_and(|w| w.is_finished()) {
            self.join();
        }
        if self.dirty && self.writer.is_none() && self.saved_at.elapsed().as_secs() >= 30 {
            self.save();
        }
    }
    fn join(&mut self) {
        if let Some(job) = self.writer.take() {
            self.save_failed = !matches!(job.join(), Ok(Ok(())));
            if self.save_failed {
                self.dirty = true;
                eprintln!("[map] Observation save failed; retrying");
            }
        }
    }
    fn save(&mut self) {
        let Some(path) = self
            .path
            .clone()
            .filter(|p| p.parent().is_some_and(Path::is_dir))
        else {
            return;
        };
        let saved = self.saved.clone();
        self.saved_at = Instant::now();
        self.writer = std::thread::Builder::new()
            .name("map-observations".into())
            .spawn(move || {
                let body = ron::to_string(&saved).map_err(std::io::Error::other)?;
                if body.len() as u64 > MAX_BYTES - 9 {
                    return Err(std::io::Error::other("Observation archive full"));
                }
                sidecar::write_atomic(&path, format!("VXOBS004\n{body}").as_bytes())
                    .map_err(std::io::Error::other)
            })
            .ok();
        if self.writer.is_some() {
            self.dirty = false;
        } else {
            self.save_failed = true;
        }
    }
    pub fn flush(&mut self) {
        self.finish_load(true);
        self.paging(true);
        self.join();
        if self.dirty {
            self.save();
            self.join();
        }
    }
}
