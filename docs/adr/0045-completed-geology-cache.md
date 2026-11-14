# ADR 0045: disposable completed-geology cache

Date: 2026-11-10
Status: accepted for generator 287 planets, following exact-state and saved-edit validation.

## Decision

Reuse the complete immutable geological solve immediately after `Plates::evolve`, then run the existing downstream terrain pipeline. Cache all evolved plates, motion history, crust, full strata records, ice and deep-time state, plus all 241 loading-history frames. This is not a player-save format or a final-height approximation. All decoding succeeds before publishing any restored state. Unsupported versions, missing/corrupt/unreadable entries, allocation failures and write failures retain the original cold-generation path.

Bind entries to schema, SHA-256 of the exact executable, every raw `WorldSampleConfig` field, recipe presence and dial values, civilization setting, shelf flag and atlas radius. Check regenerated mesh identity. Exhaustive configuration and Plates patterns force review of future fields. Any future external solver input must enter the key. Rebuilding conservatively invalidates prior entries, even when the code change is unrelated.

Add `sha2 = "0.10.9"` using the existing locked package rather than inventing hashing or serialization. SHA-256 binds executable identity and the complete storage envelope; it detects corruption but is not authentication against a hostile local writer.

## Storage and controls

Use a dedicated disposable cache, separate from save/session storage. Default application location is `LOCALAPPDATA/VoxelWorkshop/cache/geology-v1` on Windows, or the equivalent XDG cache location. `VOXEL_CACHE_DIR` selects the general cache root; `VOXEL_GEOLOGY_CACHE_DIR` explicitly selects a dedicated geology directory. `VOXEL_NO_GEOLOGY_CACHE` disables reuse. Paths must be absolute plain directories with no reparse/symlink ancestors. Explicit overrides are caller responsibility, not a hard save-path exclusion.

Ordinary benchmark invocations remain cold unless an explicit geology directory is supplied. Library callers opt in before workers start. Configuration is process-wide and cannot change after first installation.

Bound payloads to 160 MiB, committed entries to two, and engine-owned file usage including temporaries to 512 MiB. Writers take a nonblocking OS lock, write and sync a unique same-directory temporary, then rename. Valid same-key entries are immutable; recognized corrupt bodies can be repaired. Prune only exact engine-owned filename/header pairs. Unknown files are preserved. All failures fall back without changing saves. Power-loss durability is not promised for disposable cache files.

The encoder requests geometrically growing capacity capped at the payload limit using `try_reserve_exact`; allocator-granted memory and total process memory are not constrained by that file limit. Encoded bytes and decoded state coexist briefly on warm reads.

## Verification

The clean V8 executable passed a headless smoke test. Its no-window seed-74 horizon probe took 119.84 s cold and 11.55/11.55 s warm, with identical generated diagnostic files. CPU time fell from 125.86 s to 15.67/15.83 s. Sampled peak working set was approximately 419 MiB cold and 385/381 MiB warm. This is not interactive time-to-play, GPU/FPS, or first-generation acceleration.

Separate fresh-process worldgen tests preserved complete published world/history hashes for seeds 74 and 731. Native application save tests used real chunk residency, admitted edit commits, region/session persistence and a second process restoring cached geology. The edited voxel, adjacent original terrain, configuration and inventory all survived; saved files remained byte-identical after the warm read. Twenty-eight focused tests and four pinned legacy-history tests passed, including the unchanged generator-283 digest and generator-285 cadence/seam checks.

## Limits and rollback

Only generator 287 planets are eligible initially. Each entry is about 129 MiB. SHA-valid but semantically invalid snapshots safely miss repeatedly rather than automatically repairing; unknown headers or quota-blocking files also disable writes rather than being deleted. The private-directory checks are not a security boundary against an adversarial local writer. Two simultaneous processes can both perform a cold solve. No compression, cloud cache, save migration, renderer change or launcher replacement is included.

Disable configuration or set `VOXEL_NO_GEOLOGY_CACHE` to roll back reuse. Existing saves continue through the original procedural base plus their authoritative edits.
