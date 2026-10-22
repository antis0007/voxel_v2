# 0002 Content tables live in assets/*.ron, embedded at compile time

Date: 2026-10-22. Status: accepted.

Materials, spells, recipes, machines and bestiary rows moved from Rust tables to RON files. Each file was generated from the old code by an ignored `dump_*_ron` test, so values are identical, and a `*_ron_round_trips` test keeps the file the single source of truth.

Choices: files are embedded with `include_str!` (no runtime paths, binary stays self-contained); strings are leaked once on load so the row types stay `Copy` with `&'static str` fields and no accessor signature changed; enums such as `BlockId` and `Kind` stay in Rust because match arms across the codebase name them. Dependencies added: `serde`, `ron`.
