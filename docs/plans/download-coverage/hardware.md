# Hardware and engine coverage

Source IDs refer to [the download inventory](../../specs/downloads-2026-10-24/README.md).
`L` = inspected partial foundation; `P` = planned; `D` = deferred. All target
numbers below are source requirements, not measured support claims.

## Addendum B: targets and complete scope

[DL01](../../specs/downloads-2026-10-24/ADDENDUM_B_HARDWARE.md) was missing
from the previous intake. It supersedes conflicting mission Part VIII targets.

| Class | Source examples / backend | Target fps/tps | RAM / VRAM cap | View / awake cells | State |
|---|---|---|---|---|---|
| H0 | HD 4000/4400; GL without compute, CPU simulation | 30/30 | 1 GB / 512 MB | 6 chunks / 64k | P; unsupported until measured |
| H1 | HD 520/620, Iris 540; compute backend | 60/60 | 1.5 GB / 1 GB | 8 / 150k | P; unsupported until measured |
| H2 | GTX 750 Ti/960, RX 460/560 | 60/60 | 2.5 GB / 2 GB | 12 / 300k | P; unsupported until measured |
| H3 | Deck, M1, Ryzen iGPU | 60/60; Deck 40 Hz option | 3 GB / 2 GB | 12 / 400k | P; unsupported until measured |
| H4 | GTX 1660, RX 5600, RTX 3050 | 120/60 | 4 GB / 4 GB | 20 / 500k | P; unsupported until measured |
| H5 | RTX 3070+, RX 6800+, M2 Pro+ | 144+/60 | 6 GB / 6 GB | 32 / 1M | P; unsupported until measured |

| Requirement | Source sections | Existing owner / remaining work |
|---|---|---|
| Class presets, p95 targets, three-run median and 8% regression gate | B.1/B.9 | C18/C19/T03; P. Limited desktop profiles do not certify hardware |
| Vulkan/DX12/Metal/GL and later WebGPU | B.2 | C18/R02; P for complete matrix and supported platform builds |
| Startup `caps.ron`, compute/storage/workgroup/subgroup/timestamp/indirect/texture/memory probe and 3-second microbenchmark | B.2/B.5 | C18/C19; P. Derive presets from capabilities and measurements |
| CPU-authoritative no-compute simulation, CPU meshing, billboard volumes, batched draw fallback | B.2 | C03/C04/R01/R02/R04; L: CPU CA and meshing exist; integrated GL path P |
| CPU timing without GPU timestamps; <=128 MB pool segments; adapter pipeline cache/warmup; driver quirks data | B.2 | C03/C18/R02/C19; P; audit existing profiler/pipeline code before extending |
| Hot-reloadable data profiles and all twelve scaling knobs | B.3 | C18/C17/R01-R13/C21/N07/B01; P for full ladder, not just cloud scaling |
| Resolution/upscale; volume resolution/billboards; cascades; view/far LOD; mesh/CA resolution | B.3.1-5 | R11/R04/R03/R10/C02; distinguish visual and authoritative scale |
| Awake work; gas/heat/light rates; body/mob limits; particles/post; audio; textures; last-resort tick rate | B.3.6-12 | C17/C07/C08/C11/B01/N07/R08/R12/C21/R13; semantics unresolved below |
| Brick/mirror/quad/entity/texture/page-cache budgets, zero steady allocations and 10-minute memory route | B.4 | C03/C16/C18/B01/N01/R01/R13; P for aggregate byte admission and measured limits |
| Hysteretic p95 governor: down after 2 s, up after 30 s with 30% headroom, cooldown/pins and per-knob cost UI | B.5 | C17/C18/P17; L: quality/cloud governors; full shared policy P |
| Main/worker/readback responsibilities, SIMD+scalar, CPU throughput, arenas/cache layout and release variants | B.6 | C20/C04/C03; P for measured 2015 CPU target, zero-allocation audit and platform dispatch |
| <=5 s menu / <=10 s world load on H1 and pipeline progress | B.6 | C18/R02/P17; P; no startup result established |
| 64-thread workgroups, <=16 KB shared memory, packed bandwidth, optional fast paths, <=64 KB readback/tick | B.7 | C04/R01/C18/C19; P for hardware-wide implementation/measurement |
| Depth/front-to-back rendering, low-res volumes, capped particles and thermal-throttle pacing | B.7 | R02/R04/R08/C17; L: renderer substrate; complete coverage P |
| Conservative coarse far simulation, bounded catch-up, interpolation, 30-second replay bug package and 10-minute tour | B.8 | C02/C15/C16/C17/R10/P01; P; no generic exact catch-up claim |
| Platform matrix, synthetic capability cases, real-device owners, preset images and PERF table | B.9 | C18/T03/R14; P. Preserve as acceptance work; current no-tests/no-CI rule governs |
| S-H1 capabilities/fallback; S-H2 ladder/memory/tour; S-H3 SIMD/jobs/interpolation; S-H4 far CA/catch-up; S-H5 device/replay review | B.10 | M1, M1, M2, M3, M4+ respectively; P; each class needs measured results |

The full knob order, formats, example budgets and commands remain in DL01;
this register does not silently replace them with a smaller feature list.

## Conflicts requiring design decisions

- **Hardware definition versus proof.** H0-H5 are now supplied target classes.
  Their software limit masks cannot emulate iGPU bandwidth, driver behavior,
  thermal throttling or unified memory. Keep ADR0016's accepted Iris Xe target
  as a separate device profile. No H-class is certified by RTX 4090 evidence.
- **Identical rules versus scheduling.** B.3 defers distant work and changes
  divisors/tick rate while also requiring identical tick-N outcomes under fixed
  budgets. Specify simulation time, due work, command order, catch-up and
  comparison conditions. Never independently slow predators, storage or
  electrical energy while advancing their consumers. Ordinary gameplay still
  has legacy authority; ADR0018 is only a sealed CPU region.
- **Coarse simulation versus rendering.** B.8 coarse CA requires a conservative
  promotion/demotion contract and evidence for each supported process. A
  heightfield or a closed-form crop update cannot stand in for arbitrary caves,
  flowing water or combat. Render LOD alone must not change gameplay state.
- **Memory arithmetic.** Accepted 8-byte hybrid cells (ADR0015), double buffers,
  sparse fields, headers, mirrors, staging, meshes and textures all count.
  The source's 8 KB/brick maximum and 48k/600k counts are targets, not a proved
  layout. Do not restore superseded 120 MB estimates or allocate every field
  everywhere. Define per-buffer IDs and transfer formats before pool splitting.
- **Capabilities and visual requirements.** Compute-only atmosphere/cloud
  proposals need an honest no-compute alternative. Capability fallback must be
  decided before allocating resources or creating unsupported pipelines.
- **Build/platform recipes.** CPU feature baselines, SIMD APIs, pipeline cache
  APIs and platform builds in the brief are proposals to verify when implemented;
  this intake neither changes dependencies nor certifies their compatibility.

## Mission, mod and bible continuity

DL04 is the mission's thirteen parts, already normalized-equivalent to
[the canonical mission](../../specs/ENGINE_CORE_PROMPT_v2.md). Its complete
core, cell/brick/entity layout, CA, bodies, mobs, machines, magic, rendering,
audio, gameplay paths/profiles, text, tooling, risks and M1-M7 scope remain
owned by the existing master inventory. Accepted layout/conservation/authority
corrections in ADR0014-0019 still apply. DL07/DL08 preserve all A.1-A.16 mod
families; the Celestisynth difference is in the gameplay register. The existing
Game Bible and Project Axiom revision 4 remain additional canonical inputs.

Exact checkpoint/replay, global actions, shared VM, Primer, planes, support,
solver ledgers, all 24 bible claims, research matrix/SDDs and measured spikes
remain explicit master-plan work. Finding Addendum B closes an input gap; it
does not close those engineering or acceptance gaps.
