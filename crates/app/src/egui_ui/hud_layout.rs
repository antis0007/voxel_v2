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
use egui::{Align2, Rect, Vec2, vec2};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
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
        slot.map(|s| (s, live.layout.gap))
    });
    let Some((slot, default_gap)) = found else {
        return Placement { align: Align2::LEFT_TOP, offset: vec2(16.0, 16.0), width: None };
    };
    let align = slot.anchor.align();
    let mut offset = vec2(
        slot.offset.0 + slot.fraction.0 * content.width(),
        slot.offset.1 + slot.fraction.1 * content.height(),
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
