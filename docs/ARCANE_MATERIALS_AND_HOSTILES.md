# Arcane materials and hostile presentation

This pass adds saved magic profiles and replaces four weak common-enemy rigs.
It preserves combat rules and the boss, navigation, lighting and horizon owners'
work. Source/build proof and native visual acceptance are recorded separately.

## Magic controls

Settings -> Magic materials contains independent Enchantment, Instability and
Curse profiles. Each has three sRGB swatches, strength, speed, ribbon width,
chromatic edge, ribbon density, direction and sparkle. Roseglass, Witchlight,
Astral and per-profile Reset are available alongside live item previews.
Preferences format 008 appends the profiles and reduced-motion setting; formats
001 through 007 retain their previous values and receive default magic profiles.
The existing debounced atomic preferences writer handles persistence.

Icon shaders use native glyph-body masks and an eight-step-per-second clock.
Crossed pink/lilac/mint ribbons and occasional fixed-pixel glints retain the
original icon outline, material identity, count and label. One reusable dynamic
uniform buffer supports 256 effects; overflow keeps the original icon.

The Enchantment profile also feeds five appended vec4s in the world's Lighting
uniform, mirrored in both shared WGSL declarations. Swatches convert to linear
light for generic world shading. A newer direct user correction requires the
wand's hue to follow its actual selected spell metadata; generic swatches do not
override that hue. The world wand, crystal and Mithril share flow/strength controls
and keep their spell/material colours. Reduced motion freezes travelling patterns.
The existing crystal resonance breath is a separate intrinsic material effect.
There is no added fullscreen pass or texture allocation. World decorations use
the existing 20 texels/metre lattice, follow the viewmodel's camera coordinates,
and fade away below resolvable size. The first-person hand remains excluded.

Curse is a customizable presentation profile and labelled preview. This does
not introduce cursed-item gameplay metadata or grant affixes to ordinary gear.

## Common enemies

- Sanctum Warden: layered plate, slit visor, front-facing physical shield and a
  broad blade with guard. Actual guard-break state removes the shield. Wind-up,
  release and recovery control the weapon pose and attached rune charge.
- Burrow beetle: plated split shell, articulated six-leg gait and hinged pincers.
- Gravelback Colossus: planted heavy feet, layered shoulders/jaw and raised fists.
- Wisp: an angular floating cage, eye seam and dangling fragments.

These rigs reuse the existing distance/gait and 240 ms recovery state; they do
not invent attack timers. Material 59 is worn armour, 60 is corrupt runic metal.
Each part receives local UVs before joint rotation, so texels follow animation.
Runes charge from the real tell/recovery state. Hit/frost tint uses actual status.
Late melee wind-up gets a small direction chevron, without claiming a fictitious
damage radius. The shared warning triangle and other owners' ward/affix cues
are preserved. Hostile projectiles use directional voxel darts; player spells,
projectile velocity, collision, damage, cooldowns and enemy AI are unchanged.

## Native review

`tools/Review-Grimoire.ps1 -Screen magic` opens the settings fixture. Components
stays compact; the editor has its own scrollable review window. Review input is
isolated from desktop clicks. Normal gameplay input is unaffected.

`tools/Review-Enemies.ps1 -Mode poses` captures the four rigs, with optional
`-Kind 2|3|5|9` close views. Poses are explicitly synthetic animation samples.
`-Mode combat` uses actual AI, arming it when benchmark capture starts so startup
streaming cannot consume the encounter. It reports attack/recovery counts,
projectiles and player health. The disposable superflat arena is installed before
the benchmark snapshot, matching the existing boss-review setup. No saved world
is opened or modified. Physics/performance acceptance belongs to separate runs.

The first arena attempt installed edits during Adventure tick and stalled the
horizon readiness gate. Its logs remain diagnostic evidence. Moving installation
to benchmark startup and resetting the snapshot fixed the review: the v2 Warden
case completed with 358 resident chunks, no missing chunks and no GPU errors.
The pose inspection caught and corrected the blade's wind-up rotation direction.

Local release-fast builds and Naga validation passed through v3. No test suites
or hosted CI were added/run by this chat. Final uniform/profile integration and
native evidence are appended after their checks complete.

Final v4 passed release-fast compilation and composed Naga validation (28.69 s).
All four final native Vulkan / RTX 4090 cases completed: four-family poses,
single-Warden actual AI, rich component effects and live-clock shader settings.
The appended Lighting uniform is validated in the actual renderer. The combat
receipt records wind-up at 8.49 s, five projectiles at 9.25 s, recovery ending at
9.50 s and player health falling from 100 to 88 at 9.62 s. The pose fixture and
actual AI case remain explicitly distinct. Capture review also confirmed the
corrected raised blade, physical shield and directional hostile darts.

`review-artifacts/arcane-threats/review.json` preserves executable/source hashes,
capture paths and combat receipts. `rich-magic.gif` contains 31 actual frames;
`warden-combat.gif` contains 23. Crops retain native pixels. Full PNGs and logs
remain under the named v4 capture directories. Shared desktop GPU jobs were
active, so these establish visual integration, not a performance budget. Preference
save/reload was not exercised: benchmark mode disables preferences writes.
