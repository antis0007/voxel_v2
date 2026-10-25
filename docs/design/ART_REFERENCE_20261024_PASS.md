# Reference-led material and model pass

This continues the rejected painterly checkpoint; it is not visual acceptance
of the larger overhaul. Native binaries and captures are preserved under
`target/art-reference-20261024/`.

## References actually viewed

The Downloads images inspected in this pass include the Alex Ichim painting
study, `Витаутас Лайсонас.jpg`, `989806824364727771.jpg`,
`620793129921799020.jpg`, `440438038578522254.jpg`,
`467178161332390460.jpg`, `491807221801205561.jpg`, the Tomás Sánchez
woodland island, Tom Uttech woodland, Charles Weed woodland, Welliver winter
woodland, `S U S P I R I A.jpg`, `pearly gates.jpg`, the Ambleside landscape,
and `440015826113010468.jpg`.

Transferred relationships: quiet wall/ground planes; connected dark canopy
masses; warm solar planes against cool sky-lit planes; small plant accents
within broader patches; real plant and mineral forms. No canvas overlay or
global replacement palette was added. Source filename attributions remain
unverified. The images themselves, not filename attribution, informed the work.

## Implementation owned by this pass

- `world_shader.wgsl`: replace multiple independent per-pixel leaf hue fields
  with a metric crown-scale field shared across faces. Darker olive/teal canopy
  pigment and a continuous foliage light response group its planes. Solar and
  sky-lit vegetation have separate bounded colour responses in linear HDR;
  albedo/direct/ambient diagnostic ownership is retained. Local lamps and
  emissive material spectra retain their existing path.
- R4 removes the generic 4x4 albedo dither from bark and canopy: the close R3
  view exposed a regular checker across leaf planes. Their own metric shading
  now owns that detail. Bark uses broad vertical furrows and a greyed age wash;
  leaf clusters have bounded world-fixed shading-normal relief, with geometric
  silhouette and shadow-bias normals retained.
- The R4 native image confirms stronger bark, but a regular leaf motif remains.
  R5 replaces the repeated per-cell leaf diamond with filtered, metric cluster
  variation. Removing the generic dither alone did not close this defect.
- The same shader reduces gravel pebble contrast and normal tilt, placing its
  small detail inside a broad mineral colour wash. This changes reflectance,
  not shoreline geology, water ownership or sediment simulation.
- `mesh/ground_detail/flower.rs`: real bent stems, six cupped petals, a heart
  and two leaves, with root-to-tip colour/AO. Exact reservation: 44 vertices,
  132 indices per blossom. Support/edit ownership and wind are reused.
- `mesh/ground_detail/stone.rs`: low irregular five-sided stones with a tilted
  cleavage plane and real face normals. Exact reservation: 25 vertices,
  39 indices. This replaces the previous smooth dome, without extra scatter.
- The parent ground-detail code retains existing placement, density, species
  selection and support checks, and derives reservations from the two models.

Concurrent work in this shared checkout changed tree forms, ground-cover
rosettes/density, roofs and cloud grouping. Those changes were preserved and
are included in the frozen candidates. The rejected cloud patch did not apply;
no cloud-density edit is attributed to this pass. Native image differences
cannot all be attributed to these material/model edits alone.

## Evidence and remaining limits

Before and R1/R2/R3/R4/R5 builds completed locally; each validated 19 composed shaders.
Final build log: `target/art-reference-20261024/after-r5-build.log` (33 warnings).
No tests or hosted CI were added or run. `git diff --check` passed for tracked
edited scope. New model counts and index ranges were reviewed directly.

Before/R1 meadow images and before/R2 shoreline images were inspected at the
same seed 2738958700, forced generator 48, camera, noon/partly cloudy weather,
1280x720, six resident chunks, fixed cloud time and orbital pose. The gravel
texture is visibly quieter and the canopy darker/more connected. The shore
remains a broad flat apron; this pass does not solve its physical composition.
R3 shoreline and matching before/R3 close woodland captures were also inspected.
The close view exposed the checker above and triggered the R4 correction.
The final R5 native close woodland image was inspected: the repeated leaf-grid
motif is no longer visible, crown colour is quieter and the bark furrows read.
The same frozen executable retains the flower/stone models and gravel changes.
The large leaf-block silhouettes remain simple; this is not a claim to match
the old-growth/reference anatomy or the complete requested art direction.

R3 moving-camera capture: `after-r3-meadow-motion/walk.mp4`. Native 20 fps
receipt: 228 real frames, 12 repeated frames, no late frames, finalized encoder.
Extracted frames 1, 3 and 6 were inspected. The existing route includes a pan
and aerial section; this is camera-motion evidence, not collision/traversal,
edit/reload, low-quality acceptance or a full-rate shimmer review.

Build activity overlapped some captures and scene detail changed concurrently.
No isolated performance gain or 16.67 ms frame-budget acceptance is claimed.
The optional Vulkan validation layer was unavailable; no shader or bind-group
validation error was reported in the completed captures.

Frozen executable SHA256:

- Before: `8C6FD9E38B46F165C958BAB2AD7A2534BE1408E863E4D50375CB24AF79826870`
- R3: `B8E2FA7D4F46F0ADFC6CEACF253092C627D34ED383300CCEECCDDDCA773739E0`
- R4: `F26647433F3DCBD358A25665884032E6257568F1C8AA55C6109EBC71C6515371`
- R5: `1BC5787302C90BEFC37BFE08C1571923BEC15CAA368A39E60FABA3354B0BBD47`

Architecture/landform composition, broader biome/weather/lighting review,
larger old-growth silhouettes and target-device performance gates remain open.
