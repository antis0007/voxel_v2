# Tinker's Forge, creative catalogue and UI scale

This pass closes the Tinkers' Construct gap recorded in
`docs/PARITY_AUDIT_NOITA_TINKERS_MEKANISM.md` and expands the creative menu and
interface scaling. Existing worlds load without a reset and keep their fitted
bytes.

## Fitted parts

`Adventure` already saved three part bytes and a modifier word at the end of its
block. They were previously only reachable through a smelter fallback that
incremented arbitrary tiers. `src/world/adventure/tinkering.rs` now gives those
bytes a real material table:

| Byte | Material | Head | Binding | Grip | Trait |
| --- | --- | --- | --- | --- | --- |
| 0 | Copper | neutral | neutral | neutral | Plain; cheap to fit |
| 1 | Bronze | +2% | +6% | +5% | Balanced guard |
| 2 | Steel | +8% | +2% | +1% | Heaviest head |
| 3 | Cinder | +5% | neutral | +3% | Hits ignite; wet foes steam |
| 4 | Rime | +3% | +4% | +4% | Hits chill, setting up shatter |
| 5 | Polymer | -4% | +8% | +8% | PlusTiC-style light synthetic |

The material factor is layered **on top** of the old tier progression
(`1 + 0.15 × head`, `1 + 0.12 × binding`, and the old grip recovery curve).
That preserves the numbers of pre-forge saves with reinforced parts; it does not
replace them with smaller values. Invalid bytes are rejected by
`Adventure::decode`, and the byte table is covered by a save round-trip test.

Fitting costs use existing resources from the authoritative pack:

| Part | Copper | Bronze | Steel | Cinder | Rime | Polymer |
| --- | --- | --- | --- | --- | --- | --- |
| Head | 48 copper | 1 bronze + 32 copper | 1.5 steel + 32 coal | 1 steel + 1 core + 64 coal | 1 steel + 3 crystal + 1 essence | 1 biomass + 1 coal |
| Binding | 32 copper | 48 bronze | 48 steel + 1 circuit | 1 core + 1 coil | 2 crystal + 1 essence | 32 biomass + 1 coil |
| Grip | 32 fibre | 32 bronze + 16 fibre | 24 steel + 1 coil | 24 biomass + 32 coal | 2 crystal + 32 fibre | 48 biomass + 16 fibre |

Amounts are in resource units (64 units = one displayed ingot/crystal), matching
`Expedition::pay`.

## Modifiers

The five existing `tool_mods` bits keep their exact effects and now have names,
descriptions and costs shown in the Forge:

| Bit | Name | Effect | Cost |
| --- | --- | --- | --- |
| 1 | Jagged | +8% damage | 8 steel + 16 coal |
| 2 | Flux | +2 mana per hit | 2 crystal + 1 circuit |
| 4 | Core | Finishers stagger +0.25 s | 8 iron + 1 coil |
| 8 | Moss | +2 health per hit | 8 biomass + 8 fibre |
| 16 | Runed | +4 damage per hit | 2 cores + 2 circuits |

Removing an installed modifier is free. Removing it and reinstalling later
charges the cost again. Modifiers survive `Strip fittings`.

## Where to use it

Press **E**, then the **Forge** tab (next to Machines and Creative). The page
shows the equipped weapon's generated name, damage/sweep/swing bars, three part
cards with material swatches, trait chips, cost-aware buttons, and the modifier
grid. Every button is disabled outside a workbench's reach, with a tooltip that
says why. A smelter with nothing to smelt still offers the old convenience path:
it fits the next affordable, strictly better part or modifier.

## Creative catalogue

The Creative tab now has two modes:

- **Blocks** keeps the original searchable, drag-and-drop block palette and adds
  name/ID sorting plus **Fill hotbar** for the current category or search.
- **Items** lists all 19 resources, five weapons, nine armour pieces, five tools
  and three provisions. Choose ×1, ×8 or ×64, or shift-click for a full stack.
  Grants go through `UiAction::GiveItem`, clamp to the item's real stack limit,
  land in the pack first and sync the legacy resource/weapon fields.

Item cards use the existing pixel icons, a category tag and a bounded-name
label; hovering gives the stat/description line. No item is granted directly by
the UI, so a full pack reports overflow instead of losing the stack.

The catalogue list is built once (`OnceLock`), search is allocation-free for
ASCII probes, and trait chips are a fixed-size array, so an open menu does not
allocate per entry per frame.

The first-person held blade is repainted from its head material
(`paint_tinted`), so a Cinder fitting reads orange, Rime reads pale blue, and
Polymer reads bone-white without a second sprite atlas.

## UI scale

`Preferences` advanced to `VXPRF006`, adding a 25th word for `ui_scale`. Older
`VXPRF001`–`VXPRF005` files still decode and default to 100%. The scale slider
(75%–175%, 5% steps) appears in the home Preferences page and the pause Display
page, with a Reset button. `EguiUi::begin_frame` multiplies it by the existing
native-DPI rounding correction, so the world renderer is untouched.

## Verification and remaining gaps

The new unit tests cover material byte validation, tier-preserving multipliers,
unique modifier bits, every fitting cost, Polymer save round-trip, corrupt-byte
rejection, creative catalogue uniqueness and UTF-8-safe card labels. The
project-wide suite was run offline: **162 passed, 0 failed, 13 ignored**. The
terrain-generation timing test is load-sensitive and can fail under background
CPU load (it passed at 1.11× when the machine was idle).

Still missing versus Tinkers' Construct: part-specific tool types beyond the
five melee weapons, per-part durability and repair, material melting/scrap
refunds, tool leveling, and a visible 3D preview of the assembled weapon.
Still missing versus Immersive Engineering: conveyor belts, water wheels, wind
rotors and multiblock machines. Those need new `factory::Kind` values and a
versioned factory migration, so they remain a deliberate next step.
