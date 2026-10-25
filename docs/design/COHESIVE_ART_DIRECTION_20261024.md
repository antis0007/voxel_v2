# Woodland, workshop and instrument UI

This increment implements the visual priority in the mission specification. It
does not complete the master plan or establish parity with the reference games.

## Art direction

The world uses broad, readable voxel masses: olive foliage, timber structure,
graphite roofs, pale stone and localized warm light. Shape carries identity before
surface detail. Tree crowns overlap unevenly and connect to a woody scaffold;
workshop entrances have visible support and a roof that marks the doorway.
All architectural and woodland geometry remains ordinary editable world blocks.

The interface uses the existing canonical palette in `assets/pixel_palette.txt`:
charcoal recesses, paper text, brass selection and teal interaction or mana.
Pixel art supplies icons and restrained accents. Functional text and numbers use
readable bundled fonts. Decorative frames must not consume control space.
The camera, world exposure and native UI scale remain separate controls.

Spell cards state behavior in the shared description catalog, with numeric values
derived from casting rules. Equipped-wand readouts describe its evaluated next
cast. Detailed mechanics belong behind the existing Alt expansion gesture.

## Compatibility and ownership

- New woodland and workshop geometry uses generator 47. Existing generator
  versions retain their old sampling branches; no save migration is performed.
- UI changes preserve existing controls and gameplay. Extracted spell constants
  keep their prior values.
- Point-light attenuation is adjusted locally in `eval_torch_light`; ordinary
  torch illumination uses a less saturated warm colour. Surface colours,
  exposure and the global tone mapper are not repainted to compensate for light.
- Earlier dirty source and the recovered Claude candidates remain preserved.
  Pre-edit files live in `downloads/art-direction-20261024/` by owner.
- No automated tests or hosted CI were added or run, per session instruction.

## Review evidence

Baseline executable: `target/art-direction-20261024/before.exe`, SHA256
`59B89052C00F33B65DDA572C80019A6EF2B314E10CF0D9224F1145FAF7C3B6F7`.

Native baseline images:

- `target/reviews/forest-repair-20261021/art-20261024-before-forest/camera.png`
- `target/reviews/forest-repair-20261021/art-20261024-before-wands/bench-001.png`
- `target/dungeon-review/art-20261024-before-hall/camera.png`

Each run has executable hash, arguments, isolated generated save directory,
process receipt and renderer logs. These are visual review captures. Compilation
from another active session overlapped some captures; capture readbacks and the
loaded graphics settings also prevent using these as a performance comparison.
The measured cloud divisor was 2 at 720p despite the requested divisor 3.

## Integrated revision

The first native review caught a vertically expanding utility row and a porch
roof that consumed its own gable at one-metre resolution. Both were corrected
before the second build. Version-47 umbrella trees also gain staggered, deeper
crown masses while retaining their species forms.

All 94 authored spell records now use the catalog (93 spells and the empty
slot). Together with eight inventory entries, the content lint accepts 102
entries and two shared keyword definitions. Gameplay numbers were extracted
without balance changes and independently compared with pre-edit snapshots.
The live next-cast summary distinguishes ricochet from piercing and preserves
fractional mana costs. Production shows keyed generation/use values, balance,
hover samples and a link from the bottleneck summary to machine health.

The second release-fast build succeeds with 31 warnings in 17 files. Frozen
executable: `target/art-direction-20261024/after-r2.exe`, SHA256
`2FC8987F69EFB692FCAB3D90D4FA3DFC9BE79089F25111F1F277D8F7B7822625`.
Build and catalog-lint logs sit beside it. No automated tests were run.

The final UI revision uses a destination dropdown below 680 logical points;
wide panels keep named tabs. This preserves readable type at the existing UI
zoom while recovering vertical space for the spell library. HUD cast costs now
share the card's fractional-number formatter. The third release-fast build
also succeeds with 31 warnings. Third frozen executable:
`target/art-direction-20261024/after-r3.exe`, SHA256
`B93A3DD383B5C8432F790C58441430D993F521FBE627CE80888A7737F900823E`.

Final compass correction builds successfully as
`target/art-direction-20261024/after-r4.exe`, SHA256
`924959221735BA7908A7CFB9D2323F16A9CF1F0A9127570B3881A2D916F02057`.
The final source snapshot and executable checksums are recorded in
`downloads/art-direction-20261024/integration-manifest.json` and
`INTEGRATION-SHA256SUMS.txt`. All 26 recorded source paths in this task manifest
predate the frozen executable. Their content includes preserved earlier work;
the manifest is provenance, not a clean patch against repository HEAD.

Inspected native outputs:

- `target/dungeon-review/art-20261024-final-hall/camera.png` (R2): stronger
  source-local pools and less saturated torch illumination; the hall remains
  predominantly warm, with readable dark beam silhouettes.
- `target/reviews/forest-repair-20261021/art-20261024-final-workshop47/camera.png`
  (R2): visible entrance gable and deeper umbrella crowns. The dark roof and
  repetitive wall bays still need broader material/composition work.
- `target/reviews/forest-repair-20261021/art-20261024-final-wands-card/camera.png`
  (R2): native compact Ignite card with shared Burning/Wet links. The R2 header
  uses too much space at this UI zoom and is superseded by R3.
- `target/reviews/forest-repair-20261021/art-20261024-r3-wands-expanded/camera.png`
  (R3): bounded dropdown header, reachable library region and readable expanded
  Blood siphon card. The review hook exercises the same card renderer as hover.
- `target/reviews/forest-repair-20261021/art-20261024-r3-production/camera.png`
  (R3): readable keyed power graph and empty-state layout. This generated world
  has no producing factory; loaded production rows and bottleneck interaction
  are not established by this image.
- `target/reviews/forest-repair-20261021/art-20261024-r3-motion-hud/` (R3):
  16-second native walk/sidestep frames and `traversal.mp4`; sampled frames show
  readable mana/cast text and deeper forest masses. This revealed cardinal text
  colliding with the compass heading. R4 reserves an inset heading compartment
  and clips ticks/labels to the remaining ribbon, without changing UI scale.
- `target/reviews/forest-repair-20261021/art-20261024-r4-motion-hud/` (R4):
  completed 42-second route, exit 0, 83 PNGs and a 6,131,910-byte MP4. Inspected
  pan, grazing-angle and elevated frames (`bench-034`, `050`, `070`). The heading
  stays inside its compartment; crowns and the workshop remain readable across
  these views. This camera route does not prove physical player traversal or
  exhaustive LOD correctness. Frame timings are recorded, not performance acceptance.

`Review-ForestRepair.ps1 -CaptureHud` now records the HUD and review-card
environment in its manifest. Benchmark-only `VOXEL_REVIEW_SPELL_CARD` selects
the actual hover renderer; `VOXEL_REVIEW_CODEX=1` exercises its expanded branch.
This establishes native layout evidence, not pointer/keyboard interaction proof.

Remaining acceptance work includes wider seeds and biomes, other voxel scales,
interior traversal, live spell-card interaction and sound audition. These are
explicit limits of this increment, not completed quality gates.
