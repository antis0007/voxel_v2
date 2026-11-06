# Vista coherence and frame budget

Owner request, 2026-10-29: an external review of the hills vista capture
(seed 74, generator 53, sunset) listed visible shortcomings and a milestone.
The owner asked for a plan and then for the fixes, with frame times well
above the recorded baselines. This document triages the review against the
engine as it stands, fixes the milestone, and sets the acceptance numbers.

Status: design of record for the `vista-perf` branch. Nothing here changes a
generator gate or a save until the slice says so.

## 1. Intent

The vista should read as one landscape from the player's feet to the
mountains, in plain daylight as well as at sunset, while the frame stays under
budget on the RTX 4090 and remains playable on the Intel UHD 770 tier.

## 2. Triage of the review

The review assumed several systems were missing. They exist; the table says
where, so the builder tunes instead of re-adding.

| Review item | State in the engine | Verdict |
|---|---|---|
| Voxel corner AO | `mesh/solid.rs` corner AO per vertex, greedy-merge aware | Exists |
| Coarse sky visibility under canopy | `ChunkData::sky_visibility` sampled per vertex, column probe | Exists; coverage under crowns is a tuning item |
| Cascaded sun shadows, bounded | `render/shadow.rs`, 4 near + 1 far, per-layer invalidation (aa53325f), depth-only caster pipeline | Exists; remaining cost is cascades 0 and 1 every frame for wind |
| One atmosphere model near and far | sky probe (a8ea5ee5), veil toward `raw_sky_radiance` (horizon fix) | Exists; far-right peach wash is the veil and cloud composite, not a second fog |
| Far terrain from the same world | heightfield rings to 24,576 m, 8 m fine grid, ADR 0012 agreement p95 1.75 voxels | Exists |
| Coverage kept until replacement ready | backdrop start = min(published, visible) with overlap | Exists |
| Debug views (material id, LOD, fog off) | `RenderDebugView` via `--debug-view`, material-id decode recipe | Exists |
| Frame instrumentation | `frames.csv`, `just frame-bench`, per-pass GPU timers | Exists; `render_call` includes GPU wait, read `acquire_ms` |
| Forest stand structure | per-tree forms only (`worldgen/src/trees/`), no stand density, age classes, clearings, dead trees | Gap |
| Ground material placement explains itself | gravel/soil/stone come from slope and displacement rules; no drainage or traffic link | Gap, verify material ids at the pose first |
| Mountain foothills and fans | spires are a heightfield cone; geo drainage solve is dormant at gate 71 | Gap |
| Foreground viewmodel | known (wand and arm audit 2026-10-25) | Out of scope here; hide for landscape captures |
| Low-tier acceptance machine | UHD 770 numbers exist, no 10 to 15 year old box | Gap; named box is an owner decision |

Rejected from the review: screen-space AO (voxel AO plus sky visibility covers
it and the iGPU cannot afford a pass), geometry clipmaps (the rings already do
this), transparent foliage cards (proxies are opaque and stay so).

## 3. Goals

1. Frame time on the 4090 at the frame-bench scenes: p95 under 14 ms on
   hills-static and hills-no-clouds, under 18 ms on hills-orbit, with draw
   counts no higher than the recorded baseline.
2. Same identity on the UHD 770 tier: no new pass that the lean tier cannot
   skip.
3. The vista capture agrees with itself: material ids at the pose explain the
   grey patches, forest reads as stands with clearings, the spires sit on
   foothills.

## 4. Non-goals and anti-goals

- No new post effects (bloom, god rays, SSAO, reflections). The review's own
  order puts them last and the frame has no headroom for them.
- No engine rewrite (decision 0036 stands: no M3 or M4).
- No posterization or banding (standing decision).
- Anti-goal: a sunset-only win. Every look change is captured at noon too.
- Anti-goal: a frame-time win that drops draws by removing content the
  baseline scene shows.

## 5. Constraints

- Shared tree with 293 modified peer files; all work happens on the
  `vista-perf` worktree from HEAD 226f2428 and lands by fast-forward or
  cherry-pick when the owner says so.
- Generator gates 54 to 73 are claimed by other branches; any forest or
  foothill change takes a gate from a grep of every branch first and stays
  dormant until the owner bumps.
- Visual changes carry before and after captures from the same pose, at two
  times of day.
- Frame-time claims come from `just frame-bench-check` (3 interleaved passes,
  best per side); single runs under peer builds are not evidence.

## 6. Milestone order

1. **Re-baseline.** The recorded baselines predate aa53325f and 789c2306.
   Record fresh ones from a clean HEAD exe when the box is quiet.
2. **Frame budget, 4090.** Attack the largest measured costs in order; the
   order is fixed by the fresh baseline, not by this document. Expected
   candidates from the last audit: cascades 0 and 1 redrawn every frame for
   wind, per-frame `models::build` of the orbital scene, main-thread
   streaming walks when nothing loads, `upload_finished` spikes, cloud march
   floor.
3. **Pose evidence.** Capture the vista pose with material ids, LOD colours,
   fog off, viewmodel hidden, at noon and sunset. Classify the grey patches.
4. **Forest stands.** A stand density and age field in worldgen behind a
   dormant gate: dense cores, clearings, edge saplings, sparse dead trunks,
   larger anchor trees. Capture A/B.
5. **Foothills.** A transition band around the spire cone on the geo branch.
6. **Low tier.** Owner names the box; record the same scenes there.

## 7. Decided by the owner

- Plan first, then implement; frames are the headline deliverable.
- All review concerns are in scope; order above is the builder's.

## 8. Left to the builder

- Which cost to cut first after the fresh baseline.
- Gate number for the forest slice (grep all branches).
- Stand model parameters; capture poses for A/B.

## 9. Results on the branch (2026-10-29, RTX 4090, 1600x900)

Fresh baseline from a clean HEAD 226f2428 exe, hills-static seed 74 generator
53, interleaved passes, best per side. The recorded `frame_baselines.json`
predates aa53325f and 789c2306 and is far behind HEAD.

| Scene | Build | p50 ms | p95 ms | p99 ms | fps | GPU shadow ms | frames redrawing all cascades |
|---|---|---|---|---|---|---|---|
| view 28, 6 s warmup (streaming) | HEAD | 14.9 to 15.3 | 20.3 to 20.8 | 22.2 to 23.1 | 63 | 6.8 to 7.0 | 346 to 369 of ~930 |
| view 28, 6 s warmup (streaming) | branch | 11.3 | 13.7 to 13.9 | 14.7 to 15.3 | 88 | 1.8 | 0 of ~1280 |
| view 16, 25 s warmup (converged) | HEAD | 10.0 to 10.4 | 11.8 to 12.7 | 13.4 to 13.7 | 94 to 99 | 1.8 to 2.4 | |
| view 16, 25 s warmup (converged) | branch | 10.3 to 10.8 | 12.1 to 12.6 | 12.9 to 13.5 | 93 to 97 | 1.8 | |

Cause of the HEAD p95: every upload batch changes resident coverage, far
cascades reach nearly every column, and reached layers were urgent, so a third
of streaming frames redrew all five cascades (10 to 14k shadow draws, 10 ms).

What changed: caster-change attribution per column, far cascades staged over
four frames into a spare depth layer and copied into place, coverage changes
leave fitted far layers pending instead of urgent, cascade 1 wind refresh at
half rate, and the `idle-stream` source-eviction fix cherry-picked.

Evidence: same-pose captures at noon and sunset (seed 2738958700, the vista
pose) differ by under 0.01 mean channel error; the sun-shadow debug view
differs on 0.46 % of pixels in the far band. Material-id capture: the
"grey-purple" ground patches are the log/dirt family (5), the pale spires are
sandstone (4). Forest stands are dormant behind gate 80 and reviewable with
`--benchmark-generator-version=80`.

Open: clouds (4.3 ms) and terrain (4.3 ms) are now the GPU floor at view 28;
the CPU floor is about 10 ms (streaming 3 to 4 ms while chunks load). The
converged view-16 scene is unchanged by this pass. Foothills, ground-material
placement and the low-tier box remain.
