# 0027: Bounded polling for resident GPU readback

Date: 2026-10-26. Status: library implementation compiled; GPU execution pending.

The resident GPU solver already retains its pool, directory and activity maps.
Its synchronous `read_back` waits for device completion. A future application
adapter needs a pending result that can be polled before advancing gameplay.

## Interface and admission

`GpuWorld::begin_read_back(request_id, byte_budget)` copies the existing resident
buffers into one staging buffer and requests mapping. It returns a
`PendingReadback` whose request ID cannot change. This ID is caller metadata;
it does not by itself establish a world epoch or authoritative simulation tick.

At most three asynchronous requests may be outstanding for a GPU world. Admission
checks their combined staging bytes against the caller's budget and the new
buffer against the device's buffer-size limit before allocating or submitting.
`Busy`, `ByteBudget` and `DeviceLimit` are explicit refusals. No solver step or
world contents change on these failures. `pending_readback_bytes` includes
abandoned requests that are still waiting for their callbacks.

`PendingReadback::poll` uses `PollType::Poll` and a nonblocking callback receiver.
It returns pending, a validated snapshot, or a terminal error. A consumed token
cannot produce another result. Polling does not wait for GPU completion, but
decoding a ready snapshot still copies/scans its bounded resident payload on the
CPU. This is not a measured per-frame cost guarantee.

## Cancellation and ownership

Each request holds a byte/count lease. The mapping callback retains the lease
and staging buffer even if its token is dropped. Cancellation therefore does
not release admission credit while its copy is still in flight. The callback
unmaps an abandoned result; a consumer unmaps a delivered result. An atomic flag
prevents double unmapping when cancellation races the callback. The callback
only sends one result and performs cleanup; it never blocks on a receiver.

The device still needs polling after cancellation so callbacks can complete.
Admission remains closed when outstanding work cannot complete. Device/poll/map
failures are exposed; they do not fabricate a ready snapshot or retry simulation.

## Decode and reuse

Both synchronous and asynchronous readback use the same decoder. It checks the
complete payload size, pool-slot bounds and uniqueness, and cell volume limits
before producing a snapshot. Awake bricks retain canonical ordering. The
existing overflow flags are retained: an application adapter must reject or
recover an overflowing result before publishing it as authoritative.

The synchronous API remains available for its current callers. The
three-request/aggregate-byte admission applies to the asynchronous API.
Returned snapshots are CPU-owned results and need their own consumer residency
budget; the staging lease is released after its mapping/callback references end.

## Evidence and remaining integration

Context7 supplied the wgpu mapping/polling documentation. The pinned local
wgpu 27.0.1 source confirmed the API; no dependency was upgraded or added.
Local `cargo check --offline --locked -p engine_sim_gpu --lib` passed. No tests,
GPU dispatch, readback execution, cancellation exercise or benchmark was run.

This implements the readback prerequisite, not the complete GPU game adapter.
Command upload, epoch/tick ownership, pre-step stalls, retry/rollback and exact
CPU/GPU continuation evidence remain integration work. The application still
uses the isolated CPU region or its ordinary legacy authority.

Follow-on [ADR 0028](0028-isolated-resident-gpu-adapter.md) adds the isolated
resident adapter. Staging may now be reserved before submission through
`reserve_read_back`; the reservation's `capture` records subsequent queue work.
Runtime equivalence and recovery acceptance remain pending.
