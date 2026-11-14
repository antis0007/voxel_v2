# Axiom wand ecosystem and visual production specification

Status: revised design requirements, 26 October 2026. Implementation findings below are source inspection, not gameplay acceptance. This replaces the three industrial wand concepts as the production direction; it does not replace the existing spell system or change saved worlds.

A desirable wand is a recognizable object, a useful spell instrument, and a discovery worth experimenting with. The production direction is an equipment ecosystem with twelve initial silhouette families and five craft lineages. The earlier three wand studies remain minor craftable-item candidates, not the central visual grammar. Construction, behavior and origins must differ meaningfully, with magic that changes the same physical world used by tools and machines.

## Reference comparison

| Reference | Supported lesson | Axiom requirement |
| --- | --- | --- |
| [Noita, developer description](https://noitagame.com/) | Player-built magic interacts with materials through burning, melting, freezing, evaporation and other simulated processes. Exploration and learned knowledge matter. | Evaluate spell combinations by changes to encounters and materials, not color variants or nominal combination counts. |
| [Thaumic Wands, author description](https://www.curseforge.com/minecraft/mc-mods/thaumic-wands) | Rods and caps affect crafting costs; wands recharge through several routes and accept foci. | Construction must have a consistent relationship to function, supported by workshop and exploration loops. Keep world charging devices distinct from slender handheld wands. |
| [Ars Nouveau guide](https://ars.guide/book/getting_started/spell_casting/) | A spellbook stores authored spells; glyph learning, equipment and book upgrades expand spellcraft for combat, world alteration and automation. | Provide understandable composition and reusable presets that also support useful noncombat actions. |
| [Botania Lexicon, Mana Lenses](https://botaniamod.net/lexicon.html#mana_lenses) | Lenses alter bursts with tradeoffs; compatible pairs combine effects. Mana devices make those effects useful in machines. | Shared spell interactions should support both held instruments and placed emitters. Compatibility must be explicit rather than every-part-with-every-part. |
| [Electroblob's Wizardry, author README](https://github.com/Electroblob77/Wizardry) | Exploration and finding spellbooks offer a direct route into casting. | Finding a working wand should immediately enable play; expert editing cannot be a prerequisite for the first enjoyable cast. |

These are different approaches, not interchangeable versions of one system. Noita's detailed wand-stat rules and visual-stat correlations require further primary evidence; the linked community wiki could not be opened during this review. Do not claim that every long wand means capacity, or every split tip means multicast. Those can become authored Axiom conventions only where mechanics support them. Botania's online Lexicon follows its development book and can precede released versions.

## Existing implementation and actual gaps

Paths are relative to the repository root.

| Area | Present in inspected code | Gap or required verification |
| --- | --- | --- |
| Wand identity | `crates/app/src/world/adventure/wand_items.rs`: bank records preserve tier/deck; seeded names and chassis stats; found drops. | Appearance choices use separate seed salts from statistics. A cosmetic combination is not a construction model. |
| Composition | `world/adventure/wands.rs`: eight slots, ordered evaluation, modifiers, multicast, always-cast stat, shuffle, mana and timing. | Compare actual evaluation behavior, failure cases and UI explanation before claiming Noita parity. |
| Payloads | `CastAction`: one optional spell payload, separate payload modifiers, optional follow spell. Evaluator includes impact/timer/expiry-related behavior. | This is not a general recursively nested spell graph. Define desired nesting and bounded execution before extending it. |
| Discovery | `world/adventure/discovery.rs`, `SPELLS_AND_DISCOVERY.md`: found-spell tracking and unlock routes. | Check current native progression, readability and save behavior. The older document contains historical ID/version counts; do not use those as current totals. |
| Held geometry | `mesh/models/wayfarer.rs`: indexed skinned GLB data, sampled clips, bounded CPU skinning. | Existing art failed visual acceptance. GPU skinning is not implemented by this path. |
| Materials | Active Wayfarer path uses external atlas, UV0, material-name finish codes. | It does not render arbitrary embedded glTF PBR/emissive textures. Masks need explicit renderer support; Blender appearance is not proof. |
| Feedback | `mesh/models/wand.rs`: mana/deck inlays, selected glow, generic motes. | Inlays and tip use fixed local coordinates. New geometry needs authored sockets; spell state must drive effect shape and timing, not only tint. |
| Item presentation | `wand_items.rs::draw_dropped` creates a generic beam/cube/orb drop. | Inventory, dropped and held representations must identify the same physical wand. |

No source evidence here establishes combat feel, animation quality, broad material parity, or frame-time acceptance.

## The playable loop

1. Find an intact wand with a useful preset in an encounter, ruin, workshop or natural site. Its silhouette and materials indicate a tradition, not a guaranteed rarity tier.
2. Cast it immediately. The player sees a clear origin, trajectory, impact and persistent consequence. A scarce resource or dangerous interaction is legible before repeated failure.
3. Inspect the deck and compare the actual stat changes from one edit. Show ordering, cost, cast interval, recharge and delivery conditions compactly.
4. Try the change on both a target and a material. A recovered example should teach one useful relationship without revealing all discoveries.
5. Keep, trade, salvage or rebuild it. Preserve the player's deck and unique item identity across drops, storage, equipment changes and saves.
6. Bring the same useful interaction to a placed device where supported: irrigation, heating, excavation, defense or material processing. Machines consume actual resources and obey world rules.

Early variety must include a precise low-cost wand, a slow burst wand, an unreliable but rewarding found wand, a utility instrument, and an unusual delivery mechanism. These are encounter roles, not five recolors or mandatory class restrictions.

## Spell grammar and parity criteria

Keep the existing evaluator until a concrete behavior requires extension. Specify delivery, effects, modifiers and control flow separately. A preview must use the same rules as execution, including modifier scope and payment.

| Behavior | Acceptance example |
| --- | --- |
| Ordered evaluation | Reordering a modifier changes the intended shot only; preview and actual cost agree. |
| Multicast | Two visibly distinct trajectories launch as one paid cast, with readable combined recovery. |
| Trigger and timer | A carrier releases its payload once at its documented condition; a miss does not invent an impact. |
| Recharge and mana | Fast casts can exhaust a reserve; a sustained low-cost build has a different rhythm. Hand, sound and UI agree with actual readiness. |
| Shuffle | A repeated experiment demonstrates order variation without silently adding/removing spells. |
| World effects | Fire, fluid and frost alter actual material state, with results persisting as ordinary world state. |
| Utility | At least one traversal and one construction/production use changes what the player can accomplish. |
| Discovery | A found usable preset teaches an interaction; editing it creates a materially different outcome. |

Before nested payloads or recursive modifiers are added, choose explicit limits for depth, emitted actions, active projectiles and world edits. The inspected evaluator already caps one multicast action path at twelve; that does not establish a global execution budget. Budget exhaustion must have predictable behavior and must not grant a free partial cast. Deterministic seeds belong to gameplay state where replay or persistence requires them.

Parity means demonstrating selected behaviors, not copying every Noita exploit or importing every Minecraft mod progression tree. Keep an evidence table for each promised behavior: code location, native reproduction, result, remaining limitation.

## Visual language and collection breadth

The approved direction is refined Minecraft-like 3D forms with deliberate SNES/GBA material clusters. No global pixel filter, photographic albedo, noisy voxel subdivision or constant cyan glow. Quiet materials make the casting spectacle legible.

### Equipment classes and craft lineages

Wands provide flexible personal casting; staves support sustained or specialized work; foci and instruments measure or manipulate interactions; grimoires and spell circuits store reusable programs; placed machinery industrializes those operations. Share effect primitives while keeping distinct targeting, resource and ergonomic constraints. These are target roles, not five implemented subsystems.

| Lineage | Construction | Motion and energy identity |
| --- | --- | --- |
| Vernacular witchcraft | Knotted wood, bone, river stones, fiber, herbs, sparse wire. | Light, imperfect hand motion; brief local effects rather than a powered enclosure. |
| Scholarly thaumaturgy | Ceramic, ink, restrained brass, measured geometry, lenses. | Deliberate alignment and tracing; marks respond in a readable sequence. |
| Industrial magitech | Replaceable conductors, heat-resistant inserts, standard connections. | Firm mechanical preparation and discharge; heat/charge follows working parts. |
| Biological and fungal | Roots, chitin, mycelial growth, mineral organs. | Growth, flexure and contraction tied to actual active effects. |
| Ancient and anomalous | Broken mineral, blackglass, skeletal structures, unusual joins. | Sparse displaced or optical behavior; mystery without constant particle clutter. |

### Twelve initial silhouette families

These are original Axiom design targets from the expanded user brief, not claims about other games' exact rules. Mechanical personality guides presets and construction choices without making family a hard spell lock.

| Family | Distinguishing silhouette | Initial mechanical personality |
| --- | --- | --- |
| Witchwood switch | Tiny crooked twig with a knot; no oversized head. | Cheap rapid casting with controlled unpredictability. |
| Bifurcated focus | Unequal branching terminal; narrow continuous body. | Splitting and multicast. |
| Bone needle | Fine pale needle with a dark point. | Precision, piercing, biological effects. |
| Runic key | Short asymmetric key with distinctive teeth. | Wards, locks, structural/spatial interactions. |
| Rootbound scepter | Roots enclosing a long mineral organ. | Growth, healing, fungal control. |
| Cinder rod | Charred broken wood with a few ember seams. | Heat and combustion. |
| Alchemical quill | Tapered stylus with a clear writing edge. | Sigils, transformation, precise placement. |
| Forked conductor | Exposed functional prongs and a restrained insulator. | Electrical, magnetic and kinetic behavior. |
| Shard cluster | Unequal mineral slivers bound along a spine. | Unusual ordering and elemental combinations. |
| Grave reliquary | Off-center iron/bone body and sparse hanging charms. | Summons, curses, lingering fields. |
| Astral filament | Thin frame with deliberate open space around a small focus. | Gravity, teleportation, spatial effects. |
| Ancient prism | Compact stone/blackglass geometry with minimal grip. | Intrinsic effects and discoverable behavior. |

Judge this collection by a black silhouette sheet at inventory and held scale. Reject a batch if most entries still use the same shaft, collar and jewel assembly. Some should be plain, found or improvised; rarity does not require extra ornament.

### Representation comparison

Compare three techniques in the actual WGPU renderer: deliberately authored 24–48 px sprites with slim 3D proxies for tiny common objects; minimal 3D with painted atlases for standard equipment; more involved low-poly models only where mechanism, scale or exceptional silhouette justifies them. Test oblique/edge views, shadow, held motion and dropped appearance. A sprite proxy that becomes unreadable edge-on needs an explicit solution.

Inventory icons must be authored or retouched as pixel art. A generated concept sheet or Blender beauty render does not pass as final icon art. Each approved icon needs connected clusters, readable highlights, controlled palette and identity at native size. This comparison supersedes a universal hero-mesh requirement; Blender remains required for the 3D candidates and their inspection.

### Existing three study candidates

| Wand | Construction and silhouette | Starting proportions | Character |
| --- | --- | --- | --- |
| Thornroot | Crooked carved wood, fiber binding, a tiny inset or naturally caught mineral; sparse growth scars. | 55–60 cm, 11–13 cm grip, 2–3 cm shaft. | Resourceful, organic, imperfect, approachable. Avoid a symmetric manufactured fork. |
| Copper Arcanist | Slender darkwood core, short ceramic insert, restrained conductive winding, unequal compact terminal. | 55–60 cm, 12–14 cm grip, 2–3 cm shaft, roughly 5 cm terminal. | Precise and repaired by a skilled maker; still a wand. |
| Astral Splinter | Long pale mineral spine, dark retention bands, one broken or displaced tip fragment. | 60–70 cm, 11–13 cm grip, narrow uneven cross-section. | Ancient unfamiliar construction; avoid another fork enclosing another gem. |

Ordinary 45–75 cm wands remain the starting range, not a rule for every rare artifact. Each silhouette must work without emission and remain identifiable without its color palette.

Develop the broader silhouette collection before treating these three as a reusable template. Every addition needs a different contour, grip-to-shaft relationship or casting behavior. Do not generate hundreds of permutations as a substitute for distinct authored examples.

Eventual construction fields: tradition, shaft/core, grip/binding, emitter, focus, wear and authored exception. Components use compatible sockets and ranges. Derive both stats and appearance from the same construction record; do not reverse-engineer stats from the rendered mesh. Preserve old item IDs and seed results through a versioned mapping. Family is not affinity: a wooden wand can cast lightning, and the effect still follows its conductive or arcane path.

Appearance may suggest capacity, discharge behavior or manufacture only when the rule is consistent and taught. Exact values stay inspectable. Repair history and material origin can distinguish loot without making every found item stronger.

## Blender and texture production

Preserve the failed Spark-Lantern, Annular Induction and Offset Coil as placeable-machine candidates. They are not wand replacements.

1. Author new meshes in Blender, with named functional parts. Render black front, side and three-quarter silhouettes plus individual 64/96 px inventory views. Reject appliance silhouettes and repeated fork-with-gem designs.
2. Test the actual native projection before detailed UV painting. Review idle, inspection and casting angles; a long shaft must not cross the crosshair or hide a target.
3. Refine grip, opposing thumb, wrist and fitted folded cuff in Blender. Use an appropriately licensed artist base when useful, preserve provenance, and inspect all contact points. No mitten or four identical finger loops.
4. Unwrap continuous materials with consistent apparent texel size. Try 128×128 and 128×256 hero atlases. Use 3–5-tone material ramps, connected clusters, restrained warm/cool shifts and directional grain. Broad albedo stays free of strong baked lighting.
5. Author material identity and emission masks separately. Wood/bone/ceramic/metal/crystal are not interchangeable recolors. Emission is limited to actual channels, inscriptions and focus surfaces.
6. Save editable sources before export. Export from the saved file without regenerating geometry or overwriting GUI work. Record source/export hashes and Blender logs.

Current runtime contract: metres, local Y along the shaft, `export_yup=False`, indexed triangles, UV0 in 0..1, normalized normals, joint indices and normalized weights; compatible `GripSocket`/`Forearm` rig and sampled clips. Material names currently provide only a small finish mapping. Adapt that contract deliberately after art approval; do not silently assume arbitrary glTF material features work.

Required authored points: grip origin, cast pivot, muzzle with forward axis, energy-channel endpoints and optional support grip. Use the same transformed muzzle for light, particles and visible projectile origin. Aim toward the crosshair's world target, checking obstructions between the muzzle and target; never move gameplay collision forward merely to hide clipping.

## Animation and material feedback

| State | Hand and wand | Material and sound |
| --- | --- | --- |
| Idle | Relaxed contact, tiny breathing motion; no thumb sliding through grip. | Mostly quiet, resource state readable without perpetual particle emission. |
| Walk and sprint | Damped inertial follow-through; sprint lowers the wand and changes wrist angle. | No detached trail caused by camera motion alone. |
| Aim | Settle into a stable pose, retaining target visibility. | Small focus response only when the system is actually ready. |
| Cast | Readable anticipation, release and recovery driven by actual cast timing. | Source-channel-tip progression and a distinct release sound. |
| Continuous cast | Stable sustained grip with modest strain; blend in and out. | Continuous effects use real duration and resource flow. |
| Rapid fire | Accumulate bounded motion; recover smoothly without restarting a large animation every shot. | Prioritize the nearest/strongest cues; do not drown impacts in muzzle bloom. |
| Empty or blocked | Brief understandable failed-cast response. | No false projectile, hit flash or resource fill. |

Fire: channels warm, restrained embers, actual ignition where supported. Ice: localized frost and cool facets that follow cold state. Lightning: short discharges at actual conductive/arcane paths, with readable contact. Arcane: focus and inscription response tracks charge and release. Mixed spells have a primary delivery shape plus secondary payload cues; do not average every combination into the same glow. Audio rhythm communicates casting cadence and recharge separately from impact.

Shader polish follows geometry and native material validation. No proprietary Mycopunk shader access is assumed. Interaction highlights, shield impacts and distortion must be original implementations with explicit performance limits.

## First complete vertical slice

Copper Arcanist remains a diagnostic minimal-3D candidate, not the established quality baseline. Choose the first integrated item after the twelve-family and representation comparisons. Deliver its appropriate hand/proxy presentation, authored pixel art, idle/walk/sprint/aim/cast feedback and one useful preset. Then demonstrate: a direct cast, one modified or multicast build, one delayed payload, and one real material interaction. Use existing verified effects where possible instead of adding another shallow spell list.

Review all three reference wands in neutral daylight and underground magical light. Capture front/side/three-quarter Blender views and native WGPU gameplay at the same FOV, resolution and camera positions. Include an idle still and a motion sequence; a beauty render cannot establish animation acceptance.

Record clipping, UV seams, grip contact, silhouette occlusion, effect origin, shader support and frame time. Compare rendering with readback disabled for performance. State scene/hardware/build and percentile frame times rather than an isolated FPS number. Set the performance budget from a measured baseline before claiming a pass.

Completion requires saved `.blend` files, reproducible export scripts, editable texture sources and masks, compatible GLBs, native captures and a short critical comparison listing remaining defects. Preserve older sources and world/save compatibility. Do not add unrelated renderer or world-generator changes.

## Current art study evidence

`tools/blender/wand_family_study.py` executed in Blender 5.0.1 and saved `assets/source/blender/studies/wand_families_v1.blend`. Outputs and log: `target/wand-family-study/` and `target/wand-family-study.log`.

Version 2 revised Thornroot's growth pocket and Astral's broken spine. Copper's focus was shortened to 72 percent along its length directly in Blender Edit Mode, then saved in `wand_families_v2.blend`. Do not rerun the revision generator over this artist-edited source.

`assets/source/blender/copper_arcanist_artist.blend` and `target/copper-arcanist-review/copper_arcanist.glb` are isolated textured candidates. Blender export and the existing Rust asset probe passed: 516 triangles, 19 joints, four materials, embedded atlas and nine inherited clips. Those clips are compatibility data, not newly accepted animation. The thin stem/fork still risks a staff/pencil read; native review remains necessary.

The CC0 Quaternius hand base is preserved with license/provenance. `studies/faceted_hand_grip.blend` is a 438-vertex, 560-face anatomical grip study. Finger wrap and thumb opposition were explored, but triangulated palm/wrist shading fails the clean Minecraft style. Earlier exploratory renders inadvertently used the uncropped source body; the corrected study manifest and latest images supersede them. It needs intentional plane topology and a fitted cuff, not automatic decimation as final art.

The next bounded pass created original 32px icons and minimal Blender assets for Bone Needle, Runic Key and Cinder Rod. `assets/source/wand_ecosystem/minimal_v2/README.md` records sources, native evidence and critical findings. A benchmark-only selector now renders those GLBs and a new quad-plane hand through the actual WGPU path. Daylight, underground and a sampled pose cycle have been captured. They remain provisional: the sleeve, finger silhouettes, material treatment and spell feedback still need work. Normal gameplay assets remain unchanged. The rejected machine concepts and all earlier sources remain preserved.
