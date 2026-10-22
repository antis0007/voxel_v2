//! Records: short found texts tied to where they are found.
//!
//! A Record is chosen from the place name (`sites::location`), the depth and the
//! container's cell, so the same coffer always holds the same paper. Nothing here
//! touches terrain sampling or the save. Pools follow docs/WORLD_LORE.md: depth is
//! age, each office keeps to its own concern, and Records may be wrong.
use glam::IVec3;

/// Who wrote it. Depth picks the stratum when the place name does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    Artificers,
    Watch,
    Sluice,
    Founders,
    Keepers,
    Archivists,
    Choir,
    Ballast,
    Anchor,
    Surveyors,
    Holds,
}

const ARTIFICERS: &[&str] = &[
    "Tally plate: 41 given, 0 owed. Signed with a hook-shaped mark.",
    "Work order, unfinished: \"Coil for the ninth office. Do not log.\"",
    "A coil wound backwards, labelled TEST, kept anyway.",
    "Inspection slip: \"Card passes. Card should not pass. Passing it.\"",
];
const WATCH: &[&str] = &[
    "Watch log: \"Descent 18 211. On time. The carriage lamp showed two colours.\"",
    "Toll board: up, one spark. Down, nothing. Someone added: \"Down, everything.\"",
    "Timetable slate. One corridor is struck out and dated, twice, in different hands.",
    "Watch log: \"Light on the braid with no carriage under it. Not logged.\"",
];
const SLUICE: &[&str] = &[
    "Gauge card: \"Level held at three breaths. Ballast nominal.\"",
    "Sluice permit, stamped VOID in a later ink.",
    "Conversion table, held breaths to tonnes. The last row is in pencil.",
];
const FOUNDERS: &[&str] = &[
    "Order book: \"Coupling 7 of 12. Flaw noted. Ship.\"",
    "A test bar snapped clean. Grey at the break, as if something ate the core.",
    "Melt record. Every entry for a month reads \"for the Loom\".",
];
const KEEPERS: &[&str] = &[
    "Seed label: \"Strain R-9, recovery. Do not plant near boards.\"",
    "Air filter packed solid with pale threads. Dated the week the doors were taped.",
    "Planting rota for two gardens. The upper one has no soil column.",
];
const ARCHIVISTS: &[&str] = &[
    "Ledger leaf: forty names, then one line scraped bare.",
    "Index card: \"Ninth office: see Return.\" The card for Return is missing.",
    "Ledger leaf. The same name appears in the list of those who stayed and those who rose.",
];
const CHOIR: &[&str] = &[
    "Tuning chart: one note per strand. Strand 31 is struck through.",
    "Hymn sheet. Under the words, in small type, a tension in newtons.",
    "A crystal reed, still faintly ringing a note nobody here can sing.",
];
const BALLAST: &[&str] = &[
    "A child's slate: the timetable copied badly, and a ladder drawn up into the sky.",
    "Still ledger, unregistered: \"For the wake. Make it strong.\"",
    "Scratched on a pot: \"lanterncap, salt, patience.\"",
    "Note pinned to a door: \"Gone to the ninth. Keep the fire banked.\"",
];
const ANCHOR: &[&str] = &[
    "Cold-store tag: \"Ring 4, resonance held 212 years.\" The number is crossed out and raised.",
    "Clean-room notice: \"Gown. Shower. Wait. The Bloom waits longer.\"",
    "Wafer in a sealed sleeve, grey across half its face. Yield: none.",
    "Office directory. Eight offices. A ninth line, painted over.",
];
const SURVEYORS: &[&str] = &[
    "A loose tile of fused basalt, flat to the limit of any tool you own.",
    "Survey mark: three dots and a line, the same spacing as the last one you passed.",
    "An Anchor surveyor's note: \"Our span is theirs. We never found their zero.\"",
];
const HOLDS: &[&str] = &[
    "A wand-lord's writ in wax: \"Salvage rights to this ruin are the Hold's.\" The Hold is gone.",
    "Copyist page: a circuit drawn as an illumination, gold leaf on every trace.",
    "Prayer card of the Orders: \"Hold the note. Keep the ring. Do not ask the number.\"",
    "Pilgrim's note: \"The carriage came. Nobody got off. Next one, then.\"",
];

impl Pool {
    fn records(self) -> &'static [&'static str] {
        match self {
            Self::Artificers => ARTIFICERS,
            Self::Watch => WATCH,
            Self::Sluice => SLUICE,
            Self::Founders => FOUNDERS,
            Self::Keepers => KEEPERS,
            Self::Archivists => ARCHIVISTS,
            Self::Choir => CHOIR,
            Self::Ballast => BALLAST,
            Self::Anchor => ANCHOR,
            Self::Surveyors => SURVEYORS,
            Self::Holds => HOLDS,
        }
    }

    /// Place names come from `sites::location`; unknown places fall back to depth.
    pub fn for_place(location: Option<&str>, depth_m: f32) -> Self {
        let name = location.unwrap_or("").to_ascii_lowercase();
        let has = |keys: &[&str]| keys.iter().any(|k| name.contains(k));
        if has(&["basalt"]) {
            Self::Surveyors
        } else if has(&["rime"]) {
            Self::Anchor
        } else if has(&["fungal layer", "lanternroot", "embercap", "mycel", "catacomb"]) {
            Self::Ballast
        } else if has(&["workshop"]) {
            Self::Artificers
        } else if has(&["watchtower", "toll"]) {
            Self::Watch
        } else if has(&["waterworks", "drowned", "undercroft", "cistern"]) {
            Self::Sluice
        } else if has(&["foundry", "forge"]) {
            Self::Founders
        } else if has(&["conservatory", "grove", "garden"]) {
            Self::Keepers
        } else if has(&["archive"]) {
            Self::Archivists
        } else if has(&["cathedral", "nave", "prism", "choir"]) {
            Self::Choir
        } else if depth_m < -60.0 {
            Self::Surveyors
        } else if depth_m < -25.0 {
            Self::Anchor
        } else if depth_m < -4.0 {
            Self::Ballast
        } else {
            Self::Holds
        }
    }
}

fn mix(p: IVec3, seed: u32) -> u32 {
    let mut h = (p.x as u32).wrapping_mul(0x9e37_79b1)
        ^ (p.y as u32).wrapping_mul(0x85eb_ca77)
        ^ (p.z as u32).wrapping_mul(0xc2b2_ae3d)
        ^ seed.wrapping_mul(0x27d4_eb2f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h
}

/// The Record found in the container at `cell`, if any. About half of all
/// containers hold one: scarce enough that each line is read.
pub fn record(seed: u32, cell: IVec3, location: Option<&str>, depth_m: f32) -> Option<&'static str> {
    let h = mix(cell, seed);
    if h & 1 == 0 {
        return None;
    }
    let records = Pool::for_place(location, depth_m).records();
    Some(records[(h >> 1) as usize % records.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_stable_and_follow_place() {
        let cell = IVec3::new(12, -40, 7);
        assert_eq!(record(9, cell, Some("Abandoned foundry"), -2.0), record(9, cell, Some("Abandoned foundry"), -2.0));
        assert_eq!(Pool::for_place(Some("Ruin descent — rime layer"), -50.0), Pool::Anchor);
        assert_eq!(Pool::for_place(Some("Resonant archive"), 0.0), Pool::Archivists);
        assert_eq!(Pool::for_place(None, 3.0), Pool::Holds);
        assert_eq!(Pool::for_place(None, -80.0), Pool::Surveyors);
        let found = (0..64).filter(|i| record(9, IVec3::new(*i, 0, 0), None, 0.0).is_some()).count();
        assert!((16..48).contains(&found), "{found} of 64 containers hold a record");
    }

    #[test]
    fn records_fit_the_message_line() {
        for pool in [Pool::Artificers, Pool::Watch, Pool::Sluice, Pool::Founders, Pool::Keepers,
            Pool::Archivists, Pool::Choir, Pool::Ballast, Pool::Anchor, Pool::Surveyors, Pool::Holds] {
            for r in pool.records() {
                assert!(r.len() <= 100, "{r}");
            }
        }
    }
}
