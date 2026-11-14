# Manyfold dry tube: Stage A

Requested implementation: 2026-10-29. This implements the first tube from
`docs/NEW/AXIOM_Open_Maze_Geometry_and_Gravity.txt`; the later geometries remain
outside this increment.

## Play

Run `tools/Play-Manyfold.ps1` to open the isolated build with its runtime assets.
Create a world and select **Manyfold Tube**. Half-metre blocks give the curved
floor finer steps; all three existing block sizes are supported by the source.
Follow the sandstone band around either side to the opposing workshop. The
entrance has a square belfry; the workshop has an inward-facing roof and bench.
The route closes into a full circumference, with two axial aisles and low maze
partitions beside it. The lower side has grass and the band has planted edges.

Normal movement, sprint, crouch and jump use local up. Creative double-jump
flight and spectator movement retain their existing bindings. Look at the small
brass weight near the entrance and press the existing Use binding to toss it.
The weight and player sample the same field. Ordinary jumps stay close to the
floor. Edits and the player's upright frame survive saving.

## Implementation boundaries

- The axis is Z, centre `(0,36,0)` metres, floor radius 24 m, length 48 m.
  Ordinary Euclidean voxel chunks contain all districts and collision.
- Acceleration is `(28/24) * (x,y-36,0)` m/s² inside radius 32 m and
  `abs(z) <= 27 m`; it is exactly zero at the axis and outside that domain.
  End walls and an outer safety shell enclose the admitted region.
- A capsule controller handles local-up input, contact, stepping, crouch
  clearance and jump. Translation uses bounded capsule/voxel-box sweeps;
  rotational clearance covers short swept chords with arc padding. Airborne
  alignment does not rotate velocity. At zero gravity, the last frame remains.
- Camera, culling, shadow fitting, held-item basis, pickup reach and placement
  bounds follow that frame. Remote poses carry a validated unit quaternion;
  gravity itself is determined by world type and physical position.
- The brass weight is one bounded shared body stepped by solo/host authority.
  It uses voxel/fitting collision and a damped bounce. Its position and velocity
  are persisted and replicated; using it does not create inventory resources.
- The world has a new terrain tag, **5**, preserving tags 0–4. Its header alone
  carries ten extra floats: upright quaternion, weight position and velocity.
  Save-library header reads account for that extension. Existing world layouts
  and generator version gates are unchanged. Gameplay delta protocol is 3;
  connected players must run matching builds.
- Vertical streaming retains the whole finite tube. The ordinary heightfield
  horizon and orbital models are omitted. Guide lighting checks the edited
  crystal cells before emitting light. Mob spawning defaults off in this world.

This is a dry traversal district. Arbitrary-gravity liquids, gas, creature AI,
vehicles, machines, combat projectiles, bent tubes, shell worlds and optical
workshop puzzles are not implemented by this slice. Existing advanced movement
gear (dash, slide, wall-run) is not enabled by the radial controller.

## Evidence

`cargo check -p minecraft_style_voxel_prototype --offline` passed, followed by
an optimized build with `cargo build -p minecraft_style_voxel_prototype
--profile release-fast --offline --target-dir target/manyfold-build`.
The executable is `target/manyfold-build/release-fast/minecraft_style_voxel_prototype.exe`.
Two concurrent compile blockers were repaired minimally: duplicate network
commands return from their packet closure; scanner requests use typed get/remove
without requiring a default command.

No tests or gameplay runs were performed, per the
session instruction. Full-circuit traversal, camera comfort, capsule clearance
under edits, save/reload, frame timing and two-player traversal still need native
acceptance. Do not treat the analytical field or a successful build as those
runtime results.
