use super::{grand, maze, shell};
use crate::{BlockId, WorldType};
use engine_core::VoxelScalePreset;
use glam::Vec3;

fn world(version: u32) -> crate::WorldSampleConfig {
    crate::WorldSampleConfig {
        planet: None,
        generator_version: version,
        seed: 424242,
        world_type: WorldType::Manyfold,
        voxel_scale: VoxelScalePreset::OneMeter,
    }
}

fn at(version: u32, sector: i32, z: f32, above: f32, along: f32) -> BlockId {
    let cfg = world(version);
    let twist = if version >= shell::GRAND {
        grand::twist(z)
    } else {
        0.0
    };
    let angle = sector as f32 * shell::pitch_for(version) + along - twist;
    let dist = shell::shell_radius(version, z) + above;
    let p = shell::center_of(version) + Vec3::new(angle.sin() * dist, -angle.cos() * dist, z);
    let cell = (p / cfg.voxel_meters() - Vec3::splat(0.5))
        .floor()
        .as_ivec3();
    super::block(&cfg, cell)
}

#[test]
fn the_maze_opens_a_path_and_older_drums_stay() {
    let version = maze::VERSION;
    let prior = shell::GRAND;
    assert!(version > prior);
    let row = shell::row_len(version);
    let mid = row * 0.5;
    let pitch = shell::pitch_for(version);
    let hall = row + mid + 24.0;
    let wall = row + mid + 36.0;
    let lane = pitch * 0.27;
    assert_eq!(at(version, 5, wall, -2.0, lane), BlockId::Limestone);
    assert_eq!(at(version, 5, hall, -2.0, lane), BlockId::Air);
    assert_ne!(
        (
            at(prior, 5, wall, -2.0, lane),
            at(prior, 5, hall, -2.0, lane)
        ),
        (BlockId::Limestone, BlockId::Air)
    );
    assert_eq!(at(version, 4, hall, -2.0, pitch * 0.85), BlockId::Air);
    let (gallery, inside) = maze::mouths(version).expect("mouths");
    assert!(maze::mouths(prior).is_none());
    assert!((gallery.center - inside.center).length() > 8.0);
    assert!(gallery.into_maze.dot(gallery.up).abs() < 1e-4);
    assert!(gallery.up.dot(shell::center_of(version) - gallery.center) > 0.0);
    let cfg = world(version);
    let feet = super::spawn(&cfg).floor().as_ivec3();
    assert!(!super::block(&cfg, feet).is_solid());
}

fn at_angle(version: u32, z: f32, above: f32, angle: f32) -> BlockId {
    let pitch = shell::pitch_for(version);
    let angle = angle.rem_euclid(std::f32::consts::TAU);
    let sector = (angle / pitch).floor() as i32;
    at(version, sector, z, above, angle - sector as f32 * pitch)
}

#[test]
fn the_shell_is_a_maze_and_the_drum_stays_open() {
    let version = maze::FIELD;
    assert!(version > crate::SEEDED_REALMS);
    let cell = std::f32::consts::TAU / 96.0_f32;
    let z0 = -400.0_f32;
    let iz = (z0 / 8.0_f32).floor() as i32;
    let mut plaster = false;
    let mut air = false;
    let mut tall = false;
    for ia in 0..96 {
        let mid_a = (ia as f32 + 0.5) * cell;
        let mid_z = (iz as f32 + 0.5) * 8.0;
        let centre = at_angle(version, mid_z, -2.0, mid_a);
        assert_eq!(centre, BlockId::Air, "corridor blocked at {ia},{iz}");
        let east = at_angle(version, mid_z, -2.0, (ia as f32 + 1.0) * cell + 0.001);
        let north = at_angle(version, (iz as f32 + 1.0) * 8.0 + 0.3, -2.0, mid_a);
        assert!(
            east == BlockId::Air || north == BlockId::Air,
            "no exit at {ia},{iz}"
        );
        if east == BlockId::WhitePlaster || north == BlockId::WhitePlaster {
            plaster = true;
            let wall_z = if north == BlockId::WhitePlaster {
                (iz as f32 + 1.0) * 8.0 + 0.3
            } else {
                mid_z
            };
            let wall_a = if north == BlockId::WhitePlaster {
                mid_a
            } else {
                (ia as f32 + 1.0) * cell + 0.001
            };
            tall |= at_angle(version, wall_z, -10.5, wall_a) == BlockId::WhitePlaster
                && at_angle(version, wall_z, -12.0, wall_a) == BlockId::Air;
            let pitch = shell::pitch_for(version);
            let off_divider = north == BlockId::WhitePlaster
                && (wall_a.rem_euclid(pitch) - pitch * 0.5).abs() < pitch * 0.2;
            if off_divider {
                assert_eq!(at_angle(crate::SEEDED_REALMS, wall_z, -2.0, wall_a), BlockId::Air);
            }
        }
        air |= centre == BlockId::Air;
    }
    assert!(plaster && air && tall, "plaster {plaster} air {air} tall {tall}");
    assert_eq!(at_angle(version, z0, 0.6, cell * 3.5), BlockId::Limestone);
    let cfg = world(version);
    let feet = super::spawn(&cfg).floor().as_ivec3();
    assert!(!super::block(&cfg, feet).is_solid(), "spawn {feet} is inside a wall");
    let (gallery, inside) = maze::mouths(version).expect("mouths");
    for mouth in [gallery, inside] {
        let cell = (mouth.center - Vec3::splat(0.5)).floor().as_ivec3();
        assert!(!super::block(&cfg, cell).is_solid(), "mouth {cell} is walled in");
    }
}

fn flags_near_spawn(version: u32, radius: i32) -> [bool; 4] {
    let cfg = world(version);
    let feet = super::spawn(&cfg) * cfg.voxel_meters();
    let axis = shell::center_of(version);
    let radial = Vec3::new(feet.x - axis.x, feet.y - axis.y, 0.0);
    let inward = -radial.normalize();
    let around = inward.cross(Vec3::Z).normalize();
    let mut brick = false;
    let mut amber = false;
    let mut grass = false;
    let mut plant = false;
    for dz in -radius..=radius {
        for da in -radius..=radius {
            if dz * dz + da * da > radius * radius {
                continue;
            }
            let offset = around * da as f32 + Vec3::Z * dz as f32;
            let feet_p = feet + offset;
            for p in [feet_p, feet_p - inward * 1.9, feet_p + inward * 1.2] {
                let cell = (p / cfg.voxel_meters() - Vec3::splat(0.5))
                    .floor()
                    .as_ivec3();
                match super::block(&cfg, cell) {
                    BlockId::StoneBricks | BlockId::Cobblestone => brick = true,
                    BlockId::AmberCap => amber = true,
                    BlockId::Grass => grass = true,
                    BlockId::Leaves | BlockId::Vines => plant = true,
                    _ => {}
                }
            }
        }
    }
    [brick, amber, grass, plant]
}

#[test]
fn the_close_maze_has_ruins_plants_lamps_and_life() {
    let version = maze::CLOSE;
    assert!(version > maze::FIELD);
    assert!(crate::CURRENT_GENERATOR_VERSION >= version);
    let [brick, amber, grass, plant] = flags_near_spawn(version, 18);
    assert!(brick && amber && grass && plant, "brick {brick} amber {amber} grass {grass} plant {plant}");
    let [old_brick, old_amber, old_grass, old_plant] = flags_near_spawn(maze::FIELD, 5);
    assert!(!old_brick && !old_amber && !old_grass && !old_plant);
    let cell = std::f32::consts::TAU / 216.0_f32;
    let z = -77.5_f32;
    let mut neg_amber = false;
    let mut neg_brick = false;
    for ia in (0..216).step_by(2) {
        let angle = (ia as f32 + 0.5) * cell;
        neg_amber |= at_angle(version, z, -2.6, angle) == BlockId::AmberCap;
        neg_brick |= matches!(
            at_angle(version, z, -2.0, angle),
            BlockId::StoneBricks | BlockId::Cobblestone
        );
    }
    assert!(neg_amber && neg_brick, "amber {neg_amber} brick {neg_brick}");
    let cfg = world(version);
    let player = super::spawn(&cfg);
    let life = super::encounters(&cfg, player);
    let stand = life.iter().find(|e| e.0.z == 7).expect("creature");
    let away = (stand.1 - player).length();
    assert!(away < 42.0, "{away}");
    let approached = super::encounters(&cfg, stand.1);
    assert!(
        approached.iter().any(|e| e.0 == stand.0 && (e.1 - stand.1).length() < 0.05),
        "relic left when the player arrived"
    );
    let feet = (stand.1 / cfg.voxel_meters() - Vec3::splat(0.5))
        .floor()
        .as_ivec3();
    assert!(!super::block(&cfg, feet).is_solid(), "creature inside {feet}");
    assert!(super::encounters(&world(maze::FIELD), player).iter().all(|e| e.0.z != 7));
}

#[test]
fn the_shell_is_a_roofed_lit_room() {
    let version = maze::ROOF;
    assert!(version > maze::CLOSE);
    assert!(crate::CURRENT_GENERATOR_VERSION >= version);
    let cfg = world(version);
    let feet = super::spawn(&cfg) * cfg.voxel_meters();
    let axis = shell::center_of(version);
    let radial = Vec3::new(feet.x - axis.x, feet.y - axis.y, 0.0);
    let inward = -radial.normalize();
    let around = inward.cross(Vec3::Z).normalize();
    let ceiling = feet + inward * 3.2;
    let block = super::block(&cfg, cell_of(&cfg, ceiling));
    assert!(
        matches!(block, BlockId::StoneBricks | BlockId::AmberCap),
        "ceiling {block:?}"
    );
    assert!(!super::block(&cfg, cell_of(&cfg, feet + inward * 0.6)).is_solid());
    let prior = world(maze::CLOSE);
    assert!(
        !super::block(&prior, cell_of(&prior, ceiling)).is_solid(),
        "113 grew a roof"
    );
    let mut amber = false;
    let mut wall = false;
    for dz in -14..=14 {
        for da in -14..=14 {
            let d2 = dz * dz + da * da;
            if d2 > 14 * 14 {
                continue;
            }
            let at = feet + around * da as f32 + Vec3::Z * dz as f32;
            if super::block(&cfg, cell_of(&cfg, at + inward * 3.2)) == BlockId::AmberCap {
                amber = true;
            }
            if (36..=144).contains(&d2)
                && matches!(
                    super::block(&cfg, cell_of(&cfg, at)),
                    BlockId::StoneBricks | BlockId::Cobblestone
                )
            {
                wall = true;
            }
        }
    }
    assert!(amber && wall, "amber {amber} wall {wall}");
    assert!(matches!(
        at_angle(version, -80.0, -4.0, 1.2),
        BlockId::StoneBricks | BlockId::AmberCap
    ));
}

#[test]
fn chambers_rise_above_the_corridor() {
    let version = maze::STOREY;
    assert!(version > maze::ROOF);
    assert!(crate::CURRENT_GENERATOR_VERSION >= version);
    let cfg = world(version);
    let prior = world(maze::ROOF);
    let axis = shell::center_of(version);
    let iz = 33;
    let ia = 3;
    let z = (iz as f32 + 0.5) * 5.0;
    let step = std::f32::consts::TAU / 216.0;
    let angle = ((ia as f32 + 0.5) * step - grand::twist(z)).rem_euclid(std::f32::consts::TAU);
    let radius = shell::shell_radius(version, z);
    let at = axis + Vec3::new(angle.sin() * (radius + 0.4), -angle.cos() * (radius + 0.4), z);
    let inward = -Vec3::new(at.x - axis.x, at.y - axis.y, 0.0).normalize();
    let low = at + inward * 5.0;
    let high = at + inward * 8.6;
    assert_eq!(super::block(&cfg, cell_of(&cfg, at)), BlockId::Grass);
    assert_eq!(super::block(&cfg, cell_of(&cfg, low)), BlockId::Air);
    assert!(matches!(
        super::block(&cfg, cell_of(&cfg, high)),
        BlockId::StoneBricks | BlockId::AmberCap
    ));
    assert!(matches!(
        super::block(&prior, cell_of(&prior, low)),
        BlockId::StoneBricks | BlockId::AmberCap
    ));
}

#[test]
fn avenues_are_open_where_the_hash_was_walled() {
    let version = maze::ROUTE;
    assert!(version > maze::STOREY);
    assert!(crate::CURRENT_GENERATOR_VERSION >= version);
    let cfg = world(version);
    let prior = world(maze::STOREY);
    let mut found = false;
    for iz in 4..16 {
        let at = on_shell(version, 1, iz, 0.04, 0.5, -1.2);
        if super::block(&cfg, cell_of(&cfg, at)) != BlockId::Air {
            continue;
        }
        let old = super::block(&prior, cell_of(&prior, at));
        if !matches!(old, BlockId::StoneBricks | BlockId::Cobblestone) {
            continue;
        }
        let floor = on_shell(version, 1, iz, 0.5, 0.5, 0.4);
        assert_eq!(super::block(&cfg, cell_of(&cfg, floor)), BlockId::Grass);
        found = true;
        break;
    }
    assert!(found, "avenue did not open a wall");
}

#[test]
fn halls_replace_the_hash_with_doors() {
    let version = maze::HALLS;
    assert!(version > maze::ROUTE);
    assert!(crate::CURRENT_GENERATOR_VERSION >= version);
    let cfg = world(version);
    let prior = world(maze::ROUTE);
    let mut opened = false;
    for iz in 4..16 {
        let at = on_shell(version, 5, iz, 0.04, 0.5, -1.2);
        if !matches!(
            super::block(&prior, cell_of(&prior, at)),
            BlockId::StoneBricks | BlockId::Cobblestone
        ) {
            continue;
        }
        assert_eq!(super::block(&cfg, cell_of(&cfg, at)), BlockId::Air);
        opened = true;
        break;
    }
    assert!(opened, "hash wall survived");
    let z = (6.0 + 0.1) * 5.0;
    let arc = std::f32::consts::TAU / 216.0 * shell::shell_radius(version, z);
    let into = 1.0 / arc;
    let pier = on_shell(version, 3, 6, into, 0.1, -1.2);
    let door = on_shell(version, 3, 6, into, 0.5, -1.2);
    assert_eq!(super::block(&cfg, cell_of(&cfg, pier)), BlockId::StoneBricks);
    assert_eq!(super::block(&cfg, cell_of(&cfg, door)), BlockId::Air);
}

#[test]
fn room_corridors_are_taller_and_vines_stay_on_the_wall() {
    let version = maze::ROOM;
    assert!(version > maze::ARRIVAL);
    assert!(crate::CURRENT_GENERATOR_VERSION >= version);
    let mut ceiling_old = 0;
    let mut ceiling_new = 0;
    let mut vines_mid = 0;
    for z in [20.0_f32, 40.0, 80.0, 120.0] {
        for i in 0..48 {
            let angle = i as f32 / 48.0 * std::f32::consts::TAU;
            if at_angle(maze::ARRIVAL, z, -4.2, angle) == BlockId::StoneBricks {
                ceiling_old += 1;
            }
            if at_angle(version, z, -4.2, angle) == BlockId::StoneBricks {
                ceiling_new += 1;
            }
            if at_angle(version, z, -3.0, angle) == BlockId::Vines {
                vines_mid += 1;
            }
        }
    }
    assert!(ceiling_old > 20, "118 ceiling samples {ceiling_old}");
    assert!(ceiling_new * 2 < ceiling_old, "119 still capped at 3.4 m ({ceiling_new} vs {ceiling_old})");
    assert_eq!(vines_mid, 0, "vines still fill the corridor");
}

#[test]
fn arrival_is_a_lit_court() {
    let version = maze::ARRIVAL;
    assert!(version > maze::HALLS);
    assert!(crate::CURRENT_GENERATOR_VERSION >= version);
    let cfg = world(version);
    let prior = world(maze::HALLS);
    let feet = super::spawn(&cfg) * cfg.voxel_meters();
    let axis = shell::center_of(version);
    let inward = -Vec3::new(feet.x - axis.x, feet.y - axis.y, 0.0).normalize();
    let floor = feet - inward * 2.2;
    assert_eq!(super::block(&cfg, cell_of(&cfg, floor)), BlockId::Grass);
    assert_eq!(super::block(&cfg, cell_of(&cfg, feet + inward * 2.0)), BlockId::Air);
    let post = on_shell(version, 1, 1, 0.5, 0.5, -2.0);
    assert_eq!(super::block(&cfg, cell_of(&cfg, post)), BlockId::AmberCap);
    assert_ne!(super::block(&prior, cell_of(&prior, post)), BlockId::AmberCap);
}

fn on_shell(version: u32, ia: i32, iz: i32, frac_a: f32, frac_z: f32, above: f32) -> Vec3 {
    let axis = shell::center_of(version);
    let z = (iz as f32 + frac_z) * 5.0;
    let step = std::f32::consts::TAU / 216.0;
    let angle = ((ia as f32 + frac_a) * step - grand::twist(z)).rem_euclid(std::f32::consts::TAU);
    let radius = shell::shell_radius(version, z) + above;
    axis + Vec3::new(angle.sin() * radius, -angle.cos() * radius, z)
}

fn cell_of(cfg: &crate::WorldSampleConfig, p: Vec3) -> glam::IVec3 {
    (p / cfg.voxel_meters() - Vec3::splat(0.5)).floor().as_ivec3()
}
