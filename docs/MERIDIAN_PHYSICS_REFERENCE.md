# Skyhook physics and surface appearance

Research checked 2026-10-22. Earth-like gravity/radius are an explicit reference,
not an established cosmology for the voxel world. Proposed designs are not
operational infrastructure. Rendering recommendations below are our inferences.

## Recent work and engineering references

- [Landgraf et al., Acta Astronautica, November 2025](https://doi.org/10.1016/j.actaastro.2025.07.018):
  proposes multiple momentum-exchange stages from **already achieved LEO** to
  higher-energy transfers. The abstract reports a two-stage system mass around
  3.4 times payload mass. This is not a demonstrated ground-to-orbit elevator.
  The publisher abstract/highlights were accessible; full text returned HTTP 403.
- [Kazanskii, April 2026 arXiv preprint](https://arxiv.org/html/2604.11221v1):
  studies synchronized rotating tethers and elliptical transfer nodes. It uses
  an approximately 1,000 km lower safe altitude. Its stated limitations exclude
  atmospheric drag, manufacturing, extensibility, fatigue and debris impacts.
  Treat it as an idealized numerical proposal, not a near-surface catcher design.
- [NASA/NIAC Boeing HASTOL Phase II report, 2001](https://www.niac.usra.edu/files/studies/final_report/391Grant.pdf):
  a useful older engineering scale reference. Its evolved system uses a roughly
  **636 km** tether and a hypersonic rendezvous around **150 km altitude**.
  Earlier variants considered about 100 km. Raising rendezvous altitude reduced
  thermal difficulties. Catch/release exchange energy and angular momentum with
  the facility; orbit and spin must be restored by reboost or balancing traffic.
  These are study configurations, not certified hardware specifications.
- [NASA MXER dynamics report, 2006](https://ntrs.nasa.gov/citations/20060051891):
  models an approximately 100 km rotating tether for catching payloads in LEO
  and transferring them to higher-energy trajectories. This is a different
  mission from catching payloads near a planetary surface.

A skyhook rotates and travels in orbit. Matching the low tip's velocity to a
payload does not generally match it to stationary ground. A permanent
Earth-equatorial elevator instead reaches through geostationary altitude, about
[35,786 km](https://ntrs.nasa.gov/citations/20000105202), with the full structure
providing the necessary balance. Neither description fits a stationary 18 km
unpowered platform. Meridian's current structures require their magitech lift.

## What a surface observer would see

The tether can occupy a large fraction of the sky while its hardware remains
unresolved. Those are different measures: **projected length versus width**.
A near-vertical tether viewed directly from below is strongly foreshortened;
a side-on observer may see a long leaning line sweeping across the sky. Project
the endpoints and intermediate samples rather than drawing a fixed sky arc.

Ordinary orbital stations usually resemble moving stars to unaided observers,
not detailed spacecraft. Reflected sunlight favors twilight passes; entering
the planet's shadow removes that contribution. See [NASA's visibility FAQ](https://www.nasa.gov/missions/station/spot-the-station-frequently-asked-questions/).
Clouds should hide the structure, and low elevations add atmospheric attenuation.
Powered windows and beacons are a separate, flux-limited light source.

Subpixel does not mean undetectable. A bright unresolved object can remain a
visible point or line without acquiring a resolved physical width. ESA reports
that the deployed [TSS-1R tether system](https://www.esa.int/Science_Exploration/Human_and_Robotic_Exploration/Tethered_Satellite_System-1_Reflight_USMP-3)
was visible from the ground after its 1996 deployment/break at roughly 296 km
altitude. That does not justify making a thin orbital cable an opaque wide beam.

For a transverse span at approximately constant slant range:

`angle = 2 atan(span / (2 range))`

`pixels at screen centre = screen_height / tan(vertical_FOV / 2) * tan(angle / 2)`

The game's default FOV is **vertical**, not horizontal: 70 degrees. At 1280x900,
the focal length is 642.67 pixels. Resolution, zoom, orientation and slant range
matter. These are geometric extents, not predictions of apparent brightness:

| Object / transverse span | Slant range | Angular diameter | Centre-screen pixels |
| --- | --- | --- | --- |
| 80 m atmospheric deck | 700 m | 6.541 degrees | 73.45 |
| Earlier 420 m archive ring | 18 km | 1.337 degrees | 15.00 |
| Current 2.4 km archive crown | 18 km | 7.628 degrees | 85.69 |
| 100 m orbital station | 400 km | 0.01432 degrees | 0.161 |
| 420 m orbital habitat | 500 km | 0.04813 degrees | 0.540 |
| 1 km orbital habitat | 500 km | 0.1146 degrees | 1.285 |
| 10 km megahabitat | 500 km | 1.146 degrees | 12.85 |
| 1.2 m tether width | 150 km | 0.0004584 degrees | 0.00514 |

The 18 km row is an overhead approximation, not an exact screenshot measurement.
The anchor offset is measured above the berth, itself at least 360 m above
terrain. The archive's full array span also exceeds its 2.4 km ring diameter.
The crown was enlarged in the [megastructure increment](MERIDIAN_MEGASTRUCTURES.md);
its pressure-tube cross section remains approximately human-deck scale.
Moon-sized architecture requires a genuinely enormous structure, a much closer
atmospheric platform, or optical magnification; enlarging a 100 m satellite on
screen would misrepresent its scale.

## Reproducible surface-view reference

`tools/Analyze-SkyhookScale.py` uses only Python's standard library. It writes
`reference.json`, `angular-sizes.csv` and `surface-pass.csv`. Current results are
in `reviews/skyhook-physics-20261022/`. No image or engine capture is substituted
for these calculations.

The illustrative configuration has a circular 700 km hub orbit, a 550 km lower
arm, 100 km counter-arm and 3.5 km/s inertial rotational tip speed. This is our
kinematic example, **not the HASTOL design**. It includes spherical Earth
curvature, a rotating surface observer, and full perspective projection.

- Minimum catcher altitude: 150 km.
- Hub orbit period: 98.62 minutes.
- Inertial spin period: 16.46 minutes; relative to local vertical: 19.75 minutes.
- Hub speed: 7.508 km/s. Near nadir, approximate equatorial ground-relative tip
  speed is still **3.543 km/s**, so a surface receiver cannot simply hook onto it.
- For an observer 100 km north of the orbital plane, the nadir sample puts the
  catcher at 55.55 degrees elevation and the hub at 80.98 degrees: a roughly
  **25.43-degree apparent tether segment** despite tiny unresolved hardware.

The script is not a flexible tether, stress/taper, capture impulse, drag,
thermal, reboost or mass-budget solver. Constant spin is prescribed. Its
rotational tip-acceleration number is not total payload g-load. The current
600-second game day is accelerated presentation, not an Earth sidereal day.

## Engine decisions

Implemented now: one source of truth for deck widths, ring radii, 360 m berth
clearance, 18 km anchorage offset and upper parking fraction. The orbital ledger
identifies the atmospheric service network and active levitation lattice. This
preserves paid deliveries and existing saves rather than silently turning their
fixed ground corridor into a hypersonic interception problem.

The [megastructure increment](MERIDIAN_MEGASTRUCTURES.md) adds a separate celestial
layer: double-precision, circular two-body orbits at 420/700 km, rotating surface
observer frames, calendar-derived phases, planet occultation, a softened eclipse
test and spherical atmospheric extinction. Both scene camera paths now use
infinite projection through the existing reverse-Z GPU path. Terrain residency
and culling remain bounded independently. Orbital habitat drum rotation remains
separate from station attitude and the local service carriage.

Still pending: a true end-over-end orbital tether and matched-speed pickup,
flexible tether/strain dynamics, comprehensive unresolved-point flux filtering,
solar-panel glints and optical zoom. The paid ground contract retains its
separate atmospheric last-mile carrier. True skyhook catches occur high up at
matched vehicle speed and a finite window.

This permits a huge cinematic sweep from real tether length and motion while
keeping near-ground magitech machinery large, inspectable and playable.
