# Every command a developer or agent needs. Output is short on purpose:
# errors first, warnings collapsed to per-file counts, at most 60 lines.
# Windows is the primary platform; recipes avoid shell pipes so they work
# identically from PowerShell, cmd and Git Bash.

set windows-shell := ["powershell.exe", "-NoLogo", "-NonInteractive", "-Command"]

filter := "python -I scripts/filter.py --"

# `crate` is accepted for forward compatibility; the repo is one crate until Phase 3.
check crate="":
    {{filter}} cargo check -q --all-targets --message-format=short

# dead_code stays a warning until the P4 dead-code pass lands (docs/PLAN.md);
# every other warning is an error.
clippy crate="":
    {{filter}} cargo clippy -q --all-targets --message-format=short -- -D warnings -A dead_code

# Compile warnings are collapsed by the filter; test failures print in full.
# `just test headless` runs the tests whose name contains "headless".
test name="" crate="":
    {{filter}} cargo nextest run --status-level fail --failure-output immediate --hide-progress-bar --cargo-quiet {{name}}

# Criterion benches land in Phase 2 (docs/PLAN.md). Until then this is the
# only honest answer.
bench name="":
    @echo "no criterion benches yet; see docs/PLAN.md P2"

# Headless, deterministic scenario run; JSON summary on stdout, exit 1 on a failed check.
run scenario="assets/scenarios/smoke.ron":
    {{filter}} cargo build -q --message-format=short
    ./target/debug/minecraft_style_voxel_prototype --headless={{scenario}}

# Lines of code now, and the net change against main.
loc:
    tokei src --sort code
    git diff --shortstat main -- src crates

# The single pre-commit gate: check, clippy, tests, then the LOC report.
verify: check clippy test loc
