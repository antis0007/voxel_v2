//! Stylized fluid body for a tank's fill level.
//!
//! The body is a box of fluid inside the tank's inner walls: a lit top face at
//! the surface and the four sides down to the floor. Colours follow the shared
//! pixel palette's shading roles (`assets/pixel_palette.txt`): the top lifts
//! toward the highlight by `top_light_mix`, and the sides are darkened by the
//! palette's `shade` values, so the fluid matches the pixel-art surfaces
//! around it instead of looking like a separate rendering style.
//!
//! Output is plain quads in tank-local voxel units. The mesher turns them into
//! vertices; keeping this layer free of GPU types makes it testable.

use super::catalog;
use super::containers::FluidKind;

/// Palette roles, as sRGB hex from `assets/pixel_palette.txt`.
const HIGHLIGHT: [f32; 3] = rgb(0xf7, 0xe4, 0x9f);
/// Palette `shade` and `top_light_mix`.
const SIDE_SHADE: f32 = 0.62;
const TOP_LIGHT_MIX: f32 = 0.20;
/// The z-facing sides sit between the x-facing sides and the lit top.
const SIDE_Z_SHADE: f32 = 0.76;

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// Body colour for a fluid, from the catalog.
pub fn body_color(kind: FluidKind) -> [f32; 3] {
    catalog::spec(kind).color
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    /// Corners wound counter-clockwise when seen from `normal`.
    pub corners: [[f32; 3]; 4],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

/// The tank's inner box in voxel units: the fluid may occupy `min..max` less
/// `wall` on the horizontal axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interior {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub wall: f32,
}

/// Fluid body quads for a tank `fraction` full (clamped to `[0, 1]`). Empty
/// tanks produce no geometry, and a full tank's top sits just under the rim.
pub fn fluid_body(kind: FluidKind, fraction: f32, interior: Interior) -> Vec<Quad> {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.0 {
        return Vec::new();
    }
    let base = body_color(kind);
    let top_color = mix(base, HIGHLIGHT, TOP_LIGHT_MIX);
    let x0 = interior.min[0] + interior.wall;
    let x1 = interior.max[0] - interior.wall;
    let z0 = interior.min[2] + interior.wall;
    let z1 = interior.max[2] - interior.wall;
    let floor = interior.min[1];
    let height = interior.max[1] - interior.min[1];
    let level = floor + height * fraction;

    let top = [
        [x0, level, z0],
        [x0, level, z1],
        [x1, level, z1],
        [x1, level, z0],
    ];
    let mut quads = vec![oriented(top, [0.0, 1.0, 0.0], top_color)];
    let x_side = base.map(|c| c * SIDE_SHADE);
    let z_side = base.map(|c| c * SIDE_Z_SHADE);
    for (x, sign) in [(x1, 1.0), (x0, -1.0)] {
        let corners = [
            [x, floor, z0],
            [x, floor, z1],
            [x, level, z1],
            [x, level, z0],
        ];
        quads.push(oriented(corners, [sign, 0.0, 0.0], x_side));
    }
    for (z, sign) in [(z1, 1.0), (z0, -1.0)] {
        let corners = [
            [x0, floor, z],
            [x1, floor, z],
            [x1, level, z],
            [x0, level, z],
        ];
        quads.push(oriented(corners, [0.0, 0.0, sign], z_side));
    }
    quads
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Reverse the winding if needed so the quad faces `normal`.
fn oriented(corners: [[f32; 3]; 4], normal: [f32; 3], color: [f32; 3]) -> Quad {
    let a = sub(corners[1], corners[0]);
    let b = sub(corners[2], corners[0]);
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let facing = cross[0] * normal[0] + cross[1] * normal[1] + cross[2] * normal[2];
    let corners = if facing < 0.0 {
        [corners[0], corners[3], corners[2], corners[1]]
    } else {
        corners
    };
    Quad {
        corners,
        normal,
        color,
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tank() -> Interior {
        Interior {
            min: [0.0, 0.0, 0.0],
            max: [3.0, 3.0, 3.0],
            wall: 0.1,
        }
    }

    fn top_y(quads: &[Quad]) -> f32 {
        quads
            .iter()
            .find(|q| q.normal == [0.0, 1.0, 0.0])
            .expect("a filled body has a top")
            .corners[0][1]
    }

    #[test]
    fn empty_tank_has_no_fluid_geometry() {
        assert!(fluid_body(FluidKind::Water, 0.0, tank()).is_empty());
        assert!(fluid_body(FluidKind::Water, -1.0, tank()).is_empty());
    }

    #[test]
    fn filled_body_is_a_top_and_four_sides() {
        let quads = fluid_body(FluidKind::Water, 0.5, tank());
        assert_eq!(quads.len(), 5);
        assert_eq!(quads.iter().filter(|q| q.normal[1] > 0.0).count(), 1);
    }

    #[test]
    fn surface_height_tracks_fill_fraction() {
        for f in [0.1_f32, 0.25, 0.5, 0.9] {
            let y = top_y(&fluid_body(FluidKind::Water, f, tank()));
            assert!((y - 3.0 * f).abs() < 1e-5, "fraction {f} gave top {y}");
        }
    }

    #[test]
    fn full_tank_top_is_clamped_to_the_rim() {
        let y = top_y(&fluid_body(FluidKind::Water, 2.0, tank()));
        assert!((y - 3.0).abs() < 1e-5);
    }

    #[test]
    fn body_stays_inside_the_walls() {
        for q in fluid_body(FluidKind::BioEthanol, 0.7, tank()) {
            for c in q.corners {
                assert!(c[0] >= 0.1 - 1e-5 && c[0] <= 2.9 + 1e-5);
                assert!(c[2] >= 0.1 - 1e-5 && c[2] <= 2.9 + 1e-5);
                assert!(c[1] >= -1e-5 && c[1] <= 3.0 + 1e-5);
            }
        }
    }

    #[test]
    fn every_quad_faces_its_declared_normal() {
        for kind in FluidKind::ALL {
            for q in fluid_body(kind, 0.6, tank()) {
                let a = sub(q.corners[1], q.corners[0]);
                let b = sub(q.corners[2], q.corners[0]);
                let cross = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                let facing: f32 = (0..3).map(|i| cross[i] * q.normal[i]).sum();
                assert!(facing > 0.0, "{kind:?} quad {:?} faces away", q.normal);
            }
        }
    }

    #[test]
    fn top_is_lighter_than_the_sides_for_every_fluid() {
        for kind in FluidKind::ALL {
            let quads = fluid_body(kind, 0.5, tank());
            let top = quads.iter().find(|q| q.normal[1] > 0.0).unwrap().color;
            let side = quads.iter().find(|q| q.normal[0] != 0.0).unwrap().color;
            let lum = |c: [f32; 3]| c[0] + c[1] + c[2];
            assert!(lum(top) > lum(side), "{kind:?}: top must read as lit");
        }
    }

    #[test]
    fn fluids_have_distinct_body_colours() {
        let colours: Vec<_> = FluidKind::ALL.iter().map(|k| body_color(*k)).collect();
        for i in 0..colours.len() {
            for j in i + 1..colours.len() {
                assert_ne!(colours[i], colours[j]);
            }
        }
    }
}
