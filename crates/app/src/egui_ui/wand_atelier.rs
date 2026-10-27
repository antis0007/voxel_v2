//! The wand atelier: pick what the wand is made of, what crowns it, how it glows
//! and what it sheds. A live schematic shows the result; the first-person wand
//! changes the moment a choice is made. Choices persist beside the saves.
use crate::wand_style::{self, GLOWS, GRIPS, HEADS, SHAFTS, TRAILS, WandStyle};
use egui::{Align2, Color32, FontId, Sense, Stroke, vec2};

fn rgb(c: [f32; 3]) -> Color32 {
    Color32::from_rgb(
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// One row: a label, previous/next arrows and the current name with a swatch.
fn picker(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u8,
    names: &[&str],
    swatch: &dyn Fn(u8) -> Option<Color32>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            [84.0, 22.0],
            egui::Label::new(egui::RichText::new(label).size(12.0).weak()),
        );
        if ui
            .add(egui::Button::new("◀").min_size(vec2(24.0, 22.0)))
            .clicked()
        {
            *value = (*value + names.len() as u8 - 1) % names.len() as u8;
            changed = true;
        }
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        if let Some(c) = swatch(*value) {
            // Square, native-pixel swatches match the rest of the interface.
            ui.painter().rect_filled(rect, 0, c);
            ui.painter().rect_stroke(
                rect,
                0,
                Stroke::new(1.0, Color32::from_black_alpha(160)),
                egui::StrokeKind::Inside,
            );
        }
        ui.add_sized(
            [128.0, 22.0],
            egui::Label::new(egui::RichText::new(names[*value as usize]).size(13.0)),
        );
        if ui
            .add(egui::Button::new("▶").min_size(vec2(24.0, 22.0)))
            .clicked()
        {
            *value = (*value + 1) % names.len() as u8;
            changed = true;
        }
    });
    changed
}

fn draw_preview(ui: &mut egui::Ui, style: WandStyle, time: f32) {
    let (response, painter) = ui.allocate_painter(vec2(220.0, 270.0), Sense::hover());
    let r = response.rect;
    painter.rect_filled(r, 0, Color32::from_rgb(10, 12, 24));
    // Pixel checker backdrop plus square stars. The old rounded panel and
    // antialiased circles read as a web card next to the voxel interface.
    for y in 0..(r.height() / 12.0) as i32 {
        for x in 0..(r.width() / 12.0) as i32 {
            if (x + y) % 2 == 0 {
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        r.min + vec2(x as f32 * 12.0, y as f32 * 12.0),
                        vec2(12.0, 12.0),
                    ),
                    0,
                    Color32::from_rgba_unmultiplied(18, 21, 38, 90),
                );
            }
        }
    }
    for i in 0..26 {
        let h = (i as f32 * 12.9898).sin() * 43758.545;
        let (fx, fy) = (h.fract().abs(), (h * 1.7).fract().abs());
        let tw = 0.5 + 0.5 * (time * (1.0 + fx * 2.0) + i as f32).sin();
        let star = egui::Rect::from_min_size(
            r.min + vec2(fx * r.width(), fy * r.height()),
            vec2(1.0 + tw, 1.0 + tw),
        );
        painter.rect_filled(
            star,
            0,
            Color32::from_white_alpha((60.0 + 120.0 * tw) as u8),
        );
    }
    let shaft = &SHAFTS[style.shaft as usize];
    let grip_col = rgb(GRIPS[style.grip as usize].1);
    // Spell-colour glow cycles through a few bolts so you can see it follow.
    let cycle = [
        crate::world::adventure::PALETTE_OLIVE,
        crate::world::adventure::PALETTE_MANA,
        crate::world::adventure::PALETTE_PLUM,
        crate::world::adventure::PALETTE_COPPER,
    ];
    let spell = cycle[((time * 0.4) as usize) % cycle.len()];
    let glow = rgb(style.glow_colour(spell));
    let base = r.center_bottom() + vec2(-46.0, -26.0);
    let tip = r.center_top() + vec2(38.0, 74.0);
    let axis = (tip - base).normalized();
    let normal = vec2(-axis.y, axis.x);
    let at = |t: f32| base + (tip - base) * t;
    let width = 11.0;
    painter.line_segment([base, tip], Stroke::new(width, rgb(shaft.base)));
    painter.line_segment(
        [at(0.0) + normal * 2.0, at(0.97) + normal * 2.0],
        Stroke::new(2.5, alpha(rgb(shaft.accent), 120)),
    );
    if style.shaft == 2 {
        painter.line_segment([base, tip], Stroke::new(4.0, alpha(rgb(shaft.accent), 230)));
    }
    for i in 0..5 {
        let c = at(i as f32 * 0.055 + 0.02);
        painter.line_segment(
            [c - normal * 6.5, c + normal * 6.5],
            Stroke::new(
                4.0,
                if i % 2 == 0 {
                    grip_col
                } else {
                    alpha(grip_col, 170)
                },
            ),
        );
    }
    if style.runes > 0 {
        let lit = 0.5 + 0.5 * (time * 0.8).sin();
        for i in 0..4 {
            let on = ((lit * 4.0) - i as f32).clamp(0.0, 1.0);
            let c = at(0.30 + 0.14 * i as f32) + normal * 4.0;
            painter.circle_filled(c, 2.6, alpha(glow, (50.0 + 205.0 * on) as u8));
        }
    }
    painter.line_segment(
        [at(0.90) - normal * 7.0, at(0.90) + normal * 7.0],
        Stroke::new(5.0, rgb(shaft.accent)),
    );
    let crown = tip + axis * 12.0;
    let pulse = 0.5 + 0.5 * (time * 2.4).sin();
    let halo = |radius: f32, a: u8| painter.circle_filled(crown, radius, alpha(glow, a));
    halo(34.0 + pulse * 4.0, 22);
    halo(22.0 + pulse * 3.0, 46);
    match style.head {
        0 => {
            painter.circle_filled(crown, 11.0, glow);
            painter.circle_filled(crown - vec2(3.0, 3.0), 4.0, Color32::from_white_alpha(200));
            for k in 0..3 {
                let a = 0.3 + k as f32 * 2.094;
                painter.line_segment(
                    [
                        tip + (normal * a.cos()) * 7.0,
                        crown + normal * a.cos() * 9.0,
                    ],
                    Stroke::new(2.0, rgb(shaft.accent)),
                );
            }
        }
        1 => {
            let pts = vec![
                crown + axis * 22.0,
                crown + normal * 8.0,
                crown - axis * 10.0,
                crown - normal * 8.0,
            ];
            painter.add(egui::Shape::convex_polygon(
                pts,
                alpha(glow, 215),
                Stroke::new(1.5, Color32::from_white_alpha(180)),
            ));
        }
        2 => {
            for k in 0..3 {
                let a = time * 1.4 + k as f32 * 2.094;
                painter.circle_filled(crown + vec2(a.cos() * 14.0, a.sin() * 5.0), 5.5, glow);
            }
            painter.circle_filled(crown, 3.5, Color32::WHITE);
        }
        3 => {
            painter.circle_filled(crown, 6.0, glow);
            for k in 0..5 {
                let a = time * 0.55 + k as f32 * 1.256;
                painter.circle_filled(crown + vec2(a.cos() * 18.0, a.sin() * 7.0), 2.2, glow);
            }
        }
        4 => {
            for i in 0..7 {
                let h = i as f32 * 5.5;
                let wob = (time * 1.3 + i as f32 * 0.7).sin() * 0.8 * i as f32 * 0.3;
                painter.circle_filled(
                    tip + axis * (6.0 + h) + normal * wob,
                    9.0 * (1.0 - i as f32 * 0.12),
                    alpha(glow, 215),
                );
            }
        }
        _ => {
            painter.circle_filled(crown, 12.0, Color32::from_rgb(236, 242, 255));
            painter.circle_filled(crown + vec2(0.0, 1.0), 7.0, glow);
            painter.rect_filled(
                egui::Rect::from_center_size(crown + vec2(0.0, 1.0), vec2(2.0, 9.0)),
                1.0,
                Color32::from_rgb(8, 8, 14),
            );
        }
    }
    match style.trail {
        1 => {
            for k in 0..7 {
                let ph = (time * 0.8 + k as f32 / 7.0).fract();
                let a = k as f32 * 2.4;
                painter.circle_filled(
                    crown + vec2(a.cos(), a.sin()) * ph * 26.0 + vec2(0.0, ph * ph * 22.0),
                    2.8 * (1.0 - ph),
                    alpha(glow, (255.0 * (1.0 - ph)) as u8),
                );
            }
        }
        2 => {
            for k in 0..3 {
                let a = time * 1.8 + k as f32 * 2.094;
                let c = crown + vec2(a.cos() * 30.0, a.sin() * 12.0);
                painter.line_segment(
                    [c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)],
                    Stroke::new(1.5, Color32::from_rgb(255, 248, 220)),
                );
                painter.line_segment(
                    [c - vec2(0.0, 5.0), c + vec2(0.0, 5.0)],
                    Stroke::new(1.5, Color32::from_rgb(255, 248, 220)),
                );
            }
        }
        3 => {
            for k in 0..8 {
                let ph = (time * 0.5 + k as f32 / 8.0).fract();
                painter.circle_filled(
                    crown + vec2((k as f32 * 3.1 + time).sin() * 7.0 * ph, -ph * 52.0),
                    2.6 * (1.0 - ph),
                    alpha(Color32::from_rgb(255, 150, 70), (255.0 * (1.0 - ph)) as u8),
                );
            }
        }
        4 if style.head != 4 => {
            for i in 1..=12 {
                let f = i as f32 / 12.0;
                painter.circle_filled(
                    crown
                        + axis * (8.0 + f * 46.0)
                        + normal * ((time * 1.1 - f * 2.0).sin() * 2.2 * f),
                    4.5 * (1.0 - f * 0.8),
                    alpha(glow, (200.0 * (1.0 - f)) as u8),
                );
            }
        }
        _ => {}
    }
    painter.text(
        r.left_top() + vec2(8.0, 8.0),
        Align2::LEFT_TOP,
        SHAFTS[style.shaft as usize].name,
        FontId::proportional(11.0),
        Color32::from_white_alpha(150),
    );
}

/// Draw the atelier. `open` is closed by the window's own button.
pub fn draw(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let mut style = wand_style::current();
    let before = style;
    let time = ctx.input(|i| i.time) as f32;
    egui::Window::new("Wand atelier")
        .open(open)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                draw_preview(ui, style, time);
                ui.vertical(|ui| {
                    picker(
                        ui,
                        "Shaft",
                        &mut style.shaft,
                        &SHAFTS.map(|s| s.name),
                        &|i| Some(rgb(SHAFTS[i as usize].base)),
                    );
                    picker(ui, "Crown", &mut style.head, &HEADS, &|_| None);
                    picker(ui, "Grip", &mut style.grip, &GRIPS.map(|g| g.0), &|i| {
                        Some(rgb(GRIPS[i as usize].1))
                    });
                    picker(ui, "Glow", &mut style.glow, &GLOWS.map(|g| g.0), &|i| {
                        GLOWS[i as usize].1.map(rgb)
                    });
                    picker(ui, "Sheds", &mut style.trail, &TRAILS, &|_| None);
                    picker(
                        ui,
                        "Runes",
                        &mut style.runes,
                        &["Plain", "Mana beads", "Mana vein"],
                        &|_| None,
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Surprise me").clicked() {
                            let t = (time * 1000.0) as u32;
                            let h = |k: u32, n: usize| {
                                ((t.wrapping_mul(2654435761).rotate_left(k)
                                    ^ k.wrapping_mul(40503))
                                    % n as u32) as u8
                            };
                            style = WandStyle {
                                shaft: h(3, SHAFTS.len()),
                                head: h(7, HEADS.len()),
                                trail: h(11, TRAILS.len()),
                                grip: h(13, GRIPS.len()),
                                glow: h(17, GLOWS.len()),
                                runes: 1 + h(19, 2),
                            };
                        }
                        if ui.button("Reset").clicked() {
                            style = WandStyle::default();
                        }
                    });
                    ui.small(format!("[{}] Close", crate::keybinds::hint(crate::keybinds::Action::WandAtelier)));
                });
            });
        });
    if style != before {
        wand_style::set(style);
    }
    ctx.request_repaint();
}
