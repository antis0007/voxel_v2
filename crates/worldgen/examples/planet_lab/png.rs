//! Minimal PNG writer (8-bit RGB, stored deflate blocks). No dependency; the
//! lab's images are review evidence, not shipped assets, so size is secondary.
use std::io::{self, Write};

fn crc32(bytes: &[u8], mut crc: u32) -> u32 {
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    crc
}

fn chunk(out: &mut impl Write, kind: &[u8; 4], data: &[u8]) -> io::Result<()> {
    out.write_all(&(data.len() as u32).to_be_bytes())?;
    out.write_all(kind)?;
    out.write_all(data)?;
    let crc = !crc32(data, crc32(kind, !0));
    out.write_all(&crc.to_be_bytes())
}

/// zlib stream of stored (uncompressed) deflate blocks with its Adler-32.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = Vec::with_capacity(raw.len() + raw.len() / 65_535 * 5 + 16);
    z.extend_from_slice(&[0x78, 0x01]);
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for part in raw.chunks(5552) {
        for &v in part {
            a += u32::from(v);
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    z
}

pub fn write(path: &std::path::Path, width: usize, height: usize, rgb: &[u8]) -> io::Result<()> {
    assert_eq!(rgb.len(), width * height * 3, "rgb buffer size");
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut raw = Vec::with_capacity((width * 3 + 1) * height);
    for row in rgb.chunks(width * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut out = io::BufWriter::new(std::fs::File::create(path)?);
    out.write_all(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])?;
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(height as u32).to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header)?;
    chunk(&mut out, b"IDAT", &zlib_stored(&raw))?;
    chunk(&mut out, b"IEND", &[])?;
    out.flush()
}

/// Linear radiance to display bytes: filmic shoulder, then the sRGB curve.
pub fn encode(linear: [f32; 3], exposure: f32) -> [u8; 3] {
    linear.map(|c| {
        let x = (c * exposure).max(0.0);
        // Narkowicz ACES fit keeps sunlit snow below clipping.
        let t = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
        let s = if t <= 0.003_130_8 { t * 12.92 } else { 1.055 * t.powf(1.0 / 2.4) - 0.055 };
        (s.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
    })
}
