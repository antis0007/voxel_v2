use super::*;

fn view() -> View {
    View { eye: Vec3::ZERO, forward: Vec3::NEG_Z, right: Vec3::X, up: Vec3::Y, time: 0.3, sway: glam::Vec2::ZERO, gait: 0.0, speed: 0.0, clip: Default::default(), lowered: 0.0, torch_lit: true }
}

fn closed(m: &Model) -> Vec3 {
    m.vertices.chunks_exact(3).fold(Vec3::ZERO, |acc, t| {
        let [a, b, c] = [t[0].pos, t[1].pos, t[2].pos].map(Vec3::from_array);
        acc + (b - a).cross(c - a)
    })
}

#[test]
fn blocks_and_items_draw_closed_geometry_low_on_the_right() {
    for item in [Item::Block(BlockId::Stone), Item::Block(BlockId::Torch), Item::Food(crate::world::inventory::FoodKind::RoastedCap), Item::Weapon(1), Item::Resource(2)] {
        let mut m = Model::new(1.0);
        draw(&mut m, item, &view());
        assert!(!m.vertices.is_empty(), "{item:?} drew nothing");
        assert!(closed(&m).length() < 1e-5, "{item:?} is not closed");
        for vertex in &m.vertices {
            let p = Vec3::from_array(vertex.pos);
            // The torch flame rises above the fist by design (see the lift test below).
            let flame = vertex.surface[3] < -1.5;
            // A blade held ready points up past the eye line; only its grip sits low.
            let blade = matches!(item, Item::Weapon(_));
            assert!(p.x > 0.05 && (p.y < 0.0 || flame || blade) && p.z < -0.3 && p.length() < 0.9, "{item:?} vertex {p}");
            let n = Vec3::from_array(vertex.normal);
            assert!((n.length() - 1.0).abs() < 1e-4);
        }
    }
}

#[test]
fn held_torch_flame_is_opaque_flat_colour() {
    let mut m = Model::new(1.0);
    draw(&mut m, Item::Block(BlockId::Torch), &view());
    let flame: Vec<_> = m.vertices.iter().filter(|v| v.surface[3] < -1.5).collect();
    assert!(flame.len() >= 24, "the flame is solid boxes, not a billboard");
    for v in flame {
        let c = v.color;
        assert!(c[0] > 0.9 && c[1] > 0.2 && c[2] < 0.5, "flat fire colour {c:?}");
    }
}

#[test]
fn held_torch_stick_passes_through_the_fist() {
    let mut m = Model::new(1.0);
    draw(&mut m, Item::Block(BlockId::Torch), &view());
    // The rig's fist closes `TORCH_SEAT` away from the placement point.
    let grip = REST + TORCH_SEAT + torch_tilt() * Vec3::new(0.0, -0.085, 0.0);
    let stick = m.vertices.iter().filter(|v| v.surface[3] > 0.0).map(|v| Vec3::from_array(v.pos) - grip);
    let (mut below, mut above) = (false, false);
    for d in stick { below |= d.y < -0.05; above |= d.y > 0.05; }
    assert!(below && above, "the stick must pass through the fist");
}

#[test]
fn held_torch_light_sits_at_the_opaque_flame() {
    let mut m = Model::new(1.0);
    draw(&mut m, Item::Block(BlockId::Torch), &view());
    let flame = torch_flame_offset();
    let mean = m.vertices.iter().filter(|v| v.surface[3] < -1.5)
        .fold(Vec3::ZERO, |a, v| a + Vec3::from_array(v.pos));
    let count = m.vertices.iter().filter(|v| v.surface[3] < -1.5).count();
    let mean = mean / count as f32;
    assert!((Vec3::new(flame.x, flame.y, -flame.z) - mean).length() < 0.2, "light {flame} vs flame {mean}");
}

#[test]
fn wands_are_left_to_the_wand_viewmodel() {
    let mut m = Model::new(1.0);
    draw(&mut m, Item::Wand { id: 0, look: 1 }, &view());
    assert!(m.vertices.is_empty());
}

#[test]
fn lowering_moves_the_item_down_out_of_view() {
    let mut a = Model::new(1.0);
    draw(&mut a, Item::Block(BlockId::Stone), &view());
    let mut b = Model::new(1.0);
    draw(&mut b, Item::Block(BlockId::Stone), &View { lowered: 1.0, ..view() });
    let top = |m: &Model| m.vertices.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
    assert!(top(&b) < top(&a) - 0.3);
}

#[test]
fn held_block_fits_inside_the_widescreen_frame() {
    let mut mesh = Model::new(1.0);
    draw(&mut mesh, Item::Block(BlockId::Stone), &view());
    let half_fov = 36.0f32.to_radians().tan();
    for vertex in mesh.vertices {
        let p = Vec3::from_array(vertex.pos);
        let depth = -p.z;
        assert!(depth > 0.0);
        let ndc_y = p.y / depth / half_fov;
        let ndc_x = p.x / depth / (half_fov * (16.0 / 9.0));
        assert!((-1.0..=1.0).contains(&ndc_y), "vertical clip: {ndc_y}");
        assert!((-1.0..=1.0).contains(&ndc_x), "horizontal clip: {ndc_x}");
    }
}

#[test]
fn mining_moves_the_held_item_without_changing_its_topology() {
    let mut rest = Model::new(1.0);
    let mut strike = Model::new(1.0);
    draw(&mut rest, Item::Block(BlockId::Stone), &view());
    let chop = crate::player::viewmodel::clips::Name::Chop.clip();
    let clip = chop.sample(chop.impact.unwrap_or(0.0));
    draw(&mut strike, Item::Block(BlockId::Stone), &View { clip, ..view() });
    assert_eq!(rest.vertices.len(), strike.vertices.len());
    assert!(rest.vertices.iter().zip(&strike.vertices).any(|(a, b)| a.pos != b.pos));
}

use crate::player::viewmodel::clips::Name;

/// Tip minus grip in camera-local space (right, up, back) at `t` seconds into `name`.
fn blade(name: Name, t: f32) -> Vec3 { (name.clip().sample(t).rotation * blade_rest()) * Vec3::Y }

#[test]
fn stroke_contact_lays_the_blade_across_the_view() {
    for (name, side) in [(Name::Forehand, -1.0), (Name::Backhand, 1.0)] {
        let b = blade(name, name.clip().impact.unwrap_or(0.0));
        assert!(b.x * side > 0.8, "{name:?} contact should point the tip across the screen: {b}");
        assert!(b.z < 0.0, "{name:?} tip should still lead forward: {b}");
    }
    let b = blade(Name::Overhead, Name::Overhead.clip().impact.unwrap_or(0.0));
    assert!(b.y < -0.7, "overhead contact should point the tip down the view: {b}");
}

#[test]
fn stroke_wind_up_raises_the_tip_above_the_hand() {
    for name in [Name::Forehand, Name::Backhand, Name::Overhead] {
        let load = name.clip().keys[1].t;
        assert!(blade(name, load).y > 0.7, "{name:?}");
    }
}

#[test]
fn backhand_rises_over_the_off_hand() {
    let clip = Name::Backhand.clip();
    let impact = clip.impact.unwrap_or(0.0);
    for t in [impact, impact + 0.02, impact + 0.035] {
        let p = clip.sample(t);
        let b = blade(Name::Backhand, t);
        assert!(b.y > 0.2 && p.offset.y > 0.05, "backhand at {t} should stay above the torch: {b} {p:?}");
    }
}

#[test]
fn overhead_lifts_higher_than_slashes() {
    let top = |name: Name| (0..=60).map(|i| name.clip().sample(i as f32 * 0.008).offset.y).fold(f32::MIN, f32::max);
    assert!(top(Name::Overhead) > top(Name::Forehand));
}

#[test]
fn every_clip_moves_held_items_without_changing_topology() {
    for name in Name::ALL {
        let clip = name.clip();
        for item in [Item::Block(BlockId::Stone), Item::Block(BlockId::Torch), Item::Weapon(2), Item::Resource(2)] {
            let mut rest = Model::new(1.0);
            draw(&mut rest, item, &view());
            for i in 0..=12 {
                let mut posed = Model::new(1.0);
                draw(&mut posed, item, &View { clip: clip.sample(clip.duration * i as f32 / 12.0), ..view() });
                assert_eq!(rest.vertices.len(), posed.vertices.len(), "{name:?} {item:?}");
                assert!(posed.vertices.iter().all(|v| Vec3::from_array(v.pos).is_finite()));
            }
        }
    }
}
