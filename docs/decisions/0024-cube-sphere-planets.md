# 0024: Worlds are bounded cube-sphere planets

Date: 2026-10-24. Status: decided by the human ("bounded or wrap, whichever is
best if we expand to space"); mechanism chosen here. Resolves the planet-topology
conflict in [0023](0023-worldgen-spec-input.md).

## Context

W-GEO P2 climate needs latitude (insolation, circulation, seasons). Current
worlds are unbounded planes. The roadmap includes space: W08 per-world physics
constants, W09 orbital layer, P10 vessels, landing and multiple planets.

## Options

1. **Cylinder wrap** (latitude on one axis, the other repeats): cheapest, keeps
   infinite play, but never closes into a sphere; orbit view and poles need a
   fake transition. Rejected for space.
2. **Cube-sphere planet:** six square faces, each an ordinary voxel grid
   (`IVec3` chunks, floor division unchanged within a face), projected onto a
   sphere. Latitude/longitude come from the projection. **Chosen.**
3. **True sphere with radial gravity** (one global grid): blocks tilt away from
   local vertical everywhere except the poles. Rejected for a block game.
4. **Latitude from z, no wrap:** climate turns extreme far from spawn; no orbit
   story. Rejected.

## Decision

- A world is a planet of six faces. Within a face, play, meshing and simulation
  stay flat and axis-aligned; "down" is the face normal.
- Coordinates gain a face id. C02's SDD (session 3) defines the face-aware
  coordinate type, the rotation applied when a chunk neighbour lies across a
  face edge, and metric distance across edges. Single-face code paths stay valid.
- Planet size is a per-world constant owned by W08. Proposed default: face edge
  32 km (equator about 128 km), so W-GEO's G0 (8 km) and G1 (64 km window) grids fit.
- Distant terrain bends onto the sphere in rendering (R10 far LOD); orbit view and
  landing reuse the same projection (W09).

## Phasing

| Step | Owner | Milestone |
|---|---|---|
| Face id reserved in coordinate contract | C02 | M1 (SDD only; slice runs on one face) |
| Climate and plates read lat/long from the projection | W11/W12 | M3 |
| One-face world bounded at its edges, face-edge crossing | C02/C03/R10 | M3–M4 |
| Curvature rendering, orbit view, landing | R10/W09 | M6 |

## Consequences and risks

- The 12 face edges and 8 corners are where gravity turns 90 degrees: the C02
  SDD must specify crossing behaviour (player rotation, fluids, bodies) and
  the CA must treat a cross-edge neighbour as a normal neighbour after rotation.
- Infinite worlds end: a new generator version produces planets; older versions
  keep generating their unbounded plane unchanged.
- Projection distortion near cube corners (area ratio about 1.6 with a plain
  gnomonic map) is reduced with an equal-area cube mapping, chosen in W01's SDD.

## Reversal

If face-edge crossing proves unworkable in the C02 SDD critic pass, fall back to
option 1 for the surface and treat orbit as a separate scene; climate code is
unaffected because it only reads latitude.
