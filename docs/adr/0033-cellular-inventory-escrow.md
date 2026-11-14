# ADR 0033: Cellular inventory escrow

2026-10-26. Accepted for the isolated cellular application session.

Conditional terrain edits need an inventory owner across delayed results. A
successful queue admission reserves one selected stone or sand block for a
placement into published empty space. Rejection leaves the selected stack alone.
Mining accepts a published full stone or sand cell and promises one block only
if that conditional removal succeeds. Water, protected cells, machines and other
materials remain outside this ingress contract.

Claims belong to command sequence IDs and share the existing 128 pending/result
slots. They do not enlarge queue capacity. Applied placement consumes its escrow;
failed placement returns it. Applied mining produces its block; failed mining
produces nothing. Settlement waits until the outcome's CA tick is published.
A full inventory retains the result and claim, with FIFO backpressure, until the
block fits. Generic outcome consumers cannot discard owned results. The shared
fixed-step simulation retries settlement on each completed player step.

`BuildAt { at, slot }` and `MineAt { at }` expose this path in the headless driver.
These absolute-coordinate actions are driver inputs, without player reach, aim,
mining duration or collision admission. Window tool ingress still needs those
checks and routing. Existing `PlaceAt`, `EditAt` and `Water64` remain fixture edits.
No scenario or automated test was added or run for this change.

The isolated region format becomes `VXCA0003`. Each claim is checked against its
pending command or retained result on decode. The source model identity includes
the ownership implementation. Any region that has admitted inventory-owned work
must thereafter save and restore through the exact application envelope, which
contains both inventory and claims in one cut. Standalone region checkpoint APIs
reject it even after all claims settle; a cells-only restore cannot reproduce
inventory costs. Ordinary world save formats and historical generation are
unchanged. Earlier development-region formats are explicitly rejected.

Local offline application check and native build passed. Inventory-full retries, conflict
refunds, CPU/GPU agreement and interrupted session continuation remain runtime
acceptance work. This change does not activate cellular authority in the window.

The window clock now retains a bounded unfinished step batch when `prepare_step`
waits for authority. Retry frames do not accumulate more wall time until that
batch is completed; pause clears it. Weather/presentation durations use only
steps actually admitted before a stall. The legacy frame-driven item-drop pass
is suspended when a cellular view is attached. These changes prepare window
integration; the existing startup guard still rejects windowed CA options.

The follow-on [ADR 0034](0034-windowed-cellular-session.md) adds the opt-in window
driver and supersedes the development format with `VXCA0004`.
