# Resident gameplay audit

Source review: 2026-10-29. No gameplay sessions or tests run in this pass.

Read against `CLAUDE.md`, `ARCHITECTURE.md`, `TRACKING.md` and
`MANYFOLD_HORROR_AND_MULTIPLAYER.md`. Source takes precedence over older status claims.

## Fixed in this pass

- Gameplay images now include checksummed farming and travel records. Clients
  previously received visible crop blocks and resident actors without their plot
  clock, growth stages, stock, absences, boats or agreed homes. Both components
  decode before image publication. Missing fields retain compatibility with older
  captured images. Herd state already travels with Adventure.
- Gardening services require a loaded plot whose crop block still exists before
  selecting it for paid growth. Stale plot records no longer consume biomass.
- Resident service reach checks reject nonfinite interaction positions.
- Farming file coordinate validation uses unsigned magnitude, rejecting i32::MIN
  without signed absolute-value overflow in both plot and wild-harvest records.

## Remaining holes

| Scope | Source evidence | Required next work |
| --- | --- | --- |
| Multiplayer resident interactions | `main.rs` gates the trade page on `!network_mode`; `multiplayer/actions.rs` has no trade/service request | Add ordered host requests with host-derived reach and personal inventory swapping before enabling the page. |
| Multiplayer companions and riding | `travel::State` has one `following`, `riding` and fishing cast; `gameplay::Personal::swap` does not own these | Assign travel actions to player identities, resolve occupied boats across all players, persist/disconnect safely. Snapshotting shared records does not solve ownership. |
| Replication cost | Full gameplay images include bounded sidecars, including wild-harvest history | Measure populated-world snapshot size and host cost with two clients; introduce revision-based components if needed. |
| Single-player tree felling | Rigid trunk motion and support checks are implemented | Visually review thick trunks, nearby crowns, slopes, save/reload during a fall and collision outcomes. Compilation does not establish quality. |
| Persistence | Farming and travel use separate sidecars | Review interrupted multi-file save recovery; individual file replacement does not prove atomic world checkpoints. |

`TRACKING.md`'s multiplayer section is historical: multiplayer is now a default
Cargo feature and lobby/UI code exists. Its old CLI-only and feature-default
claims must not be used as a current acceptance checklist.

## Habitat automation follow-up

- Existing Spark emitters now use brood reserve as a defense/harvest selector.
  Defense targets visible wild hostiles within 6 m; harvest targets only its own
  resident. Selection prefers nearest clear targets with stable identity ties.
- Automated projectiles exclude friendly wildlife, other habitat residents and
  multiplayer players. Spark chaining is disabled for these shots so secondary
  arcs cannot bypass those exclusions. Existing Spark mana and cooldown apply.
- Receivers draw charge from enabled Mana Font batteries within 6 m, up to four
  mana per quarter-second service, at four stored energy units per mana (1 kJ).
  Charge buffers stop at 64; Matron emitters retain 32 for the next birth.
- Receiver ports visit nearby storage in distance/coordinate order, allowing
  separate biomass and filtered output crates. Automatic feed holds two birth
  costs rather than draining a crate into the full 65,536-unit source buffer.
- Full receivers no longer block loose mana/XP from reaching another receiver.

This uses existing saved flags and buffers. It does not mount an inventory wand
or execute a programmed wand board; arbitrary wand loadouts still need explicit
item ownership, persistence, spell restrictions and multiplayer integration.
Native review remains open for target switching, obstruction, resource rates,
save/reload and two-player behavior. No tests were added or run.

## Resident voices and imperial records

`assets/fernwake_residents.ron` now owns the four residents' voices and eight
attributed local records. Comedy comes from institutions surviving their purpose:
river fees without ports, roots disregarding district boundaries, imperial credit
for ordinary gardening, and lost property transferred downstairs. The lower-world
accounts remain testimony, not promises of implemented rooms or encounters.

Dialogue reacts to actual workshop faults, today's gardening service, following,
and saved homes. Mara's additional cistern observation requires loaded cells and
distinguishes blocked, open-but-dry and water-arrived states. It does not set a
quest flag, grant items, or treat an open sluice as proof of water delivery.
Older generators retain their existing route clues. Local records expand in the
resident page; controls and refusal messages retain literal functional wording.

Future physical encounters should pay off these accounts with usable evidence,
alternate routes and finite consequences. Do not turn every room into a joke:
inhabited districts can be funny because their residents cope; the deeper spaces
need unanswered questions and periods without commentary.
