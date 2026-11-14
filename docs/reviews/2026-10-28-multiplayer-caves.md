# Multiplayer combat and underground review

Read against current source: `00_MASTER_DESIGN_AND_INTEGRATION_V2.md`,
`MASTER_PLAN.md` (ownership and feature routing), `specs/MOD_ADDENDUM.md`,
`superpowers/specs/2026-10-26-multiplayer-design.md`,
`design/MULTIPLAYER_PREDICTION_20261026.md`,
`design/MULTIPLAYER_SOCIAL_20261026.md`,
`design/MCRS_PARITY_AND_MINING_20261028.md`, `design/UI_MOTION_20261028.md`,
`SHADER_AND_NETCODE_STUDY_20261026.md` and `TRACKING.md`.

## Implemented in this increment

- Remote character poses carry host-derived melee progress and duration.
  Clients advance it between updates; arm and sleeve geometry share the joint.
  This does not add client damage authority, shield blocking or remote held items.
- Shadow pipelines use the existing driver pipeline cache.
- Camera enclosure darkens the sky and suppresses cloud compositing deep below
  terrain. Deep cave terrain bypasses outdoor atmosphere and fades to darkness.
- Generator 73 replaces natural wet gallery floors with sparse water pockets
  at the water table. Previous generator versions, authored water areas and saved
  player water edits retain their behavior. This is not an existing-save drain.
- `cave-dry` in the disposable receiver review compares generated water against
  version 72 and selects a dry natural cave camera.

## Evidence and remaining acceptance

`target/online-runtime/combat-visible-local/result.json` records two rendered
clients completing movement, strafing, attacks, authoritative damage and respawns.
The captured opponent has a raised attack arm; logs contain remote swing state.
Both clients shared a measurement barrier and remained visible side by side.
The SHA-256 in that receipt identifies the build before the cave/cache changes.

The second client's full measured window had a 935 ms maximum. After its first
100 frames the maximum was 31.85 ms, with p99 23.10 ms. The full-window result
fails smooth-play acceptance. Early GPU shadow/terrain times are high; cache
integration remains a hypothesis to measure, not a proven stall fix.

The generated-water diagnostic (`cave_water_review` example) sampled seeds
7/74/512 across a 512 m square. Version 72 produced 2099/1773/2172 water cells
in sampled carved cave space; version 73 produced 5/3/3. This is generated-cell
evidence, not proof that existing saved water was drained. Receipt:
`target/visual-review/cave-water-counts2.log`.

## Source-verified gaps worth implementing next

1. Shared container sessions: multiplayer inventory validation explicitly rejects
   `SlotAddr::Container`. Implement host-owned open/close subscriptions and
   revision-checked transactions, not client inventory replacement.
2. Shields and Curios-style accessories: the inventory has armor and offhand but
   no shield/accessory item variants. They need save encoding, equipment rules,
   host-side effects and player-facing slots together.
3. Remote equipment/casting poses: melee now has state; rendered equipment still
   comes from the character kit rather than the actor's authoritative loadout.
4. Poor-link gameplay acceptance: local two-client rendering does not establish
   WAN latency, jitter/loss behavior or 16-player server budgets.

Older prediction notes say integration is absent; current owner movement exists.
Do not reimplement it based on that stale status. Broad industrial/space feature
lists remain design intent until their runtime ownership and feedback are verified.
