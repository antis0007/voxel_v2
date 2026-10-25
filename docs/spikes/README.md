# Spike results

Status: protocol in force per [decision 0022](../decisions/0022-master-plan-review.md);
no spike has run yet. Spike inputs and alternatives are in the
[spike register](../plans/2026-10-24-bible-spikes.md).

## Order

S1 kernel shape → S4 CPU reference throughput → S2 liquid model. S6 and S7 wait
for M1.5. S3 is deferred until M1 slice numbers exist.

## Measurement protocol

Each spike gets `docs/spikes/<id>/` (for example `docs/spikes/S1/`) containing:

- `README.md`: scenario name, exact adapter string (as wgpu reports it), driver,
  CPU, build profile, commit, alternatives compared, results table, decision.
- Raw CSV, one row per run: alternative, run index, tick ms, frame ms, awake
  bricks/cells, bytes uploaded.
- **5 runs** per alternative; report **median and p95** of tick ms and frame ms,
  awake counts and bytes uploaded.

The decision is recorded as an ADR in [docs/decisions/](../decisions/README.md)
and linked from the spike README. Compare alternatives on identical inputs;
report failed or unsupported runs instead of dropping them. Interleave A/B runs
and pin to performance cores on hybrid CPUs.
