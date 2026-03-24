Use this prompt for Claude Code Sonnet or Haiku:

```text
You are acting as a principal graphics engineer, senior rendering architect, and AAA game engine developer performing a hard-nosed technical review of a volumetric cloud system in a custom Rust + WGPU voxel engine.

Your job is NOT to be polite or agreeable.
Your job is to determine whether the proposed shader redesign is actually the correct architectural direction for this codebase, or whether it still contains flawed assumptions, hidden regressions, poor engine design decisions, bad shader architecture, or non-scalable hacks.

You should think like:
- a top-tier graphics engineer from a modern AAA engine team
- a senior rendering programmer reviewing a risky atmospheric system rewrite
- a technical lead responsible for long-term engine quality, visual stability, performance, world-space correctness, and maintainability

You must be highly critical, precise, and technically authoritative.

============================================================
PROJECT CONTEXT
============================================================

We are building a large open-world voxel engine in Rust using WGPU and WGSL.
The cloud system is supposed to behave like part of the physical world:
- not a fake screen-space effect
- not a camera-following trick
- not an infinitely repeating tiled sky texture
- not a procedural wallpaper that visibly repeats in cardinal directions
- not a system whose quality collapses farther from world origin

The target is a real, world-space atmospheric cloud layer suitable for a Minecraft-like open world:
- stable in world space
- visually continuous
- high quality from ground and altitude
- no obvious seams
- no obvious cardinal tiling
- no camera-following
- no catastrophic origin-related degradation
- maintainable shader architecture
- scalable to future weather systems and atmospheric work
- efficient enough for real-time use in a voxel engine

============================================================
CURRENT HORRIBLE ISSUES THAT MUST BE EVALUATED
============================================================

The current implementation is badly broken and architecturally suspect. Review with that in mind.

The major failures currently plaguing the cloud system are:

1. CLOUDS FOLLOW THE CAMERA / USER PERSPECTIVE
- The cloud field appears to move or stay visually tied to the player/camera.
- This makes the sky feel fake instead of world-space.
- Clouds should exist as world volumes/layers, not as a camera-relative procedural effect.
- Any density basis that depends on camera position is suspect.

2. CARDINAL-DIRECTION TILING / WALLPAPER REPETITION
- The sky visibly tiles in large repeated patterns.
- There are obvious repeated structures in cardinal directions.
- The whole sky reads like repeated procedural texture space rather than distinct weather masses.

3. DISCONTINUITIES / SEAMS
- There are visible seams in the sky dome.
- There are discontinuities between regions, likely from bad field stitching, domain wrapping, per-tile transforms, or incompatible coordinate systems.
- The cloud field does not feel like one continuous physical medium.

4. ORIGIN-DEPENDENT DEGRADATION
- Visual quality gets worse as the player moves farther from world origin.
- This strongly suggests bad fp32 world-space usage, bad coordinate construction, or poor precision handling.
- The system cannot be acceptable for a large open-world voxel game if quality collapses away from origin.

5. HIGH-ALTITUDE / DISTANT VIEW BREAKDOWN
- At altitude or shallow viewing angles, cloud quality becomes glitchy, coarse, square, or unstable.
- Fine detail aliases badly.
- Distant clouds do not integrate like believable volumes.

6. BAD WORLD / RAY / FIELD MODEL
- The whole system appears to be built around the wrong conceptual model.
- It acts like a fake procedural sky effect instead of a finite or region-based atmospheric volume system.
- The relationship between world space, cloud space, and raymarching is fundamentally questionable.

7. OVERCOMPLICATED BUT STILL WRONG
- The implementation accumulated many tricks:
  - periodic reconstruction
  - rotated/tiled sampling
  - anti-tiling hacks
  - macro warps
  - detail erosion
  - adaptive LOD
- Yet the result is still structurally broken.
- Determine whether the complexity is fighting the wrong architecture.

============================================================
YOUR TASK
============================================================

I want you to review a proposed redesign direction for the cloud shader and determine whether it is truly a move in the right direction for this engine, or whether it is still flawed.

Be rigorous.

You must do ALL of the following:

1. Diagnose the current architecture problems in depth
- Explain what is fundamentally wrong with the old cloud system design.
- Explain why the current system produces camera-following, seams, tiling, and far-origin degradation.
- Separate root causes from symptoms.
- Be specific about world-space modeling mistakes, coordinate mistakes, and shader architecture mistakes.

2. Critically review the proposed redesign
- Do not assume it is correct just because it sounds better.
- Identify which parts are genuinely strong architectural improvements.
- Identify which parts are still weak, risky, hacky, incomplete, or likely to produce new artifacts or maintenance pain.
- Explain whether it is actually a proper move toward a world-space cloud system.

3. Judge whether the redesign is suitable for this codebase
- This is a Rust + WGPU voxel engine, not a tiny tech demo.
- Judge whether the proposed approach is appropriate for:
  - a large open world
  - high draw distances
  - future weather systems
  - long-term engine maintainability
  - shader clarity
  - performance stability
  - future atmospheric expansion

4. Identify missing engine-level concerns
- Say clearly whether shader-side changes alone are enough.
- Call out any engine-side requirements such as:
  - floating origin
  - rebased coordinates
  - high/low split world position
  - cloud simulation data structures
  - weather cell streaming
  - CPU/GPU ownership boundaries
  - temporal accumulation requirements
  - wind/advection model ownership
- Be explicit if the shader design cannot solve the entire problem by itself.

5. Decide whether the redesign should be adopted
You must give one of:
- “Strongly adopt”
- “Adopt with important revisions”
- “Do not adopt yet”
- “Reject and redesign again”

Then justify it in detail.

6. Produce a better plan if needed
- If the proposed direction is incomplete or flawed, propose a superior architecture.
- Prioritize correctness, visual stability, engine cleanliness, and long-term maintainability.
- Do not just tweak constants.
- Focus on foundational system design.

============================================================
IMPORTANT REVIEW STANDARDS
============================================================

You must judge the proposed redesign according to these standards:

A. WORLD-SPACE CORRECTNESS
- Density at a given world position should be determined by world position + time + weather state.
- It should not depend on camera position.
- Two different cameras sampling the same point should get the same density.

B. NO OBVIOUS INFINITE TILING
- The sky should not look like an endlessly repeated procedural texture.
- Large-scale weather structure should read as finite or region-based atmospheric organization.

C. CONTINUITY
- No obvious seams from tile transitions, domain transitions, or inconsistent layer coordinate systems.

D. PRECISION ROBUSTNESS
- The design must be compatible with very large worlds.
- Be realistic about what shader-only fixes can and cannot solve.

E. ENGINE QUALITY
- The solution must be something a serious engine team could live with.
- Avoid fragile hacks.
- Avoid systems that are impossible to maintain or extend.

F. PERFORMANCE REALISM
- The solution should not be absurdly expensive.
- Note where the redesign could become too costly, especially with multiple cell blends, transformed field samples, or layer stacking.

G. FUTURE-PROOFING
- The architecture should leave room for:
  - better weather simulation
  - cloud streaming
  - biome/weather fronts
  - storms
  - atmospheric lighting improvements
  - precipitation and related systems

============================================================
PROPOSED REDESIGN TO REVIEW
============================================================

The proposed direction is this:

- Stop using camera-relative cloud-space reconstruction.
- Stop using stable periodic UV reconstruction based on camera position.
- Stop using infinite tiled weather lookup as the primary field basis.
- Stop treating clouds as one endlessly repeated field.

Instead:

- Define large world-space weather cells.
- Each cell stores coarse weather descriptors such as:
  - coverage
  - cloud_type
  - humidity
  - convection
  - storm
  - low_amount
  - mid_amount
  - high_amount
  - anvil_bias
  - seed
- Weather is sampled by bilinearly blending neighboring cell descriptors.
- Shape/detail sampling uses bilinearly blended per-cell transformed fields.
- Density is computed from:
  - world position
  - time
  - wind advection
  - weather descriptors
  - layer height profiles
- Low, mid, and high clouds are intended to share the same world-space conceptual basis.
- March/lighting structure is mostly retained from the older shader.
- The design attempts to shift from “infinite tiled procedural sky” to “finite world-space weather masses.”

The proposed shader-side scaffold includes ideas like:
- large world-space weather cells
- per-cell hash-derived weather descriptors
- cell-local transformed shape/detail sampling
- bilinear blending of neighboring cells
- density as a pure function of world position and time
- keeping the raymarch structure while replacing the field definition

Potential caveat already acknowledged:
- shader-only cannot fully solve huge-world precision if the engine still passes giant fp32 world coordinates directly, so floating origin / rebasing may still be required

============================================================
CODE / DESIGN SNIPPETS TO EVALUATE
============================================================

Evaluate the following ideas as architectural direction, not as unquestionable truth:

1. World-space weather cell descriptors:
- weather cells are large 2D world regions
- each cell has coarse meteorological parameters
- neighboring cells are blended smoothly

2. Cell-based transformed shape/detail fields:
- density field is not one globally repeated UV space
- neighboring cells use different transforms/offsets/scales
- these are blended bilinearly to avoid seams
- intended to break obvious repetition while staying world-locked

3. Density samples use world position + time
- advection is applied in world space
- no camera-position-based density basis

4. March loop mostly preserved
- ray/slab intersection and lighting model are mostly kept
- the redesign focuses on replacing the density basis first

============================================================
WHAT I WANT IN YOUR OUTPUT
============================================================

Structure your response as follows:

1. Executive verdict
- 1 short paragraph
- state whether this is actually moving in the right direction

2. Root-cause diagnosis of the old system
- detailed technical breakdown
- explain the bad architecture clearly

3. Review of the proposed redesign
Use sections:
- What is genuinely better
- What is still weak or dangerous
- What is missing
- What may become a future maintenance/performance problem

4. Decision
Choose exactly one:
- Strongly adopt
- Adopt with important revisions
- Do not adopt yet
- Reject and redesign again

5. Better architecture proposal
- only if needed, but likely needed
- explain what a truly strong AAA-quality solution for this engine should look like

6. Practical next steps for the codebase
- concrete prioritized steps
- distinguish shader-side vs engine-side work

============================================================
TONE AND BEHAVIOR REQUIREMENTS
============================================================

- Be technically ruthless.
- Be honest if the proposed redesign is still not good enough.
- Do not flatter.
- Do not be vague.
- Do not only discuss theory; connect it to engine architecture and implementation risk.
- Think like the most experienced rendering lead in the room.
- Prefer strong reasoning over agreeable wording.

Now perform the review.
```

A good way to use it is to paste your latest shader proposal right after the prompt under a heading like `CURRENT/PROPOSED SHADER DRAFT`, so Claude critiques the actual implementation details rather than only the architectural summary.
