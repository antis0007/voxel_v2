//! Shared chat overlay, player list and name tags.
//! T opens the chat and slash opens it with a command started; Enter sends, Escape closes.
use crate::social::Social;
use egui::{Align2, Color32, FontId, Key, Pos2, RichText};
use engine_net::protocol::ChatKind;
use engine_netcode::scoreboard::{self, Slot};
use glam::{Mat4, Vec3, Vec4};
use std::time::Duration;

/// Seconds a line stays on screen while the chat is closed.
const SHOWN: f32 = 10.0;
/// Lines visible while closed and while open.
const CLOSED_LINES: usize = 5;
const OPEN_LINES: usize = 20;
/// Name tags further than this are hidden, in metres.
const TAG_RANGE: f32 = 64.0;
const BACK: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 120);

#[derive(Default)]
pub struct ChatUi {
    open: bool,
    draft: String,
    focus: bool,
    /// Lines this player sent, newest last, for Up and Down recall.
    sent: Vec<String>,
    recall: Option<usize>,
}

impl ChatUi {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, start: &str) {
        self.open = true;
        self.draft = start.to_owned();
        self.focus = true;
        self.recall = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.draft.clear();
    }
}

fn colour(kind: ChatKind) -> Color32 {
    match kind {
        ChatKind::Say => Color32::from_rgb(236, 236, 236),
        ChatKind::Action => Color32::from_rgb(214, 170, 255),
        ChatKind::Whisper => Color32::from_rgb(170, 170, 190),
        ChatKind::System => Color32::from_rgb(255, 222, 120),
    }
}

/// Draws the overlay. Returns `true` on the frame the chat closes, so the caller can grab the mouse again.
pub fn draw(ctx: &egui::Context, chat: &mut ChatUi, social: &mut Social) -> bool {
    let screen = ctx.content_rect();
    let mut closed = false;
    let lines: Vec<_> = social
        .lines()
        .rev()
        .take(if chat.open { OPEN_LINES } else { CLOSED_LINES })
        .filter_map(|line| {
            let age = line.at.elapsed().as_secs_f32();
            let alpha = if chat.open { 1.0 } else { (SHOWN - age).clamp(0.0, 1.0) };
            (alpha > 0.0).then(|| (line.kind, line.text.clone(), alpha))
        })
        .collect();
    if !lines.is_empty() || chat.open {
        // Keep fading lines animating while the chat is closed.
        if !chat.open {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        egui::Area::new(egui::Id::new("chat_overlay"))
            .anchor(Align2::LEFT_BOTTOM, [12.0, -96.0])
            .interactable(chat.open)
            .show(ctx, |ui| {
                ui.set_max_width((screen.width() * 0.45).clamp(280.0, 640.0));
                egui::Frame::new().fill(if chat.open { BACK } else { Color32::TRANSPARENT }).inner_margin(6.0).show(ui, |ui| {
                    let draw_lines = |ui: &mut egui::Ui| {
                    for (kind, text, alpha) in lines.iter().rev() {
                        let back = if chat.open { Color32::TRANSPARENT } else { BACK.gamma_multiply(*alpha) };
                        egui::Frame::new().fill(back).inner_margin(egui::Margin::symmetric(4, 1)).show(ui, |ui| {
                            let label = RichText::new(text).size(14.0).color(colour(*kind).gamma_multiply(*alpha));
                            if chat.open { ui.label(label); }
                            else { ui.add(egui::Label::new(label).truncate()); }
                        });
                    }
                    };
                    if chat.open {
                        egui::ScrollArea::vertical().id_salt("chat_history")
                            .max_height((screen.height() * 0.4).max(60.0))
                            .auto_shrink([false, true]).stick_to_bottom(true)
                            .show(ui, draw_lines);
                    } else { draw_lines(ui); }
                    if chat.open {
                        let edit = egui::TextEdit::singleline(&mut chat.draft)
                            .hint_text("Message or /command · /help")
                            .char_limit(engine_net::protocol::MAX_TEXT_BYTES)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Body);
                        let response = ui.add(edit);
                        if chat.focus {
                            response.request_focus();
                            chat.focus = false;
                        }
                        let (enter, escape, up, down) = ui.input(|i| {
                            (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown))
                        });
                        if chat.draft.starts_with('/') && !chat.draft.contains(char::is_whitespace) {
                            let commands: &[&str] = if social.own.is_some() {
                                &["/help", "/gamemode", "/time", "/tp", "/seed", "/save-all", "/list", "/me", "/msg"]
                            } else {
                                &["/help", "/gamemode", "/gamerule", "/give", "/time", "/tp", "/seed", "/save-all", "/list", "/me"]
                            };
                            let matches: Vec<_> = commands.iter().copied().filter(|s| s.starts_with(&chat.draft)).collect();
                            if !matches.is_empty() {
                                ui.label(RichText::new(matches.join("  ")).small().color(Color32::GRAY));
                                if ui.input(|i| i.key_pressed(Key::Tab)) && matches.len() == 1 {
                                    chat.draft = format!("{} ", matches[0]);
                                    chat.focus = true;
                                }
                            }
                        }
                        if up || down {
                            let len = chat.sent.len();
                            chat.recall = match (chat.recall, up) {
                                (None, true) if len > 0 => Some(len - 1),
                                (Some(i), true) => Some(i.saturating_sub(1)),
                                (Some(i), false) if i + 1 < len => Some(i + 1),
                                _ => None,
                            };
                            chat.draft = chat.recall.map(|i| chat.sent[i].clone()).unwrap_or_default();
                            chat.focus = true;
                        }
                        if enter {
                            let line = chat.draft.trim().to_owned();
                            if !line.is_empty() {
                                social.send(&line);
                                if chat.sent.last() != Some(&line) {
                                    chat.sent.push(line);
                                }
                                if chat.sent.len() > 50 {
                                    chat.sent.remove(0);
                                }
                            }
                            chat.close();
                            closed = true;
                        } else if escape {
                            chat.close();
                            closed = true;
                        }
                    }
                });
            });
    }
    if chat.open && !social.roster().is_empty() {
        player_list(ctx, social);
    }
    sidebar(ctx, social);
    closed
}

/// Who is connected, with operator and creative markers.
fn player_list(ctx: &egui::Context, social: &Social) {
    egui::Area::new(egui::Id::new("chat_players"))
        .anchor(Align2::CENTER_TOP, [0.0, 12.0])
        // Interactable for the Copy button; the list only shows while the chat holds the mouse.
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::new().fill(BACK).inner_margin(8.0).show(ui, |ui| {
                ui.label(
                    RichText::new(format!("Players {}/{}", social.roster().len(), engine_net::protocol::MAX_PLAYERS))
                        .strong()
                        .color(Color32::WHITE),
                );
                if let Some(code) = &social.code {
                    ui.horizontal(|ui| {
                        let kind = if engine_net::punch::is_offer(code) { Some("Direct Offer") }
                            else if code.starts_with(engine_net::punch::ANSWER_PREFIX) { Some("Direct Answer") }
                            else { None };
                        ui.label(RichText::new(kind.unwrap_or("Session Code")).color(Color32::GRAY));
                        if kind.is_none() {
                            ui.label(RichText::new(code).monospace().color(Color32::WHITE));
                        }
                        if ui.small_button("Copy").clicked() {
                            ui.ctx().copy_text(code.clone());
                        }
                    });
                }
                let column = social.board.shown(Slot::List);
                egui::Grid::new("chat_player_grid").num_columns(5).spacing([16.0, 2.0]).show(ui, |ui| {
                    for _ in 0..3 {
                        ui.label("");
                    }
                    ui.label(RichText::new("Ping").color(Color32::GRAY));
                    ui.label(RichText::new(column.map_or("", |(_, o)| o.label.as_str())).color(Color32::GRAY));
                    ui.end_row();
                    for row in social.roster() {
                        let own = social.own == Some(row.id);
                        let name = RichText::new(&row.name).color(if own { Color32::from_rgb(140, 220, 255) } else { Color32::WHITE });
                        ui.label(name);
                        ui.label(RichText::new(if row.creative { "Creative" } else { "Survival" }).color(Color32::LIGHT_GRAY));
                        ui.label(RichText::new(if row.op { "Operator" } else { "" }).color(colour(ChatKind::System)));
                        match social.quality.get(&row.id) {
                            Some(q) => {
                                ui.horizontal(|ui| {
                                    signal(ui, q.bars());
                                    let loss = if q.loss_per_mille > 0 { format!(" {:.1} %", f32::from(q.loss_per_mille) / 10.0) } else { String::new() };
                                    ui.label(RichText::new(format!("{:.0} ms{loss}", q.rtt_ms)).color(Color32::LIGHT_GRAY));
                                });
                            }
                            None => {
                                ui.label(RichText::new(if row.id == 0 { "Host" } else { "" }).color(Color32::GRAY));
                            }
                        }
                        let value = column.and_then(|(id, o)| Some(scoreboard::format_value(social.board.score(id, row.id)?, o.unit)));
                        ui.label(RichText::new(value.unwrap_or_default()).color(Color32::WHITE));
                        ui.end_row();
                    }
                });
            });
        });
}

/// Five rising bars, filled up to `bars`, coloured by grade.
fn signal(ui: &mut egui::Ui, bars: u8) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 10.0), egui::Sense::hover());
    let lit = match bars {
        0..=1 => Color32::from_rgb(230, 90, 80),
        2..=3 => Color32::from_rgb(235, 190, 80),
        _ => Color32::from_rgb(110, 210, 120),
    };
    for i in 0..5u8 {
        let x = rect.left() + f32::from(i) * 3.6;
        let bar = egui::Rect::from_min_max(egui::pos2(x, rect.bottom() - 2.0 - f32::from(i) * 2.0), egui::pos2(x + 2.6, rect.bottom()));
        ui.painter().rect_filled(bar, 0.0, if i < bars { lit } else { Color32::from_gray(70) });
    }
}

/// The sidebar objective's top ten, at the right edge.
fn sidebar(ctx: &egui::Context, social: &Social) {
    let Some((id, objective)) = social.board.shown(Slot::Sidebar) else { return };
    let ranking = social.board.ranking(id, 10);
    if ranking.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("chat_sidebar"))
        .anchor(Align2::RIGHT_CENTER, [-12.0, 0.0])
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new().fill(BACK).inner_margin(5.0).show(ui, |ui| {
                ui.label(RichText::new(&objective.label).size(12.0).strong().color(Color32::WHITE));
                egui::Grid::new("chat_sidebar_grid").num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                    for (player, value) in ranking {
                        ui.label(RichText::new(social.name_of(player).unwrap_or("?")).size(11.0).color(Color32::LIGHT_GRAY));
                        ui.label(RichText::new(scoreboard::format_value(value, objective.unit)).size(11.0).color(Color32::WHITE));
                        ui.end_row();
                    }
                });
            });
        });
}

/// Draws each remote player's name above its head. `view_proj` maps voxel positions to clip space.
pub fn name_tags(ctx: &egui::Context, view_proj: Mat4, eye: Vec3, metres_per_voxel: f32, tags: &[(Vec3, String)]) {
    const PLAYER_COLORS: [[u8; 3]; 6] = [
        [157, 229, 255], [255, 207, 150], [187, 246, 177],
        [223, 194, 255], [255, 184, 215], [251, 233, 151],
    ];
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let font = FontId::proportional(15.0);
    for (at, name) in tags {
        let distance = at.distance(eye) * metres_per_voxel;
        if distance > TAG_RANGE {
            continue;
        }
        let clip = view_proj * Vec4::new(at.x, at.y, at.z, 1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1.2 || ndc.y.abs() > 1.2 {
            continue;
        }
        let pos = Pos2::new(
            screen.left() + (ndc.x * 0.5 + 0.5) * screen.width(),
            screen.top() + (0.5 - ndc.y * 0.5) * screen.height(),
        );
        let alpha = (1.0 - (distance - TAG_RANGE * 0.75).max(0.0) / (TAG_RANGE * 0.25)).clamp(0.0, 1.0);
        let key = name.bytes().fold(0x811c_9dc5u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193));
        let [r, g, b] = PLAYER_COLORS[key as usize % PLAYER_COLORS.len()];
        let color = Color32::from_rgb(r, g, b).gamma_multiply(alpha);
        let galley = painter.layout_no_wrap(name.clone(), font.clone(), color);
        let rect = Align2::CENTER_BOTTOM.anchor_size(pos, galley.size()).expand2(egui::vec2(4.0, 1.0));
        painter.rect_filled(rect, 2.0, BACK.gamma_multiply(alpha));
        painter.galley(rect.min + egui::vec2(4.0, 1.0), galley, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, OutputCommand, PointerButton, RawInput, Rect, pos2, vec2};

    fn frame(ctx: &egui::Context, social: &Social, events: Vec<Event>) -> egui::FullOutput {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0))),
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| player_list(ui.ctx(), social));
        output.textures_delta.clear();
        output
    }

    /// Screen rectangle of the first drawn text equal to `text`.
    fn text_rect(output: &egui::FullOutput, text: &str) -> Option<Rect> {
        fn walk(shape: &egui::Shape, text: &str) -> Option<Rect> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == text => Some(t.galley.rect.translate(t.pos.to_vec2())),
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| walk(s, text)),
                _ => None,
            }
        }
        output.shapes.iter().find_map(|clipped| walk(&clipped.shape, text))
    }

    #[test]
    fn player_list_shows_the_session_code_and_copy_puts_it_on_the_clipboard() {
        let ctx = egui::Context::default();
        let mut social = Social::default();
        social.code = Some("ABCD-EFGH".into());
        frame(&ctx, &social, Vec::new());
        let out = frame(&ctx, &social, Vec::new());
        assert!(text_rect(&out, "ABCD-EFGH").is_some(), "the code is drawn");
        let at = text_rect(&out, "Copy").expect("a Copy button").center();
        let press = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        frame(&ctx, &social, vec![Event::PointerMoved(at)]);
        frame(&ctx, &social, vec![press(true)]);
        let out = frame(&ctx, &social, vec![press(false)]);
        let copied = out.platform_output.commands.iter().any(|c| matches!(c, OutputCommand::CopyText(t) if t == "ABCD-EFGH"));
        assert!(copied, "Copy puts the code on the clipboard");

        social.code = None;
        frame(&ctx, &social, Vec::new());
        let out = frame(&ctx, &social, Vec::new());
        assert!(text_rect(&out, "Copy").is_none(), "no code, no Copy button");
    }
}
