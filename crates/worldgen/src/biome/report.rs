//! Per-region biome counts, habitat transition coverage and boundary lengths.
use crate::WorldSampleConfig;
use crate::landscape::{self, Biome};

/// Runner-up share above which a column counts as an ecotone.
pub const ECOTONE_BLEND: f32 = 0.3;
/// Border pairs listed by `to_text`.
const TEXT_BORDERS: usize = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct BiomeReport {
    pub columns: u32,
    /// Columns per label, most common first.
    pub families: Vec<(Biome, u32)>,
    pub ecotone_columns: u32,
    /// Metres of boundary per pair of differing labels, longest first.
    pub borders: Vec<(Biome, Biome, f32)>,
}

/// Samples a `size_m` square centred on `centre_m` every `step_m` metres.
pub fn report(cfg: &WorldSampleConfig, centre_m: (f32, f32), size_m: f32, step_m: f32) -> BiomeReport {
    assert!(size_m.is_finite() && size_m >= 0.0 && step_m.is_finite() && step_m > 0.0, "Invalid biome report sampling extent");
    let samples = (size_m / step_m).round();
    assert!(samples <= 2048.0, "Biome report exceeds 2048 samples per axis");
    let n = samples as usize;
    let metres = cfg.voxel_meters();
    let start = (centre_m.0 - size_m / 2.0, centre_m.1 - size_m / 2.0);
    let mut grid = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let x = ((start.0 + i as f32 * step_m) / metres).round() as i32;
            let z = ((start.1 + j as f32 * step_m) / metres).round() as i32;
            let s = landscape::sample(cfg, x, z);
            grid.push((s.biome, s.ecotone.min(1.0 - s.ecotone)));
        }
    }

    let mut families: Vec<(Biome, u32)> = Vec::new();
    let mut ecotone_columns = 0;
    for &(biome, blend) in &grid {
        match families.iter_mut().find(|f| f.0 == biome) {
            Some(f) => f.1 += 1,
            None => families.push((biome, 1)),
        }
        ecotone_columns += (blend > ECOTONE_BLEND) as u32;
    }
    families.sort_by_key(|&(b, count)| (std::cmp::Reverse(count), b as u8));

    let mut borders: Vec<(Biome, Biome, f32)> = Vec::new();
    let mut edge = |a: Biome, b: Biome| {
        if a == b {
            return;
        }
        let (a, b) = if (a as u8) <= (b as u8) { (a, b) } else { (b, a) };
        match borders.iter_mut().find(|e| e.0 == a && e.1 == b) {
            Some(e) => e.2 += step_m,
            None => borders.push((a, b, step_m)),
        }
    };
    for j in 0..n {
        for i in 0..n {
            let here = grid[j * n + i].0;
            if i + 1 < n {
                edge(here, grid[j * n + i + 1].0);
            }
            if j + 1 < n {
                edge(here, grid[(j + 1) * n + i].0);
            }
        }
    }
    borders.sort_by(|a, b| {
        b.2.total_cmp(&a.2).then((a.0 as u8, a.1 as u8).cmp(&(b.0 as u8, b.1 as u8)))
    });

    BiomeReport { columns: grid.len() as u32, families, ecotone_columns, borders }
}

impl BiomeReport {
    /// Three-line text dump: totals, family counts, the longest borders in metres.
    pub fn to_text(&self) -> String {
        let join = |parts: Vec<String>| parts.join(", ");
        let share = 100.0 * self.ecotone_columns as f32 / self.columns.max(1) as f32;
        let total: f32 = self.borders.iter().map(|b| b.2).sum();
        format!(
            "columns {}, ecotone columns {} ({share:.1} %)\nfamilies: {}\nborders: {} pairs, {total:.0} m; longest (m): {}",
            self.columns,
            self.ecotone_columns,
            join(self.families.iter().map(|(b, n)| format!("{} {n}", b.label())).collect()),
            self.borders.len(),
            join(
                self.borders
                    .iter()
                    .take(TEXT_BORDERS)
                    .map(|(a, b, m)| format!("{}|{} {m:.0}", a.label(), b.label()))
                    .collect()
            ),
        )
    }
}
