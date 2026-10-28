//! Circuit sprites: the packaged part, one glyph per module role, and the
//! wafer map of a lot. Glyph digits are the shared `pixel_art` roles.
use super::icons::{self, paint_rows};
use egui::{Color32, Painter, Pos2, Rect, Stroke, vec2};
use engine_chip::fab::Grade;
use engine_chip::firmware::Task;
use engine_chip::lot::Die;
use crate::world::factory::chips::production::Supply;
use engine_chip::registry::Role;

pub const BIN_A: Color32 = Color32::from_rgb(101, 183, 175);
pub const BIN_B: Color32 = Color32::from_rgb(218, 173, 96);
pub const BIN_C: Color32 = Color32::from_rgb(214, 128, 64);
pub const SLOW: Color32 = Color32::from_rgb(120, 104, 140);
pub const DEAD: Color32 = Color32::from_rgb(92, 40, 36);
const WAFER: Color32 = Color32::from_rgb(64, 72, 92);
const UNPROBED: Color32 = Color32::from_rgb(96, 108, 132);

/// Die-window colour of a part by its firmware task.
pub fn task_colour(task: Task) -> Color32 {
    match task {
        Task::FillBand => Color32::from_rgb(110, 170, 255),
        Task::StockBand => icons::GOLD,
        Task::HeatGuard => Color32::from_rgb(235, 110, 70),
    }
}

pub fn grade_colour(g: Grade) -> Color32 {
    match g {
        Grade::A => BIN_A,
        Grade::B => BIN_B,
        Grade::C => BIN_C,
    }
}

const PACKAGE: &[&str] = &[
    "  4 4 4 4 4  ",
    "  4 4 4 4 4  ",
    " 33333333333 ",
    " 35333333333 ",
    " 33322222333 ",
    " 33321112333 ",
    " 33322222333 ",
    " 33333333333 ",
    "  4 4 4 4 4  ",
    "  4 4 4 4 4  ",
];

/// A packaged part: legs, body, pin-1 dot and a die window in `window`.
pub fn package(p: &Painter, r: Rect, window: Color32) -> Rect {
    paint_rows(p, r, PACKAGE, window)
}

/// A package with a status lamp at its lower right.
pub fn package_lamp(p: &Painter, r: Rect, window: Color32, lamp: Color32) {
    let drawn = package(p, r, window);
    let d = (drawn.height() * 0.22).max(3.0);
    p.circle_filled(drawn.right_bottom() - vec2(d * 0.6, d * 0.6), d * 0.5, lamp);
    p.circle_stroke(drawn.right_bottom() - vec2(d * 0.6, d * 0.6), d * 0.5, Stroke::new(1.0, icons::PAPER.gamma_multiply(0.5)));
}

fn glyph(role: Role) -> &'static [&'static str] {
    match role {
        Role::Core => &[" 4 4 4 ", "4333334", " 32223 ", "4321234", " 32223 ", "4333334", " 4 4 4 "],
        Role::Alu => &["222 222", "2122212", " 21412 ", " 24442 ", " 21412 ", "  222  "],
        Role::Rom => &["2222222", "2414142", "2141412", "2414142", "2222222"],
        Role::Sram => &["2222222", "2424242", "2222222", "2424242", "2222222"],
        Role::Timer => &[" 22222 ", "2111112", "2114112", "2114442", "2111112", " 22222 "],
        Role::Clock => &[" 22    ", "2  2   ", "    2 2", "     2 "],
        Role::Gpio => &["4 4 4 4", "4 4 4 4", "2222222", "2111112", "2222222"],
        Role::Adc => &["      2", "    222", "    2  ", "  222  ", "  2    ", "222    "],
        Role::Watchdog => &["  222  ", " 21112 ", "2114112", " 21112 ", "  222  "],
        Role::Supply => &["   55", "  55 ", " 5555", "  55 ", " 55  ", "55   "],
    }
}

/// The module's glyph, drawn in its signal tone.
pub fn module(p: &Painter, r: Rect, role: Role) -> Rect {
    let tone = match role {
        Role::Core | Role::Alu => Color32::from_rgb(200, 150, 240),
        Role::Rom | Role::Sram => Color32::from_rgb(120, 170, 240),
        Role::Timer | Role::Clock | Role::Watchdog => Color32::from_rgb(220, 205, 110),
        Role::Gpio | Role::Adc => Color32::from_rgb(130, 215, 150),
        Role::Supply => icons::GOLD,
    };
    paint_rows(p, r, glyph(role), tone)
}

pub fn die_colour(d: Die) -> Color32 {
    match d {
        Die::Dead => DEAD,
        Die::Slow => SLOW,
        Die::Bin(g) => grade_colour(g),
    }
}

/// A wafer disc with its flat, and dies at `sites` (centres and side in
/// wafer radii) coloured by `fill`.
pub fn wafer(p: &Painter, r: Rect, side: f32, sites: impl Iterator<Item = ((f32, f32), Color32)>) {
    let c = r.center();
    let radius = r.width().min(r.height()) * 0.5 - 1.0;
    // The disc with its orientation flat at the bottom, 6% of the radius in.
    let a0 = 0.94f32.asin();
    let span = std::f32::consts::PI + 2.0 * a0;
    let outline: Vec<Pos2> = (0..=48).map(|i| a0 - span * i as f32 / 48.0).map(|a| c + vec2(a.cos(), a.sin()) * radius).collect();
    p.add(egui::Shape::convex_polygon(outline.clone(), WAFER, Stroke::NONE));
    let px = |(x, y): (f32, f32)| -> Pos2 { c + vec2(x, y) * radius };
    let size = (side * radius - 1.0).max(1.0);
    for (at, colour) in sites {
        p.rect_filled(Rect::from_center_size(px(at), vec2(size, size)), 0, colour);
    }
    p.add(egui::Shape::closed_line(outline, Stroke::new(1.0, icons::PAPER.gamma_multiply(0.6))));
}

/// Dies before probe: every site in the unprobed tone.
pub fn unprobed(sites: &[(f32, f32)]) -> impl Iterator<Item = ((f32, f32), Color32)> + '_ {
    sites.iter().map(|&s| (s, UNPROBED))
}

/// Lab supply sprite and tone: silicon stages, wafer and mask, bottled
/// chemistry, sputter targets and hood filters.
fn supply_glyph(s: Supply) -> (&'static [&'static str], Color32) {
    match s {
        Supply::Crude => (
            &["  3333   ", " 322223  ", "32242213 ", "32222213 ", " 3211123 ", "  331133 ", "   3333  "],
            Color32::from_rgb(142, 118, 96),
        ),
        Supply::Pure => (
            &["  33     ", " 3243    ", " 32243   ", "  32243  ", "   32213 ", "    3213 ", "     333 "],
            Color32::from_rgb(190, 196, 206),
        ),
        Supply::Ingot => (
            &["   333   ", "  34423  ", " 3242213 ", " 3222213 ", " 3222213 ", " 3211113 ", "  33333  "],
            Color32::from_rgb(168, 180, 204),
        ),
        Supply::Wafer => (
            &["  33333  ", " 3244223 ", "32422223 ", "32222213 ", "32222113 ", " 3211113 ", "  33333  "],
            Color32::from_rgb(118, 132, 164),
        ),
        Supply::Mask => (
            &["333333333", "342222213", "322222213", "322222213", "322222213", "311111113", "333333333"],
            Color32::from_rgb(146, 190, 210),
        ),
        Supply::Resist => (
            &["   333   ", "   323   ", "  33333  ", " 3242223 ", " 3222213 ", " 3222213 ", " 3211113 ", "  33333  "],
            Color32::from_rgb(214, 150, 70),
        ),
        Supply::Solvent => (
            &["   333   ", "   323   ", "  33333  ", " 3244423 ", " 3222213 ", " 3222213 ", " 3211113 ", "  33333  "],
            Color32::from_rgb(118, 190, 200),
        ),
        Supply::Dopant => (
            &["333333333", "322242223", "322444223", "322242223", "322222223", "311111113", "333333333"],
            Color32::from_rgb(172, 122, 204),
        ),
        Supply::Gas => (
            &["   33    ", "  3323   ", "  3333   ", " 324223  ", " 322223  ", " 322223  ", " 321113  ", "  3333   "],
            Color32::from_rgb(120, 200, 140),
        ),
        Supply::Metal => (
            &["  33333  ", " 3444423 ", "322222213", "322222213", "311111113", " 3111113 ", "  33333  "],
            Color32::from_rgb(204, 160, 110),
        ),
        Supply::Filter => (
            &["333333333", "324242423", "342424243", "324242423", "342424243", "311111113", "333333333"],
            Color32::from_rgb(160, 170, 150),
        ),
    }
}

/// A lab supply's sprite in its own tone.
pub fn supply(p: &Painter, r: Rect, s: Supply) -> Rect {
    let (rows, tone) = supply_glyph(s);
    paint_rows(p, r, rows, tone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supply_sprite_obeys_the_pixel_grid() {
        for s in Supply::ALL {
            let (rows, _) = supply_glyph(s);
            let label = s.name();
            assert!(rows.len() <= 9, "{label} is taller than 9 px");
            assert!(rows.iter().all(|row| row.len() <= 9), "{label} is wider than 9 px");
            assert!(rows.iter().any(|row| row.contains('2')), "{label} has no body role");
            assert!(rows.iter().any(|row| row.contains('3')), "{label} has no outline role");
            for row in rows {
                for ch in row.bytes() {
                    assert!(
                        ch == b' ' || super::super::pixel_art::glyph(ch, icons::PAPER).is_some(),
                        "{label} contains unknown glyph {ch}"
                    );
                }
            }
        }
    }
}
