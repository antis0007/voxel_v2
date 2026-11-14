# ADR 0031: Shared portable water transfer

2026-10-26. Accepted for the live legacy world.

## Decision

Spell 95, Water transfer, uses the same finite free-water planner and commit
path as pump extraction and discharge. Units remain 1/512 cubic metre. Legacy
worlds admit whole eighth-cell packets; exact environmental worlds preserve
fractional remainders and sediment-reduced destination capacity.

A world-to-world transfer plans both exchanges before committing either.
Residency, authority, assembly ownership, bounds, aggregate cellular wake
capacity, environmental wake capacity and fractional encoding capacity must all
admit the operation. Both cells then commit under one world revision and publish
their geometry dependencies. No rollback or temporary persisted water buffer is
required. Sediment stays in its cell, matching the existing pump behavior.

The spell selects the water cell immediately before an aimed bed surface.
Subsequent casts discharge into the cell immediately before the receiving
surface. Sneaking reselects the intake. The intake must remain within 12 metres;
the receiving ray also has a 12-metre limit. A cast moves at most 128 machine
units (0.25 cubic metre), limited further by the selected cell and destination.
Base cost is 8 mana per committed cast, with normal deck cost modifiers.
Selection, rejected writes, full destinations and empty sources consume no mana
and do not advance the deck. Multicasts containing the utility are rejected
before any action or payment. Existing casting and sneak bindings are retained.

Only an intake coordinate is held by the spell. Reload discards that selection;
transferred water remains in ordinary world storage. The spell fits the existing
0–95 discovery bitset and save format. It enters ordinary spell discovery and
uses the existing water icon, card and particle/sound systems. No new dependency,
terrain version, save field or separate resource inventory is introduced.

## Environmental encounter connection

Creatures and existing guardians now observe resident water at bounded foot
contacts, including fractional surface height. Contact refreshes Wet and douses
Burning through the existing elemental status rules. This does not remove or
create world water. It enables the existing shock and frost reactions after
water arrives by ordinary flow, a pump or the portable transfer. The projectile
frost fallback preserves a longer reaction slow instead of overwriting it.

This supplies the environmental interaction part of O3. The subsequent
[Observatory guardian integration](0032-observatory-guardian-recovery.md) connects
the existing guardian and claimed sanctum to the site. Runtime combat, recovery
and reuse evidence remains open.

## Evidence

Local offline check/build passed for the transfer and water-contact integration. The first native
startup rejected its description copy; the corrected shared catalog removes the
semicolon and numeric prose. The retry and final compilation are recorded in the
execution ledger. The existing description linter passed: 105 entries and four
keyword definitions. No tests or scripted cast scenarios were added or run. Normal
casting, denied admission, conservation, environmental combat, streaming and
save/reload remain runtime acceptance gates. These effects deliberately reject
an attached CA view until its admitted gameplay ingress is integrated.
