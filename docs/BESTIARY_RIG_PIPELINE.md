# Bestiary families and articulated animation pipeline

This is the first production slice of the creature overhaul. It establishes the
shared definition/rig layer and proves eight families with distinct bodies,
instead of adding unreviewed model code for all twelve.

## Shared family definition

`src/world/adventure/bestiary.rs` now owns, per family:

- display name,
- role (`Skirmisher`, `Defender`, `Flyer`, `Rooted`),
- health,
- physical body height/width used by `free()` for movement and spawning,
- center height used by aiming, lighting and effects,
- tall/flying/brawler flags,
- movement speed,
- attack range, wind-up and cooldown.

`enemy_name`, `enemy_health`, `tall`, `flying`, `brawler`, `Enemy::center`,
`Enemy::windup`, `free()`, attack range, cooldown and movement speed now query
that definition. Kinds 2, 3 and 9 keep their existing fallback values and
presentation until they receive their own authored pass. Bosses 6–8 remain
separate.

## Proven families

| Kind | Family | Body and motion | Current distinction |
|---|---|---|---|
| 0 | Ruin Skitterer | Low wedge body, eight two-joint legs, feelers, mandibles | Fast skirmisher; legs splay/crouch with distance-driven walk phase; mandibles open on wind-up |
| 1 | Spore Sentinel | Root flare, segmented stalk, broad cap, gill fringe, three sacs | Rooted until alerted; sacs swell during the tell; spores hover |
| 4 | Aether Moth | Thorax, segmented abdomen, four hinged wings, antennae | Flying; wing beat folds back during attack wind-up |
| 5 | Sanctum Warden | Armoured legs, torso, pauldrons, helmet/plume, weapon arm, shield arm | Defender; shield braces with wind-up and can break |
| 10 | Threshold Mimic | Chest-like body, hinged lid/jaw, teeth, tongue, four feet | Cache-room ambusher; lid opens as alert rises |
| 11 | Cistern Lamprey | Seven-segment serpentine body, fins, lateral glow spots, circular mouth | Low, sinuous skirmisher |
| 12 | Kiln Crawler | Boiler body, chimney, vent, six legs, piston arms | Vent flares during its tell; heavy heat silhouette |
| 13 | Vault Porter | Hunched frame, long arms, carried cargo crate | Tall carrier; crate lifts with the tell |

All body animation is driven by `Enemy.phase`, which advances only by distance
walked. There is no wall-clock foot sliding and no cosmetic timing that can
change combat timing.

## Warden part damage

`Enemy.guard` is a lightweight per-fight state, not a save field, because enemy
instances are reconstructed from deterministic site records.

- The Warden blocks frontal Spark and melee only while `guard` is true.
- At 50% health the shield breaks once (`check_guard_break`), emits a shard
  flash and clang, disappears from the rig, and leaves a broken strap.
- The exposed Warden can no longer block, so the same encounter changes its
  approach after a meaningful body part is lost.
- No save migration is required.

## Encounter roles

Escorts are now chosen for contrast by role rather than fixed flavour pairs:
flyer beside brawler, defender beside rooted body, skitterer beside ranged
pressure. Deep rooms and some escorts deterministically select the mimic,
lamprey, kiln crawler or porter from the same site record, so a cleared site can
never resurrect a different family on load.

## Remaining target roster

The long-term target remains the twelve-family bestiary in the design brief:
Mirror Grazer, Ruin Delver and Astral Siphon still need data, rigs, habitats and
one shared interaction each. They are not represented by placeholder kinds.

Blockbench/glTF asset import is not wired yet; the runtime rig is intentionally
procedural and local so it can later be replaced or blended with authored
meshes without changing `Enemy` state.

## Validation

- Naga WGSL validation unchanged and passing.
- Full Rust suite: **166 passed, 0 failed, 13 ignored** with
  `--test-threads=1`; the bestiary tests cover shared definitions for all eight
  families and the shield-break transition.
- Deep families are selected deterministically from site id/seed; no save
  version change is required because enemy instances are reconstructed from
  cleared-site records.
- Runtime visual acceptance still requires launching the rebuilt game and
  inspecting each family in a real room under carried and room lighting.
