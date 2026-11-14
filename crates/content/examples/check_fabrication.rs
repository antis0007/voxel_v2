//! Check Fabricator drafts with the runtime compiler; this command never writes.
//! `cargo run -p engine_content --example check_fabrication -- assets/fabrication.ron`
//! Use `-` to validate an editor/AI draft on stdin (at most 4 MiB).
use std::io::Read;

fn main() {
    match run() {
        Ok((source, rows)) => println!(
            "{}",
            serde_json::json!({ "valid": true, "source": source, "rows": rows })
        ),
        Err(report) => {
            println!("{report}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(String, usize), serde_json::Value> {
    let mut args = std::env::args().skip(1);
    let source = args.next().unwrap_or_else(|| "assets/fabrication.ron".into());
    if args.next().is_some() {
        return Err(
            serde_json::json!({ "valid": false, "error": "Usage: check_fabrication [path|-]" }),
        );
    }
    if source == "--describe" {
        use engine_content::{fabrication as f, factory_resources as r};
        println!("{}", serde_json::json!({
            "version": 1, "resources": r::NAMES, "resourceCount": r::RESOURCE_COUNT,
            "capacity": f::CAPACITY, "maxPower": f::CONSUMER_BUFFER,
            "minRecipes": f::MIN_RECIPES, "maxInputs": f::MAX_INPUTS, "maxTicks": f::MAX_TICKS,
            "unitsPerItem": f::UNITS_PER_ITEM, "ticksPerSecond": f::TICKS_PER_SECOND,
        }));
        std::process::exit(0);
    }
    let read = if source == "-" {
        let mut text = String::new();
        std::io::stdin()
            .take(4 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map(|_| text)
    } else {
        std::fs::File::open(&source).and_then(|file| {
            let mut text = String::new();
            file.take(4 * 1024 * 1024 + 1)
                .read_to_string(&mut text)
                .map(|_| text)
        })
    };
    let text = read.map_err(
        |error| serde_json::json!({ "valid": false, "source": source, "error": error.to_string() }),
    )?;
    if text.len() > 4 * 1024 * 1024 {
        return Err(serde_json::json!({
            "valid": false, "source": source, "error": "Fabricator draft exceeds 4 MiB"
        }));
    }
    let rows = engine_content::fabrication::parse(&text).map_err(
        |errors| serde_json::json!({ "valid": false, "source": source, "diagnostics": errors }),
    )?;
    Ok((source, rows.len()))
}
