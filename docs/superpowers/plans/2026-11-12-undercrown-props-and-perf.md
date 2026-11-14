# Undercrown props visible + underground perf — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop interaction prompts from appearing over empty air, give every Undercrown prop kind a distinct voxel model with a visible plinth reward, and measure and fix the top underground frame cost.

**Architecture:**
- An app-side guard ships immediately, with no gate. A prompt appears only when the focused prop has solid voxels at its model box, and the focus aims at that box.
- Behind a new dormant generator gate, the uniform `F::Pedestal` is replaced by per-kind `F::PropModel` shapes, read from a RON table.
- The app draws the plinth's reward item above unclaimed plinths.
- Perf work starts from frames.csv captures in the review rooms.

**Tech Stack:** Rust, engine_worldgen (`crates/worldgen`), app (`crates/app`), RON assets, wgpu model path used by `held_art`.

**Spec:** `docs/superpowers/specs/2026-11-12-undercrown-overhaul-design.md` (slices 1–2; §1, §6, §9, §11).

## Global Constraints

- Never edit a historical gate. Generators < `UNDERCROWN_V3_GENERATOR` must sample byte-identically.
- Claim the gate number in `docs/worldgen/GENERATOR_GATES.md` before it appears in code. The gate is a named `pub const`, dormant (not `CURRENT_GENERATOR_VERSION`) until the owner approves the slice 3–4 looks.
- Content in RON (`assets/*.ron`), not Rust tables. No new dependencies. No unwrap/expect outside tests.
- No `cargo fmt` (no rustfmt.toml).
- Shared tree: commit only your own hunks (temp index / clean worktree recipe).
- Visual changes need A/B captures from the same `VOXEL_REVIEW_DUNGEON=undercrown:<room>` pose. Shading stays smooth (no posterisation).
- Player-visible text follows `crates/app/src/egui_ui/UI_RULES.md`. Canon terms only.
- No `Instant::now()` in simulation. Worldgen stays pure on `WorldSampleConfig`.

## Review Focus

1. **Old worlds with gen 19–31:** these props have no voxels, so after the guard their prompts disappear. Expected: no prompt, and no crash or mutation. Test in Task 1.
2. **Prop model carved away or submerged** by space carving, water or a mechanism at the new gate. Expected: the model still has at least one solid voxel. Test in Task 3 (every prop across seeds).
3. **Half-metre voxel scale.** Model boxes and focus must use metres → cells via `voxel_scale`. Test at `Scale::HalfMeter` in Tasks 1 and 3.
4. **Claimed plinth.** The reward display disappears and the prompt reads "Already claimed". The item must not reappear after a save/load. Test in Task 4.
5. **Aiming at the pedestal from below or at a steep angle** must still focus without a wall in between. Test in Task 1 with an eye 1 m below the target.

---

### Task 1: Prompt guard — focus only on a visible prop (no gate, ships now)

**Files:**
- Modify: `crates/app/src/world/undercrown/interact.rs:10-120` (focus target + visibility check)
- Modify: `crates/worldgen/src/undercrown/plan.rs` (add `Prop::model_box`)
- Test: `crates/app/src/world/undercrown/interact.rs` (tests module)

**Interfaces:**
- Produces: `impl Prop { pub fn model_box(self) -> (Vec3, Vec3) }`, in metres. The box is `pos - (0.6,1.8,0.6) .. pos + (0.6,-0.6,0.6)`, which matches today's pedestal. Task 3 redefines this per kind at the new gate through `Plan::model_box(&self, prop: &Prop) -> (Vec3, Vec3)`; define that wrapper now as returning `prop.model_box()`.
- Produces: `fn prop_visible(world: &World, lo: Vec3, hi: Vec3) -> bool` in interact.rs. It is true when any cell overlapping the box is solid. Convert with `world.voxel_scale.meters()`.

- [ ] **Step 1: Write failing tests**
  - `prompt_hidden_when_model_box_is_air`: superflat world, `Plan::fixture()`, no blocks at the prop. `focus_with_plan` returns `None`.
  - `prompt_shown_when_model_box_has_a_block`: set Stone at the cell of `model_box` centre. The prompt is `Some`.
  - `focus_from_below_still_hits`: eye 1 m below the box centre, aimed at it. `Some`.
  - `half_metre_scale_guard`: same as test 2 at `VoxelScalePreset::HalfMeter`.
  - Update the existing tests to stamp the pedestal block first (their eye/target math moves to the box centre).
- [ ] **Step 2:** Run `cargo test -p app undercrown::interact` and expect the new tests to FAIL.
- [ ] **Step 3: Implement.**
  - The focus target becomes the box centre (`(lo+hi)/2`, top-biased: `y = hi.y - 0.1`), replacing `pos + Y*FOCUS_HEIGHT`.
  - Skip a prop whose `prop_visible` is false.
  - The `trace::solid` line-of-sight check must ignore a hit whose cell lies inside the prop's own box. Otherwise the pedestal blocks itself.
  - `interact_with_plan` uses the same focus, so no mutation happens without a visible prop.
- [ ] **Step 4:** Run the same command and expect PASS. Also run `cargo test -p engine_worldgen undercrown`, which must PASS unchanged.
- [ ] **Step 5:** In-game check: load the screenshot world. "[G] Claim" appears only while aiming at the block. Capture before/after.
- [ ] **Step 6:** Commit `fix(undercrown): prompt only when the prop is visible`.

### Task 2: Claim the dormant gate

**Files:**
- Modify: `docs/worldgen/GENERATOR_GATES.md` (new row)
- Modify: `crates/voxel/src/config.rs:150-158` (const only; `CURRENT_GENERATOR_VERSION` is untouched)

**Interfaces:**
- Produces: `pub const UNDERCROWN_V3_GENERATOR: u32 = <next free>;`. 290 is the highest const on 2026-11-12. Grep every branch: `git grep -n "= 291;" $(git for-each-ref --format='%(refname)' refs/heads)`. Re-export it where `engine_worldgen` reads gates.

- [ ] **Step 1:** Grep branches and the gates doc for the candidate number. Pick the first free one.
- [ ] **Step 2:** Add the doc row "Undercrown v3: prop models, kits, history (dormant)" and the const.
- [ ] **Step 3:** `cargo check -p engine_voxel -p engine_worldgen` passes.
- [ ] **Step 4:** Commit `worldgen: claim dormant Undercrown v3 gate`.

### Task 3: Per-kind prop models at the gate

**Files:**
- Create: `assets/undercrown_props.ron`: per `PropKind`, a list of boxes `{ lo: (x,y,z), hi: (x,y,z), block: <BlockId name> }` in metres relative to `prop.pos`, plus a `model_box` for focus.
- Create: `crates/worldgen/src/undercrown/v2/prop_model.rs`: loads the RON with `include_str!` and parses it once (`OnceLock`), then samples cells.
- Modify: `crates/worldgen/src/undercrown/v2/layout.rs:38-46`: at `version >= UNDERCROWN_V3_GENERATOR`, push `F::PropModel { kind, yaw, style }` instead of `F::Pedestal`.
- Modify: `crates/worldgen/src/undercrown/v2/feature.rs:44,243`: add the variant and the sample arm.
- Modify: `crates/worldgen/src/undercrown/plan.rs`: `Plan::model_box` returns the RON `model_box` at the gate.
- Test: `crates/worldgen/src/undercrown/v2/tests_props.rs` (new; register it in `v2/mod.rs` in the same edit).

**Interfaces:**
- Consumes: `UNDERCROWN_V3_GENERATOR` (Task 2) and `Plan::model_box` (Task 1).
- Produces: `pub fn prop_block(kind: PropKind, yaw: f32, local: Vec3) -> Option<BlockId>`.

Models (authored voxel shapes; the style palette supplies the stone, and accents use existing blocks such as iron, brass/copper, chain, plank, glass and glow):

| Kind | Shape (≈ metres) |
|---|---|
| Plinth | stepped base 1.2² → 0.8² column 1.0 high, emissive cap |
| Cradle | 1.6 m open cage of iron bars, chain column to 3 m |
| Winch | 1.0 m drum between two posts, crank arm |
| SluiceGate | frame 2×2 m, plank leaf |
| BellClapper | chain + 0.6 m bulb |
| ObservatoryLens | 1.4 m ring on a mount |
| Marker | 0.8×1.2 plaque on a post |
| Counterweight | 1.2 m block on a chain |

- [ ] **Step 1: Write failing tests.**
  - `every_prop_has_solid_model_voxels_at_v3`: for 5 seeds × {OneMeter, HalfMeter}, every `plan.props` has at least one solid sampled cell inside `plan.model_box`.
  - `old_gates_unchanged`: `tests_pin` hashes for gates 32, 128 and 290 stay equal. Reuse the existing pin harness.
  - `props_ron_round_trips`.
- [ ] **Step 2:** Run `cargo test -p engine_worldgen undercrown::v2::tests_props` with `UC_GEN=<gate>` and expect FAIL.
- [ ] **Step 3:** Implement the RON loader, the feature variant and the layout switch. The model must win over carving and water inside its box: check feature precedence in `v2/mod.rs:191` and put `PropModel` first.
- [ ] **Step 4:** Run the tests and expect PASS. Run the full `cargo test -p engine_worldgen undercrown` with and without `UC_GEN`; everything passes.
- [ ] **Step 5:** Capture A/B at `undercrown:<room>` for the landing, the counterweight hall, the bridge village and the Bellkeeper plinth room, using `--generator-version <gate>`. Show the owner.
- [ ] **Step 6:** Commit `worldgen(undercrown): per-kind prop models behind dormant v3 gate`.

### Task 4: Visible plinth reward (app)

**Files:**
- Modify: `crates/app/src/world/undercrown/rewards.rs`: `pub fn display_item(id: ObjectId) -> Option<Item>`, the first row item.
- Modify: the model draw path used by `held_art` / world item drops. Find the dropped-item render, `rg -n "dropped" crates/app/src/render | head`, and add Undercrown plinth displays as extra instances. They spin at a fixed rate from the render clock (render only, not sim) and bob 0.1 m.
- Test: `crates/app/src/world/undercrown/rewards.rs` tests.

**Interfaces:**
- Consumes: `Plan::model_box` (Task 1/3) and `flags::CLAIMED`.
- Produces: `pub fn plinth_displays(plan: &Plan, state: &State, gen: u32) -> Vec<(Vec3, Item)>`. It is empty below the gate, skips claimed plinths, and positions each item at `model_box.hi.y + 0.4` above the box centre.

- [ ] **Step 1: Write failing tests.**
  - `unclaimed_plinth_is_displayed`.
  - `claimed_plinth_is_not_displayed`.
  - `no_display_below_gate`.
  - `display_survives_state_round_trip`: serialize and deserialize `State`, and the claimed plinth stays hidden.
- [ ] **Step 2:** Run `cargo test -p app undercrown::rewards` and expect FAIL.
- [ ] **Step 3:** Implement `plinth_displays` and hook it into the item-drop render list once per frame. The plan is cached, so there is no per-frame plan rebuild.
- [ ] **Step 4:** Run the tests and expect PASS. Capture A/B at the plinth room.
- [ ] **Step 5:** Commit `app(undercrown): show plinth rewards before claim`.

### Task 5: Underground perf baseline and fix

**Files:**
- Create: `docs/perf/2026-11-12-undercrown-frames.md` (evidence)
- Modify: decided by the measurement. The candidates are `crates/app/src/render/torch_occlusion.rs:12-150` (restamp at half scale), shadow work per light, and the water wake after mechanisms.

- [ ] **Step 1: Baseline.**
  - On the 4090, release-fast build, default view distance.
  - For each room of {landing, counterweight hall, chasm reveal, bridge village, borough street, the screenshot tower room}, run `VOXEL_REVIEW_DUNGEON=undercrown:<room>` with frames.csv.
  - Interleave 3 passes and keep the best of each.
  - Record p95 frame time, draws, triangles, torch count and the torch-occlusion upload time.
  - Pin the bench to P-cores.
- [ ] **Step 2:** Get a surface reference: the same view distance on the review seed's spawn pose.
- [ ] **Step 3:** Identify the top cost from the CSV columns. Write the finding into the evidence doc before touching code.
- [ ] **Step 4:** Implement the smallest fix for that cost. Examples: limit the occlusion restamp to dirty chunks only, or cap lights per chunk.
- [ ] **Step 5:** Re-capture interleaved A/B. Pass condition: p95 and draws are ≤ baseline in every room, and the target room improves measurably. Run the iGPU (`VOXEL_GPU_POWER=low`) once and report it; it is not gating.
- [ ] **Step 6:** Commit the fix plus the evidence doc: `perf(undercrown): <cost> — p95 X→Y ms`.

## Verification checkpoint

After each task, run `python -I scripts/workflow.py verify`, and record manual captures with `attest`.
