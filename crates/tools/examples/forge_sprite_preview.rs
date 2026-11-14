//! Renders saved masks with the same composer used by game tool icons.
use std::{fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("Missing sprite source")?);
    let output = PathBuf::from(args.next().ok_or("Missing output path")?);
    let source = fs::read_to_string(&input)?;
    let sheet = engine_tools::sprite::parse(&source)?;
    let schemas = &engine_tools::PARTS.schemas;
    let cell = 24usize;
    let width = schemas.len() * cell;
    let height = 4 * cell;
    let mut pixels = vec![0u8; width * height * 4];
    for pixel in pixels.chunks_exact_mut(4) { pixel.copy_from_slice(&[24, 22, 28, 255]); }
    for (row, material) in ["stone", "copper", "iron", "steel"].iter().enumerate() {
        for (col, schema) in schemas.iter().enumerate() {
            if !sheet.has_tool(schema.id.as_str()) { continue; }
            let mut materials = vec!["wood"; schema.parts.len()];
            for (i, part) in schema.parts.iter().enumerate() {
                let kind = engine_tools::PARTS.part_kind(part.kind.as_str()).ok_or("Unknown part kind")?;
                materials[i] = match kind.role {
                    engine_tools::parts::PartRole::Head => material,
                    engine_tools::parts::PartRole::Binding | engine_tools::parts::PartRole::Reinforcement => "bone",
                    _ => "wood",
                };
            }
            let image = sheet.render(schema.id.as_str(), &materials).ok_or("Invalid tool composition")?;
            for (i, pixel) in image.iter().enumerate() {
                if let Some((_, rgb)) = pixel {
                    let x = col * cell + 4 + i % 16;
                    let y = row * cell + 4 + i / 16;
                    let target = (y * width + x) * 4;
                    pixels[target..target + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
            }
        }
    }
    let mut file = fs::File::create(output)?;
    file.write_all(b"AXSP01")?;
    file.write_all(&(width as u32).to_le_bytes())?;
    file.write_all(&(height as u32).to_le_bytes())?;
    file.write_all(&pixels)?;
    Ok(())
}
