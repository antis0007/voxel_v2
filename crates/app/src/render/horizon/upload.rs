//! Incremental staging. Publish a backdrop only after every page is complete.
use super::{GpuMesh, GpuPage, Samples, pages::Page};
use std::{collections::VecDeque, time::Instant, future::Future, pin::Pin,
    task::{Context, Poll, Waker},sync::atomic::{AtomicBool,Ordering}};

type Check = Pin<Box<dyn Future<Output=Option<String>>>>;

const FRAME_BYTES: usize = 8 * 1024 * 1024;

struct Current {
    source: Page,
    gpu: GpuPage,
    vertex_offset: usize,
    index_offset: usize,
}

pub(super) struct Upload {
    check: Option<Check>,
    waiting: VecDeque<Page>,
    current: Option<Current>,
    finished: Vec<GpuPage>,
    samples: Samples,
    center: Option<(i32, i32)>,
    frames: usize,
    bytes: usize,
    started: Instant,
}

impl Upload {
    pub fn new(pages: Vec<Page>, samples: Samples, center: Option<(i32, i32)>) -> Self {
        Self {
            check: None,
            waiting: pages.into(),
            current: None,
            finished: Vec::new(),
            samples,
            center,
            frames: 0,
            bytes: 0,
            started: Instant::now(),
        }
    }

    /// One scoped batch at a time. Pop every scope on the creating frame;
    /// only its futures survive. The complete bundle remains unpublished until
    /// its final allocation/write checks succeed, independent of captures.
    pub fn advance(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, lost:&AtomicBool) -> Result<bool,String> {
        if lost.load(Ordering::Acquire) {return Err("horizon device loss reported before staging".into());}
        device.poll(wgpu::PollType::Poll).map_err(|e|format!("horizon device poll: {e}"))?;
        if lost.load(Ordering::Acquire) {return Err("horizon device loss reported during poll".into());}
        if let Some(check)=self.check.as_mut() {
            match check.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
                Poll::Pending=>return Ok(false),
                Poll::Ready(error)=>{
                    self.check=None;
                    if let Some(error)=error {return Err(error);}
                }
            }
        }
        if self.current.is_none() && self.waiting.is_empty() {return Ok(true);}
        device.push_error_scope(wgpu::ErrorFilter::Internal);
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        self.write_batch(device,queue);
        let memory=device.pop_error_scope();
        let validation=device.pop_error_scope();
        let internal=device.pop_error_scope();
        self.check=Some(Box::pin(async move {
            let errors=[memory.await,validation.await,internal.await];
            let messages:Vec<_>=errors.into_iter().flatten().map(|e|e.to_string()).collect();
            (!messages.is_empty()).then(||messages.join("; "))
        }));
        Ok(false)
    }

    fn write_batch(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.frames += 1;
        // Review-only pressure injection uses the normal queue-ordered path.
        // It never introduces a device-idle wait or early partial publication.
        let frame_bytes = std::env::var("VOXEL_HORIZON_UPLOAD_BYTES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(FRAME_BYTES)
            .clamp(4, FRAME_BYTES)
            / 4
            * 4;
        let mut budget = frame_bytes;
        while budget > 0 {
            if self.current.is_none() {
                let Some(source) = self.waiting.pop_front() else {
                    return;
                };
                let buffer = |label, size, usage| {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(label),
                        size,
                        usage: usage | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                };
                let gpu = GpuPage {
                    vertices: buffer(
                        "horizon vertices",
                        std::mem::size_of_val(source.mesh.vertices.as_slice()) as u64,
                        wgpu::BufferUsages::VERTEX,
                    ),
                    indices: buffer(
                        "horizon indices",
                        std::mem::size_of_val(source.mesh.indices.as_slice()) as u64,
                        wgpu::BufferUsages::INDEX,
                    ),
                    count: source.mesh.indices.len() as u32,
                    batches: source.batches.clone(),
                };
                self.current = Some(Current {
                    source,
                    gpu,
                    vertex_offset: 0,
                    index_offset: 0,
                });
            }
            let current = self.current.as_mut().unwrap();
            let vertices: &[u8] = bytemuck::cast_slice(&current.source.mesh.vertices);
            let indices: &[u8] = bytemuck::cast_slice(&current.source.mesh.indices);
            // Every size and offset is four-byte aligned. The final slice may
            // split a vertex, but is never drawn until the complete swap.
            for (buffer, bytes, offset) in [
                (&current.gpu.vertices, vertices, &mut current.vertex_offset),
                (&current.gpu.indices, indices, &mut current.index_offset),
            ] {
                let count = (bytes.len() - *offset).min(budget);
                if count > 0 {
                    queue.write_buffer(buffer, *offset as u64, &bytes[*offset..*offset + count]);
                    *offset += count;
                    budget -= count;
                    self.bytes += count;
                }
            }
            if current.vertex_offset == vertices.len() && current.index_offset == indices.len() {
                self.finished.push(self.current.take().unwrap().gpu);
            }
        }
    }

    pub fn finish(self) -> (GpuMesh, Samples, Option<(i32, i32)>) {
        assert!(self.check.is_none() && self.current.is_none() && self.waiting.is_empty(),
            "horizon upload must be fully validated before publication");
        println!(
            "[horizon] upload_complete pages={} bytes={} frames={} staging_limit_bytes={} elapsed_ms={:.1}",
            self.finished.len(),
            self.bytes,
            self.frames,
            FRAME_BYTES,
            self.started.elapsed().as_secs_f64() * 1000.0
        );
        (
            GpuMesh {
                collared:false,
                pages: self.finished,
                cull: std::env::var("VOXEL_HORIZON_CULL").as_deref() != Ok("0"),
            },
            self.samples,
            self.center,
        )
    }
}

#[cfg(test)]
mod validation_contract {
    use super::*;
    use crate::mesh::{ChunkMesh,Vertex3};

    fn page(count:usize)->Page {
        let v=Vertex3{pos:[0.0;3],normal:[0.0,1.0,0.0],color:[1.0;3],
            surface:[0.9,0.0,0.0,0.0],ao_strength:-1.0};
        Page{mesh:ChunkMesh{vertices:vec![v;count],indices:vec![0,1,2]},batches:Vec::new()}
    }

    #[test]
    #[ignore="actual RTX4090 Vulkan scoped upload checks; run serially"]
    fn horizon_upload_waits_for_validation_and_rejects_invalid_allocations() {
        let instance=wgpu::Instance::new(&wgpu::InstanceDescriptor{
            backends:wgpu::Backends::VULKAN,..Default::default()});
        let adapter=pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
        let info=adapter.get_info();
        assert!(info.name.contains("4090"),"required RTX4090, got {info:?}");
        println!("[horizon-upload-contract] adapter={info:?}");
        let mut limits=wgpu::Limits::default();limits.max_buffer_size=65536;
        let (device,queue)=pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor{
            required_limits:limits,..Default::default()})).unwrap();
        assert!(!device.features().contains(wgpu::Features::TIMESTAMP_QUERY));
        let lost=super::super::super::install_device_loss_signal(&device);
        // No profiler/capture callback and no Wait poll: queue ordering plus the
        // production nonblocking poll must complete the successful transaction.
        let mut valid=Upload::new(vec![page(3)],Samples::new(),Some((0,0)));
        assert!(!valid.advance(&device,&queue,&lost).unwrap(),"queued bytes alone cannot authorize publication");
        let deadline=Instant::now()+std::time::Duration::from_secs(5);
        loop {
            queue.submit(std::iter::empty());
            if valid.advance(&device,&queue,&lost).unwrap(){break;}
            assert!(Instant::now()<deadline,"valid scope did not progress");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let (published,_,_)=valid.finish();
        let old_vertex_bytes=published.pages[0].vertices.size();
        let mut horizon=super::super::Horizon::default();
        horizon.set_device_loss_signal(&lost);
        horizon.mesh=Some(published);
        horizon.upload=Some(Upload::new(vec![page(4096)],Samples::new(),Some((512,0))));
        let deadline=Instant::now()+std::time::Duration::from_secs(5);
        loop {
            queue.submit(std::iter::empty());
            assert!(!horizon.advance_upload(&device,&queue,Instant::now()),
                "invalid allocation became publishable");
            if horizon.upload.is_none(){break;}
            assert!(Instant::now()<deadline,"invalid scope did not progress");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let old=horizon.mesh.as_ref().expect("failed replacement removed published cover");
        assert_eq!(old.pages[0].count,3);
        assert_eq!(old.pages[0].vertices.size(),old_vertex_bytes);
        println!("[horizon-upload-contract] invalid allocation rejected; installed cover survived production failure branch");
        device.destroy();
        let deadline=Instant::now()+std::time::Duration::from_secs(5);
        while !lost.load(Ordering::Acquire) {
            let _=device.poll(wgpu::PollType::Poll);
            assert!(Instant::now()<deadline,"profiler-independent loss callback did not progress");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let mut after_loss=Upload::new(vec![page(3)],Samples::new(),Some((1024,0)));
        assert!(after_loss.advance(&device,&queue,&lost).is_err());
        assert_eq!(after_loss.bytes,0,"known device loss must not start staging");
        println!("[horizon-upload-contract] intentional device destruction detected without timestamp profiler; staging refused");
    }
}
