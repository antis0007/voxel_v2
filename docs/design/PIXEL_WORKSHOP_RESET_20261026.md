# Pixel workshop reset, 26 October 2026

Owner direction: a coherent Minecraft/Noita/RimWorld-inspired game with the
colour craft of Chrono Trigger, Link to the Past and GBA art. Contemporary
lighting and material effects should support that direction. This document
supersedes the held-kit implementation in `VIEWMODEL_RESTART_20261026.md`.

## Why the previous models failed

The previous implementation confused surface treatment with design. Remeshing
an awkward silhouette into voxels retained the awkward silhouette. Dense
geometry and procedural grain gave the arm and wand different levels of detail
from the world. Repeated sleeve bulges looked machined. Separate finger bars
did not describe a convincing grip. Increasing detail amplified those problems.

The first new atlas candidate also had faults: a slab-like palm, fingers that
did not curl in depth, an extra sleeve band, and broad peach surfaces with too
little material definition. Native inspection exposed these; the second
candidate adds closed finger profiles, palm volume and a quieter sleeve.
It remains a candidate for visual review, not proof of a finished art style.

## Working visual rules

- Use a readable silhouette before surface detail. A wand must remain identifiable
  as a thumbnail and as a shadow. Three large masses: grip, shaft, working head.
- Give objects a construction story from the lore's workshops: split timber,
  ceramic insulation, a copper strap, a recessed mineral. Function guides accents.
- Use connected pixel clusters. Draw grain, chips, seams and compression folds;
  independent random specks do not substitute for painting.
- Keep broad surfaces quiet. Concentrate contrast at the grip, fittings and
  working element. Skin has warm and cool planes, not a crease grid everywhere.
- The SNES/GBA references guide colour and clustering. The 3D game keeps stable
  perspective and continuous movement. Avoid a global pixelation filter or
  deliberate perspective wobble as a substitute for authored assets.
- World albedo uses its existing 5 cm grid. Held assets have about 4.5–5.5 mm
  texels because they occupy the foreground. Keep this distinction explicit;
  do not arbitrarily mix texel sizes within one material or adjacent kit parts.

## Implemented pipeline

`tools/pixel_kit/build.py` creates the kit directly in Blender. Its outline
cages, single cut edges, painted atlas and five grip joints are editable in
`assets/viewmodel/pixel_source/workshop_kit.blend`. There is no voxel remesher
or texture-sized geometry. `forms.py` owns shapes; `atlas.py` owns colour
clusters; `mesh.py` exports the native triangle cache.

`assets/viewmodel/kit.png` is a 64×64 atlas using eight five-colour ramps.
`kit.rgba` is the identical 16 KiB upload, with a compile-time length check.
`palette.gpl` can be imported into a pixel editor. Rebuilding regenerates paint
from `atlas.py`; preserve manual PNG edits before running it.

Blender uses Closest filtering. The game uses an sRGB texture and integer
`textureLoad`, so colour reaches lighting in linear space and no bilinear
interpolation invents intermediate pixels. UVs follow the asset during camera
motion and articulation. Existing material IDs and the vertex layout stay intact.

There are six shaft materials and six head silhouettes. Sleeve scale stays
uniform. Mana and deck inlays sit on the shaft. The selected spell colours the
mineral; release energy increases its emission without repainting the wood,
skin or cloth. This is presentation of the existing cast state.

## Software workflow researched

Blender is appropriate for proportions, UVs, grip articulation and repeated
native-size renders. Its Python API makes the current asset reproducible;
the saved source allows direct edits. Judge both silhouette and material in the
actual game before accepting a studio render.

Aseprite's [indexed colour](https://www.aseprite.org/docs/color-mode/) supports
palette-wide edits. Its [tiled view](https://www.aseprite.org/docs/tiled-mode/)
supports checking repeating terrain textures. Its
[CLI](https://www.aseprite.org/docs/cli/) supports repeatable exports and padded
sheets. These are researched capabilities; Aseprite was not used to paint this
candidate. Pixel placement and colour relationships still require art judgment.

Review a texture at 1×, a magnified integer scale, and in a repeated wall/floor.
Then review the mapped object at play distance. A good standalone swatch can
fail on a long shaft or a bent hand. Do not upscale a generated concept into a
shipping texture; the generated sheet in `pixel_source/concept.png` is a shape
reference, not a screenshot or production atlas.

## Material and effect direction

These are design targets beyond this held-kit slice; they are not newly
implemented simulation systems.

| Material/state | Visual information | Restraint |
|---|---|---|
| Timber | connected grain, exposed ends, charring following actual damage | dry broad diffuse response |
| Cloth/leather | compression folds, worn edges, wet value shift | no metal sheen or repeated rings |
| Copper | warm edge response, patina in protected areas, current at contacts | no constant full-object glow |
| Ceramic | large quiet planes, glaze breaks, heat at a real hot region | chips near stressed edges |
| Mineral | internal palette clusters, energy tied to charge/release | preserve the body silhouette |
| Water/ice | depth, shore contact, phase and temperature | same spatial pixel language |
| Shield | thin boundary, local impact ripple, overload and recovery | preserve the scene behind it |

For combinatory science and magic, communicate cause and persistence: heat
changes a material; water carries charge where the simulation allows it;
pressure vents through an opening; a shield impact shows a local normal and
energy cost. A shader must read real state rather than imply a nonexistent
interaction. Distinguish heat, charge, pressure, dissolution and resonance by
motion and shape as well as colour. Establish these on a small material scene
before applying decorative effects to every surface.

## What the references contribute

Our interpretation: modded Minecraft is compelling when simple, identifiable
blocks become tools for building larger systems. The visual lesson is legible
interfaces between parts and materials. Its
[official texture iteration](https://www.minecraft.net/en-us/article/try-new-minecraft-textures)
is a useful reminder that even a small texture set needs repeated art review.

Noita explicitly combines [material simulation and player-crafted spells](https://www.noitagame.com/).
Petri Purho's [GDC talk](https://www.gdcvault.com/play/1025695/Exploring-the-)
describes the physics and emergent gameplay. Our design inference is that
discovery comes from consistent rules with surprising combinations. Effects
should help players infer those rules and remember a consequence.

RimWorld informs state readability and events that become memorable stories.
Chrono Trigger, Link to the Past and GBA references inform colour separation,
selective contrast and composed areas. These are art interpretations, not a
claim that their different hardware restrictions form one technical standard.

Mycopunk's public developer posts include projection-correction shader code and
layered rendering explanations in its
[Steam developer feed](https://steamcommunity.com/app/3247750/announcements/?l=english).
The investigated code addresses perspective warping, not energy shields. No
public shield-shader source was found in this research. We have not claimed to
read or port that source. The transferable goal is tactile, readable response.

## Evidence

Builds and frozen executables: `target/pixel-kit-reset-20261026/`.
Native captures: `target/artificer-20261025/pixel-reset-*` (the harness retains
its historical directory name). Manifests record the executable hash, arguments
and isolated save. Capture readbacks are not performance measurements.
The earlier rejected source and assets remain available; unrelated world,
factory, UI and renderer work was not reset.
