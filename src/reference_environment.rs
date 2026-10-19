//! A repeatable physical-metre art courtyard, saved as ordinary editable blocks.
use crate::{
    player::{FlyMode, Player},
    world::{BlockId, World},
};
use glam::{IVec3, Vec3};

struct Builder {
    scale: f32,
    edits: Vec<(IVec3, BlockId)>,
}
impl Builder {
    fn cuboid(&mut self, a: [f32; 3], b: [f32; 3], block: BlockId) {
        let a = (Vec3::from_array(a) / self.scale).floor().as_ivec3();
        let b = (Vec3::from_array(b) / self.scale).ceil().as_ivec3();
        for y in a.y..b.y {
            for z in a.z..b.z {
                for x in a.x..b.x {
                    self.edits.push((IVec3::new(x, y, z), block));
                }
            }
        }
    }
    fn ellipsoid(&mut self, c: Vec3, r: Vec3, block: BlockId) {
        let lo = ((c - r) / self.scale).floor().as_ivec3();
        let hi = ((c + r) / self.scale).ceil().as_ivec3();
        for y in lo.y..hi.y {
            for z in lo.z..hi.z {
                for x in lo.x..hi.x {
                    let p = IVec3::new(x, y, z);
                    if (((p.as_vec3() + Vec3::splat(0.5)) * self.scale - c) / r).length_squared()
                        < 1.0
                    {
                        self.edits.push((p, block));
                    }
                }
            }
        }
    }
    fn torch(&mut self, x: f32, y: f32, z: f32) {
        self.edits.push((
            (Vec3::new(x, y, z) / self.scale).floor().as_ivec3(),
            BlockId::Torch,
        ));
    }
}
fn river_x(z: f32) -> f32 {
    5.0 + (z * 0.065).sin() * 6.0
}
fn valley_height(x: f32, z: f32) -> f32 {
    let flank = ((x - river_x(z)).abs() - 10.0).max(0.0) / 29.0;
    let ridge = flank.min(1.35).powf(1.6) * (10.0 + 3.0 * (z * 0.09 + x * 0.05).sin());
    let upstream = (-z - 42.0).max(0.0) * 0.22;
    let edge = ((48.0 - x.abs()) / 8.0)
        .min((z + 80.0) / 8.0)
        .min((40.0 - z) / 8.0)
        .clamp(0.0, 1.0);
    let outer = 17.0 + (1.0 + ridge + upstream) * edge;
    let r = ((x / 23.0).powi(2) + ((z + 4.0) / 22.0).powi(2)).sqrt();
    let t = ((r - 0.8) / 0.4).clamp(0.0, 1.0);
    18.0 + (outer - 18.0) * t * t * (3.0 - 2.0 * t)
}
fn grove_tree(b: &mut Builder, x: f32, z: f32, h: f32, pine: bool) {
    let y = valley_height(x, z).floor();
    b.cuboid([x, y - 1.0, z], [x + 0.75, y + h, z + 0.75], BlockId::Log);
    if pine {
        for layer in 0..4 {
            let k = layer as f32;
            let r = 3.2 - k * 0.65;
            b.ellipsoid(
                Vec3::new(x + 0.3, y + h - 3.0 + k * 1.3, z + 0.3),
                Vec3::new(r, 1.7, r),
                BlockId::PineLeaves,
            );
        }
    } else {
        for (dx, dz, dy, r) in [
            (-1.2, 0.0, 0.0, 3.0),
            (1.0, 0.5, 0.5, 2.7),
            (0.0, -0.6, 1.8, 2.4),
        ] {
            b.ellipsoid(
                Vec3::new(x + dx, y + h + dy, z + dz),
                Vec3::new(r, r * 0.7, r),
                BlockId::Leaves,
            );
        }
    }
}
fn valley(b: &mut Builder) {
    let s = b.scale;
    for z in (-80.0 / s) as i32..(40.0 / s) as i32 {
        let wz = (z as f32 + 0.5) * s;
        let water = 17.0 + (-wz - 48.0).max(0.0) * 0.22;
        for x in (-48.0 / s) as i32..(48.0 / s) as i32 {
            let wx = (x as f32 + 0.5) * s;
            let channel = (wx - river_x(wz)).abs();
            let width = 2.2 + 0.5 * (wz * 0.11).cos();
            let land = valley_height(wx, wz);
            let top = if channel < width { water - 1.2 } else { land };
            for y in (15.0 / s) as i32..((land.max(water) + s) / s).ceil() as i32 {
                let wy = (y as f32 + 0.5) * s;
                let block = if wy < top - s {
                    if wy < top - 2.0 {
                        BlockId::Stone
                    } else {
                        BlockId::Dirt
                    }
                } else if wy < top {
                    if channel < width + 1.2 {
                        BlockId::Gravel
                    } else if land > 28.0 {
                        BlockId::Limestone
                    } else {
                        BlockId::Grass
                    }
                } else if channel < width && wy < water {
                    BlockId::Water
                } else {
                    BlockId::Air
                };
                b.edits.push((IVec3::new(x, y, z), block));
            }
        }
    }
    // Layered shoulders close the flat horizon while leaving a central spring pass.
    for (x,z,rx,ry,rz) in [(-34.0,-77.0,26.0,24.0,20.0),(35.0,-79.0,25.0,28.0,21.0),(-5.0,-99.0,27.0,34.0,16.0)] {
        b.ellipsoid(Vec3::new(x,16.0,z),Vec3::new(rx,ry,rz),BlockId::Limestone);
        b.ellipsoid(Vec3::new(x-2.0,15.0,z+3.0),Vec3::new(rx+1.0,ry*0.70,rz+1.0),BlockId::Grass);
    }
    // Woodland edges frame the open stream corridor without hiding the landmarks.
    for (x, z, h, pine) in [
        (-31.0, -10.0, 9.0, false),
        (-34.0, -25.0, 10.0, true),
        (-25.0, -37.0, 11.0, true),
        (-35.0, -48.0, 9.0, true),
        (-22.0, -59.0, 10.0, true),
        (-11.0, -69.0, 9.0, true),
        (29.0, -17.0, 9.0, false),
        (32.0, -32.0, 11.0, true),
        (24.0, -45.0, 10.0, true),
        (18.0, -62.0, 12.0, true),
        (35.0, -58.0, 9.0, true),
        (-32.0, 14.0, 8.0, false),
    ] {
        grove_tree(b, x, z, h, pine);
    }
    // A ruined watchtower and broken arch on the western shoulder.
    let y = valley_height(-22.0, -39.0).floor();
    b.cuboid(
        [-26.0, y - 1.0, -43.0],
        [-19.0, y + 1.0, -36.0],
        BlockId::Cobblestone,
    );
    b.cuboid(
        [-26.0, y + 1.0, -43.0],
        [-24.0, y + 10.0, -36.0],
        BlockId::WeatheredBricks,
    );
    b.cuboid(
        [-21.0, y + 1.0, -43.0],
        [-19.0, y + 7.0, -36.0],
        BlockId::WeatheredBricks,
    );
    b.cuboid(
        [-24.0, y + 7.0, -43.0],
        [-21.0, y + 9.0, -41.0],
        BlockId::StoneBricks,
    );
    b.torch(-23.0, y + 1.0, -38.0);
    // A narrow crossing upstream creates a second readable destination.
    let rz = -35.0;
    let rx = river_x(rz);
    b.cuboid(
        [rx - 5.0, 18.0, rz],
        [rx + 5.0, 19.0, rz + 2.0],
        BlockId::Planks,
    );
    for x in [rx - 4.5, rx + 4.5] {
        b.cuboid([x, 17.0, rz], [x + 0.5, 20.0, rz + 0.5], BlockId::Log);
        b.torch(x, 20.0, rz);
    }
    // Lichen boulders and meadow shrubs soften the shore and stone shoulder.
    for n in 0..32 {
        let z = -70.0 + n as f32 * 3.2;
        let side = if n % 2 == 0 { -1.0 } else { 1.0 };
        let x = river_x(z) + side * (8.0 + (n as f32 * 1.7).sin().abs() * 9.0);
        let y = valley_height(x, z);
        b.ellipsoid(
            Vec3::new(x, y, z),
            Vec3::new(1.4, 0.8, 1.1),
            if n % 3 == 0 {
                BlockId::Limestone
            } else {
                BlockId::Bush
            },
        );
    }
}
pub fn camera(player: &mut Player, scale: f32) {
    let eye = Vec3::new(29.0, 38.0, 46.0);
    let direction = (Vec3::new(-1.0, 21.0, -24.0) - eye).normalize();
    player.pos = (eye - Vec3::Y * 1.62) / scale;
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
    player.vel = Vec3::ZERO;
    player.flying = true;
    player.fly_mode = FlyMode::Noclip;
    player.crouch_t = 0.0;
    player.selected_block = BlockId::Stone;
}
pub fn install(world: &mut World) {
    let s = world.voxel_scale.meters();
    let mut b = Builder {
        scale: s,
        edits: Vec::new(),
    };
    valley(&mut b);
    // Consistent land and water elevations, independent of voxel resolution.
    for z in (-27.0 / s) as i32..(18.0 / s) as i32 {
        for x in (-23.0 / s) as i32..(23.0 / s) as i32 {
            let wx = (x as f32 + 0.5) * s;
            let wz = (z as f32 + 0.5) * s;
            let pond = (((wx - 5.0) / 7.0).powi(2) + ((wz - 1.0) / 9.0).powi(2))
                .min(((wx - river_x(wz)) / 2.4).powi(2));
            let top = valley_height(wx, wz);
            for y in (15.0 / s) as i32..((top + s).max(19.0) / s).ceil() as i32 {
                let wy = (y as f32 + 0.5) * s;
                let block = if pond < 1.0 {
                    if wy < 16.0 {
                        BlockId::Clay
                    } else if wy < 17.0 {
                        BlockId::Water
                    } else {
                        BlockId::Air
                    }
                } else if wy < top - s {
                    BlockId::Dirt
                } else if wy < top {
                    if pond < 1.3 {
                        BlockId::Gravel
                    } else {
                        BlockId::Grass
                    }
                } else {
                    BlockId::Air
                };
                b.edits.push((IVec3::new(x, y, z), block));
            }
        }
    }
    // A warm masonry cottage with a deep porch and readable gabled silhouette.
    b.cuboid(
        [-12.0, 18.0, -15.0],
        [-3.0, 19.0, -6.0],
        BlockId::Cobblestone,
    );
    b.cuboid(
        [-12.0, 19.0, -15.0],
        [-3.0, 23.0, -7.0],
        BlockId::StoneBricks,
    );
    b.cuboid([-11.0, 19.0, -14.0], [-4.0, 23.0, -8.0], BlockId::Air);
    b.cuboid([-8.0, 19.0, -9.0], [-6.0, 22.0, -7.0], BlockId::Air);
    b.cuboid([-12.0, 20.0, -12.0], [-10.0, 22.0, -10.0], BlockId::Air);
    b.cuboid([-5.0, 20.0, -12.0], [-3.0, 22.0, -10.0], BlockId::Air);
    for layer in 0..4 {
        let inset = layer as f32;
        b.cuboid(
            [-13.0 + inset, 23.0 + inset, -16.0],
            [-2.0 - inset, 24.0 + inset, -6.0],
            BlockId::Planks,
        );
    }
    for x in [-12.0, -4.0] {
        b.cuboid([x, 19.0, -7.0], [x + 1.0, 23.0, -6.0], BlockId::Log);
    }
    b.cuboid([-7.0, 19.0, -14.0], [-5.0, 20.0, -12.0], BlockId::Planks);
    b.torch(-6.5, 20.0, -12.5);
    b.torch(-11.5, 19.0, -6.5);
    b.torch(-3.5, 19.0, -6.5);
    // Path leads the eye over the pond to a cool stone sanctuary.
    b.cuboid([-8.0, 17.0, -6.0], [-5.0, 18.0, 6.0], BlockId::Cobblestone);
    b.cuboid([-5.0, 18.0, 2.0], [14.0, 19.0, 5.0], BlockId::Planks);
    for x in [-2.0, 3.0, 8.0, 13.0] {
        for z in [2.0, 4.0] {
            b.cuboid([x, 16.0, z], [x + 1.0, 20.0, z + 1.0], BlockId::Log);
        }
    }
    for x in [-1.5, 13.5] {
        b.torch(x, 20.0, 2.5);
    }
    b.ellipsoid(
        Vec3::new(12.0, 18.0, -14.0),
        Vec3::new(7.0, 8.0, 6.0),
        BlockId::Stone,
    );
    b.cuboid([9.0, 18.0, -13.0], [14.0, 22.0, -7.0], BlockId::Air);
    b.cuboid([8.0, 18.0, -9.0], [15.0, 19.0, -6.0], BlockId::StoneBricks);
    b.cuboid(
        [8.0, 19.0, -9.0],
        [9.0, 23.0, -7.0],
        BlockId::WeatheredBricks,
    );
    b.cuboid(
        [14.0, 19.0, -9.0],
        [15.0, 23.0, -7.0],
        BlockId::WeatheredBricks,
    );
    b.cuboid(
        [8.0, 23.0, -9.0],
        [15.0, 24.0, -7.0],
        BlockId::WeatheredBricks,
    );
    b.torch(9.5, 18.0, -11.5);
    b.torch(13.5, 18.0, -11.5);
    for (x, z) in [
        (-15.0, 5.0),
        (-11.0, 7.0),
        (-16.0, -8.0),
        (16.0, -3.0),
        (14.0, 8.0),
        (0.0, -13.0),
        (-1.0, 10.0),
    ] {
        b.ellipsoid(
            Vec3::new(x, 18.3, z),
            Vec3::new(1.4, 0.9, 1.1),
            BlockId::Leaves,
        );
    }
    // Asymmetric clustered crowns, with open sky and an unblocked central vista.
    for (x, z, h) in [
        (-17.0, -17.0, 8.0),
        (-19.0, -4.0, 7.0),
        (-14.0, 7.0, 8.0),
        (-2.0, -21.0, 9.0),
        (4.0, -23.0, 7.0),
        (19.0, -5.0, 8.0),
        (-20.0, 13.0, 6.0),
    ] {
        b.cuboid([x, 16.0, z], [x + 1.0, 18.0 + h, z + 1.0], BlockId::Log);
        for (dx, dz, dy, r) in [
            (-1.0, 0.0, 0.0, 3.2),
            (1.5, 0.5, -0.5, 2.8),
            (0.0, -1.0, 1.5, 2.6),
        ] {
            b.ellipsoid(
                Vec3::new(x + dx, 18.0 + h + dy, z + dz),
                Vec3::new(r, r * 0.7, r),
                BlockId::Leaves,
            );
        }
    }
    for (x, z) in [
        (-16.0, 2.0),
        (-11.0, 9.0),
        (15.0, -2.0),
        (1.0, -11.0),
        (-18.0, -10.0),
    ] {
        b.ellipsoid(
            Vec3::new(x, 18.0, z),
            Vec3::new(1.8, 1.3, 1.5),
            BlockId::Stone,
        );
    }
    println!("[reference] installing {} edits at {} m", b.edits.len(), s);
    world.set_many(&b.edits);
    crate::material_studio::reference_palette(world.regions.view.session_name());
}
