# Voxel engine audit and refactor

The first priority is reliable world state and bounded work: terrain must load before movement starts, obsolete jobs must not overwrite edits, and queues must not grow without limit. This implementation addresses those foundations, adds local save and resume, and makes rendering controls usable. It does not establish that every defect has been found or that the engine has reached production quality.

## Implemented changes

All statuses below describe source changes. Runtime acceptance is listed separately.

| Priority | Finding and consequence | Implementation |
| --- | --- | --- |
| P0 | No persistence; leaving a world loses every edit. | Added versioned local quicksave and resume in `src/world/storage.rs`. Saves include edited chunks, camera and player size, flight, day cycle, and cloud controls. |
| P0 | Movement begins before spawn meshes arrive. | A new world waits for the 27 neighboring spawn chunks to upload. A loading screen shows actual completed chunk counts. Streaming continues without advancing physics. |
| P0 | Spawn selection assumes space above terrain is clear. Trees, neighboring slopes, and smaller voxel scales invalidate this assumption. | Candidate selection checks the complete standing body and supporting footprint against sampled blocks, rejects water, and accounts for voxel scale. If the dry search is exhausted, world creation generates a small dry support area with full standing clearance. |
| P0 | Unbounded mesh work and result channels retain stale world data and mesh allocations. | Bounded channels, 64 outstanding mesh jobs, cancellation tokens, and cancellation before and after meshing. World reset preserves monotonic job identity. |
| P0 | Failed GPU terrain readback leaves a chunk permanently requested. | Failed completions release request ownership and return the chunk to the scheduler. Stale callbacks count toward the collection budget. |
| P0 | Fallback scheduling bypasses normal capacity checks and repeatedly favors the same entries. | One admission path applies generation and meshing limits. A rotating scan of at most 64 entries repairs missed requests through the same priority heap. |
| P0 | Shader errors can ship unnoticed until GPU initialization, where validation takes the default panic path. | `build.rs` parses and validates all 12 active WGSL programs using Naga 27.0.3 and the exact source composition used by the renderer. Staged renderer initialization also captures GPU validation errors through `Result`. Device-specific pipeline and driver behavior still require runtime acceptance. |
| P1 | Large renderer file owns residency, terrain generation, meshing, visibility, and graphics passes. | Extracted the streaming owner, GPU terrain generator, and visibility logic into separate modules under `src/render/streaming`. |
| P1 | Frame delta is truncated to 33 ms, slowing simulation when rendering takes longer. | Added a 120 Hz fixed simulation clock, with at most 16 catch-up steps per frame. Physics, tool cooldowns, and world time use the simulated elapsed time. Extreme stalls intentionally discard excess time. |
| P1 | Collision checks only the destination and can tunnel through a voxel. | Movement checks segments no longer than 0.45 voxel and advances to contact using binary refinement. Terminal fall speed bounds traversal cost during long falls. |
| P1 | Jump support uses one block and treats any non-air material as ground. | Jumping uses collision-established grounded state. Sprint no longer overrides sneak speed; automatic stepping requires grounded movement. |
| P1 | Standing up can expand the body into a ceiling. | Uncrouching checks the candidate body before increasing its height. |
| P1 | Zero ray components at exact voxel boundaries produce `0 * infinity`, contaminating DDA traversal. | Parallel ray axes explicitly retain infinite crossing times. Invalid rays are rejected and traversal length follows reach rather than a fixed 256 steps. |
| P1 | Focus and menu transitions retain stale placement and movement state. | Gameplay input resets clear all held actions and modifiers. Clicking the menu background no longer creates or resumes a world. |
| P1 | Terrain column bounds never evict during exploration. | Column bounds evict outside the retained residency range. |
| P1 | Cached bounds include distance-dependent underground culling, so moving closer does not restore caves. | Cache raw column bounds; apply distance culling using the current center. Distant bounds retain the lowest sampled surface instead of deriving a lower bound from the highest peak. |
| P1 | Returning to a contracted desired set hides otherwise valid resident meshes. | The draw list uses retained GPU meshes within render distance and applies frustum culling independently of scheduling expansion. |
| P1 | Edited chunks outside procedural terrain bounds disappear from residency selection. | Snapshot-aware selection includes edited chunks, and dirty source chunks are admitted immediately even while the player stays in the same chunk. Elevated construction no longer waits for another expansion ring. |
| P1 | LOD seam state is recorded while a different seam job is already running. | Removed height-shell skirts and seam remeshing. Every LOD retains source voxel coverage and uses unit-spaced chunk-edge vertices. Completed LOD variants are reused from a bounded cache. |
| P1 | Coarse water meshes fabricate sea-level surfaces for underground or empty chunks. | All levels emit actual exposed liquid faces from the 3D source volume. No coarse mesh invents a sea-level plane. |
| P1 | Snapshot overlay visits every voxel of neighboring edited chunks, including data outside the one-voxel halo. Each write rescans a column. | Copy only intersecting halo ranges. Height caches and their column rescans have been removed with the height-shell mesher. |
| P1 | First editing a chunk recalculates terrain columns at every vertical voxel. | Reuse one terrain column sample for each XZ position during CPU generation: 1,024 column samples instead of 32,768 per chunk. |
| P1 | Collision and picking repeatedly recalculate procedural blocks. | Added a fixed 16,384-entry procedural query cache. Edited chunks take precedence and configuration changes invalidate the cache. |
| P1 | Nearby torch collection scans entire chunk volumes every frame. | Edited chunks maintain a torch-position index. Nearby collection visits indexed positions and orders equal-distance lights deterministically. Torch radius and collection distance now use consistent meter units across voxel scales. |
| P1 | Near-frustum extraction uses the OpenGL depth convention with WebGPU projections. | The near plane uses clip Z directly for the zero-to-one depth range, including shadow caster culling. |
| P1 | Manual bilinear samplers clamp integer coordinates before computing interpolation weights. Border samples incorrectly blend into the next texel. | Clamp continuous texel coordinates first in atmosphere, world, cloud, and composite shaders. |
| P1 | Upload work uses counts alone; several expensive meshes can occupy one frame. | Retained vertex and index budgets and added a 3 ms elapsed check between uploads. A single upload is not preemptible. |
| P1 | View distance cannot go below 12 chunks and permits 96, a large quadratic residency range. | Default sixteen chunks; range four to 64, with performance, balanced, and far-view presets. The UI displays the physical radius and explains the cost of long distances. |
| P1 | Cloud and sky motion continue while the world is paused. | Sky motion follows gameplay time. |
| P1 | Continuous redraws and unsynchronized presentation consume work in menus and hidden windows. | VSync defaults on, can be changed in graphics controls, and is disabled for the benchmark path. Occluded or zero-sized windows stop rendering; unfocused rendering is limited to ten requests per second. |
| P1 | Nearly every logical CPU is assigned to meshing, competing with the event thread and driver. | Worker count leaves two logical CPUs available where possible and caps the pool at eight. |
| P1 | The pause toolbar places too many controls in one horizontal row. | Replaced it with a compact wrapping status strip and a scrollable settings menu. Advanced rendering controls and time/weather settings are collapsed into the pause menu. Loading progress fits narrow windows. |
| P1 | UI reports fluid physics enabled even though the backend always disables it. | Disabled the unavailable UI control and reconciled reported state with the backend. This fixes reporting; fluid simulation remains unavailable. |
| P2 | World snapshots refresh only for edit revision changes. | Snapshot configuration also participates in freshness checks. No-op single writes share the batched editing path and out-of-height edits are rejected. |
| P2 | Chunk-corner edits invalidate face neighbors but omit diagonal ambient-occlusion dependencies. | Dirty propagation covers every neighboring chunk touched by the complete one-voxel halo. |

## Volumetric terrain LOD and streaming

Every level now represents a full 3D voxel volume. Levels one through three build a bottom-up hierarchy with maximum uniform cell widths of two, four, and eight voxels. A parent collapses only when all eight children hold the same material and their visibility semantics permit merging. Mixed solid/air cells, material boundaries, and cutout details recurse to source voxels. There is no majority-material downsampling or height-column approximation that can seal a tunnel, erase a thin wall, or remove an overhang.

The hierarchy emits only faces exposed by the original neighboring voxels, including the exact one-voxel halo. Greedy planar merging operates on these coverage masks. Near detail keeps source tint; distant detail uses mean periodic material tint and progressively larger tint/AO patches of eight, sixteen, and 32 voxels. Untinted flat surfaces can still merge fully. Foliage and torch geometry remain present at every level. Geometry occupancy and material identity stay exact; distant tint and AO detail are deliberately simplified.

Shared chunk edges are subdivided at unit voxel positions, constructed with integer coordinates before float conversion. A greedy rectangle touching an edge receives a conforming triangle fan where necessary. All levels use the same edge contract, so neighboring detail levels do not require skirts, neighbor LOD signatures, or seam remesh jobs. The mesher and visibility selection retain elevated edited chunks, and edits above procedural bounds immediately enter streaming without waiting for movement.

Streaming now includes a separate player-height band beneath the terrain surface, including distant columns. It does not fill every vertical chunk between a deep cave and a mountain summit. Terrain surface bands use the world vertical bounds independently of camera height, overlapping bands are deduplicated, and vegetation margins use meters rather than a fixed voxel count. Ocean surface chunks are explicitly included above deep seabeds.

| Resource or work | Current policy |
| --- | --- |
| Inactive completed GPU LODs | At most 64 MiB and 512 entries, with oldest-use eviction. Dirty source or halo edits invalidate every cached variant for those coordinates. |
| Mesh worker heap and completed results | Configurable shared 256 MiB default, reserved before construction; includes preallocated output and 4 MiB scratch per active build. No oversized-result exemption. |
| Terrain generation | At most 64 outstanding jobs through bounded CPU workers using the same generator as collision and edits; cancellation remains charged until completion. |
| Terrain completion | At most 16 results, with a two-millisecond elapsed check between integrations. |
| Terrain scheduling | Up to 16 jobs, reduced to eight or four under downstream pressure. Outstanding job capacity also gates admission. |
| Empty GPU meshes | Shared vertex and index buffers, rather than fresh dummy allocations for every empty chunk. |

The active terrain path is CPU generation shared with collision, picking and edits. The older GPU terrain shader remains inactive; earlier GPU pool/dispatch optimizations are historical, not the current generator architecture.

Completed old LODs remain useful cache entries instead of replacing a newer desired active mesh. Source generation survives changes that affect only detail level. Queued jobs retarget to the current desired level before meshing; busy heap entries no longer repeatedly pop and reinsert ahead of other coordinates. Ready queues sort in place, and expansion does not rebuild the full pending heap just to reapply an unchanged distance limit.

The performance panel now reports LOD reuse, cached voxel bytes, active and inactive mesh-buffer bytes, pooled generation-buffer bytes, and completed-mesh allocation bytes. These are owned resource sizes, not total process memory or driver-reported VRAM. Frame-time and visual gains require runtime measurement; conservative LOD intentionally retains expensive irregular geometry when further merging would change its features.

## Residency budgets and flight rendering

Regenerable CPU voxel residency now defaults to **256 MiB**, configurable from 32 to 2048 MiB. GPU terrain residency defaults to **512 MiB**, configurable from 64 to 4096 MiB in Advanced rendering. The GPU allowance covers active meshes, inactive LOD meshes, shared empty buffers, and a reserved **1 MiB** for shared empty mesh buffers. Other render targets, shadows, cloud resources, upload staging, driver-retired allocations, and allocation metadata are outside that allowance. These are application-owned resource budgets, not an operating-system RAM or VRAM guarantee.

CPU source arrays carry an allocation owner shared with workers. Evicting a cache entry does not pretend that a worker-held allocation was freed. Dirty sources update in place only when uniquely owned; a worker-pinned source is regenerated from the current edited-world snapshot rather than cloned outside the budget. Source eviction sorts candidates once, keeps nearby data, and uses a 7/8 low-water mark to avoid full-cache scans for every arriving chunk. A reduced limit drains unpinned data immediately; outstanding worker references remain counted until released.

GPU admission happens before buffer creation. It trims spare LODs, evicts lower-priority active meshes using a stable distance ordering, and rejects candidates that cannot fit without displacing more important terrain. Rejected coordinates retain their known size so the repair loop does not repeatedly regenerate and remesh them. Movement, edits and budget changes reopen admission. Tight budgets can reduce terrain coverage; they never delete persistent edits or replace cave/build geometry with height surfaces. The performance panel reports usage against limits, deferred chunks and eviction totals.

Mesh workers and completed results now share a pre-construction heap allowance. Edited chunks use a separate disk-backed cache target. These owners remain distinct from the regenerable CPU voxel-cache budget; details follow below.

Uniform air and opaque chunks, including their complete halos, now bypass CPU source residency and mesh workers. They install a zero-triangle resident record using shared GPU buffers, preserving spawn completion and edit invalidation. Workers release source references immediately after mesh construction, before waiting for completed-result capacity.

The default atmosphere is now the scattering sky-view path. The LUT used extinction coefficients per kilometer while integrating meter distances, disagreed with its sampler about sun-relative azimuth, and never refreshed for flight altitude. Those defects are corrected. Horizon-focused elevation mapping, periodic azimuth interpolation, a stable sphere intersection, ground-reflected radiance below the horizon, planet occlusion of celestial disks, and matched aerial-perspective coordinates replace the blank or discontinuous lower hemisphere. Lighting changes are staged and the sky solve runs after the current camera uniform is written. Horizontal flight does not rebuild the sky LUT; altitude changes of one meter or lighting changes do.

The current material rendering includes face-specific procedural grass, soil, stone, sand, bark, foliage, snow, water and lava, with derivative-filtered detail and sky reflections. Existing concurrent material work is retained. The integration corrects the rough-diffuse angular term, protects singular half vectors, applies the incident specular cosine, filters ripple highlights using normal variance, and fades the outer terrain boundary into the same sky radiance for downward as well as horizontal views. The lower hemisphere is atmospheric ground radiance, not a substitute terrain mesh or a claim that unstreamed caves/builds are rendered.

Visual quality, GPU time, memory-pressure behavior and flight acceptance have not been observed in a running game. Local compilation confirms shader and Rust validity only.

## Edited regions and mesh-worker heap admission

Edited chunks now page to append-only 8x8x8 region files under `saves/regions/<world-id>/`. Background I/O has a maximum of eight outstanding operations, bounded completion processing, near-first prefetch, and far-first writeback. Repeated active edits coalesce for 750 ms; out-of-window or memory-pressure writes bypass that delay. The region index shares immutable per-region maps so an update copies at most the affected 512-entry region plus the outer directory, rather than all historical edited chunks.

Each immutable record contains chunk coordinates and revision, compressed blocks and light, partial water levels, and a checksum. Data is synchronized before its index entry is published. Stale write acknowledgements cannot clean a newer edit; stale reads cannot replace dirty resident state. A per-region append lock prevents concurrent writers from calculating overlapping record offsets. Interrupted lock files are preserved for explicit recovery rather than deleted speculatively.

The edited cache targets **64 MiB**, adjustable from 16 to 1024 MiB. Admission uses decoded byte estimates including torch indices and water maps, not compressed file size. A small decoded-read cache is bounded by bytes and entry count. Nearby collision data, unsaved changes and outstanding writeback remain pinned. Consequently this target can be exceeded while editing or after I/O failure: preserving player work takes priority over eviction. Immutable snapshots and bounded I/O can temporarily retain additional allocations.

Terrain scheduling and completion both require the edited source and its full one-voxel halo to be resident. Known cold edits remain in visibility selection, including elevated construction. They are never silently treated as procedural terrain. Cold collision/picking or direct edit queries have a bounded cached, synchronous correctness fallback; ordinary terrain prefetch and writeback use the I/O worker. Read failures conservatively block access and editing rather than overwrite saved data; the UI reports storage errors. Worker loss retains dirty data, and an explicit paused-menu save can fall back to direct writes.

Mesh heap admission now happens **before** construction. A cancellable, allocation-free source scan computes conservative output limits for visible unit faces, conforming edges, water and torch geometry. Output vectors are preallocated within the reservation; each active build also reserves **4 MiB** for hierarchy/mask/cell scratch. The default shared allowance is **256 MiB**, adjustable from 160 to 2048 MiB; the minimum admits the worst supported single-chunk geometry. The oversized-result exemption is removed. Admission is FIFO so a large nearby chunk cannot be indefinitely starved by a stream of smaller meshes.

The same lease follows the mesh through workers, result channels and upload queues. Cancellation, failed jobs and resets release reservations after the corresponding buffers are dropped. Small completed meshes compact within the already-reserved scratch headroom before returning unused capacity; large meshes retain their charged capacities. A zero-face preflight bypasses construction. The budget covers mesh buffer/scratch heap allocations, not source voxels already charged to the voxel cache, OS thread stacks, allocator metadata or unrelated renderer resources. Reducing a limit stops new admission while existing reservations drain.

Mesh-thread startup errors now propagate through renderer initialization. A panicking mesh job releases its reservation and returns a retryable failure. Pool disconnection attempts a rate-limited background restart; it never invokes synchronous meshing on the event thread.

Checkpoints now use a checksummed **VXWSAV10** manifest, including factory and adventure session state. Each record includes a segment generation, offset, checksum, decoded-size estimate and edit version. V1–V9 saves remain readable; older expedition state is retained. Keep the entire `saves` directory when moving a world.

### Asynchronous checkpoints, recovery and compaction

The old menu-thread flush and persistent create-new lock files are removed. One bounded checkpoint worker captures an immutable region index, dirty chunk Arcs and matching fluid values. Array pinning and copied fluid data have a conservative 128 MiB capture admission limit; over-budget captures request background writeback and retry at most every 100 ms. Main-thread capture still visits dirty metadata and copies its fluid entries; it performs no file writes or syncs. Checkpoint records flush once per touched region, then publish together. Normal paging writes remain independently durable before eviction.

Autosaving defaults to 30 seconds, with an initial checkpoint after one second in a new/resumed world. Pause settings allow disabling autosave or choosing 10–300 seconds. Manual save requests coalesce behind the single active job. Closing or choosing Save and quit pauses gameplay, waits for a fresh final checkpoint and keeps the world open on failure. Minimized windows continue persistence maintenance. Checkpoint acknowledgements apply at most 64 entries / approximately 2 ms per event-loop iteration, reject older versions, and match the exact world instance so an old save cannot alter a reload.

Each world retains three immutable committed manifests. Manual and autosave discovery slots each retain a predecessor; the immutable commit is written and synced before slot replacement. Resume ranks all valid committed generations and slots, so a completed commit survives an interrupted slot update. It rejects checksum failures and missing/truncated extents, falls back to an older complete checkpoint and reports recovery in the menu. Record payload checksums are validated on demand. Partial temporary manifests and uncommitted append tails are not replayed; changes after the last complete checkpoint can be lost on a crash.

An OS-backed ownership lock stays alive with region snapshots and releases when the owning process exits. Region worker jobs catch failures and disconnections trigger rate-limited background restart. Queued terrain reads take priority, with a write after four reads to prevent starvation. Both streaming write batches and checkpoint capture partition fluid data once, replacing repeated scans for each chunk. The I/O worker no longer pins its initial full region index.

Compaction chooses regions with at least 1 MiB of physical data and more than twice their live compressed size. It verifies and copies record bytes into a fresh generation without regenerating voxels. Work is limited to 16 MiB and at most two completed regions per checkpoint; larger regions resume across subsequent checkpoints with a single record-sized copy buffer. Newer edits are preserved. Only a fully synced segment is published, and subsequent writes switch to that generation.

Obsolete segments are now reclaimed during an active session. Each record carries a shared generation lease; copy-on-write indices, WorldSnapshots, queued reads, write completions and checkpoint deltas keep it alive. Compaction pins both its sources and unpublished output. Active append targets retain a separate lease, and completed compaction switches that lease to the new generation. Leases also retain OS ownership independently of the store.

A single bounded persistence worker schedules collection approximately once a second when no save is pending, including with autosave disabled. The collector protects every retained commit and both discovery slots/predecessors, holds the discovery-pointer lock through unlink, and shares the writer gate for lease registration versus deletion. Expired weak leases cannot be resurrected while unlink is in progress. A file is eligible only when it has neither a retained manifest reference nor a live lease. Retained recovery history intentionally takes precedence over disk-space recovery.

Directory enumeration resumes across passes: at most 128 entries and 16 unlinks, with an approximately 4 ms sweep budget checked between filesystem calls. Manifest references are cached by length and checksum trailer so an unchanged large save is not repeatedly decoded. Damage or unreadable retained metadata defers collection; failures back off for 30 seconds. Permission-denied files remain for a later pass. Only exact owned region filenames are considered; abandoned world directories are preserved. The old reopen-only generation cutoff and synchronous load-time garbage sweep are removed.

Streaming clean-cache eviction now handles at most 32 chunks per pass, accounts for chunk bytes once and filters fluid entries once for the whole batch. Diagnostic storage text includes reclaimed bytes. These changes reduce repeated work; their frame-time and disk-space effects have not been measured.

### Player-facing interface

The creation and loading screens now use a painted landscape composition, custom landscape choices, a clear primary action and responsive scrolling. Pause navigation replaces the old floating settings window and persistent diagnostic bars with Journey, Journal, Display, Controls and World pages. Display and input controls retain their existing live behavior.

The journal presents the actual expedition objective, completed milestones and live resource quantities. In play, location/time, the current objective, contextual interactions and short discovery notifications have separate positions. Diagnostics remain behind F3. Inventory receives a custom heading, hotbar controls expose accessibility labels, and painted menu controls retain egui keyboard focus and widget metadata. The egui renderer/input backend remains in use; player screens have their own presentation instead of the default window chrome.

No visual or interaction session was launched for these UI changes. Layout, readability, focus order, small-window behavior and gameplay feel still need runtime acceptance.

No save round trip, injected crash, pressure flight, timing measurement or gameplay session was run for this persistence change.

## World library, flight coverage and asynchronous planning (2026-10-18)

The launcher now has **Your worlds**, **New world** and **Preferences**. A background worker groups checkpoint headers by stable world identity, keeping discovery off the UI thread. Painted virtualized cards expose search, recent/name/favorite sorting, editable names, favorites, seed, landscape, voxel scale and distinct recovery checkpoint counts. Loading is asynchronous and validates complete manifests/region extents before replacing the current world; older complete checkpoints are tried only for the selected world. Discovery reads bounded headers, not every region payload. Named metadata is checksummed and written atomically under a per-world lock. Existing saves keep their terrain generator version.

FOV, sensitivity, inversion, HUD, VSync, FXAA, atmosphere, view/detail settings, all four memory targets, autosave policy, creation defaults, fullscreen and the last usable windowed size persist in `saves/preferences.vxp`. Writes debounce for 500 ms and coalesce on one worker; a validated predecessor is retained. Invalid or future preferences do not silently replace valid data during startup. Write failures remain visible and retry in the background. Normal exit waits for pending successful writes. Benchmark mode does not load/write user preferences or start world-library discovery.

Two source defects explained missing water/ground while flying: column visibility stopped at the seabed/tree margin even when sea level was higher, and desired/retained terrain was vertically clipped around the camera. Sea-surface chunks now participate in the column range and horizontal residency is independent of camera altitude. This changes coverage, not saved voxel generation. The camera's actual far plane and memory budgets still limit visibility.

Visibility planning no longer evaluates eroded terrain columns synchronously on the frame thread. One or two column workers share bounded 64-entry work/result channels, with at most 64 outstanding jobs and a 32,768-position deduplicated backlog. Results integrate under a one-millisecond/64-item allowance, and enter the normal nearby-first terrain scheduler. Configuration epochs and current-interest checks discard obsolete work after world changes or fast travel. Worker startup/disconnection retries back off; there is no synchronous erosion fallback. Raw column cache/backlog entries evict outside the retained range.

Erosion tile lookup now uses shared keyed single-flight initialization: adjacent workers join the same tile calculation. A 256-entry shared cache retains completed tiles, with in-flight entries kept until initialization completes; each worker also keeps eight hot tiles, avoiding repeated shared locks in per-column sampling. Terrain formulas are unchanged. Conservative volumetric meshes still preserve actual cave, overhang and build occupancy.

The default radius rises from eight to sixteen chunks; controls now permit four to 64. The maximum is 2,048 m at 1 m voxels, 1,024 m at 0.5 m and 512 m at 0.25 m. A larger target does not bypass CPU/GPU/mesh admission or establish fast massive-world loading. Streaming throughput, pressure behavior and flight appearance remain unmeasured. The separately integrated HDR/atmosphere/cloud depth changes are preserved; no visual approval is claimed from compilation.

## Spatial world simulation (2026-10-18)

The local-only solver has been replaced by a graphics-independent `simulation::WorldSimulation`. Sparse persistent chunk masks, rotating cell cursors, near/fair scheduling, source-completion waiters and a separately budgeted asynchronous source pool continue active water/gravity beyond the old 24 m cutoff. Pumps, drains and drills use the same residency path beyond their former 64 m cutoff. Rules do no disk reads or procedural chunk generation on the frame thread; conservative changes commit only after source preparation. Activity capacity and edited writeback pressure apply backpressure instead of dropping work.

VXWSAV10 retains pending cell masks/cursors, compact deferred regions, emergency sweep and recovery state; V1-V9 remain readable. A bounded background rescan resumes old stored activity. Partial volumes are chunk-partitioned, near render snapshots avoid remote-only invalidation, region discovery uses a spatial directory, index publication clones partitioned roots, and dirty-source overlays visit intersecting halos. Remote writeback coalesces ordinary hot edits rather than syncing immediately because of distance. See [the scheduler implementation and limits](WORLD_SIMULATION_SCHEDULER.md).

Local compilation does not establish simulation throughput, save round trips or gameplay acceptance. The scheduler budgets active cellular work; pressure, thermal reactions, dynamic bodies and parallel rule execution remain future work.

## Ownership after the refactor

- `build.rs` and `src/shader_sources.rs`: shared shader composition and compile-time WGSL parsing and validation.
- `src/mesh/lod.rs`: adaptive 3D cell hierarchy and planar detail reduction.
- `src/mesh/boundary.rs`: unit-spaced shared chunk-edge tessellation.
- `src/simulation.rs`: fixed player simulation timing and bounded catch-up.
- `src/simulation/world.rs`, `src/simulation/residency.rs`, `src/simulation/cellular.rs`, `src/world/activity.rs` and `src/world/water.rs`: persistent spatial activity, fair service, asynchronous sources, conservative rules and chunk-partitioned partial volumes.
- `src/render.rs`: renderer resource initialization, frame passes, and presentation.
- `src/render/streaming.rs`: request ownership, cancellation, capacity control, mesh workers, and upload.
- `src/render/streaming/terrain.rs`: authoritative CPU landscape workers, cancellation and completion identity.
- `src/render/streaming/visibility.rs`: terrain bounds, underground streaming bands, desired residency, LOD selection, and frustum culling.
- `src/render/streaming/lod_cache.rs`: bounded reuse of inactive GPU meshes.
- `src/render/streaming/mesh_budget.rs` and `src/mesh/memory.rs`: pre-construction heap reservations, output bounds, and result lifetime.
- `src/world/collision.rs`: player collision and voxel picking.
- `src/world/sample_cache.rs` and `src/world/generation_cache.rs`: bounded procedural query and worker dependency caching.
- `src/world/landscape.rs`: versioned metre-space terrain, shared single-flight erosion tiles and worker-local hot tiles.
- `src/render/streaming/columns.rs`: asynchronous, bounded and cancellable column visibility planning.
- `src/world/storage/library.rs`, `src/egui_ui/world_browser.rs` and `src/preferences.rs`: world discovery, named metadata, scoped recovery and durable user preferences.
- `src/world/storage.rs`, `src/world/storage/background.rs`, `src/world/regions.rs` and `src/world/regions/maintenance.rs`: versioned checkpoint manifests, background autosave/recovery, immutable region records, resumable compaction, writeback and prefetch.
- `src/egui_ui/settings.rs`: shared graphics settings.
- `src/egui_ui/front_end.rs`: painted launcher/loading/pause screens, journal, objective HUD and contextual notifications.

## Save and resume

Pause and choose **Save progress**, or rely on the configurable background autosave. The launcher opens **Your worlds**, with named worlds, search, sorting and favorites. Opening a world selects its newest complete checkpoint across manual, automatic and recovery commits. Recovery stays within the selected identity. **Save and quit** and the window close action wait for a final background save. **Leave for the main menu** keeps completed/in-flight saves and does not request a new checkpoint.

Manual and automatic slots are `saves/quicksave.vxw` and `saves/autosave.vxw`; their predecessor slots and per-world `regions/<identity>/checkpoints` provide recovery history. These paths are relative to the launch directory and excluded from Git. Creative hotbar/tool configuration, weather-grid contents and GPU history are not persisted.

## Remaining priorities

### P0 Runtime acceptance

Rust compilation cannot establish device-specific shader pipeline validity, driver behavior, frame pacing, visual quality, save round trips, or gameplay feel. The saved `run_error.txt` records an earlier cloud shader failure involving `sheet`; that identifier exists in the current source, so the historical log alone does not establish a current failure or a repaired runtime.

The new emergency support area is reserved for exhausted spawn searches. Saved positions still need collision-aware recovery if generation compatibility changes.

### P1 World streaming and renderer architecture

1. Profile and refine pressure behavior across the implemented voxel cache, edited-region cache, mesh-worker heap and GPU terrain owners. Add process-wide coordination if needed; dirty edited data and snapshot pins intentionally take precedence over the edited cache target on I/O failure.
2. Extend the conservative volumetric LOD with error-bounded surface simplification if irregular terrain remains too expensive. The height-shell path is removed, mixed cells retain their source voxels, and chunk boundaries have conforming tessellation. Complex caves and intricate builds intentionally remain complex rather than disappearing.
3. Profile the shared CPU generator and dependency caches. Rendering, collision and edits now share one versioned generator; do not reintroduce an independently maintained distant/GPU terrain algorithm.
4. Split frame orchestration, render resources, atmosphere, clouds, world rendering, and UI composition into explicit owners. The main loop, renderer, and UI remain large despite this extraction. Add clear resource lifetime and resize contracts before introducing more passes.
5. Review the integrated HDR scene composition and shared display transform for consistent exposure and color under flight and extreme weather. Source integration does not establish visual acceptance.
6. Give clouds configurable sample and resolution budgets tied to GPU timing. The cloud storage types in `src/render/clouds.rs` remain unused scaffolding. They do not constitute sparse volumetric cloud streaming.
7. Complete conservative cloud reprojection with trustworthy volume depth and disocclusion handling. Current temporal anchors are approximations, especially inside cloud volumes. Visual stability remains open.
8. Extend the implemented mesh-pool and region-I/O error propagation and bounded background recovery to landscape workers. Coordinate process-wide pressure without synchronous fallback work on the event thread.

### P1 Durable worlds and simulation

1. Add explicit generator/material migration support. The named world browser and persistent user preferences are implemented. Measure compaction, reader-lease retention, collector throughput and capture cost for very large dirty/fluid sets. Background checkpoints, recovery generations, resumable compaction, live reader-safe collection and autosave policy are implemented.
2. Extend the implemented persistent spatial scheduler with simulation detail levels, fluid pressure, thermal/material reactions and conflict-safe parallel rule execution. Active work now continues beyond player/render residency, with bounded source loading, fair service and checkpointed wakeups. Profile overload and recovery before claiming massive-world throughput.
3. Resolve the two lighting models. The CPU flood-filled light array is not the active material lighting input, only spreads through existing edited chunks, and lacks removal propagation. The active renderer uses a limited nearby torch list instead.
4. Add interpolated presentation between fixed physics steps and retain short input edges across frames with no simulation tick. Rendering at high refresh rates can otherwise expose stepping or miss very brief presses.
5. Add camera-relative terrain coordinates. Cloud rebasing exists, but terrain meshes and player position still use floating-point world coordinates with limited precision far from the origin.

### P2 Product and maintenance

Finish responsive layouts beyond the menus changed here, rebindable controls, accessibility settings, asset-root resolution independent of working directory, runtime error presentation, and a coherent world identity. Remove duplicate and unused UI paths after replacing their remaining responsibilities. Existing dead-code warnings and older overlapping design documents remain cleanup work; they have not been silenced to imply completion.

## Verification

The world scheduler is checked with local `cargo check --offline --target-dir target` and `cargo build --release --offline --target-dir target`. The build validates the active composed WGSL shaders. Current diagnostics are `target/world-simulation-check.log` and `target/world-simulation-final-release.log`. The optimized review candidate is `target/world-simulation-review/voxel-workshop-v10.exe`; use the repository as its working directory for assets and saves. Changed owned modules are formatted directly. No tests were added or run, no CI was used, and no benchmark, game, physics-pressure or visual session was launched for this scheduler change. Throughput and runtime behaviour remain unmeasured.
Existing dirty renderer, shader, weather, and documentation work was retained. Previously deleted planning files were not restored or replaced.
