# Magic circles: voxel appearance, emergence and inscribed runes

Date: 2026-11-03. Status: proposed design, extends the magic rework brief
(`2026-11-03-magic-rework-brief.md`: mana blight, two-anchor circle tool,
rune phrases). Nothing here is implemented; the brief's section 2 (circle
maker), 3 (rune grammar), 4 (wand tiers) and 6 (implementation rules) are
taken as given. The rune set itself is not yet fixed; slice step 2 starts
with the seven runes the brief's examples use (Bolt, Ring, Disk, Fire, Push,
Place, Purge) plus Stone, Pierce, Sustain and On first hit.

## Intent

A circle is the one piece of magic the player shapes directly in the world.
It should look like the spell it holds: a player who can read runes reads a
circle from ten metres away, and a player who cannot still sees which band is
the program, where it starts, whether it is fed, and where it is broken.

Goals: a circle is real voxels; its runes are the installed program, not
decoration; its state (inert, armed, running, starved, broken) is readable
without the HUD; it rises from the ground rather than appearing.

Non-goals: freehand glyph drawing; circles on arbitrary planes in the first
slice; a general decal system.

Anti-goals: a purple fog ring (purple is the blight's colour); a glowing disc
that hides the floor; runes that are random filler.

## Anatomy

All dimensions are metres, rasterised at the construction resolution the
player chose (`VoxelScalePreset`: 1, 0.5, 0.25 m). The inscription is a
groove model at every scale (the lintel lettering already cuts 0.045 m
grooves on metre cells), so a circle carries its program at any grid. Scale
only changes how round the band is and how fine the start mark and nodes
are; the slot map is stored in metres along the circumference, not per
cell, so it is scale-independent.

```
          start mark (notch + one taller cell)
               v
      . . # # # # # # . .        outer band     width w_o
    . # # o o o o o o # # .      rune track     width w_r  (glyph slots)
  . # o . . . . . . . o # .      inner band     width w_i  (child spell)
  . # o .   core   . o # .       disk           interior, usually untouched
  . # o . . . . . . . o # .
    . # # o o o o o o # # .      cardinal nodes at 0, 90, 180, 270 degrees
      . . # # # # # # . .
```

| Part | Role | Default | Notes |
|---|---|---|---|
| Outer band | the physical RING target | w_o = 0.5 m, height 0.25 m | the handles set its outer edge; stroke thickens inward (brief section 2) |
| Rune track | glyph slots for the top-level phrase | w_r = 0.75 m | only at radius 2 m or more; below that the track merges into the band and the tool refuses to inscribe |
| Inner band | one band per nested clause | w_i = 0.5 m | `ON_FIRST_HIT { ... }` children occupy the next band inward, so tree depth is visible as concentric rings |
| Cardinal nodes | material and mana sockets, integrity anchors | 1 cell proud of the band | four at most; a functional circle registers these first |
| Start mark | reading origin | notch cut into the outer edge plus one cell raised | always at the first anchor's bearing from the centre, so the player picks where the phrase starts |
| Core | the socket for a ritual core (brief section 4) | centre cell | plain floor until a core is installed |
| Disk | DISK target | not placed | the preview shows its outline only |

Minimum readable radius for an inscribed circle is 2.0 m (one slot per
rune, see below); the tool reports "Radius 1.5 m: band only, no inscription"
rather than squeezing glyphs.

## Three layers, three owners

| Layer | Owner | What it is |
|---|---|---|
| Footprint | terrain edits (`World` edit path, undoable) | real cells: band, nodes, start mark. Ordinary stone until inscribed. Breaks like stone. |
| Inscription | circle registry (new; stored in a side file beside the world save, keyed to the world, so no `SAVE_VERSION` bump in the first slice) | the compiled program plus a cell map: which cell holds which glyph slot. Rendered as grooves on the band's top face, the same technique as the hell gate lintel lettering (`world/hell_gate/appearance.rs`): shaded cuboids 2 mm proud, shades of the band's own material, no paint. |
| Activity | renderer, per frame from registry state | emission along the grooves, the running pulse, the core light. Cosmetic only; never applies effects (brief section 6). |

The footprint is the only layer that costs material. The inscription is the
only layer that carries meaning. The activity layer is the only one allowed
to animate. Keeping them apart means a broken cell breaks exactly one slot,
and an LOD mesh never counts as geometry.

The groove approach was chosen over an inlay material because it needs no new
`BlockId`, so no save bump and no new `materials.ron` row for every glow
colour. The cost is that the band's top face must be exposed: a rune under
a placed block is simply hidden, and the integrity check treats a covered
slot as "obscured", not broken.

## Runes in the band

Slots. The rune track is divided into glyph slots of pitch p = 1.0 m along
the mid-circumference. Slot count n = floor(2 pi r_mid / p). A 2.0 m circle
has 11 slots, a 4 m circle 24, the 16 m cap 99.

Fill order. Slot 0 is at the start mark. Runes fill clockwise as seen from
the side the plane normal points to (above, for a floor). The phrase is laid
out form, effect, operand, qualifiers, exactly the editor order; nothing is
reordered for looks. Qualifiers are drawn as small marks inside the slot of
the rune they modify (a diacritic), not as their own slot, so scope is
visible in the stone the same way it is in the editor.

Empty slots. Unused slots carry a ruled tick, a single groove the height of
the glyph baseline. Ticks are visibly not glyphs: no player should mistake
spare capacity for an unknown rune. A circle with 24 slots and a 3-rune
program shows 3 glyphs and 21 ticks; that is correct and tells the player
the circle has room.

Nested clauses. An event rune in the outer track opens a bracket glyph; the
child phrase lives on the next band inward, starting at the same bearing as
its event rune. Depth two means three bands. Deeper nesting is refused by
the tool with the bands it would need.

Glyph form. A 7 by 9 cell mask per rune at 0.045 m per cell on the quarter
grid (0.3 m by 0.4 m glyph), from a text sheet like
`assets/tools/sprites16.txt`, so the same mask drives the inventory icon,
the editor strip and the carved groove. Families share strokes as the brief
asks: heat has three rising cuts, motion a hook, containment an enclosure.
The sheet is the single source of glyph shapes; the lintel letter table is
not reused.

Reading from the other side. A floor circle seen from below (through glass or
a hole) is mirrored; that is physically honest and nothing is done about it.
A wall circle reads clockwise from the side it was placed from. The start
mark's taller cell makes the origin findable from any side.

## Coming out of the ground

The authority commits the footprint in one transaction at confirm (brief
section 6). Emergence is cosmetic and client-side; it changes what is drawn,
not what is solid. This was chosen over a staged commit because a staged
commit is several transactions, each of which can fail or race another
player's edit, and because the blight and channel rules need one cast
identity.

The cost is up to one second where a cell is solid but drawn as rising. The
heave stage covers it: the ground bulges before any stone shows, so the
player reads "something is pushing up here" and does not walk into air.

Sequence, from the commit tick, deterministic from cast tick plus positional
hash (no wall time):

| Stage | Window | What is drawn |
|---|---|---|
| Heave | 0 to 0.3 s | the original floor cells over the footprint are drawn lifted 0.05 m with a dust puff; nothing of the ring yet |
| Rise | 0.3 to 1.2 s | each band cell slides up from 0.3 m below to its rest height; start order is slot order, so the ring sweeps clockwise from the start mark; the four nodes rise last |
| Etch | 1.2 to 1.8 s | grooves cut in, one slot per 0.05 s in reading order; a short-lived bright line runs ahead of the etch |
| Settle | 1.8 to 2.0 s | emission fades to the inert level; dust clears |

The rising cell is drawn as a model cuboid of the band material with a soil
collar (darker base 0.1 m) that shrinks as it rises. The first slice is
single-player; the registry entry carries the commit tick from the start so
that, when multiplayer picks this up, another player's circle plays the same
sweep from the replicated tick. The netcode and multiplayer crates are not
touched by this slice.

Removal (undo, decommission) plays the rise in reverse over 0.6 s and ends
on the restored cells.

## States and what each looks like

| State | Grooves | Band | Core | Rule |
|---|---|---|---|---|
| Preview | ghost outline, obstructed cells hatched | translucent, correct material tint | socket outline | shows outer diameter, width, height, cost, slot count |
| Inert | none (plain ring) or dark cuts | stone | plain | a ring with no program is just a ring |
| Inscribed | dark cuts | stone | socket dark | program installed, nothing feeding it |
| Armed | cuts lit steady at a low level | stone | socket lit | fed and valid, waiting for activation |
| Running | a pulse travels the track in reading order; speed = upkeep / capacity, so a hungry channel runs fast | stone | one light | the pulse is the only moving thing; no particles on the band |
| Starved | lit level flickers between inert and armed at 2 Hz | stone | dim | payment failed this tick; the HUD says which resource |
| Broken | the two slots either side of the gap glow at the armed level and nothing else does | missing cells | dark | integrity update, not a scan; the registry knows the gap |
| Obscured | hidden under blocks | stone | per above | counts as intact |

Emission colour is per aspect family (heat warm amber, chill pale cyan,
motion white, containment green-gold, purge blue-white). Purple and violet
are reserved for the blight so a lit circle and a blight seep never read as
the same thing. Levels stay continuous (no stepped brightness; see the
no-posterization rule) and at most one dynamic light per circle, at the
core. Groove emission is unlit emissive geometry, never a light.

## Rendering budget

- A circle is one model per registry entry, rebuilt only when its state or
  cell map changes, not per frame. The running pulse is a shader-side
  parameter (phase along the track), so no rebuild while it runs.
- LOD: beyond 48 m draw the band only (footprint cells are terrain anyway)
  and a single emissive ring strip; between 16 and 48 m draw grooves without
  diacritics; inside 16 m full detail. Switch at fixed distances.
- Cap: 64 registry models resident, nearest first; the rest are terrain only.
- Emission goes through the existing material-kind path the torch flame
  uses (`mesh/memory.rs`, kind 40), extended with a hue parameter, not a new
  per-material emissive scalar. If that path cannot take a hue, a second
  kind for rune glow is the fallback; either way one shader change,
  captured before and after.

## Integrity

The registry stores, per circle: frame (a world id plus an optional body
id, reserved now and always empty in this slice so a later body-frame
circle needs no format change), centre, normal, radius, widths, scale, the
commit tick, the compiled program id and version, the slot map in metres
along the circumference, and node cells. The cell map is derived from these
on load, never stored. On any terrain edit touching a registered cell the
edit path notifies the registry (a hash set lookup per edited cell; the edit
path already visits each cell). The registry marks the slot and recomputes
the state. No flood fill, no per-frame scan.

A broken circle keeps its program. Replacing the missing cells with the same
material repairs it and re-etches those slots with the etch animation only.

## Verification

Tests (`just test circle`):
- annulus rasteriser: symmetric under the four rotations and both mirrors,
  8-connected band at every radius 1 to 16 m at all three scales, identical
  output for the same inputs (no wall time, no `Instant`).
- slot count and fill order for radius 2, 4, 16 m; start slot at the first
  anchor's bearing; mirrored reading on a ceiling circle.
- nested clause goes to the inner band at the event rune's bearing; depth
  three is refused with the band count in the error.
- emergence schedule is a pure function of (commit tick, cell index).
- integrity: removing one band cell marks one slot; covering it marks none.

Captures (visual rule, before and after from the same pose): a review world
with one 4 m circle at each scale, inscribed with BOLT then FIRE, RING then
PLACE(STONE) and DISK then PURGE[SUSTAIN]; poses at 3 m, 12 m and 40 m, at
noon and at night; one frame mid-rise and one mid-etch from a fixed tick.
The 40 m night frame must show the band and start mark but not individual
glyphs; the 3 m frame must show diacritics.

## First slice

1. Annulus rasteriser plus the two-anchor tool from the brief, floor only,
   quarter grid, band and start mark, no runes. Terrain commit, undoable.
2. Registry with slot map and the inscription layer: grooves from the glyph
   sheet, inert and inscribed states, obscured handling.
3. Emergence and etch animations.
4. Armed, running, starved and broken states wired to the channel rules.
5. Inner band for one nesting level.

Calls made in this document, for the owner to check: grooves not an inlay
material; commit once then cosmetic rise; slot pitch 1.0 m and the 2.0 m
minimum inscribed radius; ticks for empty slots; concentric bands for
nesting; per-aspect hue with purple reserved for blight; one light per
circle; registry in a side file (no save bump) with a reserved body-frame
field; single-player first slice with the commit tick stored for later
replication; grooves at every voxel scale.

Open owner questions: whether the start mark should be the first anchor's
bearing (current call) or always face the caster at confirm; whether a
circle below 2 m should refuse inscription outright (current call) or carry
an unreadable program on the model layer.
