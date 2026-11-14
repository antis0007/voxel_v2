# ADR 0035: Background cellular checkpoints

2026-10-26. Accepted for exact application sessions.

The window's exact-session save previously performed region persistence, extent
validation, manifest construction, disk synchronization and slot replacement on
the event thread. It now captures one coherent cut into an immutable job and
sends that job to a dedicated writer. The headless synchronous save calls the
same capture/persist/apply implementation.

Only one job may be outstanding, including its acknowledgement phase. Both
channels have capacity one. Save requests made during a job coalesce into a
pending flag; the next cut is captured after that job is acknowledged. Closing
waits for the outstanding job and a later requested final cut. Save errors and
worker termination clear the active state, report failure and leave the window
open. Dirty data and later inventory/CA state remain live for a retry.

Capture retains the existing 128 MiB dirty-region admission limit, bounded actor
and CA sections, and the world's immutable region index. Canonical path checks,
region writes/compaction, extent verification, manifest encoding and atomic
publication run in the worker. The commit identity is assigned at capture so a
slow older cut cannot replace a newer published cut. Existing previous-slot and
immutable-commit retention rules are preserved. No save-format change is needed.

Acknowledgement verifies the world instance and region root, then merges at most
64 records per pass, stopping after a 2 ms time check. It never replaces the live
region index with the captured index or restores captured player/inventory/CA
state. Checkpoint and region-writeback completions now reject records older than
the currently published record version. Later dirty versions stay dirty until
their own acknowledgement. This also closes an existing completion-order hazard
in the ordinary background save path.

Snapshot capture and actor/footer serialization still run on the event thread.
Their frame cost, worker interruption, concurrent writeback, disk-full recovery
and close/reopen behavior remain runtime acceptance gates. No tests or scenarios
were added or run. Offline application compilation passed during integration.
