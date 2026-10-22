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
test crate="" name="":
    {{filter}} cargo nextest run --status-level fail --failure-output immediate --hide-progress-bar --cargo-quiet {{name}}

# Criterion benches land in Phase 2 (docs/PLAN.md). Until then this is the
# only honest answer.
bench name="":
    @echo "no criterion benches yet; see docs/PLAN.md P2"

# Headless scripted scenarios land in Phase 2 (docs/PLAN.md).
run scenario="":
    @echo "headless scenario runner lands in P2; see docs/PLAN.md"

# Lines of code now, and the net change against main.
loc:
    tokei src --sort code
    git diff --shortstat main -- src crates

# The single pre-commit gate: check, clippy, tests, then the LOC report.
verify: check clippy test loc
