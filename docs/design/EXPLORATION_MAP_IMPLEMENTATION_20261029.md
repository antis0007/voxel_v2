# Exploration map implementation

Date: 2026-10-29. Source implementation; native visual and multiplayer acceptance pending.

## Integration update

The observation schema is now `VXOBS004`. Version 1/2/3 observation files remain
readable. Visual evidence is stored per voxel face, separately from footsteps,
with monotonic observation ordering. A re-observed empty cell leaves an ordered
absence record so chart reconciliation cannot resurrect an older wall.

The renderer supplies a weak identity for the exact source used by its installed
near mesh. Acquisition requires a matching resident source and published column.
Replacement, mutation, eviction and coarse LODs withhold acquisition; observation
does not pin the streamer's CPU allocations. This is an opaque-voxel adapter,
not GPU pixel/depth evidence or support for arbitrary rendered illusions.

Knowledge uses copy-on-write 8x8x8 pages. Checkpoints clone page handles; worker
threads serialize the data. Cold pages are checksummed immutable files beside
the personal archive, referenced only after a successful atomic write. Reads and
writes run in batches of four. Recently drawn pages are retained, and revisiting
archived space requests its pages without generating terrain. The resident target
is 65,536 patches, the hard resident limit 262,144, and the cold-page index limit
65,536 pages. Limits are conservative bounds, not measured performance promises.
Unreferenced cold files are retained; garbage collection is not implemented.
Requested reads take priority while there is resident capacity. Eviction keeps
only four candidates instead of sorting and cloning every resident page handle.
Unreadable archives remain untouched and show **Session only**; failed writes
show **Retry save** while retaining dirty observations.

View and light rays share an 8,192-cell work allowance and the 650-microsecond soft
deadline, checked within traversal. Both stop at known Manyfold presentation
thresholds. Draw work culls page bounds before inspecting at most 32,768 patches
and emitting at most 16,384 patches. No hidden-world remap queue is created.

Player traversal producers retain a bounded queue of movement receipts, separate
from transient audio flags. Manyfold and ordinary portal transitions produce
directed historical routes, including short folds. Walking between discovered
charts produces a directed connector. The route UI lists witnessed connections
and searches that history only; it never queries live gate destinations. Local
prediction receipts are not claimed to be server-confirmed topology transactions.

Movement connections now retain the last grounded departure across observed
airtime and complete only at a grounded arrival. Walking, ascent, descent,
swimming, flight and assisted movement have separate historical labels; ascent
and descent describe displacement, not an inferred ladder or jump mechanism.
No reverse connection is synthesized. A trace uses constant memory and resets
after a sample gap over one second, displacement over eight metres per sample,
incompatible gravity, a scope/observer change or an explicit portal crossing.
It also expires after 120 seconds or 512 sampled metres. Sparse samples cannot
be labelled as walking. Approximate distances are sampled path lengths.

Route search retains the actual edges so each step shows its movement kind.
Its result is a chain of recorded chart connections, not a navigation mesh or
proof of a continuous path between two entrance markers within the same chart.
**Walked only** excludes portal, airborne, swimming, flight, assisted and legacy
unclassified records; it is a historical filter, not a current traversability
guarantee. Distinct entrances between the same charts remain separate when
their endpoints are more than one metre apart. Legacy ordinary connectors are
labelled **Traversed**, because their archives did not record movement mode.
This remains frame-sampled local experience, not fixed-tick or server-confirmed
movement proof. The offline application check passed with warnings; see
`target/map-navigation-check-final.log`. No tests or native traversal review
were run for this increment.

Completing an inspection of a short sign/lectern inscription records a landmark.
Re-inspecting the unchanged physical record in the same personal world archive
can identify an earlier compatible walking chart. Similar-looking geometry alone
does not identify it. Aliases persist before reconciliation, which merges four
pages per maintenance turn and keeps the newest evidence per face. Long books,
reports and unknown instances are not recognition evidence. The current text-page
UI disables network sign editing, so this recognition path is presently solo.

Charts can be named. Explicit clipboard reports contain at most 1,024 observed
cells in local coordinates, never world coordinates, routes or a visit claim.
Imports are bounded, attributed as reports, and kept in separate scopes. Author
labels are self-declared. Report sharing has no automatic network transport.

The shared weight traverses authored Manyfold folds with a spherical clearance
check, retaining a single object and transformed velocity. Its integration is
bounded to 64 substeps with a matching speed cap. Invalid/out-of-volume positions
recover the same object to an unoccupied point above its cradle. Docking caches
only immutable baseline cells, still checking live materials before every edit.

The working tree includes substantial concurrent renderer, networking, worldgen
and UI changes. Source compilation of this checkout is not proof that HEAD alone
contains the integration. Keep ownership and staged work separate when committing.

## Implemented

Streaming no longer reveals columns. Both map views draw owned 3D observations from
`egui_ui/map/knowledge.rs`. The existing `map.vxm` remains selectable as **Legacy
survey**. Existing waypoints remain intact. The generator-backed **Depths entrance**
button was removed.

The current opaque-voxel adapter samples a bounded set of view rays through resident
render sources. It stops at missing chunks and opaque surfaces, uses the current
camera field of view, and qualifies appearances with daylight or the rendered light
set and a light-to-surface occlusion ray. It never generates or loads terrain. Known
Manyfold fold thresholds stop this adapter regardless of their hidden armed/link
state. Acquisition pauses behind menus and the full map.

Exact voxel coordinates and independent observed faces remain in storage. Local charts
use stable walking-plane origins and bases; the minimap follows the occupied plane,
and the full map lists only discovered charts. Ceilings do not paint over floors.
Walking records narrow travelled patches without granting visual colour in darkness.
Remembered colour changes only after a new qualifying observation. The brighter
overlay means **recent observation**, not exact current-frame visibility.

Explicit crossings and otherwise large discontinuous movement begin a provisional chart instead of selecting old
knowledge by destination coordinates. Its displayed coordinates are local. Global
waypoint distances are suppressed when the waypoint cannot be placed on that chart.
The distance rule remains a conservative fallback for relocations without a receipt.
Provisional charts display only annotations explicitly made on that chart; an old
waypoint at the same stored coordinate cannot identify an unknown arrival. Viewing
an old chart from an unlocalized arrival also withholds player-distance readouts.

Archives are personal to the explicit multiplayer profile, or otherwise the local
machine/account. Creative observations have a separate filename and never merge
into survival knowledge. Versioned RON files are size-checked and loaded/saved on
workers; stale world readers do not mutate replacement state. Corrupt archives are
preserved. Save requests flush observations, and the normal interval is 30 seconds.

The adapter attempts at most 64 view rays per render frame. Each traversal has a
512-cell hard limit, inside the shared cell/time budget. Charts remain capped at
2,048. Reaching a hard resident limit displays **Survey full** without deleting
previous discoveries. Acquisition, paging, chart work and UI still need profiling.

## Remaining requirements from the supplied brief

- Renderer-provided illusion facades, optical portal glimpses and their independent
  provisional charts. The voxel fallback must not be extended through these effects
  by reading collision truth.
- Server-confirmed crossing receipts and condition histories. Current directed
  routes record local traversal experience; prediction rollback is not reconciled.
- Richer continuous-slope chart grouping and floor labels learned from records.
  Directed movement connections and player chart names are implemented; complete
  fixed-tick movement evidence and native traversal validation remain open.
- Permission-checked multiplayer report transport. Clipboard import/export is
  explicit and personal; no map knowledge is currently sent over the network.
- GPU capture tokens and illusion/portal appearance adapters, texture reuse
  validation, cold-file garbage collection, and end-to-end profiling.

The supplied reference kernel was used as a semantic reference, not installed as
a tested engine module. Its test suite was not copied or run. An existing map test
was adjusted to stop asserting that loading a chunk reveals it; no tests were added
or executed.

Local validation: offline development and `release-fast` application builds completed with
warnings, including the expanded paging and recognition implementation. The final
offline application check also passed after the paging-priority, save-status,
annotation privacy and report-validation edits. Logs are in
`target/map-integration-build.log`, `target/map-integration-release-build.log`
and `target/map-integration-final-check.log`. The full binaries predate the last
small edits. The later navigation increment was checked separately as recorded
above; these earlier build logs do not validate its runtime behaviour.
No tests were added or run. Native review could not start: the computer-use
runtime failed to write its kernel assets (OS error 3), including after a reset.
No native or multiplayer acceptance is asserted here.

The source checkpoint contains the new knowledge/perception/receipt modules and
these design records. Integration edits remain in the shared working tree in
`main.rs`, `player.rs`, map UI, streaming publication, text-page inspection and
portal/Manyfold traversal. Their files contain concurrent changes, so they are
not included wholesale in this checkpoint. A checkout of the commit alone is
not the integrated application used by the build above.

The navigation check initially stopped at the concurrent storage-scanner call to
`factory::at_bench`. Its visibility was widened from `pub(super)` to `pub(crate)`
to match that existing call, and the next check passed. This one-line build repair
remains in the shared factory file; its unrelated changes are not part of the map
commit.

## Counterweight integration

`worldgen/manyfold` selects a closed wall in the existing seeded district maze.
`app/manyfold` docks the shared brass weight in a visible socket and clears only
cells still matching that wall's baseline material, at most 64 edits per tick.
Other solid construction blocks completion and is preserved. The usual terrain
publication path carries edits to meshes and peers. The latch travels with the
already-replicated weight state and is saved by version 46; old saves start unlatched.
After completion the latch prevents reopening the same wall over subsequent player
construction. No inventory is copied and no repeatable item reward is created.

The action keeps the existing rebinding-aware Use control and adds a focused
weight/socket prompt. A successful compile does not establish that guiding the
weight feels good, that the socket is visually clear, or that two-client/save/reload
behaviour has passed native review.
