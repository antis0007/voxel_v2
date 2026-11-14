# 0028: Isolated resident GPU adapter

Date: 2026-10-26. Status: source and application build passed; GPU execution pending.

The app now depends on the existing workspace crate `engine_sim_gpu`. No new
external package or wgpu upgrade is introduced. Ordinary window play retains its
existing authority. This extends ADRs 0018 and 0025–0027.

## Activation and continuation

`--headless=<document.ron> --ca-cpu-region=x,y,z --ca-backend=gpu` activates the
resident solver for the same sealed 8-cubed interior. `--ca-backend=cpu` is the
default. Both selectors require the isolated headless region. They cannot enable
the experimental authority in an ordinary saved/windowed world.

Activation uploads all 27 reserved bricks and their exact awake masks. Restoring
a checkpoint first restores committed cells, activity, clocks, accepted commands
and delayed snapshots; GPU residency is then rebuilt from that state. Absolute
kernel tick/phase is retained. GPU resources and readback handles are disposable.

## Submission and commit

The GPU adapter owns one resident world and at most one job. Before any gameplay
writer at an eligible cellular step, it captures an ordered command prefix and
creates a bounded private copy of the acknowledged CPU cache. Shared command
logic computes conditional outcomes and wakes the same dependency neighbors.
This copy is staging state; the CPU reference physics kernel does not also run.

A readback reservation admits the complete staging allocation before GPU cell
edits or the next solver dispatch. Reserved-cell upload validates the complete
edit and awake sets before writing. It changes only initially reserved slots;
the adapter does not resize, compact or move those slots. The acknowledged awake
set plus command wakeups replaces the GPU activity input without waking the world.

Temporary staging pressure returns pending before submission. A permanent
capacity/device-limit error stops admission without changing committed state.

The existing resident solver computes the next state. Readback is captured after
that dispatch. The job retains its original tick, command prefix, conditional
outcomes and candidate cache. Its world and readback token have the same owning
adapter lifetime; an old adapter's completion cannot enter a new region's queue.
New commands admitted after submission wait for the following cellular tick.

While mapping is pending, `prepare_step` returns false before player/gameplay
updates. It retains inputs, clocks, the accepted queue and the last published
view. Repeated polls do not resubmit physics or allocate another command batch.
The headless driver waits for actual completed steps, with a 30-second wall-time
failure limit outside simulation logic. It never counts a stalled step as done.

Only a ready result with the original tick, zero overflow, all 27 reserved owners,
supported exact cells and unchanged sealed boundary can commit. Commit replaces
the acknowledged cache, resolves precisely that command prefix, and publishes
the existing S[max(0,n-2)] mirror. A missing/error/overflow result consumes no
accepted command and advances no gameplay tick. There is no silent CPU fallback.

Errors stop this development runner; they do not provide a durable command
journal across process failure. Checkpoints still capture the committed CPU cut
and its pending commands, not speculative GPU work. Retrying from a checkpoint
reconstructs residency instead of publishing a partially evaluated state.

## Bounds and metrics

The pool remains 27 bricks. A candidate clone contains one bounded copy of that
pool and its directory/activity metadata. Accepted commands and unconsumed
outcomes share the existing 128-slot limit; each job holds at most 32 commands.
GPU staging is admitted under 160 KiB. Readback decoding uses the same bounded
resident snapshot implementation as ADR 0027. No world-wide transfer is added.

Only the selected cell writes cross to the GPU during command ingress. The
development adapter still reads its entire small pool after each step to serve
collision queries, exact checkpoints and conditional command validation. This
is an integration path, not a demonstrated high-throughput production strategy.

Changed-block counts use the resident solver's counter delta. Processed-block
counts are unavailable from that solver and remain zero in the development
snapshot stats; they must not be interpreted as measured zero work. No FPS,
latency, memory reduction or CPU/GPU equivalence result is claimed.

## Evidence and next gates

Local offline application compilation and the debug application build passed.
No automated tests, GPU scenarios, CPU/GPU comparisons or device-loss exercises
were run. The manual native camera baseline is recorded separately and runs the
ordinary renderer; it does not validate this headless GPU adapter.

Full-session persistence, windowed input/clock integration, shared boundaries,
device/recovery evidence and the Observatory route remain open. Model fingerprints
now include the adapter and GPU solver sources; ordinary save/generator versions
are unchanged.
