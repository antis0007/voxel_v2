# Mining and excavation drainage

The hand tool prepares the aimed-at source before a click or held swing. A completed
break keeps its progress until the target changes or input releases, allowing
residency and admission retries without another full swing. Refused breaks do
not dirty intact meshes. Foreground source invalidation retains the cached
terrain so the existing urgent mesh lane can patch it directly.

Excavated porous ground keeps capillary-bound moisture in its persisted wet
residue. Only moisture above the material's retention limit seeps into free
water, limited by drainage conductivity and face area. Exposed residue can dry
through the existing evaporation ledger. Saturated porous walls can seep into
adjacent open cells, providing finite drainage into trenches.

Environmental worlds move free water in exact microlitres, rather than using
rounded eighth-cell heights for equalisation. Falling takes priority over
sideways spreading. Thin films look for a connected lower outlet within four
cells, with a shared 48-node bound. Sealed flat basins retain their water;
outlets move existing water and suspended sediment without creating sources.
The existing bounded hydrostatic connection remains available. Legacy worlds
retain their previous water solver; the isolated CA owner is unchanged.

No save migration or new material rates are introduced. Existing free water is
preserved; this changes subsequent excavation and flow, rather than erasing
water already released into a save.

Validation receipts: `target/mining-drainage-check.log` and
`target/mining-drainage-check.exit`. The integrated build receipt is
`target/mining-drainage-build.log`. No tests or hosted CI were requested or run.
Build/check evidence does not establish input-to-display latency, frame time,
mining feel, or visual drainage acceptance in a running world.
