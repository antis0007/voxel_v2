# Localized Fauna Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Birds call from canopy and frogs from still water near the listener, with biome voices blended across borders and a distinct call family per biome.

**Architecture:** The 0.5 s ambience probe in `crates/app/src/soundscape/ambience.rs` gains a canopy and still-water measurement per compass sector. `soundscape/fauna.rs` turns five biome samples plus those measurements into a `Fauna` voice and a `Placement`. The audio thread's `Chorus` in `crates/audio/src/fauna.rs` places each call in the stereo field from `Placement` and the listener yaw, and renders one of four synth families.

**Tech Stack:** Rust 2024, `engine_audio` (no deps), `ron` tables in `assets/`, nextest via `just test <substring>`.

**Spec:** `docs/superpowers/specs/2026-11-03-ambience-fauna-planet-clouds-design.md`, Part B.

## Global Constraints

- No `unwrap`/`expect` outside tests; no `Instant` or `rand` in game code.
- `BED_COUNT` stays 6; no save-format bytes change.
- `layers_render_far_faster_than_real_time` stays under 25% of a core (debug).
- Canopy probe cost: at most 360 cells per 0.5 s probe (8 sectors × 3 rings × 15 cells).
- Biome border blend: 5 `landscape::sample` calls per probe, weights 0.4 centre, 0.15 per side, 16 m out.
- Voice families: `Whistle | Trill | Chatter | Caw`; breath onset 8 ms at 10%; at most 4 oscillators per call.
- Distance classes: near 8 m with probability `canopy[d] * 0.3`, else far 20 m.
- Text and comment rules in CLAUDE.md apply to doc comments and the ron file.
- Shared tree: other sessions edit it. Commit only the files this plan names, via a temp index.

## Review Focus

- Fine voxel scale (0.25 m): canopy probe must still reach 12 m up, 24 m out; pinned in Task 1 by the quarter-scale test.
- Biome name missing from `biome_sounds.ron` at one of the five sample points: blend must not zero the whole voice; pinned in Task 3.
- `Placement` all zero while a call is mid-phrase: the call finishes, no new one starts; pinned in Task 4.
- Listener spins fast: pan follows yaw per frame without zipper noise; pinned in Task 4 by the finite-and-smooth test.
- Rain during a Caw call: level fades, synth stays finite; covered by the existing fade path, asserted in Task 5 RMS test.

---

### Task 1: Canopy and still-water probe

**Files:**
- Modify: `crates/app/src/soundscape/ambience.rs` (struct `Surroundings`, fn `probe_flow`, tests)

**Interfaces:**
- Produces: `Surroundings.canopy: [f32; 8]` (per `DIRECTIONS` sector, 0..1) and `Surroundings.still_water: [f32; 8]` (per sector, 0..1; water found in the rings with motion below 0.2).
- Constants: `CANOPY_RINGS_M: [f32; 3] = [6.0, 14.0, 24.0]`, weights `[1.0, 0.6, 0.35]`, column scan from 2 m below to 12 m above the eye, `canopy[d] = min(sum / 2.0, 1.0)`.

- [ ] **Step 1: Write the failing tests** in the `tests` module of `ambience.rs`:

```rust
/// Open field with a leaf block cluster 10 m to +x, 3 to 8 m up.
fn grove(p: IVec3) -> BlockId {
    if (8..=12).contains(&p.x) && (3..=8).contains(&p.y) && p.z.abs() <= 2 { BlockId::Leaves } else { open_field(p) }
}

#[test]
fn canopy_is_found_in_its_sector_only() {
    let s = probe(grove, EYE, 1.0, 0.0);
    assert!(s.canopy[0] > 0.3, "+x sector {}", s.canopy[0]);
    assert!(s.canopy[4] == 0.0 && s.canopy[2] == 0.0);
    assert_eq!(probe(open_field, EYE, 1.0, 0.0).canopy, [0.0; 8]);
}

#[test]
fn canopy_survives_fine_voxels() {
    let quarter = probe(|p| grove(IVec3::new(p.x.div_euclid(4), p.y.div_euclid(4), p.z.div_euclid(4))), EYE * 4, 0.25, 0.0);
    let metre = probe(grove, EYE, 1.0, 0.0);
    assert!((quarter.canopy[0] - metre.canopy[0]).abs() < 0.3);
}

#[test]
fn still_water_is_per_sector_and_ignores_falls() {
    let s = probe(lakeshore, EYE, 1.0, 0.0);
    assert!(s.still_water[0] > 0.0 && s.still_water[4] == 0.0);
    let fall = |p: IVec3| if p.x == 6 && p.y == 2 { BlockId::Water } else { open_field(p) };
    assert_eq!(probe(fall, EYE, 1.0, 0.0).still_water, [0.0; 8]);
}
```

- [ ] **Step 2: Run** `just test canopy` and `just test still_water`. Expected: compile failure, no `canopy` field.

- [ ] **Step 3: Implement** the two fields in `probe_flow`. The water ring walk already visits each sector; record `weight * (1.0 - motion_share)` into `still_water[d]` where `motion_share` is the per-hit motion term (falling = 1, else speed/1.5) when it is below 0.2. Add the canopy loop after it, same `stride`/`cells` helpers. `probe()` passes through.

- [ ] **Step 4: Run** `just test soundscape`. Expected: all pass, including the existing probe tests.

- [ ] **Step 5: Commit** `crates/app/src/soundscape/ambience.rs` with message `feat(soundscape): canopy and still-water probe per sector`.

### Task 2: Placement type and family in the audio crate

**Files:**
- Modify: `crates/audio/src/fauna.rs` (struct `Fauna`, new `Placement`, new `Family`)
- Modify: `crates/audio/src/lib.rs` (export `Placement`, `Family`)
- Modify: `assets/biome_sounds.ron` (add `family` per row)
- Modify: `crates/app/src/soundscape/fauna.rs` test `table_parses_and_covers_every_biome`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Family { #[default] Whistle, Trill, Chatter, Caw }

pub struct Fauna { ..existing, pub family: Family }

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    pub canopy: [f32; 8],
    pub water: [f32; 8],
    /// Listener yaw in turns, 0 facing -z, so sector bearings pan correctly.
    pub yaw: f32,
}
```

- `Fauna::mix` leaves `family` as `self.family` when `t < 0.5`, else `other.family`.

- [ ] **Step 1: Write the failing test** in `crates/audio/src/fauna.rs`:

```rust
#[test]
fn mix_keeps_the_dominant_family() {
    let a = Fauna { family: Family::Caw, ..MEADOW };
    let b = Fauna { family: Family::Trill, ..MEADOW };
    assert_eq!(a.mix(b, 0.3).family, Family::Caw);
    assert_eq!(a.mix(b, 0.7).family, Family::Trill);
}
```

and in `crates/app/src/soundscape/fauna.rs` extend `table_parses_and_covers_every_biome` with `assert!(voices.values().any(|v| v.family == Family::Caw))`.

- [ ] **Step 2: Run** `just test mix_keeps` → compile failure (no `family`).

- [ ] **Step 3: Implement** `Family`, the field, `Placement`, the exports; fill `family` in every row of `biome_sounds.ron`: Caw for Badlands, Desert, Tundra, Ocean, LastShore; Chatter for Rainforest, CloudForest, OvergrownGardens, Savanna; Trill for Meadow, Heathland, Marsh, Fen; Whistle for the rest.

- [ ] **Step 4: Run** `just test fauna`. Expected: pass.

- [ ] **Step 5: Commit** the four files, message `feat(audio): fauna family and placement types`.

### Task 3: Biome blend and placement at the listener

**Files:**
- Modify: `crates/app/src/soundscape/fauna.rs`
- Modify: `crates/app/src/soundscape.rs:126-133` (the `set_fauna` call)

**Interfaces:**
- Consumes: Task 1 fields, Task 2 types.
- Produces in `soundscape/fauna.rs`:

```rust
/// Blends the voices at the eye and four points 16 m out, 0.4 centre and 0.15 each.
pub fn blended(names: [&str; 5]) -> Fauna;
/// Voice and placement for the blended biome under the listener's conditions.
pub fn voice(base: Fauna, at: &Listening, around: &Surroundings, yaw: f32) -> (Fauna, Placement);
```

- Rules: `birds = base.birds * day * (1 - 0.8 rain) * open * canopy.max()`, `frogs = base.frogs * (0.25 + 0.75 (1 - day)) * open * still_water.max()`. A name missing from the table contributes `Fauna::default()` with its weight, and the centre's `family` wins.

- [ ] **Step 1: Write the failing tests** in `soundscape/fauna.rs`:

```rust
#[test]
fn a_forest_over_bare_ground_has_no_birds() {
    let base = blended(["Forest"; 5]);
    let (v, p) = voice(base, &DAY, &Surroundings::default(), 0.0);
    assert_eq!(v.birds, 0.0);
    assert_eq!(p.canopy, [0.0; 8]);
    let mut s = Surroundings::default();
    s.canopy[3] = 0.8;
    let (v, p) = voice(base, &DAY, &s, 0.25);
    assert!(v.birds > 0.5 && p.canopy[3] == 0.8 && p.yaw == 0.25);
}

#[test]
fn borders_blend_toward_the_mean_and_tolerate_unknown_names() {
    let mid = blended(["Desert", "Rainforest", "Rainforest", "Desert", "Desert"]);
    let (d, r) = (blended(["Desert"; 5]), blended(["Rainforest"; 5]));
    assert!(mid.calls_per_s > d.calls_per_s && mid.calls_per_s < r.calls_per_s);
    assert_eq!(mid.family, d.family);
    let partial = blended(["Forest", "Nowhere", "Nowhere", "Nowhere", "Nowhere"]);
    assert!((partial.birds - 0.4 * blended(["Forest"; 5]).birds).abs() < 1e-4);
}
```

- [ ] **Step 2: Run** `just test soundscape::fauna` → compile failure.

- [ ] **Step 3: Implement** `blended` and `voice`; replace the old `voice(&str, &Listening)`. In `soundscape.rs` sample `landscape::sample` at the eye and at `eye ± 16 m` on x and z (voxel units: `16.0 / meters`), format each `Debug` name, call `blended`, then `voice(.., yaw)` with `yaw = player.forward_dir()` turned into turns: `(-forward.x).atan2(-forward.z) / TAU`. Pass the pair to a new `audio.set_fauna(voice, placement)` (Task 4 changes the signature; until then store placement in a local and keep the old call compiling by sending only the voice).

- [ ] **Step 4: Run** `just test soundscape`. Expected: pass.

- [ ] **Step 5: Commit** `soundscape/fauna.rs`, `soundscape.rs`, message `feat(soundscape): biome blend and canopy placement for fauna`.

### Task 4: Positioned calls in the chorus

**Files:**
- Modify: `crates/audio/src/fauna.rs` (`Chorus`, `Call`, `Croak`)
- Modify: `crates/audio/src/layers.rs` (`Command::Fauna`, `set_fauna`, new yaw atomic)

**Interfaces:**
- Consumes: `Placement`, `Family`.
- Produces: `Audio::set_fauna(&self, voice: Fauna, placement: Placement)`; `Audio::set_listener_yaw(&self, turns: f32)` (atomic `YAW` in `layers.rs`, stored every frame from `Soundscape::tick`); `Chorus::set(&mut self, voice: Fauna, placement: Placement)`; `Chorus::yaw(&mut self, turns: f32)` called from `Layers::begin`.
- Sector bearing in turns: `sector / 8.0` for `DIRECTIONS` order (index 0 = +x = east = 0.25 turns clockwise from -z); pan = `sin((bearing - yaw) * TAU)` clamped to ±0.9.
- Near class gain 0.06..0.09, far class 0.02..0.04. A frog croak pans to a sector drawn from `placement.water`; all zero means no croak starts.

- [ ] **Step 1: Write the failing tests** in `crates/audio/src/fauna.rs`:

```rust
#[test]
fn calls_come_from_the_canopy_sector() {
    let mut p = Placement::default();
    p.canopy[0] = 1.0; // east
    let mut chorus = Chorus::new(48_000);
    chorus.set(MEADOW, p);
    chorus.level = [1.0, 0.0];
    chorus.yaw(0.0); // facing north: east is on the right
    let (mut l, mut r) = (0.0f64, 0.0f64);
    for _ in 0..48_000 * 6 { let [a, b] = chorus.frame(); l += (a * a) as f64; r += (b * b) as f64; }
    assert!(r > l * 4.0, "left {l} right {r}");
    chorus.yaw(0.5); // facing south: east is on the left
    let (mut l, mut r) = (0.0f64, 0.0f64);
    for _ in 0..48_000 * 6 { let [a, b] = chorus.frame(); l += (a * a) as f64; r += (b * b) as f64; }
    assert!(l > r * 4.0, "left {l} right {r}");
}

#[test]
fn no_canopy_means_no_new_calls_and_a_phrase_still_ends() {
    let mut chorus = Chorus::new(48_000);
    chorus.set(MEADOW, Placement { canopy: [1.0; 8], ..Default::default() });
    chorus.level = [1.0, 0.0];
    for _ in 0..48_000 { chorus.frame(); }
    chorus.set(MEADOW, Placement::default());
    let mut tail = 0.0f32;
    for i in 0..48_000 * 4 { let [a, _] = chorus.frame(); if i > 48_000 * 2 { tail += a.abs(); } }
    assert_eq!(tail, 0.0);
}

#[test]
fn yaw_changes_pan_without_steps() {
    let mut p = Placement::default();
    p.canopy[2] = 1.0;
    let mut chorus = Chorus::new(48_000);
    chorus.set(MEADOW, p);
    chorus.level = [1.0, 0.0];
    let mut last = [0.0f32; 2];
    let mut worst = 0.0f32;
    for i in 0..48_000 * 4 {
        chorus.yaw(i as f32 / 48_000.0); // one turn per second
        let s = chorus.frame();
        assert!(s[0].is_finite() && s[1].is_finite());
        worst = worst.max((s[0] - last[0]).abs()).max((s[1] - last[1]).abs());
        last = s;
    }
    assert!(worst < 0.05, "step {worst}");
}
```

- [ ] **Step 2: Run** `just test calls_come_from` → compile failure.

- [ ] **Step 3: Implement.** `Call` and `Croak` store `bearing` instead of `pan`; pan is computed per frame from `bearing - yaw` with the yaw smoothed by a one-pole (time constant 50 ms). Sector draw: cumulative weights over `placement.canopy`, `noise.unit()` picks. Near/far class per spec. `Command::Fauna(Fauna, Placement)`. `YAW` atomic with `store`/load like `SPACE`.

- [ ] **Step 4: Run** `just test fauna` and `just test layers_render`. Expected: pass, layers share printed under 25%.

- [ ] **Step 5: Wire** `soundscape.rs`: call `audio.set_fauna(voice, placement)` in the probe block and `audio.set_listener_yaw(yaw)` every tick. Run `just test soundscape`.

- [ ] **Step 6: Commit** `crates/audio/src/fauna.rs`, `crates/audio/src/layers.rs`, `crates/app/src/soundscape.rs`, message `feat(audio): bird and frog calls placed by canopy and water sector`.

### Task 5: Voice families

**Files:**
- Create: `crates/audio/src/fauna/families.rs` (synth per family, under 300 lines)
- Modify: `crates/audio/src/fauna.rs` (`mod families;`, call rendering delegates)
- Modify: `crates/audio/src/review.rs` (`catalogue()` adds one row per family)

**Interfaces:**
- Produces in `families.rs`:

```rust
pub(super) struct Voice { /* per-call oscillator state, at most 4 phases + one Svf */ }
impl Voice {
    pub(super) fn start(family: Family, hz: f32, noise: &mut Noise, rate: f32) -> Voice;
    /// One mono sample of the note at `t` in 0..1 of its length.
    pub(super) fn sample(&mut self, family: Family, t: f32, trill: f32, rate: f32) -> f32;
}
```

- Family rules from the spec: Whistle = falling sine, sweep 0.1..0.3, note 120..260 ms; Trill = AM at 40..60 Hz, note 40..120 ms, 3..6 notes; Chatter = carrier + ring-modulated partial at 1.5×, note 30..70 ms, random pitch steps ±15% per note; Caw = white noise through `Svf::band` tuned 1.2..1.8 kHz, Q 6, one note 250..450 ms. Every family: breath noise onset 8 ms at 10% of call gain.

- [ ] **Step 1: Write the failing tests** in `crates/audio/src/fauna.rs`:

```rust
#[test]
fn every_family_renders_in_the_chorus_band() {
    for family in [Family::Whistle, Family::Trill, Family::Chatter, Family::Caw] {
        let v = rms(Fauna { family, ..MEADOW });
        assert!((0.002..0.08).contains(&v), "{family:?} {v}");
    }
}

#[test]
fn caw_is_noisier_than_whistle() {
    // Zero-crossing rate over the same call length separates a band-passed noise from a sine.
    assert!(crossings(Family::Caw) > crossings(Family::Whistle) * 2);
}
```

with a `crossings(family) -> usize` helper that renders 8 s with `canopy = [1.0; 8]` and counts sign changes on the left channel. Update `rms` to pass a full-canopy `Placement`.

- [ ] **Step 2: Run** `just test every_family` → fails (families render alike or the file is missing).

- [ ] **Step 3: Implement** `families.rs`; `Chorus::birds` builds a `Voice` at call start and calls `sample` per frame.

- [ ] **Step 4: Run** `just test fauna` and `just test layers_render`. Expected: pass.

- [ ] **Step 5: Add** the four family rows to `review::catalogue()` and run `cargo test -p engine_audio dump_audio_wav -- --ignored` to write the WAVs; listen to the four files. Record the paths in the commit message.

- [ ] **Step 6: Commit** the three files, message `feat(audio): whistle, trill, chatter and caw bird families`.

### Task 6: Verification and gate

- [ ] **Step 1: Run** `just test audio`, `just test soundscape`, `just clippy`. Expected: pass, clippy warning count unchanged from the tree before Task 1 (record both).
- [ ] **Step 2: Run** `python -I scripts/workflow.py verify`; attest the WAV listening check with `python -I scripts/workflow.py attest`.
- [ ] **Step 3: Commit** nothing new; report the commit list and the WAV paths.
