# Working gear of the Anchor

Implemented 2026-11-05. Original Rust implementation informed by
[Create: Octo Arms](https://github.com/SenDeliver/create-octo-arms/tree/main),
Noita's Lukki mutation, Mekanism jetpacks and slime boots/slings.
No upstream code or assets are included.

The equipment comes from the Anchor's existing trades. Its use explains why
shaft crews could work without scaffolds, how the Founders inspected braid
couplings, and how the Keepers crossed damaged garden irrigation spans.
The items have recipes, creative catalogue entries, distinct icons, and short
inspection copy; longer history appears in the item codex.
Rare papers in previously empty foundry, garden and Ballast coffers connect the
gear to its former users. Existing nonempty records keep their original text.

| Equipment | Place in the world | Controls | Cost and constraint |
| --- | --- | --- | --- |
| Hang glider | Wooden spars and lumen-fibre sail. | Jump while falling deploys; Jump again folds; aim down to dive or up to flare. | Chest slot; 3 fibre, 3 planks, 1 copper ingot at a workbench. No thrust; bounded weather wind affects ground speed. Combines with slime boots and sling. |
| Ballast octo arms | Eight salvaged working limbs on a shaft harness. Maker's tally bears eight signatures. | Alt + Jump toggles grip; movement keys traverse; Jump rises; Sneak descends. | Chest slot; 3 m reach from the body; at least two contacts. No contact means a fall. Hands remain free. |
| Burner jetpack | Founders' inspection pack, fuel allowance stamped beside the valve. | Hold Jump for lift; Alt + Sneak toggles hover; Sneak descends while powered. | Chest slot; one carried bio-ethanol gives 12 seconds of burn. 7 m/s ascent. No passive refill. |
| Induction jetpack | Founders' later replacement for burners that scorched braid strands. | Same controls. | Upgrades a burner pack; 120,000 FE reservoir; 4,000 FE/s from carried charged portable batteries; 9 m/s ascent. Empty battery blocks cannot supply energy. |
| Slime boots | Keepers' elastic garden cultures grown into lumen-fibre soles, later repaired by the Ballast. | Automatic bounce; hold Sneak to settle. | Feet slot; prevents fall damage even while sneaking. Returns 82% of landing speed, capped at 20 m/s; small drops settle. Other damage still applies. |
| Slime sling | Garden gel stretched between lumen-fibre straps; children's copies entered as freight equipment in the tally. | Hold the use button on ground, aim, release. | Handheld; one second to full charge; 22 m/s added impulse; no charging in midair. No fall protection without boots. |

The arms and packs replace chest armour. Slime boots replace kinetic boots;
they can combine with a harness or jetpack. Kinetic greaves remain separate.
Garden equipment uses harvested fungal biomass and lumen fibre, with wooden
straps for the sling. Both fit the inventory's 2×2 crafting grid: boots cost
two biomass and two fibre; a sling costs two planks, one biomass and one fibre.
No wild essence, industrial processing or guardian victory is needed.
The recipe catalogue groups them with the later equipment under Traversal.
The arms use bronze, steel, coils and circuits. Propulsion feeds the existing
bio-ethanol and battery industries. Arms and jetpacks remain in the crystal
technology table; the existing recipe book exposes their actual ingredients.
No new guardian reward or terrain generation gate is required.

Common unopened caches now supply plants, basic ore, limited copper/iron ingots
and survey crystal. Steel, circuits, coils and arcane cores remain available
through manufacturing and guarded rewards. Existing initialized cache contents
are preserved. Ruin metal harvesting requires tier 2 for copper/tin/iron/bronze/
silver/gold, tier 3 for platinum and tier 4 for mithril; under-tier mining follows
the existing no-drop rule. Terrain placement and existing inventories are unchanged.

Burner fuel and induction charge belong to the wearer and persist in save
format 49. Older saves start empty. Swapping or crafting equipment does not
refill these reservoirs. Saved equipment uses appended armour identities;
the sling uses tool identity 19. Exact-session and multiplayer player snapshots
also carry reservoirs. Contact points, hover, and sling charge are transient.

The eight arms render as articulated brass/iron limbs with green gripping pads;
jetpacks render tanks, nozzles and firing exhaust. Grips use actual ray hits,
and movement passes through the existing collision sweeps. Manyfold grips use
its gravity-relative capsule sweep. Distant foreign bodies currently have bare
sphere collision and no voxel grip surfaces.

The HUD shows a small meter only during grip, charging, thrust, hover or gliding. Fuel
and charge are also visible in the inventory. Existing movement/use bindings
drive gameplay; default control names above follow the standard bindings.

Acceptance: compilation is separate from native movement, visual quality and
multiplayer acceptance. Native captures, handling review, fuel exhaustion,
save/reload play and latency review remain to be performed. Remote avatars
currently use the existing shared character model and do not show this gear.

The hang glider appends armour identity 9 without changing old identities. Its
frame supports flat and gravity-relative movement; radial sling steering retains
air momentum up to the existing movement cap. Braking a glide updates impact
speed before fall damage. Jetpack reserves are consumed on the host for remote
players and replicated as a bounded section in gameplay image version 6. Everyone
in a session needs the same build. Weather wind is limited to 8 m/s for gameplay.
Cloud adaptation now requires at least 3 ms of raymarch cost before dropping
cloud detail under load, and ignores paused or inactive cloud passes.
