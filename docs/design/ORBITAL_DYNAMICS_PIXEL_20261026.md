# Freight momentum, elastic tethers and orbital pixel materials

Implements the next slice of GAME_BIBLE C-S5 and WORLD_LORE's return-mass ledger.
The existing compressed planet, atmospheric field support and prescribed rotator
spin remain deliberate game abstractions.

## Playable freight

After receiver commissioning, the existing service ledger exposes **Talon Freight**.
Operate the intact receiver within 6 m:

- **Lift Iron** reserves eight ingots (20 kg each) and 160 kg of lift capacity.
  Cargo first rides the atmospheric service, then the hook, then the haul to its hub.
- The completed shipment becomes eight iron in orbital stock. **Lower Iron**
  reserves that stock for the next return window and repays its booked momentum.
- The return becomes ground cargo. **Collect Iron** inserts the entire shipment
  atomically; a full inventory leaves it held at the receiver. Duplicate collection
  cannot grant more material. Destroying a console does not erase its cargo.

There is no arbitrary return-mass currency. Surface iron cannot be credited as
descending orbital mass. It must first exist in orbital stock. Existing finite
commissioning/recovery imports credit momentum when their sealed cargo is unloaded.

The recorded excess angular momentum per kg is
`sqrt(mu * reference_hub_radius) - planet_spin * reference_tip_radius²`.
Each ticket retains its booked impulse. Returning stored cargo repays that same
weighted book value, rather than repricing it to manufacture momentum.
The invariant is `hook_delta + orbital_stock + in_transit - imported = 0`.
Transfers apply gradually; the physical orbital state never teleports to a newly
computed circular radius. Radius, radial velocity and phase propagate through
[Kepler's equation](https://pwg.gsfc.nasa.gov/stargaze/Smotion.htm) between transfers.

The orbital budget reserves clearance corresponding to 20% of nominal cusp altitude.
Upward loads are also refused above 2.5% calculated tether strain. Lowering cargo
remains possible. Surface lift power and thermal accounting are not added here.

## Elastic model

`world/orbital/dynamics.rs` stores three axial and two in-plane transverse modes
per hook. It uses 80 GPa Young's modulus, 1600 kg/m³ fibre density, the braid's
reference area, distributed mass, attached cargo, centrifugal loading and differential
planet gravity. The fixed/free basis follows the standard
[quarter-wave string modes](https://ocw.mit.edu/courses/res-8-005-vibrations-and-waves-problem-solving-fall-2012/pages/problem-solving-videos/standing-waves-part-ii-1/problems/).
Generalized forces are integrated along the line; each mode uses an exact damped
oscillator update under the substep's force. Stored displacement and velocity
produce the rendered 17-point line and strain/elastic-energy readings.

Atmospheric braids have two saved lateral modes driven by the actual weather wind.
Their 18 km reference uses 80 MN pretension and 0.1 m² fibre area, with an exponential
air-column density approximation. Each rendered span preserves both attachment
points and scales the bow by squared span length.

This is a small-strain reduced model around prescribed spin. It is not a fully
coupled spin/line multibody, fracture, collision or N-body solver. Long calendar
jumps have bounded work with event boundaries at departure, cusp and arrival;
they do not replay one update for every skipped second. Rewinding the calendar
does not undo cargo ownership or refund transactions.

## Pixel materials

The shared `orbital_pixel_surface` shader handles atmospheric and celestial parts.
It preserves the world's 5 cm lattice and uses connected clusters, limited material
ramps, 32-cell plates and ordered thermal channels. Macro plate pixels occupy eight
lattice cells (0.4 m); the structural plate repeat is 12.8 m. Plate-scale variation,
filtered joints and tiny fittings have separate distance filtering. The material
mean remains consistent as details disappear.

Shared linear-light ceramic, steel, graphite, copper and cargo ramps derive from
`assets/pixel_palette.txt`. Lighting and atmosphere remain continuous; no global
posterization or pixel filter is applied. This follows ART_DIRECTION.md and
PIXEL_ART_DIRECTION.md's SNES/GBA material craft, not literal framebuffer bit depth.
Cylinders and rims have unwrapped local metric UVs. Megastructure fittings and UVs
are authored near the origin before their world transform, preserving precision.

## Persistence and evidence

Save format **39** stores cargo ownership, booked/applied impulses, totals, orbital
state, elastic mode displacement/velocity/energy and atmospheric modes together.
The writer magic and version both identify 39. Versions through 38 load with empty
freight ledgers and initialize their physical state at the saved calendar. Decoding
rejects invalid numbers, stocks, conics and inconsistent momentum ledgers.

Native evidence is under `target/orbital-physics-pixel-20261026/`. Every candidate
has a hash, launch arguments and disposable save directory. `native` is marked
provisional because the initial review clock was not isolated. `native-r2` and
`release-native` verified the production receiver actions and checkpoint codec.
The release-native run restored the complete encoded orbital state byte-for-byte
mid-flight (`progression_reload_exact=true`, version 39).

Its full round trip returned eight iron exactly once. Lift capacity moved from
77,513.016689 kg to 77,353.016689 kg and back. The final ledger residual was
0.000015259 kg m²/s, floating-point roundoff relative to the booked impulses.
Strain and elastic energy remained finite and changed with the physical updates;
their saved mid-flight values restored exactly. The native fixture invokes the
production dynamics explicitly while holding its clock, rather than running an
ordinary live flight session. These captures are not performance measurements.

The `pixel-final` native close-ups reviewed the loaded grapple and elevator
anchorage after the final material-filter change. Broad plate tones and joints
remain readable while smaller fasteners filter independently. Screenshots are
`pixel-grapple.png` and `pixel-elevator.png`; this is a focused orbital art review,
not acceptance of every biome or lighting condition. Returned freight is claimed
one eight-ingot crate at a time, so accumulated stock does not require enough
inventory space for the entire stockpile at once. This final inventory correction
was build-checked after the native round-trip review.

Build logs: `target/orbital-physics-pixel-build-final.log`,
`target/orbital-physics-pixel-release.log` and the final material-filter build
`target/orbital-physics-pixel-release-final.log`. No tests or hosted CI were added
or run. Terrain generation and unrelated shared-checkout work are preserved.
