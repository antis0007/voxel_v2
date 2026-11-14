use super::config::CONFIG;
use super::*;
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

pub fn player_exposure(world: &mut World, player: &crate::player::Player, dt: f32) {
    let scale = world.voxel_scale.meters();
    let concentration = world
        .industry
        .concentration(player.eye(world.voxel_scale).floor().as_ivec3(), scale);
    let mut dose: f32 = concentration
        .iter()
        .zip(&CONFIG.species)
        .filter(|(_, s)| s.harm_mg_m3 > 0.0)
        .map(|(c, s)| c / s.harm_mg_m3)
        .sum();
    let feet = player.pos.floor().as_ivec3();
    // A clean cell cannot cause water exposure. Check the sparse pollution
    // field first so flying over untouched terrain does not generate voxels.
    let contamination = world.industry.contamination(feet, scale);
    let contaminated_water = contamination != 0 && world.get(feet) == B::Water;
    if contaminated_water {
        dose += contamination as f32 / 2000.0;
    }
    if world.rules.mode.interacts() && dose > 1.0 {
        world.adventure.environment_damage(
            ((dose - 1.0) * 0.35 * dt).min(2.0),
            super::super::adventure::death::Cause::Pollution,
        );
        if world.expedition.message_seconds <= 0.0 {
            world.expedition.say_for(
                if contaminated_water {
                    "Contaminated water"
                } else {
                    "Hazardous air"
                },
                2.0,
            );
        }
    }
}
pub fn exposure(world: &mut World, state: &State, dt: f32) {
    let scale = world.voxel_scale.meters();
    for enemy in &mut world.adventure.enemies {
        if enemy.health <= 0.0 {
            continue;
        }
        let p = (enemy.pos / scale).floor().as_ivec3();
        let c = state.concentration(p, scale);
        let dose: f32 = c
            .iter()
            .zip(&CONFIG.species)
            .filter(|(_, s)| s.harm_mg_m3 > 0.0)
            .map(|(c, s)| c / s.harm_mg_m3)
            .sum();
        if dose > 2.0 {
            enemy.health = (enemy.health - (dose - 2.0).min(10.0) * dt * 0.1).max(0.0);
        }
    }
}

pub fn draw(mesh: &mut Model, world: &World, viewer: Vec3) {
    let scale = world.voxel_scale.meters();
    let mut cells: Vec<_> = world
        .industry
        .air
        .iter()
        .filter_map(|(&k, m)| {
            let p = (pos(k).as_vec3() + Vec3::splat(k.3 as f32 * 0.5)) * scale;
            let distance = p.distance_squared(viewer);
            (distance < 96.0 * 96.0).then_some((distance, k, m, p))
        })
        .collect();
    cells.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    for (_, k, m, p) in cells.into_iter().take(384) {
        let volume = (k.3 as f32 * scale).powi(3);
        let visible = [0, 1, 2, 4]
            .into_iter()
            .filter(|&i| m[i] > 0)
            .max_by_key(|&i| m[i]);
        let Some(i) = visible else {
            continue;
        };
        let opacity = (m[i] as f32 / volume / 600.0).clamp(0.0, 0.55);
        if opacity < 0.015 {
            continue;
        }
        let color = CONFIG.species[i].color;
        let radius = k.3 as f32 * scale * 0.40;
        for j in 0..3 {
            let t = world.adventure.time * 0.35 + j as f32 * 2.1;
            let offset = Vec3::new(t.sin(), (t * 0.7).cos(), (t * 1.3).sin()) * radius * 0.32;
            mesh.glow_voxel(
                p + offset,
                Vec3::splat(radius * (0.65 + j as f32 * 0.09)),
                Quat::from_rotation_y(t * 0.13),
                color,
                0.0,
                opacity * 0.6,
            );
        }
    }
    for (&p, m) in world
        .industry
        .deposits
        .iter()
        .filter(|(p, _)| {
            (Vec3::new(p.0 as f32, p.1 as f32, p.2 as f32) * scale).distance_squared(viewer)
                < 24.0 * 24.0
        })
        .take(128)
    {
        let origin = Vec3::new(p.0 as f32, p.1 as f32, p.2 as f32) * scale;
        let color = if m[2] > m[0] + m[1] {
            [0.31, 0.37, 0.14]
        } else {
            [0.17, 0.16, 0.13]
        };
        // The overlay follows the exposed surface within its metre square.
        let size = super::super::factory::step(world);
        let centre = IVec3::new(p.0 + size / 2, p.1 + size - 1, p.2 + size / 2);
        if !world.get(centre).is_solid() {
            continue;
        }
        mesh.glow_voxel(
            origin + Vec3::new(0.5, 1.008, 0.5),
            Vec3::new(0.42, 0.006, 0.42),
            Quat::IDENTITY,
            color,
            0.0,
            (total(m) as f32 / 4000.0).clamp(0.0, 0.65),
        );
    }
}

pub fn machine(
    mesh: &mut Model,
    world: &World,
    p: IVec3,
    n: &super::super::factory::Node,
    o: Vec3,
) -> bool {
    use super::super::factory::Kind;
    if !super::super::factory::industry::component(n.kind) {
        return false;
    }
    let metal = [0.29, 0.35, 0.37];
    let dark = [0.09, 0.12, 0.13];
    let c = o + Vec3::splat(0.5);
    match n.kind {
        Kind::ExhaustDuct | Kind::Chimney => {
            mesh.cuboid(c, Vec3::splat(0.46), Quat::IDENTITY, metal, 0.0);
            for (side, d) in super::super::factory::DIRS.iter().enumerate() {
                if n.kind == Kind::ExhaustDuct && n.target >> side & 1 != 0 {
                    continue;
                }
                let q = p + *d * super::super::factory::step(world);
                if world
                    .factory
                    .nodes
                    .get(&q)
                    .is_some_and(|other| super::super::factory::industry::port(other.kind))
                    || (n.kind == Kind::Chimney && *d == IVec3::Y)
                {
                    let axis = d.as_vec3();
                    mesh.cuboid(
                        c + axis * 0.30,
                        Vec3::splat(0.36) + axis.abs() * 0.04,
                        Quat::IDENTITY,
                        metal,
                        0.0,
                    );
                    mesh.cuboid(
                        c + axis * 0.46,
                        Vec3::splat(0.50) - axis.abs() * 0.42,
                        Quat::IDENTITY,
                        dark,
                        0.0,
                    );
                }
            }
        }
        Kind::GasTank => {
            mesh.cuboid(c, Vec3::new(0.78, 0.90, 0.78), Quat::IDENTITY, metal, 0.0);
            for y in [0.2, 0.8] {
                mesh.cuboid(
                    o + Vec3::new(0.5, y, 0.5),
                    Vec3::new(0.84, 0.08, 0.84),
                    Quat::IDENTITY,
                    dark,
                    0.0,
                );
            }
        }
        _ => {
            mesh.cuboid(c, Vec3::new(0.94, 0.94, 0.94), Quat::IDENTITY, metal, 0.0);
            for y in 0..5 {
                mesh.cuboid(
                    c + Quat::from_rotation_arc(Vec3::NEG_Z, n.facing.as_vec3())
                        * Vec3::new(0.0, -0.27 + y as f32 * 0.13, -0.481),
                    Vec3::new(0.65, 0.035, 0.035),
                    Quat::from_rotation_arc(Vec3::NEG_Z, n.facing.as_vec3()),
                    dark,
                    0.0,
                );
            }
        }
    }
    if !matches!(n.kind, Kind::ExhaustDuct | Kind::Chimney) {
        let fill = n.exhaust.iter().sum::<u32>() as f32
            / super::super::factory::industry::GAS_CAPACITY as f32;
        let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, n.facing.as_vec3());
        let depth = if n.kind == Kind::GasTank { 0.40 } else { 0.481 };
        mesh.cuboid(
            c + rotation * Vec3::new(0.28, -0.35 + fill * 0.35, -depth),
            Vec3::new(0.04, fill.max(0.02) * 0.7, 0.03),
            rotation,
            [0.67, 0.47, 0.14],
            0.3,
        );
    }
    true
}
