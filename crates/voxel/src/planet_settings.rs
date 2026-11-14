//! Versioned planet recipes. Integer dials keep saves and peers bit-exact.
pub const CUSTOM_PLANET_VERSION: u32 = 136;
pub const PLANET_DIALS: usize = 12;

/// A generation choice, retained by recipes, saves, and multiplayer snapshots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CivilizationMode {
    #[default]
    Living,
    Wilderness,
}

impl CivilizationMode {
    pub fn code(self) -> u32 {
        match self { Self::Living => 0, Self::Wilderness => 1 }
    }

    pub fn from_code(code: u32) -> Option<Self> {
        match code { 0 => Some(Self::Living), 1 => Some(Self::Wilderness), _ => None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanetSettings {
    pub values: [u8; PLANET_DIALS],
    #[serde(default)]
    pub civilization: CivilizationMode,
    /// Length of the simulated geological history in Myr. Zero keeps the
    /// generator's own length, so older recipes and saves are unchanged.
    #[serde(default)]
    pub history_myr: u16,
}

/// Full length of the deep-time history: the generator default, and the only
/// length the geology snapshot format stores. Every crate reads it from here.
pub const FULL_HISTORY_MYR: u16 = 720;
/// Shortest and longest history a world may ask for, and the slider step.
/// The step matches the 10 Myr interval of the history snapshots.
pub const HISTORY_MYR_RANGE: std::ops::RangeInclusive<u16> = 20..=FULL_HISTORY_MYR;
pub const HISTORY_MYR_STEP: u16 = 10;

impl Default for PlanetSettings {
    fn default() -> Self { Self { values: [50; PLANET_DIALS], civilization: CivilizationMode::Living, history_myr: 0 } }
}

impl PlanetSettings {
    pub fn valid(self) -> bool {
        self.values.iter().all(|v| *v <= 100) && Self::valid_history(self.history_myr)
    }
    pub fn valid_history(myr: u16) -> bool {
        myr == 0 || (HISTORY_MYR_RANGE.contains(&myr) && myr % HISTORY_MYR_STEP == 0)
    }
    /// The chosen history length, or `None` when the generator decides. The
    /// full length is the default however it is stored, so one planet has one
    /// identity, recipe and cache key.
    pub fn history_myr(self) -> Option<u16> {
        (self.history_myr != 0 && self.history_myr != FULL_HISTORY_MYR).then_some(self.history_myr)
    }
    /// Any requested length as a valid one: inside the range, on the step.
    pub fn snap_history(myr: u16) -> u16 {
        let myr = myr.clamp(*HISTORY_MYR_RANGE.start(), FULL_HISTORY_MYR);
        ((myr + HISTORY_MYR_STEP / 2) / HISTORY_MYR_STEP * HISTORY_MYR_STEP).min(FULL_HISTORY_MYR)
    }
    /// The value to store for a length: zero for the full history.
    pub fn stored_history(myr: u16) -> u16 { if myr == FULL_HISTORY_MYR { 0 } else { myr } }
    pub fn unit(self, dial: usize) -> f64 { f64::from(self.values[dial].min(100)) / 100.0 }
    pub fn fingerprint(self) -> u64 {
        let terrain = self.values.iter().fold(0xcbf29ce484222325, |h, v| (h ^ u64::from(*v)).wrapping_mul(0x100000001b3));
        // Preserve existing planet identities while keeping civilization-dependent caches distinct.
        let identity = match self.civilization {
            CivilizationMode::Living => terrain,
            CivilizationMode::Wilderness => (terrain ^ 0x57494c44).wrapping_mul(0x100000001b3),
        };
        // A chosen history is a different planet; the default keeps its old identity.
        match self.history_myr() {
            None => identity,
            Some(myr) => (identity ^ (0x4d5952_0000 | u64::from(myr))).wrapping_mul(0x100000001b3),
        }
    }
    pub fn recipe(self, seed: u32) -> String {
        let dials: String = self.values.iter().map(|v| format!("{v:02X}")).collect();
        let mut body = format!("AX136-{seed:08X}-{dials}");
        if self.civilization == CivilizationMode::Wilderness { body.push_str("-W"); }
        // Optional history segment, e.g. `-H120`; absent for the generator default.
        if let Some(myr) = self.history_myr() { body.push_str(&format!("-H{myr:03}")); }
        format!("{body}-{:08X}", recipe_checksum(&body))
    }
    pub fn from_recipe(text: &str) -> Result<(u32, Self), &'static str> {
        let text = text.trim();
        if !text.is_ascii() || !matches!(text.len(), 48 | 50 | 53 | 55) { return Err("Invalid recipe length"); }
        let parts: Vec<_> = text.split('-').collect();
        if parts.len() < 4 || parts[0] != "AX136" || parts[1].len() != 8 || parts[2].len() != 24 || parts.last().is_none_or(|p| p.len() != 8) {
            return Err("Invalid recipe format");
        }
        // Optional segments sit between the dials and the checksum, in this order.
        let mut options = parts[3..parts.len() - 1].iter().copied().peekable();
        let wilderness = options.next_if_eq(&"W").is_some();
        let history = options.next_if(|p| p.len() == 4 && p.starts_with('H'));
        if options.next().is_some() { return Err("Invalid recipe format"); }
        let sum = u32::from_str_radix(parts[parts.len() - 1], 16).map_err(|_| "Invalid recipe checksum")?;
        if recipe_checksum(&text[..text.len() - 9]) != sum { return Err("Recipe checksum mismatch"); }
        let seed = u32::from_str_radix(parts[1], 16).map_err(|_| "Invalid recipe seed")?;
        let mut settings = Self::default();
        if wilderness { settings.civilization = CivilizationMode::Wilderness; }
        if let Some(history) = history {
            settings.history_myr = history[1..].parse().map_err(|_| "Invalid recipe history")?;
            if settings.history_myr == 0 { return Err("Invalid recipe history"); }
            settings.history_myr = Self::stored_history(settings.history_myr);
        }
        for (i, value) in settings.values.iter_mut().enumerate() {
            *value = u8::from_str_radix(&parts[2][i * 2..i * 2 + 2], 16).map_err(|_| "Invalid recipe dial")?;
        }
        if !settings.values.iter().all(|v| *v <= 100) { return Err("Recipe dial out of range"); }
        if !settings.valid() { return Err("Recipe history out of range"); }
        Ok((seed, settings))
    }
    pub fn randomized(seed: u32) -> Self {
        let mut h = seed ^ 0xa341316c;
        let mut settings = Self::default();
        for value in &mut settings.values {
            h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
            h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
            h ^= h >> 16;
            *value = (h % 101) as u8;
        }
        settings
    }
}

fn recipe_checksum(body: &str) -> u32 {
    body.bytes().fold(0x811c9dc5, |h, b| (h ^ u32::from(b)).wrapping_mul(0x01000193))
}
