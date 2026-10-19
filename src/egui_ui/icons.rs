//! Small original pixel illustrations. Solid mesh pixels have no AA seams.
use crate::world::factory::Kind;
use egui::{Color32 as C, Rect, Vec2, vec2};
pub const GOLD: C = C::from_rgb(218, 173, 96);
pub const PAPER: C = C::from_rgb(235, 225, 197);
pub const TEAL: C = C::from_rgb(101, 183, 175);

#[derive(Clone, Copy)]
pub enum Icon {
    Resource(usize),
    Machine(Kind),
    Spell(u32),
    Heart,
    Torch,
    Bore,
    Sword(u32),
    Armor(u8, u8),
    Food(u8),
}
fn art(icon: Icon) -> (&'static [&'static str], C) {
    let gem = &[
        "      33      ",
        "     3223     ",
        "    322113    ",
        "   32211113   ",
        "   32211113   ",
        "    321113    ",
        "     3113     ",
        "      33      ",
    ][..];
    match icon {
        Icon::Torch => (&[
            "       6     ",
            "   6  656    ",
            "  6566556    ",
            "   655456    ",
            "    65456    ",
            "    33333    ",
            "     313     ",
            "     333     ",
            "     323     ",
            "     333     ",
            "     323     ",
            "     333     ",
        ][..], C::from_rgb(150,109,73)),
        Icon::Heart => (
            &[
                "  333   333  ",
                " 32223 32223 ",
                "3221223221223",
                "3221111111123",
                " 32111111123 ",
                "  321111123  ",
                "   3211123   ",
                "    32123    ",
                "     323     ",
                "      3      ",
            ],
            C::from_rgb(195, 66, 69),
        ),
        Icon::Bore => (
            &[
                "   3333333   ",
                "  322222223  ",
                "  331111133  ",
                "    313     ",
                "   32223    ",
                "    313     ",
                "   32223    ",
                "    313     ",
                "     3      ",
            ],
            GOLD,
        ),
        Icon::Spell(0) => (
            &[
                "       33    ",
                "      323    ",
                "     3223    ",
                "    3223     ",
                "   3222223   ",
                "     3223    ",
                "    3223     ",
                "   323       ",
                "  33         ",
            ],
            GOLD,
        ),
        Icon::Spell(1) => (
            &[
                "    3333    ",
                "    3223    ",
                "    3223    ",
                " 3333223333 ",
                " 3222222223 ",
                " 3333223333 ",
                "    3223    ",
                "    3333    ",
            ],
            C::from_rgb(101, 193, 118),
        ),
        Icon::Spell(2) => (
            &[
                " 3   3   3 ",
                "  3  3  3  ",
                "   32323   ",
                "33322222333",
                "   32323   ",
                "  3  3  3  ",
                " 3   3   3 ",
            ],
            C::from_rgb(117, 208, 237),
        ),
        Icon::Spell(4) => (&[" 333333333 "," 322222223 "," 321333123 "," 321323123 "," 321333123 ","  3211123  ","   32123   ","    333    "],C::from_rgb(141,224,245)),
        Icon::Spell(5) => (&["   3   3   "," 3  333  3 ","   32223   ","  3222223  "," 322122123 ","33333333333","    323    ","   33333   "],C::from_rgb(163,219,83)),
        Icon::Spell(6) => (&["  333333  "," 32222223 ","322333223","323   323","323   323","322333223"," 3222223 ","  33333  "],C::from_rgb(175,102,244)),
        Icon::Spell(7) => (&["333    333"," 323  323 ","  322223  ","   3223   ","  322223  "," 323  323 ","333    333"],C::from_rgb(121,221,246)),
        Icon::Spell(9) => (
            &[
                "        333",
                "       3243",
                "      32423",
                "     32423 ",
                "    32423  ",
                " 3332423   ",
                "  32223    ",
                " 32333     ",
                "333        ",
            ],
            C::from_rgb(88, 255, 194),
        ),
        Icon::Spell(10) => (
            &[
                "  3333333  ",
                " 324333223 ",
                "323 111 323",
                "3431   1323",
                "3231   1343",
                "323 111 323",
                " 322333423 ",
                "  3333333  ",
            ],
            C::from_rgb(154, 72, 250),
        ),
        Icon::Spell(11) => (
            &[
                "    343    ",
                "    343    ",
                "  3 343 3  ",
                "   32423   ",
                "34444444443",
                "   32423   ",
                "  3 343 3  ",
                "    343    ",
                "    343    ",
            ],
            C::from_rgb(98, 235, 255),
        ),
        Icon::Spell(12) => (
            &[
                "   3   3   ",
                "  323 323  ",
                " 324232423 ",
                "32444244423",
                "32444444423",
                " 322444223 ",
                "  3322233  ",
                "    333    ",
            ],
            C::from_rgb(255, 118, 42),
        ),
        Icon::Spell(13) => (
            &[
                "3   343   3",
                " 3  323  3 ",
                "  3324233  ",
                "33244444233",
                "  3324233  ",
                " 3  323  3 ",
                "3   343   3",
            ],
            C::from_rgb(138, 235, 255),
        ),
        Icon::Spell(14) => (
            &[
                "   33333   ",
                "  3244423  ",
                " 324232423 ",
                " 324444423 ",
                "  3324233  ",
                "   3243    ",
                "  3243     ",
                " 343       ",
            ],
            C::from_rgb(132, 210, 255),
        ),
        Icon::Spell(15) => (
            &[
                "    343    ",
                "  3324233  ",
                " 324333423 ",
                "3243 4 3423",
                " 324333423 ",
                "  3324233  ",
                "    323    ",
                "   33333   ",
            ],
            C::from_rgb(255, 218, 72),
        ),
        Icon::Spell(16) => (
            &[
                " 333   333 ",
                "32423 32423",
                " 3243 3423 ",
                "  3243423  ",
                "   32423   ",
                "    323    ",
                "    333    ",
            ],
            C::from_rgb(118, 224, 255),
        ),
        Icon::Spell(17) => (
            &[
                "333 343 333",
                "34233433243",
                " 324242423 ",
                "  3324233  ",
                "    323    ",
                "    333    ",
            ],
            C::from_rgb(142, 236, 255),
        ),
        Icon::Spell(18) => (
            &[
                "    333    ",
                "   32423   ",
                " 332434233 ",
                "32423432423",
                " 333 3 333 ",
                "    343    ",
                "   32423   ",
                "    333    ",
            ],
            C::from_rgb(255, 174, 72),
        ),
        Icon::Spell(19) => (
            &[
                "  3333333  ",
                " 323   323 ",
                "323 343 323",
                "32334443323",
                "323 343 323",
                " 323   323 ",
                "  3333333  ",
            ],
            C::from_rgb(122, 255, 198),
        ),
        Icon::Spell(20) => (
            &[
                " 343   333 ",
                "3423  32223",
                " 33  323 323",
                "    323  323",
                "323  323    ",
                "323 323  33 ",
                "32223  3243",
                " 333   343 ",
            ],
            C::from_rgb(255, 214, 102),
        ),
        Icon::Spell(21) => (
            &[
                "    343    ",
                "   32423   ",
                "  3244423  ",
                " 324232423 ",
                "3223 3 3223",
                " 33 343 33 ",
                "   32423   ",
                "    333    ",
            ],
            C::from_rgb(245, 148, 86),
        ),
        Icon::Spell(22) => (
            &[
                "  333  333 ",
                " 3243 3243 ",
                "3242332423 ",
                " 332433243 ",
                "  3243 3243",
                " 343  343  ",
                " 33   33   ",
            ],
            C::from_rgb(102, 255, 224),
        ),
        Icon::Spell(23) => (
            &[
                "       3443",
                "      32443",
                "    332423 ",
                "  3322233  ",
                " 322233    ",
                "32133      ",
                "33         ",
            ],
            C::from_rgb(255, 138, 228),
        ),
        Icon::Spell(_) => (gem, C::from_rgb(177, 127, 224)),
        Icon::Sword(tier) => {
            let shape = &[
                "            333",
                "           32223",
                "          3211223",
                "         3211223 ",
                "        3211223  ",
                "       3211223   ",
                "      3211223    ",
                "     3211223     ",
                "    3211223      ",
                "   3211223       ",
                "333321223        ",
                " 333223          ",
                "  333            ",
            ][..];
            let color = match tier {3=>C::from_rgb(246,129,57),4=>C::from_rgb(111,216,237),2=>C::from_rgb(180,197,199),_=>C::from_rgb(208,139,75)};
            (shape, color)
        }
        Icon::Resource(i) => {
            let color = match i {
                0 | 6 | 9 => C::from_rgb(205, 125, 73),
                1 | 7 | 10 => C::from_rgb(148, 192, 201),
                2 | 8 | 11 | 15 => C::from_rgb(174, 181, 180),
                3 => C::from_rgb(88, 94, 111),
                4 | 13 => C::from_rgb(137, 194, 104),
                5 => C::from_rgb(130, 189, 235),
                14 => GOLD,
                16 => C::from_rgb(220, 139, 76),
                _ => C::from_rgb(172, 126, 226),
            };
            let shape = match i {
                6..=8 | 14 | 15 => &[
                    "   33333333  ",
                    "  3222222223 ",
                    " 32222222223 ",
                    "333333333333 ",
                    "311111111113 ",
                    " 33333333333 ",
                ][..],
                9..=11 => &[
                    "      3      ",
                    "     323     ",
                    "   333233    ",
                    "  32232233   ",
                    " 3222232223  ",
                    "333333333333 ",
                ][..],
                12 => &[
                    "  333333333  ",
                    " 32232232223 ",
                    " 32333333223 ",
                    " 32322223223 ",
                    " 32322223223 ",
                    " 32333333223 ",
                    " 32232232223 ",
                    "  333333333  ",
                ][..],
                16 => &[
                    "   3333333   ",
                    "  322222223  ",
                    " 32333333223 ",
                    " 323    3223 ",
                    " 323    3223 ",
                    " 32333333223 ",
                    "  322222223  ",
                    "   3333333   ",
                ][..],
                4 | 13 => &[
                    "    33333    ",
                    "  332222233  ",
                    " 32212212223 ",
                    "3333333333333",
                    "    3113     ",
                    "    3223     ",
                    "   333333    ",
                ][..],
                5 | 17 | 18 => gem,
                _ => &[
                    "    333333   ",
                    "  333212233  ",
                    " 32232211223 ",
                    " 32113312223 ",
                    " 32211233213 ",
                    "  311121133  ",
                    "   3333333   ",
                ][..],
            };
            (shape, color)
        }
        Icon::Machine(k) => {
            let c = if k == Kind::FluidPipe || k.water_port() {
                C::from_rgb(83, 162, 210)
            } else if k == Kind::Cable {
                TEAL
            } else {
                GOLD
            };
            let shape = if k == Kind::Generator {
                &[
                    "       333  ",
                    "       323  ",
                    " 3333333233 ",
                    " 3222222223 ",
                    " 3213333123 ",
                    " 3213223123 ",
                    " 3213233123 ",
                    " 3222222223 ",
                    "333333333333",
                ][..]
            } else if k == Kind::Crusher {
                &[
                    " 333    333 ",
                    " 323    323 ",
                    " 3233333323 ",
                    " 3222222223 ",
                    " 3233323333 ",
                    " 3322332233 ",
                    " 3322332233 ",
                    " 3233323333 ",
                    " 3222222223 ",
                    "333333333333",
                ][..]
            } else if k == Kind::Feeder {
                &[
                    "333333333333",
                    "322222222223",
                    " 3222222223 ",
                    "  32222223  ",
                    "   322223   ",
                    "    3223    ",
                    "    3223    ",
                    "    3333    ",
                ][..]
            } else if k == Kind::Storage {
                &[
                    " 3333333333 ",
                    " 3222222223 ",
                    " 3333333333 ",
                    " 3231111323 ",
                    " 3213113123 ",
                    " 3211331123 ",
                    " 3213113123 ",
                    " 3231111323 ",
                    " 3333333333 ",
                ][..]
            } else if k == Kind::Fabricator {
                &[
                    " 3333333333 ",
                    " 3222222223 ",
                    " 3133223313 ",
                    " 313 33 313 ",
                    " 3133333313 ",
                    " 3132222313 ",
                    " 313    313 ",
                    " 3222222223 ",
                    "333333333333",
                ][..]
            } else if k.conduit() {
                &[
                    "    3333    ",
                    "    3223    ",
                    " 3333223333 ",
                    " 3222222223 ",
                    " 3222112223 ",
                    " 3333223333 ",
                    "    3223    ",
                    "    3333    ",
                ][..]
            } else if k == Kind::MiningDrill {
                &[
                    " 3333333333 ",
                    " 3222222223 ",
                    " 3133333313 ",
                    " 313 33 313 ",
                    " 3133223313 ",
                    " 313 33 313 ",
                    " 3133223313 ",
                    " 313 33 313 ",
                    "333333333333",
                ][..]
            } else if k.water_port() || k == Kind::Bioreactor || k == Kind::Cultivator {
                &[
                    " 3333333333 ",
                    " 3222222223 ",
                    " 313    313 ",
                    " 313  2 313 ",
                    " 3132222313 ",
                    " 3132222313 ",
                    " 3132222313 ",
                    " 3222222223 ",
                    " 3333333333 ",
                ][..]
            } else {
                &[
                    "   33333333 ",
                    "  3222222233",
                    " 32222222313",
                    "333333333313",
                    "321111112313",
                    "321233212313",
                    "321233212313",
                    "321111112313",
                    "32222222233 ",
                    " 333333333  ",
                ][..]
            };
            (shape, c)
        }
        Icon::Armor(slot, tier) => {
            let color = match tier {
                1 => C::from_rgb(209, 128, 72),
                2 => C::from_rgb(168, 189, 206),
                _ => C::from_rgb(112, 214, 186),
            };
            let shape: &[&str] = match slot {
                0 => &[
                    "   333333   ",
                    "  32222223  ",
                    " 3222442223 ",
                    " 3223333223 ",
                    " 3213  3123 ",
                    " 333    333 ",
                ],
                1 => &[
                    " 3333  3333 ",
                    "322233332223",
                    "322222222223",
                    " 3322112233 ",
                    "  32211223  ",
                    "  32111123  ",
                    "  33333333  ",
                ],
                2 => &[
                    "  33333333  ",
                    "  32222223  ",
                    "  32133123  ",
                    "  32133123  ",
                    "  32133123  ",
                    "  31133113  ",
                    "  333  333  ",
                ],
                _ => &[
                    " 3333  3333 ",
                    " 3213  3123 ",
                    " 3223  3223 ",
                    "32223  32223",
                    "33333  33333",
                ],
            };
            (shape, color)
        }
        Icon::Food(kind) => {
            let color = match kind {
                0 => C::from_rgb(207, 152, 88),
                1 => C::from_rgb(205, 96, 68),
                _ => C::from_rgb(122, 198, 154),
            };
            let shape: &[&str] = match kind {
                0 => &[
                    "   333333   ",
                    "  32244223  ",
                    " 3222222223 ",
                    " 3211221123 ",
                    "  31111113  ",
                    "   333333   ",
                ],
                1 => &[
                    "   333333   ",
                    "  32222223  ",
                    " 3224224223 ",
                    " 3333223333 ",
                    "    3113    ",
                    "    3113    ",
                    "    3333    ",
                ],
                _ => &[
                    "  33333333  ",
                    " 3224224223 ",
                    " 3222222223 ",
                    " 3111111113 ",
                    "  33333333  ",
                ],
            };
            (shape, color)
        }
    }
}
pub fn paint(p: &egui::Painter, r: Rect, icon: Icon) {
    let (rows, color) = art(icon);
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(1) as f32;
    let extent = width.max(rows.len() as f32);
    let dpi = p.ctx().pixels_per_point();
    let unit = ((r.width().min(r.height()) * dpi / extent).floor().max(1.0)) / dpi;
    let raw_origin = r.center() - vec2(width, rows.len() as f32) * unit * 0.5;
    let origin = egui::pos2((raw_origin.x*dpi).round()/dpi, (raw_origin.y*dpi).round()/dpi);
    let mut mesh = egui::Mesh::default();
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.bytes().enumerate() {
            let tint = match ch {
                b'1' => color.gamma_multiply(0.66),
                b'2' => color,
                b'3' => C::from_rgb(29, 31, 34),
                b'4' => C::from_rgb(247,228,159),
                b'5' => C::from_rgb(232,176,76),
                b'6' => C::from_rgb(193,102,49),
                _ => continue,
            };
            let q =
                Rect::from_min_size(origin + vec2(x as f32, y as f32) * unit, Vec2::splat(unit));
            let base = mesh.vertices.len() as u32;
            for point in [
                q.left_top(),
                q.right_top(),
                q.right_bottom(),
                q.left_bottom(),
            ] {
                mesh.colored_vertex(point, tint);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
            // Light exposed top edges, rather than scattering unrelated gold
            // specks across every material. Keep the highlight on native pixels.
            let above = if y==0 { b' ' } else { rows[y-1].as_bytes().get(x).copied().unwrap_or(b' ') };
            if ch == b'2' && above != b'2' && above != b'1' {
                let h = Rect::from_min_size(q.min, vec2(unit, 1.0/dpi));
                let highlight = C::from_rgb(
                    (color.r() as f32*0.8+235.0*0.2) as u8,
                    (color.g() as f32*0.8+225.0*0.2) as u8,
                    (color.b() as f32*0.8+197.0*0.2) as u8,
                );
                mesh.add_colored_rect(h, highlight);
            }
        }
    }
    p.add(egui::Shape::mesh(mesh));
}
pub fn frame(p: &egui::Painter, r: Rect, selected: bool) {
    use super::presentation as style;
    p.rect_filled(r,0,style::INK);
    p.rect_stroke(r,0,egui::Stroke::new(2.0,if selected {style::PAPER}else{style::EDGE}),egui::StrokeKind::Inside);
    let inner=r.shrink(2.0);
    p.rect_filled(inner,0,C::from_rgb(49,42,42));
    p.line_segment([inner.left_bottom(),inner.left_top()],egui::Stroke::new(2.0,C::from_rgb(20,17,23)));
    p.line_segment([inner.left_top(),inner.right_top()],egui::Stroke::new(2.0,C::from_rgb(20,17,23)));
    p.line_segment([inner.left_bottom(),inner.right_bottom()],egui::Stroke::new(1.0,C::from_rgb(126,103,80)));
    if selected { p.rect_stroke(r.shrink(1.0),0,egui::Stroke::new(1.0,style::AMBER),egui::StrokeKind::Inside); }
}
pub fn slot(
    ui: &mut egui::Ui,
    icon: Icon,
    quantity: Option<u32>,
    selected: bool,
    label: &str,
) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(vec2(56.0, 56.0), egui::Sense::click());
    frame(ui.painter(), r, selected);
    paint(ui.painter(), r.shrink(5.0), icon);
    if let Some(n) = quantity {
        let text = if n % 64 == 0 {
            format!("{}", n / 64)
        } else {
            format!("{:.2}", n as f32 / 64.0)
        };
        ui.painter().text(
            r.right_bottom() - vec2(4.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            text,
            egui::FontId::monospace(12.0),
            PAPER,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response.on_hover_text(label)
}
pub fn gauge(ui: &mut egui::Ui, value: f32, color: C, label: &str) {
    let (r, res) = ui.allocate_exact_size(
        vec2(ui.available_width().min(250.0), 8.0),
        egui::Sense::hover(),
    );
    frame(ui.painter(), r, false);
    ui.painter().rect_filled(
        Rect::from_min_size(
            r.min + Vec2::splat(2.0),
            vec2((r.width() - 4.0) * value.clamp(0.0, 1.0), r.height() - 4.0),
        ),
        0,
        color,
    );
    res.on_hover_text(label);
}
