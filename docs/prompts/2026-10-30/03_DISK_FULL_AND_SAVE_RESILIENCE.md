# 03: Disk-full handling and save resilience

Written for: a coding agent. Read `00_INDEX_AND_SHARED_RULES.md` first.

## Goal

When the disk fills, the player never loses data silently, the game does not
accumulate unbounded unsaved state in memory, the existing save on disk is never
damaged, and the banner offers a recovery action. No save-format change.

## Observed failure

Screenshot, 2026-10-30: a banner reads "Saving needs attention. Region write
failed (1 in a row, retry in 2 s): There is not enough space on the disk. (os
error 112). Unsaved Changes". C: had 0 bytes free. The banner is drawn at
`crates/app/src/egui_ui/front_end.rs:832`. The region writer is in
`crates/app/src/world/storage/background.rs` (temp-file write at about line 277
via `path.with_extension(format!("{serial}.tmp"))`). Other writers use the same
temp-then-rename pattern: `keybinds.rs:336`, `preferences.rs:511`,
`world/block_data/sidecar.rs:192`, `world/farming/sidecar.rs:129`,
`world/travel/sidecar.rs:137`, `world/portals/sidecar.rs`,
`egui_ui/map/sidecar.rs:202`, `multiplayer.rs:1153`,
`simulation/cpu_region/checkpoint/file.rs:30`.

## What to find out first (read, do not assume)

1. How `background.rs` queues region writes: the retry policy (1 in a row, retry
   in 2 s), whether failed jobs are re-queued with their chunk payload kept
   (memory growth), what "Unsaved Changes" counts, and how the failure state is
   exposed to the UI.
2. Whether a failed temp write is deleted. A leftover `*.tmp` of a few MB per
   region per retry can itself fill the disk faster. Check the directory of an
   affected world on disk for stray `.tmp` files.
3. Whether the final rename can leave a missing or truncated region file when
   the temp write succeeded partially. The correct order is: write temp,
   `sync_all`, rename over the target. On Windows `fs::rename` replaces an
   existing file; confirm this holds for the toolchain in use.
4. What each sidecar writer does on error (swallow, log, return). A writer that
   ignores `Err` loses data without a banner.
5. Whether `Cargo.toml` already depends on a crate that exposes free disk space
   (for example `windows-sys`, `fs2`, `sysinfo`). **No new dependency without
   an ADR line.** If none exists, do not add one; skip the free-space readout.

## Design

A. **Classify errors once.** Add a small helper in the storage module (for
   example `storage/io_error.rs`) returning an enum: `DiskFull`, `Denied`,
   `Locked`, `Other`. Windows: `raw_os_error() == Some(112)` (ERROR_DISK_FULL)
   and `Some(39)` (ERROR_HANDLE_DISK_FULL); Unix: `Some(28)` (ENOSPC) and
   `Some(122)` (EDQUOT). Everything else maps by `ErrorKind`. Unit-test the
   mapping with synthetic `io::Error::from_raw_os_error`.
B. **Atomic and bounded writes.** Keep temp-then-rename. On any failure delete
   the temp file (best effort, ignore its own error). Prefer a temp name tied to
   the target so a crash leaves one stale file per target, not one per attempt,
   and clean stale temps for the world at load time (match the writer's own
   naming; never delete files that do not match it).
C. **Bound memory while blocked.** When the failure class is `DiskFull`, stop
   generating new write jobs from the retry timer faster than one probe per
   backoff step. Coalesce: keep the latest payload per region, not every
   revision. Backoff 2 s, 4 s, 8 s, capped at 30 s, reset on success. Retry
   immediately on an explicit player request.
D. **State surface.** Extend the existing save-status struct with the class,
   the count of unsaved regions, and the failing path's drive or directory. Do
   not add a second status channel; reuse what `front_end.rs:832` already reads.
E. **Banner (UI_RULES.md applies).** States and nouns, numbers with units, no
   second person, no sentences that explain the screen. Example:
   `Disk full` as the title, then `C: · 14 regions unsaved · retry 8 s`, with
   two actions: `Retry now` and `Save copy to...` (folder picker if the UI
   already has one; otherwise only `Retry now` and `Quit without saving` behind
   a confirm). The existing title "Saving needs attention" may stay for
   non-disk errors. Lint with `just lint-ui`.
F. **Exit safety.** If the player quits while regions are unsaved and the last
   failure was `DiskFull`, show the unsaved-region count in the quit confirm
   instead of quitting silently. Autosave on exit must report failure.
G. **Other writers.** Route each sidecar writer's error through the same status
   surface or log it at warn with the class. A writer that currently drops the
   error should at least raise the banner once.
H. **Optional, only if cheap and dependency-free:** a pre-flight guard before
   large writes using whatever the codebase already has. If not available, skip.

## Tests (headless, no real full disk)

- Error classification unit tests (Windows and Unix codes both).
- A writer test using a path whose parent is a read-only or nonexistent
  directory to exercise failure; assert the original target file is byte-equal
  before and after a failed write, and no `.tmp` is left.
- A scheduler test with an injected failing sink: N failed attempts keep the
  payload count at the number of distinct regions, backoff steps follow the
  schedule, and success resets state. Use an injected clock or tick counter;
  no `Instant::now()` in simulation logic.
- Existing save tests: `just test save`, `just test storage`.

## Manual verification

On a throwaway world, point the world directory at a small RAM disk or a
quota-limited VHD and play until it fills, if the machine allows it. Record the
banner and that the world loads after space is freed with the last good state.
If not possible, say so; the injected-failure tests are then the only evidence.

## Do not

- No save version bump, no new file formats, no new dependency.
- Do not delete user worlds, caches or `target/` contents as a "recovery".
- Do not suppress the banner to hide the problem.
