# Planet profiles and Surveyor works

Date: 2026-11-03. Status: first slice implemented behind dormant gate 128
(`crates/worldgen/src/planet_profile.rs`, `crates/worldgen/src/surveyor.rs`);
the rest is proposed. No new proper nouns: the Surveyors, the Anchor, the
Ballast and the braid are the existing `docs/design/WORLD_LORE.md` glossary.

## Intent

Make each planet a different place before the player lands, in the way a
RimWorld world generation screen does, and make the surface carry the three
strata the lore already has: the Surveyors (stratum III, never explained), the
Anchor's offices (stratum II, the fall), and what people built afterwards
(stratum I, villages and homesteads). The player should be able to tell which
stratum a ruin belongs to from its shape and stone before reading a word.

Goals: one set of dials per planet that every generator reads; monuments that
are unmistakably older than everything else; a surface with more to find and
almost nothing that glows.

Non-goals: a RimWorld map screen with sliders in this slice (the dials are
seed-derived until the save grows an override field); walkable cities with
interiors in this slice.

Anti-goals: lit monuments (torches, glow caps, crystal lamps on an elder
site); purple stone (reserved for the mana blight); any inscription that
explains the Surveyors.

## 1. Planet profile: seven dials from the seed

`planet_profile::profile(seed)` returns seven values in `0..=1`:

| Dial | Reads today | Reads next |
|---|---|---|
| Ocean | class (Wet / Icy / Airless) for the sphere solve | `sphere::PlanetParams` sea level, basin retention |
| Rainfall | nothing yet | `planet::climate` humidity offset, river density |
| Temperature | nothing yet | `planet::climate` base, snowline, ice class |
| Relief | nothing yet | `PlanetParams::relief_m`, ridged range share |
| Ruins | nothing yet | `households` district spacing, `sites` region span |
| Surveyor works | `surveyor` monument chance per 640 m cell | obelisk lines, buried vaults |
| Hazard | nothing yet | blight seep density, `deep` magma share, flood pockets |

Four dials (ocean, rainfall, temperature, relief) are centred: two hashes
averaged, so most planets are temperate and extreme ones are rare, the way
RimWorld's default sliders behave. Three (ruins, surveyor works, hazard) are
flat: a dead planet or a monument-strewn one is as likely as any other.

The profile is a pure function of the seed and is not saved. Wiring
`planet::climate` and the sphere solve to the dials changes existing planet
columns, so each such wiring is its own generator gate. An override field on
`WorldSampleConfig` (the planet screen) needs a save bump and is claimed in
`docs/worldgen/GENERATOR_GATES.md` when it is built; until then the summary
screen shows `PlanetProfile::rows()` read-only.

## 2. Surveyor works: the ziggurats

### Placement

One 640 m cell at most holds one monument; the chance per cell is
0.12 + 0.5 × surveyor works, so a planet carries between one per 5 km² and
one per 1.6 km². The centre jitters up to 200 m inside its cell; jitter plus
the widest footprint and apron (37 m) stays inside the half cell, so a column
asks only its own cell. The site is rejected where the centre or any corner
of the footprint is at or under water, or where the four corners spread more
than 6 m. Nothing is flattened: the monument sits on the lowest corner and a
6 m foundation of the same stone fills the rest.

### Form

Stepped square pyramid, base half-width 24, 27, 30 or 33 m, 4 to 6 tiers of
3 m, inset evenly to a 7 m top tier. On top: a 1.5 m roof slab 10 m across
with a 2 m oculus. Under it: a 7 m sunken chamber, 3 m high, with a relic
plinth (a `SurveyCache` block) at the centre under the oculus. One cleft
stair, 4 m wide, cut straight through the tiers on the facing side from the
base to the chamber floor, with a doorway through the top tier wall. Facing
is one of four turns from the seed.

Stone: the body is `VoidStone` (0.035, 0.032, 0.062: black with the faintest
cold cast; it reads as the lore's "fused basalt, flat to the limit of any
tool"). The lowest 2 m of the bottom tier is `Basalt`, the only part the
weather has got at. The ground for 4 m around the plinth is fused to
`Basalt` at the surface cell only. One course of `CarvedStone` marks runs
round the top tier, every other 1.5 m: the survey mark, three dots and a
line, is the only inscription and it is never read out.

No lamps. 14 m of clear air above the roof so no tree, vine or fungus roots
on it; the oculus is the only light the chamber gets, so the plinth is lit at
noon and dark at night.

### Why this shape

- Stepped and square so it is readable as made at 2 km through the far-field
  horizon, where a column of dark stone against pale ground is one of the
  few silhouettes the renderer keeps.
- Black stone against every biome, because the Surveyors are older than the
  climate: snow, sand, grove moss and grass all sit at its foot.
- A chamber under an oculus gives the first relic a stage without a lamp. The
  oculus also makes the interior legible on the map as a dark square with a
  bright dot at noon.
- A cleft stair instead of a ramp around the outside: the cut shows the
  tiers in section, and the stone goes all the way through.

### Next Surveyor works (proposed, one gate each)

- Obelisk lines: single VoidStone columns 3 × 3 × 12 m every 240 m along a
  great circle between two monuments on the same face; the "survey mark,
  the same spacing as the last one you passed".
- Buried vaults: a chamber 20 m under a monument with a second plinth,
  reached by a shaft from the relic chamber once the relic is lifted
  (a runtime edit, not generation).
- Fused plains: airless-class planets get 200 m discs of Basalt surface
  with no monument, the works of a survey that moved on.

## 3. More structures, almost no lights

The surface should be busier per square kilometre than it is, and darker.

| Stratum | Structures | Stone and light rule |
|---|---|---|
| III Surveyors | ziggurats, obelisk lines, fused plains, vaults | VoidStone and Basalt; no light source blocks, ever |
| II Anchor | the eight offices (existing sites), toll crossings, cisterns, the terminal | Dressed stone, StoneBricks, Limestone; a torch only where a warden still keeps it (one per office at most) |
| I After the fall | villages, homesteads, watch posts, field walls, shrines at road forks | Timber, cobble, plaster; hearths and one lantern per inhabited house, nothing on empty ones |

Rules for every new structure generator:

- Light is evidence of someone present. An empty ruin has none. A village
  has one per lived-in house and the renderer's torch list stays short. The
  `generated_torches` test in `lib.rs` already counts them; a planet at the
  next gate must not raise the count per chunk.
- Each stratum has its own stone. A player who sees VoidStone knows it is
  Surveyor work before any text; dressed stone is the Anchor; timber and
  plaster are the living.
- Structures are placed by the profile. Ruins dial for stratum I and II
  spacing, Surveyor works for stratum III, hazard for which ones are
  flooded, blighted or collapsed.

### Cities (proposed, after villages)

A city is a stratum II office district that stayed inhabited: the
`households` district grid (already 92 and later) grown to 6 × 6 cells with a
wall, two gates on the road, a market square on the office, and the office
building itself at the centre. Interiors come from `architecture` buildings.
Lights: one per occupied house, the gate braziers, nothing else. Each city
needs a trade service and a guard route before it is more than a facade, so
it waits on the biosphere and colony design
(`2026-11-03-biosphere-and-colony-design.md`).

## 4. Art direction refinements

- **Darkness is the default on the surface at night.** The sky, the moon and
  the player's own light do the work. Monuments and ruins are silhouettes.
  A lit window is a destination.
- **Three stones, three ages.** VoidStone, dressed stone, timber. No fourth
  "elder" material; age is shown by what grows on it (grove moss on Anchor
  stone, nothing on Surveyor stone) and by what is missing (roofs).
- **Scale tells age too.** Surveyor works are the largest things on the
  surface. Anchor offices are house-sized. Post-fall buildings are smaller
  than both.
- **No glow on the elder stratum.** The blight glows, the crystal caves
  glow, wands glow. The Surveyors left nothing that needs tending and
  nothing that shines.
- **Inscriptions are marks, not text.** The survey mark is three dots and a
  line. The Anchor writes ledgers. The living write notes. Only the last two
  ever appear as readable text, following the voice rules in `WORLD_LORE.md`.

## 5. Verification

Implemented now (`just test surveyor`, `just test planet_profile`):

- dormant before gate 128 and off planets;
- jitter plus apron inside the half cell;
- chamber, plinth, oculus, roof, skirt and foundation at their cells; no
  torch, glow cap or crystal anywhere in the footprint; the cut course has
  more than eight marks;
- the stair never drops and never rises more than 1 m per metre from the
  foot to the chamber floor;
- the same seed gives the same monument at all three voxel scales;
- profile dials in range, repeatable, all three classes reached within 512
  seeds, ruins and surveyor works independent.

Before the gate wakes: a review pose set (noon and night, 40 m and 2 km,
the stair from the foot) captured before and after per the visual rule, and
a `generated_torches` count on a planet chunk spread at 127 and 128 that
does not change.

## 6. Open owner questions

- Should the relic plinth be a `SurveyCache` (existing container block) or
  a new relic block tied to the relic spells (`sites::next_relic`)? Current
  call: `SurveyCache`, no new `BlockId`, no save bump.
- Ocean dial today only reports a class; wiring it to the sphere solve
  changes every planet's coastline. Wake it with the next planet gate or
  keep the current fixed class?
- The planet summary screen: read-only rows in the loading screen now, or
  wait for the override field and ship sliders once?
