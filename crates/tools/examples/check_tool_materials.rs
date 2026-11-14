//! Check tool-material drafts with the runtime parser; this command never writes.
//! `cargo run -p engine_tools --example check_tool_materials -- assets/tools/materials.ron`
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
    let source = args
        .next()
        .unwrap_or_else(|| "assets/tools/materials.ron".into());
    if args.next().is_some() {
        return Err(
            serde_json::json!({ "valid": false, "error": "Usage: check_tool_materials [path|-]" }),
        );
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
            "valid": false, "source": source, "error": "Tool-material draft exceeds 4 MiB"
        }));
    }
    let rows = engine_tools::parse_materials(&text).map_err(
        |errors| serde_json::json!({ "valid": false, "source": source, "diagnostics": errors }),
    )?;
    Ok((source, rows.len()))
}
