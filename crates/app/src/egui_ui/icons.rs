//! Small original pixel illustrations. Solid mesh pixels have no AA seams.
//! The shared six-role palette lives in `pixel_art`; icons only supply an
//! accent colour plus native-pixel glyphs.
use super::pixel_art::{self, GOLD as PALETTE_GOLD, PAPER as PALETTE_PAPER, TEAL as PALETTE_TEAL};
use crate::world::factory::Kind;
use egui::{Color32 as C, Rect, Vec2, vec2};
pub const GOLD: C = PALETTE_GOLD;
pub const PAPER: C = PALETTE_PAPER;
pub const TEAL: C = PALETTE_TEAL;

#[derive(Clone, Copy)]
pub enum Icon {
    Resource(usize),
    Machine(Kind),
    Spell(u32),
    Wand(u8),
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
        Icon::Wand(tier) => (
            &[
                "          33  ",
                "         3223 ",
                "         3223 ",
                "          33  ",
                "         3553 ",
                "        3663  ",
                "       3663   ",
                "      3663    ",
                "     3663     ",
                "    3663      ",
                "   3663       ",
                "  3663        ",
                "   33         ",
            ],
            match tier {
                4 => C::from_rgb(117, 208, 237),
                5 => C::from_rgb(178, 140, 219),
                _ => GOLD,
            },
        ),
        Icon::Torch => (
            &[
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
            ][..],
            C::from_rgb(150, 109, 73),
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
        Icon::Spell(4) => (
            &[
                " 333333333 ",
                " 322222223 ",
                " 321333123 ",
                " 321323123 ",
                " 321333123 ",
                "  3211123  ",
                "   32123   ",
                "    333    ",
            ],
            C::from_rgb(141, 224, 245),
        ),
        Icon::Spell(5) => (
            &[
                "   3   3   ",
                " 3  333  3 ",
                "   32223   ",
                "  3222223  ",
                " 322122123 ",
                "33333333333",
                "    323    ",
                "   33333   ",
            ],
            C::from_rgb(163, 219, 83),
        ),
        Icon::Spell(6) => (
            &[
                "  333333  ",
                " 32222223 ",
                "322333223",
                "323   323",
                "323   323",
                "322333223",
                " 3222223 ",
                "  33333  ",
            ],
            C::from_rgb(175, 102, 244),
        ),
        Icon::Spell(7) => (
            &[
                "333    333",
                " 323  323 ",
                "  322223  ",
                "   3223   ",
                "  322223  ",
                " 323  323 ",
                "333    333",
            ],
            C::from_rgb(121, 221, 246),
        ),
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
        Icon::Spell(id) if super::icons_spells::art(id).is_some() => {
            super::icons_spells::art(id).expect("checked above")
        }
        Icon::Spell(id) => {
            let shape: &[&str] = match id % 4 {
                0 => &[
                    "      33      ",
                    "     3223     ",
                    "    322113    ",
                    "   32211113   ",
                    "    321113    ",
                    "     3113     ",
                    "      33      ",
                ],
                1 => &[
                    "  3       3  ",
                    "   3     3   ",
                    "    32223    ",
                    "   3222223   ",
                    "    32223    ",
                    "   3     3   ",
                    "  3       3  ",
                ],
                2 => &[
                    "     333     ",
                    "    32223    ",
                    "   3222223   ",
                    "  322222223  ",
                    "   3222223   ",
                    "    32223    ",
                    "     333     ",
                ],
                _ => &[
                    "   3333333   ",
                    "   3222223   ",
                    "   3211123   ",
                    "   3211123   ",
                    "   3222223   ",
                    "   3333333   ",
                ],
            };
            let color = C::from_rgb(
                (70 + (id * 37) % 170) as u8,
                (60 + (id * 19) % 160) as u8,
                (80 + (id * 53) % 150) as u8,
            );
            (shape, color)
        }
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
            let color = match tier {
                3 => C::from_rgb(246, 129, 57),
                4 => C::from_rgb(111, 216, 237),
                5 => C::from_rgb(136, 222, 198),
                2 => C::from_rgb(180, 197, 199),
                _ => C::from_rgb(208, 139, 75),
            };
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
                19 => C::from_rgb(120, 118, 124),
                20 => C::from_rgb(150, 92, 44),
                21 => C::from_rgb(156, 210, 118),
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
                // Coke: a porous, glassy-grey lump.
                19 => &[
                    "    3333     ",
                    "  33212333   ",
                    " 3212221123  ",
                    " 3221312223  ",
                    "32122221213  ",
                    " 3213122123  ",
                    "  333221233  ",
                    "     3333    ",
                ][..],
                // Liquids come in a stoppered flask.
                20 | 21 => &[
                    "    3333    ",
                    "    3113    ",
                    "     33     ",
                    "    3223    ",
                    "   322223   ",
                    "  32222223  ",
                    "  32211223  ",
                    "  32222223  ",
                    "   333333   ",
                ][..],
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
            let c = if k == Kind::CokeBrick || k == Kind::CokeOven {
                C::from_rgb(178, 98, 62)
            } else if k == Kind::BlastBrick || k == Kind::BlastFurnace {
                C::from_rgb(150, 132, 122)
            } else if k == Kind::Battery || k == Kind::InductionCell {
                C::from_rgb(110, 196, 226)
            } else if matches!(k, Kind::InductionCasing | Kind::InductionPort) {
                C::from_rgb(150, 170, 186)
            } else if k == Kind::InductionProvider || k == Kind::Assembler {
                C::from_rgb(214, 170, 82)
            } else if k == Kind::Transformer {
                C::from_rgb(196, 128, 70)
            } else if k == Kind::FluidPipe || k.water_port() {
                C::from_rgb(83, 162, 210)
            } else if k == Kind::Cable {
                TEAL
            } else {
                GOLD
            };
            let shape = if k == Kind::Battery {
                &[
                    "  33    33  ",
                    "  31    31  ",
                    " 3333333333 ",
                    " 3222222223 ",
                    " 3211111123 ",
                    " 3212222123 ",
                    " 3211111123 ",
                    " 3222222223 ",
                    " 3333333333 ",
                ][..]
            } else if k == Kind::Transformer {
                &[
                    "   3    3   ",
                    "  323  323  ",
                    " 3333333333 ",
                    " 3211331123 ",
                    " 3212332123 ",
                    " 3211331123 ",
                    " 3212332123 ",
                    " 3211331123 ",
                    " 3333333333 ",
                ][..]
            } else if k == Kind::Assembler {
                &[
                    "    3333    ",
                    "  33222233  ",
                    " 3222332223 ",
                    " 3223113223 ",
                    " 3231111323 ",
                    " 3223113223 ",
                    " 3222332223 ",
                    "  33222233  ",
                    "    3333    ",
                ][..]
            } else if matches!(k, Kind::InductionCasing | Kind::InductionPort | Kind::InductionCell | Kind::InductionProvider) {
                &[
                    "333333333333",
                    "322222222223",
                    "321111111123",
                    "321333333123",
                    "321322223123",
                    "321333333123",
                    "321111111123",
                    "322222222223",
                    "333333333333",
                ][..]
            } else if k.structural() {
                &[
                    "333333333333",
                    "322223222223",
                    "333333333333",
                    "322222232223",
                    "333333333333",
                    "322223222223",
                    "333333333333",
                ][..]
            } else if k == Kind::CokeOven || k == Kind::BlastFurnace {
                &[
                    "333333333333",
                    "322223222223",
                    "333333333333",
                    "322331133223",
                    "323311113323",
                    "323111111323",
                    "323111111323",
                    "333333333333",
                ][..]
            } else if k == Kind::Generator {
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
                0 => C::from_rgb(196, 143, 75),
                1 => C::from_rgb(174, 76, 52),
                _ => C::from_rgb(113, 164, 105),
            };
            let shape: &[&str] = match kind {
                0 => &[
                    "     33     ",
                    "   332133   ",
                    "  32222223  ",
                    " 3224222223 ",
                    " 3222222223 ",
                    "  32222223  ",
                    "   333333   ",
                ],
                1 => &[
                    "     33     ",
                    "   332133   ",
                    "  32222223  ",
                    " 3222222223 ",
                    " 3224222223 ",
                    "  32222223  ",
                    "   333333   ",
                ],
                _ => &[
                    "      33    ",
                    "    332133  ",
                    "   32222223 ",
                    "  3224222223",
                    "  3222222223",
                    "   32222223 ",
                    "    333333  ",
                ],
            };
            (shape, color)
        }
    }
}
pub fn paint(p: &egui::Painter, r: Rect, icon: Icon) {
    paint_tinted(p, r, icon, None);
}

/// Paints an icon with an optional accent for its body colour. Tool fittings
/// use this to make the held weapon read as steel, cinder, rime or polymer
/// without duplicating every sprite.
pub fn paint_tinted(p: &egui::Painter, r: Rect, icon: Icon, accent: Option<C>) {
    paint_styled(p, r, icon, accent, None);
}

/// Explicit presentation hook for future item affixes; does not create gameplay state.
pub(super) fn paint_with_effect(p: &egui::Painter, r: Rect, icon: Icon, effect: super::magic_fx::Effect) {
    paint_styled(p, r, icon, None, Some(effect));
}

/// Paints native-pixel glyph rows centred in `r` on whole pixels with the
/// shared top-left key light. Returns the drawn rectangle.
pub(super) fn paint_rows(p: &egui::Painter, r: Rect, rows: &[&str], color: C) -> Rect {
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(1) as f32;
    let extent = width.max(rows.len() as f32);
    let unit = (r.width().min(r.height()) / extent).floor().max(1.0);
    let raw_origin = r.center() - vec2(width, rows.len() as f32) * unit * 0.5;
    let origin = egui::pos2(
        raw_origin.x.round(),
        raw_origin.y.round(),
    );
    let mut mesh = egui::Mesh::default();
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.bytes().enumerate() {
            let Some(tint) = pixel_art::glyph(ch, color) else {
                continue;
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
            // Light exposed top edges and shade exposed bottom edges on the
            // native pixel grid. The ramp comes from the shared palette, so
            // every item, machine and spell sprite reads with the same
            // top-left key light.
            let above = if y == 0 {
                b' '
            } else {
                rows[y - 1].as_bytes().get(x).copied().unwrap_or(b' ')
            };
            if ch == b'2' && above != b'2' && above != b'1' {
                let h = Rect::from_min_size(q.min, vec2(unit, 1.0));
                mesh.add_colored_rect(h, pixel_art::top_light(color));
            }
            let below = rows
                .get(y + 1)
                .and_then(|row| row.as_bytes().get(x))
                .copied()
                .unwrap_or(b' ');
            if ch == b'2' && below != b'2' && below != b'1' {
                let h = Rect::from_min_size(
                    egui::pos2(q.left(), q.bottom() - 1.0),
                    vec2(unit, 1.0),
                );
                mesh.add_colored_rect(h, pixel_art::bottom_shadow(color));
            }
        }
    }
    p.add(egui::Shape::mesh(mesh));
    Rect::from_min_size(origin, vec2(width, rows.len() as f32) * unit)
}

fn paint_styled(p: &egui::Painter, r: Rect, icon: Icon, accent: Option<C>, override_fx: Option<super::magic_fx::Effect>) {
    let (rows, base) = art(icon);
    let color = accent.unwrap_or(base);
    let drawn = paint_rows(p, r, rows, color);
    let effect = override_fx.or(match icon {
        Icon::Spell(26 | 46) => Some(super::magic_fx::Effect::Unstable),
        Icon::Spell(67 | 68) => Some(super::magic_fx::Effect::Cursed),
        Icon::Spell(_) | Icon::Wand(_) | Icon::Armor(_, 3) | Icon::Sword(3 | 4) => Some(super::magic_fx::Effect::Enchanted),
        _ => None,
    });
    if let Some(effect) = effect {
        let source=matches!(icon,Icon::Spell(_) | Icon::Wand(_) | Icon::Sword(3 | 4) | Icon::Armor(_,3)).then_some(color);
        super::magic_fx::paint(p, drawn, rows, effect, source);
    }
}
pub fn frame(p: &egui::Painter, r: Rect, selected: bool) {
    super::grimoire::slot(p, r, selected);
}
pub fn slot(
    ui: &mut egui::Ui,
    icon: Icon,
    quantity: Option<u32>,
    selected: bool,
    label: &str,
) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(vec2(56.0, 56.0), egui::Sense::click());
    let r = super::grimoire::react(&response, r, selected);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_sprite(label: &str, icon: Icon) {
        let (rows, _) = art(icon);
        assert!(!rows.is_empty(), "{label} is empty");
        assert!(rows.len() <= 16, "{label} is taller than a 16px sprite");
        assert!(
            rows.iter().all(|row| row.len() <= 16),
            "{label} is wider than a 16px sprite"
        );
        assert!(
            rows.iter().any(|row| row.contains('2')),
            "{label} has no body role"
        );
        assert!(
            rows.iter().any(|row| row.contains('3')),
            "{label} has no outline role"
        );
        for row in rows {
            for ch in row.bytes() {
                assert!(
                    ch == b' ' || pixel_art::glyph(ch, PAPER).is_some(),
                    "{label} contains unknown glyph {ch}"
                );
            }
        }
    }

    #[test]
    fn every_spell_and_resource_sprite_obeys_the_pixel_grid() {
        for id in 0..=crate::world::adventure::MAX_SPELL_ID {
            assert_sprite(&format!("spell {id}"), Icon::Spell(id));
        }
        for id in 0..crate::world::factory::RESOURCE_COUNT {
            assert_sprite(&format!("resource {id}"), Icon::Resource(id));
        }
    }
}
