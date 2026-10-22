# Grimoire interface and water repair

21 October 2026. Continues the supplied visual-direction request. Ownership in
this pass: egui UI and water-only shading. Material, lighting and horizon owners
are working in separate chats; preserve their changes in shared files.

## Component rules

`egui_ui/grimoire.rs` owns the original pixel fittings and palette. Dark recess
`#121519`, leather `#29221F`, gold `#654728/#B58A42/#F1D68A`, silver
`#4B5663/#AAB8C5/#EAF0EB`, parchment `#E6D8B8`, mana `#70C9CF`.

Combat and inventory slots use 18 logical pixels, a 14-pixel icon field, and
2-pixel gaps. The expanded wand deck uses 36-pixel cells. Selection has gold
rails, corner teeth and a lower notch; silver inset brackets indicate hover or
keyboard focus independently. Counts stay inside the cavity; hotbar bindings
have a reserved baseline below it. Cooldown masks stay inside spell cavities.
Expanded-panel fittings occupy margins: stepped corner plates, silver rivets,
engraved rails and stitching; inventory adds spine bands and a clasp. Combat
elements have individual recesses and floating status icons.

The first live HUD screenshot was rejected by the user: the full leather cover
and egui's 30-point row defaults inflated a sparse HUD into a large dashboard.
The compact revision removes that cover, uses explicit short rows, puts mana
beside the spell deck, aligns status to the hotbar width, and removes the
permanent hunger-threshold tick. Six-slot and eight-slot decks get a slightly
wider instrument row rather than a taller panel. Narrow windows reflow mana
below the slots. Slot numbers have small attached recesses for bright ground.

The 640x360 reference scales by whole physical pixels (2x/3x/4x at the requested
720p/1080p/1440p sizes). Native DPI is resolved once by `begin_frame`; the UI
size preference adjusts that integer scale. Other aspect ratios extend the
layout rather than stretch it. egui shape feathering is off, rect/line/text
rounding is on, and the existing nearest font/texture filtering stays enabled.
The existing renderer draws egui after world HDR resolve and world upscaling.

Mana, next-cast cost, readiness/recharge and spell order form one instrument.
The spell deck still calls the authoritative cast-block evaluator. The current
game has fixed R/T bindings; the HUD and input dispatch now share their key
constants in `player.rs`. This pass does not invent a rebinding subsystem.
The existing Spells button now opens the actual wand page (previously crafting).

## Water diagnosis

The fresh pre-change native view reproduces the reported slab tint, large loops
and bright horizontal boundary. `fract(voxel_pos.y)` painted a new meniscus at
every voxel height. Depth-contour foam and a synthetic caustic web covered still
water irrespective of agitation or receiver geometry. Crest scattering also
treated vertical water walls as steep wave crests.

Remove those unsupported effects, attenuate normal slopes, and restrict crest
scattering to the top. Native review then exposed a separate depth bug:
`GeneratedChunk::water_optics` capped every corner with fewer than four water
samples at 18 cm, including multi-cell walls adjoining AIR. Removing that cap
retains actual column depth; solid shoreline handling is preserved.
Existing material IDs, quantities, mesh boundaries, bed
colour/depth and Beer absorption remain authoritative inputs. This is a visual
repair, not a fluid-simulation improvement. Bed colour/parallax is an existing
approximation: no scene-depth refraction or accurate lateral volume thickness
is introduced. Actual submerged caustics and agitation-driven foam remain work.

## Native review

Build locally with the repository target directory; use `tools/Review-Grimoire.ps1`
with a unique label. `Screen` selects the real HUD, inventory, wand page, or an
in-engine component/state sheet. The underlying runner preserves seed, camera,
time, weather, render settings and executable hash, and never opens a saved world.

Before: `target/visual-direction/grimoire-before/water-1280-720`. This was a
desktop-contended visual baseline, not an uncontended performance acceptance run.
The proposed High budget remains 1440p p95 <=16.67ms. Source/build proof does not
establish that target, keyboard acceptance, or completion of all nine prompts.

The named compact-v4 candidate compiled and passed composed shader validation
using the existing `release-fast` profile (opt-level 3, no LTO). The existing release executable is
locked, so the review uses a frozen source copy and a named `grimoire_review`
binary in the SAME target directory. It does not replace the running game.

Native evidence:

- v2: actual HUD at 1280x720; used to identify remaining row and binding contrast issues.
- v3: HUD/water at 1920x1080 and 3440x1440, inventory at 1280x720 and 1920x1080,
  wand page at 1280x720 and 2560x1440, component states at 2560x1440. Images were
  inspected; inventory default grid columns were still 40px and left excessive gaps.
- v4: final HUD/water at 1920x1080 and corrected inventory at 1280x720, inspected
  again. Inventory columns now match its 18px cells. HUD crop is native pixels,
  without resampling. See `review-artifacts/grimoire/final-hud.png`,
  `final-water-hud.png` and `final-inventory.png`; manifests preserve the executable
  and frozen source hashes. Earlier faulty frames remain diagnostic evidence.

No tests or hosted CI were added/run in this chat. Local build/shader validation
and native screenshots establish this bounded UI/optical repair. These captures
were desktop/CPU/GPU-contended: their logs retain timings, but there is no clean
before/after performance claim or 1440p/60 acceptance. Actual motion, low-mana
gameplay, keyboard interaction, and larger UI preferences remain unverified.
Water still uses the existing bed-colour/parallax approximation; its exposed
sides are not true transparent volumes. The bright bands/web were removed, not
replaced with a complete refraction/foam simulation.

Other acceptance gates remain separate: the material, lighting and horizon chats
own their implementation/evidence; the coupled physics workshop and full
cross-resolution/performance program are not established by these UI edits.

## Reactive slots and magic materials (October 21)

`grimoire::react` lifts hovered/focused slots by one logical pixel, sinks held
buttons by one pixel, and gives selection/release a 120 ms recoil. The allocated
hit rectangle and drag target stay fixed. Hotbar, survival inventory, equipment,
wand deck and spell-library cards use this shared response. Library hover uses
silver corners; gold remains selection. Generic egui crossfades are disabled.

`magic_fx.rs` installs an egui-wgpu callback pipeline for `ui_magic.wgsl`, also
registered in the composed build validator. Original glyph body masks clip every
effect to occupied native sprite pixels; outlines, transparent space, labels and
counts retain their original paint. Time advances in eight steps per second:

- Enchantment: a quiet six-second diagonal silver/teal glint.
- Instability: broken ember seams for actual volatile/unstable modifiers 26/46.
- Curse: a downward violet stitch, exposed through `icons::paint_with_effect`.

Spells, the existing magical swords (Cinderbrand/Winterglass), wands and Lumen
equipment receive the enchantment presentation. Ordinary weapons/resources do
not. There is **no cursed-item gameplay field** in the current inventory. The
component sheet labels its curse tile as a sample; it does not grant affixes or
alter saves. Future actual curse metadata must explicitly choose that effect.
Wands have an original 14px diagonal wooden silhouette, gold fittings and
Lumen/frost/resonance gem colours. This replaces the unrelated spell glyphs
(including the healing plus previously used by the Frost wand).

The pipeline retains one dynamic uniform buffer (256 callbacks, aligned to the
device limit) and one bind group. Overflow retains the base icon. Effects stay
in the final native UI pass, inherit clipping, and never sample world space.
The settings checkbox "Reduce UI motion and magic shimmer" disables displacement
and freezes shader time; this preference currently lasts for the app session.

`Review-Grimoire.ps1 -Screen components -Animate -CaptureHz 8` injects native egui
pointer hover/press/release events only into the component sample, using its
previously rendered hit rectangle. It records phase transitions in stderr.
Normal gameplay never enables this input driver. Native review results are
recorded separately below; build validation alone does not establish motion feel
or a performance budget. No tests/hosted CI are introduced.

Native v8 review passed on Vulkan / RTX 4090: component sheet at 1280x720,
unobstructed gameplay HUD at 1920x1080. The 63-frame native capture recorded
hover/press/release events and outer frame tops at 342/344/346 physical pixels,
matching the intended -1/0/+1 logical-pixel poses at 2x scale. Both instability
and curse tiles visibly changed while their layout stayed fixed. Inspect
`review-artifacts/grimoire/magic-motion.gif`, `magic-components.png` and
`magic-hud.png`. GIF crops retain native pixels; source PNGs/logs/manifests are
in `target/visual-direction/magic-v8-*`. `magic-v8.json` identifies the frozen
executable and source archive. This verifies the component response, not full
keyboard/drag gameplay acceptance or a clean performance budget.

A newer parallel user request assigns permanent hotbar selection displacement,
stationary number tabs, improved numerals and adjustable/default hotbar sizing
to the "Improve hotbar selection and sizing" chat. Its follow-up owns those
specifics; v8 evidence predates that follow-up. Shared inventory/deck/library
response and magic materials remain this pass's scope.
