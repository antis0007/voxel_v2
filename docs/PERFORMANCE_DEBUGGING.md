# Runtime performance and debugging map

Source review: 2026-10-29, shared `voxel_v2_backup` checkout. This is a
source-based review, not an FPS benchmark. Preserve active contributors' edits.

## Changes from this review

| Path | Change | Work removed | Contract preserved |
|---|---|---|---|
| `crates/app/src/world/weather/scroll.rs` | Extracted clipmap scrolling; copy overlapping rows in place | Three full texture clones per scroll: 786,432 bytes of temporary snapshots per 128×128 level | Same sample coordinates, exposed-texel evaluation, full GPU upload and row-major summary reduction |
| `crates/app/src/render/streaming/indirect_draws.rs` | Retained draw/byte/run vectors; `pack_arguments` emits runs alongside bytes | Fresh draw/byte vectors, temporary page list and intermediate run vectors | Stable order within each page, argument layout, per-list GPU buffers and empty-batch clearing |

Retained vector capacity is a CPU memory tradeoff, not free memory. Capacity
grows to the largest observed batch. Stable page sorting may still use temporary
scratch. Neither change establishes an overall frame-time improvement without
a matched native measurement.

## Review findings and boundaries

| Subsystem | Current observation | Next useful evidence |
|---|---|---|
| Shadow fitting/submission: `render/shadow.rs` | Selected layers rebuild their draw lists and sort projected chunk depth; the sort recomputes transforms inside its comparator | CPU cull/sort time and GPU shadow time; consider cached sort keys only if this is material |
| Solar shadow cache: `render/shadow.rs` | Angular reuse threshold is now `1e-8`; nearest plus rotating outer layers retain their update budget | Adjacent native daylight frames and shadow GPU cost; never restore long pauses to hide cost |
| Terrain streaming: `render/streaming.rs` | Ownership, upload admission, cancellation and installed mesh publication are intertwined | Missing-near counts, queue age and upload bytes during motion; preserve publication boundaries before extracting helpers |
| Atmosphere: `render/atmosphere.rs`, `render/sky_probe.rs` | Sky terms already use a small shared compute probe; LUTs have input-based reuse | Native terrain/cloud/sky pass timings; no reason to replace this with per-pixel work |
| Weather: `world/weather.rs`, `world/weather/scroll.rs` | Time no longer rebuilds samples; scrolling still needs full GPU upload because addressing is not toroidal | Weather upload bytes and CPU weather stage; partial uploads require a compatible GPU addressing design |
| Multiplayer: `multiplayer/replication.rs` | Unchanged sections are suppressed after serialization/hash; several large state sections still encode each capture | `VOXEL_NETWORK_PROFILE`, bytes and capture time with real containers/players; revision tracking must cover every writer before skipping serialization |
| Scanner: `world/scanner/view.rs` | Delta contributions and topology polling already exist; consolidation preview can still do substantial work after a change | Storage-index rebuild frequency and preview cost; active storage work is left intact |
| Fixed gameplay: `simulation/fixed.rs`, `simulation.rs` | Fixed-step ownership and deferred authority are explicit; tracing is opt-in | Stage stalls versus render stalls; changing tick rate or batching needs separate behavior evidence |
| Frame driver: `main.rs` | Large duplicated lighting/camera preparation remains a readability risk | Extract one shared frame-input owner after coordinating main-loop edits; avoid a wholesale rewrite during concurrent work |

## Start from the symptom

- A whole-frame stall: `VOXEL_FRAME_STAGE_TRACE=1` logs event entry/exit.
- A gameplay-step stall: `VOXEL_GAMEPLAY_TRACE=1` logs stages taking at least 12 ms.
- Weather rebuild/upload suspicion: `VOXEL_WEATHER_TRACE=1` logs scroll level,
  offset, reused/rebuilt texels and full-upload status. It is cached once at
  startup and silent by default. Full refreshes after seed/control edits are
  handled by `weather.rs`; scroll traces cover the movement path only.
- Slow host images: `VOXEL_NETWORK_PROFILE=1` logs capture time and raw/compressed
  byte counts. Low transferred bytes do not prove cheap serialization.
- Render cost: native benchmark output separates GPU shadow, terrain, raymarch,
  sky and composite time. Keep configuration, executable hash, seed, camera,
  resolution, weather, time and streaming state with every result.

Set environment flags before launching the executable. Do not infer performance
from a screenshot, stale benchmark output or a successful compile. Capturing
many PNGs also changes frame timing; measure separately from dense capture.

## Invariants when editing

- Weather scroll reads old rows before overwriting them: positive Y offsets
  visit destination rows ascending; negative offsets visit descending.
  `copy_within` handles horizontal overlap. Summary order remains row-major.
- Indirect page runs refer to the packed byte order, with offsets in bytes,
  not draw indices. Clear runs for empty batches. GPU argument buffers belong
  to individual draw lists; sharing them can make all lists read the last write.
- Shadow matrices and texture layers publish as a pair. Preserve current
  origins, caster coverage, urgent edits and the solar-motion threshold.
- Avoid topology/save/network changes as incidental performance refactors.

## Local verification

Compile: `cargo check --offline -p minecraft_style_voxel_prototype`.
Whitespace: `git diff --check -- <touched tracked paths>`.
No tests or CI are added/run under the current user instructions. Compile and
source review do not certify weather-scroll equivalence or native frame times.
Use disposable saves for runtime reviews; never migrate an existing world just
to obtain a benchmark. Check `docs/TRACKING.md` for actual command outcomes.
