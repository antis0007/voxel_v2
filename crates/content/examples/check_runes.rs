//! Check the rune table and compile phrases without launching the game.
//! `cargo run -p engine_content --example check_runes -- assets/runes.ron "BOLT then FIRE"`
//! With no phrase, every preset is compiled. This command never writes.

fn main() {
    let mut args = std::env::args().skip(1);
    let source = args.next().unwrap_or_else(|| "assets/runes.ron".into());
    let phrases: Vec<String> = args.collect();
    let report = match run(&source, phrases) {
        Ok(report) => report,
        Err(report) => {
            println!("{report}");
            std::process::exit(1);
        }
    };
    println!("{report}");
}

fn run(source: &str, phrases: Vec<String>) -> Result<serde_json::Value, serde_json::Value> {
    let text = std::fs::read_to_string(source)
        .map_err(|error| serde_json::json!({ "valid": false, "error": error.to_string() }))?;
    let table = engine_content::runes::Table::parse(&text)
        .map_err(|errors| serde_json::json!({ "valid": false, "errors": errors }))?;
    let phrases = if phrases.is_empty() { table.presets.clone() } else { phrases };
    let mut rows = Vec::new();
    let mut valid = true;
    for phrase in phrases {
        match table.compile(&phrase) {
            Ok(program) => rows.push(serde_json::json!({
                "text": program.text,
                "slots": program.slots,
                "mana": program.mana,
                "upkeep": program.upkeep,
                "tier": program.tier,
                "circle": program.circle,
                "id": format!("{:016x}", program.id),
                "deck": table.lower(&program),
            })),
            Err(error) => {
                valid = false;
                rows.push(serde_json::json!({ "text": phrase, "error": error }));
            }
        }
    }
    let report = serde_json::json!({ "valid": valid, "runes": table.runes.len(), "phrases": rows });
    if valid { Ok(report) } else { Err(report) }
}
