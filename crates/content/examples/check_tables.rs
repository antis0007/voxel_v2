//! Load every eagerly validated content table once and report which ones fail; this command never writes.
//! `cargo run -p engine_content --example check_tables`
//!
//! `cargo check` never executes the table loaders, so a bad row (for example a powder block past the
//! one-byte sim table without `sim_as`) compiled cleanly and only panicked when the game started. The
//! parity swarm runs this after each compile checkpoint.
use std::panic::{catch_unwind, AssertUnwindSafe};

fn main() {
    // Keep the failure text in the report instead of a second copy on stderr.
    std::panic::set_hook(Box::new(|_| {}));
    let checks: [(&str, fn()); 4] = [
        ("assets/materials.ron", || drop(engine_content::MATERIAL_TABLE.len())),
        ("assets/reactions.ron", || drop(engine_content::reactions::ROWS.len())),
        ("assets/environment.ron", || drop(&*engine_content::environment::DEFINITIONS)),
        ("portal layout", || drop(&*engine_content::portal_layout::LAYOUT)),
    ];
    let mut failed = 0;
    for (name, load) in checks {
        match catch_unwind(AssertUnwindSafe(load)) {
            Ok(()) => println!("ok {name}"),
            Err(payload) => {
                failed += 1;
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "panicked".into());
                println!("FAILED {name}: {}", message.replace('\n', " | "));
            }
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
