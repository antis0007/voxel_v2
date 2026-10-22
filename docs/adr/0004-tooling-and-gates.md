# 0004 One command surface, filtered output, rust-lld

Date: 2026-10-22. Status: accepted.

`justfile` is the only command surface; `scripts/filter.py` collapses rustc output (errors first, warnings as per-file counts, 60 lines). `just clippy` runs with `-D warnings -A dead_code` until the remaining dead code (two in-progress modules) is resolved; four style lints are allowed in the workspace `Cargo.toml` with reasons. `.claude/settings.json` runs `just check` after edits and `just verify` on stop.

`rust-lld` is the msvc linker: incremental dev builds 4 s instead of 16–22 s (measured in `docs/PLAN.md`). `.gitattributes` stores text as LF. Tools installed on the machine, not vendored: `just`, `tokei`, `cargo-nextest`, `cargo-machete`.
