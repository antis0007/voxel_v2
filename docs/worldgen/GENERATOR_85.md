# Generator 85: Fernwake freight district

## Brief-to-playable scope

The supplied design bible sections 6–8 call for a place with a physical identity,
environmental mechanism, connected geography and a reason to return. The prior
resident prose pass did not deliver that. This increment extends the **existing
far bridge**, rather than placing an unrelated dungeon elsewhere.

| Brief requirement | Implementation |
| --- | --- |
| Civic history expressed by a place | Tall freight hall, customs counters, receiving bay, worker bunk, archive and a lower service loop built around a failed shutter |
| Interoperable machinery | The copper control consumes 16 battery energy units (4 kJ) from a charged, enabled battery within 6 m; factory production supplies that energy |
| Multiple approaches | Power the shutter, remove it using ordinary tools, or take the south stairs and lower route to the receiving bay |
| Persistent change | Opening writes ordinary voxel edits; loaded geometry is the source of truth after save/reload, with no parallel completion flag |
| Inhabitants and practical returns | A receiving clerk with finite daily trades, an actual repairable counter, a saved home and the existing follow/stay interaction |
| Connected infrastructure | Lower inspection channel joins the river under Fernwake's existing bridge; service gallery has an exposed water crossing |
| Comedy with stakes | Official notice labels emergency access; the service ledger records the trapped night shift and later unclaimed meals; the clerk's ledger changes from imperial accounts to refugees' names |

## Route and interaction

Coordinates below are metres relative to the cultivation floor height, except x/z
which are world coordinates. Gate 85 applies to Hills and Ridged worlds.

- Entry: follow the original bridge east through x=535, z=48.
- Freight notice: x=548.5, z=40.5, on the first counter. Look above the counter
  within 3.4 m and use the normal interaction key.
- Copper shutter control: x=564.5, z=44.5. Only actual IronBlock shutter cells
  are retracted; water or player-built obstructions reject the operation.
- Lower bypass: south stair at x=551, z=59; gallery at y=-8, z=85; return stair
  at x=592. It remains usable without power or the clerk's approval.
- Side archive: branch east from the lower gallery to x=609.5, z=79.5.
- Receiving clerk: x=589.5, z=42.5. Repairable counter: x=589.5, z=38.5.

The shutter's iron moves into a visible overhead recess in one bounded atomic
batch (at most 1,024 edits at quarter-metre scale); it is not deleted or converted
to free drops. Player blocks in the recess prevent opening. Motion is a discrete
lift, not an animated rigid-body mechanism.

The shutter is opening-only. Repeated use does not consume power after it opens,
and it never closes on a player. All cells must be loaded and the bounded material
batch must succeed before energy is debited. The shared undercrown interaction
path handles both solo and host-authoritative multiplayer use. Multiplayer
resident trade remains disabled by the existing UI gate; this pass does not
claim to solve personal companion/travel ownership.

No save-format bump. Generator 84 and earlier retain their original samples;
existing worlds are not upgraded or regenerated. New worlds default to 85.
Golden baselines were not regenerated: the user requested no tests.

## Review

Camera modes: `place:fernwake-freight`, `place:fernwake-service`, and
`place:fernwake-receiving`. Compare generator 84/85 using an isolated save.
Screenshots establish rendering only. Traversal, powered debit/retry, bypass
access, clerk interactions, water evolution, persistence and two-player use
still require native acceptance. The rest of the empire and the larger
worldbuilding brief remain unfinished.

Verification in this pass: isolated offline `cargo check` passed with the
default multiplayer feature; scoped `git diff --check` passed. Shared-target
native builds twice waited on another process's artifact lock and were stopped
without touching the other process. No rebuilt executable, screenshots, tests,
or gameplay acceptance are claimed. The isolated check output is
`target/freight85-isolated-check.log`.
