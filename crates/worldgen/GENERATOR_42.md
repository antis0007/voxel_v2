# Generator 42: shared drainage and environmental initial conditions

New worlds use generator 42 and environmental model 1. Historical generators
retain their previous paths. Save 31 stores environmental ownership, clocks,
pending work, exact water fractions and explicit rate multipliers.

`drainage.rs` computes shared, downhill connections in physical metres, with
ocean outlets and closed basins. Neighboring chunks consume the same plan.
The immutable, bounded cache pins its base relief to generator 41 to avoid
recursive sampling or accidental changes when future generators are added.
Carving, channel water placement and bank substrates consume this plan.

`ecology.rs` supplies initial climate, drainage moisture and distinct species
niches. Species and substrate laws belong in `assets/environment.ron`.
Runtime soil and groundcover processes consume those same definitions;
initial generation does not allocate live fields for every voxel.

The drainage graph is an initial approximation: catchment discharge, recharge,
regional geological history and broad community diversity remain unfinished.
Five groundcover strategies are not acceptance of the complete biome overhaul.
Individual tree development and shared runtime tree construction are still open.

Local checks cover shared connections at negative coordinates, agreement across
physical voxel scales, historical bypass and water in ordinary generated
channels. Native runtime and acceptance evidence is recorded separately in
`docs/plans/2026-10-24-environment-implementation-evidence.md`.
