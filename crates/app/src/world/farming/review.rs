//! Disposable flora capture scene (`VOXEL_ENEMY_REVIEW_SCENE=flora`).
//! `VOXEL_FLORA_REVIEW=ripe` leaves the harvester out, so the same pose shows the
//! field before and after it works.
use super::{BED, Plot, World, key, table};
use crate::player::Player;
use crate::world::BlockId as B;
use glam::{IVec3, Vec3};

const FLOOR: i32 = 79;
const HARVESTER: IVec3 = IVec3::new(0, FLOOR + 1, -2);

/// Ripe plants in a ring the harvester's 4 m reach covers.
const FIELD: [(i32, i32, B); 7] = [
    (-3, 0, B::WheatCrop),
    (-2, 1, B::CarrotCrop),
    (-1, 1, B::Potato),
    (0, 1, B::Cabbage),
    (1, 1, B::SweetBerries),
    (2, 0, B::Pumpkin),
    (3, -1, B::Cactus),
];

pub fn install(world: &mut World, player: &mut Player) {
    let mut edits = Vec::new();
    for x in -10..=10 {
        for z in -8..=8 {
            for y in FLOOR..=FLOOR + 8 {
                edits.push((IVec3::new(x, y, z), if y == FLOOR { B::DressedStone } else { B::Air }));
            }
        }
    }
    world.set_many(&edits);
    world.adventure.enemies.clear();
    let ripe_only = std::env::var("VOXEL_FLORA_REVIEW").is_ok_and(|v| v == "ripe");
    let mut planted = 0;
    for (x, z, seed) in FIELD {
        let Some((index, crop)) = table::by_seed(seed) else { continue };
        let p = IVec3::new(x, FLOOR + 1, z);
        let soil = crop.soil.first().copied().unwrap_or(BED);
        let mut cells = vec![(p - IVec3::Y, soil)];
        if crop.gourd {
            let fruit = p + IVec3::Z;
            cells.extend([(p, crop.sprout), (fruit - IVec3::Y, soil), (fruit, crop.crop)]);
        } else if crop.stalk.is_some() {
            cells.extend((0..3).map(|y| (p + IVec3::Y * y, crop.crop)));
        } else {
            cells.push((p, crop.crop));
        }
        world.set_many(&cells);
        // A gourd stem has just set its fruit, so its stage is back at zero.
        let stage = if crop.gourd { 0 } else { crop.mature() };
        world.farming.plots.insert(key(p), Plot { crop: index, planted: 0, rolled: 0, stage });
        planted += 1;
    }
    if !ripe_only && let Some((index, crop)) = table::by_seed(B::CropHarvester) {
        world.set_many(&[(HARVESTER, B::CropHarvester), (HARVESTER - IVec3::Z, B::Chest)]);
        world.farming.plots.insert(key(HARVESTER), Plot { crop: index, planted: 0, rolled: 0, stage: crop.mature() });
    }
    camera(player, world.voxel_scale.meters());
    println!("[flora-review] {planted} ripe plots, harvester {}", if ripe_only { "absent" } else { "present" });
}

pub fn camera(player: &mut Player, scale: f32) -> bool {
    let eye = Vec3::new(0.5, FLOOR as f32 + 5.0, 6.5);
    let target = Vec3::new(0.5, FLOOR as f32 + 1.0, -0.5);
    // The scene is authored in cells; only the player's eye offset is in metres.
    player.pos = eye - Vec3::Y * (1.62 / scale);
    player.precise_pos = player.pos.as_dvec3();
    let d = (target - eye).normalize();
    player.yaw = d.x.atan2(-d.z);
    player.pitch = d.y.asin();
    player.flying = true;
    true
}
