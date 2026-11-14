//! Wand cosmetics: what the wand is made of, what crowns it, how it glows and what
//! it sheds. Purely visual, so it lives beside the saves rather than inside them:
//! a tiny text file of `key=value` lines that can never break a world.
use std::{
    fs,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

pub struct Shaft {
    pub name: &'static str,
    pub base: [f32; 3],
    pub dark: [f32; 3],
    pub accent: [f32; 3],
}

pub const SHAFTS: [Shaft; 6] = [
    Shaft {
        name: "Hearth oak",
        base: [0.42, 0.26, 0.13],
        dark: [0.24, 0.14, 0.07],
        accent: [0.72, 0.52, 0.26],
    },
    Shaft {
        name: "Pale bone",
        base: [0.86, 0.82, 0.70],
        dark: [0.55, 0.50, 0.40],
        accent: [0.98, 0.95, 0.84],
    },
    Shaft {
        name: "Frozen crystal",
        base: [0.45, 0.80, 0.95],
        dark: [0.20, 0.45, 0.62],
        accent: [0.80, 0.97, 1.00],
    },
    Shaft {
        name: "Obsidian",
        base: [0.07, 0.06, 0.11],
        dark: [0.03, 0.02, 0.06],
        accent: [0.52, 0.30, 0.90],
    },
    Shaft {
        name: "Verdigris copper",
        base: [0.27, 0.62, 0.52],
        dark: [0.14, 0.34, 0.28],
        accent: [0.86, 0.46, 0.18],
    },
    Shaft {
        name: "Starwood",
        base: [0.10, 0.12, 0.27],
        dark: [0.04, 0.05, 0.14],
        accent: [0.80, 0.86, 1.00],
    },
];

/// Family names of `assets/wands.ron` styles, in head order (a test keeps them equal).
pub const HEADS: [&str; 8] = [
    "Witchwood switch",
    "Shard cluster",
    "Bifurcated focus",
    "Astral filament",
    "Cinder rod",
    "Bone needle",
    "Runic key",
    "Rootbound scepter",
];
pub const TRAILS: [&str; 5] = [
    "None",
    "Sparks",
    "Orbiting stars",
    "Rising embers",
    "Comet ribbon",
];
pub const GRIPS: [(&str, [f32; 3]); 6] = [
    ("Saddle leather", [0.36, 0.20, 0.10]),
    ("Midnight silk", [0.12, 0.10, 0.30]),
    ("Ember cord", [0.72, 0.20, 0.10]),
    ("Moss wrap", [0.20, 0.38, 0.16]),
    ("Gilded wire", [0.86, 0.66, 0.22]),
    ("Frost linen", [0.74, 0.84, 0.90]),
];
/// None follows the spell on the wand's cursor; the rest are fixed hues.
pub const GLOWS: [(&str, Option<[f32; 3]>); 7] = [
    ("Spell colour", None),
    ("Aether blue", Some([0.32, 0.72, 1.00])),
    ("Ember", Some([1.00, 0.46, 0.14])),
    ("Verdant", Some([0.36, 1.00, 0.52])),
    ("Violet", Some([0.70, 0.36, 1.00])),
    ("Rose gold", Some([1.00, 0.62, 0.52])),
    ("Starlight", Some([0.92, 0.95, 1.00])),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WandStyle {
    pub shaft: u8,
    pub head: u8,
    pub trail: u8,
    pub grip: u8,
    pub glow: u8,
    /// Rune beads on the shaft that fill with mana: 0 none, 1 beads, 2 etched sigils.
    pub runes: u8,
}

impl Default for WandStyle {
    fn default() -> Self {
        Self {
            shaft: 0,
            head: 0,
            trail: 1,
            grip: 0,
            glow: 0,
            runes: 1,
        }
    }
}

impl WandStyle {
    pub fn clamped(mut self) -> Self {
        self.shaft = self.shaft.min(SHAFTS.len() as u8 - 1);
        self.head = self.head.min(HEADS.len() as u8 - 1);
        self.trail = self.trail.min(TRAILS.len() as u8 - 1);
        self.grip = self.grip.min(GRIPS.len() as u8 - 1);
        self.glow = self.glow.min(GLOWS.len() as u8 - 1);
        self.runes = self.runes.min(2);
        self
    }

    pub fn encode(self) -> String {
        format!(
            "shaft={}\nhead={}\ntrail={}\ngrip={}\nglow={}\nrunes={}\n",
            self.shaft, self.head, self.trail, self.grip, self.glow, self.runes
        )
    }

    /// Unknown keys and bad numbers are ignored, so an old or hand-edited file still loads.
    pub fn decode(text: &str) -> Self {
        let mut style = Self::default();
        for line in text.lines().take(32) {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Ok(n) = value.trim().parse::<u8>() else {
                continue;
            };
            match key.trim() {
                "shaft" => style.shaft = n,
                "head" => style.head = n,
                "trail" => style.trail = n,
                "grip" => style.grip = n,
                "glow" => style.glow = n,
                "runes" => style.runes = n,
                _ => {}
            }
        }
        style.clamped()
    }

    /// Glow colour for this style, given the spell currently on the wand's cursor.
    pub fn glow_colour(self, spell: [f32; 3]) -> [f32; 3] {
        GLOWS[self.glow as usize].1.unwrap_or(spell)
    }
}

fn path() -> PathBuf {
    crate::world::storage::save_directory().join("wand-style.txt")
}

static STYLE: LazyLock<Mutex<WandStyle>> = LazyLock::new(|| {
    let style = fs::read_to_string(path())
        .map(|text| WandStyle::decode(&text))
        .unwrap_or_default();
    Mutex::new(style)
});

pub fn current() -> WandStyle {
    *STYLE.lock().unwrap()
}

/// Show a style without saving it, for review captures and previews.
pub fn preview(style: WandStyle) {
    *STYLE.lock().unwrap() = style.clamped();
}

/// Apply and persist. A failed write is reported but never fatal: it is cosmetics.
pub fn set(style: WandStyle) {
    let style = style.clamped();
    let mut slot = STYLE.lock().unwrap();
    if *slot == style {
        return;
    }
    *slot = style;
    let target = path();
    if let Some(dir) = target.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Err(e) = fs::write(&target, style.encode()) {
        eprintln!("[wand-style] could not save {}: {e}", target.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_clamps() {
        let style = WandStyle {
            shaft: 3,
            head: 5,
            trail: 4,
            grip: 2,
            glow: 6,
            runes: 2,
        };
        assert_eq!(WandStyle::decode(&style.encode()), style);
        let wild = WandStyle::decode("shaft=99\nhead=200\nbogus=1\ntrail=x\nglow=250\n");
        assert_eq!(wild.shaft as usize, SHAFTS.len() - 1);
        assert_eq!(wild.head as usize, HEADS.len() - 1);
        assert_eq!(wild.trail, WandStyle::default().trail);
        assert_eq!(wild.glow as usize, GLOWS.len() - 1);
    }

    #[test]
    fn an_empty_file_is_the_default_wand() {
        assert_eq!(WandStyle::decode(""), WandStyle::default());
    }
}
