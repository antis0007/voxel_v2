# Doors, trapdoors and mob materials

Player construction now includes oak doors, oak trapdoors and local levers.
Doors occupy a 1 × 2 metre opening; hatches occupy a 1 metre square. Physical
panels are 3/16 metre thick. The four openings in the upper door and hatch are
real geometry, shared by meshing, picking, collision and combat traces.

- Right mouse or F opens, closes or switches. Sneak + right mouse places
  against a fitting. Facing selects the hinge axis; hatches can mount high or low.
- A lever switches doors and hatches within a three metre sphere. An obstructed
  panel refuses to close through the player.
- Six planks craft three doors. Six planks and one copper ingot craft two
  trapdoors. One plank and one cobblestone craft a lever. The copper ingredient
  distinguishes the hatch in the current shapeless crafting system.
- Fitting states use appended block IDs and the existing chunk persistence;
  no generator migration or save-format change. Every assembly dismantles into
  one canonical item. Partial ordinary voxel edits cannot split a fitting.
- Placement admits the entire metre assembly, including streamed air chunks,
  before consuming an item. Existing terrain and unrelated dirty work remain.

## Mob art

`ART_DIRECTION.md`, `PIXEL_ART_DIRECTION.md` and the game bible guide the work:
quiet broad planes, limited ramps, connected clusters and stable metric detail.
Wool, hide, horn and cloth use a five centimetre pixel lattice. UVs are baked
before a part rotates. Metal clusters replace independent speckle. Forty-two
common bestiary parts use attached materials and rectangular limbs; this is not
a replacement of every boss model.

Sheep and goats now have two-link legs with constant segment lengths, attached
hooves, a planted stance and lifted swing, and a head assembly whose ears, eyes,
muzzle and horns move together. Idle breathing no longer scales hostile armour.

## Evidence and limits

The local release-fast candidate is `target/fittings-mobs-20261026.exe`.
Its SHA-256 is
`DADDA124065F3599C924DE57D1EDAB0EFC58D0081D13AB0C09E6A26D8896B321`.
Final shader validation/runtime receipt: `7ffe75cfca10824d`, matching.

- Fittings: `target/furnishings-20261026/fittings-quarter-final/camera.png`.
- Livestock: `target/mob-art-20261026/wildlife-final/camera.png`.
- Earlier common-hostile material scene: `target/mob-art-20261026/hostile-r4/camera.png`.

`tools/Review-Furnishings.ps1 -View fittings` and `tools/Review-MobArt.ps1`
produce disposable native scenes with executable hashes and source receipts.
The quarter-metre fitting capture exercised production placement and open-state
changes. The art scenes remain rendering evidence, not manual interaction,
save/reload or long-run movement acceptance. No tests or hosted CI were run.

This slice adds player-built fittings. Generated houses have not been refitted;
pressure plates, connected signal wiring, paired hinges and interaction sounds
remain separate work. Doors switch between Minecraft-style discrete poses.
