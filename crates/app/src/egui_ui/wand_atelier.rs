//! Assemble the same authored families used by the held wand and inventory icon.
use crate::wand_style::{self, GLOWS, GRIPS, HEADS, SHAFTS, WandStyle};
use egui::{Align2, Color32, RichText, Sense, Stroke, vec2};
use super::presentation as ui_style;

fn rgb(c: [f32; 3]) -> Color32 {
    Color32::from_rgb(
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}


/// Names are directly selectable, including by keyboard.
fn picker(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u8,
    names: &[&str],
    swatch: &dyn Fn(u8) -> Option<Color32>,
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [72.0, 32.0],
            egui::Label::new(RichText::new(label).size(13.0).color(ui_style::MUTED)),
        );
        ui.spacing_mut().interact_size.y = 30.0;
        egui::ComboBox::from_id_salt(label)
            .width(172.0)
            .selected_text(RichText::new(names[*value as usize]).size(14.0))
            .show_ui(ui, |ui| {
                for (index, name) in names.iter().enumerate() {
                    let index = index as u8;
                    let color = swatch(index).unwrap_or(ui_style::PAPER);
                    ui.selectable_value(value, index, RichText::new(*name).color(color));
                }
            });
        if let Some(color) = swatch(*value) {
            let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
            ui.painter().rect_filled(rect, 2, color);
        }
    });
}

fn draw_preview(ui: &mut egui::Ui, style: WandStyle, compact: bool, time: f32) {
    let (response, painter) = ui.allocate_painter(
        if compact { vec2(286.0, 132.0) } else { vec2(178.0, 240.0) }, Sense::hover(),
    );
    let r = response.rect;
    painter.rect_filled(r, 4, Color32::from_rgb(18, 20, 24));
    painter.rect_stroke(r, 4, Stroke::new(1.0, ui_style::EDGE), egui::StrokeKind::Inside);
    let color = GLOWS[style.glow as usize].1.map(rgb).unwrap_or(ui_style::AMBER);
    let center = r.center() + vec2(0.0, 5.0);
    let breath = 0.84 + 0.16 * (time * 2.0).sin();
    for i in (1..=6).rev() {
        painter.circle_filled(center, r.height().min(r.width()) * i as f32 * 0.065,
            color.gamma_multiply((7 - i) as f32 * 0.018 * breath));
    }
    let look = ((style.shaft as u16) << 2) | ((style.head as u16) << 5)
        | ((style.grip as u16) << 12) | ((style.glow as u16) << 8);
    super::wand_icon::paint(&painter, egui::Rect::from_center_size(center, vec2(r.height().min(r.width()) * 0.80, r.height().min(r.width()) * 0.80)), look);
    painter.text(r.left_top() + vec2(11.0, 10.0), Align2::LEFT_TOP, "Preview",
        egui::FontId::proportional(12.0), ui_style::MUTED);
}

fn controls(ui: &mut egui::Ui, style: &mut WandStyle, time: f32) {
    ui.heading("Appearance");
    picker(ui, "Crown", &mut style.head, &HEADS, &|_| None);
    let modular = style.head < 6;
    ui.add_enabled_ui(modular, |ui| {
        picker(ui, "Shaft", &mut style.shaft, &SHAFTS.map(|s| s.name), &|_| None);
        picker(ui, "Grip", &mut style.grip, &GRIPS.map(|g| g.0), &|_| None);
        picker(ui, "Runes", &mut style.runes, &["Off", "Beads", "Etched"], &|_| None);
    });
    if !modular {
        ui.label(RichText::new("This crown uses a fixed body.").size(12.0).color(ui_style::MUTED));
    }
    picker(ui, "Glow", &mut style.glow, &GLOWS.map(|g| g.0), &|i| GLOWS[i as usize].1.map(rgb));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new("Randomize").min_size(vec2(112.0, 34.0))).clicked() {
            let seed = (time * 1000.0) as u32;
            let pick = |k: u32, n: usize| {
                ((seed.wrapping_mul(2654435761).rotate_left(k) ^ k.wrapping_mul(40503)) % n as u32) as u8
            };
            style.shaft = pick(3, SHAFTS.len());
            style.head = pick(7, HEADS.len());
            style.grip = pick(13, GRIPS.len());
            style.glow = pick(17, GLOWS.len());
        }
        if ui.add_enabled(*style != WandStyle::default(),
            egui::Button::new("Reset appearance").min_size(vec2(118.0, 34.0))).clicked()
        {
            *style = WandStyle::default();
        }
    });
}
/// Draw the atelier. `open` is closed by the window's own button.
pub fn draw(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let mut style = wand_style::current();
    let before = style;
    style = style.clamped();


    let time = ctx.input(|i| i.time) as f32;
    let screen = ctx.content_rect();
    let compact = screen.width() < 520.0;
    egui::Window::new("Wand atelier")
        .open(open)
        .default_width(if compact { 306.0 } else { 498.0 })
        .resizable(false)
        .collapsible(false)
        .max_height((screen.height() - 54.0).max(160.0))
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(12.0, 8.0);
            egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                if compact {
                    draw_preview(ui, style, true, time);
                    controls(ui, &mut style, time);
                } else {
                    ui.horizontal_top(|ui| {
                        draw_preview(ui, style, false, time);
                        ui.vertical(|ui| controls(ui, &mut style, time));
                    });
                }
            });
        });
    if style != before {
        wand_style::set(style);
    }
    if *open {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}
