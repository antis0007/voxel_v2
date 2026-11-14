# AXIOM / UNDERKINGDOM — THE MERIDIAN ANCHOR
## Orbital megastructure and living space-elevator Earthport — Design & engineering specification v2.0
**Date:** 2026-10-26  
**Status:** Original proposal for the Rust + WGPU project; not implemented, modeled, profiled, or validated in the game.  
**Evidence baseline:** user-provided native screenshot `image(20261026-230059).png`: unsupported-looking narrow mast, disconnected overhead trusses, visually dominant elevated slabs, sparse concrete volumes, weak tether, dark undersides, crude mountainous surroundings.  
**Compatibility:** current worldgen versioning, persistent save state, mixed-size voxels, machine/power/logistics systems, Underkingdom civilization lore, Axiom SNES/GBA pixel-art direction. Inspect current repository for supported features; do not assume earlier design proposals are implemented.

---

## A. Design thesis and reject criteria

**Thesis:** The station is simultaneously (1) the planet's most recognizable artificial landmark, (2) an inhabited and physically functioning salvage city, (3) a great three-dimensional dungeon built in and under an ancient machine, and (4) a credible connection to orbital industry. It must have a distinctive shape and understandable operation **before** detail, shaders, lore text, or particle effects are added.

**Immediate rejection criteria:**
- Seen from several kilometers away, does not obviously connect land to sky.
- Viewed from the arrival road, the first visual impression is an overhead highway or cranes.
- The central support is a skinny ungrounded pole or the cable is rendered as a pillar of uniform square blocks all the way upward.
- Bridge clutter and dark undersides obscure the tether.
- Buildings are disconnected flat boxes, randomly placed props, or generic concrete high-rises.
- The player cannot distinguish logistics/power/docking/maintenance/living areas from silhouettes and geometry.
- It only works in one screenshot, weather condition, seed, or voxel scale.
- A part looks functional but has no documented physical or interactive policy.
- Old generator output is overwritten, or persistent edits are lost.

## B. Reference lessons; what is and is not sourced

1. **Ace Combat 7 — International Space Elevator / Lighthouse:** Strong radial Earthport composition, concentrated main shaft, windbreak/support structures, tether reaching orbit, complete transport installation. Transfer *silhouette discipline and clarity*, not proprietary geometry. Sources: https://acecombat.wiki.gg/wiki/Space_elevator ; reference screenshots and official game.
2. **Ace Combat 8: Wings of Theve (released Oct 2 2026):** Extraordinary sky/cloud presentation, aircraft/superweapon sense of scale, personal first-person moments between monumental exterior events; **no verified original space elevator design from AC8**. Source: https://www.bandainamcoent.com/games/ace-combat-8 ; https://en.bandainamcoent.eu/ace-combat/news/ace-combat-8-wings-of-theve-developer-diary .
3. **Seveneves — Neal Stephenson:** Near-Earth orbital engineering, debris hazards, cascading infrastructure risks, repair, logistical complexity, deep time, human-scale survival inside enormous technical systems. Do **not** treat the novel as a blueprint for a conventional ground-based GEO tether, and do not confuse a rotating skyhook with a stationary tether. Source: https://www.nealstephenson.com/seveneves.html .
4. **RimWorld Odyssey:** Player-built mobile base with bedrooms, workshops, hospital, power, sealed spaces, survival requirements, crew/work priorities, wreck exploration, salvage, repurposing, orbital travel. Source: https://ludeon.com/blog/2025/06/odyssey-preview-2-gravships-and-space/ .
5. **The Wandering Earth II:** Industrial authenticity of a megastructure: cable bundles, climber interfaces, transfer bays, shuttle machinery, giant load-bearing form; avoid reusing specific protected designs. Source artist pages: https://mattabellamy.artstation.com/projects/xDzkn1 ; https://www.artstation.com/artwork/dKOkg3 .
6. **Gundam 00:** Orbital transportation as power and geopolitics; the installation's operation changes who can transport energy, people and material.
7. **ISEC engineering:** Earth Port + tether + climber + GEO Node + upper Apex Anchor; tether in *tension*, not an infinitely tall compression tower. See: https://www.isec.org/faq ; https://www.isec.org/studies/ ; https://www.isec.org/2016-study .
8. **Underkingdom architecture:** Monumental pale retaining masses framing dramatic vertical negative space, deliberately clustered warm roofs and human-scale additions; three historical construction eras. The user's accessible Витаутас Лайсонас reference is especially relevant; other named Downloads references must be inspected by whichever agent can access them before they are described in detail.

## C. Canonical physics and lore: decide once

**Preferred identity:** An equatorial *tensioned* Founder-era ribbon/bundle of load-bearing filaments, connected to a geosynchronous transit node and higher counterweight. The Founders manufactured an exotic structural material; optional **aether trim** manages alignment, vibration and localized faults but does not invisibly replace the mechanical model. The surface anchor transfers local loads into engineered bedrock, does not support thousands of kilometers of tower by compression. Climber propulsion draws real modeled power and conveys cargo with ledger/accounting.

**Planet parameters prerequisite:** Document planet rotation period, gravity and geosynchronous orbital radius in a testable configuration. If the current site is not near its equator or world gravity differs significantly, choose and describe one explicit alternative: (a) regenerate/locate the canonical equatorial port, (b) mechanically distinct **active orbital-ring** suspended descent line with powered stabilization, or (c) openly fictional aether-stabilized tether with measured field/energy requirements. Never silently break orbital geometry.

**Orbital ring consistency:** The Founders' visible orbital ring and the GEO node need not be the same body. Preferred: tether ends at a station in geosynchronous orbit; traffic transfers by vehicles to the separately orbiting ring/shipyard. This prevents pretending a low-orbit moving ring is a stationary GEO anchor. Later technology can connect the systems with separately modeled dynamic interfaces.

**Seveneves-style rotating skyhook:** A *different* optional high-level infrastructure system. It swings and transfers momentum according to orbit and rotation; it is not the elevator tether and should not be visually indistinguishable from it. A distant skyhook event is a spectacular, predictable late-game spectacle and engineering challenge.

**Faults:** Local failures can damage a climber, cargo, station sector, alignment or power. A tether-wide structural catastrophe is not an untelegraphed casual random event wiping the world. The player receives staged warnings, mitigation, rescue/escape options and persistence-safe consequences.

## D. Signature silhouette: the Meridian Crown

### D1. Four silhouette elements
1. **The skyward filament:** One unbroken extremely long, narrow, dark luminous-at-edges *ribbon or bundled set of ribbons* passing upward into weather, thinning with altitude, not a kilometer-thick opaque column. Its main silhouette dominates the center negative space.
2. **The anchoring crown:** Three or four staggered massive white/blackglass **buttress-fins** / pylons, angled inward only at low altitude, enclosing a **large open void**; bridges are low/short and tangential, not giant slabs across the viewing axis. Functional ring collars around the ribbon at ground port are incomplete or carefully placed so they do not obscure sightlines.
3. **The basin walls:** A terraced artificial canyon cut into volcanic/granitic bedrock, tall pale retaining walls and machine foundations embedded in rock; deep central canal/rail approach and generous arrival plazas. No freestanding rectangular skyscrapers.
4. **The inhabited edge:** Dense, small, warm-roofed terraces and industrial retrofits clinging to walls, connected by stairs, rail lifts, cranes, catwalks and service tunnels. Their human scale reveals the immensity of the Founder structures.

### D2. Three readable material eras
- **Founders:** pale ceramic/stone composite, smooth large monolithic blocks, indigo-black vitreous tensile ribs, thin patterned metallic interfaces, sparse geometric optical alignment markers, pristine surfaces where protected; inexplicably elegant but logically constructed.
- **Engineers:** functional iron, oxidized copper, heavy valves, gearhouses, substation towers, pressure pipes, rails, repair scaffolding, replacement anchors, heat-resistant masonry; signs of industrial improvisation and patchwork.
- **Present inhabitants:** clustered clay/ochre/slate roofs, timber bays, markets, schools, medical stations, rope laundry, gardens, lamps, careful repairs; inhabitation is valuable, not random clutter.
- **Choristers (sparse and purposeful):** resonance chambers and tuned stone/ceramic arrays deep in anchor vault; restrained gold-inlay inscriptions, distinct acoustic architecture, not glowing runes on every surface.

### D3. Compositional order and camera staging
**Kilometers away:** major silhouette, anchored valley, one filament disappearing into the upper sky; atmospheric height and visible climber specks.
**Approach route:** from below the valley's lip, flanking pale walls and smaller terraced buildings create strong parallax; railway points toward the central structure.
**Main civic terrace:** wide negative space allows a clear look up; foot traffic and cargo trains cross at low level; the main cable remains uncluttered.
**Climber platform:** the actual vehicle and articulated charging/guide/load-transfer machinery become focal point, framed by huge Founder fins.
**Anchor vault:** inverted cathedral: huge tension drums/strain frames, shadowed galleries, accessible repair ways, real drainage and power; sky visible via a great central slot at selected viewpoints.
**Upper observation:** an uninterrupted upward view of the filament cutting through clouds, with additional ground-level detail providing scale.

### D4. Proposed scales (hypotheses, not fixed without camera/streaming proof)
- Regional silhouette footprint: 1–2 km landscape composition with lake/river/rail outport and cliffs.
- Main inhabited precinct: ~300–700 m across, sufficiently large for actual houses and facilities.
- Monumental crown: ~100–250 m wide, main lower fins/anchor arches possibly 120–300 m tall depending on terrain and supported worldgen.
- Cargo climber: physically walkable multi-room entity, ~12–35 m overall depending on role; actual size chosen from player/vehicle/collision tests.
- Underground vault: several major halls with purposeful spans and heights, connected by real traversable maintenance paths, not concentric corridor rings.
These sizes must be validated in native camera scenes and adjusted to represent the current game's scale, view distance and simulation budgets.

## E. Site plan: actual places and routes

**1. The Pilgrim Road / arrival station:** Intermodal rail/caravan dock, provisions, visitor lodgings, customs/quarantine interpreted through current faction/lore, observational terrace, physical access to site. Not a two-kilometer empty concrete avenue. Visual cues point to active people.

**2. The Lower Crescent / civic town:** Asymmetric streets of repair shops, food gardens, archival kiosks, small dwellings, terraces and marketplaces; functional public power and water. NPCs/creatures have schedules, roles and meaningful interactions; late player can improve safety and prosperity.

**3. The Freight Basin:** Rail fan, cranes that have visible pivots and rails, pallet/storage lots with real ownership, conveyor/feedstock interfaces, customs/sorting facility, empty overflow lots; near scenes communicate cargo flow clearly. Trains follow paths with collision/headroom.

**4. The Crown / tether terminus:** Open-top central machinery, tension readouts, structural nodes, service gantries, heavy power terminals, station level climber dock and control room. Aesthetic giant arcs only where supported by load paths or actual operational purpose.

**5. The Power Spine:** Retrofit electric substation, physical conductors and return, conversion equipment, energy stores, cooling loops and safety equipment. Partly running on old power infrastructure. Optional arcane field trim with explicit energy use/risks.

**6. The Upper Gallery:** Observatories, meteorology lab, tether inspection, telescopes, dangerous exterior gantries, special vistas. Allows player to see moving climbers and changing cloud layers.

**7. The Deep Footings:** Hundreds of meters of descending Founders' masonry, tension-ground attachment, helical corridors and massive walls; Engineer rail burrows and abandoned heat exchangers intersect the ancient geometry.

**8. The Choir Chamber:** An unusual acoustic/resonance apparatus for damping dangerous tether oscillations. Magic and engineering solve the same physical issue through distinct, plausible subsystems.

**9. The Restricted Archive:** Founder manufacturing logs, maintenance blueprints, records of the orbital ring; physical conditions and environmental puzzles, not inventory-key-only locks.

**10. The External Outport:** Rail, barge or sea connections to geographically plausible resources; provides alternate player entry, automated logistics, and large-scale industrial projects. A mountainous anchor can have a remote coastal freight node without inventing an ocean directly below it.

### Path constraints
- Circulation graph: minimum two practical entrances to each major accessible precinct; main route clear for baseline traversal; at least one legitimate engineering/magical alternate path for important gates.
- Logistics routes: explicit horizontal and vertical path envelopes for climbers, lifts, freight rails and maintenance cranes; prevent overlaps at generation time.
- Protected voids: reserve skyward sight cone around tether, entire climber motion envelope, maintenance access, vaulted atria, playable central plaza, structural inspection zones.
- Site layers generated from function → circulation → masses → supports → detail; decorations cannot backfill protected space.

## F. The tether: display, physics and orbital connection

**Canonical data**: tether ID, datum, planet model ref, orbital nodes, anchor terminals, piecewise analytic centerline, cross-section/bundle data, climber lanes, tension/stability aggregate state, temperature/radiation/debris exposure bands, damage regions, permitted transports. It is not stored as a row of tens of millions of voxels.

**Rendering LODs**:
1. **Interactive 0–200 m**: actual object-local mesh / cable bundle with material IDs, service access and local collision.
2. **Near atmospheric**: camera-relative extruded ribbon/swept narrow geometry with stable tangent frame and predictable tapered appearance; no dense voxels.
3. **Far atmospheric**: parameterized line/very thin ribbon with atmospheric extinction, mathematically stable perspective, anti-aliasing and correct occlusion from clouds/terrain when available.
4. **Orbital view**: same authoritative trajectory, with planet-scale coordinate transforms and high-precision local origin.
No uniform luminous beam or all-distance fixed-pixel-width line; avoid visible discontinuities when exchanging representations. Use validation of temporal stability in movement/weather.

**Physics simulation**: A reduced axial/lateral segment model at significant nodes and potential failure points, or an analytic quasi-static model with bounded dynamic modes, adequate to drive gameplay. It need not solve continuous 100,000-km full-fidelity nonlinear dynamics. Track load from active climbers, anchors, scheduled space hazards and stabilization modules; require conserved accounting for energy/cargo and explicit model approximation. No per-voxel physics for the cable.

**Climbers**: Object-local volumes, stable IDs, cargo/passenger inventories, power and thermal states, attachment to tether lane, optional crew and life support, station-to-station reservation/schedule and arrival events. Travel time can be accelerated/abstracted with player-visible transit phases and optional onboard gameplay; do not require idle multi-day waiting. Active player's local cabin is rendered as a contained physical interior, distant climbers as analytically located meshes/proxies.

**Orbital nodes**: At geosynchronous altitude, attach a real station and transfer interfaces; a counterweight apex beyond GEO maintains the overall tension, separated from lower transfer operations. The visible Founder ring remains a distinct orbiting system accessed by spacecraft/tugs or a later explicit hybrid transport mechanism.

## G. What makes it a playable megastructure

### G1. Four-mode ownership, not a linear quest
**Explorer:** map neglected routes, find maintenance doors, bypass by digging or fluid work, discover archives and unusual environments. Restoring ordinary infrastructure is useful but not mandatory for finding secrets.
**Engineer:** repair docks and energy networks, design climb power supply, install instruments, automate cargo, repair guides and damping machinery. Large factories eventually supply materials efficiently.
**Mage:** read Chorister resonance chambers, damp oscillations with tuned spell fields, manipulate energy and inaccessible mechanisms; still subject to typed costs and discoverable world rules.
**Settlement-builder:** repair homes, clear hazards, install defenses, rescue crew, reopen civic services and make the precinct habitable; survival and humane play are valued.

### G2. Progressive states with persistent consequences
S0: long-abandoned; tether visually present but alignment unstable, station dim, communities struggle, docks partly unusable.
S1: public entry repaired; sewage/water, safe paths, lights, trader station and sheltered base function.
S2: freight and stabilization systems restored; powered dock starts, maintenance climber can conduct short test excursions.
S3: first operational cargo/passenger ascent; the player can ride; orbital transfer location opens with safety constraints.
S4: regional transport and ring traffic restored; scalable industry and orbital colony development possible.
States are **emergent criteria/observed capabilities**, not a single quest flag switching the whole art set. Partial restoration changes lighting, occupancy, animations, trading and navigation.

### G3. FTL/RimWorld systems and events
- A damaged climber during ascent: power bus damaged, redundant battery shortfall, pressure leak, cooling issues, repair drone route blocked, communications interrupted. Player may reroute systems, use backup supplies, retreat, or plan a rescue. Distinct warnings and multiple solutions.
- A cargo bottleneck: machines functional but freight store full; player reroutes rail capacity or installs additional automated handlers; no invented free cargo.
- Atmospheric storm: docking restricted due to wind/shear; player can inspect and use reserve stability system or simply wait/adjust schedule. Not every rainy day causes a catastrophe.
- Orbital debris alert: trajectory summarized, safe window, shielding or diversion option; impacts localized, legible and recoverable. No sudden silent world's-end.
- Anchor chamber resonance fault: Chorister tuned array plus Engineer vibration dampers can solve it by either discipline or hybrid method; player learns a genuine reusable mechanism.
- Settlement needs: people need safety, food, heat, light, medical care and access; restoring them is meaningful and produces gratitude, services, lore and labor, not obligatory punishment simulation.

### G4. Deep secrets and replay value
**The seventh filament:** six load-bearing paths are visible in blueprints, but one hidden strand appears in resonance measurements; it leads to older Founder control spaces.
**Buried station beneath station:** present-day ground terminal rests above a first-generation, abandoned anchor design deep below bedrock.
**The false counterweight:** the declared apex mass doesn't match tension logs. A concealed orbital mass or active mechanism exists.
**Ghost climber:** historic travel records reveal a climber that never arrived, still logged as traversing an impossible intermediate waypoint.
**Fungal gardens in the heat exchanger:** a neglected biological cooling adaptation reveals unique materials and a player-cultivable strain.
**Rain shadow map:** at a particular seasonal sky configuration, the cable casts a detectable pattern that aligns with an ancient geographic map; clues exist and player measures it (avoid requiring real-time multi-hour waiting).
Each secret has multiple clue types, deterministic seed-relative placement, recovery paths, and a concrete new capability/opportunity; no single fixed coordinate or wiki-only solution.

## H. Axiom art direction: science fiction filtered through SNES/GBA pixel-art

**Macro**: monumentality from proportions and negative space, not from thousands of tiny polygons. White ivory composite meets violet-gray shadow, charcoal-black tension ribs and localized oxidized copper, then tiny warm human habitation.
**Palette**: 3–5 deliberate bands per material, hue shifted; no normal-map grunge. Local contrast hierarchy: cable 1st, anchor crown 2nd, civic buildings 3rd, detailed trim 4th.
**Textures**: test world texel density 16/32/48 px/m in real gameplay; authored clustered pixels, small material palette; proper mip/atlas padding and camera motion testing. Intentional stone joints and erosion, distinguish Engineer repair from Founder ceramic.
**Geometry**: large curved/repeating forms authored in Blender as modular arc segments with narrow angular facets. Reusable supports, buttress fins, cable drums, loading mechanisms, rail modules, stairs, facades, arch modules, roofs and machine bases. Named pivots, ports, collision proxies, LOD and damage representation.
**Atmosphere**: AC8-inspired multi-layer clouds, humidity and lighting around **actual** cable visibility; low clouds intersect lower pylons with subtle shadows, high clouds cross the line, atmospheric extinction indicates height. No sweeping nonphysical opaque glow tower.
**Lighting**: Foundation in cool diffuse shade, high pale material readability, warm inhabited clusters, active machinery emission where actually powered; night emphasizes the tether by sparse maintenance beacons and moving climbers. Avoid flat yellow torch wash and completely black underpasses.
**Animation**: operational lifts, moving gantries, train arrivals, cable inspection crawlers and people make scale legible. Mechanical animation tied to state rather than ambient decoration.
**Sound**: distant low harmonic from tension machinery, periodic freight horns, venting pressure, footfalls on different floors, high-altitude wind, local character conversations; Chorister resonance as a clear tone pattern with actual puzzle utility.
**Set dressing**: use real repeated industrial systems and habitation needs. Do not scatter magical rings, generic holograms, random cranes or bolts purely as wallpaper.

## I. Worldgen & implementation in Rust/WGPU

1. `SiteIntent`: climate/equator/topology constraint, landform, three-era history, functions, anchor trajectory.
2. `TerrainPlan`: carve basin, ridge terracing, realistic cut/fill spoil, railway gradients, drainage, deep foundations.
3. `SkySightPlan`: high-priority visible void and camera corridors; geometry must not close these accidentally.
4. `CirculationPlan`: playable walkways, plazas, lifts, cargo clearance, protected climber envelope, entry/exit, alternate routes.
5. `StructurePlan`: crown masses, buttress fins, giant enclosing walls, arch/collar position, utility structures.
6. `FacilityPlan`: machines with stable authority, electrical/thermal/hydraulic networks, logistics, control, safety.
7. `DistrictPlan`: dense clustered habitation tied to jobs/roads, market, small architecture with legible doors/stairs.
8. `DetailPlan`: roof geometry, windows, small stone joints, cable anchors, furniture, vegetation, damage/weathering appropriate to district.
9. `LODPlan`: persistent coarse proxies derived from same authoritative geometry, no disconnected pieces or holes; tether as analytic hierarchy.
10. `ValidatePlan`: structural support, traversability, roof/cable intersection, functional clearances, lighting, edit/persistence, seed and scale sampling.

**Storage/authority:** Site and world generation deterministic by seed/version, but player repairs and edits persistent via authoritative world changes; cache meshes and collision carry revisions. Inactive climber/factory activity is advanced by compact models not millions of cell updates. Scaled voxels follow real dimensions and supported spatial hierarchy; don't fake currently unsupported detail resolution. No generator-54/55 modification in place: introduce a versioned generator change if geometry rules change.

**Performance:** No per-frame traversal of every cable segment or static station voxel. Instanced repeating geometry, chunk culling, indirect draws where feature-supported, shared GPU material families. Prebuild/stream collider summaries, group static geometry in reuse-friendly batches, delegate machinery to network solvers, crews to distant AI LOD. No huge dynamic lights on every beacon; no dense cloud particles around every cubic meter of cable. Budgeted incremental generation, mesh/upload queue, geometric LOD and streaming prefetch for ascent.

**Review:** Must compare CPU/GPU/p95/p99 frame time and VRAM to captured baseline in same scene configuration. Long-range cable continuity, visual readability and edit persistence are separate pass/fail gates. Must include deterministic tests, collision traversal, player view and external silhouette captures; cannot claim complete based on build alone.

## J. Native camera review and acceptance matrix

**Mandatory cameras:**
1. 3–5 km landmark approach, clear sky (dominant tether).
2. 800 m arrival rail, late afternoon (architecture hierarchy).
3. 200 m city street, overcast (human scale and habitat).
4. 60 m plaza view straight up, daytime (unobstructed climber/tether).
5. Climber dock at eye level, night operation (mechanical clarity).
6. Upper gallery overview, storms (climbers and weather).
7. Deep anchor vault, functioning/emergency lighting (negative space and machine storytelling).
8. Orbital node looking planetward (correct trajectory and planetary backdrop).
9. Fast-flying exterior camera traversing distance-based LOD boundaries (temporal correctness).
10. Explorable climber interior with player hand/viewmodel visible (ergonomics and state communication).

**Functional acceptance**:
- Players can reach the terminal without clipping/intersection, find multiple routes, identify power/cargo mechanisms, and interact with at least one functional one.
- Tether continuous from near to far and orbit; no fake tower, missing patches, jitter, z-fighting, always-on-neon ribbon or huge performance hit.
- A real cargo object/inventory lot moves from player factory through station to climber/terminal with stable identity and no dupe/save bugs.
- Damaged facility can be meaningfully repaired through tech, magic or physically modifying the environment as supported; visual state follows authority.
- One optional secret is discoverable from multiple environmental hints without external walkthrough.
- Player can make civic parts safe and inhabited; no endless hostile spawn within secured district.
- Procedurally varied examples retain recognizable anchor silhouette, clear roads and no giant horizontal slab across sky.
- Original historical worlds unaffected, and concurrent code owners' edits preserved.

## K. Delivery plan (small verified vertical slices)

**M0 Audit & preservation (no code):** Actual screenshots, current source inspection, scene receipts, sky trajectory, generator version, open file owners, material/mesh pipeline, profiler baseline. Verify build hash and seeded world. No invented files/metrics.

**M1 Form pass (art blockout):** Three black/white silhouettes: (A) Anchor Crown, (B) Buried Crescent, (C) Pillar of Witnesses. Use actual Blender or parametric worldgen blockout; evaluate 10 fixed cameras. Choose one or hybrid only after physical feasibility and readability. Delete stray spans and generic skyscrapers. No shader overhaul.

**M2 Worldgen mass and tether:** Build terrain cut/retaining geometry, correct support footings, open sight cones, truly continuous ribbon analytic LOD, native capture at near/far/storm/day. Keep existing world versions intact.

**M3 Inhabited site and actual kit:** Modular walkable stairs, roofs, workshops, freight, power area, realistic doors, civil services; detail texel consistency and lighting. Camera evidence, traversal tests, benchmark.

**M4 Working terminal:** One climber + one real network + one cargo transfer; player boards and obtains meaningful new exploration interface; persistence, cost, power, failure modes tested.

**M5 Underworks secret:** A small extraordinary vaulted chamber; one physical puzzle with multiple solutions, clues, a useful reward and optional lore discovery. No arbitrary locks.

**M6 Orbital extension:** Orbital node/transfer platform, real survival systems, space salvage, ring glimpses; reuse ship/life-support/vehicle abstractions.

**M7 Scalability & variety:** Multiple seeds, 1m plus supported subscale worlds, improved cache/LOD, stress tests, crew and cargo handling, economy, strong art review and final parity gate.

**No code claim:** This is a proposal; no user repository edited, no new native screenshot taken, no Blender scene rendered by preparing this document.

## L. Execution prompt to give a coding/Blender agent

> **Task: Meridian Anchor v2 — First quality gate, not final completion.**
>
> Read the current architecture, art, generator 54/55 notes, orbital lore and native screenshot baseline. Inspect repository branch/dirty state and note concurrent owners. Verify real Blender/importer and Rust/WGPU capabilities. Do not overwrite existing saved worlds. Compare reference principles from Ace Combat 7's Earthport, Ace Combat 8's atmospheric presentation, Seveneves' consequential orbital engineering, RimWorld Odyssey's inhabited machinery, Wandering Earth II's loading/structural detail, and our Underkingdom Founder/Engineer/Chorister cultures. Use reference images for principles, not literal copies.
>
> **Deliver M0 and M1 only.** M0: verified source/owner map, reproducible scene, native baseline captures, measured profile, current supported voxel scales/asset formats and site version. M1: three intentionally distinct grayscale large-form site blockouts, with unchanged seed/camera/scale and 10 prescribed viewpoints. At least one candidate must have a true unobstructed filament and clear massive anchorage. Reject highway-like horizontal slabs, floating supports, generic cubes and unrelated scaffolding. Provide objective comparison: tether sky continuity, landmark recognizability, material/civilization readability, walkable routes, production feasibility and rendering cost. Select one design and produce an explicit M2 edit plan; do not prematurely expand into hundreds of props or change shared shader files. Use Blender scripts and actual Blender UI or WGPU captures where available; clearly distinguish execution from proposals. Report tests and actual numbers, not estimates disguised as measurements.
>
> Exit only after the approved candidate is captured in the native camera with the correct scale and clearly better silhouette than the user-supplied baseline. If native access is unavailable, deliver validated Blender scene sources or generator blockout code and mark the in-game result unverified. Do not claim the station or orbit system is complete.
