//! Fernwake's shared intake supplies a bounded nursery.
//! Exact-water worlds debit the intake when a crop advances a growth stage.
use super::{B, Conditions, DIRTY_LIMIT, Key, World, effects, key, resident, stage, table};
use engine_core::environment::face_area_cm2;
use glam::{IVec3, Vec3};

pub(crate) const CAPACITY: u32 = 16;
pub(crate) const SELLER: u32 = 5;
pub(super) const SKY_METRES: f32 = 18.0;
/// 32 litres per square metre of planted area, per completed growth stage.
const STAGE_WATER_UL_PER_CM2: u64 = 3_200;

struct Nursery {
    first: IVec3,
    intakes: [Option<IVec3>; 3],
}

fn site(world: &World) -> Option<Nursery> {
    let cfg = world.sample_config();
    if !engine_worldgen::features::enabled(&cfg, engine_worldgen::features::Feature::Nursery) {
        return None;
    }
    let p = engine_worldgen::fernwake::nursery(&cfg)?;
    let cell = |m: Vec3| (m / cfg.voxel_meters()).floor().as_ivec3();
    let spillway = cfg.generator_version >= engine_voxel::FERNWAKE_SPILLWAY_GENERATOR;
    let datum = p.y - 0.02;
    let ditch =
        |x, z| spillway.then(|| cell(Vec3::new(x, datum - 2.0 + cfg.voxel_meters() * 0.5, z)));
    Some(Nursery {
        first: cell(p),
        intakes: [
            Some(cell(p + Vec3::new(2.0, -0.52, 0.0))),
            ditch(461.5, 55.5),
            ditch(462.5, 56.5),
        ],
    })
}

fn beds(world: &World, first: IVec3) -> impl Iterator<Item = IVec3> + use<> {
    let stride = (1.0 / world.voxel_scale.meters()).round() as i32;
    let count = if stage_water(world).is_some() { 4 } else { 1 };
    [
        first,
        first - IVec3::X * stride,
        first - IVec3::Z * stride,
        first - (IVec3::X + IVec3::Z) * stride,
    ]
    .into_iter()
    .take(count)
}

pub(crate) fn active(world: &World) -> bool {
    site(world).is_some()
}

pub(crate) fn manages(world: &World, p: IVec3, crop: &table::Crop) -> bool {
    managed_site(world, p, crop).is_some()
}

pub(super) fn irrigation(world: &World, p: IVec3, crop: &table::Crop) -> Option<bool> {
    let site = managed_site(world, p, crop)?;
    Some(supply(world, &site).is_some())
}

fn managed_site(world: &World, p: IVec3, crop: &table::Crop) -> Option<Nursery> {
    if crop.crop != B::GlowCap {
        return None;
    }
    site(world).filter(|site| beds(world, site.first).any(|bed| bed == p))
}

fn supply(world: &World, site: &Nursery) -> Option<IVec3> {
    site.intakes.into_iter().flatten().find(|&p| wet(world, p))
}

fn wet(world: &World, p: IVec3) -> bool {
    resident(world, p)
        && world.get(p) == B::Water
        && world.water_microlitres(p) >= stage_water(world).map_or(1, u64::from)
}

fn stage_water(world: &World) -> Option<u32> {
    super::super::environment::State::enabled(world.generator_version)
        .then(|| (face_area_cm2(world.voxel_scale) * STAGE_WATER_UL_PER_CM2) as u32)
}

pub(crate) fn status(world: &World) -> Option<String> {
    let site = site(world)?;
    let first = site.first;
    let intake = site.intakes[0]?;
    if !resident(world, first) || !resident(world, intake) {
        return Some("Nursery Unloaded".into());
    }
    let planted = beds(world, first)
        .filter(|&bed| {
            world
                .farming
                .plots
                .get(&key(bed))
                .and_then(|p| table::get(p.crop))
                .is_some_and(|c| c.crop == B::GlowCap)
        })
        .count();
    let active = supply(world, &site);
    let state = if active.is_some() && planted > 0 {
        "Nursery Irrigated"
    } else if active.is_some() {
        "Beds Empty"
    } else if !matches!(world.get(intake), B::Air | B::Water) {
        "Nursery Intake Blocked"
    } else {
        "Nursery Intake Low"
    };
    Some(stage_water(world).map_or_else(
        || state.into(),
        |dose| {
            let litres = world.water_microlitres(active.unwrap_or(intake)) as f64 / 1_000_000.0;
            let source = if active.is_some_and(|p| p != intake) {
                "Ditch"
            } else {
                "Intake"
            };
            format!(
                "{state} · {planted}/4 Beds · {source} {litres:.1} L · {} L/Stage",
                dose / 1_000_000
            )
        },
    ))
}

/// Managed plots evaluate even when out of sight, consuming unobserved rolls
/// without growth. Returning to a wet intake cannot backfill a dry interval.
pub(super) fn service(world: &mut World) -> [Option<Key>; 4] {
    let mut serviced = [None; 4];
    let Some(site) = site(world) else {
        return serviced;
    };
    for (i, bed) in beds(world, site.first).enumerate() {
        serviced[i] = service_bed(world, bed, &site);
    }
    serviced
}

fn service_bed(world: &mut World, bed: IVec3, site: &Nursery) -> Option<Key> {
    let k = key(bed);
    let mut plot = *world.farming.plots.get(&k)?;
    let crop = table::get(plot.crop).filter(|c| c.crop == B::GlowCap)?;
    let loaded = resident(world, bed);
    let block = loaded.then(|| world.get(bed));
    if block.is_some_and(|b| b != crop.crop && b != crop.sprout) {
        world.farming.plots.remove(&k);
        return Some(k);
    }
    let now = world.farming.clock;
    if plot.stage < crop.mature() && now >= plot.next_roll(crop) {
        // Evaluate at most the latest due interval. Current water cannot
        // replay every interval missed while the bed was unloaded.
        let interval = u64::from(crop.roll_ticks.max(1));
        let elapsed = now.saturating_sub(plot.planted);
        let skipped = (elapsed / interval).saturating_sub(1);
        plot.rolled = plot.rolled.max(u32::try_from(skipped).unwrap_or(u32::MAX));
        let intake = loaded.then(|| supply(world, site)).flatten();
        let conditions = if intake.is_some() {
            super::conditions_with_water(world, bed, crop, Some(true))
        } else {
            None
        };
        // Irrigation is supplied by this intake alone, rather than by the
        // generic nearby-water scan (which varies with voxel size).
        let c = conditions.unwrap_or(Conditions {
            soil: false,
            wet: false,
            light: 0,
        });
        if stage::advance(&mut plot, crop, world.seed, bed, now, c, 1)
            && let Some(dose) = stage_water(world)
            && let Some(intake) = intake
        {
            if world.adventure.dirty_voxels.len() >= DIRTY_LIMIT {
                return Some(k);
            }
            let mut consumed = 0;
            let Ok(commit) = effects::exchange_water_ul(
                world,
                intake,
                &mut consumed,
                dose,
                effects::WaterDirection::IntoBuffer,
            ) else {
                // The stage and its roll remain uncommitted so a deferred
                // water write can retry without giving away growth.
                return Some(k);
            };
            let mut changed = Vec::new();
            let amount = commit.publish_voxel(world, &mut changed);
            world.environment.ledger.transpired_water = world
                .environment
                .ledger
                .transpired_water
                .saturating_add(amount);
            world.adventure.dirty_voxels.extend(changed);
        }
        world.farming.plots.insert(k, plot);
    }
    if let Some(block) = block
        && plot.stage >= crop.mature()
        && world.adventure.dirty_voxels.len() < DIRTY_LIMIT
    {
        let (next, stock) = if block == crop.sprout {
            (crop.crop, 0)
        } else {
            // Keep one seed in the bed; only the surplus enters the workshop.
            (
                crop.sprout,
                super::harvest_count(world.seed, bed, &plot, crop).saturating_sub(1),
            )
        };
        if world.farming.nursery_stock + stock <= CAPACITY
            && let Ok(commit) = effects::replace(world, bed, block, next)
        {
            let mut changed = Vec::new();
            commit.publish_voxel(world, &mut changed);
            if next == crop.sprout {
                world
                    .farming
                    .plots
                    .insert(k, super::Plot::new(plot.crop, now));
                world.farming.nursery_stock += stock;
            }
            world.adventure.dirty_voxels.extend(changed);
        }
    }
    Some(k)
}
