# Wand and scale repair

The game now favors crafting, exploration and spell experimentation. The workshop-heart expedition is collapsed into an optional journal/Gear entry; claiming relics no longer automatically posts route instructions. The foundry valve puzzle is optional preparation: completing it restores health and mana, but no longer gates the guardian. Existing puzzle and relic progress remains intact.

## Playing with a wand

Craft the first lumen wand, open **E → Gear**, choose a slot, then choose its spell. The first wand supports two slots and all six basic spells. Upgrades add a third/fourth slot and reduce the recharge between sequences. Rift well and Echo step remain discoveries from guardians.

- **Direct:** cast filled slots in order, skipping empty slots. Duplicate spells are allowed.
- **Twin:** projectile slots fire two slightly spread shots for 1.6 times the normal mana cost and a longer cast delay. Healing, ward and movement still cast once.
- **Impact:** a projectile carries the next filled projectile slot in the same sequence. Both spells cost mana up front; the second releases at impact, without recursively creating another trigger. A lone final projectile or a utility spell casts normally.

Hold **R** to cast; **T** moves to the next filled slot. The compact HUD shows the actual deck and next slot. The deck, cursor and mode persist in save format 16; earlier saves start with their previously selected spell in the first slot. That format also contains the concurrent inventory/drop changes.

Useful combinations: **Frost → Spark** in Impact mode chills and then shatters an enemy; **Spark → Burst** carries an explosive payload. Burst now queues a two-metre terrain crater and damages the player within its three-metre combat blast. Craters use the same bounded editing path as tools, allowing existing water/sediment systems to respond. Material edits are applied over frames rather than forcing a large synchronous world rebuild.

## Scale and targeting fixes

- Physical radii retain their metre size across 1 m, 0.5 m and 0.25 m voxels. The previous 15-voxel clamp and scale-dependent radius limit are gone. An 8 m radius at quarter-metre scale uses 32 voxels.
- Builder/destructor area limits convert square metres to faces without the old 2,048-face cap. Layer depth remains one metre in metre mode. Exposed matching-material faces define the connected selection.
- Edit strokes no longer silently discard cells after 32,768 entries. Generated tools remain bounded by the supported settings. Blasts apply from the centre outward, with a per-frame time budget.
- Menus pause committed strokes instead of deleting their remaining cells. Loading/creating/leaving worlds explicitly cancels old work. A busy blast cannot replay feedback for a discarded second click.
- An explosive/destructor/brush click is no longer consumed by nearby melee targeting; melee takes priority only with the Hand tool.
- Previews show boxes, sphere/cylinder guides and merged wand boundary contours, rather than sampled internal voxel wireframes. Large volume previews compute a boundary and count without querying every voxel. Brush/area placement previews use the same side of the hit face as execution.
- Spell collision and visibility traverse every crossed voxel and intersect conduit bounds in metre coordinates. Projectiles retain the impact on the incident side of walls. This replaces fixed-distance point sampling that could miss thin geometry.

This is an ordered four-slot wand system with three casting patterns, not a reproduction of Noita's full spell-card/modifier simulation. Its useful combinations are playable through the current casting and damage systems, without another mandatory quest chain or crafting tier for each basic spell.

## Verification

The initial implementation was compile-only. The follow-up on 2026-10-18 runs the native wgpu game with physics enabled and inspects GPU captures at 1 m, 0.5 m and 0.25 m.

Runtime findings and repairs:

- The active outline shader omitted voxel-to-metre conversion. Half/quarter-metre selections disappeared despite valid CPU selections. The shader now uses the same world scale as terrain.
- A brief R/T press on a render frame without a simulation step was discarded. Cast/slot input now survives until the next step and is cleared when interaction is blocked.
- Tool explosions queued sound after the audio drain, then the next simulation tick erased it. Audio drains after all producers. Native follow-up reports the Windows audio device ready; this is not a subjective listening assessment.
- Blast feedback now has a pressure ring, flash lighting, material fragments and dust in metre coordinates. The committed preview stays fixed during execution and for a short recovery interval. Detonation requires a fresh click.
- Ordinary blast harvesting no longer rebuilds every legacy inventory/resource counter per tiny voxel. It synchronizes once per frame, with a bounded 4 ms blast edit budget (ordinary edits retain 2 ms). At quarter scale the 8 m fixture blast dropped from about 1,881 ms to 140 ms to drain the edit queue; 4 m dropped from about 350 ms to 20 ms. These are observed edit-completion times, not frame-rate or mesh-presentation guarantees.
- A full pack no longer prevents ordinary terrain destruction. Overflow remains as recoverable world stacks, respecting saved-stack size/count limits. Cache and machine inventory safeguards remain in place.
- Hand/brush/area idle previews match left-click removal; Alt previews right-click placement. Tool HUD corners are square and explosive size is displayed as a radius.

`target/wand-native/final-{1,half,quarter}.log` records 20 native scenarios per scale, all passing: hand placement/removal, builder/destructor, cube/sphere brushes, square/circle area edits, 4/8 m blasts, all eight spells, Twin, and Impact. Captures in the corresponding directories include before/action/result frames. The follow-up `polish-*` runs repeat changed brush/blast behavior and exercise single-cell tools, point-blank Burst/Impact and full-pack explosions.

Focused regression checks: `cargo test --offline --target-dir target wand_ -- --nocapture`, recorded in `target/wand-native/focused-tests.log`. Eight tests pass, including three-scale thin-wall collision, point-blank detonation/self-damage, zero-step input retention, deck patterns, idle preview agreement, maximum-radius completeness, and recoverable overflow/fractional ore accounting.

The review is opt-in via `VOXEL_WAND_REVIEW=auto` (full scene) or `polish` (changed/edge cases), together with `--benchmark-world=superflat --benchmark-scale=quarter --benchmark-physics --benchmark-capture-hud --benchmark-duration=160`. Use `1`, `half`, or `quarter` for scale and `VOXEL_CAPTURE_DIR` for the output directory. It replays the normal gameplay input/edit/spell paths inside an unsaved benchmark world; it is not a human play session or a broad survival-balance acceptance test.
