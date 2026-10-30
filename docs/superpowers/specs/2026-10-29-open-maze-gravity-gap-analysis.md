# Open maze geometry and gravity brief: gap analysis

Status: analysis only, no code changed. Owner questions in section 6.

Inputs (untracked owner staging, 2026-10-29):
- `docs/NEW/AXIOM_Open_Maze_Geometry_and_Gravity.txt` (stages A to E, lines 266-276)
- `docs/NEW/The_Manyfold_Design_and_Implementation_Brief.txt` section 42
- `docs/NEW/AXIOM_Reality_Contract_v1.txt` (gravity profiles)

Decision 0039 applies: briefs are backlog, not inputs of record. MASTER_PLAN M1
(`dam_break`) keeps priority. "Manyfold" is a new proper noun and is not
approved (0039 §3); code uses `radial_tube` or `gravity_region`.

## 1. The brief's own prerequisite is missing

Line 266 makes the open tube follow "reliable arbitrary-up movement" and a
"fixed-quarter-turn controller". Neither exists. The tree assumes world -Y
everywhere a body moves:

| Assumption | Where |
|---|---|
| Gravity is a scalar applied to `vel.y` | `crates/app/src/player.rs:424`, `:727`, `:1004`; `world.rs:81` (`GRAVITY = 28`) |
| Player collider is an axis-aligned box | `world/collision.rs:5` (`player_aabb`) |
| Look is yaw/pitch about world Y | `player.rs:190-191`; camera roll only for effects, `render.rs:4049` |
| Pose wire format is yaw/pitch, pitch clamped to +-90 | `crates/netcode/src/pose.rs:20-35` |
| Movement validator measures ascent and hover along Y | `crates/netcode/src/movement.rs:8-12`, `:41`, `:95` |
| Enemy gravity | `world/adventure/navigation.rs:163` |
| Projectile gravity | `world/adventure.rs:1991` (`shot.velocity.y`) |
| Sky light columns run down Y | `mesh/solid.rs`, `world_shader.wgsl` |
| Liquids and sand fall to -Y | `engine_sim` Margolus kernel, `simulation/cellular.rs` |

At HEAD 0697be94 no gravity frame or quarter-turn code exists. The shared tree
has uncommitted peer work for Stage A: `WorldType::Manyfold`
(`voxel/src/config.rs`, save tag 5 in `world/storage.rs`), `Player::gravity_frame`
(`player.rs:195`), a capsule sweep with rotation clearance in
`player/radial.rs`, and `engine_worldgen::manyfold::acceleration`. Sections 2
and 5 describe the cost from HEAD; that work answers question 3 with a capsule.

## 2. What a quarter-turn controller costs

An AABB stays an AABB under 90-degree turns about a cell axis, so quarter-turn
down (six directions) keeps the voxel collision path. It needs:
- `Player::down: Axis6` and a basis built from it; yaw/pitch become local;
- movement, step-up, crouch, wall-run (`player/movement.rs`) written against
  the basis instead of `.y`;
- pose format carries `down` (3 bits); validator measures ascent along `down`;
- enemies and projectiles read the same per-cell field;
- a save field for `down`, which takes a save version (memory: factory
  save-format constraint).

Smooth arbitrary up (the tube, where down changes every column) needs more:
the AABB no longer fits, so a capsule or rotated box against voxels, plus the
rotation clearance check the brief cites. That is a new collision primitive and
a new ADR.

## 3. Brief maths checked

- Linear field `g(p) = (g_wall / R) r` is a repulsive harmonic field. With
  `r'' = k r`, `k = g_wall / R`, a radial inward launch from `R` reaches the
  axis only when `v > sqrt(g_wall R)`. Correct.
- With the engine's numbers (`GRAVITY = 28 m/s^2`, `JUMP_VELOCITY = 8.4 m/s`):
  at `R = 64 m` crossing needs 42.3 m/s; a jump rises 1.27 m (flat: 1.26 m).
  At `R = 32 m`, 29.9 m/s. Ordinary jumps stay local at any tube this size.
- A down direction that rotates with distance along a passage has nonzero curl,
  so a loop can gain energy. The brief says so; ADR 0007 (energy rule) would
  need the source declared.
- Mobius surfaces have no continuous normal. Correct; the brief's thick-track
  and seam variants avoid it.

## 4. Fit with current decisions

- Authority: players stay host-authoritative (ADR 0011, 0039 §2). Gravity as a
  pure function of position and world seed fits; the validator must use it.
- Liquids: integer `engine_sim` has one gravity direction. Stage A is dry, so
  no conflict. Stage D's "local water circuit" needs per-region gravity in the
  sim kernel, which touches the byte-identical CPU/GPU pair (`sim`, `sim_gpu`).
- Worldgen: a tube region needs a generator gate. Gates collide across branches
  (memory: generator gate collisions); check the registry first.
- Rendering: opposite-wall views are long sightlines inside the view radius;
  the horizon and veil assume an outdoor sky. Budget unknown until a capture.

## 5. Smallest slice that fits

1. Quarter-turn down, player only, behind a creative-mode block that sets the
   field in a box region. AABB collision unchanged. Tests: walk a cube room
   floor to wall to ceiling through three fixed-down boxes; same field sample
   for two positions in one cell; pose round trip with `down`.
2. Stage A dry tube: needs smooth up (capsule) first. Own ADR before code.

Slice 1 adds a save field and a netcode pose field; neither is taken without a
MASTER_PLAN row (0039 consequences).

## 6. Owner questions

1. Route this brief as backlog under 0039, or name the tube as a milestone?
2. Approve a quarter-turn controller slice (save bump, pose format change)?
3. Collision for smooth up: capsule against voxels, or rotated AABB sweep?
4. Placeholder for "Manyfold" in code and RON: `gravity_region` acceptable?
