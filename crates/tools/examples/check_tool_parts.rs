//! Check authored tool-part drafts without compiling or launching the game.
//! `cargo run -p engine_tools --example check_tool_parts -- assets/tools/parts.ron`
//! Use `-` to validate an editor/AI draft on stdin. This command never writes.
use std::io::Read;

fn main() {
    let result = run();
    match result {
        Ok(report) => println!("{report}"),
        Err(report) => {
            println!("{report}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<serde_json::Value, serde_json::Value> {
    let mut args = std::env::args().skip(1);
    let source = args.next().unwrap_or_else(|| "assets/tools/parts.ron".into());
    if args.next().is_some() {
        return Err(serde_json::json!({ "valid": false, "error": "Usage: check_tool_parts [path|-|--describe]" }));
    }
    if source == "--describe" {
        use engine_tools::capabilities::{Capabilities, Mining, MiningClass};
        return Ok(serde_json::json!({
            "valid": true, "miningStyles": MiningClass::ALL,
            "defaults": Capabilities::default(),
            "miningDefaults": Mining { class: MiningClass::Dig, charged: false, cuts_bamboo: false, fells_trees: false },
        }));
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
        return Err(
            serde_json::json!({ "valid": false, "source": source, "error": "Tool-part draft exceeds 4 MiB" }),
        );
    }
    let rows = engine_tools::parse_parts(&text).map_err(
        |errors| serde_json::json!({ "valid": false, "source": source, "diagnostics": errors }),
    )?;
    Ok(serde_json::json!({ "valid": true, "source": source, "rows": rows.schemas.len() }))
}
