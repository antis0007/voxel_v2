//! Local spring inflow. Planning stays off the frame thread; blocked/unloaded
//! outlets accrue no debt. Existing exact-volume edits own all admitted water.
use super::{Budget, cellular, residency::Residency};
use crate::world::{BlockId, World, WorldSampleConfig, chunk_coord_from_world_block};
use engine_worldgen::drainage::{self, Spring};
use glam::{IVec3, Vec2, Vec3};
use std::sync::mpsc;

type Region = (i32, i32);
type Plan = (Region, Vec<Spring>);

#[derive(Default)]
pub(super) struct Springs {
    request: Option<mpsc::SyncSender<(WorldSampleConfig, Region)>>,
    reply: Option<mpsc::Receiver<Plan>>,
    requested: Option<Region>,
    planned: Option<Region>,
    outlets: Vec<Spring>,
    elapsed: f32,
    worker_failed: bool,
}

impl Springs {
    pub fn update(
        &mut self,
        world: &mut World,
        sources: &mut Residency,
        player: Vec3,
        dt: f32,
        budget: Budget,
    ) {
        if world.ca_view.is_some()
            || world.generator_version < drainage::RIVER_FIT_GENERATOR
            || world.world_type == crate::world::WorldType::Superflat
            || self.worker_failed
        {
            return;
        }
        if self.request.is_none() {
            let (tx, rx) = mpsc::sync_channel::<(WorldSampleConfig, Region)>(1);
            let (done, reply) = mpsc::sync_channel::<Plan>(1);
            match std::thread::Builder::new()
                .name("spring-plan".into())
                .spawn(move || {
                    while let Ok((cfg, region)) = rx.recv() {
                        if done.send((region, drainage::springs_near(&cfg, region))).is_err() {
                            break;
                        }
                    }
                }) {
                Ok(_) => {
                    self.request = Some(tx);
                    self.reply = Some(reply);
                }
                Err(error) => {
                    eprintln!("[water] spring planning unavailable: {error}");
                    self.worker_failed = true;
                    return;
                }
            }
        }
        let scale = world.voxel_scale.meters();
        let metric = player * scale;
        let region = drainage::spring_region(Vec2::new(metric.x, metric.z));
        let cfg = world.sample_config();
        if let Some(reply) = &self.reply {
            while let Ok((center, outlets)) = reply.try_recv() {
                if std::env::var_os("VOXEL_RIVER_REVIEW").is_some() {
                    println!("[river-review] planned_region={center:?} outlets={outlets:?}");
                }
                self.planned = Some(center);
                self.outlets = outlets;
            }
        }
        if self.requested != Some(region) {
            if self
                .request
                .as_ref()
                .is_some_and(|tx| tx.try_send((cfg, region)).is_ok())
            {
                self.requested = Some(region);
            }
        }
        self.elapsed += dt;
        if self.elapsed < 0.25 {
            return;
        }
        self.elapsed = 0.0;
        if self.planned != Some(region) {
            return;
        }
        for source in &self.outlets {
            if !budget.has_time() {
                break;
            }
            if (source.position_m - metric).length_squared() > 128.0 * 128.0 {
                continue;
            }
            let head = source.position_m / scale;
            let added = admit(world, sources, head, head.y, source.microlitres_per_second / 4);
            if added > 0 && std::env::var_os("VOXEL_RIVER_REVIEW").is_some() {
                println!(
                    "[river-review] spring_head_m={} admitted_ul={added} total_external_ul={}",
                    source.position_m.y, world.environment.ledger.edited_water_added_ul
                );
            }
        }
    }
}

fn admit(
    world: &mut World,
    sources: &mut Residency,
    head: Vec3,
    stage: f32,
    mut volume: u64,
) -> u64 {
    let start = volume;
    let bottom = head.y.ceil() as i32 - 1;
    for y in bottom..stage.ceil() as i32 {
        if volume == 0 {
            break;
        }
        let p = IVec3::new(head.x.floor() as i32, y, head.z.floor() as i32);
        if !cellular::allowed(world, p) {
            break;
        }
        let below = p - IVec3::Y;
        let chunk = chunk_coord_from_world_block(p);
        if !sources.request_environment(world, chunk, chunk)
            || !sources.request_environment(world, chunk_coord_from_world_block(below), chunk)
            || !sources
                .read(world, p)
                .is_some_and(|(b, _)| matches!(b, BlockId::Air | BlockId::Water))
            || !sources
                .read(world, below)
                .is_some_and(|(b, _)| b == BlockId::Water || b.is_opaque())
            || !sources.prepare(world, &[p, below])
        {
            break;
        }
        let full = engine_core::environment::cell_volume(world.voxel_scale);
        let target = ((stage - p.y as f32).clamp(0.0, 1.0) as f64 * full as f64) as u64;
        let target = target.min(world.environment.water_capacity_ul(p));
        let old = world.water_microlitres(p);
        if old > 0 && world.is_steady_surface_water(p) {
            break;
        }
        let added = target.saturating_sub(old).min(volume);
        if added == 0 {
            continue;
        }
        if !world.set_water_microlitres(p, old + added) {
            break;
        }
        volume -= added;
        // Existing persisted external-input ledger, also used by water edits.
        world.environment.ledger.edited_water_added_ul = world
            .environment
            .ledger
            .edited_water_added_ul
            .saturating_add(added);
        #[cfg(feature = "multiplayer")]
        if world.network_host {
            world.network_changes.insert(p);
        }
    }
    start - volume
}
