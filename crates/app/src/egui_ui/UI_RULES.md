# UI Rules

Applies to everything a player can see: HUD, overlays, menus, tooltips, item/block names, machine GUIs, maps, chat messages, errors, loading screens.

## Identity

Vanilla Minecraft minimalism, extended with the best QoL mods (Jade, JEI/EMI, Xaero's). The default screen is the world, a hotbar, and almost nothing else. Information appears when the player looks at something, opens something, or holds a key, and disappears when they stop.

The UI shows state. It never narrates, coaches, encourages, or explains itself. Players learn by looking: at slots, bars, recipes, numbers, and the map.

If a screen only makes sense because a sentence explains it, the screen is wrong. Fix the widget, not the copy.

---

## Part 1: Text

### Hard rules

1. **No instructional prose.** No "Click here to...", "Drag items into the slot!", "Use this panel to manage...". Slots, arrows, and bars carry that.
2. **No second person.** No "you", "your", "let's".
3. **No exclamation marks.** Anywhere.
4. **No mood or marketing words.** Banned: amazing, powerful, ultimate, epic, seamless, unleash, journey, adventure, immersive, explore, harness, mysterious, ancient, forgotten, echo, essence, resonance (unless one is a literal material or item name).
5. **No capitalized abstract concepts.** No "the Weave", "the Void", "the Source", "the Veil".
6. **No rule-of-three filler.** "Mine, craft, and automate." Delete.
7. **No dev text in the UI.** No TODO, "Coming soon", "This feature allows...", enum names, snake_case, `::`.
8. **Labels are nouns.** `Input`, `Output`, `Fuel`, `Fluid Buffer`. Title Case.
9. **Numbers over words, always with units.** `1,240 / 8,000 FE`, `12.5 mB/t`, `64x`, `3.2s`, `840°C`.
10. **States are one or two words.** `Idle`, `Working`, `No Fuel`, `Output Full`.
11. **Errors say what is wrong.** `Structure Incomplete: 3/12 Casings`. Never "Oops!".
12. **One name per thing, everywhere.** Machines are named for what they do: `Electric Furnace`, `Pulverizer`. Never "Processor Unit" or "Advanced Device".
13. **Keys show as keys.** `[Shift]`, `[R]`, `[RMB]`.
14. **Lore exists only on items/blocks/guidebook entries, and only if it explains something the player can do or observe.** One italic line, under 10 words. Never on machines, ores, or components.

### Vanilla text budget

- Item name: 1–3 words.
- Tooltip: name + at most 3 stat lines without [Shift].
- HUD text visible at rest: none, except chat that fades out.
- Menu buttons: 1–2 words. `Singleplayer`, `Options`, `Done`, `Back`.

---

## Part 2: UI elements

### Baseline: vanilla
- Hotbar, health/hunger-equivalent, crosshair. Nothing else on screen by default.
- Inventory and machine GUIs use the slot grid. Every machine GUI: input slots left, progress arrow center, output right, energy/fluid bars on the edge. Same layout language everywhere so no machine needs explaining.
- Chat fades after a few seconds. Messages are terse system lines, never prose.

### Look-at overlay (Jade / WAILA / HWYLA)
Small panel, top center, shown only while the crosshair is on a block, entity, or material.
```
[icon] Electric Furnace
       ▓▓▓▓▓░░░ 62%   2,400 / 8,000 FE
       Iron Ore → Iron Ingot
       ModName / Source
```
- Line 1: icon + name. Last line: source (blue italic) if the game has addons or modules.
- Middle: only live state. Progress bar, energy/fluid, current recipe, owner, growth stage.
- `[Shift]` expands to details (stored contents, side configuration, harvest tool/tier).
- Hidden entirely when nothing useful to show (plain stone shows name only, or nothing if configured).
- For this engine's physics materials: show material, phase, temperature, and volume. `Molten Iron · Liquid · 1,540°C · 340 mB`. This replaces any need for lore explaining how materials behave.
- Fully configurable: position, scale, opacity, which lines show.

### Recipe browser (JEI, refined by EMI)
Item panel on the right side of any inventory screen.
- Search bar bottom of panel. Prefixes: `@mod`, `#tag`, `$material`, `&fluid`, `^recipe-type`. No natural-language search.
- `[R]` on any item: how to make it. `[U]`: what it's used in. Works from inventory, tooltips, the overlay, and the map.
- Recipe view shows the machine icon, inputs, outputs, time, energy, and chance outputs as `%`. No descriptive text. A multiblock recipe shows a rotatable 3D structure preview, not a paragraph.
- `[+]` button transfers ingredients from inventory into the grid or machine.
- Bookmarks panel on the left (`[A]` to bookmark).
- From EMI: recipe tree / crafting plan view that resolves a full tree to raw materials and shows what is missing from inventory. Favorites with counts.
- This is the game's teaching system. Recipes and structure previews teach; text does not.

### Map (Xaero's Minimap + World Map)
- Minimap: corner, square or round, toggle with a key. Shows terrain, entities as colored dots, waypoints, cardinal letter, and coordinates on one line beneath (toggleable).
- Waypoints: single-letter or short label, color-coded, `[B]` to create at current position. Auto-created death waypoint marked `✕`.
- World map: full screen `[M]`, pan/zoom, click to add waypoints, shows explored area only.
- Cave/depth mode: since the world is variable-scale 3D, the map slices by depth layer automatically when underground, with a manual layer selector. Show depth as a number, not a description.
- No map labels beyond waypoint names and player-set markers. No auto-generated region names unless the world actually generates named structures.

### Small QoL to adopt
- AppleSkin-style previews: hovering food or fuel shows what it will restore/add as a ghost overlay on the bar.
- Mouse Tweaks / Inventory Profiles: scroll to move items, drag to distribute, middle-click to sort. No buttons labeled "Sort Inventory"; a small icon.
- Durability and charge as bars on the item icon, numbers in the tooltip.
- Guidebook item for anything recipes cannot show. Terse, no second person, structure previews and recipes before any text.

---

## Quick test for any string or widget

- Would this appear in vanilla Minecraft, Jade, JEI, or Xaero's? If no, rewrite or remove.
- Could an icon, color, bar, number, or recipe view replace it? If yes, replace it.
- Is it telling the player what to do rather than what is? If yes, delete it.
- Is it visible when the player isn't asking for it? If yes, hide it until they are.

## IV.11a Description style guide

Default is no description. Text earns its place only when its absence would
cause a wrong guess. This is the exception to the general no-prose default above.

- Standard items: name only, with relevant automatic stats. No prose.
- Behavior items: one clause, at most 8 words, for a hidden twist only.
- Mechanic items: one sentence, at most 14 words, followed by a cost from data.
- At most two functional-copy lines: description and cost. Numbers belong in
  automatic stats or cost lines. Shift expands codex copy, at most 50 words.
- Present tense. No adjectives or lore in functional text. Shared keywords
  Burning, Wet, Charged and Sealed are capitalized and defined in one catalog.
- The content validator enforces tier limits and flags Standard items with prose.
  Present tense, useful distinctions and adjectives also require editorial review.
- References: vanilla, Quark, Supplementaries, Farmer's Delight, Create,
  Botania's lexicon split, Ars Nouveau glyph cards and Noita.

Write it, then delete every word that does not change a decision. Empty is valid.
