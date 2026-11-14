use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest path"));
    let path = root.join("../../crates/app/src/world/farming/jobs/crew.rs");
    println!("cargo:rerun-if-changed={}", path.display());
    let source = fs::read_to_string(path)
        .expect("current app crew source")
        .replace("\r\n", "\n");
    let start = source
        .find("pub(super) fn next_wake(")
        .expect("next_wake signature");
    let end = start
        + source[start..]
            .find("\n}\n")
            .expect("next_wake closing brace")
        + 3;
    let function = &source[start..end];
    assert!(
        function.contains("-> u64"),
        "unexpected next_wake interface"
    );
    let jobs_path = root.join("../../crates/app/src/world/farming/jobs.rs");
    let time_path = root.join("../../crates/core/src/time.rs");
    println!("cargo:rerun-if-changed={}", jobs_path.display());
    println!("cargo:rerun-if-changed={}", time_path.display());
    let jobs = fs::read_to_string(jobs_path).expect("app pulse source");
    let time = fs::read_to_string(time_path).expect("core step source");
    let pulse = jobs
        .lines()
        .find(|line| line.starts_with("const PULSE:"))
        .expect("app pulse constant");
    let step = time
        .lines()
        .find(|line| line.starts_with("pub const PLAYER_STEP_SECONDS:"))
        .expect("core step constant");
    let compiled_reference =
        format!("mod engine_core {{ pub mod time {{ {step} }} }}\n{pulse}\n{function}");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    fs::write(output.join("current_next_wake.rs"), compiled_reference)
        .expect("write isolated reference function");
}
