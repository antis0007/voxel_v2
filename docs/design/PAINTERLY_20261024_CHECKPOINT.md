# Painterly world overhaul: implementation checkpoint, 2026-10-24

## Reference-driven shader revision after user rejection

**User explicitly rejected the R2 appearance as awful. It is not an accepted
visual baseline or a successful art-direction result.**

Follow-up diagnostics: `rejected-look-ambient` and `rejected-look-direct` under
the forest review root separate fill from direct illumination at the same pose.
Ambient alone retains much of the flat meadow appearance; direct illumination
shows substantially stronger shadow/face separation. A subsequent candidate
reduces clear-day sky fill and approximate sun bounce, retaining the original
overcast/twilight limits. Evidence/build log is under
`target/lighting-rebalance-20261024/`; capture label
`rejected-look-lighting-candidate`. This is a correction under review, not a claim
that the requested reference quality has been reached.
The candidate built and rendered successfully, but inspection still shows the
same weak overall image. Stronger direct/fill separation alone is insufficient;
this candidate does not pass the reference-quality gate either. Do not describe
these palette or lighting adjustments as the requested visual overhaul.

Reopened Downloads references: `Витаутас Лайсонас.jpg`,
`989806824364727771.jpg`, `620793129921799020.jpg`, and
`Practice painting study by Alex Ichim.jpg`. Their quiet wall/ground masses,
selective detail and separation of light/shadow drove this shader revision.

Changes in `world_shader.wgsl`: olive/sage terrain reflectance, removed the
second natural-material saturation boost, broad wear instead of dense spots,
quieter grass/gravel/leaf microcontrast, crown-scale leaf pigment, reduced leaf
light wrap/transmission, and larger stone courses with narrower quieter mortar.
These are material/light-response changes, not an image filter or save change.

Build `target/reference-shaders-20261024/after-r2.exe` passed release-fast and
shader validation (30 warnings); SHA256
`00F4DE482480CB38338AC72E12FAAA541DC889C122B42379A025B435C2677C53`.
Shader SHA256 at that review:
`FA873ABA73DE6D8F76C7CFCEDF8557E2932D36E0812A21B7D7899F0E071637B3`.

Actual engine captures:
- `target/reviews/forest-repair-20261021/reference-shaders-before/camera.png`
- `target/reviews/forest-repair-20261021/reference-shaders-after-r2/camera.png`
- `target/dungeon-review/reference-shaders-hall-r1/camera.png` (stone response
  unchanged in R2; R2 further adjusts grass).
- `reference-shaders-motion-r2` in the forest review directory records the
  camera route; this is not collision or saved-edit validation. The 24-second
  route completed and `traversal.mp4` was encoded; representative moving frames
  were inspected, not a full-rate temporal/shimmer acceptance review.

Visual inspection: grass is less acidic and less noisy; stone joints no longer
dominate the hall. This does not attain the supplied paintings' quality. Sky
grouping, tree/ground composition and hall light-color separation remain weak.
Before/R2 use the same seed, generator45, camera and 1280x720 settings; their
reported final scene counts match. R1 had differing streamed counts and is not
a performance comparison. Before/R2 p95: 9.687/10.151 ms; GPU terrain:
1.392/1.661 ms. These short captures include readback and do not prove performance
acceptance (R2 includes a 531.851 ms outlier). No new automated tests were run.

This is an unfinished implementation checkpoint, not acceptance of P0–P9 or
V01–V13. The supplied thirteen artwork references were inspected. The exact
040732 shoreline and 041222 hall image files were not found; generated fixtures
are comparable contexts, not exact screenshot reconstructions.

## Implemented

- `crates/worldgen/src/trees/woodland.rs`: version-45 broadleaf age forms,
  connected bent wood, roots and unequal crown lobes; bounds integrated in
  `trees.rs`. Existing specialized forms are retained, not rebuilt.
- `landscape.rs`: coherent stand openings using the existing placement path.
- `terrain_variety.rs`: broad coherent shore sections and sediment transitions.
- `dungeon/bearer_hall.rs`, `dungeon.rs`, `dungeon/v2/kit.rs`: structural hall
  articulation, readable floor, actual removable supported light sources and
  bay-scale masonry variation. Single-source correction followed capture review.
- `world_shader.wgsl`: quieter cut-stone variation/mortar, shared near/far water
  depth palette, removal of fabricated underwater ripple/grain detail in favor
  of cached actual bed material. This does not fix every water ownership issue.
- `cloud_shader.wgsl`: reduced fine cirrus contribution; cloud grouping still
  misses the reference hierarchy in the actual forest capture.

No demonstrated duplicate sRGB conversion was found. Existing linear HDR and
output ownership were retained. No new decorative terrain/forest authority,
physics system, ECS, hydrology framework or save migration was introduced.

## Reproduction and evidence

Immutable local builds under `target/painterly-20261024/`:

| Build | SHA256 |
|---|---|
| before.exe | 8F21990067D49F9E723B79C3E52C189ACFE8665FDC724A406DEAAE9BAC4DDCC6 |
| after-r1.exe | F766C75EBF9B294B7023A9A3622D32905F75D601E27E950A903A6C45E453704A |
| after-r2.exe | 9DDB1825FD95D8D4B7F47B1160F77F6B0DB76CFDBF9BD90AC3ED063EFC26978C |

Release-fast build R2 passed (33 warnings). Composed WGSL validation runs during
the build. Three focused worldgen tests passed; see `tests-r2-final.log`.
These tests were explicitly requested by the attached overhaul brief.

Native captures use seed 2738958700, fixed midday/sky time, normal held wand,
isolated generated worlds and recorded camera/settings/executable manifests.

- Hall: `target/dungeon-review/painterly-before/` and `painterly-after-r1/`.
  R1 visibly exposed structure and floor but was too uniformly amber; inspection
  found multiple torch voxels per added source. R2 reduces these to single voxels.
  `target/dungeon-review/painterly-after-r2/` confirms fewer high-wall flames and
  clearer dark ceiling recesses, with the route still visible. Overall lighting
  remains amber and existing decorative candle groups remain; neutral-light and
  ordinary-equipment comparisons are still needed.
- Forest: `target/reviews/forest-repair-20261021/painterly-before-forest/` and
  `painterly-after-r1-forest/`. Camera `44,35,1,-1.0,-0.12`, 1280×720.
  The meadow still dominates and many silhouettes remain flat. This is not P2
  acceptance. Forest R1 p95 19.407 ms versus baseline 17.429 ms; GPU 9.891 versus
  8.036 ms. Capture readback and concurrent build activity prevent an isolated
  performance conclusion. The 16.67 ms target is not passed by these runs.
- Coast camera: `192,-192,1.7,0,-0.12`; `painterly-before-shore` and
  `painterly-after-r2-shore` labels in the same forest review directory.
  R2 inspection: the near cyan band is reduced and sediment patches vary, but
  the broad gravel apron remains flat and the cloud field remains too busy.
  This is not V01/P3/P4 acceptance. R2 p95 was 18.126 ms with capture enabled.
- Old broadleaf camera: `35,58,1.7,0,0.18`, generated cell (2,3), tree base
  (35,28,43), conservative radius 10.45 m. Selected by the location example.
  Capture label `painterly-after-r2-oldtree`: close wood/root geometry is visible,
  but neighboring foliage overlaps and the crown is cropped. Full silhouette,
  side views and motion are required; this is not an accepted hero-tree specimen.

Before uses generator42; after captures force45. R2's compiled current default
was already46 due to concurrent work; its manifest confirms the forced45 run.
Independent work is present in the
after builds, so these images cannot attribute every difference to this pass.
Static camera captures establish neither physical traversal nor edit/reload
correctness. Performance capture acceptance remains false in run manifests.

## Remaining gates and next work

P0/P1 have useful source/capture evidence but lack exact original-camera and
neutral-material/held-item controls. P2/P3/P4/P5 have implemented changes; their
full visual gates remain open. P6/P7 specimens, P8 consolidation and P9 final
reference review are unfinished.

Next: fix the largest mismatches found in R2 shore/hall and close old-tree captures;
silhouette/value defects. Validate a production stand in motion, crown LOD and
edited/felled-tree persistence before broadening scatter. Author V03–V08 with
real routes using production assets, then local V09–V13 seasonal/garden/storm
fixtures. Conifer/dead-tree forms, layered ground cover, connected distant ridge
composition, neutral lighting controls, low quality, edit/reload, rapid-travel
and isolated performance validation remain outstanding. Do not report those as
implemented or approved from these three tests or static images.
