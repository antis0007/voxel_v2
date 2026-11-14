Axiom modded-system studies, 26 October 2026

Five selected original concept sheets, generated with the built-in imagegen tool:
  keepers_garden.png       - 8 living automation and cultivation devices
  archive_logistics.png    - 8 storage, request, transport and control devices
  founders_drive_train.png - 6 kinetic/industrial modules and two press states
  rites_of_resonance.png   - 8 research/ritual/service instruments
  arrival_day_exchange.png - one inhabited settlement combining the systems

The batch adds 30 object concepts. Repeated diagrams, paired states and scene
reuses are not counted as new objects. The initial gearbox drawing is retained
as founders_drive_train_v1.png; the selected sheet received a targeted bevel-gear
revision. That revision is clearer visually, but actual shaft routing/meshing
still needs three-dimensional design before it can guide a working machine.

Grounding in the repository

The planning matrix is docs/research/MOD_FEATURE_MATRIX.md and its adoption
brief is docs/specs/MOD_ADDENDUM.md. The notes below describe the inspirations
we translated, not independently verified current behavior of the named mods.

Keepers: docs/research/mod-family-botania.md and A.11 Forestry/growth candidates.
Spatial resource flow, local controls, visible storage, feed and spent substrate.

Archive: docs/research/mod-family-storage-links.md, the AE2 planning rows, and
A.15 shared links/VM. Finite physical stock, clear compartment identity and a
distinct path for information versus cargo. No matter duplicated by a signal.

Founders: docs/research/mod-family-create.md plus Immersive Engineering,
GregTech and Mekanism candidates. Visible drive paths, supported working
assemblies, useful access, process heat and local failure indications.

Orders/Copyists: docs/research/mod-family-thaumcraft.md,
docs/research/mod-family-ritual-magic.md, Ars Nouveau and Hex Casting references.
Observation, ordered procedures, supplied reagents and dangerous byproducts.
The covered resonance instrument and opened service view are the same apparatus.

Lore: docs/design/WORLD_LORE.md. Every new concept has its proposed era, purpose,
physical evidence, motif/office link and an unanswered question in manifest.json.
No new world facts are asserted. Resonant crystal manufacture remains unsolved;
the sender of the return, the Surveyors and the ninth office remain unresolved.

Palette and source files

palette.json and anchor32.gpl are unchanged copies of the shared 32-colour sRGB
authoring target from the preceding expansion. Generated reference PNGs are not
quantized shipping atlases. Each exact generation prompt and the gearbox edit
prompt are preserved as neighbouring .prompt.txt files.

The selected images were visually inspected. provenance.json records dimensions,
hashes, palette size and concept count. No production models, engine integration,
mechanics, performance result or native gameplay acceptance is claimed. These
art files required no tests or hosted CI.
