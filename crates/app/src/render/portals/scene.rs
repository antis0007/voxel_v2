//! Destination passes: terrain, horizon, models and sky from each virtual eye.
//! Clouds, indirect light and outlines stay primary-view only.
use super::super::{ABSOLUTE_SLOT, Gpu, render_isolation};
use super::super::streaming::ChunkCache;

impl Gpu {
    /// Record every planned destination, then composite windows and veils
    /// into the primary scene. Call after the primary sky and clouds.
    pub(in crate::render) fn record_portals(&mut self, encoder: &mut wgpu::CommandEncoder, cache: &ChunkCache) {
        let stage = &self.portals;
        if stage.plan.views.len() < 2 && stage.unready.is_empty() {
            return;
        }
        let mut rendered = vec![false; stage.plan.views.len()];
        rendered[0] = true;
        for (index, view) in stage.plan.views.iter().enumerate().skip(1) {
            let Some(uniform) = stage.camera_uniform(view) else { continue };
            let target = stage.composite.target_of(index);
            self.queue.write_buffer(&target.camera, 0, bytemuck::bytes_of(&uniform));
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
                    label: Some("portal destination pass"),
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &target.scene,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                        }),
                        Some(wgpu::RenderPassColorAttachment {
                            view: &target.receiver,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard },
                        }),
                    ],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &target.depth,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.world_pipeline);
                pass.set_bind_group(0, &target.camera_group, &[]);
                pass.set_bind_group(1, &self.lighting_bind_group, &[]);
                pass.set_bind_group(2, self.atmosphere.bind_group(), &[]);
                pass.set_bind_group(3, &self.world_surface_bind_group, &[]);
                if let Some(offsets) = self.draw_offsets.buffer() {
                    pass.set_vertex_buffer(1, offsets.slice(..));
                }
                let mut bound_page = None;
                for key in &stage.draws[index] {
                    let Some(slice) = cache.draw_slice(key.coord) else { continue };
                    let Some(slot) = self.draw_offsets.existing_slot(key.coord) else { continue };
                    if bound_page != Some(slice.page) {
                        pass.set_vertex_buffer(0, slice.vertices.slice(..));
                        pass.set_index_buffer(slice.indices.slice(..), wgpu::IndexFormat::Uint32);
                        bound_page = Some(slice.page);
                    }
                    pass.draw_indexed(slice.index_range, slice.base_vertex, slot..slot + 1);
                }
                if let Some(mesh) = self.horizon.mesh.as_ref().filter(|_| self.planet_mesh.is_none() && !render_isolation().skip_horizon) {
                    for page in &mesh.pages {
                        pass.set_vertex_buffer(0, page.vertices.slice(..));
                        pass.set_index_buffer(page.indices.slice(..), wgpu::IndexFormat::Uint32);
                        // Same batch selection as the primary pass, without its frustum cull.
                        if mesh.cull || page.batches.iter().any(|b| b.conditional()) {
                            for batch in page.batches.iter().filter(|b| b.selected(mesh.collared)) {
                                pass.draw_indexed(batch.indices.clone(), 0, ABSOLUTE_SLOT..ABSOLUTE_SLOT + 1);
                            }
                        } else {
                            pass.draw_indexed(0..page.count, 0, ABSOLUTE_SLOT..ABSOLUTE_SLOT + 1);
                        }
                    }
                }
                // Models share the scene stream; the shader drops camera-glued ones.
                if self.scene_vertices > 0 && let Some(stream) = &self.scene_stream {
                    pass.set_vertex_buffer(0, stream.slice::<crate::mesh::Vertex3>(self.scene_vertices));
                    pass.draw(0..self.scene_vertices as u32, ABSOLUTE_SLOT..ABSOLUTE_SLOT + 1);
                }
            }
            {
                let mut sky = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
                    label: Some("portal destination sky"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target.scene,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &target.depth,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                sky.set_pipeline(&self.sky_pipeline);
                sky.set_bind_group(0, &target.camera_group, &[]);
                sky.set_bind_group(1, &self.lighting_bind_group, &[]);
                sky.set_bind_group(2, self.atmosphere.bind_group(), &[]);
                sky.set_bind_group(3, &self.world_surface_bind_group, &[]);
                sky.draw(0..3, 0..1);
            }
            rendered[index] = true;
        }
        let stage = &mut self.portals;
        stage.composite.composite(
            &self.device,
            &self.queue,
            encoder,
            &stage.plan,
            &rendered,
            (&self.postprocess.scene, &self.depth),
            &stage.unready,
            self.effect_time,
        );
    }
}
