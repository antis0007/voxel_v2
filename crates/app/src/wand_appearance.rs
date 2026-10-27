//! Wand appearance table: visual families, authored pixel sprites, style mapping.
//! Purely visual: the save keeps `WandStyle` indices, and `styles` maps the head
//! index to a family, so old items keep their identity when art changes.
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

pub const SCHEMA_VERSION: u32 = 1;
/// Ordinary wands span 45-75 cm (WAND_ECOSYSTEM_20261026.md).
pub const LENGTH_RANGE_M: std::ops::RangeInclusive<f32> = 0.45..=0.75;
/// Keys whose colour comes from the style's shaft material, darkest first.
pub const SHAFT_KEYS: [u8; 4] = *b"1234";
/// Keys whose colour comes from the style's grip binding, darkest first.
pub const GRIP_KEYS: [u8; 3] = *b"567";

#[derive(Deserialize)]
pub struct Table {
    pub version: u32,
    /// Authored texel edge in model metres; the held rig scales it again.
    pub pixel_m: f32,
    pub width: usize,
    /// Family id per `WandStyle::head`.
    pub styles: Vec<String>,
    pub families: Vec<Family>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lineage { Vernacular, Scholarly, Industrial, Biological, Anomalous }

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status { Modeled, Planned }

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MaterialKind { Wood, Bone, Fiber, Metal, Ceramic, Mineral, Glass, Ember }

#[derive(Deserialize)]
pub struct Material {
    pub kind: MaterialKind,
    /// sRGB `#rrggbb`.
    pub srgb: String,
    /// Takes the spell colour and emits while casting.
    #[serde(default)]
    pub emitter: bool,
}

/// Rows top (tip) to bottom (butt). `Fill(share, rows)` tiles its rows over its
/// share of the rows left after the fixed ones, so length holds at any texel size.
#[derive(Deserialize)]
pub enum Segment { Fixed(Vec<String>), Fill(f32, Vec<String>) }

#[derive(Deserialize)]
pub struct Family {
    pub id: String,
    pub name: String,
    #[allow(dead_code, reason = "design data for the family catalogue; no runtime rule reads it yet")]
    pub lineage: Lineage,
    pub status: Status,
    pub silhouette: String,
    #[serde(default)]
    pub length_m: f32,
    /// Extrusion depth cap in texels; a run narrower than this extrudes to its width.
    #[serde(default)]
    pub max_depth: u8,
    #[serde(default)]
    pub materials: BTreeMap<char, Material>,
    #[serde(default)]
    pub segments: Vec<Segment>,
}

/// A family rasterised at one texel size. Cell 0 is empty, else the ASCII key.
pub struct Sprite {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u8>,
    /// Centre row of the grip binding: the hand socket.
    pub grip_row: f32,
    /// Texel coordinates of the casting origin, just above the topmost emitter row.
    pub muzzle: [f32; 2],
}

impl Sprite {
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { 0 } else { self.cells[y as usize * self.width + x as usize] }
    }
}

pub static TABLE: LazyLock<Table> = LazyLock::new(|| {
    ron::from_str(include_str!("../../../assets/wands.ron")).unwrap_or_else(|error| panic!("assets/wands.ron: {error}"))
});

/// Review-only override of the texel size, in model millimetres.
pub fn pixel_m() -> f32 {
    static OVERRIDE: LazyLock<Option<f32>> = LazyLock::new(|| {
        std::env::var("VOXEL_REVIEW_WAND_PIXEL_MM").ok().and_then(|s| s.parse::<f32>().ok()).filter(|mm| (2.0..=10.0).contains(mm)).map(|mm| mm / 1000.0)
    });
    OVERRIDE.unwrap_or(TABLE.pixel_m)
}

impl Table {
    pub fn family_index(&self, head: u8) -> usize {
        let id = &self.styles[head as usize % self.styles.len()];
        self.families.iter().position(|f| &f.id == id).unwrap_or(0)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != SCHEMA_VERSION { return Err(format!("version {} != {SCHEMA_VERSION}", self.version)); }
        if !(0.002..=0.01).contains(&self.pixel_m) { return Err(format!("pixel_m {} out of range", self.pixel_m)); }
        for (i, f) in self.families.iter().enumerate() {
            if self.families[..i].iter().any(|g| g.id == f.id) { return Err(format!("duplicate family {}", f.id)); }
            if f.name.is_empty() || f.silhouette.is_empty() { return Err(format!("{}: missing name or silhouette", f.id)); }
            // Emission belongs to channels and focus surfaces, never to bindings or fittings.
            if let Some((key, _)) = f.materials.iter().find(|(_, m)| m.emitter && matches!(m.kind, MaterialKind::Metal | MaterialKind::Fiber)) {
                return Err(format!("{}: {key:?} binding or fitting cannot emit", f.id));
            }
            match f.status {
                Status::Planned if !f.segments.is_empty() => return Err(format!("{}: planned family has art", f.id)),
                Status::Planned => {}
                Status::Modeled => { f.sprite(self.width, self.pixel_m).map_err(|e| format!("{}: {e}", f.id))?; }
            }
        }
        for id in &self.styles {
            match self.families.iter().find(|f| &f.id == id) {
                Some(f) if f.status == Status::Modeled => {}
                _ => return Err(format!("style {id} is not a modeled family")),
            }
        }
        Ok(())
    }
}

pub fn parse_srgb(hex: &str) -> Option<[f32; 3]> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 { return None; }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let linear = |v: u8| { let s = v as f32 / 255.0; if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) } };
    Some([linear(byte(0)?), linear(byte(2)?), linear(byte(4)?)])
}

impl Family {
    pub fn is_emitter(&self, key: u8) -> bool {
        self.materials.get(&(key as char)).is_some_and(|m| m.emitter)
    }

    pub fn sprite(&self, width: usize, pixel_m: f32) -> Result<Sprite, String> {
        if !LENGTH_RANGE_M.contains(&self.length_m) { return Err(format!("length {} m outside {LENGTH_RANGE_M:?}", self.length_m)); }
        if !(1..=6).contains(&self.max_depth) { return Err(format!("max_depth {}", self.max_depth)); }
        for m in self.materials.values() {
            if parse_srgb(&m.srgb).is_none() { return Err(format!("bad colour {}", m.srgb)); }
        }
        let (mut fixed, mut fills, mut shares) = (0usize, 0usize, 0.0f32);
        for segment in &self.segments {
            let rows = match segment {
                Segment::Fixed(r) => { fixed += r.len(); r }
                Segment::Fill(share, r) => {
                    if !share.is_finite() || *share <= 0.0 { return Err(format!("fill share {share}")); }
                    fills += 1;
                    shares += share;
                    r
                }
            };
            if rows.is_empty() { return Err("empty segment".into()); }
            for row in rows {
                if row.len() != width { return Err(format!("row {row:?} is {} wide, not {width}", row.len())); }
                if let Some(c) = row.bytes().find(|&c| c != b'.' && !SHAFT_KEYS.contains(&c) && !GRIP_KEYS.contains(&c) && !self.materials.contains_key(&(c as char))) {
                    return Err(format!("undefined key {:?}", c as char));
                }
            }
        }
        let height = (self.length_m / pixel_m).round() as usize;
        if fills == 0 || height < fixed + fills { return Err(format!("{fixed} fixed rows do not fit {height} rows at {pixel_m} m")); }
        let spare = height - fixed;
        let counts: Vec<usize> = self.segments.iter().filter_map(|s| match s { Segment::Fill(share, _) => Some(((spare as f32 * share / shares).floor() as usize).max(1)), _ => None }).collect();
        // Rounding leftovers go to the first fill, the long shaft.
        let mut leftover = spare as isize - counts.iter().sum::<usize>() as isize;
        let mut cells = Vec::with_capacity(height * width);
        let mut fill = 0;
        for segment in &self.segments {
            match segment {
                Segment::Fixed(rows) => rows.iter().for_each(|r| cells.extend(r.bytes())),
                Segment::Fill(_, rows) => {
                    let count = (counts[fill] as isize + std::mem::take(&mut leftover)).max(1) as usize;
                    fill += 1;
                    (0..count).for_each(|i| cells.extend(rows[i % rows.len()].bytes()));
                }
            }
        }
        let height = cells.len() / width;
        for c in &mut cells { if *c == b'.' { *c = 0; } }
        let mut sprite = Sprite { width, height, cells, grip_row: 0.0, muzzle: [0.0; 2] };
        let grip: Vec<usize> = (0..height).filter(|&y| (0..width).any(|x| GRIP_KEYS.contains(&sprite.cells[y * width + x]))).collect();
        let (Some(&top), Some(&bottom)) = (grip.first(), grip.last()) else { return Err("no grip binding".into()) };
        sprite.grip_row = (top + bottom) as f32 * 0.5 + 0.5;
        let Some(tip) = (0..height).find(|&y| (0..width).any(|x| self.is_emitter(sprite.cells[y * width + x]))) else { return Err("no emitter".into()) };
        let xs: Vec<usize> = (0..width).filter(|&x| self.is_emitter(sprite.cells[tip * width + x])).collect();
        sprite.muzzle = [(xs[0] + xs[xs.len() - 1] + 1) as f32 * 0.5, tip as f32];
        if top <= tip { return Err("grip above the emitter".into()); }
        connected(&sprite)?;
        Ok(sprite)
    }
}

/// One 4-connected body: no floating texels that read as noise or detach when held.
fn connected(s: &Sprite) -> Result<(), String> {
    let Some(start) = s.cells.iter().position(|&c| c != 0) else { return Err("empty sprite".into()) };
    let mut seen = vec![false; s.cells.len()];
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(i) = stack.pop() {
        let (x, y) = ((i % s.width) as i32, (i / s.width) as i32);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if s.at(x + dx, y + dy) == 0 { continue; }
            let j = (y + dy) as usize * s.width + (x + dx) as usize;
            if !seen[j] { seen[j] = true; stack.push(j); }
        }
    }
    match s.cells.iter().zip(&seen).position(|(&c, &v)| c != 0 && !v) {
        Some(i) => Err(format!("detached texel at ({}, {})", i % s.width, i / s.width)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wand_style::HEADS;

    #[test]
    fn wand_table_validates() {
        TABLE.validate().unwrap();
    }

    #[test]
    fn every_head_style_has_its_own_family() {
        assert_eq!(TABLE.styles.len(), HEADS.len());
        let mut families: Vec<usize> = (0..HEADS.len() as u8).map(|h| TABLE.family_index(h)).collect();
        for (h, &f) in families.iter().enumerate() {
            assert_eq!(TABLE.families[f].name, HEADS[h], "head {h} name disagrees with its family");
        }
        families.sort_unstable();
        families.dedup();
        assert_eq!(families.len(), HEADS.len(), "two head styles share a family");
    }

    #[test]
    fn twelve_planned_families_and_modeled_ones_are_honest() {
        assert_eq!(TABLE.families.len(), 12);
        for f in &TABLE.families {
            assert_eq!(f.status == Status::Modeled, !f.segments.is_empty(), "{}", f.id);
        }
    }

    #[test]
    fn silhouettes_differ_between_styles() {
        // Compare black masks at one texel size; palette swaps would score near 1.
        let sprites: Vec<Sprite> = (0..HEADS.len() as u8).map(|h| TABLE.families[TABLE.family_index(h)].sprite(TABLE.width, TABLE.pixel_m).unwrap()).collect();
        for (i, a) in sprites.iter().enumerate() {
            for b in &sprites[i + 1..] {
                let rows = a.height.min(b.height);
                let (mut both, mut either) = (0, 0);
                for y in 0..rows { for x in 0..a.width {
                    let (p, q) = (a.at(x as i32, y as i32) != 0, b.at(x as i32, y as i32) != 0);
                    both += usize::from(p && q);
                    either += usize::from(p || q);
                }}
                let overlap = both as f32 / either as f32;
                assert!(overlap < 0.72, "silhouette overlap {overlap:.2}");
            }
        }
    }

    #[test]
    fn length_holds_across_texel_sizes() {
        for f in TABLE.families.iter().filter(|f| f.status == Status::Modeled) {
            for mm in [4.0, 5.36, 6.0, 6.55] {
                let s = f.sprite(TABLE.width, mm / 1000.0).unwrap_or_else(|e| panic!("{} at {mm} mm: {e}", f.id));
                assert!((s.height as f32 * mm / 1000.0 - f.length_m).abs() <= mm / 1000.0, "{}", f.id);
            }
        }
    }

    #[test]
    fn rejects_broken_rows() {
        let family = |rows: Vec<&str>| Family {
            id: "t".into(), name: "t".into(), lineage: Lineage::Vernacular, status: Status::Modeled, silhouette: String::new(),
            length_m: 0.5, max_depth: 3, materials: BTreeMap::new(),
            segments: vec![Segment::Fixed(rows.into_iter().map(String::from).collect()), Segment::Fill(1.0, vec!["33".into()])],
        };
        assert!(family(vec!["3x"]).sprite(2, 0.006).is_err(), "undefined key");
        assert!(family(vec!["333"]).sprite(2, 0.006).is_err(), "wrong width");
        assert!(family(vec!["33", "56"]).sprite(2, 0.006).is_err(), "no emitter");
    }
}
