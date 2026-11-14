use super::{
    B, CELLS, Cell, Command, Cpu, EDGE, EditOutcome, EditStatus, IVec3, Snapshot, StepResult,
    StepStats, split_voxel, validate,
};
use engine_sim_gpu::{
    GpuWorld, Window,
    resident::{PendingReadback, ReadbackStartError},
};
use std::task::Poll;

const STAGING_BYTES: u64 = 160 * 1024;

struct Job {
    tick: u64,
    commands: Vec<Command>,
    candidate: Cpu,
    outcomes: Vec<EditOutcome>,
    readback: Option<PendingReadback>,
    result: Option<Snapshot>,
    active_bricks: u32,
}

pub(in crate::simulation::cpu_region) struct Gpu {
    world: GpuWorld,
    job: Option<Job>,
    last_changed: u32,
}

impl Gpu {
    pub(in crate::simulation::cpu_region) async fn new(cpu: &Cpu) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env()
                .unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::DX12),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| format!("CA GPU adapter unavailable: {e}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("isolated CA region"),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("CA GPU device unavailable: {e}"))?;
        let lo = split_voxel(cpu.origin - IVec3::splat(8)).0;
        let hi = split_voxel(cpu.origin + IVec3::splat(15)).0;
        let window = Window {
            origin: lo,
            size: (hi - lo + IVec3::ONE).as_uvec3(),
            capacity: 27,
        };
        let world = GpuWorld::new(
            device,
            queue,
            &engine_content::SIM_RULES,
            window,
            &cpu.store,
        )
        .map_err(|e| format!("CA GPU activation rejected: {e:?}"))?;
        eprintln!(
            "[ca-gpu] adapter={} pool_bricks=27 staging_limit={STAGING_BYTES}",
            adapter.get_info().name
        );
        Ok(Self {
            world,
            job: None,
            last_changed: 0,
        })
    }

    pub(in crate::simulation::cpu_region) fn is_staging(&self) -> bool {
        self.job.is_some()
    }

    pub(in crate::simulation::cpu_region) fn ready(
        &mut self,
        cpu: &Cpu,
        tick: u64,
        mut commands: impl Iterator<Item = Command>,
    ) -> Result<bool, String> {
        if self.job.is_none() {
            let batch: Vec<_> = commands.collect();
            let kernel_tick = tick
                .checked_sub(1)
                .and_then(|t| u32::try_from(t).ok())
                .ok_or("GPU kernel tick exhausted")?;
            let mut candidate = cpu.copy_state();
            let outcomes = candidate.apply_commands(tick, &batch)?;
            let reservation = match self.world.reserve_read_back(tick, STAGING_BYTES) {
                Ok(reservation) => reservation,
                Err(ReadbackStartError::Busy) => return Ok(false),
                Err(ReadbackStartError::ByteBudget { .. })
                    if self.world.pending_readback_bytes() != 0 =>
                {
                    return Ok(false);
                }
                Err(error) => return Err(format!("CA GPU readback admission failed: {error:?}")),
            };
            let edits: Vec<_> = outcomes
                .iter()
                .filter(|o| o.status == EditStatus::Applied)
                .map(|o| (o.at, o.after))
                .collect();
            if !edits.is_empty() {
                self.world
                    .edit_reserved(&edits, &candidate.store.dirty_bricks())
                    .map_err(|e| format!("CA GPU edit admission failed: {e:?}"))?;
            }
            let active_bricks = candidate.store.active_bricks() as u32;
            self.world.step(cpu.seed, kernel_tick);
            self.job = Some(Job {
                tick,
                commands: batch,
                candidate,
                outcomes,
                readback: Some(reservation.capture()),
                result: None,
                active_bricks,
            });
        } else {
            let job = self.job.as_ref().ok_or("missing GPU job")?;
            if job.tick != tick
                || job
                    .commands
                    .iter()
                    .any(|expected| commands.next() != Some(*expected))
            {
                return Err("GPU command batch changed during a stalled step".into());
            }
        }
        let job = self.job.as_mut().ok_or("missing GPU job")?;
        if job.result.is_some() {
            return Ok(true);
        }
        let readback = job.readback.as_mut().ok_or("missing GPU readback")?;
        if readback.request_id() != tick {
            return Err("GPU result tick mismatch".into());
        }
        match readback.poll() {
            Poll::Pending => Ok(false),
            Poll::Ready(Err(e)) => Err(format!("CA GPU readback failed: {e:?}")),
            Poll::Ready(Ok(result)) => {
                if result.overflow != 0 || result.bricks_in_use() != 27 {
                    return Err(format!(
                        "CA GPU rejected overflow/topology: flags={} bricks={}",
                        result.overflow,
                        result.bricks_in_use()
                    ));
                }
                job.candidate.absorb_gpu(&result)?;
                job.candidate.completed = tick;
                let stats = StepStats {
                    active_bricks: job.active_bricks,
                    blocks: 0,
                    changed_blocks: result.changed_blocks.wrapping_sub(self.last_changed),
                    stalled_blocks: 0,
                };
                self.last_changed = result.changed_blocks;
                job.result = Some(job.candidate.snapshot(stats));
                job.readback = None;
                Ok(true)
            }
        }
    }

    pub(in crate::simulation::cpu_region) fn take(&mut self) -> Result<(Cpu, StepResult), String> {
        if self.job.as_ref().is_none_or(|job| job.result.is_none()) {
            return Err("GPU commit candidate is not ready".into());
        }
        let mut job = self.job.take().ok_or("missing GPU commit candidate")?;
        let snapshot = job.result.take().ok_or("GPU result is not ready")?;
        Ok((
            job.candidate,
            StepResult {
                snapshot,
                outcomes: job.outcomes,
            },
        ))
    }
}

impl Cpu {
    fn absorb_gpu(&mut self, result: &engine_sim_gpu::Snapshot) -> Result<(), String> {
        for y in -1..=1 {
            for z in -1..=1 {
                for x in -1..=1 {
                    let base = self.origin + IVec3::new(x, y, z) * 8;
                    let (owner, local) = split_voxel(base);
                    if !result.contains_brick(owner, engine_sim::brick_and_cell(local).0) {
                        return Err("GPU result lost a reserved brick".into());
                    }
                    for i in 0..CELLS {
                        let at = base + super::super::local(i);
                        let q = at - self.origin;
                        let cell = result.get(at);
                        validate(cell)?;
                        let interior = q.min_element() >= 0 && q.max_element() < EDGE;
                        let shell = q.min_element() >= -1 && q.max_element() <= EDGE;
                        if !interior
                            && cell
                                != if shell {
                                    Cell::filled(B::Bedrock as u8)
                                } else {
                                    Cell::EMPTY
                                }
                        {
                            return Err("GPU result changed sealed region boundary".into());
                        }
                        let (chunk, local) = split_voxel(at);
                        self.store
                            .set(chunk, local, cell)
                            .map_err(|_| "GPU result exceeded reserved CPU mirror")?;
                    }
                }
            }
        }
        self.store.take_all_dirty();
        for brick in &result.dirty {
            self.store.wake(brick.chunk, brick.brick);
        }
        Ok(())
    }
}
