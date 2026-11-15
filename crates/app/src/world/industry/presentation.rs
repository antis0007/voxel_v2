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

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Positional hash of a gas cell and puff index; cosmetic, never simulated.
fn hash4(k: (i32, i32, i32, i32), j: u32) -> u32 {
    let mut h = 0x9e37_79b9u32 ^ j.wrapping_mul(0x85eb_ca6b);
    for v in [k.0, k.1, k.2, k.3] {
        h = (h ^ v as u32).wrapping_mul(0xc2b2_ae35);
        h ^= h >> 15;
    }
    h
}

/// 0..1 from the low 8 bits of a hash.
fn unit(h: u32) -> f32 {
    (h & 0xff) as f32 / 255.0
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
    // Industry gas is drawn as the engine's own smoke and steam: voxel-grid
    // cubes in shader kinds 43/44 (billowed, dithered rims, top-lit). Each puff
    // hops up the grid in whole voxels, like the cellular gas it stands for, and
    // enters and leaves through the ordered screen dither of `fade_since`.
    let mut puffs_left = 900usize;
    let previous_kind = mesh.set_surface_kind(0.0);
    for (_, k, m, _) in cells.into_iter().take(192) {
        if puffs_left == 0 {
            break;
        }
        let volume = (k.3 as f32 * scale).powi(3);
        let visible = [0, 1, 2, 4]
            .into_iter()
            .filter(|&i| m[i] > 0)
            .max_by_key(|&i| m[i]);
        let Some(i) = visible else {
            continue;
        };
        let density = m[i] as f32 / volume / 600.0;
        // Fractional puff counts fade the last puff in, so thin gas never pops.
        let wanted = smoothstep(0.002, 0.05, density) * (1.0 + density.min(0.55) * 14.0);
        if wanted < 0.05 {
            continue;
        }
        let species = &CONFIG.species[i];
        mesh.set_surface_kind(if i == STEAM { 44.0 } else { 43.0 });
        // Steam is brief; soot and dust hang about.
        let life = if i == STEAM { 3.2 } else { 6.5 };
        // Visual climb speed (voxels per second); heavy species sink slowly.
        let speed = {
            let v = species.rise_m_s * 4.0;
            (if v.abs() < 0.4 { 0.4f32.copysign(v) } else { v }) / scale
        };
        let origin = pos(k);
        let span = k.3.max(1);
        for j in 0..(wanted.ceil() as usize).min(puffs_left) {
            puffs_left -= 1;
            let h = hash4(k, j as u32);
            let phase = (world.adventure.time / life + unit(h)).fract();
            let age = phase * life;
            // Whole-voxel hops keep every puff on the grid the gas shader samples.
            let climb = (age * speed).floor() as i32;
            let drift = (age * 0.45).floor() as i32;
            let wind = IVec3::new(
                (unit(h >> 8) * 3.0) as i32 - 1,
                0,
                (unit(h >> 12) * 3.0) as i32 - 1,
            );
            let cell = origin
                + IVec3::new(
                    (unit(h >> 4) * span as f32) as i32,
                    (unit(h >> 16) * span as f32) as i32,
                    (unit(h >> 20) * span as f32) as i32,
                )
                + IVec3::Y * climb
                + wind * drift;
            let center = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
            let coverage = (wanted - j as f32).clamp(0.0, 1.0)
                * smoothstep(0.0, 0.18, phase)
                * (1.0 - smoothstep(0.55, 1.0, phase));
            if coverage < 0.04 {
                continue;
            }
            let mark = mesh.mark();
            mesh.cuboid(center, Vec3::splat(scale), Quat::IDENTITY, species.color, 0.0);
            mesh.fade_since(mark, coverage);
        }
    }
    mesh.set_surface_kind(previous_kind);
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
            // Framed pressure vessel on the 10-voxel-per-metre grid: plinth,
            // cap and corner posts around the body, banded in its tier colour.
            let tier = [[0.42, 0.44, 0.46], [0.60, 0.17, 0.13], [0.15, 0.50, 0.54]]
                [n.upgrade.min(2) as usize];
            mesh.cuboid(c, Vec3::splat(0.8), Quat::IDENTITY, metal, 0.0);
            for y in [0.05, 0.95] {
                mesh.cuboid(o + Vec3::new(0.5, y, 0.5), Vec3::new(1.0, 0.1, 1.0), Quat::IDENTITY, dark, 0.0);
            }
            for (x, z) in [(0.05, 0.05), (0.95, 0.05), (0.05, 0.95), (0.95, 0.95)] {
                mesh.cuboid(o + Vec3::new(x, 0.5, z), Vec3::new(0.1, 0.8, 0.1), Quat::IDENTITY, dark, 0.0);
            }
            for y in [0.25, 0.75] {
                mesh.cuboid(o + Vec3::new(0.5, y, 0.5), Vec3::new(0.9, 0.1, 0.9), Quat::IDENTITY, tier, 0.0);
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
            / super::super::factory::industry::gas_capacity(n) as f32;
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
