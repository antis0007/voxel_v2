# Generator 77: town interiors and exploration cost

New non-planet worlds retain six settlement parcels and the existing workshop.
The five residential/service buildings use grid-aligned frames, connected roof
bearings, larger interiors and graded entrance paths. Single-storey buildings
have a dividing wall and doorway; the inn and larger homes have upstairs rooms,
a rear staircase, a matching floor opening and a landing rail. Beds, workbenches
and storage occupy wall bays outside the main circulation route.

Settlement frames, floor elevations and entrance-path coefficients are prepared
once in the cached valley plan. New residences use one complete sampler rather
than layering a facade over the legacy interior. Generators 76 and earlier keep
their geometry; existing saves are not migrated. Planet generation remains 74.

Independent of generator version, relic objectives cache their candidate survey
by world configuration, progression and search region. Moving still selects the
nearest candidate and updates distance/bearing. Region or objective changes
rebuild the survey; that first lookup is still synchronous. The map reuses the
objective name allocation while the label is unchanged.

Normal-sized players can automatically step up one coarse voxel, with the
existing collision/headroom checks. Reduced player sizes retain a body-sized
step limit.

No tests or performance benchmarks were run. In-game traversal, visual review
at all voxel scales and measured frame-time improvement remain unverified.
The worldgen crate compiled during the offline application check with incremental
compilation disabled. The application check stopped at an unrelated concurrent
assignment to the absent DayNightCycle.paused field in receiver_review.rs.
