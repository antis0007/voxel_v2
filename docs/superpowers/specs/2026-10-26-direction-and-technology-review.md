# Direction and technology review, 2026-10-26

Owner request: re-review the latest repo docs, current voxel-engine optimisation
techniques, and the direction of the game as a standalone tech and magic modpack
game. This is analysis with recommendations. It changes no code, no ADR and no
plan row. Companion: [far field and water](2026-10-25-far-field-and-water-design.md).

Evidence quality is marked per claim. "Read" means I read the repo file. "Search
snippet" means a web search result I could not confirm at the primary source
(several fetches returned 403 or 429). "Maintainer" means the wgpu tracking issue.

## 1. Direction as the docs now state it

Working title Underkingdom, engine Axiom. The new U1 set (`docs/NEW/01` to `07`,
the combined spec, untracked, dropped 2026-10-26) restates the target:

- Thesis: matter has rules, rules can be learned, learning them is power. Tech and
  magic are two dialects of one discipline.
- Four pillars that must stay coupled: habitation (Minecraft building and
  farming), exploration and secrecy (Noita layers, Zelda dungeons), invention and
  automation (Factorio logistics, modded-Minecraft industry), interaction and
  danger (Noita material chains, systems failures).
- Promise ladder: tactile in 5 minutes, home plus one unusual reaction in an hour,
  first repeatable automated process in about 2 hours without a wiki.
- Engine rules: five independent resolutions (storage, render LOD, material update,
  collision, nested machine scale); never process all stored matter per tick; no
  Bevy rewrite, no dense global voxels, no mesh shaders as a requirement.
- 14 measured spikes (S01 to S14) before any binding ADR.

Modpack parity is the mod addendum matrix (`docs/specs/MOD_ADDENDUM.md` A.1 to
A.16): Tinkers, Ender IO, RFTools, space mods, ICBM, force fields, reactors,
Thermal, Factorio, Forestry, IC2/BuildCraft, Mekanism. Addendum 07 adds twelve
player-freedom promises (C-PF01..12); ADR-style decision 0034 routes it as an
acceptance addendum and waits for owner sign-off (read).

Planning state: master plan approved with amendments; intake frozen; M1 to M3 gate
everything; no spike or claim has passed (read). The 07 brief and the variable-scale
brief are not inputs of record.

## 2. Where the repo actually is

| Area | State (read) |
|---|---|
| Committed saves and generator | Save 34, generator 53. Peers' uncommitted tree carries 39 and 65. Any slice picks its version against then-current source. |
| Cellular sim | `engine_sim` Margolus kernel, fine sub-bricks, coordinated block/fine tick with `R_fine`, awake cap and a 2000 sub-brick fine pass landed 2026-10-26. Not wired into play. GPU port exists, byte-identical. |
| Space track | Cube-sphere coordinates, Kepler rails, tidal lock, in `engine_space`; orbital monument and megastructures in the working tree. |
| Renderer | Multi-draw indirect landed, CPU frustum, cascade shadows, atmosphere LUTs, volumetric clouds. No Hi-Z. `render.rs` still 4.8k lines. |
| Far field | Heightfield rings plus opt-in cube columns, one sea sheet. ADR 0012 measured the interpolation error and recommended replacing the near band. Not done. |
| Meshing | CPU greedy mesher per chunk, LOD 0 to 3 by chunk distance. No bitmask mesher. |
| Machines | Factory layer exists; compiled-network design (S06) not done. |
| Process | About 300 modified or untracked paths; two live peer sessions; whole-workspace test and clippy fail on peers' in-flight files, not committed code. |

## 3. Technique survey

Fit is against our constraints: editable world, 1 m blocks with 0.25 m fine matter,
deterministic simulation, wgpu, a 4090 down to an iGPU.

| Technique | Evidence | Fit | Verdict |
|---|---|---|---|
| Binary greedy meshing (64-bit column masks, 64 faces per op) | Reference C project reports 50 to 200 µs per 64³ chunk, 74 µs mean single thread on a Ryzen 3800X (search snippet, one author). A Rust port exists (`block_mesh_bgm`). | Direct. Faster remesh helps edit latency and the horizon build. Needs per-face material and AO packing. | Spike inside S03. Adopt if it beats the current mesher on the same chunk set. |
| GPU culling: frustum plus Hi-Z, compacted indirect stream | Our spec §5 makes it the baseline. Vulkan Guide describes compute culling into an indirect-count buffer for millions of voxels (snippet). Sodium's single-threaded occlusion loop is a known bottleneck when LOD mods add geometry (snippet). | High. We already issue multi-draw. The 2026-10-22 baseline shows cascade refresh hitches of 1,000 to 5,500 draws. | Adopt. Next render slice after the far-field water work. Needs history invalidation on rebase and big edits (spec §5.6). |
| Multi-level voxel hierarchy for far terrain, built on background threads | Voxy: 5 LOD levels, compute traversal with occlusion culling, pluggable storage (DeepWiki snippet, unconfirmed). Distant Horizons: LOD chunks on background threads. Voxy "up to 10x faster than DH" is a secondary-article claim. | High as a pattern: one canonical data source, derived coarse levels. Matches ADR 0012 and spec §5 (no separate heightfield). | Adopt the pattern, not the mods. Build coarse levels from the same generator and edit journal. |
| Sparse voxel DAG (Aokana: cubic regions with small DAGs, claimed 9x memory cut and 4.8x speed over prior work) | ACM abstract via search snippet; the paper fetch failed. SVDAG lineage: Kämpe et al. 2013. | Strong for static far tiles. Poor for edited and simulated matter: merging subtrees fights per-cell state. | Do not adopt for the live world. Revisit as a read-only compressed tile format for far rings after the coarse field exists. |
| Mesh shaders and meshlets | wgpu maintainer, 2026-04-07: all three backends and output languages "fully supported"; remaining items (SPIR-V input, indirect validation, queries) "should not block real usage"; API redesign may follow. Tuning notes: small task payloads, 32-thread groups on NVIDIA, 64 on AMD, cap 1024 per group on Metal. | Medium on the 4090, none on the iGPU target. Our spec forbids making it a requirement. | Defer. Add behind a feature probe only after Hi-Z and indirect are proven. |
| Marching cubes or surface nets for natural terrain (Mycopunk) | Mycopunk devlog 2026-05-01 (SteamDB mirror): voxel grid, marching cubes, per-layer distance functions, GPU compute. | Fits the far rings, not 1 m cubic architecture. Spec §5 agrees: smooth only where art demands it, Transvoxel for 2:1 seams. | Use for rings 2 and 3 only, as in the far-field design. |
| Voxel ray marching and volume lighting (Teardown) | Lighting, fog and reflections rely on voxel volumes without special hardware (PC Games Hardware, snippet). Steven Wittens' one-frame breakdown exists at acko.net (unread). | Medium. A coarse occupancy volume could drive far shadows and fog without cascades. | Candidate for S03/S10 style spike. Not before Hi-Z. |
| GPU cellular automata (Noita, falling sand) | Noita rules: liquids move down then sideways, density swaps, random-direction fire (2019 interview, snippet). | Already built (`engine_sim`, `engine_sim_gpu`). | No new technique needed. Risk is integration, not method. |

Techniques not found in current sources: anything from 2026 papers on voxel meshlets
or Nanite-style virtual geometry for editable voxels. I did not find it; absence of
results is not evidence it does not exist.

## 4. Findings

1. **The modpack risk is machines, not pixels.** The addendum asks for thousands of
   machines, belts, pipes and reactors. Rendering and meshing have known fixes
   above. Per-tick cost of the machine layer (S06 compiled networks, isolated idle
   islands, bounded readback) decides whether the Mekanism and Factorio promises are
   achievable. S06 is not done. It should outrank any further renderer work.
2. **The far field contradicts the new spec.** Spec §5 forbids a heightfield
   pretending to match near caves and arches, and sets a LOD-residency contract.
   The live horizon is a heightfield with a 4 to 19 m interpolation error (ADR
   0012). The far-field design's coarse field answers this; the spec's S04 spike
   should run before ring 1 is built.
3. **Hi-Z and a bitmask mesher are the two concrete renderer wins.** Both are in
   the spec, neither exists, both reduce the hitch and build-time hotspots already
   measured in memory.
4. **Version drift is a standing hazard.** Committed 34/53 against working-tree
   39/65, with about 300 dirty paths and two peers, means every save or generator
   slice must re-read both constants and use a clean worktree.
5. **The doc set is now larger than the code can honour.** Master plan, 14 spikes,
   twelve player-freedom promises, a 169 KB combined spec and the addendum matrix
   overlap. Decision 0034 and the intake freeze exist to hold this; they depend on
   the owner's sign-off, which is still pending.
6. **Hunger and material yield conflict with C-PF08 and the material routes.**
   Decision 0034 lists them: sprint-linked hunger, drills deleting stone without
   yield, sand and gravel as blocks not resources. Each needs a product decision;
   the third forces a save bump.

## 5. Recommended order

1. S03 geometry spike with a bitmask mesher variant, on the existing cave, castle
   and edited-overhang fixtures. Same session: measure edit latency and remesh p99.
2. S04 LOD topology spike (parent-cover contract, edit hole at the far/near
   boundary), then the coarse-field far rings from the far-field design.
3. Hi-Z visibility into the existing multi-draw path, with shadow cascades 2 and 3
   on coarse tiles. Gate: shadow draws at cascade refresh and p95.
4. S06 machine-network spike with 2,000 machines, idle ticks and a long linear
   network. This is the modpack-scale gate.
5. Wire the fine tick into `World` (variable-scale step 2).
6. Hold: SVDAG tiles, mesh shaders, volume ray marching. Re-evaluate after 1 to 4.

Each step keeps the repo rules: fixed seed and pose, before and after captures,
interleaved timing, a branch or worktree of its own.

## 6. Decisions needed from the owner

- Sign off or amend decision 0034 (07 as an acceptance addendum).
- Which comes first after M1: far-field and water, machine-network spike, or
  variable-scale step 2. They touch different files; sessions can run in parallel.
- Default hunger behaviour (C-PF08) and whether machines yield sand and gravel.
- Whether untracked U1 docs and the combined spec are committed as inputs, or kept
  as references only.

## 7. Sources

- Mycopunk devlog, procedural terrain: https://steamdb.info/patchnotes/23052561/
- wgpu mesh shading tracking issue: https://github.com/gfx-rs/wgpu/issues/7197
- wgpu mesh shading docs: https://wgpu.rs/doc/wgpu/documentation/extensions/mesh_shading/index.html
- Aokana (ACM, abstract only): https://dl.acm.org/doi/10.1145/3728299
- Hybrid voxel formats for ray tracing: https://arxiv.org/pdf/2410.14128 (not read)
- Binary greedy meshing reference: https://github.com/cgerikj/binary-greedy-meshing
- `block_mesh_bgm`: https://docs.rs/block-mesh-bgm
- Voxy overview (DeepWiki, snippet): https://deepwiki.com/MCRcortex/voxy
- Sodium LOD Fixer listing: https://modrinth.com/mod/sodium-lod-fixer
- Teardown one-frame breakdown (not read): https://acko.net/blog/teardown-frame-teardown/
- Repo: `docs/NEW/01` to `07`, `docs/TRACKING.md`, `docs/decisions/0034-player-freedom-addendum.md`, `docs/adr/0012-horizon-representation.md`, `docs/plans/2026-10-25-variable-scale-and-underground.md`.
