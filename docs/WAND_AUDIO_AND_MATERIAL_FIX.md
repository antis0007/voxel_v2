# First-person wand material and spell-audio repair

## Wand rendering

The wand body was using the **glow-particle material path** (`kind 40`) for the
shaft, grip bands, pommel and collars because those pieces were authored with a
small positive glow. That path adds rim/emissive shading and bypasses the solid
surface detail, producing the flat, broken colour bands visible in the
screenshot.

- The hand remains `kind 54` (skin/leather).
- Solid wand pieces now use `kind 55`, a dedicated wood/leather/wand material
  with fine grain and metre-scaled dark bands.
- Shaft, grip, pommel and collar glow values are zeroed; only the head, runes,
  trail, crystal shaft and cast flare remain emissive.
- The first-person rig is scaled around the grip (`VIEWMODEL_SCALE = 0.84`),
  moved farther right/down/forward, and shortened. The tip light follows the
  scaled tip.
- A unit test asserts the hand uses `kind 54`, the wand body uses `kind 55`, and
  no geometry extends beyond a readable first-person bound.

## Spell audio

- Rapid identical cues used to stack up to 24 copies of the same transient at
  full gain. The output now:
  - caps each cue at four voices by replacing the voice furthest into its
    sample (least likely to click),
  - normalizes equal-cue voices into one shared bus,
  - retains isolated cues at full level,
  - ducks repeats to 36% gain: the same cue within a per-cue window
    (`rapid_window_ms`: 70 ms for spark/frost/ward/swing, 110 ms for
    burst/bloom/splash, 55 ms otherwise), or any cue of the same spell family
    within 52 ms (`src/audio.rs`).
- Every cue now has **three decorrelated synthesized variants**. The render
  seed, tonal phase and noise are varied, so a held rapid-fire stream does not
  produce an identical waveform on every shot.
- Every generated cue is peak-normalized into a common range, soft-limited, and
  given a 12 ms cosine release so cues neither arrive wildly hotter than their
  neighbours nor click at the end of the buffer.
- The UI/sprite palette and generator rules are documented in
  `docs/PIXEL_ART_DIRECTION.md`; `assets/pixel_palette.txt` is the shared source
  for Rust and Python art tools.
- Each spell now selects a family cue and pitch through
  `world::adventure::spell_defs::spell_audio`: frost, burst, cinder, bloom, rift, echo, thunder/clang,
  treasure and spark.
- Projectile bounce impacts use the same family cue instead of always Spark.
- Event pitch is clamped and combined with the existing low-discrepancy rate
  jitter, so rapid casts vary without becoming chipmunked or distorted.
- A short damped ping-pong delay/reverb bus gives rifts, bursts, blooms and
  splash impacts space. Sparks and frost stay tight so rapid fire remains
  articulate rather than turning into a wash.
- Reverb feedback is damped and tested for decay, so it cannot ring forever.

## Verification

- `cargo check --offline --target-dir target` passes.
- Full Rust suite: **180 passed, 0 failed, 15 ignored** with
  `--test-threads=1`.
- `viewmodel_stays_inside_a_readable_first_person_bounds` passes, including the
  `kind 54` hand / `kind 55` wand-body assertions.
- `spell_families_do_not_share_one_projectile_cue`,
  `equal_cue_voices_share_one_normalized_bus`,
  `every_cue_has_three_distinct_variants`,
  `cues_are_normalized_declicked_and_finite` and
  `reverb_keeps_a_damped_tail_after_the_input_stops` pass.
- Optimized release build completed successfully; see the release binary path
  and hash in the build report.
