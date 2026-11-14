# Visual audit, 8 November 2026

The reviewed world still reads as a rendering prototype. It contains detailed
systems, but their visual priorities compete: near-black cubic crowns, soft
ground mottling, sharply outlined equipment, blank roof slabs and pale distant
terrain do not form one convincing place. More procedural detail by itself will
not fix this. Missing terrain at the distance handoff is a release blocker.

This is a critique and implementation guide, not a completed app-wide art pass.

## Evidence boundary

Images inspected directly:

- User's forest screenshot, `codex-clipboard-ade8b315-7f09-4153-a6b5-dca6c289bc4a.png`.
  Its exact generator, material underfoot, time and graphics settings are unknown.
- `target/civilization-planet-review/final-town/captures/bench-006.png`:
  the integrated seed-74 Planet-286 town view, compiled candidate
  `717B3870D1610CB834EF36C0B91E9237DADED53C1471B97A7ECA8AE8DFAA2FC3`.
- `target/orbital-surface-review/final-day/captures/camera.png`, captured today
  before the civilization integration. This is an earlier orbital candidate.
- `target/workshop-ui-review/OneMeter-002.png`, captured 6 November: a historical
  native Logistics screen, not proof of every current menu.
- `target/visual-continuity-review/refined/title/title.png`, `title-compact.png`
  and `title-narrow.png`: current production title renderer at 1600 x 900,
  640 x 480 and 360 x 640, with staged menu data. The export receipt identifies
  binary `CFD97538904049AB3E703329686BA049C43E28C268AEC5EBC5778B8FCD786823`.
- `target/civilization-planet-review/refined-ground-material/captures/bench-000.png`
  and `refined-inventory/captures/camera.png` under the same review directory:
  current native material IDs and actual-world inventory from the same
  `CFD975...6823` binary. Their receipts identify isolated review saves.

Current source and existing art documents were also inspected. No native game,
build, tests or CI were run for this audit. Still images do not establish sway,
particle motion, transitions, sound, interaction, or smooth performance.

## Critique by scale

| Level | What fails in the inspected evidence | Required visual result |
| --- | --- | --- |
| Ground continuity | Bright triangular openings interrupt the town's distant hillside. The viewer sees the construction of the renderer before seeing a landscape. | The same continuous landform through every handoff, including moving views. Fog must not conceal missing geometry. |
| Immediate foreground | Grey-blue floor planes and dark recesses dominate the forest image. Material boundaries are weak; fine mottling looks filtered while foliage texels remain hard. | A readable substrate, a small material ramp and connected clusters at the same gameplay distance. Identify the material before repainting it. |
| Foliage | Large rectangular leaf masses look like stacked boxes. Adjacent dark faces merge, and the leaf pattern resembles camouflage on a solid slab. Trunks often disappear into the crown. | Distinct crown silhouettes, visible branch support, restrained sky gaps, and a readable top/side/underside value hierarchy. Texture cannot supply missing silhouette. |
| Plants | The forest floor has repeated thin stalks and similar fern forms scattered across open planes. There is no strong visual distinction between a colony, a clearing and a walked route. | Clumps with cores and edges, bare intervals, size hierarchy and substrate-linked species. More instances everywhere would make the noise worse. |
| Local colour | Shadowed ground, foliage and trunks collapse into blue-green darkness. The sky is readable while the playable space loses information. | Separate moss, earth, bark and foliage in open shade while retaining dark cavities. Warm and cool hues need distinct material roles, not a universal saturation increase. |
| Settlement | Repeated square stone boxes with flat brown lids have almost no roof hierarchy, eaves or construction rhythm. Crops explain activity better than the buildings do. | Foundations, supported roof masses, clear doors, work/storage edges and variation derived from use. A prettier brick pattern will not turn these boxes into a convincing town. |
| Valley | Foreground dark masses, reflective river and pale backdrop occupy different visual styles. Repeated trees do not consistently frame a destination or route. | Three legible depth ranges and a walkable route that remains readable when the camera moves. Rivers and settlements should explain each other's location. |
| Planet | The inspected globe has relief, but its large colour islands still read as a categorical map; the strongly textured regional patch differs from surrounding terrain. | The same geology, ecological masses and material families at orbital and walking scales. Preserve the actual continents and drainage; add no decorative fake erosion. |
| Sky and atmosphere | The town's distant forms wash toward a nearly white band. That reduces contrast exactly where gaps need diagnosis. The forest sky is simple compared with the noisy foreground. | Weather-dependent depth that preserves a readable skyline. A fog or bloom increase is not a geometry repair. Motion and weather variety remain unreviewed here. |
| Water | The town water is legible and reflects the scene, but high-frequency broken highlights compete with small plants and roof edges. | Calm broad water masses with detail concentrated at current changes, banks and contact. Do not reduce real current behaviour to an ornamental texture. |
| Equipment and effects | The articulated hand/wand is more sharply authored than the surrounding world. The town torch's large flat orange shape becomes the strongest focal point. | Preserve the authored kit; bring world materials up to its deliberate clustering. Fire needs a compact readable core and local illumination without dominating daylight. Motion was not inspected. |
| Interface | The historical Logistics capture is more coherent than the world: flat dark panels, warm type and bounded colour work. Repeated headings/status text and several borders compete at the same weight. | One primary heading, one useful status and consistent spacing/slot geometry. Preserve the approved rainbow selection and pixel magic. No evidence supports replacing the whole UI theme. |

Current generation epochs, map, trade, tooltips, caves,
underwater views, nights and moving effects require fresh native review. Their
source capabilities are not visual acceptance. The current title exports and
inventory capture plus historical Logistics image do not establish that all
screens were inspected.

## Art direction to retain

The existing direction is sufficiently specific: editable voxel masses, SNES/GBA
colour craft, a modded-Minecraft interface, and functional workshop construction.
The failure is inconsistent execution, not the absence of a new style label.

Keep broad planes and connected pixel clusters; pale warm masonry, dark timber,
restrained warm roofs; related olive/sage ground and crown families; small teal,
gold and ember accents attached to meaningful objects. Materials own albedo;
sun, sky and local sources own illumination. Keep the existing linear-light path,
footprint filtering, authored equipment, spell identity and rainbow selection.

Do not put the entire world through a stronger contrast/saturation grade. Do not
add random specks to hide dull materials, lift all caves, flatten conifer values,
or redesign menus based on an old screenshot. Global sharpness is not material
craft and will make distant foliage shimmer.

## Three bounded implementation candidates

### 1. Restore readable canopy material separation

Owner: `crates/app/src/shaders/world_shader.wgsl`, `block_surface` foliage branch
around line 2662 and ambient/botanical response around lines 4260 and 4520.

The foliage branch already contains three-tone ramps, but its pine weight is
inferred from `base.b - base.r`. `block` is already an argument. Prefer explicit
PineLeaves identity for resident materials, with a deliberate documented fallback
for proxies that have no block identity. A hue correction should not silently
switch a broadleaf crown into the needle shading branch.

Before increasing light, inspect the existing albedo (18), AO (28), overhead
access (29), ambient access (36), direct diffuse (23), ambient diffuse (24), and
texture-free massing (32) modes. The current response combines a 0.80-0.90 middle
tone factor, tropical remapping, shaped AO, sky visibility, a darkened botanical
fill tint, and possibly screen-space AO. These terms are source observations;
which dominates the supplied screenshot is not established.

First material candidate: retain conifer range and current top hue, but lift
broadleaf middle/underside reflectance by about 10-15% and narrow the darkest
leaf-cluster pigment. Compare against a second candidate that only relaxes
foliage cavity shaping. Keep whichever restores distinct masses with less loss
of depth. Do not combine both blindly. Use the same response for distant crowns.

Risks: chalky leaves, uniform bright conifers, excessive night visibility and
near/far colour mismatch. Keep light-dependent transmission and actual sky
occlusion; add no emissive canopy floor. Reject a candidate that makes a sealed
cave or nighttime forest look sunlit.

### 2. Give ground clusters definition without changing their distant mean

Owner: `world_shader.wgsl`, `grass_top`, `grass_side`, `natural_surface`.

The current grass-top pigment values mainly span 0.87-0.98 and filter to
`turf * 0.90`; repeated low-amplitude mottle is weak beside crisp foreground
equipment. First confirm that the target surface is grass using material IDs
(19) and albedo (18): the grey foreground cannot be identified reliably by eye.

Candidate: strengthen the connected tuft/root signal around its existing 0.90
mean, keeping broad areas quiet, rather than increasing independent texel noise.
Aim for a modest local root-to-tip separation (roughly 0.84-1.02), then normalize
the actual averaged response back to the same distant mean. Retain the existing
footprint fade. Keep top/side fringe averages consistent; no abrupt new metre
grid or artificial edge outline. Apply an equivalent bounded cluster treatment
to the actual stone/soil material if the diagnostic identifies a different floor.

Risks: swimming texels, visible distance bands, patterned carpet and overbright
terrain. Review at standing height, grazing angle and middle distance in motion.
Avoid a global texture-resolution increase until these reads work.

### 3. Give one generated house a complete construction language

Owner: `crates/worldgen/src/civilization/fabric.rs::surface`, with the civilization
owner preserving doors, worker circulation, stockpile/station positions and saves.

The source confirms the captured houses: every occupied lot uses the same square
wall envelope and `v >= ROOF` plank slab. Start with a modest pitched or stepped
roof, narrow eaves, a visible wall plate, a recessed framed entrance and a plinth
that fits the existing datum. Keep work areas and access open. Use two or three
seed-stable construction variants tied to livelihood/material availability;
random decorative blocks are not architectural variety.

This is more consequential than a shader adjustment and requires the appropriate
generator/save gate. It must not silently rewrite inhabited saved homes or place
fixtures solely for the review camera. Improve a real generated town and view it
from both approaches and indoors. Larger growth, commerce and city planning
remain separate functionality, not implied by a better roof.

## Review order and acceptance

Repair continuity first. Compare the bounded canopy and ground candidates with
fixed seed, camera, resolution, weather, time and normal presentation conditions.
Keep rejected captures and performance failures. Check the same site at noon,
in open shade and after dark; include a dry or cold contrasting biome. Then review
the real town's approach, doors and work areas before extending architecture.

Inspect UI at ordinary scale on a clean world, inventory/recipes, map, trade,
world creation and generation handoff. Reuse existing typography, margins,
palette roles and active-state effects. Remove duplication and inconsistent
hierarchy where observed; do not infer acceptance from compilation.

The acceptance question is whether a player can identify material, route, depth
and purpose without diagnostics, while the renderer stays continuous and stable.
No implementation or visual improvement is claimed by this audit document.

## Ground material follow-up

Two further native images were inspected from the same `717B...FC3` candidate,
seed 74, generator 286, camera `323325,2127,1,-0.4815,-0.1`:

- `target/civilization-planet-review/ground-albedo-before/captures/bench-001.png`
  is a **beauty** image. Its receipt used the unrecognized
  `--benchmark-debug-view=albedo` spelling; the folder name does not make it an
  albedo diagnostic.
- `target/civilization-planet-review/ground-albedo-diagnostic/captures/bench-001.png`
  uses the recognized `--benchmark-debug=albedo` flag and shows material colour
  without scene illumination. These are a diagnostic pair, not an art-change
  before/after comparison.

The largest material failure is uneven visual frequency: the foreground's
gravel/rock-like blocks carry a dense dark maze across almost every visible
face, while adjoining terrace tops become nearly blank. The pattern persists
in albedo. Dark lighting exaggerates it, but cannot be its sole cause. It reads
as a noisy patterned carpet beside untextured slabs, rather than mineral planes,
loose chips and weathered ground. Increasing global contrast or sharpness would
strengthen the wrong feature.

The later material-ID capture is at the same seed, generator and ground camera.
The root agent's sampled foreground RGB 111,242,114 matches kind 33, Gravel;
adjacent terrace RGB 188,195,238 matches kind 3, the rock family
(Stone/Basalt/Hellstone/Limestone); canopy RGB 169,212,209
matches kind 6. Direct inspection confirms those distinct diagnostic regions,
and `natural_surface` routes kind 33 to `gravel_surface`. The earlier conditional
rock-fracture proposal is therefore rejected for this scene: it would change
the wrong material.

The implemented source candidate instead changes gravel fines from 0.56 to
0.68, crevice darkening from 0.30 to 0.10, pebble value from `0.78 + id * 0.34`
to `0.82 + id * 0.22`, and painted directional modulation from 0.20 to 0.10.
It retains the same chip shapes, coverage, mineral palette, distant 0.80 fallback,
filtering, normal tilt and material finish. The intended effect is mineral chips
in a continuous bed, rather than near-black outlines around every chip. Reducing
painted directional shading also leaves more of that work to actual lighting.

An unchanged far fallback does not establish an unchanged near average. Dark
fines rise from about 0.392 to 0.612 times the mineral colour, while the centre
of the pebble range moves only from 0.95 to 0.93. Native standing-height and
middle-distance comparisons must check for bright foreground gravel that fades
too dark at distance. Do not add noise to the quieter adjoining rock just to equalize their
visual frequency. This source review does not accept the rendered candidate.

The current canopy/grass candidate was reviewed in source only during this
follow-up. Its explicit PineLeaves classification removes palette-dependent
species selection; its 12% broadleaf side/underside lift remains light-dependent.
The grass contrast expansion retains the 0.90 distant fallback, but that does
not prove an identical near-field average. Current material candidate images
remain pending at this point.

## Current title review

The three title exports above use the production egui/wgpu menu and GPU workshop
diorama, with staged recent-world data. They establish current composition and
static readability at those sizes. They do not establish live world rendering,
navigation, animation, focus behaviour, generation handoff or a full UI pass.

The hierarchy is substantially more coherent than the inspected world. Warm
paper type, a restrained amber primary edge and dark blue secondary controls
give Continue clear priority. Settings and Quit are quieter without disappearing.
The title is crisp and the small world name stays subordinate. The 360-pixel
view stacks Worlds and New World cleanly; none of the three inspected layouts
clips or overlaps its visible labels. Preserve this structure and the approved
focus effects. A wholesale palette or typography replacement is unsupported.

The backdrop is the failure. At 1600 x 900, timber framing, masonry, warm tools
and a small teal machine communicate a workshop coherently. At 640 x 480 the
menu covers nearly all of those useful props; the bench becomes a thin strip.
At 360 x 640 the visible scene is mostly roof beam, dark wall and paving. The
background has lost its subject, while its roof line passes directly behind the
heading. This is a framing problem rather than insufficient texture detail.

The title and diorama also sell a small crafting room much more clearly than
planet exploration or living settlements. That is a gap in the game's visual
promise, not permission to rename the game or replace its established workshop
identity. The current scene cannot establish the quality of generated towns.

A bounded later candidate is an aspect-aware diorama composition that retains
one recognizable working prop outside the control stack in compact and portrait
views. Preserve the menu's quiet region and button geometry; verify the same
framing with busy and long-world-name states before adopting it. In the wide
view, one restrained outward sightline to a recognizable landscape or settlement
would communicate scope better than extra UI copy. This remains an art candidate,
not implemented or accepted evidence. Keep the scene self-contained and cheap;
do not add live terrain streaming merely to decorate the title.

## Current inventory review

`refined-inventory/captures/camera.png` is a 1280 x 720 actual-world inventory
capture from seed 74, Planet 286, camera `323480,1782,18,0.635,-0.36`. The receipt
records review panel 2, HUD capture and an isolated save. This is the production
inventory over the terrain, not the separate menu exporter's inventory showcase.
It is still a static review, not verification of drag, crafting, search or tabs.

The slot grid, item silhouettes, warm type, restrained outline and selected
hotbar accent form a coherent panel. The adjacent catalogue is useful and fits
the image. The body is unmistakably inventory and hand crafting, but the first
visible labels are Projects at top left and Portals above the general catalogue.
That makes unrelated destinations look like the current screen and section names.

Source inspection finds no page-routing failure. Projects is the navigation
control for page 9; the selected crate icon and body correctly use page 2.
Portals is a real collapsed guide, prepended unconditionally by
`recipe_book::sidebar`; the same guide is already present in the dedicated
Recipes page. The bounded source correction now uses an Items heading in the
inventory sidebar and retains the portal guide in Recipes. It is not present in
the inspected `CFD975...6823` image. The subsequent
`visual-final-inventory/captures/bench-000.png` verifies the Items heading in
compiled candidate `251973...4480`, with the grid and search controls still fitting.
The Projects navigation control retains its correct destination and name.

## Final native pass: what improved and what did not

The final candidate was rebuilt and reviewed in the same real town and on the
ten-leg valley walk. The town's reported white hillside wedges are absent in
`visual-final-town/captures/bench-000.png`; the distance diagnostic independently
shows nearby terrain throughout the sampled wedge window. The basin has clearer
landform separation. This is a specific observed improvement, not a guarantee
that every possible LOD boundary is sealed.

`visual-final-walk/captures/bench-000.png`, `walk-05.png` and `walk-10.png`
still fail a finished-art standard. Gravel is less harsh but its repeated chip
network remains too busy; neighbouring rock planes are too quiet. Dark trunks
and crown undersides merge, while thick leaf slabs and repeated horizontal crown
tiers remain mechanically obvious. The broadleaf reflectance correction is too
small to repair those silhouettes. The town's old generated houses still have
flat brown lids and repeated envelopes. No architectural improvement is implied
by new civilization behaviour or newer runtime construction code.

The lime crop cubes are probably wild Melons: warm crop placement, full-cube
material geometry and the generic family 39 route support that identification.
Their exact BlockId at the screenshot coordinate was not sampled. They currently
receive generic grain rather than an authored rind, so they read as placeholder
green blocks. ID19's support mask incorrectly marks valid family 39 magenta;
that colour alone is not evidence of an unsupported new material.

Native results and their limits are recorded in
[the continuity report](../worldgen/VISUAL_CONTINUITY_286.md). The sampled town
frame rate recovered and the walk completed on foot, but cold loading is not
proven faster. The world is more readable in this pass; it is still not a cohesive,
finished visual release. Weather motion, wind/audio synchronization, underwater
appearance, full climate epochs and the remaining interface screens have not
received current acceptance from these captures.

The subsequent `visual-final-orbit` flight adds current night-side evidence:
`03-space.png` and `04-face-crossing.png` show the globe through the face crossing.
The luminous rim and tightly patterned cloud cap dominate dark, map-like land
patches. The image does not yet make atmosphere, relief and surface colour feel
like one material system. A daytime comparison is still required; increasing
night exposure globally would not establish it. `06-landed.png` is actually
underwater according to its position/grounding log. It shows a dark seabed, not
a verified landfall or a general underwater art pass.
