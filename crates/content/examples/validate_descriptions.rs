//! Author-facing content lint; does not launch the game or run tests.
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| {
        format!("{}/../../assets/descriptions.ron", env!("CARGO_MANIFEST_DIR"))
    });
    if args.next().is_some() {
        eprintln!("Usage: validate_descriptions [path/to/descriptions.ron]");
        return ExitCode::FAILURE;
    }
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    match engine_content::descriptions::parse(&source) {
        Ok(catalog) => {
            println!("{path}: valid ({} entries, {} keyword definitions)",
                catalog.entries.len(), catalog.keywords.len());
            let heap = catalog.entries.capacity() * std::mem::size_of::<engine_content::descriptions::Description>()
                + catalog.keywords.capacity() * std::mem::size_of::<engine_content::descriptions::Keyword>()
                + catalog.entries.iter().map(|entry| {
                    entry.key.capacity() + entry.text.capacity() + entry.codex.capacity()
                }).sum::<usize>()
                + catalog.keywords.iter().map(|keyword| {
                    keyword.name.capacity() + keyword.definition.capacity()
                }).sum::<usize>();
            println!("CPU catalog: {} bytes inline + {heap} bytes owned capacity; allocator overhead excluded",
                std::mem::size_of_val(&catalog));
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("{path}: {error}");
            }
            ExitCode::FAILURE
        }
    }
}
