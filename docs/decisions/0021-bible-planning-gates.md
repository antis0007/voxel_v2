# 0021: Gate new work on the bible's planning process

Date: 2026-10-24. Status: planning gate applied; session-grouping interpretation
is proposed for the B.7 session-2 human review. No runtime architecture selected.

## Context and evidence

The latest attachment repeats the preserved `docs/specs/GAME_BIBLE.md` exactly
(SHA-256 `43f869c8271f54d541203d38c12f1b1fcf8ff028d5e28ed8b2ca7134ecfd0534`).
Its B.1/B.4 prohibit implementation before reviewed system plans; B.7 requires
human review after the master plan and after the first passed core SDDs.
Earlier recovery/description implementation and its evidence remain historical.

B.7's third session contains two spikes, and its fourth/fifth contain several
systems. This conflicts with B.6's one-system-or-spike rule. B.5 also requires
S2 before liquid design, but B.7 has no separate S2 session before session five.
The hardware addendum was absent at this decision's initial intake. The later
[Downloads reconciliation](../plans/download-coverage/README.md) found and
preserved it, closing the input gap; hardware contracts, fallbacks and measured
results remain pending. None of these gaps permits invented results.

## Decision and proposed interpretation

Deliver research and the master review draft now. Continue to S1/S4 only after
the specified human checkpoint. New implementation requires its SDD to pass
all twelve rubric items at 2/2, independently reviewed. Record each revision,
hash and result; after three failed rounds escalate the concrete disagreement.

Propose retaining B1–B5 as ordered stages, with one system/spike per sub-session.
Insert a standalone S2 measurement before the liquid SDD in B5. The master plan
shows the exact proposed order so the human can review the deviation now.
This does not authorize changing fixed brick/chunk decisions through a spike;
a conflicting result needs a separate ADR and its applicable review.

Keep existing saves, controls, generator gates and unrelated dirty work. No
tests, benchmark runs or hosted CI are part of this documentation pass. Planned
acceptance remains required and unpassed; `just verify` is not a receipt here.

## Alternatives and consequences

- Treat earlier recovery as approval of every future system: rejected; it did
  not provide the required SDDs, critic scores or hardware evidence.
- Force literal five-session completion: rejected as a planning assertion; it
  would skip a required experiment or violate the single-system rule.
- Write all SDDs before measurement: rejected; dependent decisions need spike
  numbers and known hardware definitions before they can pass the rubric.

The cost is additional bounded design sessions. The benefit is a reviewable
scope with one owner per concept and claims whose evidence is still honest.
The human may revise the session grouping at the current checkpoint; any
later process change records a superseding ADR rather than editing history.
