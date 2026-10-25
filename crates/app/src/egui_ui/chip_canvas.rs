//! The die as a site grid: modules, ports, wires per metal layer and vias.
//! [Ctrl]+wheel zooms, the wheel pans. Select drags a module, Connect joins
//! two ports ([RMB] on a port detaches it), Draw Wire lays a manual path for
//! the selected net on the current layer and [Enter] commits it.
use super::chip_editor::{Editor, Tool};
use super::icons;
use crate::world::factory::chips::registry;
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use engine_chip::compile::net_at;
use engine_chip::design::{Design, Pin, Site, rotate_local};
use engine_chip::registry::{Registry, Role, Signal};

const LAYER: [Color32; 3] = [Color32::from_rgb(214, 128, 64), Color32::from_rgb(70, 190, 180), Color32::from_rgb(170, 120, 230)];

fn role_fill(role: Role) -> Color32 {
    match role {
        Role::Core => Color32::from_rgb(72, 58, 96),
        Role::Alu => Color32::from_rgb(92, 62, 72),
        Role::Rom | Role::Sram => Color32::from_rgb(48, 70, 92),
        Role::Timer | Role::Clock | Role::Watchdog => Color32::from_rgb(70, 78, 48),
        Role::Gpio | Role::Adc | Role::Supply => Color32::from_rgb(88, 70, 44),
    }
}

fn signal_colour(s: Signal) -> Color32 {
    match s {
        Signal::Power => Color32::from_rgb(235, 90, 70),
        Signal::Clock => Color32::from_rgb(240, 220, 90),
        Signal::Reset => Color32::from_rgb(240, 150, 200),
        Signal::MemBus => Color32::from_rgb(110, 170, 255),
        Signal::PeriphBus => Color32::from_rgb(120, 230, 140),
        Signal::AluBus | Signal::Data => Color32::from_rgb(230, 140, 255),
    }
}

/// The port whose contact is at (x, y).
pub(super) fn port_at(d: &Design, r: &Registry, x: u8, y: u8) -> Option<Pin> {
    let part = d.part_at(r, x, y)?;
    let spec = r.module(&part.module)?;
    spec.ports
        .iter()
        .find(|p| rotate_local(spec, part.rot, p.at) == (x - part.x, y - part.y))
        .map(|p| Pin { part: part.id, port: p.name.clone() })
}

/// Module label on the die.
pub(super) fn short(role: Role) -> &'static str {
    match role {
        Role::Supply => "PWR",
        Role::Clock => "OSC",
        Role::Core => "CPU",
        Role::Alu => "ALU",
        Role::Rom => "ROM",
        Role::Sram => "RAM",
        Role::Timer => "TMR",
        Role::Gpio => "GPIO",
        Role::Adc => "ADC",
        Role::Watchdog => "WDT",
    }
}

pub(super) fn canvas(ui: &mut egui::Ui, ed: &mut Editor, max_height: f32) {
    let r = registry();
    let (w, h) = (f32::from(ed.design.width), f32::from(ed.design.height));
    let fit = ((ui.available_width() - 4.0) / w).min((max_height - 40.0) / h).clamp(8.0, 40.0);
    let cell = (fit * ed.zoom).clamp(6.0, 96.0);
    let scroll = egui::ScrollArea::both()
        .id_salt("chip_canvas")
        .max_height(max_height - 36.0)
        .auto_shrink([false, true])
        .show(ui, |ui| ui.allocate_painter(vec2(cell * w, cell * h), Sense::click_and_drag()));
    let (response, painter) = scroll.inner;
    ed.canvas = Some((response.rect.min, cell));
    if response.hovered() {
        let (zoom, wheel) = ui.input(|i| (i.modifiers.command, i.raw_scroll_delta.y));
        if zoom && wheel != 0.0 {
            ed.zoom = (ed.zoom * if wheel > 0.0 { 1.12 } else { 1.0 / 1.12 }).clamp(0.5, 4.0);
        }
    }
    let o = response.rect.min;
    let at = |x: f32, y: f32| o + vec2(x * cell, y * cell);
    let centre = |s: Site| at(f32::from(s.x) + 0.5, f32::from(s.y) + 0.5) + vec2(1.0, 1.0) * (f32::from(s.layer) * cell * 0.08);
    let hover = response.hover_pos().and_then(|p| {
        let (x, y) = ((p.x - o.x) / cell, (p.y - o.y) / cell);
        (x >= 0.0 && y >= 0.0 && x < w && y < h).then_some((x as u8, y as u8))
    });

    painter.rect_filled(response.rect, 0, Color32::from_rgb(16, 20, 19));
    let grid = Stroke::new(0.5, Color32::from_rgb(34, 40, 38));
    for i in 0..=ed.design.width {
        let x = o.x + f32::from(i) * cell;
        painter.line_segment([pos2(x, o.y), pos2(x, o.y + h * cell)], grid);
    }
    for j in 0..=ed.design.height {
        let y = o.y + f32::from(j) * cell;
        painter.line_segment([pos2(o.x, y), pos2(o.x + w * cell, y)], grid);
    }
    painter.rect_stroke(response.rect, 0, Stroke::new(1.5, icons::GOLD), StrokeKind::Inside);

    for part in &ed.design.parts {
        let (Some(spec), Some((x, y, pw, ph))) = (r.module(&part.module), ed.design.rect(r, part)) else { continue };
        let rect = Rect::from_min_max(at(f32::from(x), f32::from(y)), at(f32::from(x + pw), f32::from(y + ph))).shrink(1.5);
        painter.rect_filled(rect, 2, role_fill(spec.role));
        let selected = ed.selected == Some(part.id);
        let edge = if selected { Stroke::new(2.0, icons::PAPER) } else { Stroke::new(1.0, Color32::from_rgb(120, 110, 90)) };
        painter.rect_stroke(rect, 2, edge, StrokeKind::Inside);
        painter.text(rect.center(), Align2::CENTER_CENTER, short(spec.role), FontId::proportional((cell * 0.42).max(9.0)), icons::PAPER);
    }
    for net in &ed.design.nets {
        let picked = ed.net == Some(net.id);
        for pair in net.paths.iter().flat_map(|p| p.windows(2)) {
            let (a, b) = (pair[0], pair[1]);
            if a.layer != b.layer || ed.hidden[usize::from(a.layer).min(2)] {
                continue;
            }
            let colour = if picked { Color32::WHITE } else { LAYER[usize::from(a.layer).min(2)] };
            painter.line_segment([centre(a), centre(b)], Stroke::new(cell * if picked { 0.22 } else { 0.14 }, colour));
        }
    }
    for part in &ed.design.parts {
        let (Some(spec), Some((x, y, _, _))) = (r.module(&part.module), ed.design.rect(r, part)) else { continue };
        for port in &spec.ports {
            let (dx, dy) = rotate_local(spec, part.rot, port.at);
            let c = at(f32::from(x + dx) + 0.5, f32::from(y + dy) + 0.5);
            let colour = signal_colour(port.signal);
            if net_at(&ed.design, part.id, &port.name).is_some() {
                painter.circle_filled(c, cell * 0.2, colour);
            } else {
                painter.circle_stroke(c, cell * 0.2, Stroke::new(1.5, if port.optional { Color32::GRAY } else { colour }));
            }
            if ed.pin.as_ref().is_some_and(|p| p.part == part.id && p.port == port.name) {
                painter.circle_stroke(c, cell * 0.34, Stroke::new(2.0, icons::PAPER));
            }
        }
    }
    // Vias last: a hollow square where a wire changes layer.
    for pair in ed.design.nets.iter().flat_map(|n| n.paths.iter()).flat_map(|p| p.windows(2)) {
        if pair[0].layer != pair[1].layer {
            let c = at(f32::from(pair[0].x) + 0.5, f32::from(pair[0].y) + 0.5);
            painter.rect_stroke(Rect::from_center_size(c, vec2(cell * 0.5, cell * 0.5)), 0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Middle);
        }
    }
    for pair in ed.path.windows(2) {
        painter.line_segment([centre(pair[0]), centre(pair[1])], Stroke::new(cell * 0.16, Color32::WHITE.gamma_multiply(0.7)));
    }
    if let (Tool::Place(id), Some((x, y))) = (&ed.tool, hover)
        && let Some(spec) = r.module(id)
    {
        let (gw, gh) = engine_chip::design::rotated_size(spec, ed.rot);
        let ok = ed.design.clone().place(r, id, x, y, ed.rot).is_ok();
        let ghost = Rect::from_min_size(at(f32::from(x), f32::from(y)), vec2(f32::from(gw) * cell, f32::from(gh) * cell));
        let tint = if ok { icons::TEAL } else { Color32::from_rgb(235, 80, 60) };
        painter.rect_filled(ghost, 2, tint.gamma_multiply(0.35));
    }

    let tip = hover.and_then(|(x, y)| ed.design.part_at(r, x, y).cloned());
    interact(ed, r, &response, hover);
    if let Some(part) = tip
        && let Some(spec) = r.module(&part.module)
        && matches!(ed.tool, Tool::Select)
    {
        response.on_hover_ui(|ui| {
            ui.label(RichText::new(format!("{} #{}", spec.name, part.id)).color(icons::PAPER));
            ui.small(format!("{}×{} · {:.0} µW · {:.0} ns", spec.size.0, spec.size.1, spec.idle_uw, spec.delay_ns));
            if spec.bytes > 0 {
                ui.small(format!("{} B", spec.bytes));
            }
            if spec.edge {
                ui.small("Edge Pads");
            }
        });
    }
    status(ui, ed, r, hover);
}

fn interact(ed: &mut Editor, r: &Registry, response: &egui::Response, hover: Option<(u8, u8)>) {
    let Some((x, y)) = hover else { return };
    match ed.tool.clone() {
        Tool::Place(id) if response.clicked() => {
            let rot = ed.rot;
            ed.commit(|d, r| d.place(r, &id, x, y, rot).is_ok());
        }
        Tool::Select => {
            if response.drag_started() {
                ed.drag = ed.design.part_at(r, x, y).map(|p| (p.id, x - p.x, y - p.y));
                ed.selected = ed.drag.map(|d| d.0);
            }
            if response.drag_stopped()
                && let Some((id, gx, gy)) = ed.drag.take()
                && x >= gx
                && y >= gy
            {
                let (nx, ny) = (x - gx, y - gy);
                ed.commit(|d, r| d.part(id).cloned().is_some_and(|p| (p.x, p.y) != (nx, ny) && d.relocate(r, id, nx, ny, p.rot).is_ok()));
            }
            if response.clicked() {
                ed.selected = ed.design.part_at(r, x, y).map(|p| p.id);
                ed.net = ed
                    .design
                    .nets
                    .iter()
                    .find(|n| n.paths.iter().flatten().any(|s| (s.x, s.y, s.layer) == (x, y, ed.layer)))
                    .or_else(|| port_at(&ed.design, r, x, y).and_then(|p| ed.design.net_of(&p)))
                    .map(|n| n.id);
            }
        }
        Tool::Wire if response.secondary_clicked() => {
            if let Some(pin) = port_at(&ed.design, r, x, y) {
                ed.pin = None;
                ed.commit(|d, _| d.net_of(&pin).is_some() && {
                    d.disconnect(&pin);
                    true
                });
            }
        }
        Tool::Wire if response.clicked() => {
            let Some(pin) = port_at(&ed.design, r, x, y) else { return };
            match ed.pin.take() {
                None => ed.pin = Some(pin),
                Some(first) if first != pin => {
                    let mut joined = None;
                    ed.commit(|d, r| {
                        joined = d.connect(r, first, pin).ok();
                        joined.is_some()
                    });
                    ed.net = joined.or(ed.net);
                }
                Some(_) => {}
            }
        }
        Tool::Draw => {
            if response.clicked() {
                extend(&mut ed.path, Site::new(x, y, ed.layer));
            }
            let commit = response.ctx.input(|i| i.key_pressed(egui::Key::Enter));
            if commit && ed.path.len() >= 2
                && let Some(net) = ed.net
            {
                let path = std::mem::take(&mut ed.path);
                ed.commit(|d, _| {
                    let Some(n) = d.nets.iter().find(|n| n.id == net) else { return false };
                    let mut paths = if n.manual { n.paths.clone() } else { Vec::new() };
                    paths.push(path);
                    d.draw(net, paths);
                    true
                });
            }
        }
        _ => {}
    }
}

/// Append a site: straight runs fill in, a click on the last site on another
/// layer adds the via.
fn extend(path: &mut Vec<Site>, s: Site) {
    let Some(&last) = path.last() else {
        path.push(s);
        return;
    };
    if (last.x, last.y) == (s.x, s.y) {
        let mut l = last.layer;
        while l != s.layer {
            l = if s.layer > l { l + 1 } else { l - 1 };
            path.push(Site::new(s.x, s.y, l));
        }
        return;
    }
    let s = Site::new(s.x, s.y, last.layer);
    if last.x == s.x || last.y == s.y {
        let mut c = last;
        while c != s {
            c.x = if s.x > c.x { c.x + 1 } else if s.x < c.x { c.x - 1 } else { c.x };
            c.y = if s.y > c.y { c.y + 1 } else if s.y < c.y { c.y - 1 } else { c.y };
            path.push(c);
        }
    }
}

fn status(ui: &mut egui::Ui, ed: &Editor, r: &Registry, hover: Option<(u8, u8)>) {
    let mut line = String::new();
    if let Some((x, y)) = hover {
        line += &format!("{x}, {y} · M{}", ed.layer + 1);
        if let Some(pin) = port_at(&ed.design, r, x, y) {
            let spec = ed.design.part(pin.part).and_then(|p| r.module(&p.module));
            if let Some((spec, port)) = spec.and_then(|s| s.port(&pin.port).map(|p| (s, p))) {
                line += &format!(" · {} {} · {} · {} b", spec.name, port.name.to_uppercase(), port.signal.label(), port.width);
            }
        }
    }
    if let Some(n) = ed.net.and_then(|id| ed.design.nets.iter().find(|n| n.id == id)) {
        line += &format!(" · {} · {} Sites · {} Vias{}", n.name, n.length(), n.vias(), if n.manual { " · Manual" } else { "" });
    }
    ui.small(line);
    if ui.input(|i| i.modifiers.shift) {
        ui.small("[R] Rotate  [Del] Remove  [A] Route  [1-3] Layer  [Enter] Commit  [RMB] Detach  [Ctrl]+Wheel Zoom");
    }
}
