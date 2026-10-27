//! Rebindable input actions, their bindings and the user override file.
//!
//! A binding drives at most one action: assigning it elsewhere unbinds the old
//! action, so dispatch never has to choose between two.

mod names;
#[cfg(test)]
mod tests;

pub use names::Binding;

use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::{LazyLock, Mutex, PoisonError, RwLock},
    time::{Duration, Instant},
};

const DEFAULTS: &str = include_str!("../../../assets/keybinds.ron");
const MAX_FILE_BYTES: u64 = 64 * 1024;
pub const SLOTS: usize = 2;
/// A rebind the controls screen stops drawing lapses, so a closed menu cannot
/// leave the next in-game key waiting to be captured.
const CAPTURE_LAPSE: Duration = Duration::from_millis(500);

macro_rules! actions {
    ($($name:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Action { $($name),* }
        impl Action {
            pub const ALL: &[Action] = &[$(Action::$name),*];
        }
    };
}

// `Input` keeps one bit per slot of each input-held action, so those come first.
actions!(
    Forward, Back, Left, Right, Jump, Sneak, Sprint, Dash, Break, Place, Pick, Cast,
    Inspect, Zoom,
    Interact, NextSpell, Lamp, ToolHand, ToolBrush, ToolBuilders, ToolDestructor,
    ToolArea, ToolExplosive, RangeDown, RangeUp, Inventory, CreativePanel, WandAtelier,
    QuickSelect, ToggleHud, Screenshot, Debug, Fullscreen, Hotbar1, Hotbar2, Hotbar3,
    Hotbar4, Hotbar5, Hotbar6, Hotbar7, Hotbar8, Hotbar9,
);

/// Actions `Input` tracks per frame; only these accept mouse buttons.
pub const INPUT_ACTIONS: usize = Action::Zoom as usize + 1;
const _: () = assert!(INPUT_ACTIONS * SLOTS <= 32, "Input::held is a u32");

impl Action {
    pub fn name(self) -> String {
        format!("{self:?}")
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.name() == name)
    }

    pub fn mouse_bindable(self) -> bool {
        (self as usize) < INPUT_ACTIONS
    }

    pub fn hotbar_slot(self) -> Option<usize> {
        (self as usize).checked_sub(Self::Hotbar1 as usize).filter(|slot| *slot < 9)
    }

    pub fn label(self) -> &'static str {
        TABLE.meta.get(self as usize).map_or("", |(label, _)| label.as_str())
    }

    pub fn group(self) -> &'static str {
        TABLE.meta.get(self as usize).map_or("", |(_, group)| group.as_str())
    }
}

#[derive(serde::Deserialize)]
struct DefaultRow {
    action: String,
    label: String,
    group: String,
    keys: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UserRow {
    action: String,
    keys: Vec<Option<String>>,
}

struct Table {
    meta: Vec<(String, String)>,
    defaults: Bindings,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    // The asset is compiled in and checked by a test; a parse failure leaves every action unbound.
    let rows: Vec<DefaultRow> = ron::from_str(DEFAULTS).unwrap_or_default();
    let mut meta = vec![(String::new(), String::new()); Action::ALL.len()];
    let mut defaults = Bindings::empty();
    for row in rows {
        let Some(action) = Action::from_name(&row.action) else { continue };
        meta[action as usize] = (row.label, row.group);
        for (slot, key) in row.keys.iter().take(SLOTS).enumerate() {
            defaults.assign(action, slot, Binding::parse(key));
        }
    }
    Table { meta, defaults }
});

static ACTIVE: LazyLock<RwLock<Bindings>> = LazyLock::new(|| RwLock::new(Bindings::defaults()));
static CAPTURE: Mutex<Option<(Action, usize, Instant)>> = Mutex::new(None);
static STATUS: Mutex<Option<String>> = Mutex::new(None);

/// Every action's bindings, indexed by action.
#[derive(Clone, PartialEq, Debug)]
pub struct Bindings {
    slots: Vec<[Option<Binding>; SLOTS]>,
}

impl Bindings {
    fn empty() -> Self {
        Self { slots: vec![[None; SLOTS]; Action::ALL.len()] }
    }

    pub fn defaults() -> Self {
        TABLE.defaults.clone()
    }

    pub fn slots(&self, action: Action) -> [Option<Binding>; SLOTS] {
        self.slots[action as usize]
    }

    pub fn lookup(&self, binding: Binding) -> Option<(Action, usize)> {
        self.slots.iter().enumerate().find_map(|(i, slots)| {
            let slot = slots.iter().position(|b| *b == Some(binding))?;
            Some((Action::ALL[i], slot))
        })
    }

    /// Binds `slot`; returns the action that lost `binding`, if another held it.
    pub fn assign(&mut self, action: Action, slot: usize, binding: Option<Binding>) -> Option<Action> {
        if slot >= SLOTS || binding.is_some_and(|b| matches!(b, Binding::Mouse(_)) && !action.mouse_bindable()) {
            return None;
        }
        let displaced = binding.and_then(|b| self.lookup(b)).filter(|(a, s)| (*a, *s) != (action, slot));
        if let Some((other, other_slot)) = displaced {
            self.slots[other as usize][other_slot] = None;
        }
        self.slots[action as usize][slot] = binding;
        displaced.map(|(other, _)| other).filter(|other| *other != action)
    }

    /// Defaults overridden by the actions a user file lists.
    fn from_user(text: &str) -> Result<Self> {
        let rows: Vec<UserRow> = ron::from_str(text).context("Bindings file is not valid")?;
        let mut bindings = Self::defaults();
        for row in rows {
            let Some(action) = Action::from_name(&row.action) else { continue };
            for slot in 0..SLOTS {
                bindings.assign(action, slot, None);
            }
            for (slot, key) in row.keys.iter().take(SLOTS).enumerate() {
                bindings.assign(action, slot, key.as_deref().and_then(Binding::parse));
            }
        }
        Ok(bindings)
    }

    fn to_user(&self) -> Result<String> {
        let rows: Vec<UserRow> = Action::ALL
            .iter()
            .map(|a| UserRow {
                action: a.name(),
                keys: self.slots(*a).iter().map(|b| b.map(Binding::name)).collect(),
            })
            .collect();
        Ok(ron::ser::to_string_pretty(&rows, ron::ser::PrettyConfig::default())?)
    }
}

pub fn current() -> Bindings {
    ACTIVE.read().unwrap_or_else(PoisonError::into_inner).clone()
}

pub fn lookup(binding: Binding) -> Option<(Action, usize)> {
    ACTIVE.read().unwrap_or_else(PoisonError::into_inner).lookup(binding)
}

pub fn action_for_key(code: winit::keyboard::KeyCode) -> Option<Action> {
    lookup(Binding::Key(code)).map(|(action, _)| action)
}

/// First bound input of `action`, for HUD hints.
pub fn hint(action: Action) -> String {
    current().slots(action).iter().flatten().next().map_or_else(|| "Unbound".to_owned(), |b| b.label())
}

/// Current label for a key named by its default label, so fixed hint strings follow rebinding.
pub fn rebound_label(default_label: &str) -> String {
    let defaults = Bindings::defaults();
    Action::ALL
        .iter()
        .find(|a| defaults.slots(**a).iter().flatten().any(|b| b.label() == default_label))
        .map_or_else(|| default_label.to_owned(), |a| hint(*a))
}

/// Installs `bindings` and writes them to the user file; the result is shown by the controls screen.
pub fn apply(bindings: Bindings) {
    let saved = save(&bindings);
    *ACTIVE.write().unwrap_or_else(PoisonError::into_inner) = bindings;
    set_status(saved.err().map(|e| format!("Bindings Not Saved: {e:#}")));
}

pub fn status() -> Option<String> {
    STATUS.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

fn set_status(status: Option<String>) {
    *STATUS.lock().unwrap_or_else(PoisonError::into_inner) = status;
}

pub fn begin_capture(action: Action, slot: usize) {
    *CAPTURE.lock().unwrap_or_else(PoisonError::into_inner) = Some((action, slot.min(SLOTS - 1), Instant::now()));
}

/// Called each frame the controls screen is drawn.
pub fn refresh_capture() {
    if let Some((_, _, seen)) = CAPTURE.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
        *seen = Instant::now();
    }
}

pub fn capturing() -> Option<(Action, usize)> {
    let mut capture = CAPTURE.lock().unwrap_or_else(PoisonError::into_inner);
    if capture.is_some_and(|(_, _, seen)| seen.elapsed() > CAPTURE_LAPSE) {
        *capture = None;
    }
    capture.map(|(action, slot, _)| (action, slot))
}

pub fn cancel_capture() {
    *CAPTURE.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

/// Offers a pressed input to a pending rebind. Returns whether it was consumed.
/// Escape cancels; a mouse button offered to a key-only action keeps listening.
pub fn offer(binding: Binding) -> bool {
    let Some((action, slot)) = capturing() else { return false };
    if binding == Binding::Key(winit::keyboard::KeyCode::Escape) {
        cancel_capture();
        return true;
    }
    if matches!(binding, Binding::Mouse(_)) && !action.mouse_bindable() {
        return true;
    }
    let mut bindings = current();
    let displaced = bindings.assign(action, slot, Some(binding));
    cancel_capture();
    apply(bindings);
    if let Some(other) = displaced
        && status().is_none()
    {
        set_status(Some(format!("{} · Unbound", other.label())));
    }
    true
}

fn path() -> PathBuf {
    crate::world::storage::save_directory().join("keybinds.ron")
}

/// Loads the user file if present; a bad file leaves the defaults and reports why.
pub fn load() {
    let result = (|| -> Result<Option<Bindings>> {
        let path = path();
        if !path.exists() {
            return Ok(None);
        }
        let mut text = String::new();
        fs::File::open(&path)?.take(MAX_FILE_BYTES + 1).read_to_string(&mut text)?;
        ensure!(text.len() as u64 <= MAX_FILE_BYTES, "Bindings file exceeds 64 KiB");
        Bindings::from_user(&text).map(Some)
    })();
    match result {
        Ok(Some(bindings)) => *ACTIVE.write().unwrap_or_else(PoisonError::into_inner) = bindings,
        Ok(None) => {}
        Err(e) => set_status(Some(format!("Bindings File Invalid: {e:#}"))),
    }
}

fn save(bindings: &Bindings) -> Result<()> {
    let path = path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temp, bindings.to_user()?)?;
    fs::rename(temp, path)?;
    Ok(())
}
