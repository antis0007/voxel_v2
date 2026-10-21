# Compass, spell grade, low-light detail, and high-fidelity pixel resolve

This pass adds six connected pieces:

1. a live pixel-art compass with an authored quest waypoint,
2. spell-driven refraction, chromatic separation, shock rings and an arcane
   screen-edge aurora,
3. footprint-filtered micro-relief for resident surfaces,
4. a Wind Waker-inspired water contact treatment: a saturated turquoise shallow
   shelf, a darker waterline, and breathing shoreline foam driven by bed depth;
   lava receives a bounded animated heat-normal shimmer;
5. a modern pixel resolve that keeps smooth sky and fog gradients continuous,
6. faster, more contrasty volumetric clouds with a consistent wind field.

## Compass and waypoint

`Adventure::quest_marker` resolves the same targets as the journal:

- the next `sites::next_relic` target for the current sigil mask,
- the starter workshop heart once all three sigils are held,
- the workshop heart again after restoration, but only when the player is more
  than 64 m away.

The marker stores world position, label, bearing, horizontal distance and
vertical delta. `src/egui_ui/adventure.rs` draws a 36 px heading strip with:

- 15° ticks and N/NE/E/SE/S/SW/W/NW labels,
- a fixed centre caret,
- a coloured target diamond,
- an edge chevron when the bearing leaves the visible 208° arc,
- `Foundry resonance  412 m  ↑ 18 m`-style text below the strip.

The same target also gets a bobbing diamond-and-beam beacon in the 3D model
pass. It is depth-tested, so walls hide it, and its scale grows modestly with
distance so the objective remains legible at several hundred metres.

The compass is part of the native-resolution HUD, so it is crisp text rather
than a pixelated world texture. Enemy names, guardian health, rewards and
journey notices moved below the compass strip.

`src/world/sites.rs::workshop_heart` exposes the same mine-heart position used by
the starter encounter, so the marker cannot drift from the interaction.

## Spell grade

`Adventure::screen_magic` produces a bounded `(intensity, RGB)` envelope from:

- cast pose and mana flash,
- active projectiles and particles,
- plasma crosses and gravity wells,
- active laser beams,
- mirror-ward state.

The resolve receives it as a fourth `vec4` (RGB + intensity). While intensity is
non-zero it layers:

- a block-snapped spiral/ripple lens warp,
- edge-weighted red/blue chromatic separation (the centre stays readable),
- a slow, contrast-preserving colour wash,
- one expanding arcane shock ring,
- fine radial filaments and a low-amplitude screen-edge aurora in the spell
  colour.

The effect clock is `sky_time`, not `frame_index / 60`, so casts keep the same
cadence at 30, 60 or 144 fps. Ordinary exploration stays clean. Debug views
bypass the treatment entirely.

Projectiles now carry a short helical wake of five translucent micro-voxels
behind the core, with a white-hot head fading into the spell colour. This gives
fast bolts a readable 3D silhouette without adding a draw call per projectile.
A cast also injects a short-lived point light at the wand tip through the
existing dynamic torch-light array, so a spell actually illuminates a cave
surface instead of reading as a glowing prop.

## Low-light surfaces

`surface_relief` evaluates filtered value-noise gradients in world metres and
projects them into the shading tangent plane. Built and mined materials receive
it everywhere; natural ground receives it below the skylight threshold, where
the existing far-field relief is weakest. Foliage and water are excluded. The
normal is not a baked normal map, but it gives torchlight a surface to catch and
removes the “milled cube” read from stone, soil, wood and machinery.

Underground also gets:

- a grime multiplier for surfaces with low skylight access,
- a small non-sky cave fill that follows cavity and face hemisphere,
- roughness variation at 22 cm scale,
- pulsing Glowcap and Crystal emission, including a per-face crystal shimmer.

The cave fill is intentionally tiny: it preserves dark caverns while keeping
silhouettes and micro-relief legible.

## Pixel resolve, colour and lighting

The previous resolve sampled and bloomed at native resolution, then threw the
resolution away in 3×3 blocks and quantized every pixel to 32 sRGB levels. That
made broad sky gradients band and spent most of its fill rate on samples that no
output pixel could keep.

The resolve now renders at native screen resolution:

- The resolve target is `Rgba16Float`, so smooth sky, fog and cloud gradients keep
  full precision through the glow and grade.
- The present pass writes the surface once, with a blue-noise dither per native
  screen pixel. This removes 8-bit sky banding without the coarse 8×8 grid.
- Palette quantization and the block-edge contour are disabled by default. The
  authored material ramps and native FXAA carry the image instead of a simulated
  low-resolution palette.
- A restrained split-tone cools shadows and warms highlights after the neutral
  tonemap.
- Selective vibrance lifts less-saturated sky and cloud chroma without pushing
  already vivid grass or spell colours further into neon.
- 6% vignette and two-scale Noita glow at strength 0.30 / threshold 0.72 remain.

Native resolution costs more fill than the 2× art-pixel experiment, but it
restores the material, foliage and lighting detail that made the scene read as a
downscale rather than a designed render. The benchmark scene still sustains
197 fps at 1280×720 on the RTX 4090 review machine.

## Cloud speed, contrast and filtering

Cloud advection previously had six hard-coded multipliers. Increasing shape
speed without the matching weather and shadow offsets would shear a cloud away
from its own shadow and precipitation system. A single
`CLOUD_ADVECTION_SPEED_SCALE` (1.65) now feeds low, mid and cirrus shape noise,
weather advection, shadow taps and cirrus attenuation. Average drift is 65%
faster without changing the wind direction or weather field.

Cloud self-shadowing is slightly deeper (`local_scale` 1.08 / 0.90 / 0.58) with
cooler ambient fill and a stronger low-sun silver rim. The composite filter now
uses five depth-aware taps instead of a 3×3 kernel and blends only 40% toward
the smoothed result, preserving fair-weather silhouettes. The default cloud
detail is High (3× divisor); Ultra (2×) and Balanced (4×) remain available in
Graphics. Cloud images also keep at least 360 rows, so a 720p window no longer
ray-marches a 240-line cloud layer.

## Distant terrain X-ray

The previous world shader discarded resident terrain beyond
`backdrop_start + seam` and faded still-valid terrain toward sky. When streaming,
a remesh, a GPU-budget eviction or a horizon page lagged behind the camera, that
band became transparent ground: the sky and clouds were visible through it.

The fix separates the two responsibilities:

- Resident terrain is never discarded because it is past the backdrop handoff.
- The heightfield is a distant, sunk, depth-tested fallback. It is no longer
  discarded across the entire inner coverage; it begins at a fixed 96 m near
  clear, so it can fill a transient hole without appearing as a wall beside the
  player.
- `terrain_streaming_fade` returns zero. Valid ground never blends to sky. The
  depth-tested fallback fills a genuinely missing chunk instead.

Foliage proxies still begin near the handoff so they do not double resident tree
crowns.

## Surface wayfinding and worldbuilding

`world::surface` is a deterministic generator-18 layer sampled after authored
sites. It connects the spawn clearing to the starter workshop and up to three
nearby ruins with meandering gravel/cobblestone paths, then adds:

- roadside cairns, camps, paired-stone wayshrines, and overlook benches,
- one enterable stone nook cut into a bank with a short spur path,
- torch/crystal markers that make the route legible from a distance,
- a vegetation clearing pass so trunks and bushes do not grow through paths,
- additive location names (`The surveyor's trace`, `Wayfarer's rest`, and so on)
  that yield to authored sites, homesteads, dungeons, and the Undercrown.

The plan is cached per world configuration and sampled only in the top few
voxels. Resident edits still win. Existing explored chunks keep their saved
geometry; unedited/new chunks receive the trails.

## Inventory throw gesture

A stack can be picked up and released outside the inventory panel to throw it.
Releasing over the panel (recipe book, tabs, equipment) keeps the stack on the
cursor, so browsing cannot discard items. While the pointer is outside the panel
with a carried stack the cursor shows `RELEASE TO THROW`. Thrown stacks land to
the side of the aim ray and can be retrieved with `F`.

## First wand

Fresh worlds now start with a seeded Lumen Twig, and loading a pre-wand save
grants the same focus with a control hint (`R` cast, `T` cycle, `E → Gear`).
Later chassis upgrades still require the workbench or a Holy Mountain altar.

## Verification

`cargo check --offline --target-dir target` passed, including WGSL validation.
`cargo test --offline --target-dir target` produced **64 passed, 0 failed,
5 ignored**, including trail-plan, path-paving, nook-carving, drag-release,
and authored-waypoint regressions.

The final candidate was built separately because the normal executable was in
use. Reference-review captures at 1280×720 on an RTX 4090
(`target/visual-capture-native/OneMeter-noon.png`,
`target/visual-capture-hills-v2/OneMeter-noon.png`, and
`target/visual-capture-final3/OneMeter-noon.png`) rendered correctly through the
native resolve/present path. The final benchmark reports **197.15 fps /
5.07 ms**, with 0.72 ms terrain and 4.02 ms cloud raymarch at the enforced
360-line cloud minimum.

Staged binary: `target/release/minecraft_style_voxel_prototype.next.exe`.
Runtime visual acceptance still needs a longer in-game pass for cloud motion,
a dark cave, a lit surface close-up, a cast/ward sequence, and a walk along the
spawn trail. The hills reference capture contains an authored valley edge that
reads as a close backdrop wall; that is diagnostic scene geometry, not the
normal-world fallback.
