# Headless Foundry receipts (integrated v2)

Integrated in the repository after isolated review and tests. No engine files, saves, launchers, UI or runtime instrumentation changed. No dependencies added.

## API

Use the existing foundry_perf.py --input DIR --output NEWDIR --context FILE. Input contains completed stdout.log/stderr.log and manifest.json. The existing 64 MiB snapshot limit, digest verification, fresh output directory and raw re-analysis remain in use. New analyzer digest is retained and checked by foundry_perf_compare.py.

Context workload_kind is explicitly headless-chunks or headless-worldgen. Omitted/native retains the original native path. Invalid headless evidence raises ValueError rather than emitting a success report. No FPS, frame distribution, population or physical-work metrics are manufactured.

Required context: workload_id, hardware object, build.configuration_hash SHA256, build.source_snapshot SHA256, world_config {seed, generator, voxel_scale, world_type: Planet, full_config_sha256}, geology_cache_state hit or miss-or-disabled, expected_output. Manifest must record exit_code:0, sha256 executable hash and identical world_config. These hashes and hardware fields are CALLER-SUPPLIED provenance, not executable inspection or independently verified runtime configuration. Raw chunk receipts independently cross-check seed/version/scale; worldgen-only cold logs do not print version and report that gap. Full_config_sha256 must come from the complete canonical serialized config, not a reduced selected-field object.

Chunk context also supplies chunk_coordinates in exact execution order; expected_output includes world_checksum, checksum, blocks and solid_count. Worldgen-only expected_output contains world_checksum. Worldgen-only currently accepts completed Rust test harness logs with one spherical summary, not arbitrary native startup traces. A log containing chunk results cannot be relabelled worldgen-only.

Spherical spans and chunk setup/generate/wall spans retain exact field provenance. Optional slope/watershed/chunk/cold-builder/history diagnostics retain individual values and counts, never sum across nested or concurrent builders. They do not contain causal parent IDs, so the report explicitly says parent identity is unavailable. No post-hoc guessed parent assignments.

Comparison reuses existing checked_report raw/analyzer reconstruction. Headless comparison rejects differing workloads, full config hashes, scales, ordered chunk lists, cache states, output hashes/counts, hardware/build configurations, duplicate source captures and duplicate raw log evidence. One executable per variant is required. Three or more runs per variant enable the existing run-level bootstrap. Results remain diagnostic-only and never authorize merge. No independent executable/source-binding validation or performance acceptance threshold is introduced.

## Tests

python -I scripts/test_foundry_perf_headless.py -v
python -I scripts/test_frame_bench.py

Ten headless tests (with subcases) and eleven existing native frame parser tests passed. Portable fixtures include six unmodified actual 1m/quarter cold/warm logs; initial screening also parsed all fourteen retained fine-reuse logs. Test provenance fields deliberately use dummy hashes/hardware and are not real benchmark reports. Tests cover missing/malformed/incomplete/duplicate output, nonfinite/zero/negative durations, index/order/count/coordinate bounds, mismatched scale/version/checksums, failed process, raw/report tampering, comparison incompatibility and duplicate evidence. Frozen native analyzer compares identical missing/partial/completed-native results to the new native branch. No new engine execution took place.

## Limits

This is a CLI/report/agent-packet integration, not a Forge web panel. It records a single completed process, no run orchestration or automatic instrumentation. Digest checks detect subsequent changes, not dishonest caller input. Bit checksums are regression evidence, not proof of all state or saves. Mixed or multi-world process logs are intentionally rejected. No automated deployment, Rust build, visual or multiplayer validation was performed.

## Instrumentation identity revision

Use headless-foundry-v2.patch as the complete patch, or instrumentation-identity.patch after the first headless-foundry.patch (not both). Updated hashes: source-hashes-v2.json. The original patch/hashes remain as checkpoint evidence.

Context now requires instrumentation: {profile: nonempty-string, diagnostic_kinds: []}. For a probe without optional timers use profile "probe-only" and an empty list. Declared kinds must be unique known tags: slope-build, watershed-build, cold-builder, chunk-build, history-stages. Every observed kind must be declared. Workload identity stores the profile, sorted declared kinds and sorted observed kinds, but no dynamic node/build counts. Comparisons therefore reject differing profile or kind sets, including a declared timer profile that emitted no diagnostics versus one that did. This is conservative: a legitimate zero-build run may be incomparable. Hidden instrumentation that prints none of these tags still depends on the caller's truthful profile declaration and binary/source binding.

Metric comparisons now reject missing, empty, differently keyed, nonnumeric, nonfinite or nonpositive metric dictionaries before indexing. Final Python-only verification passed 10 headless tests plus 11 existing native tests. Added cases cover missing/blank profiles, duplicate/unknown kinds, undeclared observed kinds, mixed profiled/unprofiled comparisons, differing observed sets, unchanged identity with different counts, and invalid metric sets.


Integration: all 10 headless and 11 native tests passed on shared code. See the worldgen performance review for guarded integration receipts.
