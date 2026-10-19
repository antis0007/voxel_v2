# Graphics and world design review

## Rendering changes

The scene now uses an RGBA16F intermediate. Sky, terrain and premultiplied
cloud radiance are composed in linear HDR and tone-mapped together, before
optional FXAA and the native-resolution HUD. Disabling FXAA still resolves HDR.
Clouds are independent of the editable terrain ceiling: low, middle and high
layers have separate altitude offsets, shared by marching and terrain shadows.
Fine cloud erosion reuses the packed noise channels and distance/footprint mips;
it does not increase the view-ray step budget. Weather changes density and
vertical profiles rather than the frequency of noise multiplied by position.

## Why the world still lacks variety

1. **Landform grammar is narrow.** `world/landscape.rs::bedrock` combines the
   same continental, ridge and hill noise. Generator 3 adds warping, folded
   mountains and an inland spawn clearing, but most silhouettes still come
   from a single height per column; cave carving adds volume below it, but
   cannot create broad above-surface arches and overhanging mountain masses.
   Add region-scale landform families and constraints before adding more noise.
2. **Biome selection has hard overrides.** Outside the generator 3 spawn
   clearing, `landscape::sample` declares any
   height above 85 m or slope above 0.85 alpine before considering climate.
   Taiga columns always use snow grass. This collapses many different hills
   into snowy/bare terrain and causes abrupt surface boundaries. Use continuous
   ecotone weights, a climate-driven snowline and separate slope exposure.
3. **Climate is mostly two noise fields.** Temperature and rainfall lack
   prevailing-wind rain shadows, coastal moderation, seasonal state and
   drainage connectivity. These relationships would make regions recognizable.
4. **Erosion is local, not watershed hydrology.** Overlapped 256 m tiles run
   48 iterations on an 8 m lattice. Lowland channels are noise contours, without
   guaranteed connected sources, outlets, lakes or downstream flow. Build a
   coarse drainage graph and channel geometry before refining local erosion.
5. **Decoration has few families.** Trees use jittered 12 m cells and mainly
   broadleaf/conifer variants; bushes use 8 m cells. There are few distinct
   ecological assemblages or ground-cover species. Generator 3 now adds three
   ruin grammars, fungal chambers and connected galleries in `world/sites.rs`;
   those are a useful expansion, but a small family of templates still repeats.
   Clustered plant communities, scree, deadwood and region-specific settlement
   or landmark rules would add stronger identity than increasing object density.
6. **The distant world is still a coarse heightfield.** The current renderer
   builds an asynchronous, seven-ring backdrop from `world::distant_surface`
   and blends it under editable terrain. It adds landscape silhouettes beyond
   the chunk stream, but cannot show caves, overhangs, landmarks, or voxel-scale
   structures. The view is atmosphere-aware, though its far-distance shading
   remains a low-cost approximation rather than full voxel lighting.
7. **Geology is mostly depth/noise rules.** Copper/tin and limestone expand the
   newer sites, but terrain ores are still weakly tied to geological formations;
   the basalt check below -24 m lies below the current -16 m physical world
   minimum. Geological regions and reachable strata need a versioned generator
   change rather than silently changing existing saved worlds.

## Remaining visual limits

Cloud silhouettes and self-shadowing remain approximations. Fine texture is
still soft in several views; coarse weather shadows do not exactly reproduce
the marched density. Water is an opaque terrain material with sky reflection,
not transparent/refraction-based water. Indirect lighting is ambient/fill, not
transport through the voxel world. The flat hand does not yet share world PBR
lighting. These are visible limits, not evidence of finished graphics quality.

Sunlight now uses a nearly white source; atmospheric transmittance supplies its
low-elevation colour. The previous orange source multiplied by spectral
extinction applied sunset warmth twice and made the upper sky brown.

The inexpensive direction for better sunsets is coherent spectral illumination,
cloud shape/edge refinement, bounded self-shadowing and shared HDR exposure.
The reference approaches are [Guerrilla's Nubis](https://www.guerrilla-games.com/read/nubis-evolved)
and [Hillaire's atmosphere work](https://sebh.github.io/publications/egsr2020.pdf).
They demonstrate viable techniques; their performance figures do not predict
this engine's cost or establish photographic realism here.

## Reproduce the terrain review

```powershell
cargo build --release --locked --offline --target-dir target
target/release/minecraft_style_voxel_prototype.exe --benchmark-motion=cloud-review-far --benchmark-capture-frames --benchmark-world=hills --benchmark-width=1600 --benchmark-height=900 --benchmark-warmup=2 --benchmark-duration=149
```

The route follows actual terrain near X=16384 m, Z=-8192 m, with a 24 m camera
loop and yaw/pitch sweeps. Five 15-second noon phases precede five sunset phases.
GPU captures include the final HUD and never capture unrelated desktop windows.
Review input leaves the mouse released. `--benchmark-review-start=45` jumps to
Stormy at noon; `--benchmark-debug=cloud-alpha`, `cloud-density` or `cloud-shadow`
selects a diagnostic surface for a focused replay. Capture logs record camera
position, terrain height and mouse-capture state. Runtime evidence is kept under
`target/cloud-captures`; build success alone is not visual acceptance.

## Verified V5 candidate

The release build passed and its preserved executable completed a 149-second
measurement plus warmup over Hills at X=16384 m, Z=-8192 m. All five presets
were reviewed at noon and sunset while translating and sweeping yaw/pitch.
Forty fresh 1600x900 GPU exports were checked, including paired sky and terrain
views. No full cloud dropout or the earlier sharp storm plane appeared in those
finite samples. Every capture event recorded `mouse_grabbed=false`. Automated
review skips the save service. HUD text contrast also survived native startup.

Evidence, executable hash, logs and comparisons are preserved in
`target/graphics-review/verification.json` and `target/graphics-review/captures`.
Use `target/graphics-review/run-reviewed-game.cmd` to launch this frozen build
from the correct asset directory. Subsequent edits in other chats are separate
from this verified executable.

RTX 4090/Vulkan at 1600x900: average frame 7.953 ms, cloud pass 6.607 ms, p99
frame 15.351 ms. Increased morphology/self-shadow work costs more despite the
unchanged view-ray step budget. This is not yet a general low-cost renderer and
is not photographic parity. Distant terrain, thin geometry aliasing, repeated
landforms and the hand's independent lighting remain visible limitations.

## Source and capture review — 2026-10-18

The existing 1600×900 capture set confirms that cloud work is ahead of the
ground scene. `comparison-noon-terrain.png` shows an alpine view with nearly
continuous white slopes, abrupt dark stair risers, and a long empty horizon.
`terrain-review/v3/woodland/.../Hills-00-Clear-noon-3.png` shows the
complementary case: yellow-green ground, high-contrast orange crowns, and
block-separated tree silhouettes. These are observations from preserved
captures, not claims about every seed or setting.

The source explains the main color and biome failures:

1. `world/landscape.rs::sample` subtracts `0.0035` from normalized temperature
   per metre above 30 m, then lets elevations above 105 m or slopes above 1.25
   override climate as Alpine. At high ground, this drives temperature to zero
   and turns broad ranges into alpine/snow material. `world/landscape.rs::surface`
   also gives Taiga a snow surface before patch selection. Generator 7 now uses
   a gentler lapse, reserves Alpine for summits, narrows the designed spawn
   clearing, and combines elevation, cold, shelter, and local variation for
   patchy snow. Earlier generator versions retain their saved terrain rules.
2. `shaders/world_shader.wgsl::block_surface` selected dry grass with an
   olive-yellow linear color and drove autumn foliage with a broad procedural
   field at up to 85% mix. `material.rs` also gave the main ground and leaf
   materials brighter, more saturated swatches. The new defaults use a
   restrained earth/forest palette; grass gets moss and leaf-litter patches,
   foliage uses less saturated seasonal variation, and rock gets a quieter
   cool-to-warm mineral range. Generator 7 varies crown proportions while
   keeping the branch sampling budget unchanged. Generator 8 preserves that
   climate work while varying broadleaf form per tree instead of per species
   cluster, reducing repeated crown silhouettes.
3. `shaders/world_shader.wgsl::fs_main` used low sky fill and minimal ground
   bounce; its ambient term was also multiplied by mesh cavity AO. Terrain
   faces consequently lost color separation in shadow. The revision raises
   broad sky fill and warm ground bounce without adding shadow taps or cloud
   render cost. This remains a hemisphere approximation, not global
   illumination.
4. `render/streaming` still ends editable terrain at the chunk reach, while
   the sky and cloud renderer extend much farther. `render/horizon.rs` now
   supplies a shared-world-sampler heightfield beyond that edge; it is a visual
   backdrop, not an editable terrain LOD or a representation of caves and
   above-surface volume.

Generator 8 is the default for new worlds and is accepted by the save decoder.
It carries forward generator 7's landscape and snow rules; versions 1–7 retain
their generation rules.

## Fantasy-industrial art direction — 2026-10-18

The palette now uses quiet lichen/sage ground and mineral stone as the shared
environment base. Industrial blocks keep graphite housings and worn timber,
with copper/ember reserved for heat and patinated teal for power and fluids;
arcane violet is reserved for rare crystal and spell accents. The cultivator
cache no longer uses an unrelated magenta swatch. This applies the readable
material families of classic voxel automation games within the same fantasy
world palette rather than reproducing a specific mod's assets.

Generator 8 is the new-world default. It inherits generator 7's climate,
snowline, and spawn-basin changes, and varies broadleaf crown family, branch
height, and lobe size per tree. Versions 1–7 remain stable. The release build
completed, but the follow-up cloud-review run emitted repeated terrain upload
staging failures and a GPU device-loss/profiling warning; no fresh frame was
saved. The earlier generator 7 capture is not evidence for this final tree
change, and no performance acceptance is claimed. Distant terrain
LOD, true voxel indirect light, water refraction, and above-surface landform
volume remain open rendering work.

## Distant atmosphere shader — 2026-10-18

The first dedicated backdrop fragment path was rejected during the subsequent
capture review: it omitted cloud-shadow attenuation and made far ground much
brighter than nearby ground. Both now share diffuse sky/weather lighting and
atmospheric composition in `shaders/world_shader.wgsl`. The backdrop still skips
local torch evaluation and voxel shadow-map sampling. Coarse tree proxies are
excluded within the larger of 1.15 times the resident radius and 512 metres so they cannot
overlay detailed trees near the player.

## Screenshot artifact repair — 2026-10-18

The user's screenshot shows stippled terrain/foliage, a repeating cloud sampling
pattern, and abrupt distant illumination. The terrain cause is explicit in
`world_shader.wgsl::fs_main`: screen-space threshold discards were exposing the
backdrop through foreground geometry. Terrain has no temporal reconstruction to
resolve those holes. Both surfaces now retain opaque coverage and use depth with
the existing backdrop underlap. Recentring replaces the backdrop atomically;
`render.rs` no longer draws an old mesh through a second dither mask. A geometry
transition at the residency edge may still be visible and needs capture review.

The screenshot's running executable predates the ranked noise texture and cloud
sampling changes already present in source. Those changes must be reviewed in a
fresh executable before attributing remaining cloud patterns to the current code.
Capture runs can use `VOXEL_CAPTURE_DIR` to preserve earlier evidence. The HUD
now centres hearts and hunger icons without redundant numeric labels.

### Capture review and remaining art work

The four art issues recorded below were subsequently addressed and visually
reviewed in `ATMOSPHERE_ART_REFINEMENT.md`; retain this section as the earlier
capture receipt.

The release in `target/visual-repair/refined/voxel-repair.exe` was built with
offline Cargo and the build script's composed-WGSL validation. Fresh 1600x900
captures were inspected at noon and during a moving 0.72 solar-time flyover,
using Hills seed 2738958700. These are two bounded views, not all-weather or
performance acceptance. No automated tests were run.

- `refined/noon/hills-detail/None/Hills-01-PartlyCloudy-noon-3.png`:
  opaque terrain and foliage, no repeating cloud checker pattern in this view,
  no near-camera crown proxies, and substantially closer near/far illumination.
- `refined/dusk-flight/horizon-review/None/Hills-01-PartlyCloudy-noon-3.png`:
  the filename retains the route's `noon` label, but the log records solar time
  0.72 and a moving elevated camera. This shows the remaining dark distant band
  and shelf-like cloud silhouettes. Those are not resolved by this repair.
- The cold backdrop took about 22 seconds to appear. The interim horizon and
  atomic replacement still need a better loading/LOD transition.
- Distant crown proxies remain visibly geometric and sparse; the plain hand
  remains oversized and disconnected from environmental illumination.

Code ownership: `world_shader.wgsl::fs_main` handles opaque surface overlap,
shared weather lighting, and near proxy rejection; `render/horizon.rs::update`
replaces the backdrop; `render.rs` draws one backdrop; `egui_ui/adventure.rs`
centres icon-only survival meters. The final integration also includes the
inner-radius backdrop exclusion so heightfields cannot fill caves/excavations.
Use `run-atmosphere-repair.cmd` for the packaged build. Build logs and source
hash receipts live beside each executable; other active work in this checkout
may advance source after a receipt was taken.

Final packaging receipt: `target/visual-repair/final/voxel-repair.exe` completed
the release build and composed-WGSL validation. Its fresh noon capture at
`final/noon/hills-detail/None/Hills-01-PartlyCloudy-noon-3.png` was inspected after
the cave/excavation exclusion was integrated; the run's error log was empty.
This verifies the displayed outdoor view, not excavation gameplay. The moving
dusk evidence above belongs to the preceding refined binary. Source changed
concurrently in `creative.rs` and `main.rs` during packaging; the frozen binary
hash is recorded in `final/binary-receipt.json` for reproducible identification.
