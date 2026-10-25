# Hardware breadth: targets, ownership and unresolved contracts

2026-10-24. Source: [Addendum B](../specs/ADDENDUM_B_HARDWARE.md), found in the
user's Downloads and preserved unchanged, SHA-256
`129902bc8f2dbc6866d6f4c54b604f8ff05a36a4bc3cf5cffe0a3c587a4c2c30`.
It supersedes mission Part VIII where they conflict. These are targets, not
measured capability or a change to accepted simulation/authority invariants.

This focused audit supplements the broader [Downloads coverage register](download-coverage/hardware.md);
its original-source preservation and other downloaded scopes remain intact.

## Six target classes

| Class | Example / preset | Frame and simulation target | Process RAM / GPU memory limit | View / awake cap |
|---|---|---|---|---|
| H0 | HD4000/4400, 4 GB, 2c/4t; Minimal; GL without compute | 30 fps / 30 tps | 1.0 GB / 512 MB | 6 chunks / 64k |
| H1 | HD520/620/Iris540, 8 GB, 2c/4t; Low | 60 / 60 | 1.5 GB / 1 GB | 8 / 150k |
| H2 | GTX750Ti/960 or RX460/560; Medium | 60 / 60 | 2.5 GB / 2 GB | 12 / 300k |
| H3 | Deck/M1/Ryzen5 iGPU; Medium+ | 60 / 60; Deck 40-Hz presentation option | 3 GB / 2 GB | 12 / 400k |
| H4 | GTX1660/RX5600/RTX3050; High | 120 / 60 | 4 GB / 4 GB | 20 / 500k |
| H5 | RTX3070+/RX6800+/M2Pro+; Ultra | 144+ / 60 | 6 GB / 6 GB | 32 / 1M |

Targets apply to p95, not average FPS. The addendum's GB/MB and k units must
be converted to explicit bytes/counts in C18's schemas. CPU/GPU shared pages
need an ownership-aware physical-memory total, not two blindly added counters.
All classes are **unsupported/unmeasured in this pass**. Existing unrelated
desktop captures cannot fill this table's acceptance column.

ADR0016's accepted Iris Xe target (500k awake/60 tps, 720p internal/1080p output)
remains a separate named target. H1 means an older HD520/620-class machine;
it cannot inherit an Iris Xe result or silently lower the accepted Iris target.
Feature masks/thread caps reproduce capability branches and workload limits,
not bandwidth, thermals, driver behavior or a particular laptop's performance.

## Capability and fallback mechanisms

- C18 probes adapter features, downlevel capabilities, enabled device limits,
  formats and surface capabilities; serializes versioned `caps.ron`. Use an
  unknown/estimated memory field when the backend cannot report physical VRAM.
  Three-second microbench coefficients and caps inform a preset, not the name.
- C03 uses bounded buffer pages no larger than both the configured ceiling
  (128 MB in the brief) and actual buffer/binding limits, with `(buffer,index)`
  addressing. CPU pools obey the same allocation/eviction admission contract.
- H0: CPU CaBackend, CPU worker meshing, class-batched draws without indirect
  dependence, billboard volumes, optional effect removal. GPU-only features
  cannot be requirements for gameplay; state/rules use the same semantics.
- Missing timestamps use labelled CPU-side submission/latency measurements;
  do not report them as GPU execution time. Optional subgroup/multi-draw/format
  paths require individual checks, including legal shader/bind-group layouts.
- Pipeline warming has visible load progress; persistent pipeline caches are
  optional by capability/backend and invalidation key. Driver quirks apply
  before device/pipeline creation. Startup targets remain <=5s menu and <=10s
  world load on H1, with measurement method and cold/warm caches recorded.

The repo pins wgpu 27.0.1. Context7 was resolved and queried first; its main
result follows trunk and is not pinned evidence. Local installed 27.0.1 source
was then read: `src/api/pipeline_cache.rs` documents Vulkan-only application
pipeline caching, and its README labels GL3.3+/GLES3.0+ downlevel/best effort.
Therefore Addendum B's general pipeline-cache requirement becomes an optional
optimization with warming on other paths, never a universal startup guarantee.
Official links: [PipelineCache](https://docs.rs/wgpu/27.0.1/wgpu/struct.PipelineCache.html),
[backend source](https://github.com/gfx-rs/wgpu),
[capability model](https://docs.rs/wgpu/27.0.1/wgpu/struct.DownlevelCapabilities.html).
The browser could not fetch pinned docs.rs pages; local package source is the
version-specific evidence. Old-OS support still requires a full binary/toolchain
and windowing dependency audit; an API backend label alone is insufficient.

## Scaling ladder ownership

All values are planned RON profiles, independently pinnable. C17's governor
tracks moving p95, two-second downshift, 30%-headroom/30-second upshift and
per-knob cooldowns; its log is visible in C19. Deterministic measurements disable
the governor. Authoritative changes require the contract resolution below.

| Order | Knob | Single owning systems | Required fallback / invariant |
|---|---|---|---|
| 1 | Resolution 0.5–1.0; bilinear/FSR1 | R11/C18 | Basic scaling first; preserve inspectable UI |
| 2 | Volume quarter/eighth/billboard | R04 | Same gas/fire state, cheaper presentation |
| 3 | Shadow count/size | R03 | 4/2/1/none |
| 4 | View 6–32 and fine distance | R10/C18 | Page bounded state, not discard it |
| 5 | Fine/2x/4x mesh/simulation LOD | R10/C02/C04 | Rendering can degrade; CA requires conservative contracts |
| 6 | Awake 64k–1M | C17/C03/C04 | Deterministic deferred queue, no dropped matter/actions |
| 7 | Gas/thermal divisors, light quota | C07/C08/C11 | Replay schedule and error policy explicitly defined |
| 8 | Body/mob caps and LOD radii | B01/N01/N07 | Admission/aggregation policy; never erase live entities |
| 9 | Particles/decals/SSR/bloom/AO | R08/R12 | Presentation reduction cannot remove authoritative fragments |
| 10 | Voices/occlusion rays | C21 | Budgeted audio; hazard information remains legible |
| 11 | Textures/procedural detail | R13 | Format/detail branch from capabilities |
| 12 | 60→30 tick rate | C01/C15/C17 | Unresolved: never double state velocities silently |

## Conflicts that must be resolved before dependent SDDs pass

| Conflict | Evidence / consequence | Required resolution owner |
|---|---|---|
| Identical tick-N outcomes vs changing quotas/divisors/caps | The addendum's headless hash rule fixes budgets, but live governor changes work; live cross-preset equality does not follow | C15/C17 persist deterministic service cursors/quotas; define comparable schedules and distinguish presentation-only knobs |
| 30-Hz doubled velocities | Nonlinear contact, reaction and thermal operations need not equal two canonical steps; tick-count equality is not wall-time equality | C01/C04 define canonical time, substeps and units; an ADR must reconcile H0 cadence without weakening deterministic rules |
| One-tick readback vs D4 | B.6 threading says one tick; accepted D4/ADR0017 requires exactly two CA ticks | C15/C20 preserve two-tick published authority, explicitly separate transport latency; measured deadline stalls remain required |
| Coarse CA / catch-up | Scaled local rates and closed forms cannot reproduce arbitrary interacting reactions or boundary transfers | C02/C04/C16 restrict proven cases, retain residuals and deterministic events; SDDs specify when exact simulation is required |
| 8KB brick vs more than eight planes | With 512 cells, 8 base bytes and 13 one-byte optional planes plus a 32-byte header, even an illustrative lower bound is 10,784 B/brick | C03/C14 compute actual plane widths/occupancy and hard byte admission; no free-header or guaranteed-all-plane claim |
| H0/H5 pools under that lower bound | 48,000 bricks need 517,632,000 B; 600,000 need 6,470,400,000 B before other pools | Cap allocations by actual bytes; optional-plane growth can trigger eviction; choose explicit units and total headroom |
| CPU throughput has little margin | 64,000 cells ×30 ticks =1.92M updates/s; a 2M/core target barely covers one cell visit and excludes multi-pass/thermal/host cost | S4 measures complete passes and derives viable H0 caps; no target supported from peak inner-loop rate |
| Hardware emulation vs acceptance | Masks/caps cannot reproduce an old iGPU's power, bandwidth or shared memory | T05/T06 distinguish branch correctness, constrained host timings and real-device performance |
| OS/build/SIMD breadth | Adapter support does not establish supported old Windows runtimes, ISA availability or baseline SIMD crate feasibility | C20/C18 audit dependency/compiler floors and runtime dispatch before shipping variants |
| Halo/workgroup assumptions | A 10³ halo of 8-byte cells is 8,000 B; staging 13 byte fields adds 13,000 B, exceeding 16 KiB. Mandating workgroup 64 also preempts S1's 64/128 comparison | C04 stages declared fields only; S1 measures the choice. Split-u32 storage alone does not provide atomic 64-bit operations |

## Five hardware work packages

These interleave with M1–M4+ as required by B.10. Each contains separate scoped
system sessions under B.6 and ends in a class-labelled report. Their deliverables
are future work, including proposed commands; none was run or added here.
Caps/schema design may start early; H0 dam_break acceptance waits for S1/S4/S2,
reviewed core/liquid/render SDDs and completed fallback implementations.

| Package | Milestone | Owners and exit |
|---|---|---|
| S-H1 | Before M1 ends | C18/C03/R01/R02/R04: caps, six profiles, constrained-mode flags, no-compute CPU path, split buffers, legal fallback pipelines, startup/warming; dam_break H0 30-tps evidence |
| S-H2 | M1 | C17/C19/C18/R11/P17: ladder, governor logging/pins, cost view, memory accounting and ten-minute tour workload; all authoritative scheduling conflicts resolved first |
| S-H3 | M2 | C20/C04/B01/N03: SIMD/scalar paths, worker ownership, no steady allocations, interpolation; pass parity and full-pass cost evidence |
| S-H4 | M3 | C02/C04/C16/R10: proven coarse transfer, allowed catch-up, far meshes; explicit conservation/replay limits |
| S-H5 | M4+ | T05/T06/T04/R12/P19: class images, driver quirks, owned real-device runs, Deck/M1 evidence, 30-second replay reports |

Future evidence work includes a ten-minute tour memory report, three-run median
p95 regression limit of 8%, class-specific images and capability-case coverage.
`just mem`, `just run --profile h1` and F7/F8 bindings in the brief are proposed
interfaces, not current commands or approved key changes. P01/P19 must audit
existing controls before assigning bindings. Hosted CI/nightly schedules remain
deferred under the user's operating instructions; use local evidence when later
authorized and retain unmeasured classes as unsupported.
