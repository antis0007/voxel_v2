# Generator 84: Manyfold architecture

Read `ART_DIRECTION.md`, the Manyfold brief's visual language and open-maze
sections, `AXIOM_Open_Maze_Geometry_and_Gravity.txt`, and the supplied reference
index before implementation. Visually inspected these original references in
`docs/AXIOM_COMPLETE_IMPLEMENTATION_PACKAGE/axiom_complete_implementation/reference_art/`:

- `hyper-light-breakers-game-rant-advance-illness-feature.jpg`: layered civic
  remnants, deep openings, small concentrated luminous details.
- `hyper-light-breaker-exclusive-concept-art_46ek.jpg`: grouped canopies,
  quiet paths, vegetation framing a distant destination.
- `djS8kNgj67ov7zLETw27va.jpg`: slender arcades between unequal tower masses.
- `1000021699.jpg`: receding annular structures with visible human-scale routes.

New Manyfold worlds have a limestone annular arcade, side stairs, and distinct
pergola, shrine, archive, copper workshop, planted ruin and belfry roof profiles.
Recessed end-wall masonry frames the finite tube. The ring route has ceramic
edge bands. Plants form coherent crowns and beds beside the circulation.
All of this is editable voxel geometry. Existing seal courts, bypasses, fold
landings and the brass-weight door use their previous geometry. Historical
generators retain their existing samples. This does not implement the brief's
larger branching landscape or populate the buildings with inhabitants.

The existing seal overlay now has broken outer rings, radial nib marks and a
conductor toward its physical anchor. Unknown/unloaded edited anchors no
longer render as though their inactive state were known.

Native review cameras: `manyfold:street`, `manyfold:vista`, `manyfold:seal`.
Use `tools/Review-Places.ps1 -World manyfold -Generator 84` with an isolated
output label. Captures establish rendering only, not traversal or performance.
At metre scale stair rasterization produces metre steps; finer scales provide
smaller steps. No fluid simulation is implied by the dry arcade.

Initial native street and overhead captures in
`target/place-review/manyfold84-{street,vista}-preview/` exposed uniformly warm,
dark cave lighting that obscured the inverted district. The renderer now gives
the Manyfold hollow a cool radial environment fill while retaining cavity
shading and separate local lamps. The existing lighting uniform's unused fog
word carries this flag; other world types keep zero. This is environmental
illumination, not a powered device or simulated sunlight entering a sealed tube.

## Native evidence

Optimized build completed with 86 warnings and all 22 composed WGSL programs
validated; shader hash `424246003f7f6be3`. Log:
`target/manyfold-architecture-build-r3.log`.
Frozen executable: `target/voxel-manyfold-architecture.exe`, SHA256
`7C3FC48511AFAF955302C042DEB27DD7DAC6A9908409DEE5C6F2B28E93186D2F`.

Inspected native `camera.png` captures, all using that executable, seed 424242,
0.5 m voxels, clear midday, and fixed cameras:

- `target/place-review/manyfold84-street-final/`: dressed masonry, pergola,
  connected hanging fixture and clear central aisle.
- `target/place-review/manyfold84-vista-final/`: raised wrapping arcade,
  distinct roof masses, canopy and legible cool overhead district.
- `target/place-review/manyfold83-vista-final/`: matched prior geometry under
  the same lighting, confirming the additional arcade and planted silhouettes.

The earlier preview and final 84 views also show the lighting correction:
overhead masonry is now visible with cool shadows, while the nearby torch
retains its warm contribution. These remain modest finite-tube scenes, not
the brief's landscape-scale branching dimension. Runtime logs include
frame-pacer timeout retries; captures completed with final render reports,
but movement, performance and multiplayer acceptance remain unverified.
No tests or hosted CI were run by this pass. The concurrent freight change
advanced the shared default to 85; this sampler remains gated at 84.
