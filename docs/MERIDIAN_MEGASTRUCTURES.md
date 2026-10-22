# Meridian megastructures — 2026-10-22 increment

The supported Meridian service network and the free-flying megastructures now
have separate physical roles. A local delivery carriage is still a magitech
atmospheric lift. The foundries and twin habitats follow spherical-planet orbits.

| Installation | Physical dimensions | Placement and motion |
| --- | --- | --- |
| Needle courier anchorage | 76 m bus reference diameter | Supported atmospheric corridor |
| Talon freight crown | 600 m habitat diameter | Bearing follows the suspension axis |
| Harbour conservatory crown | 2.4 km habitat diameter; roughly 20 m pressure tube | 18 km above berth, 96 pressure-rim segments |
| Loom orbital foundry | 64 km fabrication truss; repeated sealed industrial districts | Circular orbit, 420 km altitude |
| Continuum twin habitat | Two 36 km cylinders, 4.8 km shell diameter; centres 12.4 km apart | Circular orbit, 700 km altitude; counter-rotating drums |

The carriages retain their 12/32/80 m widths, 1.8 km approach arc and finite
receiver contract. Enlarging a crown adds circumference without enlarging its
pressure-tube windows/decks. The celestial structures retain narrow observation
strips; their model UVs scale with physical dimensions to keep the 5 cm lattice.

## Spherical ephemerides

`world/orbital/megastructures.rs` evaluates six persistent orbital identities
(three foundries, three twin habitats). Seeded epochs and inclinations define
inertial orbital planes, rather than spawning traffic near the camera. Circular
two-body motion uses radius 6,371 km and gravitational parameter
3.986004418e14 m^3/s^2. Angular rate is sqrt(mu / r^3), and the rotating planet
moves beneath each orbit. A world day is 86,400 physical seconds; the normal
game-calendar acceleration therefore accelerates orbital motion too.

Planet-centred positions and observer frames use f64. The voxel surface is a
local chart with east=X, north=-Z and altitude=Y. The observer latitude/longitude
and planet rotation determine a tangent frame into which celestial displacement,
orbital velocity and local vertical are projected. The body attitude follows
that frame; the pressure drums spin about their longitudinal axes at the rate
for 0.3 g at a 2.4 km radius, with equal opposite rotations. The docking spine
remains mechanically separate. Reloads reconstruct poses from saved calendar,
not accumulated integration or a render clock.

A centre/bounding-radius horizon selection retains bodies intersecting the limb.
The celestial fragment path clips their hidden parts against the planet sphere,
uses an eclipse fraction from a planet-shadow calculation, integrates spherical
Rayleigh/aerosol extinction, and retains normal cloud/depth occlusion. The bodies
do not inherit the terrain residency fade or ground shadow cascades. Both scene
camera paths use an infinite projection, converted by the existing GPU path to
reverse-Z; finite terrain culling budgets remain separate. At night,
a sunlit spacecraft can remain visible after the surface enters darkness; an
eclipsed hull loses direct sunlight while its small service lamps remain.

The supported crowns retain their separate material-family-63 16–20 km horizon
fade, plus a 4 km CPU bounding margin. Celestial geometry is tagged independently
and is not cut off at that range. Radio and booking ranges remain unchanged.
The radio ledger exposes a published orbital ephemeris with identifiers, slant
range, bearing, elevation and physical period; this predicts orbits rather than
claiming successful remote reception.

The orbital layer is a circular two-body approximation: no atmospheric drag,
J2 precession, thrust, structural stress or orbital collision simulation. The
terrain renderer remains a local voxel world, not a completed spherical terrain
engine. These sealed exterior models have no boarding, interiors or collision.
Their scale is speculative engineering, not a claim that existing materials can
support the design. No save fields or resources were added; save version stays 28.

## Review

Build offline with CARGO_INCREMENTAL=0 and --profile release-fast --target-dir target.
`tools/Review-MeridianDescent.ps1 -NormalRenderer -MegastructuresOnly` selects real
sunlit passes above terrain/water-level cameras. Add -Night or -Weather storm.
Review clocks run at 1:1 physical seconds for inspectable motion; player clocks
retain their own settings. A habitat pass also captures fixed-camera samples at
45 seconds before/after its peak and a later planet-occulted sample, plus a
chronological drum-rotation frame. Receipts include physical period, altitude,
elevation, slant range and eclipse state. Evidence uses fresh disposable saves
under target/meridian-reviews/. No unit tests or hosted CI are part of this review.

References: [ESA orbit types](https://www.esa.int/Enabling_Support/Space_Transportation/Types_of_orbits)
and [NASA visibility and eclipse](https://www.nasa.gov/missions/station/spot-the-station-frequently-asked-questions/).
