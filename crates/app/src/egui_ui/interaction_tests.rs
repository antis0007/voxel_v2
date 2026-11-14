use super::*;

#[test]
fn interaction_regression_hotbar_and_offhand_have_equal_rects_at_all_supported_scales() {
    use crate::world::inventory::{Item, ItemStack, SurvivalInventory};
    for zoom_step in 15..=50 {
        let zoom = zoom_step as f32 * 0.05;
        for bar_step in 14..=32 {
            for right in [false, true] {
                for item in [None, Some(Item::Block(BlockId::Torch))] {
                    let ctx = egui::Context::default();
                    ctx.set_zoom_factor(zoom);
                    ctx.data_mut(|d| {
                        d.insert_temp(egui::Id::new("hotbar_size_scale"), bar_step as f32 * 0.05);
                        d.insert_temp(egui::Id::new("offhand_right"), right);
                    });
                    let mut creative = CreativeState::default();
                    creative.selected_slot = 4;
                    let inv = SurvivalInventory { offhand: item.map(|i| ItemStack::new(i, 1)), ..Default::default() };
                    let mut player = Player::new();
                    let mut output = ctx.run_ui(egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0))),
                        ..Default::default()
                    }, |root| {
                        egui::CentralPanel::default().show(root, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = grimoire::GAP;
                                draw_hotbar_row(ui, &mut creative, &mut player, Some(&inv), &mut None, &mut None);
                            });
                        });
                    });
                    output.textures_delta.clear();
                    let rects = ctx.data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new("interaction_regression_hotbar_rects"))).unwrap();
                    assert_eq!(rects.len(), 10);
                    for r in &rects { assert_eq!(r.size(), rects[0].size()); }
                    let (a,b) = if right { (8,9) } else { (0,1) };
                    assert!((rects[b].left() - rects[a].right() - 8.0).abs() < 0.01);
                    assert_eq!(creative.selected_slot, 4);
                }
            }
        }
    }
}
