//! HUD placement table (`assets/hud_layout.ron`) and a per-frame region registry.
//!
//! Persistent HUD elements ask for their slot here instead of hard-coding an
//! anchor, then report the rect they drew with [`claim`]. A slot that lists
//! regions in `below` (or `above`) is pushed clear of them, so stacked panels
//! never overlap whatever else is on screen. Rects stay valid for one extra
//! pass, which covers elements drawn later in the frame than their dependants.
//!
//! `VOXEL_HUD_LAYOUT=<path>` reloads a layout file whenever it changes (checked
//! four times a second), so placement can be tuned in a running game without a
//! rebuild. Placement rules and findings: `docs/design/HUD_LAYOUT.md`.
//!
//! Players move elements themselves with the in-game editor ([`editor`],
//! Alt+F6): each drag adds a per-slot nudge on top of the table, saved to
//! `hud_offsets.ron` in the save directory. Nudges apply before stacking, so an
//! element moved into another still gets pushed clear of it.
use egui::{Align2, Rect, Vec2, vec2};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime};

const BUILT_IN: &str = include_str!("../../../../assets/hud_layout.ron");
const RECHECK: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum Anchor { TopLeft, Top, TopRight, Left, Center, Right, BottomLeft, Bottom, BottomRight }

impl Anchor {
    fn align(self) -> Align2 {
        match self {
            Self::TopLeft => Align2::LEFT_TOP,
            Self::Top => Align2::CENTER_TOP,
            Self::TopRight => Align2::RIGHT_TOP,
            Self::Left => Align2::LEFT_CENTER,
            Self::Center => Align2::CENTER_CENTER,
            Self::Right => Align2::RIGHT_CENTER,
            Self::BottomLeft => Align2::LEFT_BOTTOM,
            Self::Bottom => Align2::CENTER_BOTTOM,
            Self::BottomRight => Align2::RIGHT_BOTTOM,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Slot {
    anchor: Anchor,
    /// Points from the anchor edge; negative x moves left, negative y moves up.
    #[serde(default)]
    offset: (f32, f32),
    /// Added to `offset` as a fraction of the window size in points.
    #[serde(default)]
    fraction: (f32, f32),
    /// Top-anchored slots start below the lowest of these regions.
    #[serde(default)]
    below: Vec<String>,
    /// Bottom-anchored slots end above the highest of these regions.
    #[serde(default)]
    above: Vec<String>,
    gap: Option<f32>,
    width: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    gap: f32,
    slots: HashMap<String, Slot>,
}

/// Where one element goes this frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placement {
    pub align: Align2,
    /// Offset for `egui::Area::anchor`, relative to the content rect.
    pub offset: Vec2,
    pub width: Option<f32>,
}

impl Placement {
    /// Top edge in screen points, for elements painted without an `Area`.
    pub fn top(&self, ctx: &egui::Context) -> f32 {
        ctx.content_rect().top() + self.offset.y
    }
}

struct Live {
    layout: Arc<Layout>,
    path: Option<PathBuf>,
    stamp: Option<SystemTime>,
    checked: Instant,
    warned: HashSet<String>,
    /// Player nudges per slot, in points, from the editor.
    nudges: HashMap<String, (f32, f32)>,
}

const NUDGE_FILE: &str = "hud_offsets.ron";
static EDITING: AtomicBool = AtomicBool::new(false);

fn nudge_path() -> PathBuf {
    crate::world::storage::save_directory().join(NUDGE_FILE)
}

/// Missing or unreadable nudges leave the table as authored.
fn load_nudges() -> HashMap<String, (f32, f32)> {
    let Ok(text) = std::fs::read_to_string(nudge_path()) else { return HashMap::new() };
    ron::from_str(&text).unwrap_or_else(|e| {
        eprintln!("[hud-layout] {NUDGE_FILE}: {e}");
        HashMap::new()
    })
}

fn save_nudges(nudges: &HashMap<String, (f32, f32)>) {
    let written = ron::to_string(nudges).map_err(|e| e.to_string()).and_then(|text| {
        std::fs::create_dir_all(crate::world::storage::save_directory())
            .and_then(|()| std::fs::write(nudge_path(), text))
            .map_err(|e| e.to_string())
    });
    if let Err(e) = written {
        eprintln!("[hud-layout] cannot save {NUDGE_FILE}: {e}");
    }
}

fn parse(text: &str) -> Result<Layout, String> {
    ron::from_str(text).map_err(|e| e.to_string())
}

static LIVE: LazyLock<Mutex<Live>> = LazyLock::new(|| {
    let layout = parse(BUILT_IN).unwrap_or_else(|e| {
        eprintln!("[hud-layout] built-in table: {e}");
        Layout { gap: 6.0, slots: HashMap::new() }
    });
    Mutex::new(Live {
        layout: Arc::new(layout),
        path: std::env::var_os("VOXEL_HUD_LAYOUT").map(PathBuf::from),
        stamp: None,
        checked: Instant::now() - RECHECK,
        warned: HashSet::new(),
        nudges: load_nudges(),
    })
});

fn with_live<R>(f: impl FnOnce(&mut Live) -> R) -> R {
    let mut live = LIVE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(path) = live.path.clone()
        && live.checked.elapsed() >= RECHECK
    {
        live.checked = Instant::now();
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if stamp.is_some() && stamp != live.stamp {
            live.stamp = stamp;
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
                Ok(layout) => {
                    live.layout = Arc::new(layout);
                    live.warned.clear();
                    eprintln!("[hud-layout] loaded {}", path.display());
                }
                Err(e) => eprintln!("[hud-layout] {}: {e}", path.display()),
            }
        }
    }
    f(&mut live)
}

#[derive(Clone, Default)]
struct Regions(HashMap<&'static str, (Rect, u64)>);

fn regions_id() -> egui::Id {
    egui::Id::new("hud_layout_regions")
}

/// Records the rect an element drew this pass so other slots can stack on it.
pub(crate) fn claim(ctx: &egui::Context, name: &'static str, rect: Rect) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Regions>(regions_id()).0.insert(name, (rect, pass));
    });
}

/// The rect `name` drew this pass or the previous one.
pub(crate) fn region(ctx: &egui::Context, name: &str) -> Option<Rect> {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<Regions>(regions_id()))
        .and_then(|r| r.0.get(name).copied())
        .filter(|(_, at)| pass.saturating_sub(*at) <= 1)
        .map(|(rect, _)| rect)
}

/// Resolves slot `name` against the window and this frame's claimed regions.
pub(crate) fn place(ctx: &egui::Context, name: &str) -> Placement {
    let content = ctx.content_rect();
    let found = with_live(|live| {
        let slot = live.layout.slots.get(name).cloned();
        if slot.is_none() && live.warned.insert(name.to_owned()) {
            eprintln!("[hud-layout] no slot named {name}");
        }
        let nudge = live.nudges.get(name).copied().unwrap_or_default();
        slot.map(|s| (s, live.layout.gap, nudge))
    });
    let Some((slot, default_gap, nudge)) = found else {
        return Placement { align: Align2::LEFT_TOP, offset: vec2(16.0, 16.0), width: None };
    };
    let align = slot.anchor.align();
    let mut offset = vec2(
        slot.offset.0 + slot.fraction.0 * content.width() + nudge.0,
        slot.offset.1 + slot.fraction.1 * content.height() + nudge.1,
    );
    let gap = slot.gap.unwrap_or(default_gap);
    for rect in slot.below.iter().filter_map(|n| region(ctx, n)) {
        offset.y = offset.y.max(rect.bottom() - content.top() + gap);
    }
    for rect in slot.above.iter().filter_map(|n| region(ctx, n)) {
        offset.y = offset.y.min(rect.top() - content.bottom() - gap);
    }
    let width = slot.width.map(|w| w.min((content.width() - 2.0 * offset.x.abs()).max(120.0)));
    Placement { align, offset, width }
}

/// Whether the HUD editor is open.
pub(crate) fn editing() -> bool {
    EDITING.load(Ordering::Relaxed)
}

/// Opens or closes the HUD editor; the caller frees or captures the mouse.
pub(crate) fn set_editing(on: bool) {
    EDITING.store(on, Ordering::Relaxed);
}

/// The HUD editor overlay, drawn after every element has claimed its rect:
/// each placed element on screen gets an outline and its slot name; dragging
/// it moves it, right-click returns it to the table position. Elements that
/// are not on screen (a boss bar with no boss) cannot be picked up.
pub(crate) fn editor(ctx: &egui::Context) {
    if !editing() {
        return;
    }
    let names: Vec<String> = with_live(|live| live.layout.slots.keys().cloned().collect());
    let mut shown: Vec<(String, Rect)> = names.into_iter()
        .filter_map(|n| region(ctx, &n).map(|r| (n, r)))
        .collect();
    shown.sort_by(|a, b| a.0.cmp(&b.0));
    let held_id = egui::Id::new("hud_editor_held");
    let mut held = ctx.data(|d| d.get_temp::<Option<String>>(held_id)).flatten();
    let (pointer, pressed, down, released, secondary, delta) = ctx.input(|i| (
        i.pointer.interact_pos(),
        i.pointer.primary_pressed(),
        i.pointer.primary_down(),
        i.pointer.primary_released(),
        i.pointer.secondary_clicked(),
        i.pointer.delta(),
    ));
    let under = |p: egui::Pos2| shown.iter().rev().find(|(_, r)| r.contains(p)).map(|(n, _)| n.clone());
    if pressed && let Some(p) = pointer {
        held = under(p);
    }
    if down && let Some(name) = &held && delta != Vec2::ZERO {
        with_live(|live| {
            let n = live.nudges.entry(name.clone()).or_default();
            n.0 += delta.x;
            n.1 += delta.y;
        });
    }
    if released && held.take().is_some() {
        with_live(|live| save_nudges(&live.nudges));
    }
    if secondary && let Some(name) = pointer.and_then(under) {
        with_live(|live| {
            live.nudges.remove(&name);
            save_nudges(&live.nudges);
        });
    }
    ctx.data_mut(|d| d.insert_temp(held_id, held.clone()));

    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("hud_editor")));
    let accent = super::presentation::AMBER;
    let hover = pointer.and_then(under);
    for (name, rect) in &shown {
        let active = held.as_ref() == Some(name) || (held.is_none() && hover.as_ref() == Some(name));
        let colour = if active { accent } else { accent.gamma_multiply(0.55) };
        p.rect_filled(*rect, 0, colour.gamma_multiply(0.12));
        p.rect_stroke(*rect, 0, egui::Stroke::new(if active { 2.0 } else { 1.0 }, colour), egui::StrokeKind::Outside);
        let label = name.replace('_', " ");
        let at = rect.left_top() + vec2(3.0, 2.0);
        p.text(at + vec2(1.0, 1.0), Align2::LEFT_TOP, &label, egui::FontId::monospace(10.0), egui::Color32::BLACK);
        p.text(at, Align2::LEFT_TOP, &label, egui::FontId::monospace(10.0), colour);
    }
    let banner = "HUD Layout · [Drag] Move · [RMB] Reset · [Alt+F6] Done";
    let at = egui::pos2(ctx.content_rect().center().x, ctx.content_rect().bottom() - 140.0);
    let galley = p.layout_no_wrap(banner.to_owned(), egui::FontId::proportional(13.0), egui::Color32::WHITE);
    let plate = Rect::from_center_size(at, galley.size() + vec2(20.0, 10.0));
    p.rect_filled(plate, 0, egui::Color32::from_black_alpha(200));
    p.galley(plate.center() - galley.size() * 0.5, galley, egui::Color32::WHITE);
    ctx.request_repaint();
}
