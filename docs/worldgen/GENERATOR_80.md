# Generator 80: Manyfold maze districts

Requested 2026-10-29 after the open-maze notes intake. This expands the existing
dry Manyfold tube; other world types keep their existing generation paths.

- 24 angular sectors by 6 longitudinal rows: 144 maze chambers.
- Seeded depth-first spanning tree with extra loop doors; angular neighbors
  wrap across the seam. One fixed-size layout is cached per generation worker.
- A six-metre circumferential street, six axial streets, open courtyards and
  pavilion interiors are reserved before walls. Passage doors are four metres
  wide at the nominal floor radius; voxelization still needs movement review.
- Garden, limestone court, archive, opposite copper workshop, grove and basalt
  district use existing materials. Crowns and pavilions face the local hollow.
- Solid floors and the existing outer safety shell remain. The shared radial
  field and controller are unchanged. No fluid or machine is given pretend
  radial behavior; workshop furniture is ordinary geometry.
- Generator versions through 79 retain the original tube block sampler. Start
  a new **Manyfold Tube** world to use this layout; existing saves are not migrated.

This increment supplies terrain and connected passage topology. It does not
add bent tubes, layered return routes, folded doors, an optical puzzle or new
inhabitants. Those still need their own mechanics and runtime acceptance.

Source: `crates/worldgen/src/manyfold/districts.rs`.
Input: `downloads/documents-2026-10-29/Manyfold_Reachable_Districts_And_Gravity_Expansion.txt`.

Validation: worldgen library compilation and the final offline `release-fast`
game build passed. Executable: `target/voxel-manyfold-districts.exe`; build log:
`target/manyfold-districts-final-build.log`. No tests were requested or run;
walking, camera comfort, visual quality and two-player traversal remain unverified.
