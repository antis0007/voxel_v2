use anyhow::{Result, ensure};
use engine_worldgen::anchor_studio::{Design, MAX_PLAN_BYTES, city_blueprint};
use std::io::{self, Read, Write};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let operation = args.next().unwrap_or_default();
    ensure!(
        matches!(operation.as_str(), "validate" | "bake" | "city"),
        "Use validate, bake [voxel metres], or city"
    );
    let scale: f32 = args.next().as_deref().unwrap_or("1").parse()?;
    ensure!(args.next().is_none(), "Unexpected argument");
    let mut source = String::new();
    io::stdin()
        .take(MAX_PLAN_BYTES as u64 + 1)
        .read_to_string(&mut source)?;
    let design = Design::parse(&source)?;
    let output = match operation.as_str() {
        "bake" => design.blueprint(scale)?,
        "city" => city_blueprint(design)?,
        _ => return Ok(()),
    };
    io::stdout().lock().write_all(&output)?;
    Ok(())
}
