# Submerged water dependency audit

Scope: volume-only changes at a cell that is `Water` with `Water` above in both
snapshots. Block/air topology, simulation volumes and save contents remain authoritative.
This is a source audit; this chat added or ran no formal tests.

| Consumer | Stored submerged fraction matters? | Finding |
| --- | --- | --- |
| Top geometry | No, when above is available | `append_water_mesh::volume` returns 1 when water is above and suppresses the internal top face. |
| Shared corner heights | No | `free_water_level` excludes submerged cells; neighbouring free surfaces use the same four-column mean. |
| Side/bottom geometry | No | Submerged cell height and submerged neighbour wall floor are 1; visibility depends on block topology. |
| Bed RGB | No directly | The bed walk and its block colours depend on blocks, not lower-cell fractions. |
| Optical depth | **Yes before the patch** | `water_optics` read the initial cell's raw fraction, even if water was above. This affects side/bottom vertices and corners shared with an adjacent lower free surface. Lower cells in the downward walk already count as 1. |
| Packed water metadata | Yes through optics | `encode_water_vertex` packs a quantized depth and sky/pin fields; a depth change can affect absorption/tint. Sky visibility and pinning themselves use block fields and positions. |
| Meshing path/tessellation | **Yes before the patch** | Any nonempty override map selected per-cell water instead of greedy water, whose deep top patches can span 2×2. Hidden overrides could therefore change vertices and wave interpolation for the whole chunk. |
| Coarse water | Same effective-height rule | Coarse solid meshes append the exact shared water mesh and encode optics at those vertices. |
| Occlusion plane proof | Only free surface fractions | Water colour output is opaque with depth writes. Full submerged geometry blocks independently of its optical tint. Camera-water/lava handling must remain conservative. |

## Narrow source changes handed back to the integration owner

`GeneratedChunk::water_optics` now counts the initial cell as 1 when its source
contains water above; otherwise it retains the stored fraction. `SolidPlan::new`
selects the partial path only for a stored level below 8 at Water with no Water
above. Both retain the existing source halo. No simulation/save values are removed.

These changes remove the interior-cell optics and path dependencies. They have
not been compiled or visually validated by this chat. The integration owner owns
production vertex/index/metadata equivalence fixtures and all build/runtime checks.

## Top halo limit: do not globally ignore the dependency yet

`GeneratedChunk::get_world` returns Air outside its padded source. A cell at
`p.y % CHUNK_SIZE == 0` is the top halo row of the chunk below. That lower source
can contain p but not p+Y, so it treats p as free even when authoritative world
data says water exists above. Its upper side-wall vertices sample p for optics,
and the conservative partial-path predicate can also see that override.

Therefore old/new authoritative `Water + Water above` alone is not sufficient
to ignore every volume-only update across all source consumers. Either retain
this boundary dependency, or canonicalize the transient mesh-source override map
using authoritative above-block reads, including outside the padded array, in
every source builder. Preserve real simulation and serialized volumes. Include
both adjacent chunks in any equivalence validation; interior-only fixtures miss
this case. Exposed fractional levels and drainage must remain visual dependencies.

## Scope-proof cache contract

Caching only free-water volume dependencies is independently valid for opaque
occlusion geometry if:

- Every encountered free Water position is recorded before its blocking test,
  including full cells with no override, so insertion/removal of a fractional
  override invalidates the proof in both directions.
- Water at p and above p are covered by existing block identity dependencies,
  including both vertical chunk layers across the y boundary. Cold unknown
  source data continues to fail conservatively without render-thread region IO.
- Every cache hit still checks current block identities and current effective
  stored values for those free positions. Camera-medium rules remain unchanged.
- Projection keys preserve exact float bounds and the eye voxel. Weak allocation
  pins remain alive until replacement; allocating new pins only on a miss is safe.

This does not authorize ignoring Air/Water topology changes or applying a stale
collision result. It does not establish bounded first-check latency.
