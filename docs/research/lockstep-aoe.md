# Lockstep and replay

Reviewed 2026-10-24; C01/C15/C20/T04. Bettner/Terrano's [original article](https://www.gamedeveloper.com/programming/1500-archers-on-a-28-8-network-programming-in-age-of-empires-and-beyond) describes synchronized commands and simulation, including determinism challenges. Its networking turnaround is not the engine's two-CA-tick authority contract.

## What we adopt

Version command streams, tick assignment, canonical state hashes and first-divergence reports. Pin hash algorithms, iteration order, worker commit order and bounded service quotas, including behavior at saves and paging boundaries.

## What we reject

Command-only transmission does not create determinism. Do not assume integer arithmetic is sufficient when wall-clock budgets, overflow, unordered containers or thread completion can alter the result; network multiplayer remains deferred scope.

## What we improve

C15 separates simulation time from host scheduling and records every authoritative action. Exact checkpoint/resume includes pending work and publication history. Replay acceptance is a proposed gate, not established by a headless driver existing.
