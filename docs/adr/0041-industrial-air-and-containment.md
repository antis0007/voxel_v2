# Industrial air and containment

Industry extends the existing authoritative factory and environmental paths.
It does not write terrain to represent smoke. Existing worlds retain terrain
generation and receive empty industrial stocks when loaded before save 52.

## Machines and production

Exhaust ducts, chimneys, dust filters, wet scrubbers, chemical tanks and gas
generators use ordinary machine kits, placement admission, item feeds, power
and water networks. Committed work produces emissions; elapsed animation time
and camera visibility do not. Combustion fuel and generator mode change the
emissions. Coke ovens supply combustible byproduct gas. Gas generators consume
that gas and supply the existing electrical network.

Gas moves through adjacent components with bounded pressure equalisation.
Closed duct faces and paused components interrupt flow. A chimney releases
through its actual top opening. Sources without attached containment release
at an open face. Blocked outlets retain stock and eventually stop production.
Venting is explicit. TNT and queued spell blasts rupture components; repairs
consume iron and seal the leak. Damage and leaks survive save/load. Dismantling cannot discard retained gas or captured waste.

Dust filters consume fibre media and capture dust/soot. Wet scrubbers consume
water and electricity and also capture acid fumes. Neither captures fuel gas.
Captured residue has finite capacity and packs into sealed waste items. Local
intake fans remove airborne pollution; ground cleanup collects nearby deposits.
These controls are in the existing machine panel. Waste packaging abstracts
container fabrication and partially filled containers.

## Air and environmental coupling

Air stores integer milligrams by species. Open one-metre volumes aggregate;
obstructed volumes refine to native voxels. Face connectivity samples native
geometry, including saved nonresident edits. Conservative transfers combine
wind advection, concentration-gradient diffusion and species buoyancy.
Terrain edits invalidate connectivity and refine/coarsen retained stocks.

The fixed-step scheduler services air at 30 Hz, independently of rendering.
Each step is bounded by 8,192 air volumes and 8,192 deposit records. Capacity
pressure refuses transfers and retains source stock. No LOD or distance cull
removes simulated pollution. These limits can restrict very large factories;
regional atmospheric aggregation is not implemented.

Dust and soot settle. Canonical local precipitation also deposits pollutants
onto the first intercepted surface, including water. Pumps refuse contaminated
intakes until local cleanup. Deposits suppress crop
and ecological growth. Admitted water-pair transfers carry a conservative
share of the containing metre volume's pollutant stock. Contamination persists
until collected; excavation does not erase it. Air exposure affects players
and creatures; contaminated-water contact also affects players. Creative
immunity and the host's per-player authority remain in effect.

Smoke visuals read actual concentration and retain the existing art direction.
Fuel gas is invisible and uses instrument readings and hazard notifications.
Readings show mg/m³; stored gas and waste use grams. Steam is a separate species.

Buoyancy and atmospheric transformation rates are game parameters in
`assets/industry.ron`. This is a bounded transport model, without a pressure
solver for rooms, oxygen depletion, full atmospheric chemistry, thermal plume
cooling, combustible-air ignition or a general machine-health solver. Steam decay is an
accounted atmospheric transformation, not recovered liquid water. Chemical
refineries and nuclear processes are not added by this change.

## Persistence and multiplayer

Save 52 appends industrial clocks, species stocks, deposition and accounting.
Factory format 14 appends gas, residue, media credit, vent state and retained
combustion fuel. Previous format kind counts remain fixed. Loaders bound counts,
coordinates and stocks. Multiplayer images use the same industrial codec and
validate before publishing. Per-player exposure runs while that player's
authoritative inventory and health are mounted.

## Review

`VOXEL_REVIEW_ENEMIES=industry` plus `VOXEL_ENEMY_REVIEW_SCENE=industry`
installs a disposable native factory scene. `VOXEL_INDUSTRY_REVIEW_CLEAN=1`
pauses its machines for a visual comparison. Ordinary worlds never install it.
Local build and native capture evidence belong in the session artifacts.
No automated tests or hosted CI were requested for this change.
