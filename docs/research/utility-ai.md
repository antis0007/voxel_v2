# Utility decisions and creature service

Reviewed 2026-10-24; N05/N06/N07/N12. Dave Mark's Behavioral Mathematics for Game AI (2009, ISBN 978-1584506843) remains a reference lead. The [author's site](https://www.intrinsicalgorithm.com/IAonAI/) could not be fetched; exact Rain World talk claims and Sims internals were not verified. The algorithm below is our proposal, not a sourced account of either game.

## What we adopt

RON considerations evaluate quantized input curves into bounded integer scores. A pinned priority/ID tie break chooses from legal actions; hysteresis prevents oscillation. Inputs include needs, sensed hazards, job state and reachable reservations.

## What we reject

Do not score every mob every frame, mutate authoritative state from a thought query, or script the crew breach response merely to pass C-F1. Do not claim creature quality from a decision score alone.

## What we improve

N06 emits shared actions under fixed instruction/service quotas; N07 defines distance divisors and wake interrupts. Track decision backlog, invalid-path rate, need failures and job completion; prove seal/repair/evacuate through the same utility mechanism.
