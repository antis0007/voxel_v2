# Cheap intelligence and networked co-op: how the AI is computed and replicated

Status: design, 2026-11-06. Owner question: how to wire truly intelligent but cheap AI into the
game, and make it work across the internet with friends.

Extends, and must agree with:

- [Axiom: persistent civilization simulation](2026-11-06-axiom-direction-persistent-civilization-simulation.md)
  (shared computational machinery, persistent individual state, transactional commits, the
  learning boundaries).
- [Cities, anchors and history](2026-11-06-cities-anchors-and-history-design.md).

Reuse words: `L` = legacy substrate exists, `P` = design only.

## 1. The one rule: replicate outcomes, not thoughts

A player's machine never needs to know what another player's villagers *think*. It needs to know
what they **did**. So the network carries:

- player input;
- **accepted outcomes** (an order, an edit, an ownership or stock change, a completed project);
- compact entity snapshots for nearby bodies;
- **cohort summaries** for everyone else;
- events that other systems consume.

and it never carries weights, activations, or per-agent neural state. This is the single
decision that makes intelligent populations affordable online: **intelligence is a local
computation whose result is a small fact.**

## 2. Why it is cheap (the cost model)

Intelligence is structure, not a big model per person:

- The **shared policy** is a few MiB total (16 policies ~3.5 MiB in the direction's numbers), run
  in **batches**, not per agent per frame.
- Most individuals run **no inference at all**: they are event-scheduled activities
  (`population::Schedule`), advanced by arithmetic.
- Only the **decider tier** runs the policy, and only for a **budgeted** subset chosen by
  relevance each tick.
- **Per-agent state is data** (identity, needs, memory, activity), bytes not networks.
- The rest is **explicit**: structured memory with provenance, task contracts, needs, needs ->
  plans -> projects, and a settlement ledger. That is where "believable" comes from.

So cost scales with *decisions taken*, not with population. A million persistent records is a
memory question; a few thousand decisions per second is a compute question.

## 3. One policy, two mirrors

The engine already proves this pattern: `crates/sim` is the CPU reference and `crates/sim_gpu`
is a WGSL port kept byte-identical, tested via `crates/gpu`. AI inference follows the same shape:

- a **CPU reference policy** (the authority, and the thing under test);
- a **batched GPU policy** for the same computation, offloaded through the existing
  `crates/sim_gpu/src/offload.wgsl` pattern;
- an **equivalence test** where the computation is integer and must match, and a
  **bounds test** where it is float and need only stay within tolerance.

But the GPU policy is **authority-restricted** (section 4): it runs on the host. A client may run
the same policy only for **cosmetic, non-authoritative** behaviour (a crowd's local flavour) that
can be corrected without divergence.

## 4. The authority model (already the engine's posture)

The repo already speaks this language: authority and replica appear hundreds of times, and
`crates/app/src/multiplayer.rs` (with `crates/app/src/multiplayer/host_commands.rs`) already
routes host-only mutations through a `Deferred` queue.

| Subject | Owner | Client role |
|---|---|---|
| Society, economy, AI decisions, construction, ownership, stock | **host** | consume replicated outcomes |
| A player's own input, avatar movement, local effects | **client** | predict, then reconcile |
| Nearby bodies (visual) | host | interpolate snapshots |
| Distant populations | host | read **cohort summaries** |
| Purely cosmetic local flavour (spray, idle sway) | client | free to diverge |

The **policy runs only where authority lives.** `may_run_policy(Host, Decider)` is true;
`may_run_policy(Client, Decider)` is false. A client that ran the authoritative policy would have
to agree with the host bit for bit, which section 5 explains is not something we rely on.

## 5. Replication policy: command or outcome

The repo's existing rule, applied to AI: *replicate commands where determinism is proven, and
compact accepted outcomes where it is not.*

| Subsystem | Deterministic? | Replicate |
|---|---|---|
| Integer cellular world, positional hashing | yes (proven) | **commands** (the `lockstep.rs` path) |
| Movement, edits, containers | yes, validated | commands plus server validation (`movement.rs`, `edits.rs`) |
| **AI policy inference** (float, possibly WGSL) | **no** | **accepted outcomes** |

WGSL permits variation in rounding and expression evaluation, so a GPU policy is not bit-stable
across devices. That is fine, because the AI is on the *outcome* side of the line: the host
decides, and ships the **result**, never the reasoning. The AI therefore never needs
cross-device determinism - a large simplification, and the reason "cheap" and "online" are not
in tension.

## 6. What crosses the wire

| Payload | Direction | Note |
|---|---|---|
| Input | client -> host | the only client-authored authority |
| Accepted outcome | host -> all | an order, an edit, ownership/stock delta, a completed project |
| Entity snapshot | host -> all | prioritised, acked delta baselines (`crates/netcode/src/snapshot.rs`, `pose.rs`) |
| Cohort summary | host -> all | counts by region and role; the wire cost of "everyone else" |
| Event | host -> all | something other systems consume |
| Weights, activations, per-agent brain state | **never** | not a payload at any rate |

A cohort summary fits a few words (16 bytes in the prototype model): a settlement with 4,000
people costs the same order of magnitude as one body, not four thousand.

## 7. Prediction, interpolation, reconciliation, divergence

- **Predict** what is local and cheap to correct: the player's own movement and effects.
  `crates/net/src/predict.rs` and `crates/netcode/src/movement.rs` already carry this.
- **Interpolate** every replicated body between snapshots (`crates/netcode/src/interp.rs`,
  `pose.rs`, `rotation.rs`), so a 10 Hz snapshot rate still reads as smooth motion.
- **Replay accepted decisions** that must be shown faithfully: the engine already repeats this
  shape for carried bodies and cell runs (`crates/netcode/src/cells/assembly.rs`,
  `crates/netcode/src/carrier.rs`, `crates/netcode/src/interp/parented.rs`). An AI body's host-accepted decision travels as an outcome the client applies,
  not as a simulation the client re-derives.
- **Detect divergence**: `crates/netcode/src/lockstep/desync.rs` already exists. For the AI, a desync
  check is on **outcomes** (ownership, stock, occupancy), not on internal beliefs, because beliefs
  are allowed to differ by device.

## 8. Bandwidth: individual near, cohort far

- **Bodies near a player** get individual snapshots, prioritised with acked delta baselines
  (`crates/netcode/src/snapshot.rs`) and paced with the existing chunk machinery
  (`chunk_stream.rs`, `chunk_pace.rs`).
- **Everyone else** is a **cohort summary** per region and role. A distant settlement of
  thousands is a few dozen bytes per update.
- **Flood limits** (`flood.rs`) already exist; the AI adds no new flood surface because it emits
  outcomes, and outcome rates are bounded by the settlement tick, not by population.

## 9. Co-op with friends, specifically

- **Ownership is a host decision.** Contested outcomes (a captured settlement, a stock transfer)
  commit on the host and replicate; a client's order is a *request* the host validates, exactly
  like `host_commands.rs` today.
- **Companions are host-authoritative, player-commanded.** A friend orders their companion; the
  order goes to the host, which ticks the pawn and replicates the body. The friend predicts
  cosmetic motion only.
- **Joins and rejoins** get the current compact state (cohorts, owned places, standing orders)
  and then snapshots, not a replay of history. `crates/net/src/session.rs`, `invite.rs`,
  `punch.rs`, `relay.rs`, `transfer.rs` already carry discovery, join and transfer.
- **Frozen shared policy is a feature here**: because the common skill foundation does not update
  mid-session (the direction's rule), a joiner cannot acquire a practice the host lacks, and no
  session transmits knowledge between villages through a weight update.

## 10. How we prove it

| Claim | Test |
|---|---|
| The CPU and GPU policies agree where the maths is integer | equivalence test, the `crates/gpu` / `sim_gpu` pattern |
| The GPU policy stays within tolerance where it is float | bounds test with a recorded variance |
| A client never drives shared state | a test that a client-issued change is a request, not a commit |
| Outcomes replicate without shipping neural state | a wire-size test: a cohort summary is small and no frame carries weights |
| Latency and loss do not break decisions | the netcode **link simulator** (`crates/netcode/src/link.rs`) with the existing test harness |
| Divergence is caught | a `desync.rs`-style check on outcomes |

## 11. Open questions

- Which AI subsystems, if any, a client may compute for cosmetic flavour (crowd idle, pet
  wandering) without risking a visible correction.
- Whether per-agent adaptation is replicated as compact deltas (so friends see the same
  improving carpenter) or kept host-local until an outcome depends on it.
- Whether cohorts are summarised per settlement or per district for the strategy map's detail.
- The decision budget per tick on the host (the `budget` module's constants) and how it yields
  under load without changing committed outcomes.

