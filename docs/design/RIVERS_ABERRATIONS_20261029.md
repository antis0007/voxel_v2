# River and deep encounter review

The native review of `voxel-river-consistency.exe` showed repeated visible-mesh
eviction at the automatic 512 MiB terrain ceiling on a 24 GiB adapter. Automatic
residency now allows one quarter of the OS GPU budget, up to 8 GiB, still bounded
by live memory pressure and the existing non-terrain reserve. This is a ceiling,
not a preallocation. Explicit manual limits retain their meaning.

Natural inflow now follows the active generator. Geological worlds take only
the first wet nodes of the solved receiver network, excluding lake nodes and
nodes with an already-wet parent. Older generators use the final river/lake head
sampler. A source discharges at a bounded runoff rate, stops at its original
head, and accrues no catch-up debt while unavailable. It does not refill each
river cell. Inflow enters the existing persisted external-water ledger; exact
cellular gravity and drainage own subsequent movement. No save migration or
terrain regeneration is performed.

The native hydraulic review found that full-voxel initial water had erased the
37.570-to-37.534 m fall along a 76 m headwater reach. Generator 79 retains exact
fractional starting stocks instead. Meshing, pre-admission simulation reads and
resident stock installation use the same sampler. Initial stock admission is
bounded by the existing precision capacity; edits and saved stocks override
procedural defaults. The resting tolerance is 10 micrometres for these worlds,
below that reach's natural gradient. Earlier save generators retain their
existing inventories; this change does not silently drain a saved river.

The next native pass isolated a separate blocker: the generic smeltery predicate
accepted the reversible water/ice pair as a molten metal. `thermal::active`
therefore claimed every water cell and returned before the fluid solver. Water
is now excluded from the metal predicate, restoring fluid stepping for old and
new worlds alike. Water still participates in ordinary heat/phase exchange.

`VOXEL_RIVER_REVIEW=1` selects a generated headwater without editing terrain.
With benchmark physics enabled it logs source inflow, cumulative exact-volume
transfers and downward transfers. These scene-wide counters count repeated
cell-to-cell movement; they are not net river discharge. The local probe now
records signed throughput across a 16 m wide section 6 m below the selected
geological source, oriented toward its receiver. `section_lps` uses elapsed
simulation steps, and `local_transferred_ul` bounds gross movement to the
headwater neighborhood. `wave_cells` reports the transient wave proxy, which
can be zero during gentle physical drainage. Shader ripple animation alone is
not evidence of physical flow.

The completed generator-79 native review in
`target/natural-river-review/verified-20261029` exited successfully. Its frozen
`voxel-rivers-aberrations-ready.exe` hash is
`B826A0E247CAD8F312B870D47D9E0A30C73AE24C6278B2B4033825479BDA6DD4`.
At headwater (-160.020, 37.570, -97.736) m it recorded 614,097,808 microlitres
of external inflow, 2,372,477,964,535 microlitres of gross scene transfers and
1,084,053,189,474 microlitres of downward transfers before its final sample.
The capture shows a wet channel. This establishes live fluid stepping and
source admission; the newer local section probe measures downstream throughput.

The final refinement executable is `target/voxel-river-creature-refinement.exe`,
SHA256 `DE35671B484F06A2EA8505AD480A19E60B4CA410D31D50D7121286C2C5FDDB81`.
The current-generator review compiled as generator 85 and exited 0 in
`target/natural-river-review/refinement-current-20261029`. The unchanged seed
selected the same headwater. Its signed section reached 173,855,198 microlitres
(173.855 L) net downstream; late full-interval samples were about 6–8 L/s. The
last external-water sample was 291,022,292 microlitres, with another 4,792,847
admitted afterward. Local water remained present in the native captures.
These are transient headwater observations, not a steady-state discharge model
or proof for every catchment. Concurrent builds produced scheduling stalls,
so these recordings do not establish frame-rate or mining-latency acceptance.

## Underground encounters

Kinds 26–29 are Spectator, Beholder, Roper and Choker. Passive actors use the
separate 40+ range. Bestiary rows, original articulated voxel bodies, drops and
deterministic site-slot selection use the existing encounter record authority.

Selection measures actual metres below local terrain: Choker from 30 m,
Spectator from 45 m, Roper from 80 m, and rare lead Beholder from 150 m. Existing
bosses remain authored. These creatures do not join surface night spawns.

- Eye rays commit to the announced aim; sidestepping and solid cover defeat them.
- Beholder central-eye suppression occupies a visible forward cone. The eye
  closes during stalk casts, creating a spell counterattack window.
- Roper stays rooted and pulls a latched target toward its bite. Cover, terrain
  collision, range and stagger sever the tether.
- Choker approaches physically and uses a shorter grasp.

Attack origins now share the body's breathing, yaw, lean and impact transform.
Only the selected casting stalk takes the ray color and brightens during its
windup; other stalks keep their quiet iris material.
The selected stalk emits eye rays, the central eye emits suppression, and a
visible tendril or hand anchors each grasp. Reach holds through contact and an
active latch. Eye status effects no longer draw a tracking tether after the
committed ray has ended. Hit and cover queries use those same emitter points.

The disposable enemy review accepts `VOXEL_ENEMY_REVIEW_SCENE=aberrations` and
the existing single-kind selector. Native captures and runtime logs under
`target/` distinguish model presentation from actual combat and river movement.
`tools/Review-Aberrations.ps1` enables ordinary survival damage in disposable
arenas; its `hold` scene leaves movement input idle to expose physical pulling
and bites. `camp` exercises ordinary movement resistance, while `aberrations`
sidesteps along the existing patrol. It does not grant invulnerability or alter
damage/windows. Each run freezes the executable hash in its manifest.

Native survival results for the refinement hash above (all runs exited 0):

| Review under `target/aberration-review/` | Observed result |
| --- | --- |
| `refinement-20261029-hold-26` | Three Spectator hits; minimum player HP 75.3 |
| `refinement-20261029-hold-27` | Three Beholder hits; minimum player HP 64.1 |
| `refinement-20261029-hold-29` | Choker grasp/contact damage; minimum player HP 81.0 |
| `refinement-long-20261029-hold-28` | Rooted Roper pulled player from z=7.00 to z=1.83 m; first bite at t=9.26 s took HP to 76.0; repeated unattended bites eventually killed the player |
| `refinement-defense-20261029-aberrations-26` | Four committed casts missed the sidestepping player; HP stayed 100 |
| `refinement-defense-20261029-obstacle-28` | Solid cover prevented Roper casts; HP stayed 100 |

Captures cover the animated bodies and gameplay contact, including the closing
central eye and active stalk material. These arenas do not establish natural
room-distribution frequency, every stagger/cover-release case, or a complete
underground exploration session.

## Reference constraints

This increment follows [art and state](../NEW/03_ART_CONTENT_AND_WORLDGEN.md),
[reference contracts](../NEW/06_REFERENCE_CONTRACTS_AND_SCENARIOS.md),
[body-attached materials](DOORS_AND_MOB_ART_20261026.md), and
[the lore slots](WORLD_LORE.md#the-lore-slot-fill-all-five-for-anything-new).
It retains the authored palette and pixel lattice. Creature associations below
are design interpretations for future Records, not new cosmological Facts or
claims that prose Records have already been installed in the world.

| Creature | Era association | Suspected purpose | Existing physical evidence | Link | Unanswered gap |
| --- | --- | --- | --- | --- | --- |
| Choker | Ballast, stratum I | Scavenge tight passages | Long grasping arms, close pursuit | Fungal descent | What kept it out of inhabited passages? |
| Spectator | Anchor, stratum II | Watch a threshold | Four stalks, committed rays | Ruin doorways | Is it watching for entry or escape? |
| Roper | Anchor, stratum II | Seize intruders near works | Stone silhouette, rooted tendrils | Sluices and old stonework | Was its stillness trained or inherited? |
| Beholder | Below Anchor works, near stratum III | Defend an exclusion zone | Central suppression eye, ten stalks | Deep geometric chambers | Why does it fear an unoccupied room? |

These are not sightings or explanations of the Surveyors. Runtime selection
currently uses depth and persistent encounter slots; contextual room affinity
and authored Records remain separate future work.

Creature references: [Spectator](https://www.dndbeyond.com/monsters/17094-spectator)
and [Roper](https://www.dndbeyond.com/monsters/16999-roper). These are game-specific
combat adaptations and original voxel rigs, not tabletop stat-block copies.
