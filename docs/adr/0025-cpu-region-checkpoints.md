# 0025: Exact CPU-region checkpoints

Date: 2026-10-26. Status: source implemented; runtime acceptance pending.

The next foundation after ADR 0018 is checkpoint/resume for its sealed,
headless CPU region. The user's renewed Axiom implementation request reopens
this bounded part of the parked track. Windowed authority and the broader
simulation migration remain separate work.

## State and ownership

`simulation/cpu_region/checkpoint.rs` writes an independent `VXCA0002` file.
The second revision adds conditional observations and unconsumed edit outcomes
([ADR 0026](0026-acknowledged-edits-and-ca-publication.md)); version 1 is rejected.
It retains every cell in the 27 reserved 8-cubed bricks, including empty
reserved bricks, and each brick's awake flag. Rebuilding the awake index does
not change canonical dispatch order. Pool allocation addresses are disposable.

The file also retains the completed player tick, completed CA tick, next command
sequence, all accepted FIFO commands and all retained delayed snapshots. Every
snapshot includes exact cells, original tick, applied sequence, state hash
and step counters. The first retained result remains the published mirror.
Restoration does not advance authority, drain commands, warm up snapshots or
round water through the legacy map. Odd and even player ticks are retained;
the next kernel tick and its two phases follow from the completed CA tick.

The sealed boundary is checked while rebuilding a private backend. Restore
checks the snapshot timeline and command sequence before attaching the region.
The restored backend must match the latest snapshot's cells and canonical hash.
No partially decoded state is published on error.

## File admission and publication

The reader admits at most 160 KiB, bounds all counts before allocation, and
rejects truncation, trailing bytes, checksum failures, unsupported cells and
modified boundaries. Fields use explicit little-endian encoding. Seed, world
type, scale, generator version and aligned region origin must match the session.

A model fingerprint includes the compiled simulation rules and relevant CPU
kernel/adapter source. This deliberately rejects continuations across source
changes, including harmless source edits, instead of claiming a migration.
The format has no cross-version migration promise. The FNV checksum detects
accidental damage and is not authentication.

Writes use an exclusively created sibling temporary file, followed by a file
sync and rename. A failed write does not replace the prior checkpoint. Only the
temporary file created by that attempt is removed on failure. No power-loss
guarantee for directory metadata is asserted.

## Command line

Both options require `--headless=<document.ron> --ca-cpu-region=x,y,z`:

- `--ca-checkpoint-out=<path>` writes after successful document completion,
  including accepted work that is still pending.
- `--ca-checkpoint-in=<path>` restores before the first document action. Supply
  a continuation document; replaying the original document repeats its actions.
- Both may be used together, including the same input/output path.

After building the application:

```powershell
.\target\debug\minecraft_style_voxel_prototype.exe --headless=begin.ron --ca-cpu-region=0,64,0 --ca-checkpoint-out=region.vxca
.\target\debug\minecraft_style_voxel_prototype.exe --headless=continue.ron --ca-cpu-region=0,64,0 --ca-checkpoint-in=region.vxca --ca-checkpoint-out=region.vxca
```

These are usage examples, not executed validation. The destination's parent
directory must already exist. Failed scenario checks do not publish output.
A failed restore stops the runner instead of silently importing a fresh region.

## Scope and remaining gates

This persists the CPU region and scheduling clock, not the complete headless
session. Player pose, inputs, day, inventory and surrounding legacy chunks start
from the continuation document's ordinary session setup. Full-session hashes
therefore need not match uninterrupted play. The region's authoritative and
delayed simulation states are the continuation contract.

The ordinary world-save format and generator version are unchanged. There are
no new dependencies, controls, rendering paths or default-mode behavior changes.

Final compilation evidence is recorded in the execution guide. No tests,
scenarios, benchmarks or game launches were added or run. Round-trip and
uninterrupted/resumed equivalence remain unverified. Acknowledged edits and
revision-tagged snapshot/mesh publication are source-implemented in ADR 0026.
Complete session persistence and the existing GPU backend adapter remain open.
