//! Procedural drafting sheets redrawn stroke by stroke as loading advances.
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use engine_core::hash::{hash01, hash_u32};

pub(super) const GROUND: Color32 = Color32::from_rgb(17, 33, 43);
const GRID: Color32 = Color32::from_rgba_premultiplied(18, 33, 33, 34);
const GRID_MAJOR: Color32 = Color32::from_rgba_premultiplied(30, 55, 54, 56);
const INK: Color32 = Color32::from_rgb(214, 226, 222);
const FADED: Color32 = Color32::from_rgba_premultiplied(62, 72, 70, 84);
const DIM: Color32 = super::pixel_art::TEAL;
const PEN: Color32 = super::pixel_art::GOLD;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Outline,
    Hidden,
    Center,
    Dimension,
}

#[derive(Clone, Copy)]
struct Line {
    a: Pos2,
    b: Pos2,
    kind: Kind,
}

/// Strokes of one view, in the unit square, y down.
#[derive(Default)]
struct View {
    lines: Vec<Line>,
}

impl View {
    fn line(&mut self, a: (f32, f32), b: (f32, f32), kind: Kind) {
        self.lines.push(Line { a: pos2(a.0, a.1), b: pos2(b.0, b.1), kind });
    }
    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, kind: Kind) {
        self.line((x0, y0), (x1, y0), kind);
        self.line((x1, y0), (x1, y1), kind);
        self.line((x1, y1), (x0, y1), kind);
        self.line((x0, y1), (x0, y0), kind);
    }
    fn circle(&mut self, c: (f32, f32), r: f32, segments: u32, kind: Kind) {
        let at = |i: u32| {
            let t = i as f32 / segments as f32 * std::f32::consts::TAU;
            (c.0 + r * t.cos(), c.1 + r * t.sin())
        };
        for i in 0..segments {
            self.line(at(i), at(i + 1), kind);
        }
    }
    fn dimension(&mut self, a: (f32, f32), b: (f32, f32)) {
        self.line(a, b, Kind::Dimension);
        // Ticks perpendicular to the dimension line.
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-4);
        let (nx, ny) = (-dy / len * 0.02, dx / len * 0.02);
        for p in [a, b] {
            self.line((p.0 - nx, p.1 - ny), (p.0 + nx, p.1 + ny), Kind::Dimension);
        }
    }
}

struct Seed(u32);

impl Seed {
    fn at(&self, i: u32) -> f32 {
        hash01(self.0 ^ hash_u32(i.wrapping_mul(0x9e37_79b9)))
    }
    fn range(&self, i: u32, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.at(i)
    }
    fn count(&self, i: u32, lo: u32, hi: u32) -> u32 {
        lo + (self.at(i) * (hi - lo + 1) as f32) as u32 % (hi - lo + 1)
    }
}

/// Side elevation of a flanged casing with bore, bolts and through pipe.
fn casing(s: &Seed) -> View {
    let mut v = View::default();
    let (hw, hh) = (s.range(1, 0.22, 0.32), s.range(2, 0.14, 0.22));
    let (x0, x1, y0, y1) = (0.5 - hw, 0.5 + hw, 0.5 - hh, 0.5 + hh);
    v.line((0.04, 0.5), (0.96, 0.5), Kind::Center);
    v.rect(x0, y0, x1, y1, Kind::Outline);
    let flange = s.range(3, 0.03, 0.05);
    let fh = hh + s.range(4, 0.04, 0.08);
    for (a, b) in [(x0 - flange, x0), (x1, x1 + flange)] {
        v.rect(a, 0.5 - fh, b, 0.5 + fh, Kind::Outline);
        for y in [0.5 - fh + 0.025, 0.5 + fh - 0.025] {
            v.circle(((a + b) * 0.5, y), 0.009, 6, Kind::Outline);
        }
    }
    let pipe = s.range(5, 0.04, 0.07);
    for (a, b) in [(0.06, x0 - flange), (x1 + flange, 0.94)] {
        v.line((a, 0.5 - pipe), (b, 0.5 - pipe), Kind::Outline);
        v.line((a, 0.5 + pipe), (b, 0.5 + pipe), Kind::Outline);
    }
    let bore = pipe * s.range(6, 1.3, 1.9);
    v.rect(x0 + 0.03, 0.5 - bore, x1 - 0.03, 0.5 + bore, Kind::Hidden);
    for i in 0..s.count(7, 2, 5) {
        let x = x0 + (i as f32 + 1.0) * (x1 - x0) / (s.count(7, 2, 5) as f32 + 1.0);
        v.line((x, y0), (x, y0 - 0.04), Kind::Outline);
        v.line((x - 0.015, y0 - 0.04), (x + 0.015, y0 - 0.04), Kind::Outline);
    }
    v.dimension((x0, y1 + 0.1), (x1, y1 + 0.1));
    v.dimension((x1 + flange + 0.08, 0.5 - fh), (x1 + flange + 0.08, 0.5 + fh));
    v
}

/// Plan view of a toothed rotor with hub, spokes and bore.
fn rotor(s: &Seed) -> View {
    let mut v = View::default();
    let c = (0.5, 0.5);
    let r = s.range(11, 0.30, 0.38);
    v.line((0.06, 0.5), (0.94, 0.5), Kind::Center);
    v.line((0.5, 0.06), (0.5, 0.94), Kind::Center);
    v.circle(c, r, 40, Kind::Outline);
    let teeth = s.count(12, 8, 16);
    let depth = s.range(13, 0.03, 0.06);
    for i in 0..teeth {
        for half in [-0.3_f32, 0.3] {
            let t = (i as f32 + half * 0.5) / teeth as f32 * std::f32::consts::TAU;
            let (cx, cy) = (t.cos(), t.sin());
            v.line((0.5 + cx * r, 0.5 + cy * r), (0.5 + cx * (r + depth), 0.5 + cy * (r + depth)), Kind::Outline);
        }
    }
    let hub = s.range(14, 0.08, 0.13);
    v.circle(c, hub, 20, Kind::Outline);
    v.circle(c, hub * 0.4, 12, Kind::Outline);
    let spokes = s.count(15, 3, 6);
    for i in 0..spokes {
        let t = (i as f32 + 0.5) / spokes as f32 * std::f32::consts::TAU;
        let (cx, cy) = (t.cos(), t.sin());
        v.line((0.5 + cx * hub, 0.5 + cy * hub), (0.5 + cx * (r - 0.03), 0.5 + cy * (r - 0.03)), Kind::Hidden);
    }
    v.circle(c, r - 0.03, 40, Kind::Hidden);
    v.dimension((0.5 - r, 0.5 + r + depth + 0.06), (0.5 + r, 0.5 + r + depth + 0.06));
    v
}

/// Floorplan of a die: pin rows, placed blocks and routed traces.
fn die(s: &Seed) -> View {
    let mut v = View::default();
    let h = s.range(21, 0.24, 0.31);
    let (x0, x1) = (0.5 - h, 0.5 + h);
    v.rect(x0, x0, x1, x1, Kind::Outline);
    let pins = s.count(22, 6, 12);
    for i in 0..pins {
        let t = x0 + (i as f32 + 0.5) * (2.0 * h) / pins as f32;
        v.line((t, x0), (t, x0 - 0.05), Kind::Outline);
        v.line((t, x1), (t, x1 + 0.05), Kind::Outline);
        v.line((x0, t), (x0 - 0.05, t), Kind::Outline);
        v.line((x1, t), (x1 + 0.05, t), Kind::Outline);
    }
    let cells = s.count(23, 3, 4);
    let step = 2.0 * (h - 0.04) / cells as f32;
    let mut placed = Vec::new();
    for gy in 0..cells {
        for gx in 0..cells {
            if s.at(30 + gy * 8 + gx) < 0.35 {
                continue;
            }
            let (bx, by) = (x0 + 0.04 + gx as f32 * step, x0 + 0.04 + gy as f32 * step);
            v.rect(bx + 0.012, by + 0.012, bx + step - 0.012, by + step - 0.012, Kind::Outline);
            placed.push((bx + step * 0.5, by + step * 0.5));
        }
    }
    for pair in placed.windows(2) {
        let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
        v.line((ax, ay), (bx, ay), Kind::Hidden);
        v.line((bx, ay), (bx, by), Kind::Hidden);
    }
    v.dimension((x0, x1 + 0.12), (x1, x1 + 0.12));
    v
}

/// Two views of one recovered part; the stage title picks the part.
fn sheet(seed: u32) -> [View; 2] {
    let s = Seed(seed);
    let makers = [casing, rotor, die];
    let first = (s.at(0) * 3.0) as usize % 3;
    let second = (first + 1 + (s.at(9) * 2.0) as usize % 2) % 3;
    [makers[first](&s), makers[second](&Seed(seed ^ 0x5bd1_e995))]
}

pub(super) fn seed_of(title: &str) -> u32 {
    title.bytes().fold(0x811c_9dc5_u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// Paints grid and border; returns the inner sheet rect.
pub(super) fn ground(p: &Painter, r: Rect) -> Rect {
    p.rect_filled(r, 0, GROUND);
    let mut x = (r.left() / 8.0).ceil() * 8.0;
    while x < r.right() {
        let c = if (x as i32).rem_euclid(40) == 0 { GRID_MAJOR } else { GRID };
        p.vline(x, r.y_range(), Stroke::new(1.0, c));
        x += 8.0;
    }
    let mut y = (r.top() / 8.0).ceil() * 8.0;
    while y < r.bottom() {
        let c = if (y as i32).rem_euclid(40) == 0 { GRID_MAJOR } else { GRID };
        p.hline(r.x_range(), y, Stroke::new(1.0, c));
        y += 8.0;
    }
    let inner = r.shrink(24.0);
    let sheet = Rect::from_min_max(inner.min.round(), inner.max.round());
    p.rect_stroke(sheet, 0, Stroke::new(1.0, INK), egui::StrokeKind::Inside);
    p.rect_stroke(sheet.shrink(4.0), 0, Stroke::new(1.0, DIM), egui::StrokeKind::Inside);
    sheet
}

/// Draws both views into `area`; strokes past `reveal` show only as faded fragments.
pub(super) fn draw(p: &Painter, area: Rect, seed: u32, reveal: f32) {
    let views = sheet(seed);
    let total: usize = views.iter().map(|v| v.lines.len()).sum();
    let done = reveal.clamp(0.0, 1.0) * total as f32;
    let half = area.width() * 0.5;
    let mut index = 0usize;
    for (n, view) in views.iter().enumerate() {
        let cell = Rect::from_min_size(area.min + vec2(half * n as f32, 0.0), vec2(half, area.height()));
        let side = cell.width().min(cell.height()) - 16.0;
        if side <= 0.0 {
            return;
        }
        let frame = Rect::from_center_size(cell.center(), vec2(side, side));
        let map = |q: Pos2| (frame.min + q.to_vec2() * side).round();
        for line in &view.lines {
            let (a, b) = (map(line.a), map(line.b));
            let progress = done - index as f32;
            if progress >= 1.0 {
                stroke(p, a, b, line.kind, colour(line.kind));
            } else if progress > 0.0 {
                let tip = a + (b - a) * progress;
                stroke(p, a, tip, line.kind, colour(line.kind));
                p.rect_filled(Rect::from_center_size(tip.round(), vec2(3.0, 3.0)), 0, PEN);
            } else if hash01(seed ^ hash_u32(index as u32)) < 0.55 {
                // Gaps in the faded copy read as a damaged original.
                stroke(p, a, b, line.kind, FADED);
            }
            index += 1;
        }
    }
}

fn colour(kind: Kind) -> Color32 {
    if kind == Kind::Dimension { DIM } else { INK }
}

fn stroke(p: &Painter, a: Pos2, b: Pos2, kind: Kind, colour: Color32) {
    let s = Stroke::new(1.0, colour);
    match kind {
        Kind::Outline | Kind::Dimension => {
            p.line_segment([a, b], s);
        }
        Kind::Hidden => {
            p.extend(Shape::dashed_line(&[a, b], s, 5.0, 3.0));
        }
        Kind::Center => {
            p.extend(Shape::dashed_line_with_offset(&[a, b], s, &[12.0, 3.0], &[3.0, 3.0], 0.0));
        }
    }
}
