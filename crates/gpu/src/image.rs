//! CPU-built mip chains for scalar fields, masks and streamed texture layers.

#[derive(Clone, Debug)]
pub struct FieldMip<const CHANNELS: usize> {
    pub edge: u32,
    pub texels: Vec<u8>,
}
pub type ScalarMip = FieldMip<1>;
pub type RgbaMip = FieldMip<4>;

/// Square power-of-two scalar images; every level preserves the mean coverage.
pub fn scalar_mips(edge: u32, texels: Vec<u8>) -> Vec<ScalarMip> {
    field_mips::<1>(edge, texels)
}

pub fn rgba_mips(edge: u32, texels: Vec<u8>) -> Vec<RgbaMip> {
    field_mips::<4>(edge, texels)
}

fn field_mips<const CHANNELS: usize>(edge: u32, texels: Vec<u8>) -> Vec<FieldMip<CHANNELS>> {
    assert!(edge.is_power_of_two() && texels.len() == (edge as usize).pow(2) * CHANNELS);
    let mut levels = vec![FieldMip { edge, texels }];
    while levels.last().is_some_and(|m| m.edge > 1) {
        let last = &levels[levels.len() - 1];
        let edge = last.edge / 2;
        let mut texels = Vec::with_capacity((edge * edge) as usize * CHANNELS);
        for y in 0..edge {
            for x in 0..edge {
                for channel in 0..CHANNELS {
                    let i = (y * 2 * last.edge + x * 2) as usize * CHANNELS + channel;
                    let stride = last.edge as usize * CHANNELS;
                    let sum = u32::from(last.texels[i])
                        + u32::from(last.texels[i + CHANNELS])
                        + u32::from(last.texels[i + stride])
                        + u32::from(last.texels[i + stride + CHANNELS]);
                    texels.push(((sum + 2) / 4) as u8);
                }
            }
        }
        levels.push(FieldMip { edge, texels });
    }
    levels
}

pub fn scalar_mip_bytes(edge: u32) -> usize {
    let mut edge = edge;
    let mut bytes = 0;
    while edge > 0 {
        bytes += (edge as usize).pow(2);
        edge /= 2;
    }
    bytes
}

/// Upload a complete chain into one array layer, optionally starting at a coarse mip.
/// Queue writes accept tightly packed rows, including the small tail mips.
pub fn upload_scalar_mips(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    layer: u32,
    first_mip: u32,
    levels: &[ScalarMip],
) {
    upload_field_mips(queue, texture, layer, first_mip, levels);
}

pub fn upload_rgba_mips(queue: &wgpu::Queue, texture: &wgpu::Texture, layer: u32,
    first_mip: u32, levels: &[RgbaMip]) {
    upload_field_mips(queue, texture, layer, first_mip, levels);
}

fn upload_field_mips<const CHANNELS: usize>(queue: &wgpu::Queue, texture: &wgpu::Texture,
    layer: u32, first_mip: u32, levels: &[FieldMip<CHANNELS>]) {
    for (mip, level) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: first_mip + mip as u32,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &level.texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(level.edge * CHANNELS as u32),
                rows_per_image: Some(level.edge),
            },
            wgpu::Extent3d {
                width: level.edge,
                height: level.edge,
                depth_or_array_layers: 1,
            },
        );
    }
}
