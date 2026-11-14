# Execution packets — one active proof at a time

A packet is a small implementation program, not a promise to finish a whole subsystem in one response. Each starts with one specifically bounded proof; later increments are listed separately. Select one proof, fill its source paths/commands from the repository, and finish or honestly block it. Do not expand to its next increment automatically unless the user's requested scope includes that continuation. Dependencies below mean required behavior, not a requirement to rewrite an already working system. No task is pre-certified implemented.

Use `DESIGN.md` only by the section references on the selected packet. Always preserve D00/D05 authority and compatibility. Refer to `ACCEPTANCE.json` for inherited and added requirements. A packet may satisfy only a subset of its mapped requirements; record precisely which. A partial proof never marks the whole packet or its eventual feature complete.

## Work-card template

Before an implementation diff, fill this compact record in the existing tracker or `SESSION_STATE.md`:

```text
Packet / bounded proof:
Player-visible result:
Inspected source paths and ownership:
Existing behavior to preserve:
Actual prerequisites and evidence:
Hypothesis or missing behavior:
Smallest reproduction / failing check:
Proposed change and rejected easy shortcut:
Allowed changes / excluded modules:
Relevant acceptance IDs:
Commands from the repository:
Native reproduction, camera/save/settings:
Performance and memory measurements if affected:
Rollback / migration policy:
Completion / blocked stop condition:
```

Do not require an elaborate system design document for a local bug. If the proof involves a new solver, persistent schema, numerical authority boundary, network protocol, or new renderer data layout, add a short design note covering alternatives, supported range and test oracle before production integration.

## T00 — Reproduce the current game and choose one repair

**Prerequisites:** none. **Read:** D00, D13, D18; root/local agent instructions and the relevant actual source. **Purpose:** establish which defect in the supplied image or current gameplay actually exists and produce a reliable fixture.

Inspect working tree, branch/HEAD, current instructions, active owners, actual launch/build/test scripts, save/generator versions and asset tracking. Do not scan every historical document or map the entire repository when a render fixture is sufficient. The latest provided scene may have changed; reproduce current behavior rather than force obsolete geometry back into the game.

Freeze one save/seed and camera receipt. Capture normal gameplay, clean world without viewmodel, neutral daylight/clay, and the *specific* diagnostics needed for the suspected fault. Record settings, source/dirty state and executable identity. Gather one relevant timing/memory baseline if performance is part of the symptom.

**First proof:** the scene can be reproduced, and one actual cause or constrained hypothesis is identified. If an equivalent harness is missing, implement its smallest useful addition. If a safe bounded defect is found, continue into the appropriate card rather than stop with a giant new plan. Default visual choice after correctness is T03, not a universal shader rewrite.

**Required checks:** reproduced issue vs prior screenshot ambiguity; commands exist; no hidden untracked required asset; user/peer edits preserved. **Do not:** migrate saves, upgrade WGPU, create a new world engine or launch multiple unrelated tracks.

**Stop:** one selected proof implemented/verified, or exact environmental/ownership blocker documented with safe completed work. A reproduction harness is tooling, not the completed visual overhaul.

## T01 — Publish correct terrain coverage and local edits

**Prerequisites:** T00 behavior. **Read:** D05, D14, D17. **First proof:** fix one demonstrated resident/proxy/edit transition that currently renders wrong geometry or collision.

Trace canonical query → mesh inputs → neighbor/seam dependencies → upload → active ownership → retirement. Add a fixture with a surface, cave opening and lower passage sharing XZ space. Delay and reorder one child upload, return explicit empty data, edit during generation, and cross a coarse/fine edge and corner.

Change the smallest actual fault: dependency stamp, owner test, boundary topology, replacement frontier or fast patch. Keep old valid coverage until the replacement and required seam are ready. Distinguish a legitimate hole from missing occupied surface. Preserve critical edit/collision alignment. A view-facing screen-space hack does not repair the source topology.

**Proof:** no new holes or false filled passages in that fixture, stale job cannot resurrect edits, save/reload keeps changes, repeat approach/recede in motion. Test supported voxel sizes actually present. **Measure:** mesh/install/upload cost and overlap memory. **Exclude:** new geology, aesthetic grading and replacement world storage.

**Later increments:** other mesher boundary families, 3D exception coverage, multiple origin frames, wider stress tests. Do not assume one successful edge fixture proves all of them.

## T02 — Isolate shadow and atmospheric errors

**Prerequisites:** T00 behavior; use T01 only where confirmed missing geometry blocks diagnosis. **Read:** D13–D14. **First proof:** classify one suspicious dark/washed region and fix its demonstrated cause.

Preserve existing diagnostics. Add CSM-only, far-visibility-only, combined, validity/domain and disagreement views only where absent. Debug output must have a known color/tonemap path. Freeze sun/camera and test one low-angle and one midday state. Compare with geometry/depth and a controlled thin wall/slope scene.

If the cause is shadow geometry, fix transform, bias, validity or caster representation—not ambient brightness. If it is atmospheric duplication, isolate each layer's transmittance/source contribution and identify which pass already owns it. Changing a combine operator without caster-domain reasoning fails.

**Proof:** known legitimate occluders still shadow; false effect disappears under matched settings; movement/cascade boundary remains stable. **Measure:** relevant passes and scene totals; no unsupported FPS claim. **Exclude:** new terrain generator, full virtual-shadow-map system, arbitrary disabling of real shadows.

**Later increments:** validated cached far visibility, clustered local lights, bounded portal lighting, material-format cleanup. These are separate proofs.

## T03 — Ground surfaces that mean something

**Prerequisites:** T00 behavior. **Read:** D06, D13. **First proof:** replace the current flat green/gray ambiguity for one cause in the existing generator while preserving its broad composition.

Locate where exposed rock, gravel, bare soil, route surfaces and construction clearing are classified. Determine whether gray patches are actual material, slope exposure, clearing masks, or a render fallback. Do not assume from the screenshot. Choose one class—for example exposed bedrock—and make its occupied relief, boundary treatment and material/habitat metadata agree.

Use existing geological/terrain data if valid. If unavailable, introduce a minimal versioned cause field with explicit limits; do not pretend it is tectonics. Preserve drainage and site paths. A bedrock shelf should be a shape, not merely a noisy decal. Nearby habitat should use that substrate classification, but full plant succession is not a prerequisite.

**Proof:** before/after at walking and aerial views; a material-inspection/debug view confirms identity; player mines/builds correctly at contacts; old worlds remain unchanged under the proper generator policy. Inspect several predetermined seeds rather than selecting a flattering one.

**Measure:** generation/mesh cost, changed volume and patch count. **Exclude:** global biome rewrite, wholesale erosion simulation, added landmark catalog and more random grass scatter.

**Later increments:** litter/soil, sediment deposits, maintained paths, construction disturbance. Complete each identity before combining them into a terrain gallery.

## T04 — One excellent fern and one excellent shrub

**Prerequisites:** T00 behavior; substrate hookup can use T03 when present. **Read:** D07, D13–D14. **First proof:** one fern and one shrub look good and have one persistent interaction in the actual world.

Verify real asset tooling/import format. Create editable source geometry and intentional texture/UV/material assets. Fern: coherent curved fronds, organized leaflets and an anchored base. Shrub: branching near the ground, crown masses and negative space. Review silhouette before fine surface work. Native near/mid/far representations preserve coverage and recognizable morphology.

Add one harvest/cut interaction using existing material/inventory authority. Persist changed organ state; reload cannot restore harvested stock. Implement coherent bounded wind and a transient push response separately from authoritative damage. No full ecosystem rewrite is needed for this proof.

**Proof:** neutral/sunset/cave light; native first-person walk-through; one valid cut/harvest and reload; LOD transition in motion; no per-leaf ECS body. **Measure:** draw/material batches, triangles/overdraw/shadow cost and instance memory against current equivalent coverage. **Exclude:** species-count targets, a forest-wide biology solver, copied reference textures.

**Later increments:** grouped forest stands, tree branching, habitat composition, succession via T08. A documented placeholder is not an accepted master asset.

## T05 — An attractive building that occupies a site

**Prerequisites:** T00 behavior. **Read:** D08, D13. **First proof:** improve the existing shown building and its immediate usable grounds, without replacing the settlement generator.

Preserve its promising warm roof/pale trim/dark wall language. Identify actual entrance and intact/ruin state. Refine eaves/ridge/corner caps at supported local detail. Establish one usable approach, a working/inhabited yard and the relationship to one neighbor. Do not attach decorative stairways to inaccessible doors. Check foundations, door widths and real circulation before props.

Expose new compatible parts in ordinary construction/creative interfaces where supported. Use genuine supported voxel or mesh interaction semantics; finer decorative geometry must not claim physical cuttability it lacks. Preserve old generator layouts and player changes.

**Proof:** front/back/interior/roof/arrival views plus walking route and player reproduction of a useful module. Source materials, collisions and placement agree. **Measure:** generation and render delta. **Exclude:** new city-history engine, distant elevator rebuild, hundreds of modules.

**Later increments:** repair/damage grammar, site utilities, additional construction kit junctions, settlement-scale plans.

## T06 — Remove one daily usability bottleneck

**Prerequisites:** T00 behavior. **Read:** D02–D05. **First proof:** one real inventory/building interaction becomes reliable and easier, selected from the observed game.

Prefer quick-stack/sort with locked slots or the local storage scanner foundation if missing. Map existing item/lot ownership; aggregate recipe demands; respect permissions and capacity; define deterministic ordering. Do not create a second inventory backend. Provide clear UI for inaccessible/full containers. Preserve volume when mixed-scale material is stacked, mined and placed.

**Proof:** actual build/repair/craft flow; concurrent last-item test; duplicate request; container move/unload; save/reload. Relevant second-client smoke test early. **Measure:** interaction latency and index-update cost; do not scan every world container each frame.

**Exclude:** full factory automation, a replacement UI framework, universal remote logistics. **Later increments:** incremental nearby index, search/favorites, blueprint preview, bulk stacks and migration, tool selection polish.

## T07 — A connected geological watershed

**Prerequisites:** T01, T03 behaviors, or equivalent proven existing contracts. **Read:** D05–D06. **First proof:** one cross-region catchment and one actual volumetric opening remain coherent across load order and editing.

Use current macro atlas where available. Identify stable outlets, strata/fault context and bounded regional generation. Choose one formation family such as fracture-controlled karst, not all geomorphic models. Produce its 3D solid/air and material query from the shared source. Keep the surface river and underground connection compatible; avoid later noise changing outlets.

Use two development counterfactuals with the same coarse relief but changed lithology/permeability. Show a meaningful difference in water/material/habitat opportunities. Do not regenerate a saved production world during the comparison.

**Proof:** region boundary continuity, shuffled generation schedules, negative/face-seam coordinates as supported, mine/collision/shadow correctness and save persistence. **Measure:** cold/warm generation and query cost, cached atlas memory. **Exclude:** whole-planet geological simulation, new planet catalog, a new weather engine.

**Later increments:** additional landform families, bounded erosion/deposition, aquifers, spherical seams, seasonal processes. A textbook-looking noise field is not enough.

## T08 — Plant persistence across coarse and fine simulation

**Prerequisites:** T04 and T07 behavior. **Read:** D05, D07. **First proof:** one harvested patch changes predictably under moisture/time and never duplicates biomass on streaming.

Use stable specimen/recruitment IDs, one budget owner and explicit materialization state. Preserve modified specimens while coarse cohorts advance. Model the minimal sources relevant to the chosen plant; environmental inputs are explicit. Do not reseed visible trees when an age cohort changes.

**Proof:** cut, leave, advance time, return, save/reload, join client; stable plants/positions and accounted growth. Change available water in a control fixture and observe appropriate differences. **Measure:** idle/active patch cost and persisted delta size. **Exclude:** whole food web, individual AI for every plant, unbounded mutations.

## T09 — One useful unattended production line

**Prerequisites:** T06 behavior. **Read:** D04, D09, D15. **First proof:** existing source/power → process → storage produces useful goods with a legible bottleneck and correct offscreen continuation.

Choose one already supported process. Avoid using a new fluid solver to deliver the first crusher. Consume real input/work/power; reserve output or preserve a defined partial batch. Handle blocked storage, power cut, source depletion, removed transport and simultaneous collection. Expose requested versus delivered energy and actual rate.

**Proof:** compare watched and unwatched runs for equal simulated time; checkpoint mid-batch; broken input line cannot keep producing; no duplicate lots. **Measure:** idle, steady active and burst-edit cost. **Exclude:** entire industrial catalog or new global currency.

**Later increments:** multi-route refining, thermal utilities, network compilation, larger multiblocks, controller integration. Preserve source values until measured tuning justifies changes.

## T10 — One composable spell changes real matter

**Prerequisites:** T01 and T06 behaviors. **Read:** D10, D15. **First proof:** an existing or minimal wand supports two reusable operators whose combination changes a real interaction, not only visual damage.

Choose a simple supported effect pair such as a collision-triggered heat/impulse operation or a controlled excavation operation plus yield routing. Preserve core wand order/resource semantics. Validate bounded expansion and read sets; no universal new VM needed. Improve the actual first-person effect origin/readability if within ownership.

**Proof:** independent effects and combined effect, resource depletion, retry/replay, material change, save/reload and multiscale yield. Cosmetic particles cannot grant resources. **Measure:** spell evaluation and induced edit costs. **Exclude:** all wand families, a full spell-programming language, advanced folding/portal physics.

**Later increments:** data-driven families, continuous/failure states, organism growth, deeper modifiers, automation via same effects, Earthfold after constrained structures are proven.

## T11 — An enjoyable glider or elastic movement tool

**Prerequisites:** T01 behavior. **Read:** D02, D12, D15. **First proof:** choose one tool, not all mobility, and make ordinary traversal feel good in the actual landscape.

Use a small physical model with clear relative-airflow or stored-energy accounting, collision, readable deployment/charge, landing and recovery. Give effective input assistance without hidden limitless forces. Determine safe prefetch by supported speed and collision-ready latency; do not let a glider outrun ground availability.

**Proof:** launch, maneuver, recover, fail safely, repeat at different terrain/wind conditions, save relevant state and compare render-rate variations. Network prediction smoke test when relevant. **Measure:** collision/prefetch and movement latency. **Exclude:** full aircraft design, CFD, arbitrary no-fly zones.

## T12 — One useful inhabitant and one trustworthy lead

**Prerequisites:** T00 and ordinary T06 ownership/storage behavior; T07 only when the selected resident or lead depends on a newly generated hydrological system. **Read:** D03, D08, D11. **First proof:** a resident performs an existing productive/service role and reports one fact they can actually know.

Use actual supplies/order state and a simple routine. A source-linked observation/report enters the player's notebook only when learned. The player can ignore it or inspect/act through more than one feasible method; no forced quest. Prefer a practical existing mechanism over inventing a named landmark.

**Proof:** service or trade, offscreen/near transition, stock accounting, report provenance, optional help and accessibility; a change in route or supply produces the declared effect. **Measure:** active vs remote behavior. **Exclude:** fully simulated economy, generated biographies, continuous LLM dialogue, NPCs requiring the player to rescue them.

## T13 — Responsive cooperative authority and exact-domain pilot

**Prerequisites:** T01 and T06 behaviors; existing netcode may already meet part of this. **Read:** D05, D16–D17. **First proof:** two players mine/place, contend for a crate, physically interact with one object, and reconnect without duplication or global stalls.

Audit actual actor-slot assignment, edge/held input, transport lanes, receipts and baseline installation. Retain a working locally simulated state-sync fallback. Isolate one closed deterministic pilot; include future-affecting solver state and external inputs. Do not claim exact replication from matching transforms alone.

**Proof:** two machines where available, worker/render-rate variations, lost/reordered/duplicate messages, stale baseline, last-item contention, paused client, resuming checkpoint. If only localhost is available, label cross-machine proof pending. **Measure:** full bytes, state age, reconciliation/replay and correction frequency.

**Later increments:** proven command-only steady operation, domain merge/split, four roles, relay tests, changed-cover combat. No anti-cheat or free-relay promises.

## T14 — Coupled material physics with an honest envelope

**Prerequisites:** T01 behavior; T09 only when the selected fixture needs a new working supply/processing line. **Read:** D05, D15, D17. **First proof:** choose one incremental missing behavior in a tank/breach/crate or support/detachment fixture; do not implement new fluids and new rigid bodies simultaneously.

Declare the supported solver/model, quantities, boundaries, error tolerance and workload. Use a tiny reference/oracle where feasible. Retain a correct existing backend. Demonstrate one interaction transferring force/material/heat once. Keep cosmetic effects nonauthoritative.

**Proof:** same fixture across resolution/activity boundary, interruption/save and authoritative replays where supported. Dense contacts and fragmentation count toward cost. **Measure:** end-to-end coupled cost including collision/update/meshing, not just a compute dispatch.

**Later increments:** pressure/room coupling, structural strength, phase changes, finite joints, moving assemblies. Stop before extending outside the measured model; expose unsupported operation before commitment.

## T15 — A player-designed controller earns its existence

**Prerequisites:** T09 behavior. **Read:** D09. **First proof:** one editable macro-level controller design with actual IO operates an existing pump/furnace/sorter differently from a second design.

Use typed nets and validated macro components, a known-good preset, inspection and saveable versions. Compare useful throughput/latency/IO/energy under fixed resources. Manufacturing initially can use the existing compatible assembly path; do not demand a full fab before proving design value.

**Proof:** invalid wiring diagnostics, deterministic bounded execution, save/manufacture/install identity, actual machine response. **Exclude:** transistor emulation and nanosecond scheduling.

**Later increments:** routing/layers/manufacturing yield/wafer processes, not arbitrary grid-occupancy bonuses.

## T16 — A real static portal before a realm network

**Prerequisites:** T13 and T01 behaviors. **Read:** D05, D12, D14, D16. **First proof:** a local static pair with oblique views, correct parallax/clipping and one actor plus an item crossing under authority.

Declare support for partial straddling, collision, direction and closure. An unready exit remains visibly blocked. Per-view frame/depth/atmosphere/history is explicit. Start recursion at one; rendering limit does not redefine legal topology. Retain simple fluid containment policy rather than fake through-flow.

**Proof:** cross/return, camera near-plane, two opposite travelers, duplicate transfer, close while occupied, save/reconnect, permission denial. **Measure:** extra views/memory/streaming/physics. **Later increments:** orientation/gravity, moving endpoints, remote world, vehicles, selected coupled effects.

## T17 — One editable vehicle and an orbital path

**Prerequisites:** T01 and the shared-ownership core of T13; the selected slice also requires its actual rigid-body, pressure, flight, or thermal interfaces. T14/T11 may supply those interfaces but their unrelated expansions are not mandatory. **Read:** D05, D12, D15, D16. **First proof:** choose the earliest missing supported slice: static assembly to moving seated craft, real cabin leak, atmospheric maneuver, or orbit/return. Do not implement all in one patch.

Source hull, modules/cargo/compartments and inventory retain identity. Use one body manifest and model for gravity/air/orbit. A beginner blueprint is convenient but editable. Propulsion/assist has finite actuators/resources; no hidden orbit injection. Log which configuration is a creative fixture vs survival proof.

**Proof:** the selected slice plus relevant interruption/save/co-op case; later full departure/insertion/return. **Measure:** actual coupled physics, visibility and streaming costs. **Later increments:** walkable interiors, docking, station construction, frame changes. Existing example shuttle dimensions are trial art layouts, not certified flight designs.

## T18 — Meridian utility, not another monument

**Prerequisites:** T00 for the visual proof; working custody/storage and endpoints for local cargo; the orbital-return proof from T17 plus applicable T09/T12 supply/schedule behavior for full orbital service. These are separate proof gates. **Read:** D08, D12–D14. **First proof:** one actual consignment travels between two working docks or one current tether renders continuously against land/cloud/sky. Select one of these distinct proofs.

For cargo, preserve custody through coarse and local representations and radio schedule. For rendering, preserve trajectory, actual projected coverage, background composition, precision, atmospheric domain and source ownership. Neither proves the other.

**Proof:** cargo retry/reload/no duplication or native motion/occlusion/LOD/timing. **Exclude:** new kilometer-scale decorative structures. **Later increments:** scheduled orbital exchange, usable terminal, inhabited precinct, skyhook envelope/momentum accounting.

## T19 — A second world changes existing play

**Prerequisites:** T07 regional generation, T17 orbital-return behavior and T18 full orbital service or an equivalently verified return/cargo route. Passing only T18 visual rendering does not satisfy this gate. **Read:** D06–D12. **First proof:** transfer existing equipment/cargo to one destination with a materially different environmental signature and demonstrate a meaningful changed behavior plus return.

Use gravity/air/thermal/geology/ecology differences that interact. No one-exclusive-ore design and no per-planet disabled recipe list masquerading as physics. Region variation and travel safety exist before broad planet multiplication.

**Proof:** planetary mapping/edits, shared material state, compatible atmosphere and orbit, useful arrival activity, solo/co-op and return-home value. **Exclude:** galaxy simulation and ten unfinished planet archetypes.

## T20 — One encounter or dungeon with genuine alternatives

**Prerequisites:** T00 and T01 current collision/edit behavior plus an existing usable combat/movement loop. T10/T11 are conditional only when the chosen encounter requires those new mechanics. **Read:** D10–D11. **First proof:** one compact existing encounter with readable movement/telegraphs and one meaningful environmental alternative.

Improve real AI/action/hit geometry and room topology. Preserve a legitimate bypass; protect only through declared readable properties. Show why the player would revisit or use the place beyond loot. A normal play method and an unconventional method need not share a scripted completion trigger.

**Proof:** solo, cooperative pressure where supported, destruction, retreat/recovery, persistence and low-effects readability. **Exclude:** ten new bosses, required lore terminals or unexplained invulnerability.

## T21 — Realm and Hall content after spatial proof

**Prerequisites:** T16 and T20 behaviors. **Read:** D10–D12. **First proof:** one realm transition demonstrates its actual rule and lawful return; or one complete Hall precinct/warden uses existing machinery. Choose one.

Retain Crimson pocket isolation, Null Choir's revised ordinary-electricity rule, discovered-only routing, and the eventual any-two-of-four Hall progression. Do not register every named realm as a placeholder. Optional personal horror retains bounded knowledge/respite/refuges and uninvolved-player protection.

**Proof:** realm rule actually changes the supported operation; no cargo duplication or transit loophole; alternative exit and reconnect; meaningful encounter behavior. **Later increments:** four wardens/King/extraction/nexus and other realm identities as separately tested content.

## Dependency and selection policy

The machine-readable graph is in `TASK_GRAPH.json`. It deliberately allows T02/T03/T04/T05/T06 to improve current visuals and play after T00 without waiting for a whole geosphere. T13 begins before broad planet work. Performance review is part of every affected card, not a last-stage rewrite.

Correctness blockers override artistic order, but an unrelated failing test owned by another active agent is not permission to rewrite their feature. Record it, establish whether it invalidates this proof, and continue safe scoped work or mark the necessary gate blocked. Aesthetic changes cannot be called accepted without actual native observation. Automated tests should be repeated after relevant changes/failures, not mechanically rerun forever on identical bytes.

## Proof-level dependencies

`TASK_GRAPH.json` separates a packet from the particular proof being accepted. For example, `T18.visual` does not establish cargo transport and cannot unlock `T19.second_world`; `T17.moving_assembly` does not establish an orbital return. A prerequisite can be satisfied by a compatible existing implementation with recorded evidence. Never make an unrelated new glider, hydraulic solver, or full workshop scanner a prerequisite for a local art fix, a basic encounter, or a single-container networking test.
