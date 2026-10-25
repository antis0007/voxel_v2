# Noita engine research

Reviewed 2026-10-24; C03/C04/B02/B07 and C-N5. The linked [GDC talk](https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design) currently redirects to maintenance; exact chunk sizes, checkerboard phases, margins and re-pixelization details remain unverified. The [gameplay note](reference-noita.md) has separate developer evidence for material interaction. No source is evidence of our throughput.

## What we adopt

Propose dirty-region wake lists and deterministic partition scheduling, with separate body/cell ownership and conservative extraction/fusion. S1 compares partition shapes on the actual reference and device paths.

## What we reject

Do not turn secondary descriptions into fixed Noita implementation facts. Do not assume a race-free block partition proves heat, momentum or reaction conservation, or that integer cells make float body coupling deterministic.

## What we improve

Version transfer ledgers and shared CPU/GPU semantics; measure acid/fire/pressure cases against the C-N5 capture rubric. Every claimed conservation law needs its own invariant and residue accounting. No accepted kernel choice or better-than verdict exists before the spike and reviews.
