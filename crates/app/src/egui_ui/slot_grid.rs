//! Fixed-pitch slot grid shared by every container page.
//!
//! `egui::Grid` sizes columns from `min_col_width` (defaulting to
//! `interact_size.x`) and rows from `interact_size.y`, so small slots end up
//! with different horizontal and vertical gaps depending on the caller's
//! style. This grid owns its geometry instead: one allocation, one pitch on
//! both axes, and every edge snapped to whole physical pixels so neighbouring
//! cells never drift by a pixel.
use egui::{Id, Rect, Response, Sense, Ui, Vec2, pos2, vec2};

/// Gap between neighbouring cells, in points.
pub(crate) const GAP: f32 = 2.0;

/// Row-major grid of square cells of one size on one pitch.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SlotGrid {
    cols: usize,
    cell: f32,
    gap: f32,
    sense: Sense,
}

impl SlotGrid {
    pub(crate) fn new(cols: usize, cell: f32) -> Self {
        Self { cols: cols.max(1), cell, gap: GAP, sense: Sense::click() }
    }

    pub(crate) fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    pub(crate) fn sense(mut self, sense: Sense) -> Self {
        self.sense = sense;
        self
    }

    /// Columns that fit in `width` at this cell size, clamped to `1..=max`.
    pub(crate) fn columns_for(width: f32, cell: f32, max: usize) -> usize {
        (((width + GAP) / (cell + GAP)).floor() as usize).clamp(1, max.max(1))
    }

    /// Cell side and pitch snapped to whole physical pixels.
    fn metrics(&self, ppp: f32) -> (f32, f32) {
        let snap = |v: f32| ((v * ppp).round().max(1.0)) / ppp;
        let cell = snap(self.cell);
        (cell, cell + snap(self.gap.max(0.0)))
    }

    /// Outer size of a grid holding `count` cells.
    pub(crate) fn size(&self, count: usize, ppp: f32) -> Vec2 {
        let (cell, pitch) = self.metrics(ppp);
        let cols = self.cols.min(count.max(1));
        let rows = count.div_ceil(self.cols).max(1);
        vec2(pitch * (cols - 1) as f32 + cell, pitch * (rows - 1) as f32 + cell)
    }

    /// Lays out `count` cells and calls `cell(ui, index, rect, response)` for each.
    pub(crate) fn show(
        self,
        ui: &mut Ui,
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        count: usize,
        mut cell: impl FnMut(&mut Ui, usize, Rect, Response),
    ) -> Response {
        let ppp = ui.ctx().pixels_per_point();
        let (side, pitch) = self.metrics(ppp);
        let (outer, response) = ui.allocate_exact_size(self.size(count, ppp), Sense::hover());
        let origin = pos2((outer.min.x * ppp).round() / ppp, (outer.min.y * ppp).round() / ppp);
        let id = Id::new(("slot_grid", id_salt));
        for index in 0..count {
            let at = origin + vec2((index % self.cols) as f32 * pitch, (index / self.cols) as f32 * pitch);
            let rect = Rect::from_min_size(at, Vec2::splat(side));
            let hit = ui.interact(rect, id.with(index), self.sense);
            cell(ui, index, rect, hit);
        }
        response
    }
}
