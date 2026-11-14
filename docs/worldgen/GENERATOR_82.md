# Generator 82: Fernwake water court

The existing court at local x=310..333, z=34..63 becomes a finite waterworks.
Its datum remains `habitats_region::floor`; its lowest foundation is three
metres below that datum, within the world's minimum height. Versions through
81 retain the previous geometry. Resident IDs, homes, trade stock and saves
are unchanged.

## Space and consequences

- The central east-west crossing has a two-metre humpback bridge with shallow
  approach treads. Its solid deck covers a culvert with a gravity drop into
  the receiving basin, avoiding dependence on a long pressure search.
- Two stairs join a northern gallery. A southern overlook faces the receiving
  basin; the suspended copper-and-stone distributor gives the court a focal
  silhouette. Torch pools, violet mineral accents and grouped hanging leaves retain
  the existing material language. Each lamp emits exactly one torch block at
  one-, half- and quarter-metre scales rather than a cluster of tiny torches.
  Reservoir lamps stand above the waterline to illuminate the surface; the
  reflective crystals are framed accents, not local light sources.
- The northern reservoir starts full. Two ordinary plank cells form its stop
  at local x=320..322, y=-1..0, z=44..45. Breaching them admits water to the
  culvert and receiving basin. Existing fluid simulation owns all movement.
- The shelf and low service alcove reward a change in water level. Swimming,
  digging, bridging, pumping and rebuilding use ordinary systems. A copper
  fitting can be salvaged from the alcove's back wall.
- This is dormant infrastructure, not an animated or powered distributor.
  There is no hidden puzzle flag, automatic reward or scripted water deletion.

## Verification

`fernwake::waterworks::tests::waterworks_preserves_routes_and_older_generators`
checks three seed configurations, the culvert, reservoir capacity, standing
clearance around both stairs/gallery, and exclusion from older generators.

`simulation::cellular::water_tests::fernwake_stop_breach_moves_finite_water_without_flooding_crossing`
uses the production fluid step in a resident fixture. It checks transfer,
conservation, the crossing, the service shelf and a rebuilt stop after exact
water withdrawal. This does not substitute for live player traversal or a
player-operated pump test.

Local verification on 2026-10-29: geometry/scale checks passed; the resident
repair-and-saved-home regression passed; the production-fluid regression
passed after closing the uncovered metre between the stop and bridge. It
conserves exact water volume and leaves every sampled crossing cell dry after
1,024 production sweeps. The focused fluid run used an unoptimized app crate
with optimized dependencies; its duration is not a simulation benchmark.

Native review cameras use `VOXEL_REVIEW_DUNGEON=place:fernwake-cistern`,
`place:fernwake-reservoir`, and `place:fernwake-receiver`. These place only the
camera; they do not edit terrain or prove reachability.
Enable the benchmark camera hook with `--benchmark-motion=orbit`; the named
review camera then overrides the orbit with a fixed view. `static` skips the
hook and captures spawn, so require a matching `[dungeon-review]` log receipt.
For a faster local Windows review build, the app-only Cargo override
`--config profile.release-fast.package.minecraft_style_voxel_prototype.opt-level=0`
keeps dependency optimization. Pass `-C link-arg=/STACK:33554432` to `cargo
rustc` for this unoptimized executable's main-thread stack. Do not use that
build for frame-rate acceptance or change the project's release settings.

Native review on 2026-10-29 captured the reservoir, crossing and receiver.
The first pass exposed a dark, unreadable reservoir and an unframed mineral
marker. Higher lamps now produce visible surface reflections; masonry relief
frames the north wall. Captures and build hashes are in
`target/fernwake-review/`. This is camera-based visual review, not live combat,
traversal, pump interaction or performance acceptance. The enclosed reservoir
remains dark, and the wider dungeon still needs its full art and gameplay pass.

## Remaining design work

The full Cistern of the Second Rain still calls for controlled low/normal/high
water operation, a functioning distributor linked to settlement production,
the deeper Black Weir, and the Weir Mother's nonlethal encounter. This court
is a connected physical interaction, not completion of that flagship dungeon.
