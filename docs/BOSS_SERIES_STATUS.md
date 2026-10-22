# Boss series revision 2 — implementation status

## Farming brief supersedes the twelve-boss roadmap

Current continuation: the user has authorized finishing the encounters and native
farming/save/reload reviews. Save writer **27** now persists
carried brood/spore stock, nursery parentage and paid farm projectiles in flight;
reader 26 retains its original layout without granting fresh carried stock.
The Matron has its own committed planting/rake/stamp/pollen actions, paid stock
replenishment, physical recoverable child sources and nursery service behavior.
Habitat actors and emitters now stay active beyond render distance (64-source
bound); there is no expected-kill approximation. These changes supersede the
distance-suspension details below. Generator **29** adds ordinary lamps, cultivated
growth and irrigation to new habitat regions; 27/28 terrain stays unchanged.
An unoccupied source can now be carried with its receiver and buffers. Its
production, receiver and emitter pause in the pack; placing restores that same
source. The first receipt below covers the earlier increment only.

### Native continuation evidence and remaining gates

Immutable R1 executable SHA-256:
`7B0F13BD693B7C588F3D139FE9BA3236D84C59C368EC71926D29510CB4429EAD`.
Check/build passed. `reviews/habitats/r1-summary.json` records the native evidence:

- Farm: first peaceful harvest paid 2 XP with the resident still at 65 health.
  Five actual paid body kills subsequently yielded 42 XP total, not 40 or a
  duplicated product reward. Inputs reconcile: `4096 - 5*64 - 64 = 3712` biomass;
  `4096 + 8 - 180 = 3924` mana. Each kill used three normal Spark hits / 36 mana.
- A second resident was being harvested 131.4 metres from the player. Production
  checkpoint/reload preserved exact encoded habitat state, terrain, source count
  and one paid projectile in flight. That restored shot completed the kill and
  released its one body reward. Reload did not mint XP.
- Ranged Matron defeat and an interrupted carried pod becoming a viable world
  source occurred through gameplay. Both sources survived disk reload exactly.
  This is not evidence of the player picking that pod up.
- Natural generator 28 region: nursery and spider emerged from their documented
  conditions; the Matron pod reached development 192 at 48 seconds. The Matron's
  full 180-second incubation cycle has not yet been reviewed.

R1 limitations: departure script walked off its narrow bridge, so clean return
acceptance remains open. Matron inherited a masonry material. Natural habitat
lighting/readability needed improvement. Benchmark measurement restarts after
checkpoint world replacement; these mixed review runs are not isolated renderer
performance comparisons. No device-loss/surface failure was observed.

R2 (`1E2E2B85AC7312FC57EA26F840611BC5182CB60E72BF9A0BC35A874E51CD72A8`)
check/build and native process succeeded, but **close-combat acceptance FAILED**.
The Matron kept trying to reach a planting bed across water and starved its own
Rake/Stamp selection. `reviews/habitats/r2-pressure-summary.json` preserves this
failure. Authored palette/texture correction was visible. Further R2 farm/natural
cases were stopped. R3 corrects close-combat precedence and uses the existing
bounded navigation for planting, falling back when work is blocked.
R3 (`276C8DA0B193DAED75CF521C2FBC49184BC6ABBBC7F77361AA216645D4744D9B`)
passed check/build and its native pressure run: nine Stamp commitments, eight
Rake commitments and 17 actual contacts. The green body and plum crown rendered
correctly. The automated jump controller jumped too early and did not establish
the advertised counter; that gate remains open. Evidence:
`reviews/habitats/r3-pressure-summary.json`. Keep R2's failure separate.

Still required: corrected close-range attacks and counters; clean source
carry/place, departure/return, receiver logistics and reload; generator 29 visual
acceptance; a supplied three-cycle Matron farm and persistent mastery; broader
cadence/scale and interruption coverage. Automated native receipts are not a
manual fun/balance review.

The complete eight-family request remains active: Hart requires refreshed live
anatomy, recurrence and persistence acceptance; Vane Manta, Buried Miller, Vent
Choir, Unfinished Engine, Ferryman and Glassweaver still require their distinct
implementations and native reviews. Do not relabel the generic Castellan/Keeper
or other existing enemies as those completed encounters.

The current brief is preserved in `BOSS_FARMING_BRIEF.txt`. Its eight signature
families are Rime Hart, Vane Manta, Buried Miller, Vent Choir (conditional),
Unfinished Engine, Ferryman (conditional), Glassweaver and Orchard Matron.
Tollkeeper is an early boss/recurring elite; Mason/Unmaker are paired elites;
Funeral Scale is an apparatus; Cairnback is a world event/service creature.
The older twelve-entry manifest below is historical dependency context.

### First habitat increment: source implementation, acceptance pending

New generator **27** adds one connected Rootlit Hollow, cistern, silk gallery
and separate Matron nursery around (294–373, 0, 48), with a broad south stair
near (292, 0, 62). Historical generators are unchanged. Save writer **26**
adds habitat state and retains readers 1–25. Dungeon work must use its own next
generator gate (28); these coordinates and source identities belong to gate 27.

`src/world/habitats.rs` owns up to 64 distinct sources, one resident per source,
finite biomass/charging inputs, a quarter-second simulation clock, independent
product and body rewards, separate mana/XP buffers and a bounded loose-essence
ledger. New source introduction requires division from a living, productive
source, one product and four supplied biomass. Carrying moves the unique source
and all its state; it cannot run in the pack. Empty sources can be discarded
without triggering recolonization. Natural seeds are installed only once.

Sources need a supported bed, dry emergence space and water two metres beside
them; root/Matron beds need soil, spiders need an overhead anchor two metres
above the bed. Nursery/spider emergence takes 30/40 seconds and one biomass.
A Matron takes 180 seconds, four biomass and 32 mana. Natural starting stock
is finite and visible. Production costs one biomass per 20-second product:
root/spider products use existing lumen fibre, Matron products use biomass;
root/Matron growth also produces eight fictional mana. Water is currently a
local habitat condition, not a consumed or simulated irrigation flow.

Production → Habitats provides inspection, bottlenecks, input/harvest/collection,
source carrying/division, receivers, auto harvest and reserve controls. A receiver
costs one circuit and one crystal. Its nearest storage crate within 2.5 metres
supplies biomass and accepts harvested products through the normal crate filters,
barrel locks and in-flight capacity reservations; existing pipes serve the crate.
Remove an inactive, empty receiver to recover its components and move the source.
Product events pay two XP each. Body deaths pay eight XP (Matron: 80), with no
damage-owner or final-hit restriction and no deduction for earlier products.
Loose XP stays where released; full loose-packet capacity credits overflow to
the shared XP ledger rather than dropping rewards. Receiver buffers retain XP
when full. Mana accounting reports produced, used, supplied and buffered values.

An optional receiver emitter uses the ordinary **Spark** projectile path and base
spell cost/cooldown, paid from buffered mana and supplied surplus. It preserves
the next Matron's 32-mana birth input. Reserve 1 prevents body harvesting;
reserve 0 permits it. It targets the source's actual active resident, with a
six-metre limit and real terrain line of sight. Three completed Matron cycles
with receiver/emitter enabled and reserve 0 award a persistent farming mastery
credit alongside expedition clears. Spending XP never removes that credit.

The Witness Lantern treasure on the hollow's plinth collects nearby loose essence.
64 XP can imprint the existing moss melee-healing modifier. Spell knowledge is
independent and never removed by spending XP. Existing non-habitat creature
deaths now release XP through their normal once-only settlement path as well.

Residents retain position and health across departure/reload. Incubation and
peaceful production advance without render residency; combat and emitters are
**suspended when their actors are distant**. There is no fabricated offscreen
combat throughput. This is an explicit missing part of the full farming brief.
No wall-clock catch-up while the application is closed is implemented.

The Matron has a distinct articulated plant body, 240 health, committed spore
patch/fan attacks and normal terrain-aware movement, reusing the existing enemy
runtime. This is a combat blockout, not accepted signature-boss design. Spiders
currently reuse the skitterer combat/navigation body; silk route construction,
prey routing, portable physical brood pods and differentiated adult farm work
remain open. The other seven signature families and additional regions are not
implemented by this increment. The full brief is **unfinished**.

Verification: the serialized, integrated `cargo check --locked --offline
--target-dir target` passed (7.07 seconds, 101 warnings, no errors). The shared
receipt is `reviews/renderer-profiling-20261021/water-shadow-integration/cargo-check-retry.log`.
New helpers were parsed/formatted with rustfmt; whitespace checking of touched
tracked files passed. No tests, hosted CI, executable build or game run for this
increment. Native expedition, ownership/save/reload, cadence/throughput and combat
reviews remain pending. Optional Kev inference was deferred during the shared
native session; no advisor acceptance claim. Frozen renderer captures from other
chats exclude this content and must not be used as its evidence.

---

21 October 2026. The supplied brief is preserved verbatim in
[`BOSS_SERIES_REVISION_2.txt`](BOSS_SERIES_REVISION_2.txt).
This is a staged implementation. **The twelve-encounter series is unfinished.**
No roster entries, concept counts or pre-existing generic guardians count as completed bosses.

## Implemented increment: Last Tollkeeper

Central player action: win space around a gate that the opponent can plant as real cover.

* Enemy kind 14 uses a simulation-owned action clock, committed direction, one contact latch,
  swept grounded movement, finite turning before actions, and terrain line-of-sight for hits.
* Guarded movement is slower than movement without the gate. Frontal damage is reduced, never
  wholly denied. Actual incoming pressure encourages planting. The keeper can flank remembered
  positions and prioritize retrieval without magically observing a player through cover.
* Thrust: 1.00 s preparation, 0.40 s active travel, 1.25 s recovery; sidestep or use cover.
* Sweep: 1.15 s preparation, 0.45 s moving arc, 1.40 s recovery; retreat or flank.
* Shove: 0.70 s preparation, 0.20 s contact, 0.95 s recovery; close-range pressure.
* Plant: 1.25 s preparation; publishes ordinary editable timber, including a shooting gap.
  Retrieve: 1.45 s preparation; removes only surviving, successfully reclaimed timber cells.
* A heavy stagger interrupts into recovery. Ordinary taps cannot permanently cancel every action.
  At half health the keeper prioritizes retrieval sooner and sometimes signals a full sweep
  follow-up after a thrust. The follow-up retains its normal preparation.
* Spacing/recovery punishment, excavation or mining of the gate, and constructed flanks/cover
  provide execution, manipulation and preparation responses. No parry or special spell is required.
* The authored rigid figure, hooked polearm and asymmetric gate use body-anchored pixel UVs
  (render family 58); the action clock drives pose. Further animation and art polish remain.
* Generator **26** replaces watchtower sites with broad toll crossings: an upper bridge,
  landings, a lower maintenance path, stairs, a recovery floor and partial tollhouse cover.
  Their reward identity is 10; old watchtowers and small cairns retain their original rewards.
* Defeat releases remaining carried timber once. Planted timber stays ordinary world material.
  Claiming Keeper's Passage at the reliquary grants a fourth Mirror Ward charge.

Ownership is material-based: planting debits only successful writes; retrieving credits only
successful removals. A mined, burned or otherwise missing cell is never recreated. There is no
fresh stock on disengagement, streaming reactivation or reload. The initial gate has one finite
allotment, with at most 256 saved cells; no new material or machine IDs were added.

## Persistence and compatibility

Save writer **23** adds a bounded boss-state block after the found-spell log. Readers retain
versions 1–22. New saves need the updated executable. Generator versions 1–25 retain their
terrain and original watchtower encounters; no existing builds are excavated for this change.

The Tollkeeper records health, position/home, facing, carried material, placed cell identities,
and escalation state. The existing victory/claim flags remain authoritative. Dormant state
survives actor eviction; active actors override dormant snapshots during save capture. Loading
or reactivating cancels actions into a two-second delay. Applied terrain events are not refunded.
An obstructed saved body position postpones reactivation instead of deleting player construction.

One real version-23 checkpoint/reload scenario passed in the disposable fixture (below).
Older save migration, interruption at other resource transitions, and reward replay still need
runtime acceptance. That one scenario is not proof of a completed save migration review.

## Evidence and limits

`cargo check --locked --offline --target-dir target` passed before implementation.
Local release-fast builds validate the existing composed shaders through Naga. No unit tests,
test suite or hosted CI were added or run.

The first capture exposed coarse-grid planting and material problems; both were corrected.
That first run hit disk-full capture errors and is excluded from performance evidence.

`target/tollkeeper-review/gate/` records the corrected stationary first-person encounter:
eight carried cells became eight world cells, then all eight were reclaimed. Player health
remained 57 while the gate obstructed the keeper, then ordinary damage resumed after retrieval.
Its 1280×720, four-chunk fixture averaged 193.34 FPS, 7.950 ms p95, 5.123 ms GPU passes.

`target/tollkeeper-review/moving/` records movement through normal player input/physics,
committed attacks that sometimes missed, five action families, planting, pursuit and retrieval.
It averaged 152.67 FPS, 9.260 ms p95, 6.339 ms GPU passes. The scripted player eventually died.
These different camera/scene workloads are **not an overhead comparison** or manual fun review.
They do not measure or resolve the previously reported terrain-generation regression.

Reproduce with `tools/Review-Tollkeeper.ps1`. It records executable SHA-256, arguments,
first-person captures, frame samples and logs in a fresh evidence directory. Moving/Duel use
ordinary movement keys; Duel fires the real wand with normal mana and cooldowns. Baseline omits
the actor. Enraged is an explicitly low-health fixture, not evidence of a player-caused transition.

`target/tollkeeper-review/checkpoint-Duel/` used `-Mode Duel -Seconds 45 -Checkpoint`.
The production region save worker and checkpoint decoder preserved **188 health, zero carried
cells and eight placed cells**, with all eight cells present in the loaded world. Reload cleared
the active action and applied a two-second delay. Subsequent retrieval recovered those same eight
cells. This fixture explicitly reactivates its saved actor because it is in a disposable superflat
world; ordinary generated-site re-entry still needs review. The synchronous review-only reload
caused a 415 ms frame; this run is not an encounter-overhead measurement.

Still required for Tollkeeper acceptance: manual melee and ranged victories; natural generated
placement and retreat/re-entry; mining/replacement/fire during retrieval; full save/load at each
resource transition; reward replay; reduced-effects review; finer voxel scales; and art/feel polish.
The current state is a playable blockout, not a finished production encounter.

## Remaining-work manifest

| Family | Current evidence / next concrete dependency |
| --- | --- |
| Last Tollkeeper | Playable blockout above; finish the listed acceptance and polish work. |
| Rime Hart Sentinel | Existing dirty Hart/crown/sanctum work retained. Re-audit its current growth, supports, finite water, pulse severing, replanting and persistence; complete flooded-archive acceptance before counting it. |
| Unfinished Engine | Next major implementation: finite stock and recipe, physical transfer with one owner, interruption/theft into a genuinely usable powered module, installed capability change, persisted outcomes. Existing Castellan is not this encounter. |
| Vent Choir | Shared local stimulus events and bounded duct connections; inferred targets with visible discharge and manual decoys. Existing Keeper is not this encounter. |
| Buried Miller | Material-valid short routes, bounded excavation, accounted spoil, exposed confrontation and route-edit invalidation. Existing beetles are not this encounter. |
| Vane Manta | Validated perch/corridor graph, committed low passes, reachable landings and bounded draft effects. |
| Mason and Unmaker | Two distinct bodies and agents, one-major-action coordinator, finite construction, demolition friendly interaction and survivor behavior. |
| Orchard Matron | Singular carried/world pod ownership, saved development, containment, finite brood and useful kept offspring behavior. |
| Funeral Scale | Ground-accessible supported loads, a small balance/stance model, authored poses and visible changes during attacks. |
| Ferryman Without a River | Validate existing fluids first; finite connected basin reach, manual sluices/buckets, active body and conserved transfers. |
| Glassweaver | Actual spell payload routing with ownership/costs, four to six readable nodes, tool manipulation and finite repairs. |
| Cairnback Procession | Heavy grounded movement and route clearance through edits, useful stops/terraces and a supported interception objective. Existing Colossus is not this encounter. |

After each base encounter works: its smaller teaching encounter, useful world outcome/reward,
normal first-person play review, measured cost and persistent ownership checks. Add alternate
sites only after that base loop succeeds. Keep research candidates and avoided simulation work
outside this series as directed by the brief.
