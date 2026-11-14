Equipment and material study, 26 October 2026

equipment_material_study.png is an imagegen concept reference, generated with
the built-in image tool. The exact prompt is preserved alongside it. It is not
a game screenshot, Blender render, pixel-perfect shipping atlas or implemented
set of twelve weapons. It must not be used to claim those deliverables exist.

The art brief calls for five craft traditions and twelve distinct silhouettes.
The reference gives them a shared walnut/bone/moss/brick/iron/brass/blue palette.
The shared traits are economical shapes, warm dark outlines, quiet surfaces and
connected clusters. A family needs its own construction, not a recoloured head.
One sprite or minimal painted mesh may be enough for a common instrument.

The material candidate is implemented in:
  crates/app/src/shaders/furnishing_surfaces.wgsl
It uses the existing 20 texel/metre world grid. Books use three binding colours
and broad shaded edges. Oak uses quiet planks, iron straps and a small brass
fitting. Lichen uses two explicit connected five-pixel masks with two tones.
It replaces the rejected 64 texel/metre candidate and its fine noise and labels.

Native material evidence and frozen executables:
  target/furnishings-20261026/style-recovery/
  target/furnishings-20261026/style-r1-*/ (initial comparison)
  target/furnishings-20261026/style-r2-books/
  target/furnishings-20261026/style-r2-overview/
  target/place-review/style-recovery-library-r2/
Use tools/Review-Furnishings.ps1 for reproducible disposable swatch captures.
The swatches exercise production blocks, shader and lighting. They do not
establish broad world coherence, interaction acceptance or frame-time results.

The current held-equipment production work is concurrent in this checkout;
this reference does not overwrite those meshes or establish new spell mechanics.
Before integration each instrument needs a deliberately drawn small icon,
an appropriate sprite/proxy or mesh, and an actual native gameplay-size review.

Revision 2 fixes clipped book rows by aligning shelf repeats to whole metres.
Optimized build succeeded; the build script validated 19 composed WGSL programs.
Native close book/chest/lichen and library captures were opened and inspected.
The reference is intentionally more expressive than the current held model.
This pass establishes a quieter material candidate and equipment concept sheet;
the complete twelve-family inventory/held-mesh set remains production work.

Additional imagegen studies:
  anchor_expansion_20261026/ - 24 instruments/machines and a maintenance hall
  modded_systems_20261026/   - 30 system objects and an arrival-day exchange
Each pack contains selected images, exact prompts, a shared palette, a lore
manifest and provenance. catalog.json indexes the selected concept collection.

Spell language expansion: spell_language_20261026/README.txt
Five selected sheets revisit twelve equipment families, define six effect grammars,
show six illustrated first-person views, and propose a workshop and proving yard.
Existing families and repeated views are not counted as new unique objects.
All are imagegen concept references; exact prompts, shared palette and hashes included.
