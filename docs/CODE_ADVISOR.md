# Local code review advisor

RTK 0.49.0 is configured in `.codex/hooks.json`. Restart Codex and accept its
project-hook trust review. The native hook rewrites supported shell commands;
unsupported PowerShell syntax passes through. Manual `rtk git status --short`
works immediately. `rtk gain --project` reports estimated command-output savings,
not total session savings. Keep raw benchmark data, profiler traces and full
failure diagnostics outside lossy filtering (`rtk proxy <command>` bypasses it).
The project RTK enablement overrides the older user-global disabled instruction.

## Kev

Upstream: https://github.com/jaredpalmer/kev, release `kev-1.0`.
Model: `jaredpalmer/kev-0.8b@v1.0`, Apache-2.0, local CPU inference.
Runtime and weight cache: `D:\AI\voxel-code-review` (C: has little free space).
Installed upstream commit: `6b719c3c3f367295f6ef336f4f751cf5ff970abc`.
The installed PyTorch is `2.8.0+cpu`; this release auto-selects CPU with that build.
Python entry point: `D:\AI\voxel-code-review\venv\Scripts\python.exe`.
No paid API, cloud upload, GPU inference, training, or game dependency is added.

From the repository:

```powershell
& D:\AI\voxel-code-review\venv\Scripts\python.exe tools/code_advisor.py start
& D:\AI\voxel-code-review\venv\Scripts\python.exe tools/code_advisor.py status
& D:\AI\voxel-code-review\venv\Scripts\python.exe tools/code_advisor.py review --files src/perf.rs
```

The initial download requires `start --online`; subsequent session starts use
the cached weights offline. The server binds only `127.0.0.1:8009`, uses four CPU
threads and no CUDA. Avoid local benchmarks while it is classifying, since CPU
contention still changes performance. Inspect `.code-advisor/server.json` and
verify that PID's command line before stopping it through normal process tools.
Do not terminate an arbitrary process just because it occupies port 8009.

SessionStart starts the hidden server; Rust/WGSL `apply_patch` edits trigger a
background classification when it is ready. Startup/download does not block the session.
Reviews are serialized and throttled to one per 90 seconds. Shell-written edits
are not intercepted: use explicit manual final-patch review after those edits
or after throttling. Hook errors are informational and never block editing.
This workspace sandbox denies localhost sockets (`WinError 10013`); manual calls
need the app's approved outside-sandbox execution. Project hooks still require
Codex's native trust review; end-to-end automatic delivery must be checked after
restart. An unavailable hook is reported, never treated as a successful review.
The hook is configured here for this checkout; fresh worktrees need their own
hook commands pointing to their own script. It has not been scheduled.

The classifier suggests a review area (correctness, performance, visual,
maintainability, or missing context) and whether a concrete simplification merits
investigation. It does not generate a fix. Review the indicated source/callers,
preserve invariants and game behavior, and accept changes only with appropriate
independent evidence. Fewer lines alone is not the goal. Continue to respect the
user's restrictions on tests and hosted CI.

Input is limited to 8,000 characters with explicit truncation metadata. Automatic
review sees the submitted patch, not the whole repository; manual review sees
HEAD-to-working-tree changes for explicitly named source files, plus untracked
source content. Concurrent changes in those same files are included. No files
are automatically staged, reverted, committed or modified by the advisor.

`.code-advisor/latest.json` stores the latest response, input hash, scope and
coverage. A high probability is not proof of correctness, and shipped calibration
has not been established for this Rust/WGSL repository. Missing context, truncated
patches, model disagreement and unavailable service require ordinary review.
No software-engineering quality or token-saving improvement is claimed until
measured on comparable real tasks. Runtime logs and results are gitignored.

To disable locally, remove only the Kev SessionStart/PostToolUse entries from
`.codex/hooks.json`; retain RTK's PreToolUse entry. Model/runtime files remain
available for reuse.

## Installation verification (2026-10-21)

The live endpoint reported `jaredpalmer/kev-0.8b@v1.0`, CPU, torch, float32.
A 6,286-character real `src/perf.rs` diff was classified in 37.081 seconds of
server-reported latency. It returned `insufficient_context` for review focus
and `no_clear_opportunity` for simplification, both with low confidence. This
verifies inference and result recording, not better engineering decisions.
Because that latency would obstruct editing, automatic reviews run in the
background. Windows CPU uses reference kernels; no CUDA kernel packages were
added. The original response is retained in `.code-advisor/latest.json` until
the next successful review.
