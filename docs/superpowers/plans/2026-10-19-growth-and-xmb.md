# Growth, lanterncap, and XMB Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One owned lanterncap can be cultivated, conjured, and enlarged by a single growth operation, arranged on an XMB wand, and used to climb, weigh, and squeeze through the Hanging Cistern.

**Architecture:** `growth.rs` owns the companion, size classes, and `apply_growth`. The wand evaluator only names a target and a delivery. Rooted caps are ordinary tagged blocks. Menu motes never enter the world particle pool. The cistern is one version-gated structure, not a change to every dungeon.

**Tech Stack:** Rust, existing `Adventure` / `VoxelParticle` / egui gear screen, save footer versions.

**Spec:** `docs/superpowers/specs/2026-10-19-growth-and-xmb-design.md`

## Global Constraints

- One owned lanterncap. A second conjure rejects.
- Ages: a created body is `Age::Juvenile`. Growth never changes age. `Age::Mature` has no recipe.
- Body heights: Small 0.7 m, Normal 1.3 m, Large 2.4 m. Half-widths: 0.25, 0.45, 0.70 m.
- Rooted normal cap top is `feet_y + 1.3`. That is the gallery landing. Large is a second step. Plate accepts weight 2 only.
- Tunnel clear width 0.70 m. Small fits. Normal and Large do not.
- `apply_growth` steps by +1 or −1 only. Clearance failure writes nothing and spends no biomass. The cast still spends mana.
- Spell ids 34 Touch, 35 Growth, 36 Conjure, 37 Widen, 38 Seek ally, 39 Shrink. `wands::EMPTY` stays 8. `MAX_SPELL_ID` becomes 39.
- Costs: Touch 4, Growth 14, Shrink 14, Conjure 28, Widen 6, Seek ally 8.
- Widen sets projectile radius to 1.8 and does not call `apply_growth`.
- World particle cap stays 480. UI pool cap 64, discarded when the gear screen closes. A growth ring is the first 12 world motes; the burst is what remains of a 28-mote request.
- XMB: focused icon 72 px, neighbor 44 px at 0.72 opacity, two away 32 px at 0.40. Slide 180 ms ease-out. Panel dims the world to 0.55. No wallpaper.
- New worlds use generator 19 for the cistern only. Worlds below 19 do not sample it. `SAVE_VERSION` becomes 18. Versions 1–17 decode with no companion block.
- No skinned meshes, no second particle simulator, no echo/divide/curse/wiggle spells.

## Review Focus

- A ceiling closer than the next class must leave size, blocks, and biomass unchanged. Test in Task 1.
- Bolt payload Conjure then Growth must enlarge the created id and leave every other creature alone. Test in Task 4.
- Recall and save/load must keep the same id and must not increment the companion count. Test in Task 1.
- Confirming an XMB slot must not call `spawn_voxel_particle`. Test in Task 5.
- A cultivator cycle that fails clearance must leave the biomass unit in the machine. Test in Task 7.

---

## File map

- Create `src/world/adventure/growth.rs` — companion, classes, `apply_growth`, recipes, carry step.
- Modify `src/world/adventure.rs` — `mod growth`, spell tables through id 39, companion field, encode/decode tail.
- Modify `src/world/adventure/wands.rs` — `ShotMods.radius`, effect payloads, two-effect chain.
- Modify `src/world/storage.rs` — writer version 18 and a companion footer.
- Modify `src/egui_ui/icons.rs` — glyphs for 3, 24, 25, 26, 27, 34–39.
- Create `src/egui_ui/wand_xmb.rs` — focus scale and UI motes. Gear screen calls it.
- Create `src/world/cistern.rs` — one placed structure.
- Modify `src/world/sites.rs` — sample the cistern before cavern ellipses when `generator_version >= 19`.
- Modify `src/world.rs` — `CURRENT_GENERATOR_VERSION` 18 → 19, `mod cistern`.
- Modify `src/world/factory.rs` — spore output and adjacent `apply_growth`.

### Task 1: Companion and apply_growth

**Files:**
- Create: `src/world/adventure/growth.rs`
- Modify: `src/world/adventure.rs` (module line, `Adventure` field, encode/decode)
- Modify: `src/world/storage.rs` (version cap and footer)
- Test: `src/world/adventure/growth.rs`, `src/world/storage.rs`

**Interfaces:**
- Consumes: `World::solid`, `Adventure::encode` / `decode`
- Produces:
  - `enum SizeClass { Small, Normal, Large }` with `fn next(self) -> Option<Self>`, `fn prev(self) -> Option<Self>`, `fn height(self) -> f32`, `fn half_width(self) -> f32`, `fn weight(self) -> u8`, `fn carry(self) -> u8`
  - `enum Age { Juvenile, Mature }`
  - `struct Companion { id: u32, age: Age, size: SizeClass, rooted: bool, health: f32, pos: Vec3, crate_mark: Option<IVec3>, pad_mark: Option<IVec3> }`
  - `enum GrowthTarget { Companion(u32), SporeCell(IVec3) }`
  - `enum GrowthReject { Clearance, NotATarget, OwnerFull, NoStep }`
  - `fn apply_growth(world: &mut World, target: GrowthTarget, steps: i32) -> Result<SizeClass, GrowthReject>`
  - `Adventure.companion: Option<Companion>`
  - `fn clearance_ok(world: &World, feet: Vec3, size: SizeClass) -> bool` — false when any solid cell intersects the destination capsule, or the player capsule overlaps it

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn growth_steps_age_and_rejects_a_low_ceiling() {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 3);
    let mut companion = Companion::juvenile(1, Vec3::new(4.0, 80.0, 4.0));
    assert_eq!(companion.size, SizeClass::Small);
    assert_eq!(companion.age, Age::Juvenile);
    world.adventure.companion = Some(companion);
    assert_eq!(
        apply_growth(&mut world, GrowthTarget::Companion(1), 1).unwrap(),
        SizeClass::Normal
    );
    assert_eq!(world.adventure.companion.unwrap().age, Age::Juvenile);
    apply_growth(&mut world, GrowthTarget::Companion(1), 1).unwrap();
    assert_eq!(world.adventure.companion.unwrap().size, SizeClass::Large);
    // Ceiling one cell above the large head.
    let head = (world.adventure.companion.unwrap().pos + Vec3::Y * 2.5).floor().as_ivec3();
    world.set(head, BlockId::Stone);
    world.adventure.companion.as_mut().unwrap().size = SizeClass::Normal;
    assert_eq!(
        apply_growth(&mut world, GrowthTarget::Companion(1), 1),
        Err(GrowthReject::Clearance)
    );
    assert_eq!(world.adventure.companion.unwrap().size, SizeClass::Normal);
}
```

- [ ] **Step 2: Run `cargo test --lib world::adventure::growth::tests::growth_steps_age_and_rejects_a_low_ceiling -- --exact`**

Expected: FAIL, `apply_growth` or `Companion` missing.

- [ ] **Step 3: Implement the signatures in `growth.rs` and store `companion` on `Adventure`.**

`steps` other than 1 or −1 returns `NoStep`. Missing id returns `NotATarget`. `SizeClass::next/prev` return `None` at the ends, and that is `NoStep`.

- [ ] **Step 4: Add the save test in `storage.rs`**

```rust
#[test]
fn v18_roundtrip_keeps_one_companion_and_v16_has_none() {
    // Build a world, set companion id 7 at Normal, save at version 18, load.
    // Assert id 7, SizeClass::Normal, Age::Juvenile, and companion count 1.
    // A footer written with save_version 16 decodes to companion None.
}
```

Raise `save_version`'s allow range to 18. Writer magic `VXWSAV18`, `SAVE_VERSION = 18`. After `adventure.encode`, when `save_version >= 18`, write `0u32` for none or `1u32` plus the companion fields. Do not read those words on older versions.

- [ ] **Step 5: Run both tests.** Expected: PASS.

- [ ] **Step 6: Commit** `feat: persist one lanterncap and reject growth into a ceiling`

### Task 2: Root blocks

**Files:**
- Modify: `src/world/adventure/growth.rs`
- Test: `src/world/adventure/growth.rs`

**Interfaces:**
- Consumes: `Companion`, `apply_growth`, `World::set` / `World::get`
- Produces:
  - `fn set_rooted(world: &mut World, id: u32, rooted: bool)`
  - Block tag stored beside the companion: `root_cells: Vec<IVec3>` on `Companion`
  - Rooted cluster uses `BlockId::GlowCap` on the cap and `BlockId::FungalSoil` as the stem. Cells are listed on the companion. Unroot sets only those cells back to `BlockId::Air` when they still equal the block this root wrote.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn root_is_standable_and_unroot_spares_foreign_blocks() {
    // Root a small companion. The top glowcap cell is solid.
    // Replace one listed cell with Stone. Unroot.
    // Stone remains. Every other listed cell is Air.
    // One Growth while rooted moves the top cell to feet_y + 1.3 (normal).
}
```

`World::solid` on the cap cell is the stand check. No special stair type.

- [ ] **Step 2: Run that test.** Expected: FAIL.

- [ ] **Step 3: Implement `set_rooted` and teach `apply_growth` to rebuild `root_cells` when `rooted` is true, after clearance passes.**

Collision uses the new cells in the same call as the class change. Do not lerp the blocks. The drawn model lerps to the new height over 0.35 s. `Companion::juvenile` starts at health 40. Add a point light on the cap using the existing adventure light list, radius 6 m, which is under the lumen helm beam.

- [ ] **Step 4: Re-run the test.** Expected: PASS.

- [ ] **Step 5: Commit** `feat: root a lanterncap into glowcap blocks`

### Task 3: Particle recipes

**Files:**
- Modify: `src/world/adventure/growth.rs`
- Modify: `src/world/adventure/spells.rs` (call site only if spawn stays there)
- Test: `src/world/adventure/growth.rs`

**Interfaces:**
- Consumes: `Adventure::spawn_voxel_particle`, cap 480
- Produces:
  - `struct ParticleRecipe { color: [f32; 3], glow: f32, gravity: f32, life: f32, size: f32, burst: u8 }`
  - `fn recipe(spell: u32) -> ParticleRecipe` and `fn event_recipe(event: GrowthEvent) -> ParticleRecipe`
  - `enum GrowthEvent { Success, Reject, Root, Idle, Conjure }`
  - `fn emit_world(adventure: &mut Adventure, event: GrowthEvent, pos: Vec3)` — ring of 12 first for Success, then burst, stopping at 480

Colors from the spec: Growth `(0.72, 0.95, 0.45)`, Conjure `(0.95, 0.78, 0.35)`, Touch `(0.85, 0.90, 0.95)`, Widen `(0.55, 0.75, 1.0)`, Seek ally `(0.45, 1.0, 0.70)`, Shrink `(0.55, 0.42, 0.28)`. Success gravity `0.2`. Reject gravity `2.0`, burst 6, life 0.4.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn success_keeps_a_ring_when_the_pool_is_nearly_full() {
    // Fill particles to 470. emit_world(Success).
    // Count motes at the ring radius. Assert at least 12.
    // Assert particles.len() <= 480.
}
```

- [ ] **Step 2: Run it.** Expected: FAIL.

- [ ] **Step 3: Implement recipes and call `emit_world` from `apply_growth` success and `GrowthReject::Clearance`.**

Idle trickle is 1 mote per 0.4 s from a living companion, and `emit_world(Idle)` does nothing while that companion already has 8 of its motes alive.

- [ ] **Step 4: Re-run.** Expected: PASS.

- [ ] **Step 5: Commit** `feat: share growth particle recipes with a ring-first budget`

### Task 4: Wand grammar

**Files:**
- Modify: `src/world/adventure.rs` (`MAX_SPELL_ID`, names, costs, hints, `spell_available`)
- Modify: `src/world/adventure/wands.rs`
- Modify: `src/world/adventure/growth.rs` (impact entry)
- Test: `src/world/adventure/wands.rs` acceptance module

**Interfaces:**
- Consumes: `apply_growth`, `GrowthTarget`, `ParticleRecipe`
- Produces:
  - `ShotMods.radius: f32` default `1.0`. Widen sets `1.8`.
  - `fn is_effect(spell: u32) -> bool` — 35, 36, 39
  - `CastAction.follow: Option<u32>`, `CastAction.follow_slot: Option<usize>`
  - Trigger may payload an effect. A second effect in the next slot is `follow`, then attachment stops.
  - `fn resolve_growth_cast(world: &mut World, created: Option<u32>, effect: u32, at: Vec3) -> Option<u32>` — 36 inserts one juvenile and returns its id; 35 and 39 call `apply_growth` on `created` if present, otherwise the companion or spore at `at`. Returns the affected id.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn conjure_then_growth_enlarges_only_the_created_id() {
    let mut world = /* superflat, wand tier 2 */;
    // Second companion-sized enemy is not a Companion. Place an Enemy at the impact.
    // Deck: spell 0, trigger 18, conjure 36, growth 35.
    // After resolve, companion id is Some, size Normal, age Juvenile, enemy health unchanged.
}

#[test]
fn widen_does_not_change_size_without_a_growth_payload() {
    // ShotMods.radius == 1.8 after a cast block of [37, 0].
    // Companion size stays Small.
}
```

Also assert: Touch then Growth on a fungal-soil spore cell creates one juvenile at Small. Seek ally on a bolt sets `ShotMods.seek_ally` and does not change radius. A Growth cast whose `apply_growth` returns `Clearance` still reduces mana by 14. `spell_available` is false for 34–36 and 39 when wand tier is 0, and false for 37 and 38 when wand tier is 1.

- [ ] **Step 2: Run both tests.** Expected: FAIL.

- [ ] **Step 3: Extend the evaluator and the impact path.** Do not change modifier damage cap 2.6. Seek ally sets `homing` and a new `ShotMods.seek_ally: bool`. Steering in the projectile loop checks the companion inside 18 m before enemies when that flag is set.

- [ ] **Step 4: Re-run both tests plus `cargo test --lib world::adventure::wands::acceptance -- --test-threads=8`.** Expected: PASS, existing wand tests still pass.

- [ ] **Step 5: Commit** `feat: let a trigger carry conjure and growth onto one id`

### Task 5: Icons and the XMB

**Files:**
- Modify: `src/egui_ui/icons.rs`
- Create: `src/egui_ui/wand_xmb.rs`
- Modify: `src/egui_ui.rs` (`mod wand_xmb`)
- Modify: `src/egui_ui/wands.rs` (gear palette calls the XMB)
- Test: `src/egui_ui/wand_xmb.rs`

**Interfaces:**
- Consumes: spell ids 34–39, `spell_name`, `spell_hint`, `SetWandSlot`
- Produces:
  - `fn slot_metrics(distance: i32) -> (f32, f32)` — px and opacity for distance 0, 1, 2+
  - `fn category(spell: u32) -> XmbCategory` — Bolts, Modifiers, Effects, Summons, Gear
  - `struct UiMote { color: [f32; 3], life: f32 }`
  - `fn push_focus_mote(pool: &mut Vec<UiMote>, recipe: &ParticleRecipe)` — refuses past 64, never touches `Adventure.particles`
  - Gear actions, not spells: Call, Root, Mark crate, Mark pad

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn focus_metrics_match_the_xmb_scale() {
    assert_eq!(slot_metrics(0), (72.0, 1.0));
    assert_eq!(slot_metrics(1), (44.0, 0.72));
    assert_eq!(slot_metrics(2), (32.0, 0.40));
}

#[test]
fn ui_motes_do_not_require_a_world_particle() {
    let mut pool = Vec::new();
    push_focus_mote(&mut pool, &recipe(35));
    assert_eq!(pool.len(), 1);
    // Fill to 64 and assert the next push is a no-op.
}
```

- [ ] **Step 2: Run the tests.** Expected: FAIL.

- [ ] **Step 3: Draw unique pixel glyphs.** Growth rises, Shrink lowers, Widen is a fatter bolt. Spells 3, 24, 25, 26, 27 each get a glyph and stop using the purple gem arm.

Replace the wrapped 38 px palette in the gear wand page with the horizontal deck and the Down-opened column. Slide duration constant `SLIDE_MS: f32 = 180.0`. Dim factor `0.55` on the world image already behind the panel. `paint_cast_block_brackets` stays under the deck.

- [ ] **Step 4: Re-run the XMB tests.** Expected: PASS.

- [ ] **Step 5: Commit** `feat: focus the wand deck like an XMB`

### Task 6: Hanging Cistern

**Files:**
- Create: `src/world/cistern.rs`
- Modify: `src/world.rs`, `src/world/sites.rs`
- Test: `src/world/cistern.rs`

**Interfaces:**
- Consumes: `SizeClass::height`, `half_width`, generator version
- Produces:
  - `fn sample(cfg: &WorldSampleConfig, metres: Vec3) -> Option<BlockId>`
  - `fn landing_offset() -> f32` equals `SizeClass::Normal.height()` (1.3)
  - `fn tunnel_width() -> f32` equals 0.70
  - `fn plate_accepts(weight: u8) -> bool` — `weight >= 2`
  - A wild rooted lanterncap pose and a spore cell, both inside the garden, both before the stair in the local layout

`sites::cavern` calls `cistern::sample` before the ellipse chambers when `cfg.generator_version >= 19`. Below 19 it returns `None` immediately. Bump `CURRENT_GENERATOR_VERSION` to 19. Do not change branches that already check `>= 18`.

Place the origin at a horizontal point whose distance from the world origin is greater than 100 m and which `undercrown::reserved` and `dungeon::reserved` reject for seed 1. The test does not lock that origin. It locks the local relationships.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn stair_plate_and_tunnel_match_the_size_table() {
    assert_eq!(landing_offset(), SizeClass::Normal.height());
    assert!(tunnel_width() + 0.001 >= SizeClass::Small.half_width() * 2.0);
    assert!(tunnel_width() < SizeClass::Normal.half_width() * 2.0);
    assert!(!plate_accepts(SizeClass::Normal.weight()));
    assert!(plate_accepts(SizeClass::Large.weight()));
    let cfg = sample_config_version(19);
    assert!(sample(&cfg, garden_spore_local()).is_some());
    let old = sample_config_version(18);
    assert!(sample(&old, garden_spore_local()).is_none());
}
```

- [ ] **Step 2: Run it.** Expected: FAIL.

- [ ] **Step 3: Author the local rooms.** Maintenance tunnel, broken stair, gallery, plate, return shaft, wild lanterncap. Two encounter anchors reuse existing brawler and ranged kinds through `sites::encounters` only if that function already accepts an extra anchor. If it does not, leave the fight as two `Enemy` spawns recorded beside the cistern site id, using kinds 0 and 3. Do not add kind 11.

- [ ] **Step 4: Re-run.** Expected: PASS.

- [ ] **Step 5: Commit** `feat: place the hanging cistern in generator 19`

### Task 7: Cultivator and carry

**Files:**
- Modify: `src/world/factory.rs`
- Modify: `src/world/adventure/growth.rs`
- Test: both files

**Interfaces:**
- Consumes: `apply_growth`, `Companion.crate_mark`, `pad_mark`, `carry()`
- Produces:
  - Cultivator spore mode: one spore item per cycle into output, and if an adjacent cell holds the companion or a planted spore, one `apply_growth(..., 1)` that spends 1 biomass only on `Ok`
  - Adjacent lanterncap wins over an adjacent spore
  - `fn carry_step(world: &mut World) -> CarryState` where `CarryState` is `Rooted`, `Waiting`, or `Moved`
  - Waiting plays `GrowthEvent::Reject` once per wait entry, not every tick

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn cultivator_refunds_biomass_when_growth_does_not_fit() {
    // Adjacent companion at Normal, ceiling blocks Large, one biomass unit.
    // After the cycle: size still Normal, biomass still 1.
}

#[test]
fn carry_moves_one_stack_and_waits_when_the_load_is_heavy() {
    // Marks set. One small stack at the crate. carry_step returns Moved and the stack is at the pad.
    // A stack with carry class above the companion: Waiting, stack stays, one reject emit.
}
```

- [ ] **Step 2: Run them.** Expected: FAIL.

- [ ] **Step 3: Implement the cycle and `carry_step`.** Gear actions only store the looked-at ground cell into `crate_mark` or `pad_mark`. The walk does not insert into the cultivator.

- [ ] **Step 4: Re-run, then `cargo test --lib world::adventure::growth world::cistern world::storage::tests::v18_roundtrip_keeps_one_companion_and_v16_has_none -- --test-threads=8`.** Expected: PASS.

- [ ] **Step 5: Commit** `feat: grow an adjacent lanterncap and carry one stack home`

---

## Spec coverage check

- Shared `apply_growth` for touch, bolt, and cultivator: Tasks 1, 4, 7.
- Juvenile stays juvenile, enemy stays enemy: Tasks 1 and 4.
- Same id on save and recall: Task 1. Recall is `companion` moved to the player position, no new id. Gear Call does that assignment.
- Root platform and unroot tag rule: Task 2.
- Widen versus Growth: Task 4.
- XMB metrics, UI pool, dim, brackets: Task 5.
- Cistern stair, tunnel, plate, version gate: Task 6.
- Biomass refund and carry waits: Task 7.
- Glyphs for the old purple-gem spells: Task 5.
- Echo, divide, curses, wiggle: absent on purpose.
