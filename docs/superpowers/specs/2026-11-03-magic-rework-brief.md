# Magic rework brief: blight, circle tool, rune phrases, wand tiers

Date: 2026-11-03. Status: owner brief, condensed from the 2026-11-03 design
note. Proposed design and initial balancing model; no source inspected or
changed when it was written. The circle appearance design
(`2026-11-03-magic-circle-appearance-design.md`) builds on sections 2, 3, 4
and 6 here.

Summary: adopt mana blight, make circle drawing a shared targeting tool,
rebuild spell composition around readable rune phrases. Ordered casting is
the default; shuffling individual runes goes. The wand decides how much magic
can be composed and sustained, the runes decide behaviour, the circle decides
where it acts, blight changes the conditions for supplying it.

Main risk: deeper exploration becomes longer waits for mana. The system
rewards preparation, efficient spells and infrastructure instead.

## 1. Mana blight

Blight reduces mana regeneration only. Stored mana, maximum mana, release
interval, damage and physics are preserved. Sustained spells lose endurance
because replenishment no longer offsets drain.

Two quantities: concentration C (location, 0 to 1) and exposure E (caster,
0 to 1). Recovery multiplier = 1 - 0.75 E, floor 0.25.

Exposure update per second: if E < C, E rises by C / 90; if E > C, E falls
by 1 / 30; capped at C from below and floored at C from above. A C = 1
chamber: 1.00x on arrival, 0.75x at 30 s, 0.50x at 60 s, 0.25x at 90 s.
Clean air recovers fully in 30 s, during ordinary activity. C = 0.25 settles
near 0.81x. Timings are to be tested.

Reference wand: 120 mana, 24 mana/s. At 0.25x: 6 mana/s. A 6-mana spell stays
sustainable at one cast per second; a 36-mana spell once per 6 s after the
reserve. A 30 mana/s channel lasts 20 s clean, 5 s at full blight.

HUD: show the recovery multiplier beside the mana bar whenever it differs
from 1x, with Worsening or Recovering; show bonuses too; hide at exactly
normal; never round a penalty to 1.00x; keep the blight status when another
effect cancels the number; name recovery and cooldown as different
quantities; the readout stays correct with particles reduced.

Spatial structure: depth raises likelihood and severity; sources leak from
damaged magical infrastructure; dense near sources; clean seams and
refuges; routes around the worst; sources that can be sealed, dismantled or
repaired; sites for a protected workshop. Visuals: dark violet volumes with
sparse brighter filaments, contaminated mineral surfaces, pulses near the
source; glow localised, never a uniform purple fog; footing, silhouettes
and telegraphs stay readable.

Countermeasures: preparation (spare charged wands, reservoirs, efficient
spells, protection that slows exposure); temporary relief (a consumable
ward usable without mana); infrastructure (a purification circle or machine
that suppresses C while supplied; source removal is the permanent fix).
PURGE lowers C in its field; E then recovers normally; it does not reset E.
A sanctuary needs an explicit mana source; no energy from refunds or
conversion loops.

Inventory loophole: exposure belongs to the caster and applies to all
readied wands. Two attuned slots receive passive regeneration; packed wands
keep their charge; powered sockets recharge stored wands. Attunement changes
preserve charge, cooldowns and exposure.

## 2. Circle maker

Interaction: pick a spell that accepts a circle target; right-click the
first anchor; move to size; right-click the second anchor to freeze; confirm
to place or cast; cancel spends nothing. The plane locks at the first click.

Modes: Centre to edge (radius = anchor distance); Diameter (radius = half
the anchor distance). Later: three rim points; fixed radius.

Plane: the first clicked voxel face. Later: perpendicular to local gravity;
fixed to the initial view; three points. Capture the local frame at the
first anchor on cylindrical stations and the Manyfold; a planar circle on a
curved floor shows an honest preview.

Edge means the outer boundary; stroke thickens inward. Preview shows outer
diameter, stroke width and height, material, material and mana cost,
obstructed cells and the stepped footprint. Handles stay editable after the
second click. A ray nearly parallel to the plane keeps the last valid
preview and marks the target invalid.

Three actions: Construct ring (places material); Inscribe circle (installs a
rune program and components); Activate circle (starts the effect from its
power source). A stone ring stays a stone ring until inscribed. RING is the
band; DISK is the filled interior; a field above a disk has a bounded,
visible height.

Store radius, width and height in metres; rasterise at the construction
resolution; material cost follows occupied physical volume. Prototype
limits: radius up to 16 m, a maximum edited-cell count, editable chunks only,
one frame per circle, no spanning portals or bodies; the limit names the
setting that exceeded the budget.

## 3. Rune grammar

Three layers: visual components (shared strokes per family), runes
(complete meanings: FIRE, PUSH, RING, SUSTAIN), compound spells (named,
keep their full capacity cost).

Phrase: form then effect, qualifiers attached to a specific part, optional
event-triggered child spells. Roles: Form (Touch, Bolt, Beam, Ring, Disk);
Effect (Fire, Chill, Push, Grip, Place, Purge); Material operand (Stone,
Glass, Copper); Qualifier (Pierce, Sustain, Wider); Event (On first hit,
After delay, On release); Branch operator later (Split, Repeat). A form
grants no effect.

Slot costs, one per named rune:

| Spell | Slots |
|---|---|
| TOUCH then PUSH | 2 |
| BOLT then FIRE | 2 |
| BOLT[PIERCE] then FIRE | 3 |
| RING then PLACE(STONE) | 3 |
| DISK then PURGE[SUSTAIN] | 3 |
| BOLT then ON_FIRST_HIT { RING then FIRE } | 4 |
| BOLT then FIRE then ON_FIRST_HIT { DISK then PUSH } | 5 |

Numeric settings are bounded parameters, not runes. Modifier scope is
visible in the editor and in a text preview. Internally a typed tree; the
rune strip is its presentation. One direct effect per clause at first.

Triggered geometry uses a stored template (radius, width, height,
orientation policy); on collision capture hit point and normal, apply the
parent effect, create the child at the saved transform. Children keep
caster, permissions, cast identity and reserved budget.

World readability: small vocabulary with names beside glyphs; distinct
silhouettes at icon size; codex examples; saved and shared recipes; glyph-only
display optional; circular inscriptions have a start marker and reading
direction. Handwriting optional; combat never depends on drawing.

## 4. Wand tiers

Capacity ladder, testing values: I 4, II 6, III 8, IV 10, V 12. Capacity
counts every installed rune including children; presets do not compress.
Ritual cores have their own capacity from explicit hardware.

Independent constraints: rune capacity, mana capacity, mana recovery,
release interval, channel upkeep, material requirements, execution limits
(cells, bodies, targets, triggers, duration). Keep wand personalities
distinct (efficient compact, large reservoir, high capacity slow discharge,
precise construction, channel-oriented).

## 5. Noita inheritance

Selection modes: Selected spell (default), Sequence (optional), Shuffled
sequence (experimental, whole spells only). No shuffle bonus. Multicast as
explicit bounded branches; always-cast as fixed engravings or sockets; no
implicit wrap; no skipping unaffordable runes; mana restoration needs an
explicit source; cast delay separate from regeneration; limited-use magic
via reagents or charges; displayed order equals execution order.

Insufficient mana: validate the whole spell, determine permitted work,
reserve resources, execute, advance sequence state only on success; a
failure names the missing resource. Prepay a conservative budget for
triggered payloads. Repeated triggers have explicit limits.

Channels end on cancel, payment failure, unequip, tether range exceeded, or
loss of the owning object or authority. A physgun release follows normal
physics.

## 6. Implementation rules

Compile when the inscription changes: typed operations, expanded slot
count, parameter limits, trigger structure, accounting rules, maximum
workload, stable program identity and version. Renderer, tooltip, cost
preview and execution all read the compiled form.

Blight: simulation owns concentration volumes and exposure; the renderer
samples them; cosmetic particles never apply effects; bounded region fields
before any gas simulation; bounded transparency and lighting cost.

Circles: anchors in the owning frame with precise local coordinates,
camera-relative on the GPU; deterministic annulus rasteriser for aligned
rings; one idempotent placement transaction (validation, spending, terrain,
registration); authority validates program, shape, reach, permissions,
resources and chunk versions; freeze the program identity during preview;
network the cast identity and canonical shape, never cursor movement or
client edit lists. Register supporting cells; update integrity on edits,
never flood-fill per frame. Version the spell format; convert old layouts
explicitly; persist cast delay and recharge state.

## 7. First playable slice

One underground route: clean passages, a contaminated chamber, a removable
or repairable source, room for a refuge. Three programs: BOLT then FIRE,
RING then PLACE(STONE), DISK then PURGE[SUSTAIN] (8 slots, so a 4-slot wand
and an 8-slot wand compare). Judge by whether players change equipment,
route, spell choice or infrastructure rather than wait for the bar.
