# Space phase 5a: star system on rails (`engine_space`), implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A pure `engine_space` crate that loads a star system from RON and evaluates every body's position, velocity and spin orientation as an exact function of the integer simulation tick.

**Architecture:** Bodies compile from `assets/star_systems.ron` into a structure-of-arrays `System` stored in parent-first order, so one linear pass evaluates the whole hierarchy with no lag (SoA iterated a hash map and could read a parent's stale position). Orbits are Keplerian elements on rails: the mean anomaly comes from `tick mod period_ticks` in integers, so precision never decays over long saves; Kepler's equation is solved by Newton with a bracketed bisection fallback (SoA used three fixed steps). Barycentric binaries split semi-major axes by mass ratio. Spin uses tilt, north longitude and period, or tidal lock as data. No sim code reads this yet; the crate documents that f64 rails are for rendering and the map only (design 8a.4: sim-facing values will be fixed-point when a consumer exists).

**Tech Stack:** Rust 2024, glam 0.32 (`DVec3`, `DQuat`, `DMat3`), serde + ron 0.12 (already in the workspace), criterion bench.

**Spec:** [space track design](2026-10-24-soa-space-port-design.md) section 4.6 and phase 5 of section 6 (this plan is 5a: the crate; sky integration needs planet worlds and the map UI is 5b); [decision 0025](../decisions/0025-space-track-soa-port.md): system scale is provisional until the playable spike; proper nouns stay with the human.

## Global Constraints

- Rails are pure functions of data and tick; nothing about natural bodies is saved.
- Time: `t = tick * tick_seconds * time_scale`; phase from integer `tick mod period_ticks`, never from an accumulated float.
- Kepler solve: start `E0 = M + e·sin(M)` (or `π` when `e > 0.8`), Newton to `|ΔE| < 1e-14`, cap 16 iterations, bisection fallback on `[M - e, M + e]`; supports `e ∈ [0, 0.999]`.
- Position and velocity come from the same perifocal-to-system rotation.
- Evaluation order is topological (parents first), one pass.
- Tidal lock is a data flag, not a heuristic.
- System frame: right-handed, metres, `+Y` is system north; an orbit with inclination 0 has angular momentum along `+Y`.
- Data ids only, no proper nouns; the home system's numbers are marked provisional in the RON file.
- New crate depends only on glam, serde, ron (no new dependency to the workspace); no `unwrap`/`expect` outside tests; files under 300 lines; doc comments open with what the item does in under 12 words; comments state why or an invariant; no em-dash asides; banned words: seamless, robust, leverage, delve, elevate, comprehensive, crucial, ensure, streamline, powerful, elegant.
- Content is data: the system lives in `assets/star_systems.ron` with a round-trip test.

## Review Focus

1. Very long saves: tick `10^12` gives the same parent-relative position as tick `10^12 mod period_ticks` bit for bit (Task 3 test).
2. Near-parabolic orbits (`e = 0.999`) and `M` at `0`, `π`, just below `2π`: the solver converges or falls back, never returns NaN (Task 2 test).
3. RON listing children before parents: compiled order is still parent-first (Task 3 test).
4. Invalid data (unknown parent, cycle, duplicate id, binary with a missing component, `e >= 1`, non-positive period): a typed error, not a panic (Task 1 test).
5. A tidally locked moon keeps the same body-frame direction to its parent over a full orbit (Task 4 test).

---

### Task 1: Crate, data schema, validation and the home system file

**Files:**
- Create: `crates/space/Cargo.toml` (package `engine_space`; deps glam 0.32.0, serde 1.0.229 with derive, ron 0.12.2; `[lints] workspace = true`; a one-line header comment like other crates)
- Create: `crates/space/src/lib.rs`, `crates/space/src/data.rs` (schema), `crates/space/src/validate.rs`
- Create: `assets/star_systems.ron`
- Modify: `docs/ARCHITECTURE.md` (one line in the crate/location table for `engine_space`)
- Test: `crates/space/src/data.rs` / `validate.rs` test modules

**Interfaces:**
- Produces (serde types, all `Clone, Debug, PartialEq, Serialize, Deserialize`):
  - `pub struct SystemsFile { pub systems: Vec<SystemDef> }`
  - `pub struct SystemDef { pub id: String, pub time_scale: f64, pub bodies: Vec<BodyDef> }`
  - `pub struct BodyDef { pub id: String, pub kind: BodyKind, pub mass_kg: f64, pub radius_m: f64, pub parent: Option<String>, pub orbit: Option<OrbitDef>, pub binary: Option<BinaryDef>, pub rotation: Option<RotationDef> }`
  - `pub enum BodyKind { Star { temperature_k: f64 }, Planet, Moon, GasGiant, Barycenter }`
  - `pub struct OrbitDef { pub period_s: f64, pub e: f64, pub i_deg: f64, pub node_deg: f64, pub peri_deg: f64, pub mean_anomaly_deg: f64 }`
  - `pub struct BinaryDef { pub a: String, pub b: String, pub orbit: OrbitDef }` (on a `Barycenter`: its two components orbit it with these shared elements; `b` is at `peri_deg + 180`)
  - `pub enum RotationPeriod { Seconds(f64), Locked }`, `pub struct RotationDef { pub period: RotationPeriod, pub tilt_deg: f64, pub north_deg: f64 }`
  - `pub enum SystemError { DuplicateId(String), UnknownParent { body: String, parent: String }, Cycle(String), BadBinary(String), BadOrbit { body: String, reason: &'static str }, Parse(String) }` (`Display` + `std::error::Error`)
  - `pub fn parse(text: &str) -> Result<SystemsFile, SystemError>`; `pub fn validate(def: &SystemDef) -> Result<(), SystemError>`.

Rules: a body with `parent: None` is the root (exactly one per system). Components of a binary have `parent = Some(barycenter id)` and `orbit: None`. `e` must be in `[0, 0.999]`; `period_s > 0`; `mass_kg >= 0` (a barycenter's mass is derived, give `0.0`).

`assets/star_systems.ron`, one system `id: "home"`, header comment: "Provisional numbers until the system-scale spike (decision 0025). Ids only; names come from the human." Bodies: `star` (root, `Star`), `home` (planet around `star`; `radius_m: 20_371.0`; `rotation: Seconds(480.0)` to match the current 480 s day, tilt 23.4; orbit period 24 days of 480 s), `moon` (around `home`, `Locked`), `giant` (`GasGiant` around `star`). No binary in the home file; binaries are covered by test fixtures. Masses: `home` gives 9.81 m/s² surface gravity at `radius_m` (`M = g R² / G`, `G = 6.674_30e-11`); the builder computes the others for plausible periods and states the formula in a RON comment.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn home_file_parses_validates_and_round_trips() {
    let text = include_str!("../../../assets/star_systems.ron");
    let file = parse(text).unwrap();
    for s in &file.systems { validate(s).unwrap(); }
    let again = ron::ser::to_string_pretty(&file, Default::default()).unwrap();
    assert_eq!(parse(&again).unwrap(), file);
}

#[test]
fn invalid_systems_are_typed_errors() {
    // One fixture per variant, built in code from a valid two-body system:
    // duplicate id -> DuplicateId; parent "nowhere" -> UnknownParent; a <-> b parents -> Cycle;
    // barycenter whose binary names a missing body -> BadBinary; e = 1.0 -> BadOrbit; period 0 -> BadOrbit;
    // garbage text -> Parse. Each asserts the exact variant with matches!.
}
```

(The second body is described; the builder writes one assertion per listed case.)

- [ ] **Step 2: Run, expect FAIL**: compile error. Test command for every task in this plan: `just test <substring>` with a substring of the new test names (nextest filters by test path; the crate's module paths are `data::`, `kepler::`, `system_tests::`, `spin::`, `path::`), plus `cargo test -p engine_space` once before each commit to run the whole crate.
- [ ] **Step 3: Implement schema, `parse`, `validate`, the RON file and the ARCHITECTURE line.**
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `space: engine_space crate with star system schema and home system data`.

### Task 2: Kepler solver and orbital state

**Files:**
- Create: `crates/space/src/kepler.rs`
- Test: same file

**Interfaces:**
- Produces:
  - `pub const G: f64 = 6.674_30e-11;`
  - `pub fn solve_kepler(mean_anomaly: f64, e: f64) -> f64` (eccentric anomaly; `mean_anomaly` any real, reduced internally to `[0, 2π)`).
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct Elements { pub a: f64, pub e: f64, pub i: f64, pub node: f64, pub arg_peri: f64, pub mu: f64 }` (radians, metres, `mu` in m³/s² is `G·(M_parent + m)` for an ordinary orbit and `mu_pair·share³` for a binary component; `arg_peri` is the argument of periapsis, authored directly as `peri_deg`).
  - `impl Elements { pub fn from_period(period_s: f64, mu: f64, e: f64, i: f64, node: f64, arg_peri: f64) -> Elements; pub fn state_at(&self, mean_anomaly: f64) -> (DVec3, DVec3); pub fn mean_motion(&self) -> f64; }` (`a = (mu·T²/4π²)^(1/3)`; `state_at` returns parent-relative position (m) and velocity (m/s) in the system frame).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn kepler_residual_is_tiny_across_eccentricities() {
    for e in [0.0, 0.1, 0.5, 0.9, 0.97, 0.999] {
        for k in 0..=720 {
            let m = k as f64 * std::f64::consts::TAU / 720.0 - 1e-12 * (k == 720) as i32 as f64;
            let ea = solve_kepler(m, e);
            assert!(ea.is_finite());
            let r = (ea - e * ea.sin() - m.rem_euclid(std::f64::consts::TAU)).rem_euclid(std::f64::consts::TAU);
            assert!(r.min(std::f64::consts::TAU - r) < 1e-12, "e={e} m={m}");
        }
    }
}

#[test]
fn state_obeys_vis_viva_and_constant_angular_momentum() {
    let el = Elements::from_period(86_400.0, 3.986e14, 0.3, 0.4, 1.1, 0.7);
    let h0 = { let (r, v) = el.state_at(0.0); r.cross(v) };
    for k in 0..360 {
        let (r, v) = el.state_at(k as f64 * std::f64::consts::TAU / 360.0);
        let vis = el.mu * (2.0 / r.length() - 1.0 / el.a);
        assert!((v.length_squared() - vis).abs() / vis < 1e-10);
        assert!((r.cross(v) - h0).length() / h0.length() < 1e-10);
    }
}

#[test]
fn zero_inclination_orbits_spin_about_plus_y() {
    let el = Elements::from_period(1000.0, 1e12, 0.2, 0.0, 0.3, 0.5);
    let (r, v) = el.state_at(1.0);
    let h = r.cross(v).normalize();
    assert!((h - DVec3::Y).length() < 1e-12);
}

#[test]
fn velocity_matches_position_derivative() {
    // central difference of state_at over dM = n·dt (dt = 1e-3 s) equals velocity within 1e-6 relative
}

#[test]
fn full_period_returns_to_start() { /* state_at(M) == state_at(M + TAU) within 1e-9 relative */ }
```

- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `space: Kepler solver and orbital state from elements`.

### Task 3: Compiled system and tick evaluation

**Files:**
- Create: `crates/space/src/system.rs`
- Test: `crates/space/src/system_tests.rs` (declared `#[cfg(test)] mod system_tests;`)

**Interfaces:**
- Consumes: `SystemDef`, `validate` (Task 1); `Elements`, `solve_kepler`, `G` (Task 2).
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct BodyId(pub u16);`
  - `pub struct System` (fields private, structure of arrays in parent-first order: ids, kind, mass, radius, parent `Option<BodyId>`, `Option<Elements>`, `period_ticks: u64` per orbit, `mean_anomaly0`, rotation data).
  - `impl System { pub fn compile(def: &SystemDef, tick_seconds: f64) -> Result<System, SystemError>; pub fn len(&self) -> usize; pub fn id(&self, name: &str) -> Option<BodyId>; pub fn name(&self, b: BodyId) -> &str; pub fn parent(&self, b: BodyId) -> Option<BodyId>; pub fn mass(&self, b: BodyId) -> f64; pub fn radius(&self, b: BodyId) -> f64; pub fn evaluate(&self, tick: u64, out: &mut Snapshot); pub fn surface_gravity(&self, b: BodyId) -> f64; }`
  - `#[derive(Clone, Debug, Default)] pub struct Snapshot { pub position: Vec<DVec3>, pub velocity: Vec<DVec3> }` indexed by `BodyId.0`, system frame, root at the origin. `evaluate` resizes `out` and reuses its storage (no allocation after the first call).

Rules:
- `period_ticks = max(1, round(period_s / (tick_seconds · time_scale)))`; mean anomaly at `tick` is `M0 + TAU · ((tick % period_ticks) as f64 / period_ticks as f64)`.
- `mu` for a normal orbit: `G · (M_parent + m)`. Binary components: with `M = m_a + m_b`, the pair's relative orbit uses `mu = G·M`; component `a` uses semi-major axis `a_rel · m_b / M` and component `b` uses `a_rel · m_a / M`, with `b`'s periapsis rotated by `π`. The barycenter's mass is `M`, and its own orbit (if any) uses that mass.
- Position of a body = parent position + parent-relative state; velocity likewise. Parents always precede children in storage.

- [ ] **Step 1: Write the failing tests** in `system_tests.rs`:

```rust
#[test]
fn home_system_compiles_parent_first() { /* compile the asset; for every body with a parent, parent index < body index */ }

#[test]
fn child_listed_before_parent_still_compiles_parent_first() { /* fixture with moon first in the Vec */ }

#[test]
fn moon_relative_position_matches_its_elements_same_tick() {
    // snapshot.position[moon] - snapshot.position[planet] == moon elements state_at(M(tick)) within 1e-6 m
}

#[test]
fn long_saves_do_not_lose_phase() {
    // for each orbiting body: relative position at tick 10^12 equals relative position at
    // 10^12 % period_ticks exactly (==), using a fixture whose periods are not divisors of 10^12
}

#[test]
fn binary_keeps_its_barycenter_fixed_and_momentum_zero() {
    // fixture: barycenter root with components a (2e30 kg) and b (1e30 kg), period 1e7 s, e 0.3:
    // m_a r_a + m_b r_b == 0 and m_a v_a + m_b v_b == 0 (relative to barycenter) within 1e-9 relative, 50 ticks spread over a period
}

#[test]
fn home_planet_surface_gravity_is_provisional_target() {
    // surface_gravity(home) within 1% of 9.81
}

#[test]
fn evaluate_reuses_snapshot_storage() { /* capacity pointer unchanged across two evaluate calls */ }
```

- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `space: compiled star system evaluated per tick in parent-first order`.

### Task 4: Spin, tidal lock and orbit paths for the map

**Files:**
- Create: `crates/space/src/spin.rs`, `crates/space/src/path.rs`
- Test: same files

**Interfaces:**
- Consumes: `System`, `Snapshot`, `BodyId` (Task 3).
- Produces:
  - `impl System { pub fn orientation(&self, b: BodyId, tick: u64) -> DQuat; }`: body frame to system frame. Axis = `+Y` tilted by `tilt` toward the direction given by `north_deg` (longitude in the XZ plane measured from `+X` toward `-Z`); spin angle `TAU · (tick % spin_ticks) / spin_ticks` about that axis. `Locked`: `spin_ticks = period_ticks` of its own orbit and the spin phase is chosen so the body-frame `+X` axis points at the parent at tick 0.
  - `pub fn orbit_path(system: &System, b: BodyId, max_chord_error_m: f64, out: &mut Vec<DVec3>)`: closed polyline of the parent-relative orbit, sampled uniformly in eccentric anomaly with the smallest segment count whose sagitta (max distance from the true ellipse) is `<= max_chord_error_m`, capped at 8192 points.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn spin_period_returns_to_identity_rotation() { /* orientation(tick) == orientation(tick + spin_ticks) within 1e-12 */ }

#[test]
fn locked_moon_faces_its_parent_all_orbit() {
    // circular locked moon: inverse(orientation) * (parent - moon).normalize() == +X within 1e-9 at 64 ticks over a period
}

#[test]
fn tilt_and_north_set_the_spin_axis() { /* orientation(..) * Y == expected axis for tilt 23.4, north 30 */ }

#[test]
fn orbit_path_stays_within_chord_error() {
    // e = 0.9 orbit, max_chord_error 1e-3 * a: every segment midpoint's distance to the ellipse <= 1.05x the bound; point count < 8192
}
```

- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `space: body spin, tidal lock and map orbit paths`.

### Task 5: Bench and crate docs

**Files:**
- Create: `crates/space/benches/rails.rs` (criterion dev-dependency as in `crates/sim/Cargo.toml`; `[[bench]] name = "rails" harness = false`)
- Modify: `crates/space/src/lib.rs` (module doc: what the crate is, the f64-for-rendering rule, the tick rule)

- [ ] **Step 1: Bench** `evaluate` for a 64-body fixture system (parent chains of depth 3) and `solve_kepler` over 10^5 `(M, e)` pairs. Run `just bench rails` (or the crate-scoped equivalent the justfile supports); record ns per body and ns per solve in the report. Do not run `just bench-save`.
- [ ] **Step 2: `just verify`** (bench-check may be red only on `engine_worldgen/terrain_generate_8_chunks`, a stale baseline outside this plan: report it as-is).
- [ ] **Step 3: Commit** `space: rails bench and crate documentation`.
