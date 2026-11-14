# ADR 0034: Windowed cellular sessions

2026-10-26. Accepted as the next application integration step.

`--ca-window=<directory> --ca-backend=cpu|gpu` opens an existing exact application
session in the game window. The directory must contain a compatible
`simulation.vxs` or recoverable previous slot. This path restores the player,
inventory, calendar, command queue and delayed snapshots, then publishes the same
view to collision and the existing renderer. Ordinary saves, benchmarks and
multiplayer cannot be combined with this driver. Session creation remains with
the existing headless session exporter; there is no automatic conversion of an
ordinary world save.

Window and headless drivers now call `simulation::tools::step` for cellular
movement and terrain input. The hand places owned stone/sand into published air
and mines full stone/sand cells within 5 m. Held actions repeat after 12 player
ticks. Pick selects an existing matching hotbar stack. The selected slot and next
tool deadline are saved. Headless `Hold` accepts `place`, `mine` and `pick`;
`SelectSlot` and `Aim` supply the corresponding slot and camera inputs. Absolute
fixture and inventory-command inputs retain their existing purposes.

CPU and GPU admission use the same cutoff: when the next player step is a CA
boundary, newly submitted commands belong to the following CA batch. The GPU
can stage the due batch before that player step, so allowing a CPU command into
the due batch would produce different histories from identical input. Polling
does not change this cutoff. Runtime equivalence has not been established.

Placement rejects an intersecting player body. Accepted placement footprints
reserve player collision until their result becomes visible; they do not alter
the published material view or meshed cells. Reservations are reconstructed from
saved claims and removed at publication even when inventory-full refunds remain
queued. This prevents walking into an accepted placement during publication lag.
Deliberate noclip retains its existing collision bypass.

Window loading installs authority before streaming the initial meshes. Save,
autosave and close use the exact application envelope beside its region store.
A failed final save keeps the window open and reports the error. The ordinary
save service never captures the cellular world. The follow-on
[ADR 0035](0035-background-cellular-checkpoints.md) moves persistence to a bounded
worker; capture frame cost remains a performance gate.
The per-frame legacy dropped-item pass, meteor writes and renderer activity
seeding are suspended. Legacy factories, adventure writers and creative terrain
tools stay outside this isolated mode. Inventory rearrangement remains available;
unsupported external-panel actions cannot replace or mutate this session.

The isolated format is now `VXCA0004`, adding the selected slot and tool deadline
to ADR 0033's ownership data. Previous development formats are rejected. The
application model fingerprint includes the shared terrain step and collision
implementation. Ordinary world versions and historical generators are unchanged
by this increment.

This is still a sealed 8-cell region with its existing material subset. It does
not establish multi-region production authority, general spell/factory admission,
streaming-scale simulation or device recovery. No new test or scenario was added
or run. Native traversal, editing, inventory-full settlement, close/reopen,
CPU/GPU equivalence and visual acceptance remain unverified.

Offline application checks and the final native build passed. A disk-full link
failure was resolved by compressing preserved artifacts and a completed cache,
without changing the preserved executable hashes or deleting shared outputs.
