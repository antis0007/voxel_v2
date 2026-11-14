//! Per-world quest selection and journal; gameplay progress remains world-owned.
use super::presentation as style;
use crate::world::{
    World, WorldSampleConfig,
    adventure::{QuestEntry, QuestId, QuestMarker},
    workshop::{self, Gear},
};
use glam::{IVec2, Vec3};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
};

const FILE: &str = "quests.ron";
const MAX_BYTES: u64 = 16 * 1024;

#[derive(Clone, Serialize, Deserialize)]
struct Target {
    config: [u32; 4],
    pos: Vec3,
    kind: u32,
}

#[derive(Clone, Serialize, Deserialize)]
struct Record {
    id: QuestId,
    stage: u8,
    complete: bool,
    target: Option<Target>,
}

#[derive(Default, Serialize, Deserialize)]
struct Book {
    selected: Option<QuestId>,
    tracking: bool,
    records: Vec<Record>,
    #[serde(default)]
    opening: bool,
    #[serde(default)]
    leads: Vec<Lead>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Lead {
    Workshop,
    Nursery,
    FreightOffice,
    ServiceLedger,
}

impl Lead {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Workshop => "Workshop",
            Self::Nursery => "Nursery",
            Self::FreightOffice => "Freight Office",
            Self::ServiceLedger => "Service Ledger",
        }
    }

    pub(super) fn directions(self) -> &'static str {
        match self {
            Self::Workshop => "Workbench",
            Self::Nursery => "Orchard Intake",
            Self::FreightOffice => "East · Stone Bridge",
            Self::ServiceLedger => "South Stairs · Lower Gallery",
        }
    }

    fn destination(self, cfg: &WorldSampleConfig) -> Option<(&'static str, Vec3)> {
        use engine_worldgen::fernwake::{self, freight};
        if self != Self::Workshop && !freight::active(cfg) {
            return None;
        }
        let pos = match self {
            Self::Workshop if cfg.generator_version >= 48 => crate::world::workshop::home(cfg)?,
            Self::Workshop => return None,
            Self::Nursery => fernwake::nursery(cfg)?,
            Self::FreightOffice => freight::point(cfg, freight::CLERK),
            Self::ServiceLedger => freight::point(cfg, freight::ARCHIVE),
        };
        Some((self.name(), pos))
    }
}

#[derive(Default)]
struct State {
    root: Option<Arc<PathBuf>>,
    config: Option<WorldSampleConfig>,
    book: Book,
    entries: Vec<QuestEntry>,
    workshop_progress: Option<workshop::Progress>,
    workshop_gear: Option<Gear>,
    lanterncap_available: bool,
    current: Option<(QuestId, QuestMarker)>,
    attempted: Option<(QuestId, u8, IVec2)>,
    dirty: bool,
    save_failed: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(State::default))
}

fn config_key(cfg: &WorldSampleConfig) -> [u32; 4] {
    [
        cfg.seed,
        cfg.generator_version,
        u32::from(cfg.world_type.code()),
        cfg.voxel_meters().to_bits(),
    ]
}

fn load(root: &std::path::Path) -> Book {
    let read = || -> anyhow::Result<Book> {
        let file = std::fs::File::open(root.join(FILE))?;
        let mut text = String::new();
        file.take(MAX_BYTES + 1).read_to_string(&mut text)?;
        anyhow::ensure!(text.len() as u64 <= MAX_BYTES, "Quest log exceeds limit");
        let book: Book = ron::from_str(&text)?;
        anyhow::ensure!(book.records.len() <= 7 && book.leads.len() <= 4, "Too many journal entries");
        anyhow::ensure!(
            book.leads.iter().enumerate().all(|(i, lead)| !book.leads[..i].contains(lead)),
            "Duplicate lead"
        );
        for (i, record) in book.records.iter().enumerate() {
            anyhow::ensure!(
                record.stage <= 4 && !book.records[..i].iter().any(|r| r.id == record.id),
                "Invalid quest record"
            );
            if let Some(target) = &record.target {
                anyhow::ensure!(
                    target.pos.is_finite()
                        && target.pos.abs().max_element() < 1.0e8
                        && matches!(target.kind, 0 | 3 | 5 | 8),
                    "Invalid quest target"
                );
            }
        }
        Ok(book)
    };
    match read() {
        Ok(book) => book,
        Err(e) => {
            if !e
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            {
                eprintln!("[quests] Cannot load {}: {e:#}", root.join(FILE).display());
            }
            Book::default()
        }
    }
}

impl State {
    fn remember(&mut self, lead: Lead) {
        if !self.book.leads.contains(&lead) {
            self.book.leads.push(lead);
            self.dirty = true;
        }
    }

    fn save(&mut self) {
        if !self.dirty {
            return;
        }
        let Some(root) = &self.root else {
            return;
        };
        let result = ron::ser::to_string_pretty(&self.book, Default::default())
            .map_err(anyhow::Error::from)
            .and_then(|text| super::map::sidecar::write_atomic(&root.join(FILE), text.as_bytes()));
        self.save_failed = result.is_err();
        if let Err(e) = result {
            eprintln!("[quests] Cannot save quest log: {e:#}");
        }
        // Retry on the next change, without repeating a failed disk write every frame.
        self.dirty = false;
    }

    fn marker(&self, origin: Vec3) -> Option<QuestMarker> {
        let (id, marker) = self.current.as_ref()?;
        if !self.book.tracking || self.book.selected != Some(*id) {
            return None;
        }
        let mut marker = marker.clone();
        let delta = marker.pos - origin;
        marker.bearing_deg = delta.x.atan2(-delta.z).to_degrees().rem_euclid(360.0);
        marker.distance_m = delta.x.hypot(delta.z);
        marker.vertical_m = delta.y;
        Some(marker)
    }
}

/// Called before UI construction. A destination is chosen once per quest stage,
/// then persisted, so movement and relaunches cannot select another nearby site.
pub(crate) fn sync(world: &World, origin: Vec3, hand_slot: usize) -> Option<QuestMarker> {
    with(|s| {
        let root = &world.regions.view.root;
        if s.root.as_ref() != Some(root) {
            *s = State {
                root: Some(root.clone()),
                book: load(root),
                ..Default::default()
            };
        }
        let cfg = world.sample_config();
        if s.config != Some(cfg) {
            s.attempted = None;
        }
        s.config = Some(cfg);
        s.workshop_progress = workshop::progress(world);
        s.workshop_gear = s.workshop_progress.as_ref().filter(|progress| !progress.vault)
            .map(|_| workshop::gear(world, hand_slot));
        s.lanterncap_available = s.workshop_progress.as_ref().is_some_and(|progress| progress.vault)
            && world.adventure.companion.is_none();
        let workshop_prepared = s.workshop_gear.is_some_and(Gear::prepared);
        s.entries = world.adventure.quest_entries(&cfg, workshop_prepared);
        if !s.book.leads.contains(&Lead::Workshop)
            && Lead::Workshop.destination(&cfg).is_some_and(|(_, home)| {
                origin.distance_squared(home) <= 8.0 * 8.0
                    || crate::world::workshop::returned(&world.adventure)
            })
        {
            s.remember(Lead::Workshop);
        }
        // Give a fresh journal a nearby anchor or an exit from the Nether.
        // After the opening, tracking belongs to the player.
        if s.book.selected.is_none() && let Some(next) = s.entries.iter().find(|e| {
            e.available && !e.complete && matches!(e.id, QuestId::Workshop | QuestId::ReturnGate)
        }) {
            s.book.selected = Some(next.id);
            s.book.tracking = true;
            s.book.opening = next.id == QuestId::Workshop;
            s.dirty = true;
        }
        for entry in &s.entries {
            if let Some(record) = s.book.records.iter_mut().find(|r| r.id == entry.id) {
                if record.stage != entry.stage || record.complete != entry.complete {
                    record.stage = entry.stage;
                    record.complete = entry.complete;
                    record.target = None;
                    s.attempted = None;
                    s.dirty = true;
                }
            } else {
                s.book.records.push(Record {
                    id: entry.id,
                    stage: entry.stage,
                    complete: entry.complete,
                    target: None,
                });
                s.dirty = true;
            }
        }
        if s.book.tracking
            && s.entries
                .iter()
                .any(|e| Some(e.id) == s.book.selected && e.complete)
        {
            s.book.tracking = false;
            s.book.opening = false;
            s.attempted = None;
            s.dirty = true;
        }
        s.current = None;
        if let Some(entry) = s
            .entries
            .iter()
            .find(|e| {
                Some(e.id) == s.book.selected && !e.complete && e.available && s.book.tracking
            })
            .cloned()
        {
            // The workshop is a fixed authored route, so derive its target
            // live. Older journals may still have the underground target.
            if entry.id == QuestId::Workshop {
                if s.book.opening {
                    if let Some(home) = crate::world::workshop::home(&cfg) {
                        if origin.distance_squared(home) <= 8.0 * 8.0 {
                            s.book.opening = false;
                            s.book.tracking = false;
                            s.dirty = true;
                        } else {
                            s.current = Some((entry.id, QuestMarker {
                                pos: home,
                                label: "Workshop",
                                kind: 0,
                                bearing_deg: 0.0,
                                distance_m: 0.0,
                                vertical_m: 0.0,
                            }));
                        }
                    }
                } else {
                    s.current = world.adventure.quest_destination(entry.id, &cfg, origin, workshop_prepared)
                        .map(|marker| (entry.id, marker));
                }
                s.save();
                return s.marker(origin);
            }
            let index = s
                .book
                .records
                .iter()
                .position(|r| r.id == entry.id)
                .unwrap();
            let key = config_key(&cfg);
            let attempt = (
                entry.id,
                entry.stage,
                IVec2::new(
                    (origin.x / 512.0).floor() as i32,
                    (origin.z / 512.0).floor() as i32,
                ),
            );
            let valid = s.book.records[index]
                .target
                .as_ref()
                .is_some_and(|t| t.config == key);
            if !valid && s.attempted != Some(attempt) {
                s.attempted = Some(attempt);
                if let Some(marker) = world.adventure.quest_destination(entry.id, &cfg, origin, workshop_prepared) {
                    s.book.records[index].target = Some(Target {
                        config: key,
                        pos: marker.pos,
                        kind: marker.kind,
                    });
                    s.dirty = true;
                }
            }
            if let Some(target) = s.book.records[index]
                .target
                .as_ref()
                .filter(|t| t.config == key)
            {
                // Keep saved positions stable. Derive any future stage-specific
                // route label from live quest data rather than saving UI copy.
                s.current = Some((
                    entry.id,
                    QuestMarker {
                        pos: target.pos,
                        label: entry.id.name(),
                        kind: target.kind,
                        bearing_deg: 0.0,
                        distance_m: 0.0,
                        vertical_m: 0.0,
                    },
                ));
            }
        }
        s.save();
        s.marker(origin)
    })
}

pub(super) fn remember(world: &World, lead: Lead) {
    with(|s| {
        if s.root.as_ref() == Some(&world.regions.view.root)
            && lead.destination(&world.sample_config()).is_some()
        {
            s.remember(lead);
            s.save();
        }
    });
}

pub(super) fn remembered(world: &World, lead: Lead) -> bool {
    with(|s| s.root.as_ref() == Some(&world.regions.view.root) && s.book.leads.contains(&lead))
}

/// The world beacon uses exactly the same selection as the compass and map.
pub(crate) fn marker(world: &World, origin: Vec3) -> Option<QuestMarker> {
    with(|s| {
        if s.root.as_ref() != Some(&world.regions.view.root)
            || s.config != Some(world.sample_config())
        {
            return None;
        }
        s.marker(origin)
    })
}

fn workshop_gear(ui: &mut egui::Ui, gear: Gear, recipe_links: bool) {
    use crate::{keybinds::{self, Action}, world::inventory::Item};
    ui.horizontal_wrapped(|ui| {
        ui.weak("Your Loadout");
        match gear {
            Gear::Held(item) => { ui.colored_label(style::JADE, format!("{} equipped", item.name())); }
            Gear::Hotbar(item, slot) => {
                let key = Action::ALL.iter().copied().find(|a| a.hotbar_slot() == Some(slot));
                let binding = key.map(keybinds::hint).unwrap_or_else(|| (slot + 1).to_string());
                ui.label(format!("Select {} · {binding}", item.name()));
            }
            Gear::Pack(item) => { ui.label(format!("Move {} to hotbar", item.name())); }
            Gear::EmptyDeck(item) => {
                ui.label(format!("{} · Slot an attack spell", item.name()));
                ui.weak(format!("Wand Atelier · {}", keybinds::hint(Action::WandAtelier)));
            }
            Gear::Missing => {
                if recipe_links {
                    if ui.link("Copper Sword Recipe").clicked() {
                        super::recipe_book::request(ui.ctx(), Item::Weapon(1), false);
                    }
                } else {
                    ui.label("Copper Sword · Inventory Recipes");
                }
            }
        }
    });
}

pub(super) fn draw(ui: &mut egui::Ui, recipe_links: bool) {
    with(|s| {
        let mut any_active = false;
        // Put the current objective first without changing the saved journal order.
        let records = s.book.records.iter();
        let selected_id = s.book.selected;
        for record in records
            .clone()
            .filter(|r| Some(r.id) == selected_id)
            .chain(records.filter(|r| Some(r.id) != selected_id))
        {
            let Some(entry) = s
                .entries
                .iter()
                .find(|e| e.id == record.id && e.available && !e.complete)
            else {
                continue;
            };
            any_active = true;
            let selected = s.book.selected == Some(record.id);
            ui.push_id(record.id.name(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(record.id.name()).strong());
                    let tracked = selected && s.book.tracking;
                    if ui
                        .small_button(if tracked { "Untrack" } else { "Track" })
                        .clicked()
                    {
                        s.book.selected = Some(record.id);
                        s.book.tracking = !tracked;
                        s.book.opening = false;
                        s.attempted = None;
                        s.dirty = true;
                    }
                });
                ui.label(egui::RichText::new(entry.objective).color(style::PAPER));
                if entry.id == QuestId::Workshop && let Some(gear) = s.workshop_gear {
                    workshop_gear(ui, gear, recipe_links);
                }
                if selected && s.book.tracking && s.current.is_none() {
                    ui.weak("Location Unknown");
                }
            });
        }
        if !any_active {
            // Some worlds have no authored route. Offer an applicable project
            // here before adding a fallback waypoint with no real destination.
            ui.weak("No active expeditions");
        }
        if let Some(progress) = &s.workshop_progress {
            ui.add_space(6.0);
            ui.weak("Workshop · World Progress");
            ui.horizontal_wrapped(|ui| {
                for (done, label) in [
                    (progress.vault, "Vault Cleared"),
                    (progress.reclaimed, "Workshop Reclaimed"),
                    (progress.delivery, "Supply Delivered"),
                ] {
                    let mark = if done { "✓" } else { "○" };
                    ui.colored_label(if done { style::JADE } else { style::MUTED }, format!("{mark} {label}"));
                }
            });
        }
        if let (Some(cfg), Some(root)) = (s.config, &s.root) {
            if s.lanterncap_available && let Some(heart) = crate::world::sites::workshop_heart(&cfg) {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Lanterncap Rescue");
                    ui.weak("Companion · Lights your path");
                    if ui.small_button("Pin").clicked() {
                        super::map::pin_lead(root, cfg, heart, "Lanterncap Reliquary");
                    }
                });
            }
            let mut leads = s.book.leads.iter()
                .filter_map(|&lead| lead.destination(&cfg).map(|(name, pos)| (lead, name, pos)))
                .peekable();
            if leads.peek().is_some() {
                ui.collapsing("Local Leads", |ui| {
                    for (lead, name, pos) in leads {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            ui.weak(lead.directions());
                            if ui.small_button("Pin").clicked() {
                                super::map::pin_lead(root, cfg, pos, name);
                            }
                        });
                    }
                });
            }
        }
        let mut other = s
            .book
            .records
            .iter()
            .filter(|r| {
                !s.entries
                    .iter()
                    .any(|e| e.id == r.id && e.available && !e.complete)
            })
            .peekable();
        if other.peek().is_some() {
            ui.collapsing("Journal", |ui| {
                for record in other {
                    let status = if record.complete {
                        "Complete"
                    } else {
                        "Unavailable"
                    };
                    ui.weak(format!("{} · {status}", record.id.name()));
                }
            });
        }
        s.save();
        if s.save_failed {
            ui.colored_label(style::AMBER, "Quest Log: Save Failed");
        }
    });
}
