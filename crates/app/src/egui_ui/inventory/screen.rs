//! Vanilla inventory geometry and its panel, slot and arrow painters.
//!
//! Positions are in GUI units: one unit is one texel of the vanilla
//! 176 × 166 container texture, and every slot sits on the 18-unit pitch.
//! GUI units use whole physical pixels, including on scaled Windows displays.
use egui::{Color32 as C, Painter, Pos2, Rect, pos2, vec2};

pub(in crate::egui_ui) const WIDTH: f32 = 176.0;
pub(in crate::egui_ui) const HEIGHT: f32 = 166.0;
pub(in crate::egui_ui) const SLOT: f32 = 18.0;

const PANEL: C = C::from_rgb(0x33, 0x2a, 0x26);
const PANEL_LIGHT: C = C::from_rgb(0x4d, 0x3f, 0x35);
const PANEL_DARK: C = C::from_rgb(0x1f, 0x19, 0x17);
const OUTLINE: C = C::from_rgb(0x0b, 0x09, 0x08);
const WELL: C = C::from_rgb(0x1b, 0x17, 0x16);
const WELL_DARK: C = C::from_rgb(0x0e, 0x0c, 0x0b);
const WELL_LIGHT: C = C::from_rgb(0x5a, 0x4a, 0x3d);
pub(in crate::egui_ui) const LABEL: C = C::from_rgb(0xb8, 0xa8, 0x90);
const ARROW_IDLE: C = C::from_rgb(0x5a, 0x4c, 0x40);

/// Unit positions of one screen: survival (2 × 2, armour, figure) or bench (3 × 3).
pub(in crate::egui_ui) struct Layout {
    pub crafting_label: Pos2,
    pub grid: Pos2,
    pub grid_cols: usize,
    pub arrow: Rect,
    /// Outer frame of the output slot; 26 units on the bench, 18 in the pack.
    pub output: Rect,
    pub armour: Option<Pos2>,
    pub figure: Option<Rect>,
    pub inventory_label: Pos2,
    pub pack: Pos2,
    pub hotbar: Pos2,
}

impl Layout {
    pub fn new(bench: bool) -> Self {
        let shared = |crafting_label, grid, grid_cols, arrow, output, armour, figure| Layout {
            crafting_label,
            grid,
            grid_cols,
            arrow,
            output,
            armour,
            figure,
            inventory_label: pos2(8.0, 73.0),
            pack: pos2(7.0, 83.0),
            hotbar: pos2(7.0, 141.0),
        };
        if bench {
            shared(
                pos2(29.0, 6.0),
                pos2(29.0, 16.0),
                3,
                Rect::from_min_size(pos2(89.0, 35.0), vec2(22.0, 15.0)),
                Rect::from_min_size(pos2(119.0, 30.0), vec2(26.0, 26.0)),
                None,
                None,
            )
        } else {
            shared(
                pos2(97.0, 6.0),
                pos2(97.0, 17.0),
                2,
                Rect::from_min_size(pos2(135.0, 29.0), vec2(15.0, 11.0)),
                Rect::from_min_size(pos2(152.0, 26.0), vec2(SLOT, SLOT)),
                Some(pos2(7.0, 7.0)),
                Some(Rect::from_min_size(pos2(25.0, 7.0), vec2(51.0, 72.0))),
            )
        }
    }
}

/// Maps unit coordinates of one panel onto the screen.
#[derive(Clone, Copy)]
pub(in crate::egui_ui) struct Frame {
    pub origin: Pos2,
    pub unit: f32,
}

impl Frame {
    pub fn pos(self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.unit
    }

    pub fn rect(self, r: Rect) -> Rect {
        Rect::from_min_max(self.pos(r.min), self.pos(r.max))
    }

    /// Slot `index` of a row-major block of `cols` columns starting at `at`.
    pub fn slot(self, at: Pos2, cols: usize, index: usize) -> Rect {
        let cell = pos2(at.x + (index % cols) as f32 * SLOT, at.y + (index / cols) as f32 * SLOT);
        self.rect(Rect::from_min_size(cell, vec2(SLOT, SLOT)))
    }
}

/// Points per GUI unit: the largest step that fits, with room for readable three-point units on desktop.
pub(in crate::egui_ui) fn unit_for(available: egui::Vec2, pixels_per_point: f32) -> f32 {
    let ppp = pixels_per_point.max(0.5);
    let wanted = (3.0 * ppp).floor().max(1.0);
    let fit = (available.x * ppp / WIDTH).min(available.y * ppp / HEIGHT).floor().max(1.0);
    wanted.min(fit) / ppp
}

/// One `u`-thick band on each side: `light` on top and left, `dark` on the
/// bottom and right. Swapping the two turns a raised edge into a well.
fn bevel(p: &Painter, r: Rect, u: f32, light: C, dark: C) {
    p.rect_filled(Rect::from_min_max(r.min, pos2(r.right() - u, r.top() + u)), 0, light);
    p.rect_filled(Rect::from_min_max(r.min, pos2(r.left() + u, r.bottom() - u)), 0, light);
    p.rect_filled(Rect::from_min_max(pos2(r.left() + u, r.bottom() - u), r.max), 0, dark);
    p.rect_filled(Rect::from_min_max(pos2(r.right() - u, r.top() + u), r.max), 0, dark);
}

/// Flat container body with a restrained outline.
pub(in crate::egui_ui) fn panel(p: &Painter, r: Rect, u: f32) {
    if super::super::themes::get(p.ctx()) != 0 {
        let theme = super::super::themes::current(p.ctx());
        p.rect_filled(r, 0, OUTLINE);
        let body = r.shrink(u);
        p.rect_filled(body, 0, theme.panel);
        p.rect_stroke(body, 0, egui::Stroke::new(1.0, theme.edge.gamma_multiply(0.65)), egui::StrokeKind::Inside);
        return;
    }
    p.rect_filled(r, 0, OUTLINE);
    let body = r.shrink(u);
    p.rect_filled(body, 0, PANEL);
    p.rect_stroke(body, 0, egui::Stroke::new(1.0, PANEL_LIGHT), egui::StrokeKind::Inside);
}

/// Separate, fully outlined cells with a small gutter on every side.
pub(in crate::egui_ui) fn well(p: &Painter, r: Rect, u: f32) {
    let r = r.shrink(u * 0.5);
    if super::super::themes::get(p.ctx()) != 0 {
        let theme = super::super::themes::current(p.ctx());
        p.rect_filled(r, 0, theme.recess);
        p.rect_stroke(r, 0, egui::Stroke::new(1.0, theme.edge.gamma_multiply(0.55)), egui::StrokeKind::Inside);
        return;
    }
    p.rect_filled(r, 0, WELL);
    p.rect_stroke(r, 0, egui::Stroke::new(1.0, WELL_LIGHT), egui::StrokeKind::Inside);
}

/// A catalogue cell on a gapless grid: the vanilla recessed bevel, one
/// physical pixel thick, so neighbours share a dark/light seam instead of a gutter.
pub(in crate::egui_ui) fn cell(p: &Painter, r: Rect) {
    let ppp = p.ctx().pixels_per_point().max(0.5);
    let snap = |v: f32| (v * ppp).round() / ppp;
    let r = Rect::from_min_max(pos2(snap(r.left()), snap(r.top())), pos2(snap(r.right()), snap(r.bottom())));
    let u = 1.0 / ppp;
    if super::super::themes::get(p.ctx()) != 0 {
        let theme = super::super::themes::current(p.ctx());
        p.rect_filled(r, 0, theme.recess);
        bevel(p, r, u, theme.recess.gamma_multiply(0.5), theme.edge.gamma_multiply(0.45));
        return;
    }
    p.rect_filled(r, 0, WELL);
    bevel(p, r, u, WELL_DARK, WELL_LIGHT);
}

/// Gold edge for the selected hotbar slot, inside the well's bevel.
pub(in crate::egui_ui) fn selected(p: &Painter, r: Rect, u: f32) {
    p.rect_stroke(r.shrink(u * 0.5), 0, egui::Stroke::new(2.0, crate::egui_ui::grimoire::GOLD), egui::StrokeKind::Inside);
}

/// Vanilla hover: a pale wash over the 16-unit item area.
pub(in crate::egui_ui) fn hover(p: &Painter, r: Rect, u: f32) {
    p.rect_filled(r.shrink(u), 0, C::from_white_alpha(56));
}

/// The crafting arrow: a shaft and a stepped head, lit while a recipe matches.
pub(in crate::egui_ui) fn arrow(p: &Painter, r: Rect, u: f32, lit: bool) {
    let colour = if lit { crate::egui_ui::presentation::AMBER } else { ARROW_IDLE };
    let rows = (r.height() / u).round();
    let half = (rows / 2.0).floor();
    let head = half + 1.0;
    let cols = (r.width() / u).round();
    let shaft = cols - head;
    let mid = r.top() + half * u;
    // Odd so the shaft centres on the middle row.
    let thick = ((rows / 5.0).round() / 2.0).floor() * 2.0 + 1.0;
    let shaft_top = mid - ((thick - 1.0) / 2.0).floor() * u;
    p.rect_filled(
        Rect::from_min_size(pos2(r.left(), shaft_top), vec2(shaft * u, thick * u)),
        0,
        colour,
    );
    for k in 0..head as usize {
        let reach = half - k as f32;
        let x = r.left() + (shaft + k as f32) * u;
        p.rect_filled(
            Rect::from_min_max(pos2(x, mid - reach * u), pos2(x + u, mid + (reach + 1.0) * u)),
            0,
            colour,
        );
    }
}

/// A page tab: raised like the panel when selected, recessed otherwise.
pub(in crate::egui_ui) fn tab(p: &Painter, r: Rect, u: f32, selected: bool) {
    p.rect_filled(r, 0, OUTLINE);
    let body = r.shrink(u);
    if selected {
        p.rect_filled(body, 0, PANEL);
        bevel(p, body, u, PANEL_LIGHT, PANEL_DARK);
    } else {
        p.rect_filled(body, 0, WELL);
        bevel(p, body, u, PANEL_DARK, WELL_DARK);
    }
}

/// Unit-grid rectangles `(x, y, w, h)` from `at`, in one colour.
fn cells(p: &Painter, at: Pos2, u: f32, colour: C, cells: &[(f32, f32, f32, f32)]) {
    for &(x, y, w, h) in cells {
        p.rect_filled(Rect::from_min_size(at + vec2(x, y) * u, vec2(w, h) * u), 0, colour);
    }
}

fn glyph(p: &Painter, at: Pos2, u: f32, rects: &[(f32, f32, f32, f32)]) {
    cells(p, at, u, LABEL, rects);
}

/// A closed book in a 10-unit square: the recipe browser's tab.
pub(in crate::egui_ui) fn book_glyph(p: &Painter, at: Pos2, u: f32) {
    cells(p, at, u, C::from_rgb(0x7a, 0x2e, 0x24), &[(1.0, 1.0, 8.0, 8.0)]);
    cells(p, at, u, C::from_rgb(0x4a, 0x1a, 0x14), &[(1.0, 1.0, 1.0, 8.0)]);
    cells(p, at, u, C::from_rgb(0xe8, 0xdc, 0xc0), &[(2.0, 8.0, 7.0, 1.0)]);
    cells(p, at, u, crate::egui_ui::grimoire::GOLD, &[(4.0, 3.0, 4.0, 1.0), (4.0, 5.0, 3.0, 1.0)]);
}

/// Distinct navigation symbols; unrelated pages must not share a resource icon.
pub(in crate::egui_ui) fn orbit_glyph(p: &Painter, at: Pos2, u: f32) {
    glyph(p, at, u, &[(3.0, 1.0, 5.0, 1.0), (1.0, 3.0, 1.0, 5.0),
        (3.0, 9.0, 5.0, 1.0), (9.0, 4.0, 1.0, 4.0),
        (4.0, 4.0, 3.0, 3.0)]);
    cells(p, at, u, crate::egui_ui::grimoire::GOLD, &[(8.0, 1.0, 3.0, 3.0)]);
}

pub(in crate::egui_ui) fn chart_glyph(p: &Painter, at: Pos2, u: f32) {
    glyph(p, at, u, &[(0.0, 9.0, 11.0, 1.0), (1.0, 6.0, 2.0, 3.0),
        (4.0, 4.0, 2.0, 5.0)]);
    cells(p, at, u, crate::egui_ui::grimoire::GOLD, &[(7.0, 1.0, 2.0, 8.0)]);
}

/// Globe: rim, equator and meridian, with gold land.
pub(in crate::egui_ui) fn globe_glyph(p: &Painter, at: Pos2, u: f32) {
    glyph(p, at, u, &[(3.0, 0.0, 5.0, 1.0), (1.0, 1.0, 2.0, 1.0), (8.0, 1.0, 2.0, 1.0),
        (1.0, 2.0, 1.0, 1.0), (9.0, 2.0, 1.0, 1.0), (0.0, 3.0, 1.0, 5.0), (10.0, 3.0, 1.0, 5.0),
        (1.0, 8.0, 1.0, 1.0), (9.0, 8.0, 1.0, 1.0), (1.0, 9.0, 2.0, 1.0), (8.0, 9.0, 2.0, 1.0),
        (3.0, 10.0, 5.0, 1.0), (5.0, 1.0, 1.0, 9.0), (1.0, 5.0, 9.0, 1.0)]);
    cells(p, at, u, crate::egui_ui::grimoire::GOLD, &[(2.0, 3.0, 2.0, 2.0), (6.0, 6.0, 3.0, 2.0), (7.0, 2.0, 1.0, 2.0)]);
}

/// Three bars, longest first.
pub(in crate::egui_ui) fn sort_glyph(p: &Painter, at: Pos2, u: f32) {
    glyph(p, at, u, &[(2.0, 2.0, 5.0, 1.0), (2.0, 4.0, 4.0, 1.0), (2.0, 6.0, 3.0, 1.0)]);
}

/// A down arrow: pack stacks drop to the hotbar below.
pub(in crate::egui_ui) fn transfer_glyph(p: &Painter, at: Pos2, u: f32) {
    glyph(p, at, u, &[(4.0, 2.0, 1.0, 2.0), (2.0, 4.0, 5.0, 1.0), (3.0, 5.0, 3.0, 1.0), (4.0, 6.0, 1.0, 1.0)]);
}
