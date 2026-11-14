//! Production cloud-noise data and mip upload, shared with numeric profiling.
fn hash3(ix: i32, iy: i32, iz: i32, seed: u32, period: i32) -> [f32; 3] {
    let wx = ((ix % period) + period) % period;
    let wy = ((iy % period) + period) % period;
    let wz = ((iz % period) + period) % period;
    let mut h = (wx as u32).wrapping_mul(374_761_393)
        ^ (wy as u32).wrapping_mul(668_265_263)
        ^ (wz as u32).wrapping_mul(1_274_126_177)
        ^ seed;
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    let x = (h & 0xFFFF) as f32 / 65535.0;
    h = h.wrapping_mul(0x27d4eb2d);
    h ^= h >> 13;
    let y = (h & 0xFFFF) as f32 / 65535.0;
    h = h.wrapping_mul(0x27d4eb2d);
    h ^= h >> 13;
    let z = (h & 0xFFFF) as f32 / 65535.0;
    [x, y, z]
}

pub(super) fn gen_shape_noise_data() -> Vec<u8> {
    let size = 64u32;
    let voxels = (size * size * size) as usize;
    let mut data = vec![0u8; voxels * 4];

    fn worley_tileable(px: f32, py: f32, pz: f32, freq: i32, seed: u32) -> f32 {
        let fx = px * freq as f32;
        let fy = py * freq as f32;
        let fz = pz * freq as f32;
        let ix = fx.floor() as i32;
        let iy = fy.floor() as i32;
        let iz = fz.floor() as i32;
        let mut min_d = 10.0f32;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let nx = ix + dx;
                    let ny = iy + dy;
                    let nz = iz + dz;
                    let pt = hash3(nx, ny, nz, seed, freq);
                    let ddx = (nx as f32 + pt[0]) - fx;
                    let ddy = (ny as f32 + pt[1]) - fy;
                    let ddz = (nz as f32 + pt[2]) - fz;
                    let d = (ddx * ddx + ddy * ddy + ddz * ddz).sqrt();
                    min_d = min_d.min(d);
                }
            }
        }
        1.0 - min_d.clamp(0.0, 1.0)
    }

    fn grad3(hash: u32) -> [f32; 3] {
        const GRADS: [[f32; 3]; 12] = [
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [1.0, -1.0, 0.0],
            [-1.0, -1.0, 0.0],
            [1.0, 0.0, 1.0],
            [-1.0, 0.0, 1.0],
            [1.0, 0.0, -1.0],
            [-1.0, 0.0, -1.0],
            [0.0, 1.0, 1.0],
            [0.0, -1.0, 1.0],
            [0.0, 1.0, -1.0],
            [0.0, -1.0, -1.0],
        ];
        GRADS[(hash % 12) as usize]
    }

    fn perlin_tileable(px: f32, py: f32, pz: f32, freq: i32, seed: u32) -> f32 {
        let fx = px * freq as f32;
        let fy = py * freq as f32;
        let fz = pz * freq as f32;
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let z0 = fz.floor() as i32;
        let xf = fx - fx.floor();
        let yf = fy - fy.floor();
        let zf = fz - fz.floor();
        let sx = xf * xf * xf * (xf * (xf * 6.0 - 15.0) + 10.0);
        let sy = yf * yf * yf * (yf * (yf * 6.0 - 15.0) + 10.0);
        let sz = zf * zf * zf * (zf * (zf * 6.0 - 15.0) + 10.0);
        let period = freq;
        let corner_hash = |cx: i32, cy: i32, cz: i32| -> u32 {
            let wx = ((cx % period) + period) % period;
            let wy = ((cy % period) + period) % period;
            let wz = ((cz % period) + period) % period;
            let mut h = (wx as u32).wrapping_mul(374_761_393)
                ^ (wy as u32).wrapping_mul(668_265_263)
                ^ (wz as u32).wrapping_mul(1_274_126_177)
                ^ seed;
            h ^= h >> 13;
            h = h.wrapping_mul(0x5bd1e995);
            h ^= h >> 15;
            h
        };
        let dot_grad = |cx: i32, cy: i32, cz: i32, dx: f32, dy: f32, dz: f32| -> f32 {
            let g = grad3(corner_hash(cx, cy, cz));
            g[0] * dx + g[1] * dy + g[2] * dz
        };
        let n000 = dot_grad(x0, y0, z0, xf, yf, zf);
        let n100 = dot_grad(x0 + 1, y0, z0, xf - 1.0, yf, zf);
        let n010 = dot_grad(x0, y0 + 1, z0, xf, yf - 1.0, zf);
        let n110 = dot_grad(x0 + 1, y0 + 1, z0, xf - 1.0, yf - 1.0, zf);
        let n001 = dot_grad(x0, y0, z0 + 1, xf, yf, zf - 1.0);
        let n101 = dot_grad(x0 + 1, y0, z0 + 1, xf - 1.0, yf, zf - 1.0);
        let n011 = dot_grad(x0, y0 + 1, z0 + 1, xf, yf - 1.0, zf - 1.0);
        let n111 = dot_grad(x0 + 1, y0 + 1, z0 + 1, xf - 1.0, yf - 1.0, zf - 1.0);
        let x00 = n000 + (n100 - n000) * sx;
        let x10 = n010 + (n110 - n010) * sx;
        let x01 = n001 + (n101 - n001) * sx;
        let x11 = n011 + (n111 - n011) * sx;
        let y0v = x00 + (x10 - x00) * sy;
        let y1v = x01 + (x11 - x01) * sy;
        (y0v + (y1v - y0v) * sz) * 0.5 + 0.5
    }

    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let i = ((z * size * size + y * size + x) * 4) as usize;
                let px = x as f32 / size as f32;
                let py = y as f32 / size as f32;
                let pz = z as f32 / size as f32;

                // Removed freq=64 octave (weight 0.03125, visually negligible at mip
                // distances but cost 2M×27 Worley-equivalent operations per frame of startup).
                let perlin_fbm = perlin_tileable(px, py, pz, 4, 0x1111) * 0.5
                    + perlin_tileable(px, py, pz, 8, 0x2222) * 0.25
                    + perlin_tileable(px, py, pz, 16, 0x3333) * 0.125
                    + perlin_tileable(px, py, pz, 32, 0x4444) * 0.0625;
                let perlin_fbm = perlin_fbm / 0.9375;

                let w_fbm = worley_tileable(px, py, pz, 4, 0xAAAA) * 0.625
                    + worley_tileable(px, py, pz, 8, 0xABBB) * 0.25
                    + worley_tileable(px, py, pz, 16, 0xACCC) * 0.125;
                let w_fbm = w_fbm.clamp(0.0, 1.0);

                let perlin_norm = perlin_fbm.clamp(0.0, 1.0);
                // Keep broad condensate masses. Subtractive Worley remapping
                // here and again in the shader erased most of the shape field.
                let perlin_body = ((perlin_norm - 0.5) * 1.8 + 0.5).clamp(0.0, 1.0);
                let pw = (perlin_body * 0.80 + w_fbm * 0.20).clamp(0.0, 1.0);

                let w1 = (worley_tileable(px, py, pz, 8, 0xBBBB) * 0.625
                    + worley_tileable(px, py, pz, 16, 0xBCCC) * 0.25
                    + worley_tileable(px, py, pz, 32, 0xBDDD) * 0.125)
                    .clamp(0.0, 1.0);
                // Removed freq=64 from W2 and W3 — each saved 2M×27 = 54M distance ops.
                // W3 is the alpha channel (coarsest Worley detail), freq=32 is sufficient.
                let w2 = (worley_tileable(px, py, pz, 16, 0xCCCC) * 0.625
                    + worley_tileable(px, py, pz, 32, 0xCDDD) * 0.375)
                    .clamp(0.0, 1.0);
                let w3 = worley_tileable(px, py, pz, 32, 0xDDDD).clamp(0.0, 1.0);

                data[i] = (pw * 255.0) as u8;
                data[i + 1] = (w1 * 255.0) as u8;
                data[i + 2] = (w2 * 255.0) as u8;
                data[i + 3] = (w3 * 255.0) as u8;
            }
        }
    }

    data
}

pub(super) fn gen_detail_noise_data() -> Vec<u8> {
    let size = 32u32;
    let voxels = (size * size * size) as usize;
    let mut data = vec![0u8; voxels * 4];

    fn worley_tileable(px: f32, py: f32, pz: f32, freq: i32, seed: u32) -> f32 {
        let fx = px * freq as f32;
        let fy = py * freq as f32;
        let fz = pz * freq as f32;
        let ix = fx.floor() as i32;
        let iy = fy.floor() as i32;
        let iz = fz.floor() as i32;
        let mut min_d = 10.0f32;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let nx = ix + dx;
                    let ny = iy + dy;
                    let nz = iz + dz;
                    let pt = hash3(nx, ny, nz, seed, freq);
                    let ddx = (nx as f32 + pt[0]) - fx;
                    let ddy = (ny as f32 + pt[1]) - fy;
                    let ddz = (nz as f32 + pt[2]) - fz;
                    let d = (ddx * ddx + ddy * ddy + ddz * ddz).sqrt();
                    min_d = min_d.min(d);
                }
            }
        }
        1.0 - (min_d * 1.5).min(1.0)
    }

    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let i = ((z * size * size + y * size + x) * 4) as usize;
                let px = x as f32 / size as f32;
                let py = y as f32 / size as f32;
                let pz = z as f32 / size as f32;

                let w1 = (worley_tileable(px, py, pz, 4, 0xEEEE) * 0.625
                    + worley_tileable(px, py, pz, 8, 0xEFFF) * 0.25
                    + worley_tileable(px, py, pz, 16, 0xE111) * 0.125)
                    .clamp(0.0, 1.0);
                let w2 = (worley_tileable(px, py, pz, 8, 0xFFFF) * 0.625
                    + worley_tileable(px, py, pz, 16, 0xF111) * 0.25
                    + worley_tileable(px, py, pz, 32, 0xF222) * 0.125)
                    .clamp(0.0, 1.0);
                // Removed freq=64 octave — cost 262K×27 distance ops for negligible
                // visual quality on a 64³ texture (1 period per texel at freq=64).
                let w3 = (worley_tileable(px, py, pz, 16, 0x1234) * 0.667
                    + worley_tileable(px, py, pz, 32, 0x1345) * 0.333)
                    .clamp(0.0, 1.0);

                // Simplified curl: 4 samples instead of 6 (skip one axis pair).
                let eps = 2.0 / size as f32;
                let dnx_dz = worley_tileable(px, py, pz + eps, 8, 0x5678)
                    - worley_tileable(px, py, pz - eps, 8, 0x5678);
                let dny_dx = worley_tileable(px + eps, py, pz, 8, 0x9ABC)
                    - worley_tileable(px - eps, py, pz, 8, 0x9ABC);
                let curl = ((dnx_dz * dnx_dz + dny_dx * dny_dx).sqrt() * 3.0).clamp(0.0, 1.0);

                data[i] = (w1 * 255.0) as u8;
                data[i + 1] = (w2 * 255.0) as u8;
                data[i + 2] = (w3 * 255.0) as u8;
                data[i + 3] = (curl * 255.0) as u8;
            }
        }
    }

    data
}

pub(super) fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: Vec<u8>,
    size: u32,
) -> wgpu::Texture {
    // Every noise volume has a complete mip chain.
    let mip_count = size.ilog2() + 1;

    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("filtered cloud noise 3D"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: size,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // Write mip 0
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * size),
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: size,
        },
    );

    // Generate and upload mip levels via box filter
    let mut prev = data;
    let mut s = size;
    for mip in 1..mip_count {
        let ns = (s / 2).max(1);
        let nvoxels = (ns * ns * ns) as usize;
        let mut mip_data = vec![0u8; nvoxels * 4];
        for mz in 0..ns {
            for my in 0..ns {
                for mx in 0..ns {
                    let di = ((mz * ns * ns + my * ns + mx) * 4) as usize;
                    for ch in 0..4usize {
                        let mut sum = 0u32;
                        for dz in 0..2u32 {
                            for dy in 0..2u32 {
                                for dx in 0..2u32 {
                                    let sx2 = mx * 2 + dx;
                                    let sy2 = my * 2 + dy;
                                    let sz2 = mz * 2 + dz;
                                    let si = ((sz2 * s * s + sy2 * s + sx2) * 4) as usize;
                                    sum += prev[si + ch] as u32;
                                }
                            }
                        }
                        mip_data[di + ch] = (sum / 8) as u8;
                    }
                }
            }
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &mip_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * ns),
                rows_per_image: Some(ns),
            },
            wgpu::Extent3d {
                width: ns,
                height: ns,
                depth_or_array_layers: ns,
            },
        );
        prev = mip_data;
        s = ns;
    }

    tex
}
