//! The selected hotbar item held low on the right: blocks as textured cubes,
//! other items as their hotbar sprite extruded one texel deep.
//! Texels come from `egui_ui::held_art`, so the hand always matches the slot.
use super::{Model, Vertex3};
use crate::egui_ui::held_art::{self, Art, FACE};
use crate::world::inventory::Item;
use crate::world::{BlockId, World};
use glam::{Mat3, Quat, Vec3};

/// Viewmodel material: dims with `ao_strength` under a roof like the hand and wand.
const KIND: f32 = 55.0;
const GLOW_KIND: f32 = 40.0;
const CUBE_EDGE: f32 = 0.14;
const SPRITE_TEXEL: f32 = 0.016;
/// Hold point in camera space: right, up, forward (metres).
const REST: Vec3 = Vec3::new(0.34, -0.30, 0.60);

pub struct View {
    pub eye: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub time: f32,
    /// Camera-lag offset in metres (right, up), from the viewmodel spring.
    pub sway: glam::Vec2,
    /// Walk cycle phase in radians and ground speed in m/s.
    pub gait: f32,
    pub speed: f32,
    /// 0..1 pulse after placing or using.
    pub interact: f32,
    /// 0 held, 1 fully below the screen (while a wand is drawn).
    pub lowered: f32,
}

/// Review captures pin the held item: `VOXEL_REVIEW_HELD=stone|grass|planks|crystal|torch|food|sword|gem`.
static REVIEW: std::sync::LazyLock<Option<Item>> = std::sync::LazyLock::new(|| {
    if !std::env::args().any(|a| a.starts_with("--benchmark-")) { return None; }
    Some(match std::env::var("VOXEL_REVIEW_HELD").ok()?.as_str() {
        "stone" => Item::Block(BlockId::Stone),
        "grass" => Item::Block(BlockId::Grass),
        "planks" => Item::Block(BlockId::Planks),
        "crystal" => Item::Block(BlockId::Crystal),
        "torch" => Item::Block(BlockId::Torch),
        "food" => Item::Food(crate::world::inventory::FoodKind::RoastedCap),
        "sword" => Item::Weapon(2),
        "gem" => Item::Resource(6),
        _ => return None,
    })
});

/// A review capture pinned the held item, overriding any wand in the hand.
pub(crate) fn pinned() -> bool {
    REVIEW.is_some()
}

/// Item in the hand: the survival hotbar slot, or the palette block while flying in creative.
pub(crate) fn held(world: &World, player: &crate::player::Player) -> Option<Item> {
    if let Some(item) = *REVIEW {
        return Some(item);
    }
    if player.flying {
        let block = player.selected_block;
        return (block != BlockId::Air).then_some(Item::Block(block));
    }
    world.inventory.hotbar.get(world.adventure.hand_slot).copied().flatten().map(|stack| stack.item)
}

/// Whether the hand holds a torch, for the moving torch light.
pub(crate) fn holds_torch(world: &World, player: &crate::player::Player) -> bool {
    (world.adventure.wand == 0 || pinned()) && held(world, player).is_some_and(|item| item.placeable_block().is_some_and(|b| b.is_torch()))
}

/// Camera-space flame position of a held torch (right, up, forward), in metres.
pub(crate) fn torch_flame_offset() -> Vec3 {
    REST + Vec3::Y * 0.09
}

fn frame(v: &View) -> (Vec3, Quat) {
    let walk = (v.speed / 4.0).clamp(0.0, 1.0);
    let bob = Vec3::new(v.gait.cos() * 0.006, -(v.gait * 2.0).sin().abs() * 0.010, 0.0) * walk;
    // Placing or using pokes the item forward and down.
    let poke = v.interact.clamp(0.0, 1.0);
    let local = REST + v.sway.extend(0.0) + bob
        + Vec3::new(-0.03 * poke, -0.03 * poke - v.lowered.clamp(0.0, 1.0) * 0.35, 0.06 * poke);
    let centre = v.eye + v.right * local.x + v.up * local.y + v.forward * local.z;
    let camera = Quat::from_mat3(&Mat3::from_cols(v.right, v.up, -v.forward));
    (centre, camera * Quat::from_rotation_x(-0.4 * poke))
}

struct Out<'a> {
    m: &'a mut Model,
    centre: Vec3,
    rotation: Quat,
}

impl Out<'_> {
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: [f32; 3], glow: f32) {
        let world = corners.map(|c| (self.centre + self.rotation * c) / self.m.scale);
        let n = self.rotation * normal;
        let order = if (world[1] - world[0]).cross(world[2] - world[0]).dot(n) > 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
        let surface = if glow > 0.0 { [0.48, 0.42, GLOW_KIND, glow] } else { [0.48, 0.42, KIND, 0.2] };
        for i in order {
            self.m.vertices.push(Vertex3 { pos: world[i].to_array(), color, normal: n.to_array(), surface, ao_strength: 1.0 });
        }
    }
}

fn block_glow(block: BlockId) -> f32 {
    if matches!(block, BlockId::Crystal | BlockId::GlowCap | BlockId::Lava) { 1.2 } else { 0.0 }
}

fn cube(out: &mut Out, block: BlockId) {
    let h = CUBE_EDGE * 0.5;
    let s = CUBE_EDGE / FACE as f32;
    let glow = block_glow(block);
    // (normal, texel column axis, texel row axis): rows run down the sides and front to back on top.
    let faces = [
        (Vec3::Z, Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::NEG_Y),
        (Vec3::X, Vec3::NEG_Z, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::NEG_Y),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::NEG_Z),
    ];
    for (n, u, w) in faces {
        let top = n.y != 0.0;
        let origin = n * h - u * h - w * h;
        for y in 0..FACE {
            for x in 0..FACE {
                let p = |dx: u32, dy: u32| origin + u * ((x + dx) as f32 * s) + w * ((y + dy) as f32 * s);
                out.quad([p(0, 0), p(1, 0), p(1, 1), p(0, 1)], n, held_art::block_texel(block, top, x, y), glow);
            }
        }
    }
}

fn linear(v: u8) -> f32 {
    let s = v as f32 / 255.0;
    if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
}

fn sprite(out: &mut Out, icon: crate::egui_ui::icons::Icon, flame: f32) {
    let (w, h, texels) = held_art::icon_texels(icon);
    let s = SPRITE_TEXEL;
    let at = |x: i32, y: i32| (x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h).then(|| texels[y as usize * w + x as usize]).flatten();
    let point = |x: f32, y: f32, z: f32| Vec3::new((x - w as f32 * 0.5) * s, (h as f32 * 0.5 - y) * s, z * s);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let Some((ch, rgb)) = at(x, y) else { continue };
            let color = rgb.map(linear);
            let glow = if flame > 0.0 && matches!(ch, b'4' | b'5' | b'6') { flame } else { 0.0 };
            let (x0, x1, y0, y1) = (x as f32, x as f32 + 1.0, y as f32, y as f32 + 1.0);
            for z in [0.5f32, -0.5] {
                out.quad([point(x0, y1, z), point(x1, y1, z), point(x1, y0, z), point(x0, y0, z)], Vec3::Z * z.signum(), color, glow);
            }
            for (dx, dy, n) in [(-1, 0, Vec3::NEG_X), (1, 0, Vec3::X), (0, -1, Vec3::Y), (0, 1, Vec3::NEG_Y)] {
                if at(x + dx, y + dy).is_some() { continue; }
                let (a, b) = if dx != 0 {
                    let ex = if dx > 0 { x1 } else { x0 };
                    ((ex, y0), (ex, y1))
                } else {
                    let ey = if dy > 0 { y1 } else { y0 };
                    ((x0, ey), (x1, ey))
                };
                out.quad([point(a.0, a.1, -0.5), point(b.0, b.1, -0.5), point(b.0, b.1, 0.5), point(a.0, a.1, 0.5)], n, color, glow);
            }
        }
    }
}

/// Appends the held item. Wands draw through `wand::draw`, so they are skipped here.
pub(super) fn draw(m: &mut Model, item: Item, v: &View) {
    let (centre, camera) = frame(v);
    match held_art::art(item) {
        Art::Wand(_) => {}
        Art::Block(block) => {
            // Turned and tipped so the top and two sides read, as a held cube does.
            let rotation = camera * Quat::from_rotation_x(0.35) * Quat::from_rotation_y(-0.62);
            cube(&mut Out { m, centre, rotation }, block);
        }
        Art::Icon(icon) => {
            let torch = item.placeable_block().is_some_and(|b| b.is_torch());
            let flame = if torch { 1.2 + 0.25 * (v.time * 11.0).sin() + 0.15 * (v.time * 17.3).sin() } else { 0.0 };
            // Faces the screen centre and leans in, like an item gripped at its base.
            let rotation = camera * Quat::from_rotation_y(-0.55) * Quat::from_rotation_z(if torch { 0.12 } else { 0.25 });
            sprite(&mut Out { m, centre, rotation }, icon, flame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View { eye: Vec3::ZERO, forward: Vec3::NEG_Z, right: Vec3::X, up: Vec3::Y, time: 0.3, sway: glam::Vec2::ZERO, gait: 0.0, speed: 0.0, interact: 0.0, lowered: 0.0 }
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
                assert!(p.x > 0.05 && p.y < 0.0 && p.z < -0.3 && p.length() < 0.9, "{item:?} vertex {p}");
                let n = Vec3::from_array(vertex.normal);
                assert!((n.length() - 1.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn held_torch_flame_glows_and_stone_does_not() {
        let glows = |item| {
            let mut m = Model::new(1.0);
            draw(&mut m, item, &view());
            m.vertices.iter().any(|v| v.surface[2] == GLOW_KIND)
        };
        assert!(glows(Item::Block(BlockId::Torch)));
        assert!(!glows(Item::Block(BlockId::Stone)));
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
}
