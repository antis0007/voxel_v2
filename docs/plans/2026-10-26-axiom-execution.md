# Axiom execution ledger

2026-10-26. User authorized continuing through all remaining Axiom milestones.
This reopens the Axiom track, preserves the existing project scope and leaves
unrelated parked packages alone. The original documents remain design inputs;
completion requires implementation and evidence, not another roadmap rewrite.

Standing constraint: do not add or run tests or hosted CI. Local compilation is
allowed. Runtime and acceptance claims remain separate from source completion.
Other sessions are editing this checkout; only attributable changes are listed.

| Milestone | Implemented in this chat | Remaining gate |
|---|---|---|
| Exact isolated CPU continuation | Cells, reserved topology, awake flags, queued edits, real clocks, delayed snapshots; bounded checksummed files | Runtime round-trip and resumed/uninterrupted comparison |
| Isolated application persistence | One envelope for the paged world, exact CA cut, player/input/day state; previous-slot recovery; retained region references; window save/autosave/close routing; bounded persistence worker and version-safe incremental acknowledgement | Runtime continuation/interruption evidence and capture frame-cost acceptance |
| Acknowledged edits | Conditional observations; applied/no-op/conflict/protected results; shared command/result capacity; saved stone/sand escrow, conflict refunds and mining rewards; shared window/headless terrain input | Runtime inventory-full/conflict/continuation evidence; general effect ingress |
| Windowed isolated authority | Existing exact sessions load with CPU/GPU selection, published collision/rendering, owned hand tools and saved input deadlines; shared batch cutoff and collision reservations | Native traversal/edit/save evidence, full material/effect migration and multi-region authority |
| Shared placement effect | Ordinary block placement commits and publishes before hotbar debit | Gameplay admission/rejection and save/reload evidence |
| Revision-safe CA publication | Immutable world snapshots, water projection, changed dependency halos, existing final GPU stale checks; opt-in window consumption | Rendered traversal/edit/save evidence and multi-region publication |
| GPU readback lifecycle | Bounded polling tokens, aggregate staging-byte admission, retained cancellation leases, shared validated decoder | Application command upload, epoch/tick ownership, stalls/recovery and device evidence |
| Isolated GPU adapter | Resident command upload, one retained job, pre-step stalls, conditional outcomes, exact CPU-cache publication and checkpoint activation | CPU/GPU runtime equivalence, failure/recovery and device acceptance |
| Baseline laboratory (A) | Frozen native static capture inspected; separate physics-on timing run; arguments, hashes, frame/process logs retained | Motion/edit/teleport evidence, tail-frame/backlog improvement, release/device/preset acceptance |
| Finite Observatory (C/O1) | Generator 60 finite pool, excavatable drain and lower basin; existing powered pump route; water-gated lens and archive reward; native before/after chamber captures | Both routes in normal play, conservation accounting, streaming/reload and reward access |
| Portable intervention (D/O2) | Spell 95 uses the pump's shared finite planner/commit; both endpoints admitted atomically; payment after commit; discovery and card integration | Normal casting, denied admission, conservation, streaming/reload and balance at each voxel scale |
| Guardian and reuse (D/O3) | Generator 63 Observatory reliquary and persistent Ossuary Engine; real water feeds Wet/shock/frost; once-only shared clearance/reward and claimed sanctum reuse; marker teaches transfer | Normal awakening, environmental combat, capture/defeat, claim, reuse and unload/reload evidence |
| Causal scaling (E) | Existing bounded legacy solvers and cached factory networks retained | Component invalidation, background continuation contracts, sleep/wake and backlog gates |
| Research expansion (F) | Existing fine-brick/circuit substrate retained | Measured mixed scales, GPU residency, fluid/structural kernels and model exchanges |

Implementation contracts: [ADR 0025](../adr/0025-cpu-region-checkpoints.md) and
[ADR 0026](../adr/0026-acknowledged-edits-and-ca-publication.md), and
[ADR 0027](../adr/0027-bounded-polling-resident-readback.md), and
[ADR 0028](../adr/0028-isolated-resident-gpu-adapter.md), and
[ADR 0029](../adr/0029-isolated-application-checkpoints.md), and
[ADR 0030](../adr/0030-finite-observatory-gallery.md), and
[ADR 0031](../adr/0031-shared-portable-water-transfer.md), and
[ADR 0032](../adr/0032-observatory-guardian-recovery.md), and
[ADR 0033](../adr/0033-cellular-inventory-escrow.md), and
[ADR 0034](../adr/0034-windowed-cellular-session.md), and
[ADR 0035](../adr/0035-background-cellular-checkpoints.md).
The main architecture review's A-F gates remain the acceptance ordering.

Evidence: local offline application compilation passed after the latest source
increment; the GPU simulation library also compiled after its readback changes.
The application also built after isolated application persistence integration.
Existing warnings remain.
The portable transfer passed offline check and build. Its first native startup
rejected description copy; the shared catalog correction is retained. The retry
(`target/place-review/axiom-observatory-transfer-startup-g60-002`, executable
SHA-256 `12DBE8A6A781A0A3ED65C96F6733CCA7443DB04558B189A4385505990F2C8FCD`)
hit its 120-second wall limit during renderer startup and produced no frame.
This is not successful runtime acceptance of the updated spell. The later
water-contact/frost-preservation changes passed offline application compilation
and the final native build. A concurrent enemy-presentation borrow error briefly
interrupted that build; its existing owner corrected it before the successful
retry. No unrelated presentation code was changed by this increment.
The existing description linter passed with 105 entries and four keyword definitions.
The generator 63 guardian/recovery integration passed offline check and native
build. A frozen executable at
`target/axiom-20261026/observatory-guardian-build/observatory.exe` has SHA-256
`9C12A71B9058805DD9A203CB84F3FF79511AF6D3E4299484F815A7738E91CE5E`.
The same-camera, physics-enabled native runs in
`target/place-review/axiom-observatory-relic-g62-001` and
`target/place-review/axiom-observatory-relic-g63-001` both produced final benchmark
reports and inspected `bench-000.png` frames. The dry aisle is empty at 62 and
contains the new reliquary at 63. The first wrapper reported a missing
`camera.png`; its physics capture was present, and the wrapper now recognizes
that filename without claiming fully resident geometry. The 63 wrapper passed.
These runs do not awaken the guardian or establish combat, rewards or reload.
The earlier generator 59/60 chamber comparison remains the visual evidence for
the finite gallery; it does not cover portable transfer or combat.
No automated tests or GPU command scenarios ran. Two manual native runs completed
and the static frame was inspected; details and remaining limits are recorded in
[the native baseline](../design/AXIOM_NATIVE_BASELINE_20261026.md). These runs do not
validate the headless GPU adapter or checkpoint continuation.
The session-report recipe could not launch
the configured Windows Python alias. No unrelated source errors were repaired.

Continue with general admitted effects and multi-region authority before claiming
a production CA world. The isolated window path now exists in source; its runtime
acceptance is open. The finite site still uses legacy authority and admitted effects.

The inventory ownership increment uses isolated format `VXCA0003`. Absolute
headless `BuildAt`/`MineAt` route through saved bounded claims and the shared
fixed-step settlement. Once inventory work is admitted, standalone region saves
are rejected in favor of the exact application envelope. Offline application
compilation passed; no scenario or automated test ran.
The window clock retains unfinished batches across authority waits and uses the
admitted step count for subsequent presentation. It does not accrue an unbounded
wall-time backlog during waits. This clock work precedes the opt-in window path.
The final offline application build passed with existing warnings. No runtime
claim is made for escrow settlement or the authority-wait clock changes.

ADR 0034 adds `--ca-window=<exact-session-directory>` with CPU/GPU selection and
shared held terrain input. Isolated format `VXCA0004` supersedes `VXCA0003` to
retain the tool slot/deadline. Collision reservations come from saved claims.
Source inspection found and corrected a CPU/GPU admission cutoff difference at
CA-boundary player steps. Offline checks passed during integration; native and
continuation evidence is still absent. No test or scenario ran.
The final offline native build passed after the shared selection/cutoff changes.
An earlier final-link attempt exhausted disk space. Reversible NTFS compression
of this chat's two frozen Observatory executables and completed incremental cache
recovered space; both frozen executable hashes still match their recorded values.
No shared output was deleted. The window path has not been launched or visually
accepted. ADR 0035 subsequently moves disk persistence to a worker; snapshot
capture and actor/footer encoding remain on the event thread.

The background exact-session writer uses one outstanding job and bounded
incremental acknowledgement, preserving newer live records and pending final-save
requests. The synchronous headless path shares its implementation. Offline
compilation passed; interruption, disk-full recovery and frame-time acceptance
are unverified. No test or scenario ran.
