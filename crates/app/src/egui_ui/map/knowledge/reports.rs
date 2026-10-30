//! Explicit clipboard exchange. Reports never grant a visit, route or localization.
use super::{Knowledge, Layer, MAX_LAYERS, MAX_PATCHES, Patch};
use glam::Vec3;
use serde::{Deserialize, Serialize};
const MAX_REPORT: usize = 192 * 1024;
const MAX_CELLS: usize = 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub author: String,
    pub received: u64,
    pub digest: u64,
}
impl Provenance {
    pub fn valid(&self) -> bool {
        self.author.len() <= 96 && !self.author.chars().any(char::is_control)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    author: String,
    label: String,
    scale: f32,
    up: [f32; 3],
    right: [f32; 3],
    forward: [f32; 3],
    patches: Vec<([i32; 3], [Option<[u8; 3]>; 6])>,
}

#[derive(Default)]
pub struct Exchange {
    author: String,
    input: String,
    message: &'static str,
}

impl Knowledge {
    fn export_report(&self) -> Option<String> {
        let id = self.selected.or(self.active)?;
        let layer = &self.saved.layers[id as usize];
        if layer.report.is_some() {
            return None;
        }
        let base = (Vec3::from_array(layer.origin) / self.scale)
            .floor()
            .as_ivec3();
        let patches: Vec<_> = self
            .saved
            .patches
            .pages
            .range((id, [i32::MIN; 3])..=(id, [i32::MAX; 3]))
            .flat_map(|(_, page)| page.iter())
            .filter(|(_, p)| p.faces.iter().any(Option::is_some))
            .take(MAX_CELLS)
            .map(|(cell, p)| ((glam::IVec3::from_array(*cell) - base).to_array(), p.faces))
            .collect();
        if patches.is_empty() {
            return None;
        }
        let report = Report {
            author: self
                .exchange
                .author
                .trim()
                .chars()
                .filter(|c| !c.is_control())
                .take(24)
                .collect(),
            label: self.chart_name(id).chars().take(24).collect(),
            scale: self.scale,
            up: layer.up,
            right: layer.right,
            forward: layer.forward,
            patches,
        };
        let body = ron::to_string(&report).ok()?;
        (body.len() < MAX_REPORT - 9).then(|| format!("VXREP001\n{body}"))
    }

    fn import_report(&mut self) -> Result<(), &'static str> {
        if self.loading.is_some() {
            return Err("Survey loading");
        }
        let text = self.exchange.input.trim();
        if text.len() > MAX_REPORT {
            return Err("Report too large");
        }
        let body = text.strip_prefix("VXREP001\n").ok_or("Invalid report")?;
        let report: Report = ron::from_str(body).map_err(|_| "Invalid report")?;
        if self.saved.layers.len() >= MAX_LAYERS
            || self.saved.patches.len() + report.patches.len() > MAX_PATCHES
        {
            return Err("Survey full");
        }
        if report.patches.is_empty()
            || report.patches.len() > MAX_CELLS
            || !report.scale.is_finite()
            || (report.scale - self.scale).abs() > 0.0001
            || report.author.len() > 96
            || report.label.len() > 96
            || report.label.chars().any(char::is_control)
            || report
                .patches
                .iter()
                .any(|(cell, _)| cell.iter().any(|v| !(-4_000_000..=4_000_000).contains(v)))
        {
            return Err("Invalid report");
        }
        let mut seen = std::collections::BTreeSet::new();
        if report
            .patches
            .iter()
            .any(|(cell, faces)| !seen.insert(*cell) || faces.iter().all(Option::is_none))
        {
            return Err("Invalid report");
        }
        let digest = body.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        if self
            .saved
            .layers
            .iter()
            .any(|l| l.report.as_ref().is_some_and(|p| p.digest == digest))
        {
            return Err("Already imported");
        }
        let received = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let layer = Layer {
            scope: self
                .saved
                .layers
                .iter()
                .map(|l| l.scope)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
            origin: [0.0; 3],
            up: report.up,
            right: report.right,
            forward: report.forward,
            report: Some(Provenance {
                author: report.author,
                received,
                digest,
            }),
        };
        if !layer.valid() {
            return Err("Invalid report");
        }
        let id = self.saved.layers.len() as u32;
        self.saved.layers.push(layer);
        self.saved.names.insert(id, report.label);
        for (cell, faces) in report.patches {
            self.saved.patches.insert(
                (id, cell),
                Patch {
                    faces,
                    ..Default::default()
                },
            );
        }
        self.selected = Some(id);
        self.dirty = true;
        Ok(())
    }

    pub(super) fn exchange_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Reports", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.exchange.author)
                    .hint_text("Author")
                    .char_limit(24),
            );
            if ui
                .button("Copy excerpt")
                .on_hover_text("Up to 1,024 observed cells")
                .clicked()
            {
                if let Some(text) = self.export_report() {
                    ui.ctx().copy_text(text);
                    self.exchange.message = "Copied";
                } else {
                    self.exchange.message = "No local observations";
                }
            }
            ui.add(
                egui::TextEdit::multiline(&mut self.exchange.input)
                    .hint_text("Report")
                    .desired_rows(2)
                    .char_limit(MAX_REPORT),
            );
            if ui.button("Import report").clicked() {
                self.exchange.message = match self.import_report() {
                    Ok(()) => "Imported",
                    Err(e) => e,
                };
            }
            if !self.exchange.message.is_empty() {
                ui.label(self.exchange.message);
            }
        });
        if let Some(report) = self.layer(false).and_then(|l| l.report.as_ref()) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            ui.label(format!(
                "Reported · {}",
                if report.author.is_empty() {
                    "Unknown"
                } else {
                    &report.author
                }
            ));
            ui.label(format!(
                "Received {} min ago",
                now.saturating_sub(report.received) / 60
            ));
        }
    }
}
