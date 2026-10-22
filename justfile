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

# GPU tests: `gpu_*` tests that `just test` ignores, run one at a time on
# engine_gpu::test_adapter(); fail (never skip) without an adapter. Backend
# from WGPU_BACKEND, default vulkan.
test-gpu name="":
    {{filter}} cargo nextest run --run-ignored only --test-threads 1 --status-level fail --failure-output immediate --hide-progress-bar --cargo-quiet -E 'test(/::gpu_/)' {{name}}

# Criterion benches (every crate with a benches/ dir), one run, median vs the
# committed baseline in crates/<crate>/benches/baselines.json.
bench name="":
    python -I scripts/bench.py run {{name}}

# Perf gate: median of 3 runs, fails above +8% or on a bench with no baseline.
bench-check:
    python -I scripts/bench.py check

# Record new baselines (median of 3 runs). Commit them with the reason.
bench-save name="":
    python -I scripts/bench.py save {{name}}

# LOC, test, scenario and bench deltas since the branch point (or a given rev).
session-report base="":
    python -I scripts/session_report.py {{base}}

# Free disk: delete the dev, release and release-fast build outputs. Review
# evidence under target/ (captures, perf logs) and bench history are kept.
clean:
    cargo clean --profile dev
    cargo clean --release
    cargo clean --profile release-fast

# Headless, deterministic scenario run; JSON summary on stdout, exit 1 on a failed check.
run scenario="assets/scenarios/smoke.ron":
    {{filter}} cargo build -q --message-format=short
    ./target/debug/minecraft_style_voxel_prototype --headless={{scenario}}

# Lines of code now, and the net change against main.
loc:
    tokei crates --sort code
    git diff --shortstat main ./crates

# The single pre-commit gate: check, clippy, tests, the perf gate, then the LOC report.
verify: check clippy test bench-check loc
