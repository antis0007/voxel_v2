//! Authored held kit, animated from the simulation clock.
use super::Model;
use super::wand_pose::{pose, RIG};
use crate::wand_style::WandStyle;
use glam::{Mat3, Quat, Vec3};
use std::sync::LazyLock;

pub struct WandView {
    pub eye: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub time: f32,
    pub cast: f32,
    pub spell: [f32; 3],
    /// Smoothed traversal impulse shared with the camera.
    pub boost: f32,
    pub motion: crate::player::viewmodel::State,
}

pub(crate) fn book(m: &mut Model, center: Vec3, rotation: Quat, color: [f32; 3], glow: f32) {
    let part = |m: &mut Model, offset: Vec3, size: Vec3, tint, emission| {
        m.cuboid(center + rotation * offset, size, rotation, tint, emission);
    };
    part(m, Vec3::ZERO, Vec3::new(0.29, 0.22, 0.045), [0.19, 0.10, 0.28], 0.0);
    part(m, Vec3::new(0.0, 0.0, 0.022), Vec3::new(0.014, 0.213, 0.019),
        [0.34, 0.20, 0.36], 0.0);
    part(m, Vec3::new(0.083, -0.105, 0.029), Vec3::new(0.019, 0.037, 0.009),
        [0.55, 0.19, 0.24], 0.0);
    for x in [-0.069, 0.069] {
        part(m, Vec3::new(x, 0.0, 0.025), Vec3::new(0.126, 0.196, 0.018),
            [0.76, 0.67, 0.48], 0.0);
        for (y, width) in [(-0.070, 0.048), (-0.055, 0.071), (0.055, 0.071), (0.070, 0.054)] {
            part(m, Vec3::new(x, y, 0.035), Vec3::new(width, 0.004, 0.003),
                [0.27, 0.17, 0.24], 0.0);
        }
    }
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        part(m, Vec3::new(-0.069 + 0.026 * angle.cos(), 0.026 * angle.sin(), 0.036),
            Vec3::new(0.009, 0.009, 0.004), color, glow);
    }
    for (y, width) in [(-0.023, 0.034), (0.0, 0.064), (0.023, 0.049)] {
        part(m, Vec3::new(0.069, y, 0.036), Vec3::new(width, 0.006, 0.004), color, glow);
    }
}

pub(super) fn draw_grimoire(m: &mut Model, view: &WandView) {
    let rotation = Quat::from_mat3(&Mat3::from_cols(view.right, view.up, -view.forward));
    let center = view.eye + view.forward * (0.48 + view.cast * 0.045)
        + view.right * 0.16 - view.up * 0.18;
    book(m, center, rotation, view.spell, 0.35 + view.cast * 1.5);
}

pub fn tip_light(style: WandStyle, v: &WandView) -> (Vec3, [f32; 3], f32) {
    let p = pose(v);
    let tip=if super::wand_sprite::enabled() {super::wand_sprite::muzzle(style)}
        else {Vec3::from_array(crate::viewmodel_review::current().muzzle)};
    (super::wand_pose::present(v,super::wayfarer::attachment(v).transform_point3(tip)), style.clamped().glow_colour(v.spell), p.cast * 2.25)
}

pub fn draw(m: &mut Model, style: WandStyle, v: &WandView) {
    let style = style.clamped();
    let p = pose(v);
    let glow = style.glow_colour(v.spell);
    let attachment=super::wayfarer::attachment(v);
    let point=|local:Vec3| attachment.transform_point3(local);


    static STUDIO: LazyLock<bool> = LazyLock::new(|| {
        std::env::args().any(|arg| arg.starts_with("--benchmark-"))
            && std::env::var("VOXEL_VIEWMODEL_STUDIO").is_ok_and(|s| s == "1")
    });
    if *STUDIO {
        m.with_surface_kind(super::AUTHORED_SURFACE, |m| {
            m.cuboid(v.eye + v.forward * 1.5, Vec3::new(6.0, 6.0, 0.02),
                Quat::from_mat3(&Mat3::from_cols(v.right, v.up, -v.forward)),
                [0.32, 0.35, 0.37], 0.0);
        });
    }
    let start=m.vertices.len();
    super::wayfarer::draw(m, v, style);

    // Authored families light their own emitter texels; only the review GLBs,
    // which have no emitter material, keep a compact release at the socket.
    if p.cast > 0.04 && !super::wand_sprite::enabled() {
        let tip=Vec3::from_array(crate::viewmodel_review::current().muzzle);
        m.glow_voxel(point(tip), Vec3::splat(0.008*RIG.scale*(0.4+p.cast)),
            p.rotation, glow, p.cast*1.6, 1.0);
    }
    if RIG.left_handed || (RIG.fov_degrees-72.0).abs()>0.001 {
        let focal=36.0f32.to_radians().tan()/(RIG.fov_degrees.clamp(40.0,110.0)*0.5).to_radians().tan();
        let handed=if RIG.left_handed {-1.0} else {1.0};
        for vertex in &mut m.vertices[start..] {
            let position=Vec3::from_array(vertex.pos);
            let q=position-v.eye/m.scale;
            vertex.pos=(position+v.right*q.dot(v.right)*(focal*handed-1.0)+v.up*q.dot(v.up)*(focal-1.0)).to_array();
            if RIG.left_handed {
                let n=Vec3::from_array(vertex.normal);
                vertex.normal=(n-2.0*v.right*n.dot(v.right)).to_array();
            }
        }
    }
    if RIG.left_handed {for triangle in m.vertices[start..].chunks_exact_mut(3) {triangle.swap(1,2);}}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewmodel_stays_inside_a_readable_first_person_bounds() {
        let mut mesh = Model::new(1.0);
        draw(
            &mut mesh,
            WandStyle::default(),
            &WandView {
                eye: Vec3::ZERO,
                forward: Vec3::NEG_Z,
                right: Vec3::X,
                up: Vec3::Y,
                time: 0.0,
                cast: 0.0,
                spell: [0.3, 0.9, 0.5],
                boost: 0.0, motion: Default::default(),
            },
        );
        assert!(!mesh.vertices.is_empty());
        assert!(
            mesh.vertices.iter().any(|v| v.surface[2] == 55.0),
            "wand body must use the solid wand material"
        );
        assert!(
            mesh.vertices.iter().any(|v| v.surface[2] == 54.0),
            "hand must use the skin/leather material"
        );
        let farthest = mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.pos).length())
            .fold(0.0f32, f32::max);
        assert!(
            farthest < 1.35,
            "first-person wand reached {farthest:.2} m from the eye"
        );
    }
}
