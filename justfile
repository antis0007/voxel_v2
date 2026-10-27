# Every command a developer or agent needs. Output is short on purpose:
# errors first, warnings collapsed to per-file counts, at most 60 lines.
# Windows is the primary platform; recipes avoid shell pipes so they work
# identically from PowerShell, cmd and Git Bash.

set windows-shell := ["powershell.exe", "-NoLogo", "-NonInteractive", "-Command"]

filter := "python -I scripts/filter.py --"

# `crate` is accepted for forward compatibility and ignored: every recipe runs workspace-wide.
check crate="":
    {{filter}} cargo check -q --all-targets --message-format=short

# Validate authored tooltip copy without launching the game or running tests.
lint-descriptions file="assets/descriptions.ron":
    cargo run --offline --quiet -p engine_content --example validate_descriptions -- "{{file}}"

# Report player-visible text that breaks crates/app/src/egui_ui/UI_RULES.md.
lint-ui:
    python -I scripts/ui_text_lint.py

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

# Linked worktrees and why each is kept; `just worktrees remove` drops the clean, idle, saved ones.
worktrees action="report":
    python -I scripts/worktrees.py {{action}}

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

# Automated half of the seam_route gate check: chunk halos, padded chunks and neighbours agree
# along a fixed route of negative and far-out chunk coordinates (docs/TESTING.md, Seam route).
seam-route:
    {{filter}} cargo nextest run --status-level fail --failure-output immediate --hide-progress-bar --cargo-quiet seam_route

# Frame-time scenes on the windowed game (GPU and desktop needed): one pass per scene,
# printed against crates/app/benches/frame_baselines.json.
frame-bench scene="":
    cargo build -q --profile release-fast -p minecraft_style_voxel_prototype
    python -I scripts/frame_bench.py run {{scene}}

# Frame-time gate for render changes: 3 interleaved passes, best p95/p99 over +5 % or median draws over +10 % fails.
frame-bench-check scene="":
    cargo build -q --profile release-fast -p minecraft_style_voxel_prototype
    python -I scripts/frame_bench.py check {{scene}}

# Record frame-time baselines (3 interleaved passes). Commit them with the reason.
frame-bench-save scene="":
    cargo build -q --profile release-fast -p minecraft_style_voxel_prototype
    python -I scripts/frame_bench.py save {{scene}}

# Files a clean clone of `rev` would lack: `mod`, include_bytes!/include_str!, Cargo path
# dependencies and crates/* members that point outside the commit. Exit 1 on any.
deps rev="HEAD":
    python -I scripts/tracked_deps.py --rev {{rev}}

# Untracked files grouped into the commit sets their tracked referrers need, as markdown.
deps-inventory:
    python -I scripts/tracked_deps.py inventory

# The single pre-commit gate: check, clippy, tests, the perf gate, clone completeness, then the LOC report.
verify: check clippy test bench-check deps loc

# Run the checks the current task diff requires and record evidence (docs/TESTING.md).
gate:
    python -I scripts/workflow.py verify

gate_status:
    python -I scripts/workflow.py status
