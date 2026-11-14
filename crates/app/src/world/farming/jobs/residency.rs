//! Worker terrain readiness uses the existing bounded asynchronous source lane.
//! This state is derived: saves retain commitments, never in-flight disk work.
use super::{IVec3, State, Task, Vec3, World};
use crate::simulation::residency::Residency;

#[derive(Default)]
pub(super) struct Sources {
    inner: Residency,
    polled: Option<u64>,
}

impl Clone for Sources {
    fn clone(&self) -> Self { Self::default() }
}

impl std::fmt::Debug for Sources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sources").field("pending", &self.inner.pending())
            .field("bytes", &self.inner.bytes()).finish()
    }
}

fn footprint(out: &mut Vec<IVec3>, at: Vec3, scale: f32) {
    for x in [-0.5, 0.5] { for y in [-1.1, 2.0] { for z in [-0.5, 0.5] {
        out.push(((at + Vec3::new(x, y, z)) / scale).floor().as_ivec3());
    }}}
}

pub(super) fn prepare(world: &mut World, feet: Vec3) -> bool {
    let scale = world.voxel_scale.meters();
    let jobs = &world.farming.jobs;
    let mut cells = Vec::with_capacity(32);
    footprint(&mut cells, feet, scale);
    footprint(&mut cells, feet - Vec3::Y * 4.0, scale);
    if let Some(&next) = jobs.path.front() {
        footprint(&mut cells, feet + (next - feet).clamp_length_max(3.0), scale);
        footprint(&mut cells, next, scale);
    }
    if let Some(mission) = &jobs.mission {
        let waypoint = IVec3::from(mission.waypoint).as_vec3() * scale;
        // Include intermediate chunks at fine voxel scales, not just the far
        // corners of a corridor that may span several source pages.
        let spacing = (crate::world::CHUNK_SIZE as f32 * scale).min(4.0);
        let steps = (feet.distance(waypoint) / spacing).ceil().clamp(1.0, 16.0) as usize;
        for i in 0..=steps {
            let at = feet.lerp(waypoint,i as f32 / steps as f32);
            for x in [-1.0, 1.0] { for z in [-1.0, 1.0] { for y in [-2.0, 3.0] {
                cells.push(((at + Vec3::new(x,y,z)) / scale).floor().as_ivec3());
            }}}
        }
        return prepare_cells(world, cells);
    }
    if let Some(pile) = jobs.stockpile { cells.push(pile.into()); }
    if let Some(home) = jobs.worker.and_then(|id| super::routine::source_cells(jobs, id)) {
        for cell in home { footprint(&mut cells, super::metres(cell, scale), scale); }
    }
    if let Some(production) = &jobs.production {
        if let Some(pile) = jobs.stockpile { cells.push(IVec3::from(pile)-IVec3::Z); }
        if let Some(worker) = jobs.worker {
            let sky = world.sample_config().blocks_for_height(super::super::SKY_METRES).max(1);
            for bed in production.beds.iter().filter(|&&bed| production.owns(worker, bed)) {
                let at = IVec3::from(bed.at);
                cells.extend([at, at - IVec3::Y, at + IVec3::Y * sky]);
                for x in [-4, 4] { for z in [-4, 4] {
                    cells.push(at + IVec3::new(x, -1, z));
                }}
            }
        }
        cells.extend(production.bills.iter().filter(|bill| Some(bill.worker) == jobs.worker)
            .filter(|bill| jobs.stockpile.is_some_and(|pile|
                (IVec3::from(bill.station) - IVec3::from(pile)).as_vec3().length() * scale <= 1.5))
            .map(|bill| IVec3::from(bill.station)));
    }
    if let Some(batch) = &jobs.batch {
        cells.extend([IVec3::from(batch.source), IVec3::from(batch.station)]);
    }
    if let Some(task) = jobs.task {
        let at = (task.point(scale) / scale).floor().as_ivec3();
        cells.extend([at, at - IVec3::Y, at + IVec3::Y]);
        if let Task::Produce(super::production::Task::Sow(bed) | super::production::Task::Harvest(bed)) = task {
            cells.push(IVec3::from(bed.at));
        }
    }
    prepare_cells(world, cells)
}

pub(super) fn prepare_cells(world: &mut World, mut cells: Vec<IVec3>) -> bool {
    // Chunk identity, rather than request order, gives repeatable admission.
    cells.sort_unstable_by_key(|&p| crate::world::chunk_coord_from_world_block(p).to_array());
    cells.dedup_by_key(|p| crate::world::chunk_coord_from_world_block(*p));
    let mut sources = std::mem::take(&mut world.farming.jobs.sources);
    let interval = world.farming.clock / super::PULSE;
    if sources.polled != Some(interval) {
        sources.inner.poll(world);
        sources.polled = Some(interval);
    }
    let mut ready = true;
    let mut admitted = 0;
    for &cell in &cells {
        let coord = crate::world::chunk_coord_from_world_block(cell);
        if world.chunks.contains_key(&coord) { continue; }
        if sources.inner.request(world, coord) && admitted < 2 {
            // Completed pages leave the bounded source pool immediately. Waiting
            // for a whole farm/corridor can otherwise evict its earlier pages.
            let installed = sources.inner.prepare(world, &[cell]);
            admitted += usize::from(installed);
            ready &= installed;
        } else { ready = false; }
    }
    world.farming.jobs.sources = sources;
    ready
}

pub(super) fn snapshot(jobs: &State) -> (usize, usize) {
    (jobs.sources.inner.pending(), jobs.sources.inner.bytes())
}
