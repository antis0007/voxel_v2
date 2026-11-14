# ADR 0032: Observatory guardian recovery

2026-10-26. Accepted for generator 63.

## Decision

Reuse the Ossuary Engine and ordinary reliquary lifecycle at the finite
Observatory. The existing water-contact adapter supplies its environmental
weakness through normal elemental reactions. No new boss AI, resource bins,
damage rules, loot encoding or checkpoint structure is introduced.

The authored encounter owns an explicit identity and boss slot in the existing
record format. Its clearance mask is the boss-death bit, replacing the ambient
guard mask only for that identity. Stage presentation omits an absent guard
prerequisite without writing fake guard kills or clearance. Existing defeat and
claim paths therefore own one-time settlement and retained overflow salvage.
Reward identity resolves consistently during both live play and save restoration.

New awakening requires the resident cradle, its supporting floor and clear
body space. A blocked cradle does not relocate the boss. Saved machines use the
existing exact actor restoration path. The north-aisle relic uses a fixed point,
normal focus and normal claimed-sanctum benefits. The separate lens/archive
route remains available through water changes without requiring a fight.

Inspecting the existing Last window marker teaches Water transfer in the normal
spell log. This makes the portable route discoverable at the site instead of
depending solely on random spell drops. The spell still requires an owned wand.

The release stays version-gated; generations through 62 gain no authored actor
or marker reward. No existing terrain or saves are migrated.

## Evidence boundary

Local offline application check and native build passed. Source review covered
identity validation, clearance/claim flags, Engine checkpoint restore and the
shared reward path. No tests were added or run. Native render receipts belong
in the execution ledger. A successful frame does not prove normal interaction,
combat, capture/reward uniqueness, later reuse, unload/reload or conservation.

The native generator 62/63 comparison is recorded in the execution ledger. Both
`bench-000.png` frames were inspected: the reliquary appears in the authored dry
aisle only at 63. Both native runs reached their final benchmark reports, using
the same frozen build, seed 424242, one-metre scale and camera. This establishes
the actual placement/render path, not the remaining gameplay gates.
