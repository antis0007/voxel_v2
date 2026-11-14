# Enemy Perception and Stealth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Replace instant 360° enemy sight with a readable awareness meter, hearing, search and sneak attacks.

**Architecture:** New `crates/app/src/world/adventure/perception.rs` owns all perception math and the
marker; `adventure.rs` calls it from the existing sight check, chase-goal choice, damage sites and
enemy draw. Noise events are a per-tick `Vec<Noise>` on `Adventure`.

**Tech Stack:** Rust, glam, existing `Model` drawing helpers.

**Spec:** `docs/superpowers/specs/2026-11-14-enemy-perception-stealth-design.md`

## Global Constraints

- No save change; new `Enemy` fields are transient (`#[cfg_attr(feature = "multiplayer", serde(default))]`).
- No wall-clock time, no RNG in simulation; hashes from site/slot/step only.
- No unwrap/expect; no new dependencies; no tests added or run (owner default); no `cargo fmt`.
- Bosses, wildlife (passive), burrowers, field foes keep their own paths.
- Shared tree: touch only the hunks listed; peers edit `adventure.rs` concurrently.

## Review Focus

- `visible_to_mobs == false` (creative/spectator): awareness must stay 0, no markers.
- Multiplayer replicas: perception only runs where the existing AI loop runs (owner peer).
- Concealed rubble lurkers: marker hidden, disguise unchanged until wake.
- Enemy at fine voxel scales: all distances are metres (feet = player.pos * scale).
- Already-chasing enemy behind the player keeps tracking while line of sight holds.

---

### Task 1: perception module + sight hook

**Files:** Create `crates/app/src/world/adventure/perception.rs`; modify `adventure.rs` (module list,
`Enemy` fields + `Enemy::new`, sight check near line 2149).

**Produces:**
- `pub(super) struct Senses { pub light: f32, pub crouched: bool, pub still: bool, pub sprinting: bool, pub casting: bool }`
- `pub(super) fn senses(adv: &mut Adventure, world: &World, player: &Player, eye: Vec3) -> Senses`
- `pub(super) fn see(e: &mut Enemy, senses: &Senses, eye: Vec3, feet: Vec3, range: f32, los: bool, check_dt: f32) -> bool` — returns true on the spotted transition.
- `pub(super) fn decay(e: &mut Enemy, dt: f32)`
- constants from the spec (rate 1.7, cone 1/0.55/0.2, point-blank 2.2, decay 0.22, thresholds 0.3/1.0).

- [ ] Add fields `aware, investigate, search, search_step, spotted_flash` to `Enemy`.
- [ ] Replace the binary visible assignment with `perception::see`; bosses set aware = 1.
- [ ] Warning cue on spotted transition.
- [ ] `cargo check -p app` clean.

### Task 2: hearing

**Produces:** `pub(super) struct Noise { pub pos: Vec3, pub radius: f32 }`, `Adventure::noise(pos, radius)`,
`perception::hear(e, &[Noise], world)`.

- [ ] Sources: footsteps (walk 3 m / sprint 7 m on a 0.35 s cadence, none crouched), landing 8 m,
      casting 10 m, `make_noise` (block break) 8 m, melee impact 8 m, projectile impact 9 m.
- [ ] Gain `0.25 + 0.6*(1-d/r)` (x0.55 radius through walls), cap 0.95, set `investigate`.
- [ ] Rubble ambushers: replace the fixed opportunity with awareness (cone 1, point-blank 1.6 m); noise feeds it.
- [ ] `cargo check -p app` clean.

### Task 3: suspicious / search behaviour

**Produces:** `perception::goal(e, home_wander: Vec3, time) -> Option<(Vec3, f32)>` (destination, speed scale).

- [ ] Suspicious (0.3 ≤ aware < 1, not visible): go to `investigate` at 0.5 speed.
- [ ] On losing sight after spotted: last_seen then 3 hashed points ~3 m around it over ~7 s, then home.
- [ ] `cargo check -p app` clean.

### Task 4: sneak attack + marker

- [ ] `perception::unaware(e) -> bool` (aware < 0.3 && !visible && !boss).
- [ ] Melee x2 + stagger 0.6, spell x1.5; crit-style spark at the hit.
- [ ] `perception::marker(e, m, time)` drawn after `Enemy::model`; amber/searching/red per spec.
- [ ] `cargo check -p app` + `cargo clippy -p app` clean; commit only these hunks (temp-index recipe).
