# Reference: Noita

Research date: 2026-10-24. Scope: A.1 lore, magic, materials and discovery. C-N1–C-N5 remain design obligations, not demonstrated superiority.

## Primary-source findings

Nolla describes a generated action roguelite where material pixels participate in liquids, gases, powders, bodies and simplified chemical, electrical and thermal interactions. Players craft spells and learn across permanent character deaths. These are explicit product mechanisms, not proof that every possible material pair reacts. [Developer site and FAQ](https://www.noitagame.com/)

Developer release notes document continued additions and fixes to spells, materials, perks and secrets. They demonstrate interacting content, but do not establish a complete pair matrix, a count of distinct meaningful spells, or universal discoverability without a wiki. [Developer release notes](https://www.noitagame.com/release_notes/)

The existing [GDC research note](noita-gdc2019.md) separately marks chunk/checkerboard constants as unverified. This analysis does not promote those figures into architecture requirements. Exact per-seed alchemy algorithms and the bible's assessment of fixed-position secrets were not verified from primary implementation sources here.

## What we adopt

Let spells change the same material state as tools, machines and hazards. Present unknown reactions through observable changes and clues tied to actual world conditions. Spell cards compile into ordered operations with inspectable cost, timing and outputs; environment manipulation must be a useful outcome alongside damage (C-N1, C-N2, C-N4). Preserve knowledge gained through failed delves (C-F3).

## What we reject

Do not substitute named loot or a coordinate lookup for a physical secret. Do not count syntactically different card lists as meaningfully different effects. Do not equate a complete default "no reaction" table with rich chemistry, or a larger 3D simulation with improved play (C-N2–C-N5).

## What we improve

Generate secret rules and their clue chains from the same seed record; clues constrain reagents, place, time and conditions without contradicting the resulting recipe (C-N1, C-N3). Require explicit pair coverage and report the non-trivial fraction (C-N2). Define an effect-equivalence metric before counting combinations or useful fuzz discoveries (C-N4). Persistent bases and automation reuse those rules. For C-N5, record comparable acid, oil-fire and water-pressure captures with tick timing on H1; visual review and 60-tps evidence remain separate pending requirements, with liquid and resonance spikes unresolved.
