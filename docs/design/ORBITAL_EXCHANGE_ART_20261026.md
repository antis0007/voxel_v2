# Orbital structures and ground exchange: second visual pass

## Direction and references

Read ART_DIRECTION.md, PIXEL_ART_DIRECTION.md, the current NEW game bible,
WORLD_LORE.md and the architecture recovery composition rules. Documents supply
design constraints; the user's request supplies the implementation scope.

Searched Downloads recursively for named city, ruin, architecture and Titanfall
references. Inventoried 488 top-level raster files and reviewed contact sheets
for 118 landscape candidates. The 61-entry city/ruin/composition shortlist and
named recursive search are in `target/orbital-art-r2/references/`. This is a
bounded reference search, not a claim to have classified every nested asset or
every unnamed portrait-format image in Downloads.

Full-resolution study: `tu-bui-dmz-overall.jpg` and `jung-park-boomtown.jpg`.
Additional contact-sheet references include James Paick's Titanfall settlement,
Bruno Werneck's broken structure, Tu Bui's support-beam, wall-shell, aftermath
and switchback sheets; Dawnpu's city masses; Hubert Robert's inhabited ruins;
and the ruined cathedral. References remain in Downloads, not shipped textures.

Applied principles: tall structural bays, deep apertures, separated frames,
service galleries, warm practical light under cool masonry, legible former
purpose and selective collapse. Preserve negative space and routes instead of
filling surfaces with unrelated greebles or uniform random noise.

## Implementation

- Rounded, collared pressure modules replace box attachments on the crown and
  carriage. Service rails and standoff ribs reveal layered construction.
- Twin habitat drums gain separated longitudinal frames and service vessels.
- One shared orbital material function retains limited colour ramps and a
  metric pixel lattice. 3.2 m plates, connected wear, hatch outlines and stronger
  trim replace the barely visible 12.8 m pattern. Details filter by their size;
  broad plate tones persist after fine pixels become unresolved.
- Generator 64 adds the first corridor's editable exchange. It has a freight
  court, workshop, smelter, sheltered gallery, stairs and damaged eastern bays.
  Thin model fittings supplement the voxel structure; they do not own collision.
- Deploy near its existing console reclaims it for the existing receiver cost.
  Commissioning and freight retain the existing stock and momentum contracts.
- Render-only review commands are processed outside the simulation-step loop,
  so turning off physics no longer disables camera/capture control.

Only the first corridor has the new ground base. Old saves retain their terrain.
No boarding, moving-platform collision, new power network or wall-running was
added. Titanfall informs construction and ruin composition, not those mechanics.

## Native review

Evidence and frozen executable manifests: `target/orbital-art-r2/`. Seed 74,
generator 64, clear weather, 1280 by 900; indirect lighting and physics disabled.
These are native visual and production-action reviews, not FPS measurements or
walking/collision acceptance. No tests or hosted CI were added or run.

The first native run exposed review commands being gated by zero simulation
steps. R2 verified reclamation (`reclaimed=true`, `intact=true`), both generated
workshop block IDs, and paid commissioning through stage 2. Early screenshots
showing the loading screen are rejected evidence. R2's roof bracing and unlit
gallery also prompted another geometry/lighting refinement before final review.

R3 reviewed the continuous roof trusses from the ground, the lit west gallery,
the pressure-rim close-up and the framed twin drums. It also ran the restored
exchange through commissioning, lifting eight iron (160 kg), a mid-flight save
and exact reload (`progression_reload_exact=true`), lowering and collecting all
eight iron. Capacity returned to 77,513.016689 kg; the displayed momentum residual
was zero at the reported precision. These actions used production logic, with
only time and review inventory staged by the disposable fixture.

The final refinement adds recessed, mullioned crown windows and reduces grouped
torches to individual supported lights. Final build:
`target/orbital-art-r2/build-final.log`. The shared checkout contains unrelated
changes; each candidate's manifest identifies the executable actually reviewed.

Final native captures were viewed from `native-final/`: `ground-reviewed.png`,
`gallery-reviewed.png`, `rim-reviewed.png` and `court-dusk-reviewed.png`.
Capture commands waited for the resident terrain and horizon. The final close-up
shows dark window reveals and mullions; the ground views show continuous truss
chords and single supported lamps. Runtime window dimensions can differ from the
requested launch dimensions. R3's larger window captures are not matched A/B
comparisons with the final 1280 by 900 images.

`target/orbital-art-r2/release.json` confirms the reviewed frozen executable and
the release-fast executable have the same SHA256:
`6454F2D5C29E77071B38531506B370374845366B7FDF5D4AEDEE144CF387B381`.
The proof is a focused native visual and freight-action review. Route geometry
exists as collision-bearing voxels, but a full walking session and every lighting
or terrain configuration remain outside this review.
