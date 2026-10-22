# Prism Hart: model, encounter and sanctum

21 October 2026. Existing enemy kind 8 and persisted encounter/victory flags are retained.
The new rig and fight state are runtime data; no material IDs or save fields were repurposed.

## Art and animation

`mesh/models/hart.rs` authors an eight-sided mineral torso with a defined shoulder,
brisket, rising neck and tapered pointed skull. Four articulated legs have bent
hocks, separate dark hooves and cloven seams. Branching antlers grow from paired
skull roots; ears and eyes sit beneath them. Torso, neck and head have separate
flat-shaded cross sections. The sternum crystal is the focal point.

Gait follows distance travelled. Swing lifts alternate diagonal feet; stance
returns them to the floor. Attack state drives the lowered charge head, lifted
forefeet, sweep preparation, antler swing and recovery. Facing turns smoothly
through the windup; the actual attack direction commits before the dodge window.

Rendering family 56 is reserved for the authored crystal creature (not a saved
BlockId). It keeps vertex hue and geometric facets rather than the machinery
family's patina, scratches, cave grime or moving world-space relief. Low inner
scatter supplements normal lighting. Eyes and sternum remain brighter. Other
materials keep their existing paths.

## Three palettes and three difficulty profiles

| Family | Opening language | Sovereign followup |
| --- | --- | --- |
| Rime, blue/silver | Charge, shard fans, marked blooms | Charge into shard fan |
| Amethyst, violet | Targeted bloom, delayed surrounding ring, volleys | Bloom into shard fan |
| Verdant, green | Staggered parallel lanes, blooms and charges | Lanes into charge |

Sentinel encounters teach isolated patterns. Ascendants add the broader move
set and charge-to-sweep chains below half health. Sovereigns use family-specific
two-part sequences from the opening phase. Site coordinates deterministically
select palette and tier; review arguments can select all nine combinations.
Health is 320/448/576 for newly awakened tiers. Existing runtime menace 3 is
still interpreted by the existing health multiplier and uses Sovereign patterns.

All profiles transition at half of their **actual scaled maximum health**.
The transition gives a 1.7-second opening. The boss bar uses that same maximum
and is compact at the left so it cannot cover the head and antlers.

## Counterplay and authoritative damage

* Charge: 1.15-second committed tell, marked corridor, swept 10 cm movement
  increments, one damage event per charge, and collision against real terrain.
  Sidestep the corridor; cover stops it. Damage is 23/27/31 before armour.
* Antler sweep: 0.95-second tell and a matching forward arc. Retreat, move
  behind it or jump the low sweep. Damage is 19/23/27 before armour.
* Fan: 5 or 7 physical hostile projectiles, 8–10 m/s, normal collision and
  normal ward/armour handling. Move through gaps or use actual cover.
* Bloom: a delayed strike at the committed position, then a surrounding ring
  with a safe centre. Leave the first marker, then step back into the centre.
* Lanes: two staggered rows of strikes with permanent channels between them.
  Followup charges prevent Sovereign encounters from rewarding standing still.

Fan and ground damage start at 16/20/24; enrage adds 3. Ground strikes share
the existing delayed hazard/damage path. Visibility and height checks prevent
hits through cover and through other floors. Hazard/projectile admission is
checked before starting a tell; existing bounded limits remain 16/96.

Recovery lasts 1.65 seconds after ranged/sweep attacks and 1.85 after an isolated
charge. Followup pauses are 0.55–0.65 seconds plus the full next windup. Early
recovery takes 25% extra melee/projectile damage and brightens the sternum gem.
Frost shatters can interrupt, subject to existing boss poise resistance.
Torso, neck and skull use oriented hit regions rather than one tiny central sphere.

## Dungeon boundary

Generator 25 replaces crystal boss caverns with the same pure sanctum sampler
used in review: a broad open circuit, vaulted nave, peripheral buttresses that
provide cover, sparse geodes, warm torch pedestals and a low rear shrine.
The central fighting space is clear of the old large crystal installation.

Saved generator versions 1–24 retain their terrain. Existing enemies receive
the new rig/fight, but existing terrain is not automatically excavated. The new
sanctum appears in newly generated version-25 worlds. Ordinary world edits still
override generation. No player's builds are cleared by awakening a boss.

## Repeatable native review

Build locally with `cargo build --profile release-fast --locked --offline --target-dir target`.
The standard release output was already held by another running game; reviews use
the separate existing iteration profile and frozen executables under `target/prism-hart`.
No test suite or hosted CI is added or run for this task.

`tools/Review-PrismHart.ps1` records arguments, executable hash, actual captures,
frame samples and logs. It refuses to overwrite evidence. Modes:

* Showcase holds a three-quarter idle pose for material/proportion inspection.
* Combat leaves the player stationary; normal attacks, damage and respawn run.
* Orbit moves the review player around the actual arena while the fight runs.
* Duel adds auto-aim and the real equipped wand, preserving its mana/cooldowns.
* `-Enraged` explicitly starts a low-health fixture; it is not proof of a player victory.

The first stationary combat review recorded health 100 -> 77 -> 58 -> 39 -> 16
before normal respawn. A Sovereign Amethyst moving-camera review exercised
Bloom/Volley/Lanes/Charge sequences and recorded health 100 -> 73 -> 46.
These are actual game outcomes, not injected damage or a synthetic render.
Further human play is needed for final difficulty/feel acceptance.

## Reviewed delivery

The frozen reviewed executable is `target/prism-hart/final.exe`, SHA-256
`2B792A1E60F85368680253E4AC730268F4F12415FEB355C43D710CDA2517FB35`.
The final local `release-fast` build and composed-shader validation succeeded.
The existing running standard-release game was left alone; restart with the new
executable to acquire this implementation.

`target/prism-hart/prism-hart-combat.mp4` is a 20-second native game capture:
293 actual PNG frames requested at 15 Hz, with capture gaps retained and no
interpolated motion. The underlying full-health Duel run uses the real wand,
mana and cooldowns. Boss health fell from 320 to 52.5 and naturally crossed its
half-health transition. The circling player avoided damage in this run; the
stationary and Sovereign captures above provide the player-damage evidence.
Manifests, raw frames and logs remain in the corresponding review folders.

`review-artifacts/prism-hart/reviewed-source-2026-10-21-final.zip` preserves the
boss-related sources used for this executable plus the final review script and this
note. Its adjacent manifest lists per-file hashes. Shared files contain the
pre-existing work present at build time: inspect diffs before restoration.
Concurrent terrain/lighting edits made after that build remain in the live
checkout and are not overwritten by the snapshot.
