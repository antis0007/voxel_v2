# Composed vistas

Generator 95 adds three geological destination families to default Hills and
Ridged generation: a natural arch canyon, irregular sandstone buttes, and a
projecting cliff shelf. A ruined lookout anchors each skyline; a broad ascending
path approaches its platform. The tower has an entrance, windows, a broken deck
and internal stair geometry. These are physical generated blocks, not scenery
placed only for screenshots.

Generator 96 replaces the arch's extruded elliptical cut with a bent, tapered
span. Its crown and underside share one profile, and the opening clears the
whole span depth. Canyon shoulders have different heights and shorter, broken
profiles. Generator 95 keeps its original shape for existing saves.
The new arch sampler takes precedence over cavern lining within its physical
field: ray diagnostics found cavern limestone replacing intended air beneath
the span, producing rectangular bars in the native view.
After priority correction, the two affected review-camera rays reach terrain
96 m and 144 m away instead of cavern lining at approximately 70–76 m.

Generator 97 preserves the valley floor and ascending road under the opening.
The previous air carve extended five metres beneath the site base, leaving an
abrupt trench where its finite footprint ended. The corrected carve stops at
the ground profile and supplies a surface layer and solid filler below it.

Candidate centres use a 640 m grid with seeded position and rotation. Some
candidates are rejected, including coastlines and already steep mountain slopes.
Each composition occupies approximately 290 by 200 m, with a foreground frame
and destination separated by roughly 180 m. This is not a guarantee of a landmark
in every cell. Existing older generator saves retain their terrain.

The shared height sampler supplies collision columns and distant terrain.
Voxel sampling carves arch openings and shelf recesses and supplies narrower
sedimentary seams. Generator-96 arches own their physical field; the other
vista families retain the previous authored-structure sampling priority.

Composition reference: Anthony Vaccaro's original Madagascar environment work
for [Uncharted 4](https://autocon.artstation.com/projects/JOdya).
No reference assets were imported. The existing voxel material style is retained.

## Native review

`tools/Review-Vistas.ps1` launches an explicitly supplied executable in disposable
worlds. Its manifests record executable SHA256, seed, generator, voxel scale,
weather, time, resolution and camera arguments. The views are `arch`, `buttes`,
`shelf`, and their `-reverse` variants. Captures are camera-only evidence, not
survival traversal or performance acceptance.

The refined half-metre seed-424242 views and final metre-scale arch/shelf views
were captured at 1600 by 900 with normal clouds and horizon. Native inspection
drove less regular butte outlines, narrower
rock seams and removal of the artificial lower arch lip. Strong voxel face
contrast remains visible on broad cliffs; these captures do not establish
Uncharted-level rendering quality. Arch recess lighting also remains excessively
dark. No tests or hosted CI were run for this vista work.

The generator-96 forward native capture confirms removal of the rectangular
cavern bars and the thinner bent span. Directly shadowed undersides remain dark;
this correction does not change the global lighting or authored material palette.
