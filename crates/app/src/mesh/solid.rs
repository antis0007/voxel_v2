//! Packed occlusion masks and exact greedy-patch allocation planning.
use super::{ChunkMesh, FaceMaterial, GeneratedChunk};
use crate::world::{BlockId, CHUNK_SIZE};
use glam::IVec3;
use std::sync::atomic::{AtomicBool, Ordering};
const N: usize = 32;
const P: usize = super::PADDED_CHUNK_SIZE as usize;
const INTERIOR: u64 = ((1u64 << N) - 1) << 1;
struct Patch {
    axis: usize,
    positive: bool,
    plane: i32,
    u: i32,
    v: i32,
    width: i32,
    height: i32,
    material: FaceMaterial,
}
pub(super) struct SolidPlan {
    patches: Vec<Patch>,
    pub vertices: usize,
    pub indices: usize,
    torches: bool,
    compact_torches: bool,
    partial_water: bool,
    fittings: Vec<(IVec3, BlockId)>,
}
impl SolidPlan {
    pub fn new(chunk: &GeneratedChunk, cancelled: &AtomicBool) -> Option<Self> {
        let shell = chunk.manyfold_shell.then(|| engine_worldgen::manyfold::shell_curve(chunk.generator_version)).flatten();
        let curve_grid = shell.map(|curve| {
            let origin = (chunk.chunk * CHUNK_SIZE).as_vec3() * chunk.voxel_meters - curve.center;
            let coordinate = |i| i as f32 * chunk.voxel_meters;
            let x: [f32; N + 1] = std::array::from_fn(|i| (origin.x + coordinate(i)).powi(2));
            let y: [f32; N + 1] = std::array::from_fn(|i| (origin.y + coordinate(i)).powi(2));
            let z: [_; N + 1] = std::array::from_fn(|i| curve.deformation_bounds(origin.z + curve.center.z + coordinate(i)));
            (x, y, z)
        });
        let curved_face = |axis: usize, plane: usize, u: usize, v: usize| {
            let Some((x, y, z)) = &curve_grid else { return false };
            [(u, v), (u + 1, v), (u, v + 1), (u + 1, v + 1)].into_iter().any(|(u, v)| {
                let [ix, iy, iz] = match axis {
                    0 => [plane, u, v],
                    1 => [v, plane, u],
                    _ => [u, v, plane],
                };
                z[iz].contains(&(x[ix] + y[iy]))
            })
        };
        let mut opaque = vec![0u64; 3 * P * P];
        let mut renderable = vec![0u64; 3 * P * P];
        let mut water = vec![0u64; 3 * P * P];
        // Submerged volume churn must not switch the whole chunk's water
        // tessellation. Only a rendered free fractional surface needs this path.
        // One-voxel flow steps also need it: only the exact mesher ramps them.
        let partial_water = chunk.water_levels.iter().map(|(&p,&level)|(p,level))
            .chain(chunk.water_deficit.keys().map(|&p|(p,7))).any(|(p, level)| {
            level < 8
                && chunk.get_world(p) == BlockId::Water
                && chunk.get_world(p + IVec3::Y) != BlockId::Water
        }) || (1..=N).any(|y| (1..=N).any(|z| {
            let row = (y * P + z) * P;
            chunk.blocks[row + 1..row + N + 1].iter().enumerate().any(|(x, &b)| {
                b == BlockId::Water && super::has_water_flow_step(
                    chunk,
                    chunk.chunk * CHUNK_SIZE + IVec3::new(x as i32, y as i32 - 1, z as i32 - 1),
                )
            })
        }));
        let mut torches = 0;
        let mut fittings = Vec::new();
        let mut fitting_boxes = 0;
        let mut water_faces = 0;
        for y in 0..P {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            for z in 0..P {
                for x in 0..P {
                    let b = chunk.blocks[(y * P + z) * P + x];
                    if b == BlockId::Air {
                        continue;
                    }
                    let inside =
                        (1..=N).contains(&x) && (1..=N).contains(&y) && (1..=N).contains(&z);
                    let torch = b.is_torch();
                    let fitting = engine_content::fittings::state(b).is_some();
                    let thin = engine_content::thin::is_thin(b);
                    // A submerged plant shares its cell with water: no wall of air around it.
                    let is_water = b == BlockId::Water || engine_content::flora::is_submerged(b);
                    let is_opaque = b.is_opaque();
                    if inside && torch {
                        torches += 1;
                    }
                    if inside && fitting {
                        let p=chunk.chunk*CHUNK_SIZE+IVec3::new(x as i32-1,y as i32-1,z as i32-1);
                        fitting_boxes+=engine_content::fittings::boxes(b,p,chunk.voxel_meters).len();
                        fittings.push((p,b));
                    }
                    if inside && thin {
                        let p=chunk.chunk*CHUNK_SIZE+IVec3::new(x as i32-1,y as i32-1,z as i32-1);
                        fitting_boxes+=engine_content::thin::boxes_scaled(b,p,chunk.voxel_meters).len();
                        fittings.push((p,b));
                    }
                    // Six faces are a conservative bound only for cells carrying partial water.
                    if inside && partial_water && is_water {
                        water_faces += 6;
                    }
                    let rows = [z * P + y, P * P + x * P + z, 2 * P * P + y * P + x];
                    let bits = [1u64 << x, 1u64 << y, 1u64 << z];
                    let draw = !b.is_empty()
                        && !torch
                        && !b.is_model()
                        && !fitting
                        && !thin
                        && (!partial_water || !is_water)
                        || !partial_water && engine_content::flora::is_submerged(b);
                    for axis in 0..3 {
                        if is_opaque {
                            opaque[rows[axis]] |= bits[axis];
                        }
                        if is_water {
                            water[rows[axis]] |= bits[axis];
                        }
                        if draw {
                            renderable[rows[axis]] |= bits[axis];
                        }
                    }
                }
            }
        }
        let compact_torches = torches > 1024;
        let torch_vertices = if compact_torches { 48 } else { 344 };
        let mut plan = Self {
            patches: Vec::new(),
            vertices: torches * torch_vertices + water_faces * 4 + fitting_boxes * 24,
            indices: torches * (torch_vertices / 4 * 6) + water_faces * 6 + fitting_boxes * 36,
            torches: torches > 0,
            compact_torches,
            partial_water,
            fittings,
        };
        if renderable.iter().all(|&mask| mask == 0) {
            return Some(plan);
        }
        let mut masks = vec![None; (N + 1) * N * N];
        let mut lighting = vec![None; (N + 1) * N * N];
        for axis in 0..3 {
            let ua = (axis + 1) % 3;
            let va = (axis + 2) % 3;
            for positive in [false, true] {
                if cancelled.load(Ordering::Relaxed) {
                    return None;
                }
                masks.fill(None);
                lighting.fill(None);
                let mut planes = 0u64;
                for v in 0..N {
                    for u in 0..N {
                        let row = axis * P * P + (v + 1) * P + u + 1;
                        let neighbor = if positive {
                            opaque[row] >> 1
                        } else {
                            opaque[row] << 1
                        };
                        // Internal water faces were previously visited one voxel
                        // at a time even in completely submerged ocean volumes.
                        let adjacent_water = if positive {
                            water[row] >> 1
                        } else {
                            water[row] << 1
                        };
                        let mut visible =
                            ((renderable[row] & !neighbor & !(water[row] & adjacent_water))
                                & INTERIOR)
                                >> 1;
                        while visible != 0 {
                            let depth = visible.trailing_zeros() as usize;
                            visible &= visible - 1;
                            let mut p = [0; 3];
                            p[axis] = depth as i32;
                            p[ua] = u as i32;
                            p[va] = v as i32;
                            let mut q = p;
                            q[axis] += if positive { 1 } else { -1 };
                            let b = super::water_for_plant(super::mesh_block_at(chunk, p));
                            if !super::face_visible(b, super::water_for_plant(super::mesh_block_at(chunk, q))) {
                                continue;
                            }
                            let plane = depth + usize::from(positive);
                            planes |= 1 << plane;
                            let world_p = chunk.chunk * CHUNK_SIZE + IVec3::from_array(p);
                            // A water top may merge into a 2x2 quad only when all eight
                            // neighbours at this row are water: every corner is then free
                            // to move with the waves. Cells at a bank, wall or step stay
                            // 1x1 so their pinned corners never sit on a merged edge.
                            let water_interior = b == BlockId::Water
                                && axis == 1
                                && positive
                                && (-1..=1).all(|dz| {
                                    (-1..=1).all(|dx| {
                                        chunk.get_world(world_p + IVec3::new(dx, 0, dz))
                                            == BlockId::Water
                                    })
                                });
                            let water_bucket = if b == BlockId::Water && axis == 1 && positive && !water_interior {
                                0
                            } else if b == BlockId::Water && axis == 1 && positive {
                                let mut cells = 0u8;
                                let mut sample_p = world_p;
                                while cells < 6
                                    && sample_p.y >= chunk.origin().y
                                    && chunk.get_world(sample_p) == BlockId::Water
                                {
                                    cells += 1;
                                    sample_p.y -= 1;
                                }
                                if sample_p.y < chunk.origin().y {
                                    6
                                } else {
                                    cells
                                }
                            } else {
                                0
                            };
                            let mut material = FaceMaterial::from_block_with_water_bucket(
                                    b,
                                    world_p,
                                    water_bucket,
                                );
                            // Immediate air above the authoritative support is the
                            // surface rule. Glass, foliage and liquids occupy it;
                            // a roof separated by air does not cap it. The existing
                            // one-cell halo and dirty-neighbor radius cover this read.
                            material.capped_turf = matches!(b, BlockId::Grass | BlockId::SnowGrass)
                                && chunk.get_world(world_p + IVec3::Y) != BlockId::Air;
                            masks[plane * N * N + v * N + u] = Some(material);
                            // A material match alone cannot justify a greedy merge:
                            // stretching corner AO/skylight over a whole patch makes
                            // metre-wide light wedges and seams beside small faces.
                            let ao = super::face_corner_ao(
                                chunk, axis, positive, plane as i32, u as i32, v as i32, 1, 1,
                            );
                            if ao.iter().all(|&value| value == 1.0) {
                                let normal = glam::Vec3::from_array(super::face_normal_from_axis(axis, positive));
                                let mut samples = [0u32; 4];
                                for (i, (du, dv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                                    let mut corner = p;
                                    corner[axis] = plane as i32;
                                    corner[ua] += du;
                                    corner[va] += dv;
                                    let position = (chunk.chunk * CHUNK_SIZE + IVec3::from_array(corner)).as_vec3();
                                    samples[i] = chunk.sky_visibility(position, normal).to_bits();
                                }
                                if samples.iter().all(|&value| value == samples[0]) {
                                    lighting[plane * N * N + v * N + u] = Some(samples[0]);
                                }
                            }
                        }
                    }
                }
                while planes != 0 {
                    let plane = planes.trailing_zeros() as usize;
                    planes &= planes - 1;
                    let mask = &mut masks[plane * N * N..(plane + 1) * N * N];
                    let light = &lighting[plane * N * N..(plane + 1) * N * N];
                    let mut i = 0;
                    while i < N * N {
                        let Some(material) = mask[i] else {
                            i += 1;
                            continue;
                        };
                        let u = i % N;
                        let v = i / N;
                        let is_water_top =
                            material.block == BlockId::Water && axis == 1 && positive;
                        let recessed_side = axis != 1
                            && matches!(material.block, BlockId::Water | BlockId::Lava | BlockId::SulfuricAcid);
                        // A curved face must meet every adjacent face at the same
                        // projected voxel corners, including inside a chunk.
                        let max_span = if curved_face(axis, plane, u, v) || recessed_side || (is_water_top && material.water_depth_bucket <= 2) {
                            1
                        } else if is_water_top {
                            2
                        } else {
                            N
                        };
                        let mut w = 1;
                        while u + w < N
                            && w < max_span
                            && !curved_face(axis, plane, u + w, v)
                            && (!is_water_top || (u + w) % 2 == 1)
                            && mask[i + w] == Some(material)
                            && light[i].is_some() && light[i + w] == light[i]
                        {
                            w += 1;
                        }
                        let mut h = 1;
                        'grow: while v + h < N
                            && h < max_span
                            && (!is_water_top || (v + h) % 2 == 1)
                        {
                            for x in 0..w {
                                if mask[(v + h) * N + u + x] != Some(material)
                                    || curved_face(axis, plane, u + x, v + h)
                                    || light[i].is_none() || light[(v + h) * N + u + x] != light[i] {
                                    break 'grow;
                                }
                            }
                            h += 1;
                        }
                        let border = |p| p == 0 || p == N;
                        // A displaced water edge must match its unit-spaced
                        // neighbour even when the greedy patch is inside the chunk.
                        let rim = if is_water_top {
                            2 * (w + h)
                        } else {
                            (if border(v) { w } else { 1 })
                                + (if border(v + h) { w } else { 1 })
                                + (if border(u) { h } else { 1 })
                                + (if border(u + w) { h } else { 1 })
                        };
                        plan.vertices += rim + usize::from(rim > 4);
                        plan.indices += if rim > 4 { rim * 3 } else { 6 };
                        plan.patches.push(Patch {
                            axis,
                            positive,
                            plane: plane as i32,
                            u: u as i32,
                            v: v as i32,
                            width: w as i32,
                            height: h as i32,
                            material,
                        });
                        for y in v..v + h {
                            mask[y * N + u..y * N + u + w].fill(None);
                        }
                        i += w;
                    }
                }
            }
        }
        Some(plan)
    }
    #[cfg(test)]
    pub fn uses_exact_water(&self) -> bool {
        self.partial_water
    }
    pub fn lod_invariant(&self) -> bool {
        self.patches.is_empty() && !self.torches && self.fittings.is_empty()
    }
    pub fn build(&self, chunk: &GeneratedChunk, mut mesh: ChunkMesh, lod: u8) -> ChunkMesh {
        let curve = chunk.manyfold_shell.then(|| engine_worldgen::manyfold::shell_curve(chunk.generator_version)).flatten();
        let project = |p: glam::Vec3| curve.map_or(p, |curve| {
            curve.project(p * chunk.voxel_meters) / chunk.voxel_meters
        });
        for &(p,b) in &self.fittings {
            if b.chest_tier().is_some() { continue; }
            // Sub-metre plant stamps: the root cell draws the plant at metre size.
            let grow = match engine_content::stamp::role(b, p, chunk.voxel_meters, |q| chunk.get_world(q)) {
                engine_content::stamp::Role::Member => continue,
                engine_content::stamp::Role::Root(scale) => scale,
                engine_content::stamp::Role::Lone => 1.0,
            };
            if let Some(parts) = engine_content::flora::parts(b, p) {
                let base = p.as_vec3();
                for part in parts {
                    let color = part.color.unwrap_or_else(|| b.color(p));
                    let surface = if part.canopy { BlockId::Leaves.surface() } else { b.surface() };
                    if let Some(points) = part.leaf {
                        push_leaf(&mut mesh, points.map(|point| project(base + point * grow)), color, surface);
                    } else {
                        push_box(&mut mesh, base + part.lo * grow, base + part.hi * grow, color, surface, project);
                    }
                }
                continue;
            }
            if engine_content::thin::is_thin(b) {
                let boxes = if engine_content::shaped::is_fence(b) {
                    engine_content::shaped::boxes(b, p, engine_content::shaped::links(b, p, |q| chunk.get_world(q)))
                } else {
                    engine_content::thin::boxes_scaled(b, p, chunk.voxel_meters)
                };
                let base = p.as_vec3();
                for (lo,hi) in boxes {
                    push_box(&mut mesh,base+(lo-base)*grow,base+(hi-base)*grow,b.color(p),b.surface(), project);
                }
                continue;
            }
            for (lo,hi) in engine_content::fittings::boxes(b,p,chunk.voxel_meters) {
                let size=hi-lo;
                let mut surface=BlockId::Planks.surface();
                surface.material_id=b as u32+1;
                let fitting=engine_content::fittings::state(b).unwrap();
                let size_m=size*chunk.voxel_meters;
                let latch=fitting.door && (size_m.y-0.10).abs()<0.002;
                let local_top=(hi-engine_content::fittings::anchor(p,engine_content::fittings::step(chunk.voxel_meters)).as_vec3())*chunk.voxel_meters;
                let color=if latch { [0.64,0.42,0.19] } else if fitting.switch == Some(engine_content::fittings::Switch::Lever) && local_top.y < 0.141 { [0.36,0.37,0.34] } else { b.color(p) };
                if latch { surface.texture_kind=41; surface.roughness=0.46; }
                push_box(&mut mesh,lo,hi,color,surface, project);
            }
        }
        for p in &self.patches {
            super::emit_greedy_face(
                &mut mesh, chunk, p.axis, p.positive, p.plane, p.u, p.v, p.width, p.height,
                p.material,
            );
        }
        if self.torches {
            super::append_torch_mesh(&mut mesh, chunk, lod, self.compact_torches);
        }
        if self.partial_water {
            super::append_water_mesh(&mut mesh, chunk);
        }
        mesh
    }
}

fn push_leaf(mesh: &mut ChunkMesh, points: [glam::Vec3; 5], color: [f32; 3], surface: engine_content::SurfaceDetail) {
    for [a, b, c] in [[0, 1, 2], [1, 4, 2], [4, 3, 2], [3, 0, 2]] {
        let [a, b, c] = [points[a], points[b], points[c]];
        let normal = (b - a).cross(c - a).normalize_or(glam::Vec3::Y);
        // Both sides have their own lighting. Four pairs of triangles cost
        // the same 24 vertices as a single box in the fitting mesh budget.
        for (face, normal) in [([a, b, c], normal), ([a, c, b], -normal)] {
            let start = mesh.vertices.len() as u32;
            for point in face {
                mesh.vertices.push(super::Vertex3 {
                    pos: point.to_array(),
                    color,
                    normal: normal.to_array(),
                    surface: [surface.roughness, surface.specular, surface.shader_key(), surface.detail_strength],
                    ao_strength: 1.0,
                });
            }
            mesh.indices.extend([start, start + 1, start + 2]);
        }
    }
}

fn push_box(mesh: &mut ChunkMesh, lo: glam::Vec3, hi: glam::Vec3, color: [f32; 3], surface: engine_content::SurfaceDetail, project: impl Fn(glam::Vec3) -> glam::Vec3) {
    let c = (lo + hi) * 0.5;
    let size = hi - lo;
    for (normal, u, v) in [(glam::Vec3::X, glam::Vec3::Y, glam::Vec3::Z),
        (-glam::Vec3::X, glam::Vec3::Z, glam::Vec3::Y),
        (glam::Vec3::Y, glam::Vec3::Z, glam::Vec3::X),
        (-glam::Vec3::Y, glam::Vec3::X, glam::Vec3::Z),
        (glam::Vec3::Z, glam::Vec3::X, glam::Vec3::Y),
        (-glam::Vec3::Z, glam::Vec3::Y, glam::Vec3::X)] {
        let face = c + normal * size * 0.5;
        let u = u * size * 0.5;
        let v = v * size * 0.5;
        let quad = [face - u - v, face + u - v, face + u + v, face - u + v].map(|p| project(p).to_array());
        let curved = super::normal_from_quad(quad);
        let normal = if curved == [0.0; 3] { normal.to_array() } else { curved };
        super::push_face(&mut mesh.vertices, &mut mesh.indices, quad, color, normal, surface, [1.0; 4]);
    }
}
