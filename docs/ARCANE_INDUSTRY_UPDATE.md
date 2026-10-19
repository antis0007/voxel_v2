# Arcane industry update

This increment extends existing saves and generated sanctums. No world reset is required. Already-cleared encounters stay cleared. New enemy mixes appear at uncleared sites beyond the starter workshop.

## Combat and progression

- Hold **R** to cast; **T** cycles unlocked spells. **LMB** strikes nearby enemies. Three connected strikes within the combo window produce a heavier finisher.
- Hits add a brief gold hit marker, model flash, stagger, interrupted attack telegraphs and swept knockback. A visible striking tool and wand muzzle particles reinforce attacks.
- Frost followed by melee, Spark or Resonant Burst shatters the target for bonus damage. Frost slows for four seconds; shattering consumes the slow. Burst also knocks back surrounding enemies with line of sight.
- Burrow beetles telegraph a committed charge. Spore moths fly and fire a three-shot fan. Shield wardens resist frontal Spark and melee; freeze, shatter or flank them. These join sporelings, sentinels and wisps.
- Clear **1 / 3 / 7 sanctums** to become Delver / Runekeeper / Arcanist. Each rank adds 20 maximum health, 15 maximum mana, 3 melee damage, 4 direct spell damage and 1.5 mana regeneration per second. Rank-up replenishes both pools. Existing clear records grant the corresponding rank.
- Wand crafting still unlocks Spark + Mend, then Frost Lance, then Resonant Burst. Guardian essence and circuits feed advanced crafting.

## Extraction and wet processing

All assemblies occupy one metre. Craft at a workbench with F then E, equip a kit, and right-click to place it. F focuses a machine; E opens its illustrated controls. Item pipes, power cables and fluid pipes form independent face-connected networks. Machines are endpoints, not network relays.

| Assembly | Ingredients | Function |
| --- | --- | --- |
| Mining drill | 4 steel, 2 coils, 1 arcane core | Excavates a 3 × 3 m shaft down 24 m; exports actual ore and crystal through item pipes. |
| Water pump | 2 bronze, 1 coil | Draws finite water from the footprint directly underneath. Requires power. |
| Fluid pipe ×8 | 1 copper, 1 tin | Transfers water from pumps to washers or tanks, and from tanks to washers. |
| Water tank | 2 iron, 1 tin | Stores 16 m³ of water; no power required. |
| Ore washer | 2 steel, 2 coils, 2 bronze | 1 ore + 250 L water → 3 dust in 6 seconds, consuming 288 power. |

The advanced line is **drill → item pipes → washer → item pipes → electric furnace → output crate**, with a separate **pump → fluid pipes → tank/washer** supply and power cables connected to powered machines. Crates now accept raw materials as well as products. Washers triple smelted ore yield; crushers remain the cheaper, water-free doubling route. Bronze and steel lead to coils, guardian essence leads to arcane cores, and these unlock automated mining.

Drills excavate up to 2 m³/s at their base drive and spend 16 power/m³. Output quantities and energy credits remain consistent at 1 m, half-metre and quarter-metre voxels. Empty cells do not create resources. Drills stop for the player's body/footing, registered assemblies, fluids, unsupported materials or full outputs. Pumps transfer up to 1 m³/s at base drive, spending 4 power per successful intake pulse. Drive upgrades accelerate extraction and increase power demand.

Extraction runs within 64 m of the player, with bounded edits and a shared frame budget; large installations may run below their nominal rate. Existing buffered production and transport keep running while playing. Fluids, progress, extraction credits and shaft position save with the factory. Full buffers, dry intakes and failed edits retain resources.

**Drain water** pauses a machine and releases stored water into free neighbouring voxels. Clear an obstructed outlet or let simulation carry water away. Assemblies containing water cannot be packed until drained. Drain mode itself stops after reload; the remaining water is retained.

## Interface and materials

- Square frames throughout the player interface; no rounded panel corners.
- Workshop recipe grid, equipment page and satchel with original pixel illustrations, quantity overlays and ingredient tooltips. Missing ingredients receive a red underline. Building materials are tucked behind their own toggle.
- Machine panels display item slots, recipe input/output diagrams, power and water gauges, filters, drive upgrades and drain controls.
- Hearts, segmented mana, spell icons, cooldown masks and hit markers replace the old textual combat bars.
- New drill augers, pump/washer paddles, framed tanks, water-level displays and blue fluid tubes. Existing machinery gains rivets and warning stripes.
- Ore facets, mushroom spots/gills and brushed machinery surfaces; material motifs use physical metre coordinates and fade subpixel detail at distance.
- The loading/menu landscape uses a solid triangle mesh instead of individually antialiased strips, removing the source of rectangle seams. Inventory cube faces use solid mesh patches for the same reason.

## Persistence and verification

New saves use VXWSAVE8. Readers retain versions 1–7, including the original 8-kind/14-resource factory layout and version-7 11-kind/19-resource layout. New materials and machine kinds are appended, preserving existing IDs.

Final `cargo build --offline --target-dir target` passed, including Rust integration and the project's build-time WGSL validation. Log: `target/arcane-industry-build.log`; executable: `target/debug/minecraft_style_voxel_prototype.exe`. No automated tests or game launch were performed for this increment. Visual quality, combat tuning, live fluid throughput and legacy-save migration still require in-game acceptance.
