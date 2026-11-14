//! Disposable native capture scene, enabled only by the review entry point.
use super::super::factory::{self, Kind};
use super::*;
use crate::player::Player;
use glam::Vec3;

pub fn install(world: &mut World, player: &mut Player) {
    let size = factory::step(world);
    let mut edits = Vec::new();
    for x in -16..=16 {
        for z in -12..=12 {
            for y in 79..=90 {
                let b = if y == 79 { B::DressedStone } else { B::Air };
                for dx in 0..size {
                    for dz in 0..size {
                        for dy in 0..size {
                            edits
                                .push((IVec3::new(x * size + dx, y * size + dy, z * size + dz), b));
                        }
                    }
                }
            }
        }
    }
    world.set_many(&edits);
    world.adventure.enemies.clear();
    world.rules.mode = super::super::rules::GameMode::Creative;
    world.adventure.creative = true;
    for x in [-6, 3] {
        let p = IVec3::new(x * size, 80 * size, 0);
        if let Some(n) = factory::fixture_place(world, p, Kind::Generator) {
            n.input[3] = 4096;
        }
        factory::fixture_place(world, p + IVec3::X * size, Kind::Cable);
        if let Some(n) = factory::fixture_place(world, p + IVec3::X * size * 2, Kind::Crusher) {
            n.input[0] = 4096;
        }
    }
    for (x, y, k) in [
        (3, 81, Kind::ExhaustDuct),
        (3, 82, Kind::DustFilter),
        (3, 83, Kind::Chimney),
        (7, 80, Kind::GasTank),
        (7, 81, Kind::ExhaustDuct),
        (7, 82, Kind::WetScrubber),
        (7, 83, Kind::Chimney),
    ] {
        if let Some(n) = factory::fixture_place(world, IVec3::new(x * size, y * size, 0), k) {
            if k == Kind::DustFilter {
                n.input[4] = 4096;
            }
            if k == Kind::WetScrubber {
                n.water = 8192;
                n.energy = 64;
            }
            if k == Kind::GasTank {
                n.exhaust = [0, 10000, 2000, 4000, 2000, 0];
            }
        }
    }
    if std::env::var("VOXEL_INDUSTRY_REVIEW_CLEAN").is_ok_and(|v| v == "1") {
        for n in world.factory.nodes.values_mut() {
            n.enabled = false;
        }
    }
    camera(player, world.voxel_scale.meters());
    println!(
        "[industry-review] disposable factory scene: raw exhaust, filtered stack, chemical containment; live simulation"
    );
}
pub fn camera(player: &mut Player, scale: f32) -> bool {
    let eye = Vec3::new(12.0, 87.0, 14.0);
    let target = Vec3::new(0.0, 82.0, 0.0);
    player.pos = (eye - Vec3::Y * 1.62) / scale;
    player.precise_pos = player.pos.as_dvec3();
    let d = (target - eye).normalize();
    player.yaw = d.x.atan2(-d.z);
    player.pitch = d.y.asin();
    player.flying = true;
    true
}

/// Reports actual world-clock progress only in the disposable review scene.
pub fn observe(world: &World) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ENABLED
        .get_or_init(|| std::env::var("VOXEL_ENEMY_REVIEW_SCENE").is_ok_and(|v| v == "industry"))
        || world.industry.target % 120 != 0
    {
        return;
    }
    let s = &world.industry;
    let air: u64 = s.air.values().map(super::total).sum();
    let deposited: u64 = s.deposits.values().map(super::total).sum();
    let imported: u64 = s.imported.iter().sum();
    let accounted = air
        + deposited
        + s.transformed.iter().sum::<u64>()
        + s.packed.iter().sum::<u64>()
        + s.withdrawn.iter().sum::<u64>();
    let held: u64 = world
        .factory
        .nodes
        .values()
        .map(|n| n.exhaust.iter().map(|&v| u64::from(v)).sum::<u64>())
        .sum();
    let enabled = world.factory.nodes.values().filter(|n| n.enabled).count();
    println!(
        "[industry-review-state] target={} serviced={} enabled={} air_cells={} air_mg={} deposit_mg={} held_mg={} imported_mg={} balance_mg={}",
        s.target,
        s.tick,
        enabled,
        s.air.len(),
        air,
        deposited,
        held,
        imported,
        imported as i128 - accounted as i128
    );
}
