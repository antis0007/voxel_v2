//! Villager trade rows: input stacks, an arrow, the output stack and uses left today.
use super::presentation as style;
use crate::world::World;
use crate::world::travel::{self, Refusal, trades};

const SLOT_PX: f32 = 40.0;
/// The page closes beyond this distance from the villager, in metres.
const RANGE_M: f32 = 6.0;

fn stack(ui: &mut egui::Ui, s: trades::Stack, have: Option<u32>) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(SLOT_PX, SLOT_PX), egui::Sense::hover());
    let painter = ui.painter();
    super::icons::frame(painter, rect, response.hovered());
    super::inventory::draw_item_icon(painter, rect.shrink(3.0), s.item);
    let short = have.is_some_and(|n| n < s.count);
    let color = if short { style::DANGER } else { style::PAPER };
    painter.text(rect.right_bottom() - egui::vec2(3.0, 2.0), egui::Align2::RIGHT_BOTTOM, s.count.to_string(), egui::FontId::proportional(11.0), color);
    response.on_hover_text(format!("{} × {}", s.item.name(), s.count));
}

fn arrow(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, SLOT_PX), egui::Sense::hover());
    let y = rect.center().y;
    let stroke = egui::Stroke::new(2.0, style::MUTED);
    ui.painter().line_segment([egui::pos2(rect.left() + 4.0, y), egui::pos2(rect.right() - 4.0, y)], stroke);
    for dy in [-5.0, 5.0] {
        ui.painter().line_segment([egui::pos2(rect.right() - 10.0, y + dy), egui::pos2(rect.right() - 4.0, y)], stroke);
    }
}

/// Draws the open villager's page; closes it when the villager leaves reach or Done is pressed.
pub fn draw(ctx: &egui::Context, open: &mut Option<u32>, world: &mut World, eye: glam::Vec3) {
    let Some(villager) = *open else { return };
    if travel_centre(world, villager).is_none_or(|c| c.distance(eye) > RANGE_M) {
        *open = None;
        return;
    }
    world.travel.restock(travel::day(world));
    let page = trades::page(villager);
    let (mut chosen, mut done, mut refusal) = (None, false, None);
    egui::Window::new("Villager")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Grid::new("trade_rows").spacing(egui::vec2(6.0, 4.0)).show(ui, |ui| {
                for (row, t) in page.iter().enumerate() {
                    ui.horizontal(|ui| {
                        for &s in &t.give {
                            stack(ui, s, Some(world.inventory.count_item(s.item)));
                        }
                    });
                    arrow(ui);
                    stack(ui, t.get, None);
                    let left = world.travel.uses_left(villager, row, t);
                    let label = if left == 0 { "Out of Stock".to_string() } else { format!("{left}/{}", t.uses) };
                    ui.label(egui::RichText::new(label).color(if left == 0 { style::MUTED } else { style::PAPER }));
                    let affordable = t.give.iter().all(|s| world.inventory.count_item(s.item) >= s.count);
                    if ui.add_enabled(left > 0 && affordable, egui::Button::new("Trade")).clicked() {
                        chosen = Some(row);
                    }
                    ui.end_row();
                }
            });
            done = ui.button("Done").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    if let Some(row) = chosen {
        refusal = travel::trade(world, villager, row).err();
    }
    if let Some(r) = refusal {
        world.expedition.say(match r {
            Refusal::Unknown => "Trade Unavailable",
            Refusal::OutOfStock => "Out of Stock",
            Refusal::Missing => "Items Missing",
        });
    }
    if done {
        *open = None;
    }
}

fn travel_centre(world: &World, villager: u32) -> Option<glam::Vec3> {
    crate::world::adventure::wildlife::vehicle::villager_centre(world, villager)
}
