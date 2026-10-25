# DUNGEON QUALITY & SYSTEMIC IMMERSIVE-SIM PLAY — SPEC v2.0
**Scope:** Rewrites the uploaded dungeon quality overhaul as a shippable art, procedural layout, creature and gameplay project with explicit integration boundaries to the engineering simulation.  
**Engine:** Existing Rust + raw `wgpu` voxel game, existing expedition/site/quest/loot mechanisms.  
**Status:** Proposal; supplied screenshot analyses and reported defects are background, **not evidence the current repo or renderer has been inspected.**

## 1. What is actually wrong and what success looks like

The supplied earlier brief described an Astral/Cosmos dungeon whose rooms read as dark boxes containing repeated cyan/red cross wallpaper, scattered purple ceiling ornaments and disconnected encounter props. Some upper surfaces and far walls are insufficiently legible; construction, useful routes and focal hierarchy are weak. The Undertow Knot's current plinth-like appearance does not visually explain its containment function or state. The reported Sanctum Warden is an indistinct shield-bearing slab, the moth is not clearly different from an effect/pickup, and the crab looks mechanical without sufficient articulated mass. The screenshots do **not** prove AI correctness, spawn systems, actual light propagation or movement mechanics.

**No fix is accepted merely because it introduces more decorations, more rooms, a larger boss health pool, more dynamic lights or passing tests.** The target is a readable, memorable place with real editable geometry, multiple meaningful ways through it, honest encounter behaviors, linked machinery and recoverable consequences for failed experiments.

### Four acceptance stories
1. From the first doorway, a new player sees the room's volume, understands at least one useful path and can identify the objective without an enormous UI panel.
2. On encountering a Sanctum Warden, the player reads its shield direction and attack commitment, exploits the exposed flank/recovery, or avoids the fight through space/cover/terrain systems. Attacks and shield behavior match geometry.
3. At a mechanism, the player can make a real choice (cut/restored power, alternate service route, build bridge, dig a breach, lure threat) and observe the **same shared systems** changing both access and cost. No scripted false wire/prop.
4. After a mistake (fall, alarm, broken circuit), the player reaches an altered but playable state. Reload preserves edits, mechanism progress, guardian identity, rewards and claims with no duplication.

## 2. Art direction: forgotten scientific institution colonized by magic

**Place identity:** An underground astronomical archive, working magitech institution, or industrial research sanctum. It was constructed by a technically sophisticated civilization; energy, machines and instruments once served a real purpose. Time, hostile ecology, rust, leaking water, collapsed masonry and living astral fields now reclaim it. Magical ornaments are significant *functional* phenomena, not generalized neon wallpaper.

### Material hierarchy
- **Primary:** Quiet warm/violet-gray masonry; large coherent block/ashlar surfaces, not fractal noise everywhere.
- **Secondary:** Dark structural metal beams, braces, rails, machinery; worn ivory or stone plates at high-contact/institutional elements.
- **Functional trim:** Aged brass/gold on major clasps, controls and objectives; silver guides, precision inlays and high-value instrument parts.
- **Environmental wear:** Soot where equipment burns, mineral runnels below leaks, impact cracks on load-bearing geometry, polished areas at mechanical contact, fungi near moisture.
- **Active lights:** A few warm utilitarian fixtures illuminating routes and maintenance stations; cool indicators only on important machines; concentrated violet/astral glow reserved for limited containment apertures.

Retain pixel-art material identity with consistent world-space texel density and stable distant filtering. Avoid identical cross stamps across every wall/floor face, out-of-scale normal details, baked directional highlights, everywhere-emissive trim and excessive bloom. Distant material shader must not turn patterned structure into noise; however, shader changes require coordination with existing terrain/horizon owner.

### Spatial composition grammar
Each room has: a dominant form, construction logic, usable circulation, one principal focal element, subordinate landmarks, meaningful negative space, at least one readable vertical limit/ceiling, and credible maintenance/utility connections. Architecture before decoration.

**Reusable construction primitives:** wall bay, pier/capital, lintel/doorway, supported beam, coffer, corbel/stepped vault, vaulted rib and infill, balcony/guardrail, stairs/landing, service trench/covered channel, accessible hoist support, framed oculus/aperture, repair/damage seam, natural cave breach. Every primitive has dimensions in meters quantized to supported voxel grid; authored/support collider and navigation clearance must match actual player/enemy movement.

A large hanging magical object requires visual suspension *logic*: clamp, anchor, pylon, field containment frame or aperture. Static detail can be instanced/noncolliding when declared decoration. Objects that look climbable, impassable, destructible or walkable need correct authoritative collision/material/interaction.

## 3. Reference dungeon: spaces and connectivity before procedural variation

Build **one high-quality fixed-seed benchmark** with approximately the following roles, not a mandatory fixed count in every procedurally generated site:

```
                  [UPPER GALLERY] ----> [BINDING CHAMBER / UNDERTOW KNOT]
                 /        |                         |       |
[ENTRANCE]------+         |                     [RECOVERY] [RETURN SHORTCUT]
                 \        |                         |       |
                  [SERVICE WORKS] ------------------+-------+
```

The diagram represents intended relationships, not a license to use teleportation or disconnected rooms.

**Entrance overlook:** safe first footing, glimpsed destination and operational context, inspectable threat, credible retreat and access to two differentiated approaches.

**Upper gallery:** elevated exposed route with shield-bearing lane threat, piers and cover, partial views into principal chamber, meaningful risk/shorter distance tradeoff.

**Service works:** lower route with machinery, visible conduit, utility room, damaged pump/transformer/control board, alternative traversal challenge or material cost. Its mechanisms should have real typed power/control ports when implemented. The player can learn how the institution functioned.

**Binding chamber:** tall stepped/corbel vault, narrow number of important star-glass apertures, clear object silhouette and state indicators. The Undertow Knot has base, containment frame, physical power/arcane attachments and observable sealed/charged/disrupted/claimed states. Objective rules, guardians and controls agree; a decorative wireframe tent is insufficient.

**Phantom-floor/recovery:** preserve actual preexisting phantom-floor mechanic (do not replace it by random platform jumping). Show a safe preview of its states; legible groups, gaps, timings/conditions and sound/shape signals. A mistake drops into a lower route where recovery is intended, not always instant death.

**Return shortcut:** a physically credible stair, lift, repaired span, service door or bridge that opens after a real event; shortcut persists. Use only if implementable with existing collision and objective systems.

## 4. Procedural generation as a constrained planning problem

### Two-stage generator
**Stage 1: Spatial/functional plan.** Choose purpose, large volumes, elevation, access graph, approach risk, line-of-sight exposures, objective position, enemy slots, service networks, natural breach, recovery and exit. Specify *why* an access route exists, not just a randomized corridor list. Assign construction kit pieces and technological state before scatter props.

**Stage 2: Voxel realization and validation.** Materialize actual walls/arches/floors/rails/openings and collision at world resolution. Quantize dimensions to variable voxels with testable tolerances. Check player route, enemy clearance, reachable interactions, jumping/landing, safe entry, path/exit after world edits, potential fall route and boss encounter spaces. Use bounded retries with diagnostics and deterministic fallback; never silently teleport because a layout failed validation.

### Variation quality requirements
Vary entrance observation angle, upper/lower relationship, exposure vs cover, route length/resource cost, stair/well position, service-system dependencies, breach location, objective approach, enemy placement, return path and the specific recoverable failure. Palette swaps, mirror-flipping one arena and sprinkling random props do **not** qualify as substantial generation diversity.

### Procedural intent is not permanent authority
The generator's nav graph is an initial plan. If the player digs a passage, bridges a gap, blocks a corridor, disables a mechanism or removes a support, the **actual world geometry and machine state** become authoritative. Recompute/repair affected collision/navigation and objective access, not simply lock the player into originally authored route edges. Special unmineable material is acceptable only when justified by readable physical/magical laws, not because designers fear bypasses.

## 5. Creature design: three distinct and mechanically exploitable roles

### Sanctum Warden — directional lane holder
**Visual:** Stone/ivory reliquary over articulated dark internal frame, powerful feet and knees, a compact recessed sensing head/core, layered protection, substantial shield with mechanical grip/struts and weight-bearing arm. Shield side must be readable even as a silhouette, including on low graphics settings.

**State flow:** Idle/guard → target orientation/approach → brace / wind-up → committed shield bash → active collision sweep → recovery/exposed flank → reposition/guard. Attack has directional commitment and a limited hit window. The shield blocks where it physically exists and is oriented, not as an omnidirectional percentage modifier.

**Counterplay:** Bait, flank, take cover, interrupt where supported, alter terrain/access, retreat. Shield can become acquired/placed cover only if recovered item uses **the same underlying oriented blocking rule**. Don't make a fake decorative prop or an unrelated +20% stat.

### Aether Moth — sightline and light/stimulus pressure
**Visual:** Paired angled wings, articulated/clear silhouette, thorax/abdomen, antennae and a small emitting core; unmistakably a flying creature rather than particle effect or collectible.

**State flow:** Traverse → braking hover → facing/visible charge cue → directed dart/burst → reposition/cooldown. Light/stimulus seeking uses authoritative in-world light-emitter/stimulus information and bounded local spatial queries, never bloom/exposure pixel color. Respect real walls and new cover; no shooting magically through opaque terrain.

**Counterplay:** Break line of sight, redirect attention with a supported lamp/lure, exploit commitment/recovery, use ceiling/cover. If regeneration exists, make its energy/source interruptible; otherwise cut it. A recovered lens must be a functional stimulus/light component usable outside this specific room.

### Crab-like existing archetype — low flank and terrain pressure
**Visual:** Layered vaulted shell and visible heavy mass, articulated claw pivot, thick leg roots, distinct forward/back and readable underside/weak region. Do not invent an extra fourth enemy from ambiguous screenshot forms. Verify actual entity → model mapping.

**State flow:** Lateral pathing → plant → short committed claw lunge → impact/recovery, with low cover/flank tactics. Its attacks interact with actual geometry. Optional chipping/excavation uses existing terrain damage rules, limited hardness/range/work, local route feasibility and persistent edits; never arbitrary tunneling straight to the player through protected structural materials.

**Counterplay:** Elevation, narrow access, hard cover, bait/recovery, targeted physical weak regions, changing ground. If chipping isn't ready, delay the corresponding player chipping-tool drop; make a complete other reward rather than a nonfunctional promise.

### Shared combat contract
- Simulation evaluates pose/action transitions and hit regions. Rendering interpolates that authoritative pose and **does not feed back a differently animated transform**.
- Fast weapon/projectile motions use swept collision with up-to-date occupancy. Enemy navigation and pose/hit must react to terrain edits before next affected action.
- Distinct preparation/commitment/recovery/interrupt cues in animation and audio. Mandatory readability cannot rely exclusively on color, bloom or a giant screen-space label.
- Colliders match major blocking/protected volumes; gaps around thin legs/shields are not gigantic invisible cuboids. Decorative appendages may use simplified secondary collision with disclosed bounds.
- Movement obeys stairs, ledges, collision, fall physics, navigation updates, line of sight and obstacle damage limits. No teleporting or phasing to hide stale nav paths.
- Bounded perception and path queries, shared immutable meshes/material resources, reusable rigid-part animation hierarchy, cheap instancing; no per-frame remeshing of every creature part.
- A drop is a single stable instance transitioning from enemy → world pickup → inventory/placed object. Death on chunk unload is not fabricated; reloading can't duplicate drops.

**Test combinations:** Warden+moth (lane control and peek pressure); Warden+crab (shield front and low flank); moth+crab (vertical attention and ground pressure). Start each archetype alone at normal gameplay distance, then combine in spaces designed for each role.

## 6. Mechanisms, power, and the sci-magic economy of ruins

**Important:** This track cannot require an unfinished electrical rewrite. Start with a small stable `MachineMechanismState` adapter to existing authoritative power/control if needed. Later connect to full graph ports behind the same API. A fake wire triggering a scripted Boolean is not the final target; any interim approximation must be labeled and not claim systemic integration.

### Example: the binding apparatus
Suppose a power bus feeds three consumers: containment coil, maintenance lift and selected work lamps. The player may disconnect the bus to disrupt containment, but also loses lift/lighting. Alternative routes include repairing an auxiliary supply, finding a mechanical bypass, routing a cable from a salvage generator, approaching via gallery or digging/building around an obstacle. The Knot shows its state through actual indicators and apparatus movement. Emergency light and exit route prevent a softlock.

**Objective eligibility:** Distinguish `Discover`, `DefeatGuardians`, `DeactivateContainment`, `Restore/RepurposeMachine`, `Escape`, and `ClaimReward` conditions. A purely environmental route may satisfy a distinct objective **only if explicitly authored**; it must not create fake guardian deaths or silently skip legacy guardian-gated requirements. Player bypass of a fight is legitimate where objective policy permits. Objective text, object state and accessible reward agree.

### Make player tools and magical spells reusable
A movable recovered shield blocks actual projectiles. A moth lens affects real stimulus queries. A salvaged relay/switch alters a real power graph. A thermal or drilling spell uses existing voxel material/hardness/heat rules, rather than special-casing one dungeon. Spells may provide alternate engineering routes with explicit mana/field/material costs. Magitech hazards use authoritative fields, thermal, light, collision and energy data with bounded spatial effects.

### Do not allow a single magic bypass to destroy all challenge
Challenge comes from specific constraints and choices, not blanket immunity. A protected vault might have a reason to resist digging (exotic structural material, field powered by a generator that can be cut, material tool requirements, collapsible supports, environmental consequences). The player can inspect those constraints and build a solution. Avoid blanket antimagic or indestructible shells inserted merely to force one encounter order. If bypass genuinely works, respect it and let objective/reward logic respond coherently.

## 7. Lighting and performance: readable spaces, credible machinery

Existing problem diagnosis must separate local emitter placement, actual light injection, shader response, ambient/exposure, occlusion, shadow caster coverage, missing far geometry and procedural composition. The requested appearance is not achieved by multiplying ambient light or filling the room with expensive emissive props.

**Plan:** Use controlled warm practical fixtures on real supports at functional intervals (doorways, walkway corners, service stations), a few accent/cool signals on selected active equipment, and one or few concentrated astral features. Light major room planes and structural spans sufficiently to establish size while preserving dark recesses and stakes. World-space emission is real gameplay stimulus **only if registered** with the authoritative light/stimulus system; visible glow alone isn't a light source.

**Rendering:** Reuse existing voxel meshing, material and culling paths. Repeated architectural kit pieces can be static batches/instances. Distinct creature parts share meshes/materials and transform hierarchies. A few important dynamic lights/shadows, not one per decorative crystal. Critical interaction and threat cues remain readable at lower preset. Measure actual light/shadow counts, draw/instance counts, GPU pass times, chunk update/upload spikes and memory alongside CPU combat/navigation costs.

**Horizon and foliage:** Keep work limited to dungeon-specific material/geometry/light issues unless repro demonstrates a global renderer bug; coordinate shader edits with active terrain/horizon owner. No sweeping replacement of the renderer just to fix local dark room composition.

## 8. The first complete playable site — actionable design and test cases

### Minimum structural deliverable
One safe vestibule/service space, one gallery/workshop approach and one binding chamber, with usable visible ceilings, clear exits and deliberate elevation. The larger route graph includes a service alternative, an actual recovery connector and a shortcut where supported. Preserve the existing phantom-floor special encounter at its established logic; no rebranding or mechanic removal.

### Two playthroughs through the same saved site
**Route A — Combat:** Move via gallery, exploit Warden's shield/cover directional rule, proceed past objective, claim according to legacy-combat policy, unlock return.

**Route B — Engineering/terrain:** Inspect service bus, manipulate one real supported mechanism or traverse through a player-created material change; incur actual cost (light loss, tools, resources, danger, route length) and reach objective; claim only what policy permits; leave via real exit. Do not fake a guardian death.

**Recoverable mistake:** Phantom-floor fall or a failed power rearrangement changes circumstances, but the player reaches another route or restores equipment. No invisible script reset. Include a controlled test of re-entering after save/load.

### Evidence of playability
- Captures at actual normal player eye-height: before/after entrance view, upward ceiling view, two selected route views, object states, neutral/windup/recovery silhouettes. Keep overlays hidden for one set, then show diagnostics for clarity.
- Short in-engine recording or reproducible demonstration: movement through spaces, Warden attack missing and player exploiting recovery, moth dart respecting new cover, crab flank, power isolation/terrain change, phantom-floor mistake recovery, claim and return.
- At least two different route playthroughs for same seed. Then 8–16 fixed seeds (adjust based on code) inspected for reachability, route variation and spatial identity; a different color scheme doesn't count.
- Save/reload after mechanism change, terrain edit, guardian death and reward transfer. No duplicated or reset identities.

## 9. Generation, combat and physics tests

| Subsystem | Property and regression check |
|---|---|
| Geometry | All mandatory connectors accessible with current player collision; no false stairs or inaccessible interactions |
| Voxel variations | Quantization preserves room heights/door widths and enemy clearance at supported resolutions |
| Reachability | Bounded retries/rejected invalid seeds; exit and objective reachable through at least one intended route |
| World edits | Dig/build modifies actual collision and nav before next relevant movement; no stale projectiles phasing through wall |
| Enemy poses | Shield oriented physically; weapon contacts swept poses; active damage never on mere pre-windup animation |
| Moth | Local supported emitter queries; opaque wall blocks attack; low-preset glow still communicates state |
| Crab | Route and footing valid; optional chipping obeys hardness, range, bounded work and persistence |
| Objective | Distinct combat/environment claim policies; unknown/dead/unloaded guardian IDs handled exactly once |
| Loot | World-to-inventory/placement transfer exactly once across reload and interruption |
| Power | Disconnected bus disables actual consumers; mechanisms/UI reflect same source of truth |
| Environmental fallback | At least one alternate path avoids permanent softlock after planned mechanism failure |
| FX/performance | Light count, mesh instance cost, nav queries and active effects measured in real scene |
| Legacy | Existing completed/claimed sites remain completed/claimed under migration |

**Failure policy:** Failed validation must explain exactly which condition failed. Do not suppress by placing rewards inside walls, teleporting players, disabling collision, faking boss deaths, auto-unlocking objectives or reverting edits silently.

## 10. Performance architecture and testing

Prefer the current region/chunk hierarchy; local dirty sets for collision/lighting/nav; bounded dynamic obstacle re-evaluation; precomputed static architecture pieces; instanced rigid parts; event-driven mechanism state; rendering read-only interpolated simulation state. Dynamic spatial searches limited by interest region and stimulus types. Offscreen enemies affecting the player/site must remain simulated to needed fidelity; culling draw is not culling authority.

**Required instrumentation:** CPU average/p95/p99 for AI perception, route search, pose evaluation, attack collision, terrain edits/remesh, world generation; GPU frame pass and shadows; lights, meshes/instances, upload bytes, active particles/projectiles, chunk occupancy; memory consumption. Use a specified hardware/software setting and a controlled seed. Performance optimizations require before/after measurement and unchanged behavior tests, not guessed speedup percentages.

**Editor iteration workflow:** Add a deterministic teleport-to-test-site/dev scenario, scripted enemy action playback with debug overlays, optional material/light inspection and fixed camera captures. Don't build a new generalized editor if a tiny debug command or scenario already exists. Keep one in-game scene with all three creatures at normal distance as a repeatable check.

## 11. Phases and agent scopes

**D0: Audit and reference benchmark.** Inspect actual generators, existing site claims, Warden/moth/crab archetypes, UI, world edit/collision, current renderer, original screenshots; record before captures and known tests. If screenshot files aren't in repo, say so and use currently reproducible scene; don't fabricate captures.

**D1: Architecture and ceiling first.** Build one connected three-space site and small kit, physically legible Undertow Knot, useful lighting. Do not alter all biomes/dungeons. Gate: walk and understand spatial structure at gameplay camera and lower quality.

**D2: Honest three-enemy implementation.** One complete distinct action per archetype, authoritative pose/shield/hit collision, clear counters and useful reward(s) that truly reuse mechanic. Gate: solo/pair motion recordings, terrain-edit obstruction regression, no fake invulnerability.

**D3: Alternate routes, procedural validation, systemic mechanism.** Two route types, one recoverable setback, environment power/terrain changing actual state, strict legacy objective and claim support. Gate: two completions same seed, varied fixed seeds, save/load.

**D4: Integrated benchmark/performance.** Run gameplay route and enemy stress simultaneously, visual coherence on low and high settings, CPU/GPU/memory breakdown, judicious optimizations. Gate: demonstrably playable and bounded resources with exact measurements.

Ship each gate before broadening catalogs, magic effects or adding more enemy species. Agent prompts in `03_CLAUDE_CODE_IMPLEMENTATION_PROMPTS_V2.md` specify owned work and handoffs.

## 12. Art/UX acceptance scorecard

Score each criterion **0=not verified, 1=weak, 2=usable, 3=strong** after actual play; do not assign an unearned 3 from mockups:

- Room silhouette and purpose recognizable from entrance.
- Ceiling/support/construction legible without labels.
- Principal objective and at least one exit legible at gameplay exposure.
- Calm vs magical materials have disciplined hierarchy.
- Enemy archetypes distinguishable by shape and motion.
- Windup, commitment and recovery legible; hit volumes honest.
- Alternative route has meaningful different cost, not cosmetic divergence.
- One recoverable setback changes player decisions.
- Environment systems (terrain/power/light/mechanism) share authoritative state.
- Rewards and claims are persistent, useful and not duplicated.
- Same seed stable across save/reload and concurrent edits.
- Actual CPU/GPU budget measured with worst relevant effects.

**Exit bar:** none of first seven criteria at 0; all gameplay/integrity checks pass or explicitly remain unshipped; a representative new player can complete one route without verbally guided instructions. Captures and real motion evidence must be provided separately.

## 13. Source/status continuity

Based on the user's supplied `voxel_dungeon_quality_overhaul(3).txt`, including the exact existing Astral Sanctum, Undertow Knot, Warden, Aether Moth, crab archetype, phantom-floor challenge, guardian claim and lighting/material constraints. These are **preserved and developed**, not replaced with a generic fantasy procedural dungeon proposal.

The science/electronics brief supplies the future path for true power isolation, controller modules, salvage and shared state. Its absence must not block D1 or D2: use existing verified machines and world features, then integrate incrementally.

External design lessons: Noita official site https://noitagame.com ; Factorio developer article on bounded creature collision/path work https://www.factorio.com/blog/post/fff-425 ; official Minecraft Warden design https://www.minecraft.net/en-us/article/meet-warden ; FTL game description https://store.steampowered.com/app/212680/FTL_Faster_Than_Light/ . These are comparative principles, not claims of shared implementation algorithms or copied character IP.
