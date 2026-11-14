//! Compile the engine's actual schema, with no GPU/app dependency.
#[path = "../../../crates/content/src/portal_layout.rs"]
mod portal_layout;
use std::io::Read;

fn main() {
    let mut text = String::new();
    let result = std::io::stdin()
        .take(262_145)
        .read_to_string(&mut text)
        .map_err(|e| serde_json::json!({"error": e.to_string()}))
        .and_then(|_| {
            portal_layout::parse(&text).map_err(|errors| serde_json::json!({"diagnostics": errors}))
        });
    match result {
        Ok(layout) => println!(
            "{}",
            serde_json::json!({"valid": true, "warnings": layout.warnings(), "graph": layout})
        ),
        Err(error) => {
            println!("{error}");
            std::process::exit(1);
        }
    }
}
