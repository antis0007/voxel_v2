use anyhow::{Context, Result, ensure};
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, planetary};
use std::{fs::File, io::{BufWriter, Write}, time::Instant};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args.next().context("expected output file, seed, x and z")?;
    let seed = args.next().context("expected seed")?.parse()?;
    let x = args.next().context("expected x")?.parse()?;
    let z = args.next().context("expected z")?.parse()?;
    let mut axis = |default: &[f64]| -> Result<Vec<f64>> {
        args.next().map_or_else(|| Ok(default.to_vec()), |values| {
            values.split(',').map(|value| Ok(value.parse()?)).collect()
        })
    };
    let uplift = axis(&[0.65, 1.0, 1.5])?;
    let incision = axis(&[0.55, 1.0, 1.8])?;
    let drainage_area = axis(&[0.5, 1.0, 2.0])?;
    ensure!(args.next().is_none(), "unexpected arguments after the three parameter axes");
    let count = uplift.len().saturating_mul(incision.len()).saturating_mul(drainage_area.len());
    ensure!((1..=243).contains(&count), "parameter grid must have 1 to 243 candidates");
    let mut grid = Vec::with_capacity(count);
    for u in uplift {
        for &i in &incision {
            for &a in &drainage_area {
                grid.push([u, i, a]);
            }
        }
    }
    let cfg = WorldSampleConfig {
        planet: None, seed, generator_version: planetary::EVOLVED_WATERSHEDS_GENERATOR,
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter,
    };
    let started = Instant::now();
    let result = planetary::review::watershed_sweep(&cfg, x, z, &grid)
        .context("invalid site or parameter scale outside 0.05 to 4")?;
    let mut file = BufWriter::new(File::create(output)?);
    file.write_all(&(result.side as u32).to_le_bytes())?;
    file.write_all(&result.cell_m.to_le_bytes())?;
    file.write_all(&(grid.len() as u32).to_le_bytes())?;
    for value in grid.iter().flatten() { file.write_all(&value.to_le_bytes())?; }
    for value in result.wet.into_iter().chain(result.heights.into_iter().flatten()) {
        file.write_all(&value.to_le_bytes())?;
    }
    file.flush()?;
    eprintln!("seed={seed} candidates={} grid={}x{} seconds={:.2}", grid.len(), result.side,
        result.side, started.elapsed().as_secs_f64());
    Ok(())
}
