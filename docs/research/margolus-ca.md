# Block cellular automata

Reviewed 2026-10-24; C01/C04/C05-C09. [MIT Press](https://mitpress.mit.edu/9780262200608/cellular-automata-machines/) confirms Toffoli/Margolus (1987), including Margolus neighborhoods, reversibility and physical modeling. The [author-hosted book](https://people.csail.mit.edu/nhm/cam-book.pdf) exceeds the browser fetch limit; chapter-specific derivations were not re-read here.

## What we adopt

Propose shifted disjoint 2x2x2 blocks with pinned phase order and hashes. If each local operation preserves the sum of a conserved quantity, composing those operations preserves that sum; halo and cross-brick transfers must satisfy the same premise.

## What we reject

A partition alone does not imply reversibility or conservation of arbitrary reaction/heat rules. Destructive reactions are not permutations. Eight parity phases and alternating two partitions are different schedules, requiring an explicit S1 choice.

## What we improve

Specify phase-by-phase reads/writes, border ownership, reaction source terms and integer residuals in C04. S1/S4 provide cost and parity evidence before any schedule replaces the current implementation.
