# Orbital structure refinement, 2026-10-26

The subsequent [freight and elastic-physics implementation](ORBITAL_DYNAMICS_PIXEL_20261026.md)
supersedes this pass's physics limitations and introduces save format 39 and shared
pixel materials. The captures and candidate hashes below remain historical evidence.

Read with GAME_BIBLE C-S5, WORLD_LORE's momentum ledger, decision 0025 and the
SOA space-port design. The screenshot's petalled silhouette is the Dawn mirror
array; atmospheric braids and orbital rotovators are separate infrastructure.

## Causes and implementation

- Skyhook hardware previously had 14–40 pixel minimum sizes; cargo had a
  20 pixel minimum. These expanded with distance independently of the tether.
  Hardware now retains authored metres. This pass preserves the compressed
  celestial planet; catalog hardware scales are owned separately.
- Cables were widened before coverage was calculated, and celestial shading
  returned before applying coverage at all. Physical width and raster support
  are now separate, including in the celestial shader. Skyhook braids taper
  toward the tip using the reference tensile-load profile described below.
- Habitat drums used 16 slab sectors and flat triangle-fan endcaps. Closed,
  64-sector pressure hulls now have curved caps and continuous normals, with
  separate glazing, reinforcement hoops and fixed radiator banks. Their spin
  remains derived from 0.3 g at the rendered radius, read from the catalog scale.
- Atmospheric crown pressure rims now use closed toroidal geometry. Their spin
  uses sqrt(g/r) and the shared orbital clock instead of eight turns per day.
- Mirror leaves now use eight curved, framed segments per surviving petal,
  with finish-dependent solar highlights instead of a single dark slab.
  Missing and jammed petals remain seeded evidence of age.
- Celestial shading previously skipped the ordinary material finish path.
  It now includes filtered plate joints, modest Earthshine and bounded
  ceramic/alloy/mirror highlights. Thermal panels remain mostly diffuse.
- The celestial shader's limb test used Earth's uncompressed radius while
  CPU positions used the compressed planet. Both now use the same radius.
- Skyhook tip and ballast were already positioned by the orbital solution,
  then rotated again about the hub. Hardware is now oriented about its own
  attachment point, leaving the grapple on the computed tether endpoint.
- Meridian carriages previously swept a 1.8 km sideways arc. Their deterministic
  winch path now allows a 24 m lateral deflection with smooth endpoint closure.

## SOA relationship

This is an original Rust/WGSL refinement, not a translation of SOA source. It
follows the existing port design's separation of f64 orbital rails, observer-local
render geometry, and local voxel simulation. Curved geometry and filtered local
materials supply detail without allocating kilometre-long voxel structures.
SOA's published rendering work likewise separates curved geometry and atmospheric
scattering: https://www.indiedb.com/games/seed-of-andromeda/news/procedural-gas-giant-rendering-with-gpu-noise

## Physics boundary

Existing circular Kepler rails, resonant rotating skyhook tips, repeatable port
cusps and counter-rotating habitat drums remain analytic and calendar-driven.
Atmospheric crowns need the lore's field support; an 18 km stationary crown is
not a physically self-supporting geostationary elevator. The taper is not a stress
solver. No live payload mass, orbital momentum exchange, elastic tether, thermal
balance, docking collision, radiation, structural failure, or enterable habitat
simulation is added here. The lore's return-mass ledger remains future gameplay.
Working mirror petals now solve the sun-to-receiver reflection law; mechanical
jams and missing leaves remain seeded.

Saves, terrain generation and service booking formats are unchanged. No tests or
hosted CI are added or run. Local build and native review evidence are recorded
below after completion.

## Local evidence

- `target/orbital-refinement-build-final.log`: offline app build passed. The
  composed shader validation passed for all 19 programs (world shader FNV
  `565449f641d2ebf9`). Existing unrelated warnings remain.
- `target/orbital-refinement-20261026/fixed-habitat/camera.png`: inspected fixed
  orbital-pose ground view. It shows readable curved drums and their separated
  radiator banks after the Earthshine correction; native run exited zero.
- `target/orbital-refinement-20261026/hardware/orbital-anchorage-image.png`:
  inspected close view of the curved rim, spokes, keel and thermal banks.
- The hardware sequence also wrote `skyhook-cusp-image.png`, but that camera
  does not isolate the grapple sufficiently to establish catch alignment.
  Its subsequent mirror command failed in the existing review fixture's
  pass search after the previous shots changed the day length. This is not
  accepted mirror or gameplay evidence. A fresh mirror-only review is separate.
- Final candidate and its hash/launch arguments are preserved under
  `target/orbital-refinement-20261026/final-mirror/`. The preceding captures use
  the earlier candidate, before a final mirror-segment slope-sign correction;
  habitat and crown geometry are unchanged by that correction.
- The fresh mirror-only sequence completed and its `mirror-image.png` was
  inspected. The mirror is visible with segmented leaves, but foreground
  terrain was still streaming after camera relocation. This is a structure
  rendering check, not an accepted clean world-composition capture. Final
  candidate SHA256: `83317A00752566E7EDC336A5A8A4B83C2CC5CAF78AF181FECDA9A901C8EF7D44`.

Review worlds use generator 25 to isolate existing infrastructure from concurrent
terrain changes; the current world generator is untouched. Native captures disable
physics and use disposable saves. They establish rendering only. No performance,
live docking, stress, momentum conservation, or general gameplay claim follows.
The shared-checkout candidates include unrelated pre-existing work.

## Follow-up: optical tracking and cargo hardware

- The mirror array now has a raised receiver and tracking normals that bisect
  directions toward the sun and receiver. The solve compensates array rotation
  in the station's local frame. Jammed leaves retain their seeded bearing angles.
- The skyhook cargo approach uses the next resonant cusp projected through the
  same spherical chart as the moving tip, rather than a flat vertical target.
  Quintic easing closes both velocity and acceleration at the winch joins.
  Cargo orientation stays upright during lift and follows the tensile axis
  after capture, with matching orientation at the home-port cusp.
- Braid sizing now uses a conservative centrifugal-plus-radial-tidal estimate:
  `T(s) = (spin² + 2 mu / r_min³) [m L + lambda (L² - (s L)²) / 2]`.
  Rendered section diameter follows `sqrt(T(s)/T(1))`. The distributed-line
  approximation is uniform; this is not an optimal variable-density tether solve.
  Reference cargo ratings are 20 tonnes, 500 tonnes and 10,000 tonnes; fibre
  density is 1600 kg/m³ and reference area is `braid_width² / 1000`. These are
  explicit game design assumptions, not current cargo inventory or strength proof.
- Cargo cubes are replaced by curved canisters with structural bands, end
  docking fittings and a separate service hatch. Hubs have curved bearing
  housings; grapple jaws have recessed grip pads and actuator rods.
- Habitat spin reads the current catalog scale. Concurrent catalog size edits
  are preserved; this follow-up does not claim ownership of those changes.
- The review pass search now divides orbital seconds by the orbital day constant,
  handles an unavailable pass without panicking, and exports exact camera/epoch
  poses. `tools/Review-OrbitalStructure.ps1` replays those poses with a bounded
  orbital-time sweep and optional close inspection camera. It saves a candidate
  executable, hash, arguments and disposable-save paths alongside each sequence.

Follow-up evidence is under `target/orbital-followup-20261026/`. `mirror-detail`
contains inspected close views of the working bowl, receiver and jammed leaves
over a 20-orbital-second sweep. `skyhook` is the earlier motion review that exposed
the remaining cubic cargo model; it predates the canister change. Per-directory
manifests identify the exact candidate used. The ground `mirror` view includes a
nearby cliff and is not the close-detail acceptance view. Review captures establish
presentation, not live cargo transfers, orbital momentum conservation or gameplay.

The final debug candidate's `skyhook-canister` review completed a 20-orbital-second
cusp sweep and produced six interval captures plus the fully resident final frame.
The first and final frames were inspected: the rounded canister remains held
between the split jaws. Candidate SHA256:
`99A9D24A227E42A3A0EBC74590CF5C340D4C16F63FE50A3A985D4260BBB01B8D`.
The native `mirror-detail` sequence uses the preceding candidate
`444FC979B864FD992BD8CF94F5D9247451EFDCCEF3A6AEA5EF7E2AAAB468B1CD`;
mirror geometry and tracking are unchanged by the subsequent cargo-only change.
These close views use an elevated inspection camera, not a playable orbital EVA.
`target/orbital-refinement-followup-build-final.log` records the successful offline
debug build. No tests or hosted CI were added or run.

The normal `target/release-fast/minecraft_style_voxel_prototype.exe` was rebuilt
offline successfully; see `target/orbital-refinement-followup-release.log` and
`target/orbital-followup-20261026/release.json`. Native captures above identify
their debug candidates separately; no release performance claim is made.
