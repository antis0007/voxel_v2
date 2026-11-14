//! Text voxel models on an 8-per-metre grid, authored in `assets/models/**.vox`.
//!
//! Format: `size X Y Z`, palette lines `p <char> <rrggbb> <glow>`, then for each
//! occupied layer `y <n>` followed by one row of X characters per Z. `.` is empty.
//! The model fills one metre cell; +X is its front or arm direction.
use super::Model;
use glam::{IVec3, Quat, Vec3};
use std::{collections::{BTreeMap, BTreeSet}, sync::{Arc, LazyLock, RwLock, atomic::{AtomicBool, Ordering}}};

pub type Previews = BTreeMap<String, Arc<VoxelModel>>;
static PREVIEWS: LazyLock<RwLock<Previews>> = LazyLock::new(|| RwLock::new(BTreeMap::new()));
static LABELS: LazyLock<RwLock<BTreeSet<String>>> = LazyLock::new(|| RwLock::new(BTreeSet::new()));
static PREVIEW_ACTIVE: AtomicBool = AtomicBool::new(false);
pub fn previews() -> Previews { PREVIEWS.read().unwrap().clone() }
pub fn restore_previews(value: Previews) {
    let active = !value.is_empty();
    *PREVIEWS.write().unwrap() = value;
    PREVIEW_ACTIVE.store(active, Ordering::Relaxed);
}
pub fn loaded_assets() -> Vec<String> { LABELS.read().unwrap().iter().cloned().collect() }

/// Validate the entire small model before publishing. No file reads, partial
/// parsing, shader changes or accumulated leaked models on repeated edits.
pub fn preview(asset: &str, source: &str) -> Result<usize, String> {
    if !LABELS.read().unwrap().contains(asset) { return Err("Model is not loaded in this scene; inspect models for available names".into()); }
    if source.len() > 16_384 || source.lines().next() != Some("size 8 8 8") { return Err("Expected size 8 8 8 and at most 16384 bytes".into()); }
    let mut palette = BTreeSet::new();
    let mut layers = BTreeSet::new();
    let mut rows = None;
    for line in source.lines().skip(1).map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let words: Vec<_> = line.split_whitespace().collect();
        match words[0] {
            "p" if words.len() == 4 => {
                let key = words[1];
                let glow = words[3].parse::<f32>().map_err(|_| "Invalid model glow")?;
                if key.len() != 1 || key == "." || key == "#" || !key.is_ascii() || !palette.insert(key.as_bytes()[0])
                    || words[2].len() != 6 || u32::from_str_radix(words[2], 16).is_err()
                    || !glow.is_finite() || !(0.0..=8.0).contains(&glow) { return Err("Invalid or duplicate model palette entry".into()); }
            }
            "y" if words.len() == 2 => {
                let layer = words[1].parse::<u8>().map_err(|_| "Invalid model layer")?;
                if rows.is_some_and(|n| n != 8) || layer >= 8 || !layers.insert(layer) { return Err("Incomplete or duplicate model layer".into()); }
                rows = Some(0);
            }
            _ => {
                if words.len() != 1 || line.len() != 8 || rows.is_none_or(|n| n >= 8)
                    || !line.bytes().all(|c| c == b'.' || palette.contains(&c)) { return Err("Invalid voxel row or unknown palette key".into()); }
                rows = rows.map(|n| n + 1);
            }
        }
    }
    if rows != Some(8) { return Err("A complete eight-row layer is required".into()); }
    let model = VoxelModel::parse(source, "");
    let count = model.voxels.len();
    if count == 0 { return Err("Model must contain at least one voxel".into()); }
    PREVIEWS.write().unwrap().insert(asset.into(), Arc::new(model));
    PREVIEW_ACTIVE.store(true, Ordering::Relaxed);
    Ok(count)
}

pub const PER_METRE: i32 = 8;

pub struct Voxel {
    cell: IVec3,
    key: char,
    color: [f32; 3],
    glow: f32,
    /// Faces with no filled neighbour, in `FACES` order.
    exposed: [bool; 6],
}

pub struct VoxelModel {
    voxels: Vec<Voxel>,
    label: String,
}

const FACES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// Palette keys the caller may recolour per draw.
pub struct Tint {
    pub stripe: [f32; 3],
    pub trim: [f32; 3],
    /// Scales every palette glow; 0 turns a lamp off.
    pub glow: f32,
}

/// Renderer colours are display-referred, like the other authored models.
fn channel(c: u8) -> f32 {
    c as f32 / 255.0
}

impl VoxelModel {
    /// Dev-panel overrides are scoped to one playtest process and asset name.
    pub fn with_preview(source: &str, label: &str) -> Self {
        LABELS.write().unwrap().insert(label.into());
        let name = std::env::var("VOXEL_FORGE_MODEL").ok();
        if name.as_deref() == Some(label)
            && let Some(file) = std::env::var_os("VOXEL_FORGE_MODEL_FILE")
            && std::fs::metadata(&file).is_ok_and(|meta| meta.is_file() && meta.len() <= 16_384)
            && let Ok(text) = std::fs::read_to_string(file)
            && text.lines().next() == Some("size 8 8 8")
        {
            let model = Self::parse(&text, label);
            if !model.voxels.is_empty() && model.voxels.iter().all(|v| {
                v.cell.min_element() >= 0 && v.cell.max_element() < PER_METRE
            }) {
                return model;
            }
        }
        Self::parse(source, label)
    }

    pub fn parse(source: &str, label: &str) -> Self {
        let mut palette: Vec<(char, [f32; 3], f32)> = Vec::new();
        let mut voxels = Vec::new();
        let mut layer: Option<i32> = None;
        let mut row = 0;
        for line in source.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let mut word = line.split_whitespace();
            match word.next() {
                Some("size") => {}
                Some("p") => {
                    let (Some(k), Some(hex), Some(glow)) = (word.next(), word.next(), word.next()) else { continue };
                    let Some(key) = k.chars().next() else { continue };
                    let Ok(rgb) = u32::from_str_radix(hex, 16) else { continue };
                    let color = [channel((rgb >> 16) as u8), channel((rgb >> 8) as u8), channel(rgb as u8)];
                    palette.push((key, color, glow.parse().unwrap_or(0.0)));
                }
                Some("y") => {
                    layer = word.next().and_then(|w| w.parse().ok());
                    row = 0;
                }
                Some(cells) => {
                    let Some(y) = layer else { continue };
                    for (x, ch) in cells.chars().enumerate() {
                        if ch == '.' {
                            continue;
                        }
                        let Some(&(key, color, glow)) = palette.iter().find(|p| p.0 == ch) else {
                            eprintln!("[voxel-asset] {label}: unknown palette key {ch}");
                            continue;
                        };
                        voxels.push(Voxel { cell: IVec3::new(x as i32, y, row), key, color, glow, exposed: [true; 6] });
                    }
                    row += 1;
                }
                None => {}
            }
        }
        let cells: std::collections::HashSet<IVec3> = voxels.iter().map(|v| v.cell).collect();
        for v in &mut voxels {
            for (i, d) in FACES.iter().enumerate() {
                v.exposed[i] = !cells.contains(&(v.cell + *d));
            }
        }
        Self { voxels, label: label.into() }
    }

    /// Draws the model in the cell whose minimum corner is `origin` (metres),
    /// rotated about the cell centre. Faces between filled voxels are skipped.
    pub fn draw(&self, mesh: &mut Model, origin: Vec3, rotation: Quat, tint: &Tint) {
        if PREVIEW_ACTIVE.load(Ordering::Relaxed) && !self.label.is_empty() {
            let preview = PREVIEWS.read().unwrap().get(&self.label).cloned();
            if let Some(preview) = preview { preview.draw(mesh, origin, rotation, tint); return; }
        }
        let unit = 1.0 / PER_METRE as f32;
        let centre = Vec3::splat(0.5);
        for v in &self.voxels {
            let (color, glow) = match v.key {
                'S' => (tint.stripe, v.glow),
                'L' => (tint.stripe.map(|c| c * 1.6 + 0.15), v.glow),
                'T' => (tint.trim, v.glow),
                'U' => (tint.trim.map(|c| c * 1.5 + 0.2), v.glow),
                _ => (v.color, v.glow),
            };
            let glow = glow * tint.glow;
            // Grain: a stable per-voxel shift so flat runs read as pixels.
            let h = (v.cell.x * 73 + v.cell.y * 151 + v.cell.z * 211).rem_euclid(7) as f32;
            let grain = 0.96 + 0.01 * h;
            let color = color.map(|c| c * grain);
            let at = (v.cell.as_vec3() + Vec3::splat(0.5)) * unit;
            let at = origin + centre + rotation * (at - centre);
            for (i, d) in FACES.iter().enumerate() {
                if v.exposed[i] {
                    mesh.voxel_face(at, rotation, d.as_vec3(), unit, color, glow);
                }
            }
        }
    }
}

impl Model {
    /// One square face of a voxel centred at `at`, facing `dir` before `rotation`.
    fn voxel_face(&mut self, at: Vec3, rotation: Quat, dir: Vec3, unit: f32, color: [f32; 3], glow: f32) {
        let u = if dir.y.abs() > 0.5 { Vec3::X } else { Vec3::Y };
        let a = dir.cross(u).normalize() * (unit * 0.5);
        let b = dir.cross(a).normalize() * (unit * 0.5);
        let c = at + dir * (unit * 0.5);
        let p = [c - a - b, c + a - b, c + a + b, c - a + b].map(|q| at + rotation * (q - at));
        let n = rotation * dir;
        self.triangle([p[0], p[1], p[2]], n, color, glow);
        self.triangle([p[0], p[2], p[3]], n, color, glow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assets_parse_with_expected_counts_and_colours() {
        let hub = VoxelModel::parse(include_str!("../../../../../assets/models/factory/pipe_hub.vox"), "pipe_hub");
        assert_eq!(hub.voxels.len(), 64);
        let m = hub.voxels.iter().find(|v| v.key == 'M').expect("steel voxel");
        assert!(m.color[0] > 0.25 && m.color[2] > m.color[0], "steel colour {:?}", m.color);
        let arm = VoxelModel::parse(include_str!("../../../../../assets/models/factory/cable_arm.vox"), "cable_arm");
        assert!(arm.voxels.len() >= 12, "cable arm has {} voxels", arm.voxels.len());
        // Interior faces are culled: a 4x4x4 block exposes 96 faces.
        let faces: usize = hub.voxels.iter().map(|v| v.exposed.iter().filter(|e| **e).count()).sum();
        assert_eq!(faces, 96);
    }

    #[test]
    fn a_pipe_arm_spans_three_eighths_by_half_a_metre() {
        let arm = VoxelModel::parse(include_str!("../../../../../assets/models/factory/pipe_arm.vox"), "pipe_arm");
        let mut mesh = Model::with_vertices(1.0, Vec::new());
        arm.draw(&mut mesh, Vec3::ZERO, Quat::IDENTITY, &Tint { stripe: [0.3; 3], trim: [0.3; 3], glow: 1.0 });
        let (lo, hi) = mesh.vertices.iter().fold((Vec3::splat(9.0), Vec3::splat(-9.0)), |(lo, hi), v| {
            let p = Vec3::from_array(v.pos);
            (lo.min(p), hi.max(p))
        });
        assert!((hi.x - lo.x - 0.375).abs() < 1e-4 && (hi.y - lo.y - 0.5).abs() < 1e-4, "bounds {lo:?} {hi:?}");
    }
}
